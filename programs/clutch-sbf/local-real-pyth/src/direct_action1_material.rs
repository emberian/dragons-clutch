//! Chain-derived unsigned construction for current Direct action 1.
//!
//! The constructor consumes one named finalized ProductRoot snapshot and two
//! exact finalized absences. It derives the b1/v3 and b3 addresses, the
//! Product/General cursor, the immutable RevenuePolicyV2 preimage address,
//! all 41 privileges, and the empty sequence-zero wire. There is no generic
//! account vector, caller-authored action, payload, or legacy atomic-message
//! fallback at this boundary.

use crate::account_index::FinalizedAccountAbsence;
use crate::action_material::{
    chain_derived_direct_role_v2, decode_release_artifact, direct_role_label_v1,
    finish_chain_derived_direct_action1_material_v2, ActionFreshnessBoundaryV1,
    CanonicalAccountAbsenceV1, CanonicalActionMaterialErrorV1, CanonicalActionMaterialV1,
    DirectAccountRoleV1, StructuredAddressLookupTableV1, StructuredChainAccountV1,
};
use crate::direct_candidate_material::{authenticate_snapshot_set, sha256};
use crate::rpc_index::{IndexedProgramRelease, ObservedRpcAccount};
use crate::transaction_builder::{ExactEquation, IntegerUnit, ProtocolTransactionBuilder};
use crate::workflow_graph::ExplicitOperatorReleaseManifest;
use clutch_batch_policy_identity::revenue_policy_v2::{
    decode_revenue_policy_v2, revenue_policy_record_v2_id, revenue_policy_v2_digest,
};
use clutch_general_v2_contract::{
    MarketBindingV5, MarketRuntimeV3AccountV1, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_liveness::{RuntimeCompartmentKindV1, RuntimeCompartmentV1};
use clutch_product_series::{
    CompiledProductSeriesBundleV7, DirectGlobalLivenessPhaseV2, EvidenceOnlyRecoveryPolicyV1,
    FixedCodec, MarketFamilyCapabilityPolicyV1, MarketFamilyV1, MarketGenesisProfileV2,
    MarketInstancePreimageV2, MarketLifecyclePhaseV3, MarketLifecycleReplayPhaseV2,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4, RegistryCapabilityProfileV4,
    SeriesAttachmentPlanV6, SeriesFundingQuoteV6, SeriesFundingTermsV2, SeriesMarketLinkPhaseV3,
    SeriesPlanV5,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    MarketLifecycleReplayAccountV2, MarketLifecycleRootAccountV3,
    ProductDirectGlobalLivenessAccountV2, SeriesFundingAccountV5, SeriesMarketLinkAccountV3,
    SeriesRegistryAccountV4, PRODUCT_DIRECT_GLOBAL_LIVENESS_PDA_PREFIX_V2,
    SERIES_FUNDING_PDA_PREFIX_V1, SERIES_REGISTRY_PDA_PREFIX_V1,
};
use clutch_solana_layout::revenue::RevenuePolicyRecordV2;
use clutch_solana_layout::RealmAccount;
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    authenticate_source_release_account, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourceReleaseManifestV2,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_rent::Rent;
use std::collections::BTreeSet;
use std::str::FromStr;

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;

/// Semantic release row required by the current action-1 constructor.
pub const DIRECT_ACTION1_OWNER_PACKAGE_V2: &str = "clutch-direct-market-runtime";
/// Exact chain-derived action-1 material schema.
pub const DIRECT_ACTION1_OWNER_SCHEMA_V2: &str =
    "dragons-clutch/direct/initialize-market-chain-material/v2";
/// Action label presented to the operator and frontend.
pub const DIRECT_ACTION1_LABEL_V2: &str = "initialize-direct-market";
/// Fixed physical SBF frame width.
pub const DIRECT_ACTION1_ACCOUNT_COUNT_V2: usize = 41;
/// Checked bounded lifetime for a finalized action-1 snapshot.
pub const DIRECT_ACTION1_MAXIMUM_VALIDITY_SLOTS_V2: u64 = 32;
/// Exact message transport; action 1 has no legacy message fallback.
pub const DIRECT_ACTION1_TRANSPORT_V2: &str = "solana-v0-one-authenticated-alt";

const PRODUCT_ROOT_SEED_V3: &[u8] = b"dc:market-lifecycle-root:v1";
const PRODUCT_REPLAY_SEED_V2: &[u8] = b"dc:market-lifecycle-replay:v2";
const SERIES_LINK_SEED_V3: &[u8] = b"dc:series-market-link:v1";
const FAMILY_POLICY_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const DIRECT_ROOT_SEED_V3: &[u8] = b"dc:direct-market-root:v3";
const DIRECT_REPLAY_SEED_V1: &[u8] = b"dc:direct-action-replay:v1";
const DIRECT_LIVENESS_ROW_SEED_V2: &[u8] = b"dc:product-direct-row:v2";
const REALM_SEED_V1: &[u8] = b"dragons-clutch:realm:v1";
const REVENUE_RECORD_SEED_V2: &[u8] = b"dragons-clutch:revenue-policy:v1";
const REVENUE_PREIMAGE_SEED_V2: &[u8] = b"dc:revenue-preimage:v2";
const SNAPSHOT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/operator/direct-action1-product-root-snapshot/v2\0";
const WORKFLOW_DOMAIN_V2: &[u8] =
    b"dragons-clutch/operator/direct-action1-product-root-workflow/v2\0";

const SYSTEM_PROGRAM_TEXT: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_TEXT: &str = "SysvarRent111111111111111111111111111111111";
const CLOCK_SYSVAR_TEXT: &str = "SysvarC1ock11111111111111111111111111111111";
const SYSVAR_OWNER_TEXT: &str = "Sysvar1111111111111111111111111111111111111";

/// Named finalized General V5 prefix consumed by Product's action-1 owner.
#[derive(Clone, Copy, Debug)]
pub struct DirectAction1GeneralSnapshotV2<'a> {
    pub market_binding_v5: &'a ObservedRpcAccount,
    pub market_runtime_v3: &'a ObservedRpcAccount,
    pub product_root_v3: &'a ObservedRpcAccount,
    pub founder_series_link_v3: &'a ObservedRpcAccount,
    pub series_funding_v5: &'a ObservedRpcAccount,
    pub series_registry_v4: &'a ObservedRpcAccount,
    pub registry_program: &'a ObservedRpcAccount,
    pub registry_programdata: &'a ObservedRpcAccount,
    pub registry_release_artifact: &'a ObservedRpcAccount,
    pub capability_profile_artifact: &'a ObservedRpcAccount,
    pub source_release_v2: &'a ObservedRpcAccount,
    pub compiler_bundle_v7: &'a ObservedRpcAccount,
    pub market_instance_v2: &'a ObservedRpcAccount,
    pub realm: &'a ObservedRpcAccount,
    pub revenue_policy_record_v2: &'a ObservedRpcAccount,
    pub revenue_policy_preimage_v2: &'a ObservedRpcAccount,
    pub series_plan_v5: &'a ObservedRpcAccount,
    pub funding_terms_v2: &'a ObservedRpcAccount,
    pub source_template_v4: &'a ObservedRpcAccount,
    pub native_claim_basis_v1: &'a ObservedRpcAccount,
    pub recovery_policy_v1: &'a ObservedRpcAccount,
    pub price_measure_policy_v1: &'a ObservedRpcAccount,
    pub market_genesis_v2: &'a ObservedRpcAccount,
    pub funding_quote_v6: &'a ObservedRpcAccount,
    pub attachment_plan_v6: &'a ObservedRpcAccount,
}

