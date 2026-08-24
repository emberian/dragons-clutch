//! Chain-derived operator material for current General action 39.
//!
//! The public boundary names every semantic role, requires finalized absence
//! proofs for all nine fresh accounts, derives every fresh PDA and the exact
//! payload from hostile-decoded state, and emits only the current V5 account
//! geometry. It never accepts an action tag, sequence, payload, privilege
//! bitmap, or generic account vector from a caller.

use crate::account_index::FinalizedAccountAbsence;
use crate::action_material::StructuredAddressLookupTableV1;
use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    RpcCommitment,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft,
    ProtocolTransactionBuilder, SemanticOwner, TransactionTransport,
    UnsignedProtocolTransaction,
};
use crate::workflow_graph::{ResumableWorkflowCursor, WorkflowLane, WorkflowPosition};
use clutch_batch_policy_identity::revenue_policy_v2::{
    decode_revenue_policy_v2, RevenuePolicyV2,
};
use clutch_general_v2_contract::{
    AdmissionNodeV4AccountV1, CandidateFeedHeaderV2, CandidateWindowV5AccountV1,
    FinalPotSeedTupleV1, GeneralEpochPhaseV1, GeneralEpochV6AccountV1,
    CANDIDATE_ORDER_SLICE_INDEX_SEED_DOMAIN_V1, FEE_RETIREMENT_ACCUMULATOR_SEED_DOMAIN_V1,
    FROZEN_ORDER_LOCATOR_SEED_DOMAIN_V1, RECIPIENT_ALLOCATION_SEED_DOMAIN_V1,
    SELECTED_FEE_RECORD_SEED_DOMAIN_V1, SETTLEMENT_CASH_POT_SEED_DOMAIN_V1,
    SETTLEMENT_ROOT_SEED_DOMAIN_V1, TREASURY_LEDGER_SEED_DOMAIN_V1,
};
use clutch_solana_layout::registry::{ExtensionAction, GeneralV2Action};
use clutch_solana_layout::revenue::RevenuePolicyRecordV2;
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;

pub const GENERAL_ACTION39_VALIDITY_SLOTS_V1: u64 = 32;
pub const GENERAL_ACTION39_FIXED_ACCOUNT_COUNT_V1: usize = 49;
pub const GENERAL_ACTION39_MIN_ACCOUNT_COUNT_V1: usize = 50;
pub const GENERAL_ACTION39_MAX_ACCOUNT_COUNT_V1: usize = 53;
pub const GENERAL_ACTION39_LOCAL_ACTION_V1: u8 = 39;

pub const GENERAL_ACTION39_FIXED_ROLE_LABELS_V1: [&str; GENERAL_ACTION39_FIXED_ACCOUNT_COUNT_V1] = [
    "epoch-v6", "candidate-window-v5", "selected-admission-node-v4", "retained-feed-v2",
    "market-binding-v5", "market-runtime-v3", "economic-domain-v2", "price-grid",
    "realm", "collateral-profile-v2", "collateral-policy-v2", "collateral-token-program",
    "market-instance-v2", "market-genesis-v2", "selected-fee-record-v2",
    "recipient-allocation-v3", "batch-policy", "treasury-service-ledger-v1",
    "revenue-policy-record-v2", "treasury-ledger-v2", "fee-retirement-accumulator-v1",
    "product-market-root-v3", "series-market-link-v3", "series-funding-v5",
    "series-registry-v4", "registry-program", "registry-program-data",
    "registry-release-v2", "capability-profile-v4", "source-release-v2",
    "compiler-bundle-v7", "revenue-policy-preimage-v2", "series-plan-v5",
    "series-funding-terms-v2", "product-template-v4", "native-claim-basis-v1",
    "recovery-policy-v1", "price-measure-policy-v1", "funding-quote-v6",
    "attachment-plan-v6", "indexed-settlement-root-v1", "settlement-cash-pot-v1",
    "final-pot-v1", "frozen-order-locator-v1", "candidate-slice-index-v1", "rent-payer",
    "system-program", "rent-sysvar", "clock-sysvar",
];

const OWNER_PACKAGE: &str =
    "clutch-general-v2-contract+clutch-general-v2-runtime+clutch-product-series";
