//! One chain-authenticated selector for the Source funding-readiness walk.
//!
//! RPC, wallets, journals, submission, and poststate reads live outside this
//! crate. Callers provide one bounded finalized account observation. The
//! canonical Core/Resolution builders authenticate it and this crate selects
//! exactly one adjacent action or refuses the mixed frame.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

mod wire;

pub use wire::{
    derive_source_close_detail_json_v1, derive_source_readiness_base_json_v1,
    derive_source_readiness_detail_json_v1, derive_source_readiness_recovery_json_v1,
    derive_source_terminal_base_json_v1, derive_source_terminal_detail_json_v1,
    derive_source_terminal_product_json_v1, plan_funding_readiness_json_v1,
    plan_source_close_fund_json_v1, plan_source_terminal_json_v1,
    verify_source_close_receipt_json_v1,
};

use dclutch_market_core_codec::{CoreState, Phase, Readiness};
use dclutch_resolution_core_v3_operator::{
    Finality, ObservedAccount, ResolutionActivateFundReportV1, ResolutionActivateFundSnapshotV1,
    ResolutionAdmitTerminalReportV3, ResolutionAdmitTerminalSnapshotV3,
    ResolutionCloseFundSnapshotV3, ResolutionCoreOperatorErrorV3, ResolutionCreateFundReportV3,
    ResolutionCreateFundSnapshotV3, ResolutionDirectCloseFundReportV1,
    ResolutionVerifyFundReadyReportV3, ResolutionVerifyFundReadySnapshotV3,
    build_resolution_activate_fund_v1, build_resolution_admit_terminal_v3,
    build_resolution_create_fund_v3, build_resolution_direct_close_fund_v1,
    build_resolution_verify_fund_ready_v3, source_closure_receipt_rent_lamports_v1,
    validate_resolution_admit_terminal_report_v3, validate_resolution_create_fund_report_v3,
    validate_resolution_verify_fund_ready_report_v3,
};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

/// Maximum semantic accounts in one readiness observation.
pub const FUNDING_READINESS_MAX_OBSERVATION_ACCOUNTS_V1: usize = 20;
/// Exact semantic account count in one terminal-admission observation.
pub const SOURCE_TERMINAL_OBSERVATION_ACCOUNTS_V1: usize = 21;
/// Maximum semantic account count in one Source close observation.
pub const SOURCE_CLOSE_FUND_OBSERVATION_ACCOUNTS_V1: usize = 21;

/// One finalized Registry record and its vacant staging cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReadinessRecordCoordinatesV1 {
    /// Final record address.
    pub raw: Pubkey,
    /// Canonical vacant staging cursor.
    pub staging: Pubkey,
}

/// Exact Market-owned coordinates for the Source readiness walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReadinessCoordinatesV1 {
    /// Core Market.
    pub market: Pubkey,
    /// Finalized SourceMaterialV3 pair.
    pub source_material: FundingReadinessRecordCoordinatesV1,
    /// Finalized capability-manifest pair.
    pub capability_manifest: FundingReadinessRecordCoordinatesV1,
    /// Optional recovery-policy pair; absent reuses the SourceMaterial pair.
    pub recovery_policy: Option<FundingReadinessRecordCoordinatesV1>,
    /// Canonical Source resolution state.
    pub source_state: Pubkey,
    /// Canonical Resolution-owned funding subset ledger.
    pub funding_ledger: Pubkey,
    /// Immutable Market rent beneficiary.
    pub beneficiary: Pubkey,
    /// Canonical activation receipt.
    pub activation_receipt: Pubkey,
}

/// Release-selected program frame surrounding the Market coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReadinessFrameV1 {
    /// Market-owned coordinates.
    pub coordinates: FundingReadinessCoordinatesV1,
    /// Market-selected Registry activation cache.
    pub activation_cache: Pubkey,
    /// Current Registry program.
    pub registry_program: Pubkey,
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
    /// Current Resolution program.
    pub resolution_program: Pubkey,
    /// Current Resolution ProgramData.
    pub resolution_programdata: Pubkey,
}

