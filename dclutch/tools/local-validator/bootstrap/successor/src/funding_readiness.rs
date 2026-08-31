//! Chain-authenticated planning for the post-founding Resolution readiness walk.
//!
//! This module does not own Market or funding semantics. It acquires one
//! finalized account snapshot, supplies it to the canonical Resolution Core V3
//! builders, and routes only combinations those builders authenticate. The
//! reports remain the semantic owners of instruction and economic facts.

use std::collections::{BTreeMap, BTreeSet};

use dclutch_market_core_codec::{CoreState, Phase, Readiness};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionActivateFundReportV1,
    ResolutionActivateFundSnapshotV1, ResolutionCoreOperatorErrorV3, ResolutionCreateFundReportV3,
    ResolutionCreateFundSnapshotV3, ResolutionVerifyFundReadyReportV3,
    ResolutionVerifyFundReadySnapshotV3, build_resolution_activate_fund_v1,
    build_resolution_create_fund_v3, build_resolution_verify_fund_ready_v3,
    validate_resolution_create_fund_report_v3, validate_resolution_verify_fund_ready_report_v3,
};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    model::SuccessorPlan,
    plan::pubkey,
    rpc::{Rpc, RpcAccount},
};

/// One finalized Registry record pair used by the readiness builders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessRecordCoordinatesV1 {
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

/// Explicit Market-owned coordinates that are not derivable from the release plan.
///
/// Program, ProgramData, activation-cache, System, and sysvar addresses come
/// from `SuccessorPlan` or canonical SDK constants. `recovery_policy` is absent
/// only when SourceMaterial carries no recovery walk; in that case the
/// canonical builders deliberately reuse the SourceMaterial pair in the two
/// omitted-policy snapshot positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessCoordinatesV1 {
    pub(crate) market: Pubkey,
    pub(crate) source_material: FundingReadinessRecordCoordinatesV1,
    pub(crate) capability_manifest: FundingReadinessRecordCoordinatesV1,
    pub(crate) recovery_policy: Option<FundingReadinessRecordCoordinatesV1>,
    pub(crate) source_state: Pubkey,
    pub(crate) funding_ledger: Pubkey,
    pub(crate) beneficiary: Pubkey,
    pub(crate) activation_receipt: Pubkey,
}

/// Exact optional System prepayment required before one protocol instruction.
///
/// The caller supplies the fee payer when it composes the transfer. A zero
/// amount means no transfer instruction is emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessPrepayV1 {
    pub(crate) destination: Pubkey,
    pub(crate) lamports: u64,
}

/// Exact account sets a durable exterior uses around one readiness mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessAccountSetsV1 {
    /// Accounts writable in the canonical protocol instruction, in frame order.
    pub(crate) protocol_writable: Vec<Pubkey>,
    /// Minimal ordered poststate set that selects the next readiness route.
    pub(crate) completion: Vec<Pubkey>,
}

/// Exact unsigned instruction geometry, excluding an as-yet-unselected payer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessGeometryV1 {
    pub(crate) protocol_account_count: usize,
    pub(crate) protocol_unique_account_count: usize,
    pub(crate) protocol_writable_count: usize,
    pub(crate) protocol_signer_count: usize,
    pub(crate) protocol_data_len: usize,
    pub(crate) transaction_instruction_count_without_compute_budget: usize,
    /// Program, frame accounts, and prepay destination, but not the unknown payer.
    pub(crate) transaction_lock_count_without_payer: usize,
}

/// One authenticated instruction and the exact facts needed to journal it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessInstructionPlanV1<T> {
    pub(crate) report: T,
    pub(crate) prepay: Option<FundingReadinessPrepayV1>,
    pub(crate) accounts: FundingReadinessAccountSetsV1,
    pub(crate) geometry: FundingReadinessGeometryV1,
}