const OWNER_SCHEMA: &str = "dragons-clutch/operator/general-action39-material/v1";

pub type GeneralAction39MaterialResult<T> = core::result::Result<T, GeneralAction39MaterialError>;
type Result<T> = GeneralAction39MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAction39MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    FreshAccount,
    Construction,
    Arithmetic,
}

impl core::fmt::Display for GeneralAction39MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit current General action 39",
            Self::ChainSnapshot => "General action-39 accounts are not one finalized snapshot",
            Self::ChainAuthority => "current General V5 settlement authority refused",
            Self::FreshAccount => "fresh General action-39 PDA lacks exact finalized absence",
            Self::Construction => "release-bound General action-39 construction refused",
            Self::Arithmetic => "General action-39 cursor or freshness arithmetic overflowed",
        })
    }
}

impl std::error::Error for GeneralAction39MaterialError {}

/// One PDA which the exhaustive finalized scan proved absent.
#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39FreshAccountV1<'a> {
    pub address: Address,
    pub absence: &'a FinalizedAccountAbsence,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39CommonSnapshotV1<'a> {
    pub epoch: &'a ObservedRpcAccount,
    pub window: &'a ObservedRpcAccount,
    pub selected_node: &'a ObservedRpcAccount,
    pub retained_feed: &'a ObservedRpcAccount,
    pub market_binding: &'a ObservedRpcAccount,
    pub market_runtime: &'a ObservedRpcAccount,
    pub economic_domain: &'a ObservedRpcAccount,
    pub price_grid: &'a ObservedRpcAccount,
    pub realm: &'a ObservedRpcAccount,
    pub collateral_profile: &'a ObservedRpcAccount,
    pub collateral_policy: &'a ObservedRpcAccount,
    pub collateral_token_program: &'a ObservedRpcAccount,
    pub market_instance: &'a ObservedRpcAccount,
    pub market_genesis: &'a ObservedRpcAccount,
    pub selected_fee_record: GeneralAction39FreshAccountV1<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39FeeSnapshotV1<'a> {
    pub recipient_allocation: GeneralAction39FreshAccountV1<'a>,
    pub batch_policy: &'a ObservedRpcAccount,
    pub treasury_service_ledger: &'a ObservedRpcAccount,
    pub revenue_policy_record: &'a ObservedRpcAccount,
    pub treasury_ledger: GeneralAction39FreshAccountV1<'a>,
    pub fee_retirement_accumulator: GeneralAction39FreshAccountV1<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39CurrentAuthoritySnapshotV1<'a> {
    pub product_root: &'a ObservedRpcAccount,
    pub series_link: &'a ObservedRpcAccount,
    pub series_funding: &'a ObservedRpcAccount,
    pub series_registry: &'a ObservedRpcAccount,
    pub registry_program: &'a ObservedRpcAccount,
    pub registry_program_data: &'a ObservedRpcAccount,
    pub registry_release: &'a ObservedRpcAccount,
    pub capability_profile: &'a ObservedRpcAccount,
    pub source_release: &'a ObservedRpcAccount,
    pub compiler_bundle: &'a ObservedRpcAccount,
    pub revenue_policy_preimage: &'a ObservedRpcAccount,
    pub series_plan: &'a ObservedRpcAccount,
    pub funding_terms: &'a ObservedRpcAccount,
    pub product_template: &'a ObservedRpcAccount,
    pub native_claim_basis: &'a ObservedRpcAccount,
    pub recovery_policy: &'a ObservedRpcAccount,
    pub price_measure_policy: &'a ObservedRpcAccount,
    pub funding_quote: &'a ObservedRpcAccount,
    pub attachment_plan: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39CreationSnapshotV1<'a> {
    pub indexed_settlement_root: GeneralAction39FreshAccountV1<'a>,
    pub settlement_cash_pot: GeneralAction39FreshAccountV1<'a>,
    pub final_pot: GeneralAction39FreshAccountV1<'a>,
    pub frozen_order_locator: GeneralAction39FreshAccountV1<'a>,
    pub candidate_slice_index: GeneralAction39FreshAccountV1<'a>,
    pub payer: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub clock_sysvar: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction39ChainSnapshotV1<'a> {
    pub common: GeneralAction39CommonSnapshotV1<'a>,
    pub fee: GeneralAction39FeeSnapshotV1<'a>,
    pub current: GeneralAction39CurrentAuthoritySnapshotV1<'a>,
    pub creation: GeneralAction39CreationSnapshotV1<'a>,
    pub order_pages: &'a [&'a ObservedRpcAccount],
    pub address_lookup_table: &'a ObservedRpcAccount,
}