/// Exact derived frame for one terminal Source admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTerminalFrameV1 {
    /// Existing release-selected Source funding frame.
    pub readiness: FundingReadinessFrameV1,
    /// Canonical terminal certificate derived from Source state.
    pub certificate: Pubkey,
    /// Finalized Product root record.
    pub product_raw: Pubkey,
    /// Vacant Product-root staging cursor.
    pub product_staging: Pubkey,
    /// Finalized Product-selected ResultDomain record.
    pub result_domain_raw: Pubkey,
    /// Vacant ResultDomain staging cursor.
    pub result_domain_staging: Pubkey,
    /// Finalized Product-selected Portfolio record.
    pub portfolio_raw: Pubkey,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: Pubkey,
}

/// Exact derived frame for one direct Source funding close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCloseFundFrameV1 {
    /// Existing release-selected Source funding frame.
    pub readiness: FundingReadinessFrameV1,
    /// Core-admitted terminal certificate.
    pub certificate: Pubkey,
    /// Canonical vacant or finalized closure receipt.
    pub closure_receipt: Pubkey,
}

/// Optional System transfer required immediately before the protocol act.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReadinessPrepayV1 {
    /// Transfer destination.
    pub destination: Pubkey,
    /// Exact lamports, possibly zero for an idempotent replay.
    pub lamports: u64,
}

/// Accounts an exterior must journal and reacquire after execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingReadinessAccountSetsV1 {
    /// Writable protocol accounts in exact frame order.
    pub protocol_writable: Vec<Pubkey>,
    /// Minimal ordered poststate set selecting the next adjacent action.
    pub completion: Vec<Pubkey>,
}

/// Exact unsigned instruction geometry before a payer is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReadinessGeometryV1 {
    /// Protocol account-meta count.
    pub protocol_account_count: usize,
    /// Distinct protocol lock count including the program.
    pub protocol_unique_account_count: usize,
    /// Writable protocol account-meta count.
    pub protocol_writable_count: usize,
    /// Signer protocol account-meta count.
    pub protocol_signer_count: usize,
    /// Protocol instruction-data length.
    pub protocol_data_len: usize,
    /// Transfer plus protocol instructions, excluding compute budget.
    pub transaction_instruction_count_without_compute_budget: usize,
    /// Distinct locks excluding the unknown payer.
    pub transaction_lock_count_without_payer: usize,
}

/// One authenticated report plus the exact exterior facts needed to execute it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingReadinessInstructionPlanV1<T> {
    /// Canonical builder report.
    pub report: T,
    /// Required prepayment, when the route owns one.
    pub prepay: Option<FundingReadinessPrepayV1>,
    /// Writable and poststate account sets.
    pub accounts: FundingReadinessAccountSetsV1,
    /// Measured unsigned geometry.
    pub geometry: FundingReadinessGeometryV1,
}

/// The single honest next action selected by finalized state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FundingReadinessPlanV1 {
    /// Create the Source state and bind the pending funding ledger.
    Create(FundingReadinessInstructionPlanV1<ResolutionCreateFundReportV3>),
    /// Activate the pending Resolution funds.
    Activate(FundingReadinessInstructionPlanV1<ResolutionActivateFundReportV1>),
    /// Have Core accept the active funding as ready.
    Accept(FundingReadinessInstructionPlanV1<ResolutionVerifyFundReadyReportV3>),
    /// Reauthenticate an already Ready Market.
    Complete(FundingReadinessInstructionPlanV1<ResolutionVerifyFundReadyReportV3>),
    /// Atomic founding consumed readiness and left a live Source state.
    ConsumedByFounding,
}

/// The only honest terminal-admission state selected by a finalized frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceTerminalPlanV1 {
    /// Submit the permissionless terminal admission.
    Admit(FundingReadinessInstructionPlanV1<ResolutionAdmitTerminalReportV3>),
    /// The exact terminal certificate is already admitted.
    Complete(FundingReadinessInstructionPlanV1<ResolutionAdmitTerminalReportV3>),
}

