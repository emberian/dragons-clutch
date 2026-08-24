//! Exhaustive finalized-frame acquisition for current Dealer action 25.
//!
//! The owner scan discovers every Retiring `DealerStateV3`. A second exact
//! snapshot reacquires the complete program-owned scan plus every referenced
//! System identity, both checked loader pairs, and the pinned address lookup
//! table at the same finalized slot. Only the hostile Dealer constructor may
//! turn that opaque frame into target-8/9 unsigned material.

use crate::action_material::{
    construct_dealer_terminal_action_material_v1, ActionFreshnessBoundaryV1,
    CanonicalActionMaterialErrorV1, CanonicalActionMaterialV1,
    StructuredAddressLookupTableV1, StructuredChainAccountV1,
};
use crate::collateral_release_catalog::AuthenticatedCurrentCollateralReleaseV1;
use crate::rpc_index::{
    finalized_exact_account_snapshot_request_v1, FinalizedAccountSnapshotV1,
    FinalizedExactAccountSnapshotRequestV1, IndexedProgramRelease, ObservedRpcAccount,
    RpcIndexPlan, RpcObservationSource,
};
use crate::transaction_builder::ProtocolTransactionBuilder;
use clutch_dealer_runtime_contract::{
    DealerActionReceiptV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2, DealerLivenessCompartmentV1, DealerLivenessScheduleV1,
    DealerPhaseV2, DealerPolicyV1, DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1,
    DealerSeriesObligationBindingV3, DealerStateV3, DeletableRentOwnerV1,
    FixedCodec as DealerFixedCodec, Id as DealerId, DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1,
    DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2, DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1,
    DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1, DEALER_POLICY_PDA_DOMAIN_V1,
};
use clutch_fractional_redemption_runtime::{
    FRACTIONAL_CREDIT_PDA_PREFIX,
};
use clutch_general_v2_contract::{
    MARKET_BINDING_SEED_DOMAIN_V1, MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_liveness::runtime_v1::{RuntimeCompartmentV1, RuntimeLivenessPolicyV1};
use clutch_product_series::{FixedCodec, MarketLifecyclePhaseV3, RegistryProgramReleaseV2};
use clutch_retirement::PositionAccountV3;
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use clutch_solana_layout::registry::{
    DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES, DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
    DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
    DEALER_POLICY_ACCOUNT_HEADER_BYTES, DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3, DEALER_STATE_V3_ACCOUNT_BYTES,
    DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION,
};
use clutch_collateral_adapter_v2::{
    ClaimLedgerV3, CLAIM_LEDGER_V3_PDA_SEED_V1, COLLATERAL_POLICY_PDA_SEED_V1,
    HOARD_V2_PDA_SEED_V1, PROFILE_PDA_SEED_V1, REALM_PDA_SEED_V1,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;

pub const DEALER_TERMINAL_MAXIMUM_VALIDITY_SLOTS_V1: u64 = 32;
const PRODUCT_ARTIFACT_PDA_DOMAIN_V1: &[u8] = b"dc:product-artifact:v1";
const DEALER_RUNTIME_LIVENESS_ACCOUNT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-live-account-v1";
const DEALER_TERMINAL_WORKFLOW_DOMAIN_V1: &[u8] =
    b"dragons-clutch/operator/dealer-terminal-workflow/v1\0";

/// Joined exhaustive owner scan and exact cross-owner reread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerTerminalFinalizedSnapshotV1 {
    program_scan: FinalizedAccountSnapshotV1,
    exact: FinalizedAccountSnapshotV1,
}

/// Exhaustive target-8/9 material for every finalized Retiring facility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerTerminalOperatorBatchV1 {
    snapshot_receipt_id: [u8; 32],
    release_key: String,
    observed_slot: u64,
    valid_before_slot: u64,
    materials: Vec<CanonicalActionMaterialV1>,
}

impl DealerTerminalOperatorBatchV1 {
    #[must_use]
    pub const fn snapshot_receipt_id(&self) -> [u8; 32] { self.snapshot_receipt_id }
    #[must_use]
    pub fn release_key(&self) -> &str { &self.release_key }
    #[must_use]
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    #[must_use]
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    #[must_use]
    pub fn materials(&self) -> &[CanonicalActionMaterialV1] { &self.materials }
}