#[derive(Clone, Debug)]
pub struct ChainDerivedGeneralAction39MaterialV1 {
    release_key: String,
    release_manifest_sha256: [u8; 32],
    observed_slot: u64,
    valid_before_slot: u64,
    generation: u64,
    selected_ordinal: u64,
    state_sha256: [u8; 32],
    epoch: [u8; 32],
    selected_node: [u8; 32],
    revenue_policy: RevenuePolicyV2,
    payer: Address,
    ordered_accounts: Vec<AccountMeta>,
    lookup_table: StructuredAddressLookupTableV1,
}

impl ChainDerivedGeneralAction39MaterialV1 {
    pub fn release_key(&self) -> &str { &self.release_key }
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    pub const fn state_sha256(&self) -> [u8; 32] { self.state_sha256 }
    pub const fn payer(&self) -> Address { self.payer }
    pub fn account_metas(&self) -> &[AccountMeta] { &self.ordered_accounts }
    pub fn role_labels(&self) -> impl Iterator<Item = &'static str> {
        GENERAL_ACTION39_FIXED_ROLE_LABELS_V1.into_iter().chain(
            (0..self.ordered_accounts.len() - GENERAL_ACTION39_FIXED_ACCOUNT_COUNT_V1)
                .map(order_page_role_label),
        )
    }
    pub fn cursor(&self) -> ResumableWorkflowCursor {
        ResumableWorkflowCursor {
            workflow_id: action39_workflow_id(self.release_manifest_sha256, self.epoch),
            lane: WorkflowLane::Candidate,
            generation: self.generation,
            position: WorkflowPosition { phase: 39, item: self.selected_ordinal },
            observed_state_sha256: self.state_sha256,
        }
    }
    pub fn unsigned_instruction(&self, release: &IndexedProgramRelease) -> Result<OwnedInstructionDraft> {
        authenticate_material_release(self, release)?;
        OwnedInstructionDraft::checked_release_general_action39_v1(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE.into(),
                schema: OWNER_SCHEMA.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![ExactEquation {
                name: "finalized absence proves zero preexisting fresh-account rent principal".into(),
                unit: IntegerUnit::Lamports,
                left: 0,
                right: 0,
            }],
            self.epoch,
            self.selected_node,
            self.revenue_policy,
        )
        .map_err(map_construction)
    }
    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(
            self.payer,
            release.program_id,
            release.release_manifest_sha256,
            transport,
        )
        .and_then(|builder| builder.build_exact_v0(
            draft,
            self.lookup_table.table(),
            self.lookup_table.observed_slot(),
            self.lookup_table.state_sha256(),
        ))
        .map_err(map_construction)
    }
}