/// The only honest adjacent Source funding-close state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCloseFundPlanV1 {
    /// Prepay the exact remaining receipt rent, then reacquire finalized state.
    Prepay {
        /// Shared finalized observation.
        observation: dclutch_resolution_core_v3_operator::Observation,
        /// Canonical receipt destination.
        receipt: Pubkey,
        /// Existing harmless receipt lamports.
        current_lamports: u64,
        /// Exact receipt rent reserve.
        exact_rent_lamports: u64,
        /// Exact System transfer amount.
        top_up_lamports: u64,
    },
    /// Execute the signer-free V7 direct Resolution close.
    ///
    /// Boxed, and the reason is a measurement rather than a style: the plan is
    /// 712 bytes against `Prepay`'s 80, so an unboxed enum cost every
    /// `SourceCloseFundPlanV1` -- including the small variant -- the full 712
    /// on the stack and in every move. The enum has no consumer outside this
    /// crate, so the indirection is invisible at the boundary.
    Close(Box<FundingReadinessInstructionPlanV1<ResolutionDirectCloseFundReportV1>>),
}

impl SourceCloseFundPlanV1 {
    /// Stable route identifier used by journals and generated clients.
    pub const fn route_name(&self) -> &'static str {
        match self {
            Self::Prepay { .. } => "prepay",
            Self::Close(_) => "close",
        }
    }
}

impl SourceTerminalPlanV1 {
    /// Stable route identifier used by journals and generated clients.
    pub const fn route_name(&self) -> &'static str {
        match self {
            Self::Admit(_) => "admit",
            Self::Complete(_) => "complete",
        }
    }
}

impl FundingReadinessPlanV1 {
    /// Stable route identifier used by journals and generated clients.
    pub const fn route_name(&self) -> &'static str {
        match self {
            Self::Create(_) => "create",
            Self::Activate(_) => "activate",
            Self::Accept(_) => "accept",
            Self::Complete(_) => "complete",
            Self::ConsumedByFounding => "consumed-by-founding",
        }
    }
}

/// Bounded deterministic planning refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingReadinessErrorV1(String);