/// The next honest action selected by one finalized chain snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FundingReadinessPlanV1 {
    Create(FundingReadinessInstructionPlanV1<ResolutionCreateFundReportV3>),
    Activate(FundingReadinessInstructionPlanV1<ResolutionActivateFundReportV1>),
    Accept(FundingReadinessInstructionPlanV1<ResolutionVerifyFundReadyReportV3>),
    /// Founding/Ready is durably visible on chain. The embedded idempotent
    /// report reauthenticates all completion facts without inventing a second DTO.
    Complete(FundingReadinessInstructionPlanV1<ResolutionVerifyFundReadyReportV3>),
    /// Open/Consumed is durably visible on chain AND the Source resolution
    /// state is live: the readiness this Market was staged from is consumed and
    /// the Market is Open. Every adjacent builder rightly refuses (the fund
    /// exists, nothing tops up, and verify-ready demands the Founding phase the
    /// Market has left), so the only honest plan is the terminal one - there is
    /// nothing left to drive.
    ///
    /// The live Source state is load-bearing and is checked, not assumed:
    /// `(Open, Consumed)` with no buildable route is ALSO the prestate of an
    /// atomically founded Market that has not created its fund yet. See
    /// `authenticate_consumed_by_founding_v1`.
    ConsumedByFounding,
}

/// One semantic readiness plan and address-table accounts observed in the
/// same finalized RPC response. Routing remains non-authoritative, but the v0
/// compiler may not relabel stale table bytes with the semantic observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessRoutedPlanV1 {
    pub(crate) plan: FundingReadinessPlanV1,
    pub(crate) routing_tables: Vec<ObservedAccount>,
}

impl FundingReadinessPlanV1 {
    pub(crate) fn route_name(&self) -> &'static str {
        match self {
            Self::Create(_) => "create",
            Self::Activate(_) => "activate",
            Self::Accept(_) => "accept",
            Self::Complete(_) => "complete",
            Self::ConsumedByFounding => "consumed-by-founding",
        }
    }
}

/// Acquire and route one bounded, same-finalized readiness snapshot.
pub(crate) fn plan_funding_readiness_from_rpc_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
) -> Result<FundingReadinessPlanV1> {
    Ok(
        plan_funding_readiness_with_routing_from_rpc_v1(rpc, plan, coordinates, minimum_slot, &[])?
            .plan,
    )
}

/// Return every address the shared founding ALT must contain before Open.
pub(crate) fn funding_readiness_routing_addresses_v1(
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
) -> Result<Vec<Pubkey>> {
    FundingReadinessFrameV1::from_plan(plan, coordinates)?.distinct_observation_addresses()
}