impl<'a> DirectAction1GeneralSnapshotV2<'a> {
    fn ordered(self) -> [&'a ObservedRpcAccount; 25] {
        [
            self.market_binding_v5,
            self.market_runtime_v3,
            self.product_root_v3,
            self.founder_series_link_v3,
            self.series_funding_v5,
            self.series_registry_v4,
            self.registry_program,
            self.registry_programdata,
            self.registry_release_artifact,
            self.capability_profile_artifact,
            self.source_release_v2,
            self.compiler_bundle_v7,
            self.market_instance_v2,
            self.realm,
            self.revenue_policy_record_v2,
            self.revenue_policy_preimage_v2,
            self.series_plan_v5,
            self.funding_terms_v2,
            self.source_template_v4,
            self.native_claim_basis_v1,
            self.recovery_policy_v1,
            self.price_measure_policy_v1,
            self.market_genesis_v2,
            self.funding_quote_v6,
            self.attachment_plan_v6,
        ]
    }
}

/// Exact finalized Product-root action-1 snapshot.
///
/// The fresh b1/v3 and b3 addresses are values to be independently derived;
/// their absence witnesses must come from the same release scan and slot.
#[derive(Clone, Copy, Debug)]
pub struct DirectAction1ProductRootSnapshotV2<'a> {
    pub general: DirectAction1GeneralSnapshotV2<'a>,
    pub product_replay_v2: &'a ObservedRpcAccount,
    pub family_capability_policy_v1: &'a ObservedRpcAccount,
    pub product_direct_global_liveness_v2: &'a ObservedRpcAccount,
    pub liveness_compartments: [&'a ObservedRpcAccount; 7],
    pub fresh_direct_root_v3: Address,
    pub fresh_direct_root_absence: &'a FinalizedAccountAbsence,
    pub fresh_direct_replay_v1: Address,
    pub fresh_direct_replay_absence: &'a FinalizedAccountAbsence,
    pub payer: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub clock_sysvar: &'a ObservedRpcAccount,
}

impl<'a> DirectAction1ProductRootSnapshotV2<'a> {
    fn present_accounts(self) -> Vec<&'a ObservedRpcAccount> {
        let mut accounts = self.general.ordered().to_vec();
        accounts.extend([
            self.product_replay_v2,
            self.family_capability_policy_v1,
            self.product_direct_global_liveness_v2,
        ]);
        accounts.extend(self.liveness_compartments);
        accounts.extend([
            self.payer,
            self.system_program,
            self.rent_sysvar,
            self.clock_sysvar,
        ]);
        accounts
    }

    fn ordered_addresses(self) -> [Address; DIRECT_ACTION1_ACCOUNT_COUNT_V2] {
        let general = self.general.ordered();
        [
            general[0].address,
            general[1].address,
            general[2].address,
            general[3].address,
            general[4].address,
            general[5].address,
            general[6].address,
            general[7].address,
            general[8].address,
            general[9].address,
            general[10].address,
            general[11].address,
            general[12].address,
            general[13].address,
            general[14].address,
            general[15].address,
            general[16].address,
            general[17].address,
            general[18].address,
            general[19].address,
            general[20].address,
            general[21].address,
            general[22].address,
            general[23].address,
            general[24].address,
            self.product_replay_v2.address,
            self.family_capability_policy_v1.address,
            self.product_direct_global_liveness_v2.address,
            self.liveness_compartments[0].address,
            self.liveness_compartments[1].address,
            self.liveness_compartments[2].address,
            self.liveness_compartments[3].address,
            self.liveness_compartments[4].address,
            self.liveness_compartments[5].address,
            self.liveness_compartments[6].address,
            self.fresh_direct_root_v3,
            self.fresh_direct_replay_v1,
            self.payer.address,
            self.system_program.address,
            self.rent_sysvar.address,
            self.clock_sysvar.address,
        ]
    }
}