impl FundingReadinessErrorV1 {
    /// Stable diagnostic text for an exterior refusal.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for FundingReadinessErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FundingReadinessErrorV1 {}

/// Return the exact bounded account frame, refusing aliases before any RPC.
pub fn funding_readiness_observation_addresses_v1(
    frame: &FundingReadinessFrameV1,
) -> Result<Vec<Pubkey>, FundingReadinessErrorV1> {
    let coordinates = frame.coordinates;
    let mut addresses = vec![
        coordinates.market,
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        frame.resolution_program,
        frame.resolution_programdata,
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
    if addresses.iter().copied().collect::<BTreeSet<_>>().len() != addresses.len() {
        return Err(refusal(
            "funding-readiness coordinates aliased two semantic frame positions",
        ));
    }
    if addresses.len() > FUNDING_READINESS_MAX_OBSERVATION_ACCOUNTS_V1 {
        return Err(refusal(
            "funding-readiness observation exceeded its measured 20-account bound",
        ));
    }
    Ok(addresses)
}

/// Select exactly one readiness route from one complete finalized observation.
pub fn plan_funding_readiness_v1(
    frame: &FundingReadinessFrameV1,
    accounts: &[ObservedAccount],
) -> Result<FundingReadinessPlanV1, FundingReadinessErrorV1> {
    let addresses = funding_readiness_observation_addresses_v1(frame)?;
    if accounts.len() != addresses.len() {
        return Err(refusal(
            "funding-readiness snapshot omitted or added frame accounts",
        ));
    }
    let first = accounts
        .first()
        .ok_or_else(|| refusal("funding-readiness snapshot is empty"))?
        .observation;
    if first.finality != Finality::Finalized
        || accounts.iter().any(|account| account.observation != first)
    {
        return Err(refusal(
            "funding-readiness RPC snapshot was not one complete finalized observation",
        ));
    }
    let mut observed = BTreeMap::new();
    for (expected, account) in addresses.iter().zip(accounts) {
        if account.key != *expected || observed.insert(account.key, account.clone()).is_some() {
            return Err(refusal(
                "funding-readiness snapshot reordered, substituted, or repeated a frame account",
            ));
        }
    }
    plan_from_map_v1(frame, &observed)
}

/// Return the exact terminal-admission frame, refusing aliases before RPC.
pub fn source_terminal_observation_addresses_v1(
    frame: &SourceTerminalFrameV1,
) -> Result<Vec<Pubkey>, FundingReadinessErrorV1> {
    let readiness = frame.readiness;
    let addresses = vec![
        readiness.coordinates.market,
        readiness.activation_cache,
        readiness.registry_program,
        readiness.core_program,
        readiness.core_programdata,
        readiness.resolution_program,
        readiness.resolution_programdata,
        readiness.coordinates.source_material.raw,
        readiness.coordinates.source_material.staging,
        readiness.coordinates.capability_manifest.raw,
        readiness.coordinates.capability_manifest.staging,
        readiness.coordinates.source_state,
        readiness.coordinates.funding_ledger,
        frame.certificate,
        sysvar::rent::ID,
        frame.product_raw,
        frame.product_staging,
        frame.result_domain_raw,
        frame.result_domain_staging,
        frame.portfolio_raw,
        frame.portfolio_staging,
    ];
    if addresses.iter().copied().collect::<BTreeSet<_>>().len() != addresses.len() {
        return Err(refusal(
            "Source terminal coordinates aliased two semantic frame positions",
        ));
    }
    Ok(addresses)
}

/// Select terminal admission or exact completion from one finalized frame.
pub fn plan_source_terminal_v1(
    frame: &SourceTerminalFrameV1,
    accounts: &[ObservedAccount],
) -> Result<SourceTerminalPlanV1, FundingReadinessErrorV1> {
    let addresses = source_terminal_observation_addresses_v1(frame)?;
    if addresses.len() != SOURCE_TERMINAL_OBSERVATION_ACCOUNTS_V1
        || accounts.len() != addresses.len()
    {
        return Err(refusal(
            "Source terminal snapshot omitted or added frame accounts",
        ));
    }
    let observation = accounts
        .first()
        .ok_or_else(|| refusal("Source terminal snapshot is empty"))?
        .observation;
    if observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(refusal(
            "Source terminal RPC snapshot was not one complete finalized observation",
        ));
    }
    let mut observed = BTreeMap::new();
    for (expected, account) in addresses.iter().zip(accounts) {
        if account.key != *expected || observed.insert(account.key, account.clone()).is_some() {
            return Err(refusal(
                "Source terminal snapshot reordered, substituted, or repeated a frame account",
            ));
        }
    }
    let account = |key: Pubkey| {
        observed
            .get(&key)
            .cloned()
            .ok_or_else(|| refusal(format!("Source terminal snapshot omitted {key}")))
    };
    let readiness = frame.readiness;
    let snapshot = ResolutionAdmitTerminalSnapshotV3 {
        market: account(readiness.coordinates.market)?,
        activation_cache: account(readiness.activation_cache)?,
        registry_program: account(readiness.registry_program)?,
        core_program: account(readiness.core_program)?,
        core_programdata: account(readiness.core_programdata)?,
        resolution_program: account(readiness.resolution_program)?,
        resolution_programdata: account(readiness.resolution_programdata)?,
        source_material: account(readiness.coordinates.source_material.raw)?,
        source_material_staging: account(readiness.coordinates.source_material.staging)?,
        capability_manifest: account(readiness.coordinates.capability_manifest.raw)?,
        capability_manifest_staging: account(readiness.coordinates.capability_manifest.staging)?,
        source_state: account(readiness.coordinates.source_state)?,
        funding_ledger: account(readiness.coordinates.funding_ledger)?,
        certificate: account(frame.certificate)?,
        rent_sysvar: account(sysvar::rent::ID)?,
        product_raw: account(frame.product_raw)?,
        product_staging: account(frame.product_staging)?,
        result_domain_raw: account(frame.result_domain_raw)?,
        result_domain_staging: account(frame.result_domain_staging)?,
        portfolio_raw: account(frame.portfolio_raw)?,
        portfolio_staging: account(frame.portfolio_staging)?,
    };
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|error| refusal(format!("Source terminal Market: {error:?}")))?;
    let report = build_resolution_admit_terminal_v3(&snapshot)
        .map_err(|error| operator_refusal("AdmitTerminal", error))?;
    validate_resolution_admit_terminal_report_v3(&report)
        .map_err(|error| operator_refusal("AdmitTerminal validation", error))?;
    let instruction = report.instruction.clone();
    let completion = vec![
        readiness.coordinates.market,
        readiness.coordinates.source_state,
        frame.certificate,
    ];
    let plan = instruction_plan_v1(&instruction, report, None, completion);
    match (market.phase, market.terminal_receipt) {
        (Phase::Open, None) => Ok(SourceTerminalPlanV1::Admit(plan)),
        (Phase::Terminal, Some(receipt)) if receipt.to_bytes() == frame.certificate.to_bytes() => {
            Ok(SourceTerminalPlanV1::Complete(plan))
        }
        _ => Err(refusal(
            "Source terminal Market is neither unadmitted Open nor exactly admitted Terminal",
        )),
    }
}

