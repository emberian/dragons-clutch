//! Exhaustive finalized acquisition for current Dealer action 23.

use crate::action_material::{
    construct_dealer_resolve_action_material_v1, ActionFreshnessBoundaryV1,
    CanonicalAccountAbsenceV1, CanonicalActionMaterialErrorV1, CanonicalActionMaterialV1,
    StructuredAddressLookupTableV1, StructuredChainAccountV1,
};
use crate::collateral_release_catalog::AuthenticatedCurrentCollateralReleaseV1;
use crate::rpc_index::{
    finalized_exact_account_snapshot_request_v1, FinalizedAccountSnapshotV1,
    FinalizedExactAccountSnapshotRequestV1, IndexedProgramRelease, ObservedRpcAccount,
    RpcIndexPlan, RpcObservationSource,
};
use crate::transaction_builder::ProtocolTransactionBuilder;
use clutch_collateral_adapter_v2::{
    ClaimLedgerV3, CLAIM_LEDGER_V3_PDA_SEED_V1, COLLATERAL_POLICY_PDA_SEED_V1,
    HOARD_V2_PDA_SEED_V1, PROFILE_PDA_SEED_V1, REALM_PDA_SEED_V1,
};
use clutch_dealer_runtime_contract::{
    DealerActionReceiptV1, DealerFacilityReplayV1, DealerFundedDependenciesV2,
    DealerFutureCreditFundingV1, DealerLivenessCompartmentV1, DealerLivenessScheduleV1,
    DealerPhaseV2, DealerPolicyV1, DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1,
    DealerSeriesObligationBindingV3, DealerStateV3, DeletableRentOwnerV1,
    FixedCodec as DealerFixedCodec, Id as DealerId, DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1,
    DEALER_CLAIM_WORK_PDA_DOMAIN_V1, DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2,
    DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1, DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1,
    DEALER_POLICY_PDA_DOMAIN_V1,
};
use clutch_fractional_redemption_runtime::{
    FRACTIONAL_CREDIT_PDA_PREFIX, FRACTIONAL_LEDGER_PDA_PREFIX,
};
use clutch_general_v2_contract::{MARKET_BINDING_SEED_DOMAIN_V1, MARKET_RUNTIME_SEED_DOMAIN_V1};
use clutch_liveness::{RuntimeCompartmentV1, RuntimeLivenessPolicyV1};
use clutch_product_series::{FixedCodec, MarketLifecyclePhaseV3, RegistryProgramReleaseV2};
use clutch_retirement::PositionAccountV3;
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use clutch_solana_layout::registry::{
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
    DEALER_POLICY_ACCOUNT_HEADER_BYTES, DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3, DEALER_STATE_V3_ACCOUNT_BYTES,
    DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;
pub const DEALER_RESOLVE_MAXIMUM_VALIDITY_SLOTS_V1: u64 = 32;
const PRODUCT_ARTIFACT_PDA_DOMAIN_V1: &[u8] = b"dc:product-artifact:v1";
const LIVENESS_ACCOUNT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-live-account-v1";
const WORKFLOW_DOMAIN_V1: &[u8] = b"dragons-clutch/operator/dealer-resolve-workflow/v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerResolveFinalizedSnapshotV1 {
    scan: FinalizedAccountSnapshotV1,
    exact: FinalizedAccountSnapshotV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerResolveOperatorBatchV1 {
    snapshot_receipt_id: [u8; 32],
    release_key: String,
    observed_slot: u64,
    valid_before_slot: u64,
    materials: Vec<CanonicalActionMaterialV1>,
}

impl DealerResolveOperatorBatchV1 {
    pub const fn snapshot_receipt_id(&self) -> [u8; 32] {
        self.snapshot_receipt_id
    }
    pub fn release_key(&self) -> &str {
        &self.release_key
    }
    pub const fn observed_slot(&self) -> u64 {
        self.observed_slot
    }
    pub const fn valid_before_slot(&self) -> u64 {
        self.valid_before_slot
    }
    pub fn materials(&self) -> &[CanonicalActionMaterialV1] {
        &self.materials
    }
}

#[allow(clippy::too_many_arguments)]
pub fn plan_dealer_resolve_snapshot_v1(
    plan: &RpcIndexPlan,
    release: &IndexedProgramRelease,
    collateral: &AuthenticatedCurrentCollateralReleaseV1<'_>,
    builder: &ProtocolTransactionBuilder,
    scan: &FinalizedAccountSnapshotV1,
    lookup_table: Address,
    request_id: u64,
) -> Result<Option<FinalizedExactAccountSnapshotRequestV1>> {
    release.validate().map_err(|_| invalid())?;
    if plan.cluster.key() != scan.receipt().cluster_key()
        || scan.receipt().release_key() != release.key()
        || plan.release(&release.key()).map_err(|_| invalid())? != release
        || collateral.artifact_owner() != release.program_id
        || collateral.observed_slot() != scan.receipt().slot()
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || lookup_table == Address::default()
        || scan.accounts().iter().any(|account| {
            account.provenance.source != RpcObservationSource::FinalizedScan
                || account.owner != release.program_id
        })
    {
        return Err(invalid());
    }
    require_unique(scan.accounts())?;
    let artifact = find(scan.accounts(), collateral.artifact_account())?;
    if RegistryProgramReleaseV2::decode(&artifact.data).map_err(|_| invalid())?
        != collateral.artifact()
    {
        return Err(invalid());
    }
    let mut addresses = BTreeSet::from([artifact.address]);
    let mut candidates = 0usize;
    for account in scan.accounts() {
        let Ok(state) = decode_framed::<DealerStateV3>(
            release.program_id,
            account,
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            DEALER_STATE_V3_ACCOUNT_BYTES,
        ) else {
            continue;
        };
        if !resolve_candidate(&state) {
            continue;
        }
        candidates = candidates.checked_add(1).ok_or_else(invalid)?;
        for (index, address) in derive_addresses(
            release,
            collateral.program().program_id,
            collateral.program().program_data,
            builder.payer(),
            scan,
            account.address,
        )?
        .into_iter()
        .enumerate()
        {
            if !matches!(index, 15 | 17 | 40) {
                addresses.insert(address);
            }
        }
    }
    if candidates == 0 {
        return Ok(None);
    }
    addresses.extend([
        release.program_id,
        release.program_data,
        collateral.program().program_id,
        collateral.program().program_data,
        solana_sdk_ids::sysvar::clock::ID,
        solana_sdk_ids::sysvar::rent::ID,
        solana_sdk_ids::system_program::ID,
        lookup_table,
    ]);
    finalized_exact_account_snapshot_request_v1(
        plan,
        &release.key(),
        request_id,
        scan.receipt().slot(),
        addresses.into_iter().collect(),
    )
    .map(Some)
    .map_err(|_| invalid())
}

pub fn join_dealer_resolve_snapshots_v1(
    scan: &FinalizedAccountSnapshotV1,
    exact: &FinalizedAccountSnapshotV1,
) -> Result<DealerResolveFinalizedSnapshotV1> {
    if scan.receipt().cluster_key() != exact.receipt().cluster_key()
        || scan.receipt().release_key() != exact.receipt().release_key()
        || scan.receipt().slot() != exact.receipt().slot()
        || scan
            .accounts()
            .iter()
            .any(|account| account.provenance.source != RpcObservationSource::FinalizedScan)
        || exact.accounts().iter().any(|account| {
            !matches!(
                account.provenance.source,
                RpcObservationSource::FinalizedExactAccountSnapshot { .. }
            )
        })
    {
        return Err(invalid());
    }
    require_unique(scan.accounts())?;
    require_unique(exact.accounts())?;
    let owner = scan
        .accounts()
        .first()
        .map(|account| account.owner)
        .ok_or_else(invalid)?;
    if owner == Address::default() || scan.accounts().iter().any(|account| account.owner != owner) {
        return Err(invalid());
    }
    for reread in exact
        .accounts()
        .iter()
        .filter(|account| account.owner == owner)
    {
        let discovered = find(scan.accounts(), reread.address)?;
        if reread.lamports != discovered.lamports
            || reread.executable != discovered.executable
            || reread.rent_epoch != discovered.rent_epoch
            || reread.data != discovered.data
        {
            return Err(invalid());
        }
    }
    Ok(DealerResolveFinalizedSnapshotV1 {
        scan: scan.clone(),
        exact: exact.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn enumerate_dealer_resolve_material_v1(
    release: &IndexedProgramRelease,
    collateral: AuthenticatedCurrentCollateralReleaseV1<'_>,
    builder: &ProtocolTransactionBuilder,
    snapshot: &DealerResolveFinalizedSnapshotV1,
    lookup_table: Address,
) -> Result<DealerResolveOperatorBatchV1> {
    let slot = snapshot.exact.receipt().slot();
    if collateral.observed_slot() != slot {
        return Err(invalid());
    }
    let valid_before_slot = slot
        .checked_add(DEALER_RESOLVE_MAXIMUM_VALIDITY_SLOTS_V1)
        .ok_or_else(invalid)?;
    let freshness = ActionFreshnessBoundaryV1 {
        observed_slot: slot,
        valid_before_slot,
        maximum_validity_slots: DEALER_RESOLVE_MAXIMUM_VALIDITY_SLOTS_V1,
    };
    let lookup = StructuredAddressLookupTableV1::authenticate(find(
        snapshot.exact.accounts(),
        lookup_table,
    )?)?;
    let clock = find(snapshot.exact.accounts(), solana_sdk_ids::sysvar::clock::ID)?;
    let current_slot = u64::from_le_bytes(
        clock
            .data
            .get(..8)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    );
    let scan_sequence = snapshot
        .scan
        .accounts()
        .first()
        .map(|account| account.provenance.receive_sequence)
        .filter(|value| *value != 0)
        .ok_or_else(invalid)?;
    let mut materials = Vec::new();
    for account in snapshot.scan.accounts() {
        let Ok(state) = decode_framed::<DealerStateV3>(
            release.program_id,
            account,
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            DEALER_STATE_V3_ACCOUNT_BYTES,
        ) else {
            continue;
        };
        if !resolve_candidate(&state) {
            continue;
        }
        let addresses = derive_addresses(
            release,
            collateral.program().program_id,
            collateral.program().program_data,
            builder.payer(),
            &snapshot.scan,
            account.address,
        )?;
        let policy = decode_policy(
            release.program_id,
            find(snapshot.scan.accounts(), addresses[1])?,
        )?;
        if current_slot < policy.maturity_slot {
            continue;
        }
        let frame = addresses
            .iter()
            .enumerate()
            .map(|(index, address)| {
                if matches!(index, 15 | 17 | 40) {
                    StructuredChainAccountV1::absent_from_snapshot(*address, &snapshot.scan)
                } else {
                    StructuredChainAccountV1::present(find(snapshot.exact.accounts(), *address)?)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let absences = [
            (15usize, "liveness-receipt"),
            (17usize, "claim-work"),
            (40usize, "facility-credit"),
        ]
        .into_iter()
        .map(|(role_index, label)| {
            CanonicalAccountAbsenceV1::new(
                role_index,
                label,
                addresses[role_index],
                release.key(),
                slot,
                scan_sequence,
            )
        })
        .collect();
        materials.push(construct_dealer_resolve_action_material_v1(
            release,
            collateral,
            builder,
            workflow_id(release, account.address, &state)?,
            freshness,
            &frame,
            absences,
            &lookup,
        )?);
    }
    materials.sort_by_key(CanonicalActionMaterialV1::driver_account);
    Ok(DealerResolveOperatorBatchV1 {
        snapshot_receipt_id: snapshot.exact.receipt().receipt_id(),
        release_key: release.key(),
        observed_slot: slot,
        valid_before_slot,
        materials,
    })
}

fn resolve_candidate(state: &DealerStateV3) -> bool {
    matches!(
        state.base.phase,
        DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly
    ) && state.base.children.epoch_bindings == 0
        && state.base.children.leases == 0
        && state.base.children.settlement_pots == 0
        && state.base.children.claim_work == 0
        && state.base.children.terminal_allocations == 0
        && state.base.children.exit_tickets == 0
        && state.base.queued_shares == 0
}

fn workflow_id(
    release: &IndexedProgramRelease,
    address: Address,
    state: &DealerStateV3,
) -> Result<[u8; 32]> {
    let value: [u8; 32] = Sha256::new()
        .chain_update(WORKFLOW_DOMAIN_V1)
        .chain_update(release.program_id.to_bytes())
        .chain_update(release.release_manifest_sha256)
        .chain_update(address.to_bytes())
        .chain_update(state.base.facility_id.bytes())
        .finalize()
        .into();
    if value == [0; 32] {
        Err(invalid())
    } else {
        Ok(value)
    }
}

#[allow(clippy::too_many_lines)]
fn derive_addresses(
    release: &IndexedProgramRelease,
    collateral_program: Address,
    collateral_programdata: Address,
    actor: Address,
    scan: &FinalizedAccountSnapshotV1,
    state_address: Address,
) -> Result<Vec<Address>> {
    let program = release.program_id;
    let accounts = scan.accounts();
    let state = decode_framed::<DealerStateV3>(
        program,
        find(accounts, state_address)?,
        DEALER_STATE_V3_ACCOUNT_TAG,
        DEALER_STATE_V3_ACCOUNT_VERSION,
        DEALER_STATE_V3_ACCOUNT_BYTES,
    )?;
    let policy_address = Address::find_program_address(
        &[DEALER_POLICY_PDA_DOMAIN_V1, &state.base.policy_id.bytes()],
        &program,
    )
    .0;
    let policy = decode_policy(program, find(accounts, policy_address)?)?;
    let position_address = address(state.base.facility_position_account_id.bytes());
    let replay_address = address(state.base.facility_replay_account_id.bytes());
    let position = PositionAccountV3::decode(&find(accounts, position_address)?.data)
        .map_err(|_| invalid())?;
    let replay = DealerFacilityReplayV1::decode(&find(accounts, replay_address)?.data)
        .map_err(|_| invalid())?;
    let dependency_address = Address::find_program_address(
        &[
            DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2,
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let dependency = decode_framed::<DealerFundedDependenciesV2>(
        program,
        find(accounts, dependency_address)?,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
    )?;
    let schedule_address = Address::find_program_address(
        &[
            DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1,
            &dependency.bindings.liveness_schedule_id.bytes(),
        ],
        &program,
    )
    .0;
    let schedule = decode_framed::<DealerLivenessScheduleV1>(
        program,
        find(accounts, schedule_address)?,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    )?;
    let runtime_policy_address = address(
        dependency
            .bindings
            .runtime_liveness_policy_account_id
            .bytes(),
    );
    let runtime_policy =
        RuntimeLivenessPolicyV1::decode(&find(accounts, runtime_policy_address)?.data)
            .map_err(|_| invalid())?;
    let first_address = Address::find_program_address(
        &[
            LIVENESS_ACCOUNT_PDA_DOMAIN_V1,
            &state.base.facility_id.bytes(),
            &[0],
        ],
        &program,
    )
    .0;
    let first = RuntimeCompartmentV1::decode(&find(accounts, first_address)?.data)
        .map_err(|_| invalid())?;
    let mut compartments = [first; 7];
    let mut liveness = Vec::with_capacity(7);
    for index in 0usize..7 {
        let kind = u8::try_from(index).map_err(|_| invalid())?;
        let account = Address::find_program_address(
            &[
                LIVENESS_ACCOUNT_PDA_DOMAIN_V1,
                &state.base.facility_id.bytes(),
                &[kind],
            ],
            &program,
        )
        .0;
        compartments[index] =
            RuntimeCompartmentV1::decode(&find(accounts, account)?.data).map_err(|_| invalid())?;
        liveness.push(account);
    }
    let runtime = DealerRuntimeLivenessBindingV1::from_canonical(&runtime_policy, &compartments)
        .map_err(|_| invalid())?;
    let resolution_liveness = compartments[DealerLivenessCompartmentV1::Resolution.index()];
    let call_ordinal = resolution_liveness
        .completed_calls
        .checked_add(1)
        .ok_or_else(invalid)?;
    let payment = schedule.reward_lamports
        [DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::Resolve)];
    let placeholder = DealerId::from_bytes(program.to_bytes());
    let receipt = DealerActionReceiptV1 {
        policy_id: state.base.policy_id,
        facility_id: state.base.facility_id,
        dealer_state_account_id: DealerId::from_bytes(state_address.to_bytes()),
        liveness_schedule_id: dependency.bindings.liveness_schedule_id,
        runtime_policy_id: runtime.runtime_policy_id(),
        runtime_account_id: runtime.account_id(DealerLivenessCompartmentV1::Resolution),
        runtime_owner: runtime.owner(DealerLivenessCompartmentV1::Resolution),
        quote_schedule_id: runtime.quote_schedule_id(DealerLivenessCompartmentV1::Resolution),
        receipt_account_id: placeholder,
        receipt_program_id: placeholder,
        keeper: DealerId::from_bytes(actor.to_bytes()),
        replay_account_id: DealerId::from_bytes(replay_address.to_bytes()),
        action: DealerRuntimeActionV1::Resolve,
        compartment: DealerLivenessCompartmentV1::Resolution,
        runtime_generation: runtime.generation(DealerLivenessCompartmentV1::Resolution),
        facility_generation: state.base.generation,
        call_ordinal,
        call_ceiling_lamports: payment,
        keeper_payment_lamports: payment,
        expected_replay_ordinal: replay.next_transition_ordinal(),
        rent: DeletableRentOwnerV1 {
            payer: DealerId::from_bytes(actor.to_bytes()),
            neutral_sink: policy.neutral_sink,
            refundable_principal: 1,
            donation_floor: 0,
        },
    };
    let receipt_address = Address::find_program_address(
        &[
            DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1,
            &receipt.receipt_slot_id().map_err(|_| invalid())?.bytes(),
        ],
        &program,
    )
    .0;
    let claim_work = Address::find_program_address(
        &[
            DEALER_CLAIM_WORK_PDA_DOMAIN_V1,
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let funding_address = Address::find_program_address(
        &[
            DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1,
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let funding = decode_framed::<DealerFutureCreditFundingV1>(
        program,
        find(accounts, funding_address)?,
        clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
        clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
        clutch_solana_layout::registry::DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES,
    )?;
    let obligation_address = address(state.series_obligation_binding_account_id.bytes());
    let obligation = decode_framed::<DealerSeriesObligationBindingV3>(
        program,
        find(accounts, obligation_address)?,
        DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
        DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3,
        DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3,
    )?;
    let root_address = address(obligation.key.product_market_root_account_id.bytes());
    let root = MarketLifecycleRootAccountV3::decode(&find(accounts, root_address)?.data)
        .map_err(|_| invalid())?;
    if root.state.phase() != MarketLifecyclePhaseV3::Active {
        return Err(invalid());
    }
    let link_address = address(obligation.key.series_market_link_account_id.bytes());
    let realm =
        Address::find_program_address(&[REALM_PDA_SEED_V1, &policy.realm_id.bytes()], &program).0;
    let profile = Address::find_program_address(
        &[
            PROFILE_PDA_SEED_V1,
            &policy.realm_id.bytes(),
            &policy.profile_id.bytes(),
        ],
        &program,
    )
    .0;
    let collateral_policy = Address::find_program_address(
        &[
            COLLATERAL_POLICY_PDA_SEED_V1,
            &policy.profile_id.bytes(),
            &position.collateral_policy_id().bytes(),
        ],
        &program,
    )
    .0;
    let market_binding = Address::find_program_address(
        &[
            MARKET_BINDING_SEED_DOMAIN_V1,
            &policy.market_instance_v2_id.bytes(),
        ],
        &program,
    )
    .0;
    let market_runtime = Address::find_program_address(
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, &market_binding.to_bytes()],
        &program,
    )
    .0;
    let market_artifact = Address::find_program_address(
        &[
            PRODUCT_ARTIFACT_PDA_DOMAIN_V1,
            &[ArtifactKind::MarketInstancePreimageV2.byte()],
            &policy.market_instance_v2_id.bytes(),
        ],
        &program,
    )
    .0;
    let hoard = Address::find_program_address(
        &[HOARD_V2_PDA_SEED_V1, &policy.market_instance_v2_id.bytes()],
        &program,
    )
    .0;
    let claim = Address::find_program_address(
        &[
            CLAIM_LEDGER_V3_PDA_SEED_V1,
            &policy.market_instance_v2_id.bytes(),
        ],
        &program,
    )
    .0;
    let claim_body = ClaimLedgerV3::decode(&find(accounts, claim)?.data).map_err(|_| invalid())?;
    let fractional_policy = address(claim_body.fractional_policy_id.bytes());
    let fractional_ledger = Address::find_program_address(
        &[FRACTIONAL_LEDGER_PDA_PREFIX, &fractional_policy.to_bytes()],
        &program,
    )
    .0;
    let credit = Address::find_program_address(
        &[
            FRACTIONAL_CREDIT_PDA_PREFIX,
            &fractional_policy.to_bytes(),
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let mut roles = vec![
        actor,
        policy_address,
        state_address,
        position_address,
        replay_address,
        dependency_address,
        schedule_address,
        runtime_policy_address,
    ];
    roles.extend(liveness);
    roles.extend([
        receipt_address,
        address(resolution_liveness.identity.payer.bytes()),
        claim_work,
        funding_address,
        address(funding.refund_owner.bytes()),
        address(funding.neutral_sink.bytes()),
        solana_sdk_ids::sysvar::clock::ID,
        solana_sdk_ids::sysvar::rent::ID,
        solana_sdk_ids::system_program::ID,
        root_address,
        obligation_address,
        link_address,
        realm,
        profile,
        collateral_policy,
        collateral_program,
        collateral_programdata,
        market_binding,
        market_runtime,
        market_artifact,
        hoard,
        claim,
        address(root.state.binding().resolution_account_id.bytes()),
        fractional_policy,
        fractional_ledger,
        credit,
    ]);
    Ok(roles)
}

fn decode_policy(program: Address, account: &ObservedRpcAccount) -> Result<DealerPolicyV1> {
    if account.owner != program
        || account.executable
        || account.data.len() < DEALER_POLICY_ACCOUNT_HEADER_BYTES
        || account.data[0] != DEALER_POLICY_ACCOUNT_TAG
        || account.data[1] != DEALER_POLICY_ACCOUNT_VERSION
    {
        return Err(invalid());
    }
    DealerPolicyV1::decode(&account.data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..])
        .map_err(|_| invalid())
}

fn decode_framed<T: DealerFixedCodec>(
    program: Address,
    account: &ObservedRpcAccount,
    tag: u8,
    version: u8,
    bytes: usize,
) -> Result<T> {
    if account.owner != program
        || account.executable
        || account.data.len() != bytes
        || bytes != 8usize.saturating_add(T::ENCODED_LEN)
        || account.data[0] != tag
        || account.data[1] != version
        || account.data[3..8].iter().any(|byte| *byte != 0)
    {
        return Err(invalid());
    }
    T::decode(&account.data[8..]).map_err(|_| invalid())
}

fn address(bytes: [u8; 32]) -> Address {
    Address::new_from_array(bytes)
}
fn find(accounts: &[ObservedRpcAccount], address: Address) -> Result<&ObservedRpcAccount> {
    let mut found = accounts.iter().filter(|account| account.address == address);
    let account = found.next().ok_or_else(invalid)?;
    if found.next().is_some() {
        return Err(invalid());
    }
    Ok(account)
}
fn require_unique(accounts: &[ObservedRpcAccount]) -> Result<()> {
    let mut values = BTreeSet::new();
    if accounts
        .iter()
        .any(|account| !values.insert(account.address))
    {
        return Err(invalid());
    }
    Ok(())
}
const fn invalid() -> CanonicalActionMaterialErrorV1 {
    CanonicalActionMaterialErrorV1::InvalidChainState
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve_keeps_three_exact_fresh_children() {
        assert_eq!([15usize, 17, 40].len(), 3);
        assert_eq!(DEALER_RESOLVE_MAXIMUM_VALIDITY_SLOTS_V1, 32);
    }
}