/// Derive the only current action-39 instruction from one finalized snapshot.
pub fn derive_general_action39_material_v1(
    release: &IndexedProgramRelease,
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
) -> Result<ChainDerivedGeneralAction39MaterialV1> {
    authenticate_release(release)?;
    let present = present_accounts(snapshot);
    authenticate_provenance(release, &present, snapshot.address_lookup_table)?;
    let observed_slot = snapshot.common.epoch.provenance.slot;
    authenticate_fresh_accounts(release, observed_slot, snapshot)?;
    authenticate_role_shapes(release, snapshot)?;

    let epoch = GeneralEpochV6AccountV1::decode(&snapshot.common.epoch.data)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let window = CandidateWindowV5AccountV1::decode(&snapshot.common.window.data)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let node = AdmissionNodeV4AccountV1::decode(&snapshot.common.selected_node.data)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let feed = CandidateFeedHeaderV2::decode_account(&snapshot.common.retained_feed.data, true)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let epoch_address = snapshot.common.epoch.address.to_bytes();
    let node_address = snapshot.common.selected_node.address.to_bytes();
    let candidate = node.base().settlement_candidate_id.bytes();
    if epoch.phase != GeneralEpochPhaseV1::Frozen
        || epoch.window.bytes() != snapshot.common.window.address.to_bytes()
        || epoch.market_binding.bytes() != snapshot.common.market_binding.address.to_bytes()
        || epoch.market_runtime.bytes() != snapshot.common.market_runtime.address.to_bytes()
        || epoch.economic_domain.bytes() != snapshot.common.economic_domain.address.to_bytes()
        || node.base().node.bytes() != node_address
        || node.base().epoch.bytes() != epoch_address
        || node.base().market.bytes() != snapshot.common.market_runtime.address.to_bytes()
        || node.base().epoch_generation != epoch.generation
        || window.base().epoch.bytes() != epoch_address
        || window.base().market.bytes() != snapshot.common.market_runtime.address.to_bytes()
        || window.base().best_candidate_node.bytes() != node_address
        || window.base().best_settlement_candidate_id.bytes() != candidate
        || window.base().epoch_generation != epoch.generation
        || feed.epoch.bytes() != epoch_address
        || feed.node.bytes() != node_address
        || feed.market.bytes() != snapshot.common.market_runtime.address.to_bytes()
        || feed.order_set != epoch.order_set
        || feed.settlement_candidate_id.bytes() != candidate
    {
        return Err(GeneralAction39MaterialError::ChainAuthority);
    }

    let policy = decode_revenue_policy_v2(&snapshot.current.revenue_policy_preimage.data)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    authenticate_revenue_authority(release.program_id, snapshot, &policy)?;
    authenticate_fresh_pdas(release.program_id, snapshot, epoch_address, candidate)?;
    let lookup_table = StructuredAddressLookupTableV1::authenticate(snapshot.address_lookup_table)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let ordered_accounts = ordered_metas(snapshot);
    authenticate_lookup_coverage(&lookup_table, &ordered_accounts)?;
    let state_sha256 = action39_state_sha256(&present, snapshot, policy);
    let valid_before_slot = observed_slot
        .checked_add(GENERAL_ACTION39_VALIDITY_SLOTS_V1)
        .ok_or(GeneralAction39MaterialError::Arithmetic)?;
    Ok(ChainDerivedGeneralAction39MaterialV1 {
        release_key: release.key(),
        release_manifest_sha256: release.release_manifest_sha256,
        observed_slot,
        valid_before_slot,
        generation: epoch.generation,
        selected_ordinal: node.base().ordinal,
        state_sha256,
        epoch: epoch_address,
        selected_node: node_address,
        revenue_policy: policy,
        payer: snapshot.creation.payer.address,
        ordered_accounts,
        lookup_table,
    })
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    let action = ExtensionAction::GeneralV2(GeneralV2Action::InitializeSettlementRoot);
    let family = action.family();
    let coordinate = CanonicalIntentCoordinate {
        family_tag: family.tag(), family_version: family.version(), local_action: action.local_tag(),
    };
    if release.validate().is_err()
        || !release.families.contains(&CanonicalFamily::General)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(GeneralAction39MaterialError::CheckedRelease);
    }
    Ok(())
}

fn present_accounts<'a>(snapshot: GeneralAction39ChainSnapshotV1<'a>) -> Vec<&'a ObservedRpcAccount> {
    let c = snapshot.common;
    let f = snapshot.fee;
    let a = snapshot.current;
    let x = snapshot.creation;
    let mut out = vec![
        c.epoch, c.window, c.selected_node, c.retained_feed, c.market_binding, c.market_runtime,
        c.economic_domain, c.price_grid, c.realm, c.collateral_profile, c.collateral_policy,
        c.collateral_token_program, c.market_instance, c.market_genesis, f.batch_policy,
        f.treasury_service_ledger, f.revenue_policy_record, a.product_root, a.series_link,
        a.series_funding, a.series_registry, a.registry_program, a.registry_program_data,
        a.registry_release, a.capability_profile, a.source_release, a.compiler_bundle,
        a.revenue_policy_preimage, a.series_plan, a.funding_terms, a.product_template,
        a.native_claim_basis, a.recovery_policy, a.price_measure_policy, a.funding_quote,
        a.attachment_plan, x.payer, x.system_program, x.rent_sysvar, x.clock_sysvar,
    ];
    out.extend_from_slice(snapshot.order_pages);
    out
}