/// Return the exact Source close frame, refusing aliases before RPC.
pub fn source_close_fund_observation_addresses_v1(
    frame: &SourceCloseFundFrameV1,
) -> Result<Vec<Pubkey>, FundingReadinessErrorV1> {
    let readiness = frame.readiness;
    let mut addresses = vec![
        readiness.coordinates.market,
        readiness.activation_cache,
        readiness.registry_program,
        readiness.core_program,
        readiness.core_programdata,
        readiness.resolution_program,
        readiness.resolution_programdata,
        readiness.coordinates.source_material.raw,
        readiness.coordinates.source_material.staging,
        readiness.coordinates.capability_manifest.raw,
        readiness.coordinates.capability_manifest.staging,
        readiness.coordinates.source_state,
        readiness.coordinates.funding_ledger,
        frame.certificate,
        frame.closure_receipt,
        readiness.coordinates.beneficiary,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
    ];
    if let Some(recovery) = readiness.coordinates.recovery_policy {
        addresses.push(recovery.raw);
        addresses.push(recovery.staging);
    }
    if addresses.len() > SOURCE_CLOSE_FUND_OBSERVATION_ACCOUNTS_V1
        || addresses.iter().copied().collect::<BTreeSet<_>>().len() != addresses.len()
    {
        return Err(refusal(
            "Source close coordinates exceeded their bound or aliased frame positions",
        ));
    }
    Ok(addresses)
}