/// Acquire semantic accounts and caller-selected routing tables in one exact
/// finalized snapshot, then construct the canonical next readiness action.
pub(crate) fn plan_funding_readiness_with_routing_from_rpc_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
    routing_table_keys: &[Pubkey],
) -> Result<FundingReadinessRoutedPlanV1> {
    let frame = FundingReadinessFrameV1::from_plan(plan, coordinates)?;
    let semantic_addresses = frame.distinct_observation_addresses()?;
    let mut addresses = semantic_addresses.clone();
    for key in routing_table_keys {
        if addresses.contains(key) {
            return Err(refusal(
                "funding-readiness routing table aliased a semantic account or another table",
            ));
        }
        addresses.push(*key);
    }
    let (slot, accounts) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let mut accounts = accounts;
    let routing_accounts = accounts.split_off(semantic_addresses.len());
    let snapshot = FundingReadinessObservationV1::new(observation, semantic_addresses, accounts)?;
    let routing_tables = routing_table_keys
        .iter()
        .copied()
        .zip(routing_accounts)
        .map(|(key, account)| {
            let account = account.ok_or_else(|| {
                refusal("funding-readiness finalized snapshot omitted a routing table")
            })?;
            authenticate_routing_table_v1(observation, key, account)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(FundingReadinessRoutedPlanV1 {
        plan: plan_funding_readiness_from_observation_v1(frame, &snapshot)?,
        routing_tables,
    })
}

fn authenticate_routing_table_v1(
    observation: Observation,
    key: Pubkey,
    account: RpcAccount,
) -> Result<ObservedAccount> {
    let table = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("funding-readiness routing table bytes were invalid"))?;
    if account.owner != lookup_table_program::ID
        || account.executable
        || table.meta.authority.is_some()
        || table.meta.deactivation_slot != u64::MAX
        || table.meta.last_extended_slot >= observation.slot
        || table.addresses.is_empty()
    {
        return Err(refusal(
            "funding-readiness routing table was not exact, frozen, active, and activated",
        ));
    }
    Ok(ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FundingReadinessFrameV1 {
    coordinates: FundingReadinessCoordinatesV1,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    resolution_program: Pubkey,
    resolution_programdata: Pubkey,
}

impl FundingReadinessFrameV1 {
    fn from_plan(plan: &SuccessorPlan, coordinates: FundingReadinessCoordinatesV1) -> Result<Self> {
        let frame = Self {
            coordinates,
            activation_cache: pubkey(&plan.activation)?,
            registry_program: pubkey(&plan.registry.program_id)?,
            core_program: pubkey(&plan.core.program_id)?,
            core_programdata: pubkey(&plan.core.programdata_id)?,
            resolution_program: pubkey(&plan.resolution.program_id)?,
            resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        };
        frame.distinct_observation_addresses()?;
        Ok(frame)
    }

    fn distinct_observation_addresses(&self) -> Result<Vec<Pubkey>> {
        let coordinates = self.coordinates;
        let mut addresses = vec![
            coordinates.market,
            self.activation_cache,
            self.registry_program,
            self.core_program,
            self.core_programdata,
            self.resolution_program,
            self.resolution_programdata,
            coordinates.source_material.raw,
            coordinates.source_material.staging,
            coordinates.capability_manifest.raw,
            coordinates.capability_manifest.staging,
            coordinates.source_state,
            coordinates.funding_ledger,
            coordinates.beneficiary,
            coordinates.activation_receipt,
            sysvar::clock::ID,
            sysvar::rent::ID,
            system_program::ID,
        ];
        if let Some(recovery) = coordinates.recovery_policy {
            addresses.push(recovery.raw);
            addresses.push(recovery.staging);
        }
        let distinct = addresses.iter().copied().collect::<BTreeSet<_>>();
        if distinct.len() != addresses.len() {
            return Err(refusal(
                "funding-readiness coordinates aliased two semantic frame positions",
            ));
        }
        if addresses.len() > 20 {
            return Err(refusal(
                "funding-readiness observation exceeded its measured 20-account bound",
            ));
        }
        Ok(addresses)
    }

    fn recovery_pair(self) -> FundingReadinessRecordCoordinatesV1 {
        self.coordinates
            .recovery_policy
            .unwrap_or(self.coordinates.source_material)
    }
}

struct FundingReadinessObservationV1 {
    accounts: BTreeMap<Pubkey, ObservedAccount>,
}

impl FundingReadinessObservationV1 {
    fn new(
        observation: Observation,
        addresses: Vec<Pubkey>,
        accounts: Vec<Option<RpcAccount>>,
    ) -> Result<Self> {
        if observation.finality != Finality::Finalized || addresses.len() != accounts.len() {
            return Err(refusal(
                "funding-readiness RPC snapshot was not one complete finalized observation",
            ));
        }
        let accounts = addresses
            .into_iter()
            .zip(accounts)
            .map(|(key, account)| {
                let account = account.unwrap_or_else(vacant_rpc_account_v1);
                (
                    key,
                    ObservedAccount {
                        observation,
                        key,
                        owner: account.owner,
                        lamports: account.lamports,
                        executable: account.executable,
                        data: account.data,
                    },
                )
            })
            .collect();
        Ok(Self { accounts })
    }

    fn account(&self, key: Pubkey) -> Result<ObservedAccount> {
        self.accounts
            .get(&key)
            .cloned()
            .ok_or_else(|| refusal(format!("funding-readiness snapshot omitted {key}")))
    }
}

fn vacant_rpc_account_v1() -> RpcAccount {
    RpcAccount {
        lamports: 0,
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
        data: Vec::new(),
    }
}

fn plan_funding_readiness_from_observation_v1(
    frame: FundingReadinessFrameV1,
    snapshot: &FundingReadinessObservationV1,
) -> Result<FundingReadinessPlanV1> {
    let coordinates = frame.coordinates;
    let recovery = frame.recovery_pair();
    let verify_snapshot = ResolutionVerifyFundReadySnapshotV3 {
        market: snapshot.account(coordinates.market)?,
        activation_cache: snapshot.account(frame.activation_cache)?,
        registry_program: snapshot.account(frame.registry_program)?,
        core_program: snapshot.account(frame.core_program)?,
        core_programdata: snapshot.account(frame.core_programdata)?,
        resolution_program: snapshot.account(frame.resolution_program)?,
        resolution_programdata: snapshot.account(frame.resolution_programdata)?,
        source_material: snapshot.account(coordinates.source_material.raw)?,
        source_material_staging: snapshot.account(coordinates.source_material.staging)?,
        capability_manifest: snapshot.account(coordinates.capability_manifest.raw)?,
        capability_manifest_staging: snapshot.account(coordinates.capability_manifest.staging)?,
        source_state: snapshot.account(coordinates.source_state)?,
        funding_ledger: snapshot.account(coordinates.funding_ledger)?,
        beneficiary: snapshot.account(coordinates.beneficiary)?,
        clock_sysvar: snapshot.account(sysvar::clock::ID)?,
        rent_sysvar: snapshot.account(sysvar::rent::ID)?,
        activation_receipt: snapshot.account(coordinates.activation_receipt)?,
        recovery_policy: snapshot.account(recovery.raw)?,
        recovery_policy_staging: snapshot.account(recovery.staging)?,
    };
    let create_snapshot = ResolutionCreateFundSnapshotV3 {
        market: verify_snapshot.market.clone(),
        activation_cache: verify_snapshot.activation_cache.clone(),
        registry_program: verify_snapshot.registry_program.clone(),
        core_program: verify_snapshot.core_program.clone(),
        core_programdata: verify_snapshot.core_programdata.clone(),
        resolution_program: verify_snapshot.resolution_program.clone(),
        resolution_programdata: verify_snapshot.resolution_programdata.clone(),
        source_material: verify_snapshot.source_material.clone(),
        source_material_staging: verify_snapshot.source_material_staging.clone(),
        capability_manifest: verify_snapshot.capability_manifest.clone(),
        capability_manifest_staging: verify_snapshot.capability_manifest_staging.clone(),
        source_destination: verify_snapshot.source_state.clone(),
        funding_ledger: verify_snapshot.funding_ledger.clone(),
        rent_sysvar: verify_snapshot.rent_sysvar.clone(),
        system_program: snapshot.account(system_program::ID)?,
        recovery_policy: verify_snapshot.recovery_policy.clone(),
        recovery_policy_staging: verify_snapshot.recovery_policy_staging.clone(),
    };

    let market = CoreState::decode(&verify_snapshot.market.data)
        .map_err(|error| refusal(format!("funding-readiness Market: {error:?}")))?;
    let create = build_resolution_create_fund_v3(&create_snapshot);
    let activate = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot.clone(),
        system_program: snapshot.account(system_program::ID)?,
    });
    let accept = build_resolution_verify_fund_ready_v3(&verify_snapshot);
    // Kept beside the selection rather than printed past it. A stderr line
    // that a terminal route then overrode is how the post-Open suffix reported
    // "nothing left to drive" over three builders that had all refused.
    let builder_refusals = format!(
        "create={:?} activate={:?} accept={:?}",
        create.as_ref().err(),
        activate.as_ref().err(),
        accept.as_ref().err()
    );
    let selection = select_authenticated_route_v1(
        market.phase,
        market.readiness,
        create.is_ok(),
        activate.as_ref().ok().map(activation_shape_v1),
        accept.is_ok(),
    )?;

    match selection {
        AuthenticatedRouteV1::Create => {
            let report = create.map_err(|error| operator_refusal("CreateFund", error))?;
            validate_resolution_create_fund_report_v3(&report)
                .map_err(|error| operator_refusal("CreateFund validation", error))?;
            let prepay = FundingReadinessPrepayV1 {
                destination: coordinates.source_state,
                lamports: report.source_top_up_lamports,
            };
            let instruction = report.instruction.clone();
            Ok(FundingReadinessPlanV1::Create(instruction_plan_v1(
                &instruction,
                report,
                Some(prepay),
                vec![
                    coordinates.market,
                    coordinates.source_state,
                    coordinates.funding_ledger,
                ],
            )))
        }
        AuthenticatedRouteV1::Activate => {
            let report = activate.map_err(|error| operator_refusal("ActivateFund", error))?;
            let prepay = FundingReadinessPrepayV1 {
                destination: coordinates.activation_receipt,
                lamports: report.receipt_top_up_lamports,
            };
            let instruction = report.instruction.clone();
            Ok(FundingReadinessPlanV1::Activate(instruction_plan_v1(
                &instruction,
                report,
                Some(prepay),
                vec![
                    coordinates.source_state,
                    coordinates.funding_ledger,
                    coordinates.beneficiary,
                    coordinates.activation_receipt,
                ],
            )))
        }
        AuthenticatedRouteV1::Accept => {
            let report = accept.map_err(|error| operator_refusal("VerifyFundReady", error))?;
            validate_resolution_verify_fund_ready_report_v3(&report)
                .map_err(|error| operator_refusal("VerifyFundReady validation", error))?;
            let instruction = report.instruction.clone();
            Ok(FundingReadinessPlanV1::Accept(instruction_plan_v1(
                &instruction,
                report,
                None,
                completion_accounts_v1(coordinates),
            )))
        }
        AuthenticatedRouteV1::ConsumedByFounding => {
            authenticate_consumed_by_founding_v1(
                &verify_snapshot.source_state,
                frame.resolution_program,
                &builder_refusals,
            )?;
            return Ok(FundingReadinessPlanV1::ConsumedByFounding);
        }
        AuthenticatedRouteV1::Complete => {
            let report = accept.map_err(|error| operator_refusal("Ready completion", error))?;
            validate_resolution_verify_fund_ready_report_v3(&report)
                .map_err(|error| operator_refusal("Ready completion validation", error))?;
            let instruction = report.instruction.clone();
            Ok(FundingReadinessPlanV1::Complete(instruction_plan_v1(
                &instruction,
                report,
                None,
                completion_accounts_v1(coordinates),
            )))
        }
    }
}

/// Require the fact `ConsumedByFounding` asserts, instead of inferring it.
///
/// `(Open, Consumed)` with no adjacent route buildable is reached two ways and
/// only one of them is terminal:
///
///  - the readiness walk ran and finished, leaving a live
///    `SourceResolutionStateV2` in `Primary` and an Active ledger — nothing
///    adjacent remains, which is the honest terminal; and
///  - the atomic founding committed `Founding + Prepaid -> Open + Consumed` in
///    one step and the fund was never created at all, which Core admits on
///    purpose (`resolution_fund_prestate_admissible`: `Open + Consumed` is
///    where an atomically founded Market still has to create its Resolution
///    Fund). Here everything downstream of `CreateFund` refuses for the same
///    reason — the Source destination is vacant — and calling that terminal
///    reports a founding as complete while its Market has no resolution route.
///
/// The whole difference is the Source state, so this reads it. The refusal
/// carries all three builder errors because the head one is the diagnosis:
/// `RecoveryWalkUnavailable` says the material bought an ordered recovery walk
/// and Core's Q2 weld will not mint a fund over it, ever, at any later slot.
fn authenticate_consumed_by_founding_v1(
    source_state: &ObservedAccount,
    resolution_program: Pubkey,
    builder_refusals: &str,
) -> Result<()> {
    if source_state.owner == resolution_program && !source_state.data.is_empty() {
        return Ok(());
    }
    Err(refusal(format!(
        "funding-readiness is not terminal: the Market is Open with readiness Consumed, but its \
         Source resolution state {} is vacant ({} bytes, owner {}), so the atomic founding \
         consumed no staged readiness and every readiness route refused: {builder_refusals}",
        source_state.key,
        source_state.data.len(),
        source_state.owner,
    )))
}

fn completion_accounts_v1(coordinates: FundingReadinessCoordinatesV1) -> Vec<Pubkey> {
    vec![
        coordinates.market,
        coordinates.source_state,
        coordinates.funding_ledger,
        coordinates.activation_receipt,
        coordinates.beneficiary,
    ]
}

fn instruction_plan_v1<T>(
    instruction: &Instruction,
    report: T,
    prepay: Option<FundingReadinessPrepayV1>,
    completion: Vec<Pubkey>,
) -> FundingReadinessInstructionPlanV1<T> {
    let protocol_writable = instruction
        .accounts
        .iter()
        .filter(|account| account.is_writable)
        .map(|account| account.pubkey)
        .collect::<Vec<_>>();
    let geometry = instruction_geometry_v1(instruction, prepay);
    FundingReadinessInstructionPlanV1 {
        report,
        prepay,
        accounts: FundingReadinessAccountSetsV1 {
            protocol_writable,
            completion,
        },
        geometry,
    }
}

fn instruction_geometry_v1(
    instruction: &Instruction,
    prepay: Option<FundingReadinessPrepayV1>,
) -> FundingReadinessGeometryV1 {
    let protocol_keys = instruction
        .accounts
        .iter()
        .map(|account| account.pubkey)
        .chain(core::iter::once(instruction.program_id))
        .collect::<BTreeSet<_>>();
    let mut transaction_keys = protocol_keys.clone();
    let has_prepay = prepay.is_some_and(|value| value.lamports != 0);
    if let Some(prepay) = prepay {
        transaction_keys.insert(prepay.destination);
    }
    FundingReadinessGeometryV1 {
        protocol_account_count: instruction.accounts.len(),
        protocol_unique_account_count: protocol_keys.len(),
        protocol_writable_count: instruction
            .accounts
            .iter()
            .filter(|account| account.is_writable)
            .count(),
        protocol_signer_count: instruction
            .accounts
            .iter()
            .filter(|account| account.is_signer)
            .count(),
        protocol_data_len: instruction.data.len(),
        transaction_instruction_count_without_compute_budget: 1 + usize::from(has_prepay),
        transaction_lock_count_without_payer: transaction_keys.len(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationShapeV1 {
    Fresh,
    Replay,
}

fn activation_shape_v1(report: &ResolutionActivateFundReportV1) -> ActivationShapeV1 {
    if report.receipt_top_up_lamports == 0 && report.expected_beneficiary_credit_lamports == 0 {
        ActivationShapeV1::Replay
    } else {
        ActivationShapeV1::Fresh
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthenticatedRouteV1 {
    Create,
    Activate,
    Accept,
    Complete,
    ConsumedByFounding,
}

fn select_authenticated_route_v1(
    phase: Phase,
    readiness: Readiness,
    create: bool,
    activate: Option<ActivationShapeV1>,
    accept: bool,
) -> Result<AuthenticatedRouteV1> {
    let route = match (phase, readiness, create, activate, accept) {
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            true,
            None,
            false,
        ) => AuthenticatedRouteV1::Create,
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            false,
            Some(ActivationShapeV1::Fresh),
            false,
        ) => AuthenticatedRouteV1::Activate,
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            false,
            Some(ActivationShapeV1::Replay),
            true,
        ) => AuthenticatedRouteV1::Accept,
        (Phase::Founding, Readiness::Ready, false, None, true) => AuthenticatedRouteV1::Complete,
        // The atomic founding's success poststate: readiness consumed into an
        // Open Market, no adjacent route buildable. Terminal, not mixed.
        (Phase::Open, Readiness::Consumed, false, None, false) => {
            AuthenticatedRouteV1::ConsumedByFounding
        }
        _ => {
            return Err(refusal(format!(
                "funding-readiness account states were mixed or did not select one adjacent route: phase={phase:?} readiness={readiness:?} create={create} activate={activate:?} accept={accept}",
            )));
        }
    };
    Ok(route)
}

fn operator_refusal(label: &str, error: ResolutionCoreOperatorErrorV3) -> Error {
    refusal(format!("{label} chain authentication refused: {error:?}"))
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(message)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use solana_address_lookup_table_interface::state::LookupTableMeta;

    use super::*;

    fn key() -> Pubkey {
        Pubkey::new_unique()
    }

    fn coordinates() -> FundingReadinessCoordinatesV1 {
        FundingReadinessCoordinatesV1 {
            market: key(),
            source_material: FundingReadinessRecordCoordinatesV1 {
                raw: key(),
                staging: key(),
            },
            capability_manifest: FundingReadinessRecordCoordinatesV1 {
                raw: key(),
                staging: key(),
            },
            recovery_policy: Some(FundingReadinessRecordCoordinatesV1 {
                raw: key(),
                staging: key(),
            }),
            source_state: key(),
            funding_ledger: key(),
            beneficiary: key(),
            activation_receipt: key(),
        }
    }

    fn frame() -> FundingReadinessFrameV1 {
        FundingReadinessFrameV1 {
            coordinates: coordinates(),
            activation_cache: key(),
            registry_program: key(),
            core_program: key(),
            core_programdata: key(),
            resolution_program: key(),
            resolution_programdata: key(),
        }
    }

    fn routing_account(
        authority: Option<Pubkey>,
        deactivation_slot: u64,
        last_extended_slot: u64,
    ) -> RpcAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                deactivation_slot,
                last_extended_slot,
                last_extended_slot_start_index: 0,
                authority,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(vec![key(), key()]),
        };
        RpcAccount {
            lamports: 1,
            owner: lookup_table_program::ID,
            executable: false,
            rent_epoch: 0,
            data: table.serialize_for_tests().expect("table bytes"),
        }
    }

    #[test]
    fn routing_table_requires_frozen_active_activated_exact_account() {
        let observation = Observation {
            slot: 9,
            unix_timestamp: 1,
            finality: Finality::Finalized,
        };
        assert!(
            authenticate_routing_table_v1(observation, key(), routing_account(None, u64::MAX, 8),)
                .is_ok()
        );
        for account in [
            routing_account(Some(key()), u64::MAX, 8),
            routing_account(None, 7, 8),
            routing_account(None, u64::MAX, 9),
            RpcAccount {
                owner: key(),
                ..routing_account(None, u64::MAX, 8)
            },
        ] {
            assert!(authenticate_routing_table_v1(observation, key(), account).is_err());
        }
    }

    #[test]
    fn adjacent_state_routes_cover_vacant_primary_active_and_ready() {
        for (phase, readiness) in [
            (Phase::Founding, Readiness::Prepaid),
            (Phase::Open, Readiness::Consumed),
        ] {
            assert_eq!(
                select_authenticated_route_v1(phase, readiness, true, None, false).unwrap(),
                AuthenticatedRouteV1::Create,
            );
            assert_eq!(
                select_authenticated_route_v1(
                    phase,
                    readiness,
                    false,
                    Some(ActivationShapeV1::Fresh),
                    false,
                )
                .unwrap(),
                AuthenticatedRouteV1::Activate,
            );
            assert_eq!(
                select_authenticated_route_v1(
                    phase,
                    readiness,
                    false,
                    Some(ActivationShapeV1::Replay),
                    true,
                )
                .unwrap(),
                AuthenticatedRouteV1::Accept,
            );
        }
        assert_eq!(
            select_authenticated_route_v1(Phase::Founding, Readiness::Ready, false, None, true,)
                .unwrap(),
            AuthenticatedRouteV1::Complete,
        );
    }

    #[test]
    fn consumed_accept_remains_explicit_until_a_durable_journal_finalizes_it() {
        assert_eq!(
            select_authenticated_route_v1(
                Phase::Open,
                Readiness::Consumed,
                false,
                Some(ActivationShapeV1::Replay),
                true,
            )
            .unwrap(),
            AuthenticatedRouteV1::Accept,
        );
        assert!(
            select_authenticated_route_v1(Phase::Open, Readiness::Consumed, false, None, true,)
                .is_err(),
            "an unchanged atomic Market cannot prove a no-write Accept finalized",
        );
    }

    /// The terminal that must be read off chain state, not inferred from the
    /// absence of a buildable route.
    ///
    /// `(Open, Consumed)` with all three builders refusing is exactly what an
    /// atomically founded Market looks like BEFORE its fund exists, and it is
    /// also what it looks like after the walk finished. Selecting the terminal
    /// on the tuple alone reported the first as the second: the campaign
    /// printed `Open Market ... (20 steps)` over
    /// `create=RecoveryWalkUnavailable activate=Funding accept=Funding`, and
    /// only run.py's six-mutation order check downstream noticed.
    #[test]
    fn consumed_by_founding_requires_a_live_source_state_not_a_vacant_one() {
        let resolution = key();
        let live = ObservedAccount {
            observation: Observation {
                slot: 9,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            key: key(),
            owner: resolution,
            lamports: 1_000_000,
            executable: false,
            data: vec![7; 32],
        };
        assert!(
            authenticate_consumed_by_founding_v1(&live, resolution, "create=None").is_ok(),
            "a Source state the Resolution program owns is the terminal's evidence",
        );

        let vacant = ObservedAccount {
            owner: system_program::ID,
            lamports: 0,
            data: Vec::new(),
            ..live.clone()
        };
        let refusal = authenticate_consumed_by_founding_v1(
            &vacant,
            resolution,
            "create=Some(RecoveryWalkUnavailable) activate=Some(Funding) accept=Some(Funding)",
        )
        .expect_err("a vacant Source state is not a consumed readiness");
        let message = refusal.to_string();
        assert!(
            message.contains("RecoveryWalkUnavailable"),
            "the refusal must carry the head builder error, not just its own summary: {message}",
        );
        assert!(
            message.contains("is vacant"),
            "the refusal must name the fact it read: {message}",
        );

        // Owned by Resolution but still empty, and the right owner over the
        // wrong program: both are absence, not a consumed readiness.
        let empty = ObservedAccount {
            data: Vec::new(),
            ..live.clone()
        };
        assert!(authenticate_consumed_by_founding_v1(&empty, resolution, "").is_err());
        let foreign = ObservedAccount {
            owner: key(),
            ..live.clone()
        };
        assert!(authenticate_consumed_by_founding_v1(&foreign, resolution, "").is_err());
    }

    #[test]
    fn mixed_and_nonadjacent_builder_successes_refuse() {
        let mixed = [
            (true, Some(ActivationShapeV1::Fresh), false),
            (true, None, true),
            (false, Some(ActivationShapeV1::Fresh), true),
            (false, Some(ActivationShapeV1::Replay), false),
            (false, None, false),
        ];
        for (create, activate, accept) in mixed {
            assert!(
                select_authenticated_route_v1(
                    Phase::Founding,
                    Readiness::Prepaid,
                    create,
                    activate,
                    accept,
                )
                .is_err(),
            );
        }
        assert!(
            select_authenticated_route_v1(Phase::Founding, Readiness::Ready, true, None, true,)
                .is_err(),
        );
    }

    #[test]
    fn every_observed_semantic_position_is_nonaliased_and_bounded() {
        let with_recovery = frame();
        assert_eq!(
            with_recovery
                .distinct_observation_addresses()
                .unwrap()
                .len(),
            20,
        );
        let mut without_recovery = frame();
        without_recovery.coordinates.recovery_policy = None;
        assert_eq!(
            without_recovery
                .distinct_observation_addresses()
                .unwrap()
                .len(),
            18,
        );

        let mut aliased = frame();
        aliased.coordinates.activation_receipt = aliased.coordinates.source_state;
        assert!(aliased.distinct_observation_addresses().is_err());
        let mut program_alias = frame();
        program_alias.resolution_programdata = program_alias.core_programdata;
        assert!(program_alias.distinct_observation_addresses().is_err());
    }

    #[test]
    fn geometry_counts_protocol_locks_without_smuggling_in_a_payer() {
        let program = key();
        let readonly = key();
        let writable = key();
        let instruction = Instruction {
            program_id: program,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new_readonly(readonly, false),
                solana_sdk::instruction::AccountMeta::new(writable, false),
            ],
            data: vec![1, 2, 3],
        };
        let zero = instruction_geometry_v1(
            &instruction,
            Some(FundingReadinessPrepayV1 {
                destination: writable,
                lamports: 0,
            }),
        );
        assert_eq!(zero.protocol_account_count, 2);
        assert_eq!(zero.protocol_unique_account_count, 3);
        assert_eq!(zero.protocol_writable_count, 1);
        assert_eq!(zero.protocol_signer_count, 0);
        assert_eq!(zero.protocol_data_len, 3);
        assert_eq!(zero.transaction_instruction_count_without_compute_budget, 1);
        assert_eq!(zero.transaction_lock_count_without_payer, 3);

        let funded = instruction_geometry_v1(
            &instruction,
            Some(FundingReadinessPrepayV1 {
                destination: key(),
                lamports: 1,
            }),
        );
        assert_eq!(
            funded.transaction_instruction_count_without_compute_budget,
            2
        );
        assert_eq!(funded.transaction_lock_count_without_payer, 4);
    }
}