/// Opaque action-1 result with inspectable chain and transport commitments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction1CanonicalMaterialV2 {
    snapshot_id: [u8; 32],
    revenue_policy_digest: [u8; 32],
    lookup_table: Address,
    direct_root_rent_lamports: u64,
    direct_replay_rent_lamports: u64,
    canonical: CanonicalActionMaterialV1,
}

impl DirectAction1CanonicalMaterialV2 {
    pub const fn snapshot_id(&self) -> [u8; 32] {
        self.snapshot_id
    }
    pub const fn revenue_policy_digest(&self) -> [u8; 32] {
        self.revenue_policy_digest
    }
    pub const fn lookup_table(&self) -> Address {
        self.lookup_table
    }
    pub const fn direct_root_rent_lamports(&self) -> u64 {
        self.direct_root_rent_lamports
    }
    pub const fn direct_replay_rent_lamports(&self) -> u64 {
        self.direct_replay_rent_lamports
    }
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 {
        &self.canonical
    }
}

/// Construct the sole current action-1 request from finalized chain state.
pub fn construct_direct_action1_material_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectAction1ProductRootSnapshotV2<'_>,
    lookup_table: &StructuredAddressLookupTableV1,
) -> Result<DirectAction1CanonicalMaterialV2> {
    let present = snapshot.present_accounts();
    authenticate_snapshot_set(release, freshness, &present)?;
    if freshness.maximum_validity_slots != DIRECT_ACTION1_MAXIMUM_VALIDITY_SLOTS_V2
        || builder.payer() != snapshot.payer.address
        || lookup_table.observed_slot() > freshness.observed_slot
        || lookup_table.cluster_key() != snapshot.general.product_root_v3.provenance.cluster_key
    {
        return invalid();
    }
    require_distinct_action1(snapshot.ordered_addresses())?;
    require_program_owned(release, snapshot.general.product_root_v3)?;
    require_program_owned(release, snapshot.general.founder_series_link_v3)?;

    let root = MarketLifecycleRootAccountV3::decode(&snapshot.general.product_root_v3.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let binding = root.state.binding_ref();
    let binding_id = binding
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link = SeriesMarketLinkAccountV3::decode(&snapshot.general.founder_series_link_v3.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let link_binding = link.state.binding_ref();
    let link_id = link
        .state
        .semantic_id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let root_pda = pda(
        release.program_id,
        &[
            PRODUCT_ROOT_SEED_V3,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
    );
    let link_pda = pda(
        release.program_id,
        &[
            SERIES_LINK_SEED_V3,
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
        ],
    );
    if snapshot.general.product_root_v3.address != root_pda.0
        || root.stored_bump != root_pda.1
        || snapshot.general.product_root_v3.lamports < root.rent_principal_lamports
        || snapshot.general.founder_series_link_v3.address != link_pda.0
        || link.stored_bump != link_pda.1
        || snapshot.general.founder_series_link_v3.lamports
            < link
                .state
                .rent_principal_lamports()
                .checked_add(link.state.current_donation_lamports())
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?
        || root.state.phase() != MarketLifecyclePhaseV3::Founding
        || link.state.phase() != SeriesMarketLinkPhaseV3::PendingMarket
        || !root.state.foundation().complete()
        || root.state.capital().principal_remaining_lamports != 0
        || !root
            .state
            .product_families()
            .admits_new_child(MarketFamilyV1::Direct)
        || link_binding.market_root_account_id.bytes()
            != snapshot.general.product_root_v3.address.to_bytes()
        || link_binding.market_binding_id != binding_id
        || link_binding.market_instance_id != binding.market_instance_id
        || link_binding.generation != binding.generation
    {
        return invalid();
    }

    let general = authenticate_general_and_revenue(release, snapshot.general, &root, &link)?;
    let replay = authenticate_product_replay(release, snapshot.product_replay_v2, binding)?;
    let family_policy = authenticate_family_policy(
        release,
        snapshot.family_capability_policy_v1,
        replay.state.binding(),
        binding,
    )?;
    if !family_policy.is_enabled(MarketFamilyV1::Direct)
        || family_policy.realm_id != binding.realm_id
        || family_policy.collateral_profile_id != binding.collateral_profile_id
        || family_policy.registry_capability_profile_id.content_id()
            != binding.capability_profile_id
    {
        return invalid();
    }
    authenticate_direct_liveness(
        release,
        snapshot,
        binding,
        general.product_preauthorization_id(),
    )?;

    let direct_root = pda(
        release.program_id,
        &[
            DIRECT_ROOT_SEED_V3,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
    );
    let direct_replay = pda(
        release.program_id,
        &[DIRECT_REPLAY_SEED_V1, direct_root.0.as_ref()],
    );
    authenticate_absence(
        release,
        freshness,
        snapshot.fresh_direct_root_v3,
        direct_root.0,
        snapshot.fresh_direct_root_absence,
    )?;
    authenticate_absence(
        release,
        freshness,
        snapshot.fresh_direct_replay_v1,
        direct_replay.0,
        snapshot.fresh_direct_replay_absence,
    )?;
    require_system_roles(snapshot)?;

    let rent: Rent = bincode::deserialize(&snapshot.rent_sysvar.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let direct_root_rent_lamports =
        rent.minimum_balance(clutch_solana_layout::registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3);
    let direct_replay_rent_lamports =
        rent.minimum_balance(clutch_solana_layout::registry::DIRECT_ACTION_REPLAY_ACCOUNT_BYTES);
    if direct_root_rent_lamports == 0 || direct_replay_rent_lamports == 0 {
        return invalid();
    }

    let addresses = snapshot.ordered_addresses();
    let roles = action1_roles();
    let mut metas = Vec::with_capacity(DIRECT_ACTION1_ACCOUNT_COUNT_V2);
    let mut account_roles = Vec::with_capacity(DIRECT_ACTION1_ACCOUNT_COUNT_V2);
    for (index, (address, role)) in addresses.into_iter().zip(roles).enumerate() {
        let writable = matches!(index, 2 | 3 | 25 | 27 | 35 | 36 | 37);
        let signer = index == 37;
        metas.push(AccountMeta {
            pubkey: address,
            is_signer: signer,
            is_writable: writable,
        });
        let projected =
            chain_derived_direct_role_v2(direct_role_label_v1(role), address, writable, signer);
        account_roles.push(projected);
    }
    let equations = vec![
        ExactEquation {
            name: "direct-action1-root-rent-debit-lamports".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(direct_root_rent_lamports),
            right: u128::from(direct_root_rent_lamports),
        },
        ExactEquation {
            name: "direct-action1-replay-rent-debit-lamports".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(direct_replay_rent_lamports),
            right: u128::from(direct_replay_rent_lamports),
        },
    ];
    let snapshot_id = snapshot_id(release, snapshot, lookup_table);
    let account_absences = vec![
        CanonicalAccountAbsenceV1::new(
            35,
            direct_role_label_v1(roles[35]),
            snapshot.fresh_direct_root_v3,
            snapshot.fresh_direct_root_absence.release_key().to_string(),
            snapshot.fresh_direct_root_absence.slot(),
            snapshot.fresh_direct_root_absence.receive_sequence(),
        ),
        CanonicalAccountAbsenceV1::new(
            36,
            direct_role_label_v1(roles[36]),
            snapshot.fresh_direct_replay_v1,
            snapshot
                .fresh_direct_replay_absence
                .release_key()
                .to_string(),
            snapshot.fresh_direct_replay_absence.slot(),
            snapshot.fresh_direct_replay_absence.receive_sequence(),
        ),
    ];
    let workflow_id: [u8; 32] = Sha256::new()
        .chain_update(WORKFLOW_DOMAIN_V2)
        .chain_update(binding.market_instance_id.bytes())
        .chain_update(binding.generation.to_le_bytes())
        .chain_update(snapshot.general.product_root_v3.address.as_ref())
        .finalize()
        .into();
    let canonical = finish_chain_derived_direct_action1_material_v2(
        release,
        manifest,
        builder,
        workflow_id,
        freshness,
        snapshot.general.product_root_v3.address,
        snapshot.general.product_root_v3.provenance.slot,
        binding.generation,
        snapshot_id,
        metas,
        account_roles,
        account_absences,
        equations,
        lookup_table,
    )?;
    Ok(DirectAction1CanonicalMaterialV2 {
        snapshot_id,
        revenue_policy_digest: general.revenue_policy_v2_digest().bytes(),
        lookup_table: lookup_table.account(),
        direct_root_rent_lamports,
        direct_replay_rent_lamports,
        canonical,
    })
}

fn authenticate_general_and_revenue(
    release: &IndexedProgramRelease,
    frame: DirectAction1GeneralSnapshotV2<'_>,
    root: &MarketLifecycleRootAccountV3,
    link: &SeriesMarketLinkAccountV3,
) -> Result<clutch_general_v2_contract::CurrentMarketAuthorityV5> {
    for account in [
        frame.market_binding_v5,
        frame.market_runtime_v3,
        frame.series_funding_v5,
        frame.series_registry_v4,
        frame.source_release_v2,
        frame.compiler_bundle_v7,
        frame.market_instance_v2,
        frame.realm,
        frame.revenue_policy_record_v2,
        frame.revenue_policy_preimage_v2,
        frame.series_plan_v5,
        frame.funding_terms_v2,
        frame.source_template_v4,
        frame.native_claim_basis_v1,
        frame.recovery_policy_v1,
        frame.price_measure_policy_v1,
        frame.market_genesis_v2,
        frame.funding_quote_v6,
        frame.attachment_plan_v6,
    ] {
        require_program_owned(release, account)?;
    }
    let market = MarketBindingV5::decode(&frame.market_binding_v5.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&frame.market_runtime_v3.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let relation = market.base().base();
    let current = market.authority();
    let binding = root.state.binding_ref();
    let link_binding = link.state.binding_ref();
    let market_pda = pda(
        release.program_id,
        &[
            MARKET_BINDING_SEED_DOMAIN_V1,
            &relation.market_instance_v2_id.bytes(),
        ],
    );
    let runtime_pda = pda(
        release.program_id,
        &[
            MARKET_RUNTIME_SEED_DOMAIN_V1,
            frame.market_binding_v5.address.as_ref(),
        ],
    );
    if frame.market_binding_v5.address != market_pda.0
        || relation.stored_bump != market_pda.1
        || frame.market_runtime_v3.address != runtime_pda.0
        || runtime.stored_bump != runtime_pda.1
        || runtime.market_binding.bytes() != frame.market_binding_v5.address.to_bytes()
        || relation.market.bytes() != frame.market_runtime_v3.address.to_bytes()
        || relation.market_instance_v2_id.bytes() != binding.market_instance_id.bytes()
        || relation.series_plan_v5_id.bytes() != link_binding.series_plan_id.bytes()
        || relation.series_funding_terms_v2_id.bytes() != link_binding.funding_terms_id.bytes()
        || current.product_market_root_account().bytes() != frame.product_root_v3.address.to_bytes()
        || current.product_market_binding_v3_id().bytes()
            != binding
                .id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
        || current.product_generation() != binding.generation
        || current.series_market_link_account().bytes()
            != frame.founder_series_link_v3.address.to_bytes()
        || current.series_market_link_v3_id().bytes()
            != link
                .state
                .semantic_id()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?
                .bytes()
        || current.series_ordinal() != link_binding.ordinal
        || current.series_funding_v5_account().bytes() != frame.series_funding_v5.address.to_bytes()
    {
        return invalid();
    }

    let registry = SeriesRegistryAccountV4::decode(&frame.series_registry_v4.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let funding = SeriesFundingAccountV5::decode(&frame.series_funding_v5.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let registry_pda = pda(
        release.program_id,
        &[
            SERIES_REGISTRY_PDA_PREFIX_V1,
            &registry.series_plan_id.bytes(),
        ],
    );
    let funding_pda = pda(
        release.program_id,
        &[
            SERIES_FUNDING_PDA_PREFIX_V1,
            &registry.series_plan_id.bytes(),
        ],
    );
    if frame.series_registry_v4.address != registry_pda.0
        || registry.stored_bump != registry_pda.1
        || !registry.activation_consumed
        || frame.series_registry_v4.lamports < registry.rent_principal_lamports
        || frame.series_funding_v5.address != funding_pda.0
        || funding.stored_bump != funding_pda.1
        || frame.series_funding_v5.lamports < funding.rent_principal_lamports
        || registry.series_plan_id != link_binding.series_plan_id
        || registry.funding_terms_id != link_binding.funding_terms_id
        || registry.compiler_bundle_id != link_binding.compiler_bundle_id
        || funding.state.series_plan_id != link_binding.series_plan_id
        || funding.state.funding_terms_id != link_binding.funding_terms_id
        || funding.state.funding_quote_id != link_binding.funding_quote_id
        || funding.state.attachment_plan_id != link_binding.attachment_plan_id
        || funding.state.compiler_bundle_id != link_binding.compiler_bundle_id
    {
        return invalid();
    }

    let registry_release_id = decode_release_artifact(
        release,
        release.program_id,
        StructuredChainAccountV1::present(frame.registry_program)?,
        StructuredChainAccountV1::present(frame.registry_programdata)?,
        StructuredChainAccountV1::present(frame.registry_release_artifact)?,
        release.release_manifest_sha256,
    )?;
    if registry.registry_release_id != registry_release_id
        || binding.registry_release_id != registry_release_id
        || frame.registry_program.address != release.program_id
        || frame.registry_programdata.address != release.program_data
    {
        return invalid();
    }

    authenticate_artifacts(release, frame, binding, link_binding, current, &registry)?;
    authenticate_source_release(release, frame.source_release_v2, binding.source_release_id)?;

    let realm = RealmAccount::decode(&frame.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let realm_pda = pda(release.program_id, &[REALM_SEED_V1, &realm.realm.bytes()]);
    let record_pda = pda(
        release.program_id,
        &[REVENUE_RECORD_SEED_V2, frame.realm.address.as_ref()],
    );
    let preimage_pda = pda(
        release.program_id,
        &[REVENUE_PREIMAGE_SEED_V2, frame.realm.address.as_ref()],
    );
    let record = RevenuePolicyRecordV2::decode(&frame.revenue_policy_record_v2.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy = decode_revenue_policy_v2(&frame.revenue_policy_preimage_v2.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    record
        .binds_policy(&policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let record_id = revenue_policy_record_v2_id(realm.realm.bytes(), &policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let policy_id = revenue_policy_v2_digest(&policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let record_floor = record
        .terminal_payer_principal
        .checked_add(record.terminal_donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    let preimage_floor = record
        .policy_preimage_payer_principal
        .checked_add(record.policy_preimage_donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
    if frame.realm.address != realm_pda.0
        || realm.stored_bump != realm_pda.1
        || realm.realm.bytes() != binding.realm_id.bytes()
        || frame.revenue_policy_record_v2.address != record_pda.0
        || record.stored_bump != record_pda.1
        || frame.revenue_policy_record_v2.lamports < record_floor
        || frame.revenue_policy_preimage_v2.address != preimage_pda.0
        || record.policy_preimage_stored_bump != preimage_pda.1
        || frame.revenue_policy_preimage_v2.lamports < preimage_floor
        || record.realm != realm.realm
        || current.revenue_policy_record_account().bytes()
            != frame.revenue_policy_record_v2.address.to_bytes()
        || current.revenue_policy_record_v2_id().bytes() != record_id.0
        || current.revenue_policy_v2_digest().bytes() != policy_id.0
        || current.treasury_owner().bytes() != record.treasury_owner.bytes()
    {
        return invalid();
    }
    Ok(current)
}

fn authenticate_artifacts(
    release: &IndexedProgramRelease,
    frame: DirectAction1GeneralSnapshotV2<'_>,
    binding: &clutch_product_series::MarketLifecycleBindingV3,
    link: &clutch_product_series::SeriesMarketLinkBindingV3,
    current: clutch_general_v2_contract::CurrentMarketAuthorityV5,
    registry: &SeriesRegistryAccountV4,
) -> Result<()> {
    let series = SeriesPlanV5::decode(&frame.series_plan_v5.data).map_err(|_| chain())?;
    let terms = SeriesFundingTermsV2::decode(&frame.funding_terms_v2.data).map_err(|_| chain())?;
    let template =
        ProductTemplateV4::decode(&frame.source_template_v4.data).map_err(|_| chain())?;
    let basis =
        NativeClaimBasisV1::decode(&frame.native_claim_basis_v1.data).map_err(|_| chain())?;
    let recovery = EvidenceOnlyRecoveryPolicyV1::decode(&frame.recovery_policy_v1.data)
        .map_err(|_| chain())?;
    let price =
        PriceMeasurePolicyV1::decode(&frame.price_measure_policy_v1.data).map_err(|_| chain())?;
    let genesis =
        MarketGenesisProfileV2::decode(&frame.market_genesis_v2.data).map_err(|_| chain())?;
    let quote = SeriesFundingQuoteV6::decode(&frame.funding_quote_v6.data).map_err(|_| chain())?;
    let attachment =
        SeriesAttachmentPlanV6::decode(&frame.attachment_plan_v6.data).map_err(|_| chain())?;
    let bundle = CompiledProductSeriesBundleV7::decode(&frame.compiler_bundle_v7.data)
        .map_err(|_| chain())?;
    let market =
        MarketInstancePreimageV2::decode(&frame.market_instance_v2.data).map_err(|_| chain())?;
    let capability = RegistryCapabilityProfileV4::decode(&frame.capability_profile_artifact.data)
        .map_err(|_| chain())?;
    let series_id = series.id().map_err(|_| chain())?;
    let terms_id = terms.id().map_err(|_| chain())?;
    let template_id = template.id().map_err(|_| chain())?;
    let basis_id = basis.id().map_err(|_| chain())?;
    let recovery_id = recovery.id().map_err(|_| chain())?;
    let price_id = price.id().map_err(|_| chain())?;
    let genesis_id = genesis.id().map_err(|_| chain())?;
    let quote_id = quote.id().map_err(|_| chain())?;
    let attachment_id = attachment.id().map_err(|_| chain())?;
    let bundle_id = bundle.id().map_err(|_| chain())?;
    let market_id = market.id().map_err(|_| chain())?;
    let capability_id = capability.id().map_err(|_| chain())?;
    for (account, kind, id) in [
        (
            frame.series_plan_v5,
            ArtifactKind::SeriesPlanV5,
            series_id.bytes(),
        ),
        (
            frame.funding_terms_v2,
            ArtifactKind::SeriesFundingTermsV2,
            terms_id.bytes(),
        ),
        (
            frame.source_template_v4,
            ArtifactKind::ProductTemplateV4,
            template_id.bytes(),
        ),
        (
            frame.native_claim_basis_v1,
            ArtifactKind::NativeClaimBasisV1,
            basis_id.bytes(),
        ),
        (
            frame.recovery_policy_v1,
            ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
            recovery_id.bytes(),
        ),
        (
            frame.price_measure_policy_v1,
            ArtifactKind::PriceMeasurePolicyV1,
            price_id.bytes(),
        ),
        (
            frame.market_genesis_v2,
            ArtifactKind::MarketGenesisProfileV2,
            genesis_id.bytes(),
        ),
        (
            frame.funding_quote_v6,
            ArtifactKind::SeriesFundingQuoteV6,
            quote_id.bytes(),
        ),
        (
            frame.attachment_plan_v6,
            ArtifactKind::SeriesAttachmentPlanV6,
            attachment_id.bytes(),
        ),
        (
            frame.compiler_bundle_v7,
            ArtifactKind::CompiledProductSeriesBundleV7,
            bundle_id.bytes(),
        ),
        (
            frame.market_instance_v2,
            ArtifactKind::MarketInstancePreimageV2,
            market_id.bytes(),
        ),
        (
            frame.capability_profile_artifact,
            ArtifactKind::RegistryCapabilityProfileV4,
            capability_id.bytes(),
        ),
    ] {
        authenticate_artifact_address(release, account, kind, id)?;
    }
    if series_id != link.series_plan_id
        || terms_id != link.funding_terms_id
        || template_id != binding.product_template_id
        || basis_id != binding.native_claim_basis_id
        || recovery_id != binding.recovery_policy_id
        || price_id != binding.price_measure_policy_id
        || genesis_id != binding.market_genesis_profile_id
        || quote_id != link.funding_quote_id
        || attachment_id != link.attachment_plan_id
        || bundle_id != link.compiler_bundle_id
        || market_id != binding.market_instance_id
        || capability_id.content_id() != binding.capability_profile_id
        || capability.registry_release_id().content_id() != registry.registry_release_id
        || registry.capability_profile_id != capability_id.content_id()
        || registry.compiler_bundle_id.content_id() != bundle_id.content_id()
        || current.compiler_bundle_v7_id().bytes() != bundle_id.bytes()
        || current.funding_quote_v6_id().bytes() != quote_id.bytes()
        || current.attachment_plan_v6_id().bytes() != attachment_id.bytes()
    {
        return invalid();
    }
    Ok(())
}

fn authenticate_source_release(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
    expected_authentication_id: clutch_product_series::ContentId,
) -> Result<()> {
    let manifest = SourceReleaseManifestV2::decode(&account.data).map_err(|_| chain())?;
    let recipe =
        PdaRecipeV3::source_release(manifest.id().map_err(|_| chain())?).map_err(|_| chain())?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    for index in 0..usize::from(recipe.seed_count()) {
        seeds.push(recipe.seed(index).map_err(|_| chain())?);
    }
    let derived = pda(release.program_id, &seeds);
    let authenticated = authenticate_source_release_account(
        runtime_key(release.program_id),
        account_view(account),
        RuntimeDerivedPdaV1 {
            program_id: runtime_key(release.program_id),
            recipe_id: recipe.id().map_err(|_| chain())?,
            address: runtime_key(derived.0),
            bump: derived.1,
        },
    )
    .map_err(|_| chain())?;
    if authenticated.id().bytes() != expected_authentication_id.bytes() {
        return invalid();
    }
    Ok(())
}

fn authenticate_product_replay(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
    binding: &clutch_product_series::MarketLifecycleBindingV3,
) -> Result<MarketLifecycleReplayAccountV2> {
    require_program_owned(release, account)?;
    let replay = MarketLifecycleReplayAccountV2::decode(&account.data).map_err(|_| chain())?;
    let replay_binding = replay.state.binding();
    let expected = pda(
        release.program_id,
        &[PRODUCT_REPLAY_SEED_V2, &binding.market_instance_id.bytes()],
    );
    if account.address != expected.0
        || replay.stored_bump != expected.1
        || account.lamports < replay.permanent_rent_principal_lamports
        || replay.state.phase() != MarketLifecycleReplayPhaseV2::FoundationSettled
        || binding.market_lifecycle_replay_account_id.bytes() != account.address.to_bytes()
        || replay_binding.replay_account_id.bytes() != account.address.to_bytes()
        || replay_binding.market_instance_id != binding.market_instance_id
        || replay_binding.generation != binding.generation
        || replay_binding.lifecycle_root_account_id.bytes()
            != snapshot_root_address(binding, release.program_id).to_bytes()
    {
        return invalid();
    }
    Ok(replay)
}

fn authenticate_family_policy(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
    replay: clutch_product_series::MarketLifecycleGenerationBindingV2,
    binding: &clutch_product_series::MarketLifecycleBindingV3,
) -> Result<MarketFamilyCapabilityPolicyV1> {
    require_program_owned(release, account)?;
    let policy = MarketFamilyCapabilityPolicyV1::decode(&account.data).map_err(|_| chain())?;
    let id = policy.id().map_err(|_| chain())?;
    authenticate_artifact_address(
        release,
        account,
        ArtifactKind::MarketFamilyCapabilityPolicyV1,
        id.bytes(),
    )?;
    if id.content_id() != replay.market_family_capability_policy_id
        || policy.realm_id != binding.realm_id
    {
        return invalid();
    }
    Ok(policy)
}

fn authenticate_direct_liveness(
    release: &IndexedProgramRelease,
    snapshot: DirectAction1ProductRootSnapshotV2<'_>,
    binding: &clutch_product_series::MarketLifecycleBindingV3,
    preauthorization: clutch_general_v2_contract::Id32,
) -> Result<()> {
    require_program_owned(release, snapshot.product_direct_global_liveness_v2)?;
    let manifest = ProductDirectGlobalLivenessAccountV2::decode(
        &snapshot.product_direct_global_liveness_v2.data,
    )
    .map_err(|_| chain())?;
    let expected = pda(
        release.program_id,
        &[
            PRODUCT_DIRECT_GLOBAL_LIVENESS_PDA_PREFIX_V2,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
    );
    if snapshot.product_direct_global_liveness_v2.address != expected.0
        || manifest.stored_bump != expected.1
        || snapshot.product_direct_global_liveness_v2.lamports
            < manifest
                .rent_principal_lamports
                .checked_add(manifest.state.manifest_initial_donation_lamports())
                .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?
        || manifest.state.phase() != DirectGlobalLivenessPhaseV2::Founding
        || manifest.state.account_id().bytes()
            != snapshot
                .product_direct_global_liveness_v2
                .address
                .to_bytes()
        || manifest.state.market_instance_id() != binding.market_instance_id
        || manifest.state.generation() != binding.generation
        || manifest.state.lifecycle_root_account().bytes()
            != snapshot.general.product_root_v3.address.to_bytes()
        || manifest.state.founder_preauthorization_id().bytes() != preauthorization.bytes()
        || manifest.state.global_bundle_binding_id() != binding.direct_global_liveness_binding_id
        || manifest.state.admitted_allocations() != 0
    {
        return invalid();
    }
    let kinds = [
        RuntimeCompartmentKindV1::Source,
        RuntimeCompartmentKindV1::Candidate,
        RuntimeCompartmentKindV1::Clearing,
        RuntimeCompartmentKindV1::Settlement,
        RuntimeCompartmentKindV1::Resolution,
        RuntimeCompartmentKindV1::Retirement,
        RuntimeCompartmentKindV1::Recovery,
    ];
    for (index, (account, kind)) in snapshot
        .liveness_compartments
        .into_iter()
        .zip(kinds)
        .enumerate()
    {
        require_program_owned(release, account)?;
        let row = RuntimeCompartmentV1::decode(&account.data).map_err(|_| chain())?;
        let expected = pda(
            release.program_id,
            &[
                DIRECT_LIVENESS_ROW_SEED_V2,
                &binding.market_instance_id.bytes(),
                &binding.generation.to_le_bytes(),
                &[u8::try_from(index).map_err(|_| chain())?],
            ],
        );
        let floor = row
            .remaining_work_lamports
            .checked_add(row.rent_locked_lamports)
            .and_then(|value| value.checked_add(row.donation_remaining_lamports))
            .ok_or(CanonicalActionMaterialErrorV1::InvalidChainState)?;
        if account.address != expected.0
            || row.kind != kind
            || row.identity.account_id.bytes() != account.address.to_bytes()
            || row.identity.generation != binding.generation
            || manifest
                .state
                .compartment_account(index)
                .map(|id| id.bytes())
                != Some(account.address.to_bytes())
            || account.lamports < floor
        {
            return invalid();
        }
    }
    Ok(())
}

fn authenticate_absence(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    supplied: Address,
    derived: Address,
    absence: &FinalizedAccountAbsence,
) -> Result<()> {
    if supplied == Address::default()
        || supplied != derived
        || absence.release_key() != release.key()
        || absence.slot() != freshness.observed_slot
        || absence.receive_sequence() == 0
    {
        return invalid();
    }
    Ok(())
}

fn require_system_roles(snapshot: DirectAction1ProductRootSnapshotV2<'_>) -> Result<()> {
    let system = parse(SYSTEM_PROGRAM_TEXT)?;
    let rent = parse(RENT_SYSVAR_TEXT)?;
    let clock = parse(CLOCK_SYSVAR_TEXT)?;
    let sysvar_owner = parse(SYSVAR_OWNER_TEXT)?;
    if snapshot.payer.address == Address::default()
        || snapshot.payer.owner != system
        || snapshot.payer.executable
        || !snapshot.payer.data.is_empty()
        || snapshot.system_program.address != system
        || !snapshot.system_program.executable
        || snapshot.rent_sysvar.address != rent
        || snapshot.rent_sysvar.owner != sysvar_owner
        || snapshot.rent_sysvar.executable
        || snapshot.clock_sysvar.address != clock
        || snapshot.clock_sysvar.owner != sysvar_owner
        || snapshot.clock_sysvar.executable
        || snapshot.clock_sysvar.data.len() != 40
    {
        return invalid();
    }
    let slot = u64::from_le_bytes(
        snapshot.clock_sysvar.data[..8]
            .try_into()
            .map_err(|_| chain())?,
    );
    if slot != snapshot.clock_sysvar.provenance.slot {
        return invalid();
    }
    Ok(())
}

fn authenticate_artifact_address(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
    kind: ArtifactKind,
    id: [u8; 32],
) -> Result<()> {
    require_program_owned(release, account)?;
    let expected = pda(
        release.program_id,
        &[FAMILY_POLICY_ARTIFACT_SEED_V1, &[kind.byte()], &id],
    );
    if account.address != expected.0 || account.data.len() != kind.exact_len() {
        return invalid();
    }
    Ok(())
}

fn require_program_owned(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
) -> Result<()> {
    if account.address == Address::default()
        || account.owner != release.program_id
        || account.executable
        || account.lamports == 0
    {
        return invalid();
    }
    Ok(())
}

fn require_distinct_action1(addresses: [Address; DIRECT_ACTION1_ACCOUNT_COUNT_V2]) -> Result<()> {
    let mut unique = BTreeSet::new();
    for (index, address) in addresses.into_iter().enumerate() {
        if (address == Address::default() && index != 38) || !unique.insert(address) {
            return invalid();
        }
    }
    Ok(())
}

fn snapshot_root_address(
    binding: &clutch_product_series::MarketLifecycleBindingV3,
    program: Address,
) -> Address {
    pda(
        program,
        &[
            PRODUCT_ROOT_SEED_V3,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
    )
    .0
}

fn action1_roles() -> [DirectAccountRoleV1; DIRECT_ACTION1_ACCOUNT_COUNT_V2] {
    use DirectAccountRoleV1 as R;
    [
        R::GeneralMarketBinding,
        R::GeneralMarketRuntime,
        R::ProductRoot,
        R::WritableFounderSeriesLink,
        R::SeriesFunding,
        R::SeriesRegistry,
        R::RegistryProgram,
        R::RegistryProgramData,
        R::RegistryReleaseArtifact,
        R::CapabilityProfileArtifact,
        R::SourceRelease,
        R::CompilerBundle,
        R::MarketInstance,
        R::Realm,
        R::RevenuePolicyRecord,
        R::RevenuePolicy,
        R::SeriesPlan,
        R::FundingTerms,
        R::SourceTemplate,
        R::NativeClaimBasis,
        R::RecoveryPolicy,
        R::PriceMeasurePolicy,
        R::MarketGenesis,
        R::FundingQuote,
        R::AttachmentPlan,
        R::ProductReplay,
        R::FamilyCapabilityPolicy,
        R::ProductDirectGlobalLiveness,
        R::LivenessSource,
        R::LivenessCandidate,
        R::LivenessClearing,
        R::LivenessSettlement,
        R::LivenessResolution,
        R::LivenessRetirement,
        R::LivenessRecovery,
        R::DirectRoot,
        R::DirectReplay,
        R::ActorPayer,
        R::SystemProgram,
        R::RentSysvar,
        R::ClockSysvar,
    ]
}

fn snapshot_id(
    release: &IndexedProgramRelease,
    snapshot: DirectAction1ProductRootSnapshotV2<'_>,
    lookup: &StructuredAddressLookupTableV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(SNAPSHOT_DOMAIN_V2);
    hash.update(release.release_manifest_sha256);
    hash.update(release.capability_profile_id);
    for account in snapshot.present_accounts() {
        hash.update(account.address.as_ref());
        hash.update(account.owner.as_ref());
        hash.update(account.lamports.to_le_bytes());
        hash.update([u8::from(account.executable)]);
        hash.update(account.provenance.slot.to_le_bytes());
        hash.update(sha256(&account.data));
    }
    for (address, absence) in [
        (
            snapshot.fresh_direct_root_v3,
            snapshot.fresh_direct_root_absence,
        ),
        (
            snapshot.fresh_direct_replay_v1,
            snapshot.fresh_direct_replay_absence,
        ),
    ] {
        hash.update(address.as_ref());
        hash.update(
            u64::try_from(absence.release_key().len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(absence.release_key().as_bytes());
        hash.update(absence.slot().to_le_bytes());
        hash.update(absence.receive_sequence().to_le_bytes());
    }
    hash.update(lookup.account().as_ref());
    hash.update(lookup.state_sha256());
    hash.finalize().into()
}

fn pda(program: Address, seeds: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(seeds, &program)
}

fn parse(value: &str) -> Result<Address> {
    Address::from_str(value).map_err(|_| chain())
}

fn runtime_key(address: Address) -> RuntimeKey {
    RuntimeKey::from_bytes(address.to_bytes())
}

fn account_view(account: &ObservedRpcAccount) -> RuntimeAccountViewV1<'_> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.address),
        owner: runtime_key(account.owner),
        lamports: account.lamports,
        executable: account.executable,
        signer: false,
        writable: false,
        data: &account.data,
    }
}

fn chain() -> CanonicalActionMaterialErrorV1 {
    CanonicalActionMaterialErrorV1::InvalidChainState
}

fn invalid<T>() -> Result<T> {
    Err(chain())
}