/// Select exact receipt prepayment or the V7 direct close from one finalized
/// observation. A prepayment never carries a stale close instruction: callers
/// must reacquire finalized state after the System transfer.
pub fn plan_source_close_fund_v1(
    frame: &SourceCloseFundFrameV1,
    accounts: &[ObservedAccount],
) -> Result<SourceCloseFundPlanV1, FundingReadinessErrorV1> {
    let addresses = source_close_fund_observation_addresses_v1(frame)?;
    if accounts.len() != addresses.len() {
        return Err(refusal(
            "Source close snapshot omitted or added frame accounts",
        ));
    }
    let observation = accounts
        .first()
        .ok_or_else(|| refusal("Source close snapshot is empty"))?
        .observation;
    if observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(refusal(
            "Source close RPC snapshot was not one complete finalized observation",
        ));
    }
    let mut observed = BTreeMap::new();
    for (expected, account) in addresses.iter().zip(accounts) {
        if account.key != *expected || observed.insert(account.key, account.clone()).is_some() {
            return Err(refusal(
                "Source close snapshot reordered, substituted, or repeated a frame account",
            ));
        }
    }
    let account = |key: Pubkey| {
        observed
            .get(&key)
            .cloned()
            .ok_or_else(|| refusal(format!("Source close snapshot omitted {key}")))
    };
    let readiness = frame.readiness;
    let mut snapshot = ResolutionCloseFundSnapshotV3 {
        market: account(readiness.coordinates.market)?,
        activation_cache: account(readiness.activation_cache)?,
        registry_program: account(readiness.registry_program)?,
        core_program: account(readiness.core_program)?,
        core_programdata: account(readiness.core_programdata)?,
        resolution_program: account(readiness.resolution_program)?,
        resolution_programdata: account(readiness.resolution_programdata)?,
        source_material: account(readiness.coordinates.source_material.raw)?,
        source_material_staging: account(readiness.coordinates.source_material.staging)?,
        capability_manifest: account(readiness.coordinates.capability_manifest.raw)?,
        capability_manifest_staging: account(readiness.coordinates.capability_manifest.staging)?,
        source_state: account(readiness.coordinates.source_state)?,
        funding_ledger: account(readiness.coordinates.funding_ledger)?,
        certificate: account(frame.certificate)?,
        closure_destination: account(frame.closure_receipt)?,
        beneficiary: account(readiness.coordinates.beneficiary)?,
        clock_sysvar: account(sysvar::clock::ID)?,
        rent_sysvar: account(sysvar::rent::ID)?,
        system_program: account(system_program::ID)?,
        recovery_policy: account(
            readiness
                .coordinates
                .recovery_policy
                .unwrap_or(readiness.coordinates.source_material)
                .raw,
        )?,
        recovery_policy_staging: account(
            readiness
                .coordinates
                .recovery_policy
                .unwrap_or(readiness.coordinates.source_material)
                .staging,
        )?,
    };
    let exact_rent = source_closure_receipt_rent_lamports_v1(&snapshot.rent_sysvar)
        .map_err(|error| operator_refusal("CloseFund receipt rent", error))?;
    if snapshot.closure_destination.owner != system_program::ID
        || snapshot.closure_destination.executable
        || !snapshot.closure_destination.data.is_empty()
        || snapshot.closure_destination.lamports > exact_rent
    {
        return Err(refusal(
            "Source close receipt was not the canonical vacant at-most-rent destination",
        ));
    }
    if snapshot.closure_destination.lamports < exact_rent {
        let current_lamports = snapshot.closure_destination.lamports;
        // Authenticate every other close precondition through the canonical
        // direct builder, changing only the receipt balance to the state the
        // exact System transfer will establish.
        snapshot.closure_destination.lamports = exact_rent;
        build_resolution_direct_close_fund_v1(&snapshot)
            .map_err(|error| operator_refusal("CloseFund prepay", error))?;
        return Ok(SourceCloseFundPlanV1::Prepay {
            observation,
            receipt: frame.closure_receipt,
            current_lamports,
            exact_rent_lamports: exact_rent,
            top_up_lamports: exact_rent.saturating_sub(current_lamports),
        });
    }
    let report = build_resolution_direct_close_fund_v1(&snapshot)
        .map_err(|error| operator_refusal("CloseFund", error))?;
    let instruction = report.instruction.clone();
    let completion = vec![
        readiness.coordinates.source_state,
        readiness.coordinates.funding_ledger,
        frame.closure_receipt,
        readiness.coordinates.beneficiary,
    ];
    Ok(SourceCloseFundPlanV1::Close(Box::new(instruction_plan_v1(
        &instruction,
        report,
        None,
        completion,
    ))))
}