fn authenticate_provenance(
    release: &IndexedProgramRelease,
    accounts: &[&ObservedRpcAccount],
    lookup: &ObservedRpcAccount,
) -> Result<()> {
    let first = accounts.first().ok_or(GeneralAction39MaterialError::ChainSnapshot)?;
    if accounts.len() < 41
        || first.provenance.slot == 0
        || first.provenance.commitment != RpcCommitment::Finalized
        || first.provenance.release_key != release.key()
        || first.provenance.cluster_key.trim().is_empty()
    {
        return Err(GeneralAction39MaterialError::ChainSnapshot);
    }
    let mut unique = std::collections::BTreeSet::new();
    for account in accounts.iter().copied().chain(core::iter::once(lookup)) {
        if account.address == Address::default()
            || !unique.insert(account.address)
            || account.provenance.slot != first.provenance.slot
            || account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.release_key != first.provenance.release_key
            || account.provenance.cluster_key != first.provenance.cluster_key
        {
            return Err(GeneralAction39MaterialError::ChainSnapshot);
        }
    }
    Ok(())
}

fn fresh_accounts<'a>(
    snapshot: GeneralAction39ChainSnapshotV1<'a>,
) -> [GeneralAction39FreshAccountV1<'a>; 9] {
    [
        snapshot.common.selected_fee_record, snapshot.fee.recipient_allocation,
        snapshot.fee.treasury_ledger, snapshot.fee.fee_retirement_accumulator,
        snapshot.creation.indexed_settlement_root, snapshot.creation.settlement_cash_pot,
        snapshot.creation.final_pot, snapshot.creation.frozen_order_locator,
        snapshot.creation.candidate_slice_index,
    ]
}

fn authenticate_fresh_accounts(
    release: &IndexedProgramRelease,
    slot: u64,
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
) -> Result<()> {
    let mut unique = std::collections::BTreeSet::new();
    for fresh in fresh_accounts(snapshot) {
        if fresh.address == Address::default()
            || fresh.absence.slot() != slot
            || fresh.absence.release_key() != release.key()
            || fresh.absence.receive_sequence() == 0
            || !unique.insert(fresh.address)
        {
            return Err(GeneralAction39MaterialError::FreshAccount);
        }
    }
    for present in present_accounts(snapshot) {
        if unique.contains(&present.address) {
            return Err(GeneralAction39MaterialError::FreshAccount);
        }
    }
    Ok(())
}

fn authenticate_role_shapes(
    release: &IndexedProgramRelease,
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
) -> Result<()> {
    if !(1..=4).contains(&snapshot.order_pages.len())
        || snapshot.creation.payer.owner != solana_sdk_ids::system_program::ID
        || snapshot.creation.payer.executable
        || snapshot.creation.payer.lamports == 0
        || snapshot.creation.system_program.address != solana_sdk_ids::system_program::ID
        || !snapshot.creation.system_program.executable
        || snapshot.creation.rent_sysvar.address != solana_sdk_ids::sysvar::rent::ID
        || snapshot.creation.clock_sysvar.address != solana_sdk_ids::sysvar::clock::ID
        || snapshot.current.registry_program.address != release.program_id
        || snapshot.current.registry_program_data.address != release.program_data
        || !snapshot.current.registry_program.executable
        || !snapshot.common.collateral_token_program.executable
    {
        return Err(GeneralAction39MaterialError::ChainAuthority);
    }
    let program_state = present_accounts(snapshot);
    for (index, account) in program_state.iter().enumerate() {
        if matches!(index, 11 | 21 | 22 | 36 | 37 | 38 | 39) {
            continue;
        }
        if account.owner != release.program_id || account.executable || account.lamports == 0 {
            return Err(GeneralAction39MaterialError::ChainAuthority);
        }
    }
    Ok(())
}