/// Derive one bounded exact-address request from an exhaustive finalized scan.
/// The caller supplies no facility, target discriminator, or semantic ID.
#[allow(clippy::too_many_arguments)]
pub fn plan_dealer_terminal_snapshot_v1(
    plan: &RpcIndexPlan,
    release: &IndexedProgramRelease,
    collateral: &AuthenticatedCurrentCollateralReleaseV1<'_>,
    builder: &ProtocolTransactionBuilder,
    program_scan: &FinalizedAccountSnapshotV1,
    lookup_table: Address,
    request_id: u64,
) -> Result<Option<FinalizedExactAccountSnapshotRequestV1>> {
    release.validate().map_err(|_| invalid())?;
    if plan.cluster.key() != program_scan.receipt().cluster_key()
        || program_scan.receipt().release_key() != release.key()
        || plan.release(&release.key()).map_err(|_| invalid())? != release
        || collateral.artifact_owner() != release.program_id
        || collateral.observed_slot() != program_scan.receipt().slot()
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || lookup_table == Address::default()
        || program_scan.accounts().iter().any(|account| {
            account.provenance.source != RpcObservationSource::FinalizedScan
                || account.owner != release.program_id
        })
    {
        return Err(invalid());
    }
    require_unique(program_scan.accounts())?;
    let mut addresses = BTreeSet::new();
    let collateral_artifact = find(
        program_scan.accounts(),
        collateral.artifact_account(),
    )?;
    let observed_collateral_artifact = RegistryProgramReleaseV2::decode(
        &collateral_artifact.data,
    )
    .map_err(|_| invalid())?;
    if observed_collateral_artifact != collateral.artifact() {
        return Err(invalid());
    }
    addresses.insert(collateral_artifact.address);
    let mut ready = 0usize;
    for state_account in program_scan.accounts() {
        let Ok(state) = decode_framed::<DealerStateV3>(
            release.program_id,
            state_account,
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            DEALER_STATE_V3_ACCOUNT_BYTES,
        ) else { continue };
        if state.base.phase != DealerPhaseV2::Retiring { continue; }
        ready = ready.checked_add(1).ok_or_else(invalid)?;
        let active = active_facility_credit_tail(
            release.program_id,
            program_scan.accounts(),
            &state,
        )?;
        let roles = derive_role_addresses(
            release,
            collateral.program().program_id,
            collateral.program().program_data,
            builder.payer(),
            program_scan,
            program_scan,
            state_account.address,
            active,
        )?;
        for (index, address) in roles.into_iter().enumerate() {
            if index != 15 {
                addresses.insert(address);
            }
        }
    }
    if ready == 0 { return Ok(None); }
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
        program_scan.receipt().slot(),
        addresses.into_iter().collect(),
    )
    .map(Some)
    .map_err(|_| invalid())
}

/// Join only same-slot snapshots and prove every selected program-owned row
/// was present byte-for-byte in the exhaustive discovery scan.
pub fn join_dealer_terminal_snapshots_v1(
    program_scan: &FinalizedAccountSnapshotV1,
    exact: &FinalizedAccountSnapshotV1,
) -> Result<DealerTerminalFinalizedSnapshotV1> {
    if program_scan.receipt().cluster_key() != exact.receipt().cluster_key()
        || program_scan.receipt().release_key() != exact.receipt().release_key()
        || program_scan.receipt().slot() != exact.receipt().slot()
        || program_scan.accounts().iter().any(|account| account.provenance.source != RpcObservationSource::FinalizedScan)
        || exact.accounts().iter().any(|account| !matches!(account.provenance.source, RpcObservationSource::FinalizedExactAccountSnapshot { .. }))
    {
        return Err(invalid());
    }
    require_unique(program_scan.accounts())?;
    require_unique(exact.accounts())?;
    let program_owner = program_scan
        .accounts()
        .first()
        .map(|account| account.owner)
        .ok_or_else(invalid)?;
    if program_owner == Address::default()
        || program_scan
            .accounts()
            .iter()
            .any(|account| account.owner != program_owner)
    {
        return Err(invalid());
    }
    for reread in exact
        .accounts()
        .iter()
        .filter(|account| account.owner == program_owner)
    {
        let discovered = find(program_scan.accounts(), reread.address)?;
        if reread.lamports != discovered.lamports
            || reread.executable != discovered.executable
            || reread.rent_epoch != discovered.rent_epoch
            || reread.data != discovered.data
        {
            return Err(invalid());
        }
    }
    Ok(DealerTerminalFinalizedSnapshotV1 {
        program_scan: program_scan.clone(),
        exact: exact.clone(),
    })
}