fn plan_from_map_v1(
    frame: &FundingReadinessFrameV1,
    snapshot: &BTreeMap<Pubkey, ObservedAccount>,
) -> Result<FundingReadinessPlanV1, FundingReadinessErrorV1> {
    let coordinates = frame.coordinates;
    let recovery = coordinates
        .recovery_policy
        .unwrap_or(coordinates.source_material);
    let account = |key: Pubkey| {
        snapshot
            .get(&key)
            .cloned()
            .ok_or_else(|| refusal(format!("funding-readiness snapshot omitted {key}")))
    };
    let verify_snapshot = ResolutionVerifyFundReadySnapshotV3 {
        market: account(coordinates.market)?,
        activation_cache: account(frame.activation_cache)?,
        registry_program: account(frame.registry_program)?,
        core_program: account(frame.core_program)?,
        core_programdata: account(frame.core_programdata)?,
        resolution_program: account(frame.resolution_program)?,
        resolution_programdata: account(frame.resolution_programdata)?,
        source_material: account(coordinates.source_material.raw)?,
        source_material_staging: account(coordinates.source_material.staging)?,
        capability_manifest: account(coordinates.capability_manifest.raw)?,
        capability_manifest_staging: account(coordinates.capability_manifest.staging)?,
        source_state: account(coordinates.source_state)?,
        funding_ledger: account(coordinates.funding_ledger)?,
        beneficiary: account(coordinates.beneficiary)?,
        clock_sysvar: account(sysvar::clock::ID)?,
        rent_sysvar: account(sysvar::rent::ID)?,
        activation_receipt: account(coordinates.activation_receipt)?,
        recovery_policy: account(recovery.raw)?,
        recovery_policy_staging: account(recovery.staging)?,
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
        system_program: account(system_program::ID)?,
        recovery_policy: verify_snapshot.recovery_policy.clone(),
        recovery_policy_staging: verify_snapshot.recovery_policy_staging.clone(),
    };
    let market = CoreState::decode(&verify_snapshot.market.data)
        .map_err(|error| refusal(format!("funding-readiness Market: {error:?}")))?;
    let create = build_resolution_create_fund_v3(&create_snapshot);
    let activate = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot.clone(),
        system_program: account(system_program::ID)?,
    });
    let accept = build_resolution_verify_fund_ready_v3(&verify_snapshot);
    let refusals = format!(
        "create={:?} activate={:?} accept={:?}",
        create.as_ref().err(),
        activate.as_ref().err(),
        accept.as_ref().err()
    );
    match select_route_v1(
        market.phase,
        market.readiness,
        create.is_ok(),
        activate.as_ref().ok().map(activation_shape_v1),
        accept.is_ok(),
    )? {
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
        AuthenticatedRouteV1::ConsumedByFounding => {
            let source = &verify_snapshot.source_state;
            if source.owner != frame.resolution_program || source.data.is_empty() {
                return Err(refusal(format!(
                    "funding-readiness is not terminal: Open/Consumed Source state {} is vacant or not Resolution-owned; {refusals}",
                    source.key
                )));
            }
            Ok(FundingReadinessPlanV1::ConsumedByFounding)
        }
    }
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
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect();
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
    let protocol = instruction
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .chain(core::iter::once(instruction.program_id))
        .collect::<BTreeSet<_>>();
    let mut transaction = protocol.clone();
    let has_prepay = prepay.is_some_and(|value| value.lamports != 0);
    if let Some(value) = prepay {
        transaction.insert(value.destination);
    }
    FundingReadinessGeometryV1 {
        protocol_account_count: instruction.accounts.len(),
        protocol_unique_account_count: protocol.len(),
        protocol_writable_count: instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_writable)
            .count(),
        protocol_signer_count: instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .count(),
        protocol_data_len: instruction.data.len(),
        transaction_instruction_count_without_compute_budget: 1 + usize::from(has_prepay),
        transaction_lock_count_without_payer: transaction.len(),
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