fn authenticate_fresh_pdas(
    program: Address,
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
    epoch: [u8; 32],
    candidate: [u8; 32],
) -> Result<()> {
    let selected = Address::find_program_address(&[SELECTED_FEE_RECORD_SEED_DOMAIN_V1, &candidate], &program).0;
    let selected_bytes = selected.to_bytes();
    let root = Address::find_program_address(&[SETTLEMENT_ROOT_SEED_DOMAIN_V1, &epoch, &candidate], &program).0;
    let root_bytes = root.to_bytes();
    let final_seeds = FinalPotSeedTupleV1::new(
        clutch_general_v2_contract::Id32::from_bytes(epoch),
        clutch_general_v2_contract::Id32::from_bytes(candidate),
    ).map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let expected = [
        selected,
        Address::find_program_address(&[RECIPIENT_ALLOCATION_SEED_DOMAIN_V1, &selected_bytes], &program).0,
        Address::find_program_address(&[TREASURY_LEDGER_SEED_DOMAIN_V1, &selected_bytes], &program).0,
        Address::find_program_address(&[FEE_RETIREMENT_ACCUMULATOR_SEED_DOMAIN_V1, &selected_bytes], &program).0,
        root,
        Address::find_program_address(&[SETTLEMENT_CASH_POT_SEED_DOMAIN_V1, &epoch, &candidate], &program).0,
        Address::find_program_address(&[final_seeds.domain(), final_seeds.epoch(), final_seeds.settlement_candidate()], &program).0,
        Address::find_program_address(&[FROZEN_ORDER_LOCATOR_SEED_DOMAIN_V1, &root_bytes], &program).0,
        Address::find_program_address(&[CANDIDATE_ORDER_SLICE_INDEX_SEED_DOMAIN_V1, &root_bytes], &program).0,
    ];
    let observed = fresh_accounts(snapshot).map(|fresh| fresh.address);
    if observed != expected { return Err(GeneralAction39MaterialError::FreshAccount); }
    Ok(())
}

fn authenticate_revenue_authority(
    program: Address,
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
    policy: &RevenuePolicyV2,
) -> Result<()> {
    const RECORD_SEED: &[u8] = b"dragons-clutch:revenue-policy:v1";
    const PREIMAGE_SEED: &[u8] = b"dc:revenue-preimage:v2";
    let realm = snapshot.common.realm.address.to_bytes();
    let record = RevenuePolicyRecordV2::decode(&snapshot.fee.revenue_policy_record.data)
        .map_err(|_| GeneralAction39MaterialError::ChainAuthority)?;
    let (expected_record, record_bump) =
        Address::find_program_address(&[RECORD_SEED, &realm], &program);
    let (expected_preimage, preimage_bump) =
        Address::find_program_address(&[PREIMAGE_SEED, &realm], &program);
    let record_floor = record
        .terminal_payer_principal
        .checked_add(record.terminal_donation_floor)
        .ok_or(GeneralAction39MaterialError::Arithmetic)?;
    let preimage_floor = record
        .policy_preimage_payer_principal
        .checked_add(record.policy_preimage_donation_floor)
        .ok_or(GeneralAction39MaterialError::Arithmetic)?;
    if record.realm.bytes() != realm
        || record.binds_policy(policy).is_err()
        || snapshot.fee.revenue_policy_record.address != expected_record
        || record.stored_bump != record_bump
        || snapshot.fee.revenue_policy_record.lamports < record_floor
        || snapshot.current.revenue_policy_preimage.address != expected_preimage
        || record.policy_preimage_stored_bump != preimage_bump
        || snapshot.current.revenue_policy_preimage.lamports < preimage_floor
    {
        return Err(GeneralAction39MaterialError::ChainAuthority);
    }
    Ok(())
}