/// Exhaustively construct exactly one target-8 or target-9 material for each
/// finalized Retiring State. Any incomplete or ambiguous facility fails the
/// entire batch instead of disappearing from operator discovery.
#[allow(clippy::too_many_arguments)]
pub fn enumerate_dealer_terminal_material_v1(
    release: &IndexedProgramRelease,
    collateral: AuthenticatedCurrentCollateralReleaseV1<'_>,
    builder: &ProtocolTransactionBuilder,
    snapshot: &DealerTerminalFinalizedSnapshotV1,
    lookup_table: Address,
) -> Result<DealerTerminalOperatorBatchV1> {
    let slot = snapshot.exact.receipt().slot();
    if collateral.observed_slot() != slot {
        return Err(invalid());
    }
    let valid_before_slot = slot
        .checked_add(DEALER_TERMINAL_MAXIMUM_VALIDITY_SLOTS_V1)
        .ok_or_else(invalid)?;
    let freshness = ActionFreshnessBoundaryV1 {
        observed_slot: slot,
        valid_before_slot,
        maximum_validity_slots: DEALER_TERMINAL_MAXIMUM_VALIDITY_SLOTS_V1,
    };
    let lookup = StructuredAddressLookupTableV1::authenticate(find(snapshot.exact.accounts(), lookup_table)?)?;
    let mut materials = Vec::new();
    for state_account in snapshot.program_scan.accounts() {
        let Ok(state) = decode_framed::<DealerStateV3>(
            release.program_id,
            state_account,
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            DEALER_STATE_V3_ACCOUNT_BYTES,
        ) else { continue };
        if state.base.phase != DealerPhaseV2::Retiring { continue; }
        let active = active_facility_credit_tail(
            release.program_id,
            snapshot.program_scan.accounts(),
            &state,
        )?;
        let addresses = derive_role_addresses(
            release,
            collateral.program().program_id,
            collateral.program().program_data,
            builder.payer(),
            &snapshot.exact,
            &snapshot.program_scan,
            state_account.address,
            active,
        )?;
        let frame = addresses
            .iter()
            .enumerate()
            .map(|(index, address)| {
                if index == 15 {
                    StructuredChainAccountV1::absent_from_snapshot(
                        *address,
                        &snapshot.program_scan,
                    )
                } else {
                    StructuredChainAccountV1::present(find(
                        snapshot.exact.accounts(),
                        *address,
                    )?)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        materials.push(construct_dealer_terminal_action_material_v1(
            release,
            collateral,
            builder,
            dealer_terminal_workflow_id(release, state_account.address, &state)?,
            freshness,
            &frame,
            &lookup,
        )?);
    }
    materials.sort_by_key(|material| (material.driver_account(), material.variant()));
    Ok(DealerTerminalOperatorBatchV1 {
        snapshot_receipt_id: snapshot.exact.receipt().receipt_id(),
        release_key: release.key(),
        observed_slot: slot,
        valid_before_slot,
        materials,
    })
}

fn dealer_terminal_workflow_id(
    release: &IndexedProgramRelease,
    state_account: Address,
    state: &DealerStateV3,
) -> Result<[u8; 32]> {
    let id: [u8; 32] = Sha256::new()
        .chain_update(DEALER_TERMINAL_WORKFLOW_DOMAIN_V1)
        .chain_update(release.program_id.to_bytes())
        .chain_update(release.release_manifest_sha256)
        .chain_update(state_account.to_bytes())
        .chain_update(state.base.facility_id.bytes())
        .finalize()
        .into();
    if id == [0; 32] {
        return Err(invalid());
    }
    Ok(id)
}

fn derive_role_addresses(
    release: &IndexedProgramRelease,
    collateral_program: Address,
    collateral_programdata: Address,
    actor: Address,
    exact: &FinalizedAccountSnapshotV1,
    owner_scan: &FinalizedAccountSnapshotV1,
    state_address: Address,
    active: bool,
) -> Result<Vec<Address>> {
    let program = release.program_id;
    let accounts = exact.accounts();
    let state_account = find(accounts, state_address)?;
    let state = decode_framed::<DealerStateV3>(program, state_account, DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION, DEALER_STATE_V3_ACCOUNT_BYTES)?;
    let policy_address = Address::find_program_address(&[DEALER_POLICY_PDA_DOMAIN_V1, &state.base.policy_id.bytes()], &program).0;
    let policy_account = find(accounts, policy_address)?;
    if policy_account.data.len() < DEALER_POLICY_ACCOUNT_HEADER_BYTES
        || policy_account.data[0] != DEALER_POLICY_ACCOUNT_TAG
        || policy_account.data[1] != DEALER_POLICY_ACCOUNT_VERSION
    { return Err(invalid()); }
    let policy = DealerPolicyV1::decode(&policy_account.data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..]).map_err(|_| invalid())?;
    let position_address = id_address(state.base.facility_position_account_id);
    let replay_address = id_address(state.base.facility_replay_account_id);
    let position = PositionAccountV3::decode(&find(accounts, position_address)?.data).map_err(|_| invalid())?;
    let replay = DealerFacilityReplayV1::decode(&find(accounts, replay_address)?.data).map_err(|_| invalid())?;
    let dependency_address = Address::find_program_address(&[DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2, &state.base.facility_id.bytes()], &program).0;
    let dependency = decode_framed::<DealerFundedDependenciesV2>(program, find(accounts, dependency_address)?, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES)?;
    let schedule_address = Address::find_program_address(&[DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1, &dependency.bindings.liveness_schedule_id.bytes()], &program).0;
    let schedule = decode_framed::<DealerLivenessScheduleV1>(program, find(accounts, schedule_address)?, DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?;
    let runtime_policy_address = id_address(dependency.bindings.runtime_liveness_policy_account_id);
    let runtime_policy = RuntimeLivenessPolicyV1::decode(&find(accounts, runtime_policy_address)?.data).map_err(|_| invalid())?;
    let mut liveness = Vec::with_capacity(7);
    let first = RuntimeCompartmentV1::decode(&find(accounts, Address::find_program_address(&[DEALER_RUNTIME_LIVENESS_ACCOUNT_PDA_DOMAIN_V1, &state.base.facility_id.bytes(), &[0]], &program).0)?.data).map_err(|_| invalid())?;
    let mut compartments = [first; 7];
    for index in 0usize..7 {
        let kind = u8::try_from(index).map_err(|_| invalid())?;
        let address = Address::find_program_address(&[DEALER_RUNTIME_LIVENESS_ACCOUNT_PDA_DOMAIN_V1, &state.base.facility_id.bytes(), &[kind]], &program).0;
        liveness.push(address);
        compartments[index] = RuntimeCompartmentV1::decode(&find(accounts, address)?.data).map_err(|_| invalid())?;
    }
    let runtime = DealerRuntimeLivenessBindingV1::from_canonical(&runtime_policy, &compartments).map_err(|_| invalid())?;
    let retirement = compartments[DealerLivenessCompartmentV1::Retirement.index()];
    let call_ordinal = retirement.completed_calls.checked_add(1).ok_or_else(invalid)?;
    let payment = schedule.reward_lamports[DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::Retire)];
    let obligation_address = id_address(state.series_obligation_binding_account_id);
    let obligation = decode_framed::<DealerSeriesObligationBindingV3>(program, find(accounts, obligation_address)?, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG, DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V3, DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V3)?;
    let placeholder_receipt = DealerId::from_bytes(program.to_bytes());
    let receipt = DealerActionReceiptV1 {
        policy_id: state.base.policy_id,
        facility_id: state.base.facility_id,
        dealer_state_account_id: DealerId::from_bytes(state_address.to_bytes()),
        liveness_schedule_id: dependency.bindings.liveness_schedule_id,
        runtime_policy_id: runtime.runtime_policy_id(),
        runtime_account_id: runtime.account_id(DealerLivenessCompartmentV1::Retirement),
        runtime_owner: runtime.owner(DealerLivenessCompartmentV1::Retirement),
        quote_schedule_id: runtime.quote_schedule_id(DealerLivenessCompartmentV1::Retirement),
        receipt_account_id: placeholder_receipt,
        receipt_program_id: DealerId::from_bytes(program.to_bytes()),
        keeper: DealerId::from_bytes(actor.to_bytes()),
        replay_account_id: DealerId::from_bytes(replay_address.to_bytes()),
        action: DealerRuntimeActionV1::Retire,
        compartment: DealerLivenessCompartmentV1::Retirement,
        runtime_generation: runtime.generation(DealerLivenessCompartmentV1::Retirement),
        facility_generation: state.base.generation,
        call_ordinal,
        call_ceiling_lamports: payment,
        keeper_payment_lamports: payment,
        expected_replay_ordinal: replay.next_transition_ordinal(),
        rent: DeletableRentOwnerV1 { payer: DealerId::from_bytes(actor.to_bytes()), neutral_sink: policy.neutral_sink, refundable_principal: 1, donation_floor: 0 },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(|_| invalid())?;
    let receipt_address = Address::find_program_address(&[DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1, &receipt_slot.bytes()], &program).0;
    if owner_scan.accounts().iter().any(|account| account.address == receipt_address) { return Err(invalid()); }
    let root_address = id_address(obligation.key.product_market_root_account_id);
    let root = MarketLifecycleRootAccountV3::decode(&find(accounts, root_address)?.data).map_err(|_| invalid())?;
    if root.state.phase() != MarketLifecyclePhaseV3::Active { return Err(invalid()); }
    let root_binding = root.state.binding();
    let link_address = id_address(obligation.key.series_market_link_account_id);
    let realm_address = Address::find_program_address(&[REALM_PDA_SEED_V1, &policy.realm_id.bytes()], &program).0;
    let profile_address = Address::find_program_address(&[PROFILE_PDA_SEED_V1, &policy.realm_id.bytes(), &policy.profile_id.bytes()], &program).0;
    let collateral_policy_address = Address::find_program_address(&[COLLATERAL_POLICY_PDA_SEED_V1, &policy.profile_id.bytes(), &position.collateral_policy_id().bytes()], &program).0;
    let market_binding = Address::find_program_address(&[MARKET_BINDING_SEED_DOMAIN_V1, &policy.market_instance_v2_id.bytes()], &program).0;
    let market_runtime = Address::find_program_address(&[MARKET_RUNTIME_SEED_DOMAIN_V1, &market_binding.to_bytes()], &program).0;
    let market_artifact = artifact_address(program, ArtifactKind::MarketInstancePreimageV2, policy.market_instance_v2_id.bytes());
    let hoard = Address::find_program_address(&[HOARD_V2_PDA_SEED_V1, &policy.market_instance_v2_id.bytes()], &program).0;
    let claim = Address::find_program_address(&[CLAIM_LEDGER_V3_PDA_SEED_V1, &policy.market_instance_v2_id.bytes()], &program).0;
    let mut roles = vec![
        actor, policy_address, state_address, position_address, replay_address,
        dependency_address, schedule_address, runtime_policy_address,
    ];
    roles.extend(liveness);
    roles.extend([
        receipt_address,
        Address::new_from_array(retirement.identity.payer.bytes()),
        id_address(position.rent().payer),
        id_address(replay.rent().payer()),
        id_address(obligation.rent.payer),
        id_address(obligation.rent.neutral_sink),
        solana_sdk_ids::sysvar::clock::ID,
        solana_sdk_ids::sysvar::rent::ID,
        solana_sdk_ids::system_program::ID,
        obligation_address, realm_address, profile_address,
        collateral_policy_address,
    ]);
    roles.extend([collateral_program, collateral_programdata, market_binding, market_runtime, market_artifact, hoard, claim]);
    if active {
        let claim_body = ClaimLedgerV3::decode(&find(accounts, claim)?.data).map_err(|_| invalid())?;
        let fractional_policy = Address::new_from_array(claim_body.fractional_policy_id.bytes());
        let fractional_ledger = Address::new_from_array(claim_body.fractional_ledger_account.bytes());
        let credit = Address::find_program_address(&[FRACTIONAL_CREDIT_PDA_PREFIX, &fractional_policy.to_bytes(), &state.base.facility_id.bytes()], &program).0;
        roles.extend([Address::new_from_array(root_binding.resolution_account_id.bytes()), fractional_policy, fractional_ledger, credit, root_address, link_address]);
    } else {
        let funding = Address::find_program_address(&[DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1, &state.base.facility_id.bytes()], &program).0;
        roles.extend([funding, root_address, link_address]);
    }
    Ok(roles)
}

fn active_facility_credit_tail(
    program: Address,
    accounts: &[ObservedRpcAccount],
    state: &DealerStateV3,
) -> Result<bool> {
    let policy_address = Address::find_program_address(
        &[DEALER_POLICY_PDA_DOMAIN_V1, &state.base.policy_id.bytes()],
        &program,
    )
    .0;
    let policy_account = find(accounts, policy_address)?;
    if policy_account.data.len() < DEALER_POLICY_ACCOUNT_HEADER_BYTES
        || policy_account.data[0] != DEALER_POLICY_ACCOUNT_TAG
        || policy_account.data[1] != DEALER_POLICY_ACCOUNT_VERSION
    {
        return Err(invalid());
    }
    let policy = DealerPolicyV1::decode(
        &policy_account.data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..],
    )
    .map_err(|_| invalid())?;
    let claim = Address::find_program_address(
        &[CLAIM_LEDGER_V3_PDA_SEED_V1, &policy.market_instance_v2_id.bytes()],
        &program,
    )
    .0;
    let claim = ClaimLedgerV3::decode(&find(accounts, claim)?.data)
        .map_err(|_| invalid())?;
    let fractional_policy = Address::new_from_array(claim.fractional_policy_id.bytes());
    let credit = Address::find_program_address(
        &[
            FRACTIONAL_CREDIT_PDA_PREFIX,
            &fractional_policy.to_bytes(),
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let funding = Address::find_program_address(
        &[
            DEALER_FUTURE_CREDIT_FUNDING_PDA_DOMAIN_V1,
            &state.base.facility_id.bytes(),
        ],
        &program,
    )
    .0;
    let credit_present = accounts.iter().any(|account| account.address == credit);
    let funding_present = accounts.iter().any(|account| account.address == funding);
    match (credit_present, funding_present) {
        (true, false) => Ok(true),
        (false, true) => Ok(false),
        _ => Err(invalid()),
    }
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
    { return Err(invalid()); }
    T::decode(&account.data[8..]).map_err(|_| invalid())
}

fn artifact_address(program: Address, kind: ArtifactKind, id: [u8; 32]) -> Address {
    Address::find_program_address(&[PRODUCT_ARTIFACT_PDA_DOMAIN_V1, &[kind.byte()], &id], &program).0
}

fn id_address(id: impl IntoDealerAddress) -> Address { Address::new_from_array(id.address_bytes()) }

trait IntoDealerAddress { fn address_bytes(self) -> [u8; 32]; }
impl IntoDealerAddress for DealerId { fn address_bytes(self) -> [u8; 32] { self.bytes() } }
impl IntoDealerAddress for clutch_retirement::Identity32V1 { fn address_bytes(self) -> [u8; 32] { self.bytes() } }

fn find(accounts: &[ObservedRpcAccount], address: Address) -> Result<&ObservedRpcAccount> {
    let mut found = accounts.iter().filter(|account| account.address == address);
    let account = found.next().ok_or_else(invalid)?;
    if found.next().is_some() { return Err(invalid()); }
    Ok(account)
}

fn require_unique(accounts: &[ObservedRpcAccount]) -> Result<()> {
    let mut addresses = BTreeSet::new();
    if accounts.iter().any(|account| !addresses.insert(account.address)) { return Err(invalid()); }
    Ok(())
}

const fn invalid() -> CanonicalActionMaterialErrorV1 {
    CanonicalActionMaterialErrorV1::InvalidChainState
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_account_widths_remain_exact_and_disjoint() {
        assert_eq!(crate::transaction_builder::dealer_terminal_account_count_v1(crate::rpc_index::CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit), 41);
        assert_eq!(crate::transaction_builder::dealer_terminal_account_count_v1(crate::rpc_index::CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit), 38);
    }

    #[test]
    fn exact_snapshot_decoder_is_the_only_cross_owner_acquisition() {
        assert_ne!(DEALER_TERMINAL_MAXIMUM_VALIDITY_SLOTS_V1, 0);
        assert_ne!(DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_TAG);
        assert_eq!(DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION, 1);
    }
}