fn select_route_v1(
    phase: Phase,
    readiness: Readiness,
    create: bool,
    activate: Option<ActivationShapeV1>,
    accept: bool,
) -> Result<AuthenticatedRouteV1, FundingReadinessErrorV1> {
    match (phase, readiness, create, activate, accept) {
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            true,
            None,
            false,
        ) => Ok(AuthenticatedRouteV1::Create),
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            false,
            Some(ActivationShapeV1::Fresh),
            false,
        ) => Ok(AuthenticatedRouteV1::Activate),
        (
            Phase::Founding | Phase::Open,
            Readiness::Prepaid | Readiness::Consumed,
            false,
            Some(ActivationShapeV1::Replay),
            true,
        ) => Ok(AuthenticatedRouteV1::Accept),
        (Phase::Founding, Readiness::Ready, false, None, true) => {
            Ok(AuthenticatedRouteV1::Complete)
        }
        (Phase::Open, Readiness::Consumed, false, None, false) => {
            Ok(AuthenticatedRouteV1::ConsumedByFounding)
        }
        _ => Err(refusal(format!(
            "funding-readiness account states were mixed or did not select one adjacent route: phase={phase:?} readiness={readiness:?} create={create} activate={activate:?} accept={accept}"
        ))),
    }
}

fn operator_refusal(label: &str, error: ResolutionCoreOperatorErrorV3) -> FundingReadinessErrorV1 {
    refusal(format!("{label} chain authentication refused: {error:?}"))
}

fn refusal(message: impl Into<String>) -> FundingReadinessErrorV1 {
    FundingReadinessErrorV1(message.into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn key(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    fn frame() -> FundingReadinessFrameV1 {
        FundingReadinessFrameV1 {
            coordinates: FundingReadinessCoordinatesV1 {
                market: key(1),
                source_material: FundingReadinessRecordCoordinatesV1 {
                    raw: key(8),
                    staging: key(9),
                },
                capability_manifest: FundingReadinessRecordCoordinatesV1 {
                    raw: key(10),
                    staging: key(11),
                },
                recovery_policy: Some(FundingReadinessRecordCoordinatesV1 {
                    raw: key(19),
                    staging: key(20),
                }),
                source_state: key(12),
                funding_ledger: key(13),
                beneficiary: key(14),
                activation_receipt: key(15),
            },
            activation_cache: key(2),
            registry_program: key(3),
            core_program: key(4),
            core_programdata: key(5),
            resolution_program: key(6),
            resolution_programdata: key(7),
        }
    }

    #[test]
    fn frame_is_ordered_bounded_and_alias_free() {
        let addresses = funding_readiness_observation_addresses_v1(&frame()).expect("frame");
        assert_eq!(addresses.len(), 20);
        assert_eq!(addresses[0], key(1));
        let mut aliased = frame();
        aliased.coordinates.source_state = aliased.coordinates.market;
        assert_eq!(
            funding_readiness_observation_addresses_v1(&aliased)
                .expect_err("alias")
                .message(),
            "funding-readiness coordinates aliased two semantic frame positions"
        );
    }

    #[test]
    fn selector_admits_only_exact_adjacent_states() {
        assert_eq!(
            select_route_v1(Phase::Founding, Readiness::Prepaid, true, None, false),
            Ok(AuthenticatedRouteV1::Create)
        );
        assert_eq!(
            select_route_v1(
                Phase::Open,
                Readiness::Consumed,
                false,
                Some(ActivationShapeV1::Fresh),
                false
            ),
            Ok(AuthenticatedRouteV1::Activate)
        );
        assert_eq!(
            select_route_v1(
                Phase::Founding,
                Readiness::Prepaid,
                false,
                Some(ActivationShapeV1::Replay),
                true
            ),
            Ok(AuthenticatedRouteV1::Accept)
        );
        assert!(select_route_v1(Phase::Open, Readiness::Ready, false, None, true).is_err());
        assert!(
            select_route_v1(
                Phase::Founding,
                Readiness::Prepaid,
                true,
                Some(ActivationShapeV1::Fresh),
                false
            )
            .is_err()
        );
    }
}