fn ordered_metas(snapshot: GeneralAction39ChainSnapshotV1<'_>) -> Vec<AccountMeta> {
    let c = snapshot.common;
    let f = snapshot.fee;
    let a = snapshot.current;
    let x = snapshot.creation;
    let mut out = vec![
        AccountMeta::new(c.epoch.address, false), AccountMeta::new(c.window.address, false),
        ro(c.selected_node), ro(c.retained_feed), ro(c.market_binding), ro(c.market_runtime),
        ro(c.economic_domain), ro(c.price_grid), ro(c.realm), ro(c.collateral_profile),
        ro(c.collateral_policy), ro(c.collateral_token_program), ro(c.market_instance),
        ro(c.market_genesis), fresh_writable(c.selected_fee_record),
        fresh_writable(f.recipient_allocation), ro(f.batch_policy),
        AccountMeta::new(f.treasury_service_ledger.address, false), ro(f.revenue_policy_record),
        fresh_writable(f.treasury_ledger), fresh_writable(f.fee_retirement_accumulator),
        ro(a.product_root), ro(a.series_link), ro(a.series_funding), ro(a.series_registry),
        ro(a.registry_program), ro(a.registry_program_data), ro(a.registry_release),
        ro(a.capability_profile), ro(a.source_release), ro(a.compiler_bundle),
        ro(a.revenue_policy_preimage), ro(a.series_plan), ro(a.funding_terms),
        ro(a.product_template), ro(a.native_claim_basis), ro(a.recovery_policy),
        ro(a.price_measure_policy), ro(a.funding_quote), ro(a.attachment_plan),
        fresh_writable(x.indexed_settlement_root), fresh_writable(x.settlement_cash_pot),
        fresh_writable(x.final_pot), fresh_writable(x.frozen_order_locator),
        fresh_writable(x.candidate_slice_index), AccountMeta::new(x.payer.address, true),
        ro(x.system_program), ro(x.rent_sysvar), ro(x.clock_sysvar),
    ];
    out.extend(snapshot.order_pages.iter().map(|page| ro(page)));
    out
}

fn ro(account: &ObservedRpcAccount) -> AccountMeta { AccountMeta::new_readonly(account.address, false) }
fn fresh_writable(account: GeneralAction39FreshAccountV1<'_>) -> AccountMeta { AccountMeta::new(account.address, false) }

fn authenticate_lookup_coverage(
    lookup: &StructuredAddressLookupTableV1,
    accounts: &[AccountMeta],
) -> Result<()> {
    let table = lookup.table();
    if accounts.iter().filter(|meta| !meta.is_signer).any(|meta| !table.addresses.contains(&meta.pubkey)) {
        return Err(GeneralAction39MaterialError::ChainAuthority);
    }
    Ok(())
}

fn action39_state_sha256(
    accounts: &[&ObservedRpcAccount],
    snapshot: GeneralAction39ChainSnapshotV1<'_>,
    policy: RevenuePolicyV2,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/general-action39-state/v1\0");
    for account in accounts {
        hash.update(account.address.to_bytes());
        hash.update(account.owner.to_bytes());
        hash.update(account.lamports.to_le_bytes());
        hash.update(Sha256::digest(&account.data));
    }
    for fresh in fresh_accounts(snapshot) {
        hash.update(fresh.address.to_bytes());
        hash.update(fresh.absence.slot().to_le_bytes());
        hash.update(fresh.absence.receive_sequence().to_le_bytes());
    }
    hash.update(policy.treasury_owner);
    hash.finalize().into()
}

fn action39_workflow_id(release: [u8; 32], epoch: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/general-action39-workflow/v1\0");
    hash.update(release);
    hash.update(epoch);
    hash.finalize().into()
}

fn authenticate_material_release(
    material: &ChainDerivedGeneralAction39MaterialV1,
    release: &IndexedProgramRelease,
) -> Result<()> {
    authenticate_release(release)?;
    if material.release_key != release.key()
        || material.release_manifest_sha256 != release.release_manifest_sha256
        || material.observed_slot <= release.deployment_slot
        || material.valid_before_slot <= material.observed_slot
    {
        return Err(GeneralAction39MaterialError::CheckedRelease);
    }
    Ok(())
}

fn order_page_role_label(index: usize) -> &'static str {
    match index { 0 => "order-page-v5-0", 1 => "order-page-v5-1", 2 => "order-page-v5-2", _ => "order-page-v5-3" }
}

fn map_construction(_: ConstructionError) -> GeneralAction39MaterialError {
    GeneralAction39MaterialError::Construction
}
