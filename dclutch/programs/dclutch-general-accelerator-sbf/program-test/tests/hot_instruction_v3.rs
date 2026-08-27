//! First real-state exercise of `build_general_hot_instruction_v3`.
//!
//! ADR 0006 section 8 item 4: the twelve `general_hot_v3` operator tests all
//! synthesize `GeneralHotInstructionV3` values directly and never enter the
//! builder, so the constructor a frontend actually calls had no caller and no
//! evidence. This file gives it one.
//!
//! Everything here is chain shape rather than a fixture table. The frame comes
//! from the same three semantic owners the builder consults: the family-neutral
//! `HOT_*_ACCOUNT_V3` coordinates for the fixed prefix, the admitted-AOT record
//! and deployment frame for the strategy suffix, and General's own generated
//! `AccountProfileV2` -- decoded back out of the artifact bytes this test
//! publishes -- for every runtime privilege, alias and data width. No account
//! count below is transcribed; each is read out of the artifact.

use std::borrow::Cow;

use dclutch_account_profile_contract::v2::encode::AccountAliasInputV2;
use dclutch_account_profile_contract::v2::{
    AccountProfileV2, PhysicalAccountDataGeometryV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID,
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
    HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
    HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
    HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3,
    HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
    HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
    HOT_TRANSITION_RAW_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2;
use dclutch_capability_program_contract::v4::{
    CapabilityProgramV4, CapabilityRootAccountV4,
    SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
    SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, initialize_root_account_v4,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID;
use dclutch_execution_strategy_contract::v2::{
    BankTransportV2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    classify_bank_transport_v2,
};
use dclutch_general_accelerator_program_test::joined_artifacts::{
    JoinedGeneralArtifactInputV5, JoinedGeneralArtifactsV5, build_joined_general_artifacts_v5,
};
use dclutch_general_adapter_contract::account_rules_v3::{
    GeneralExternalAccountWidthsV3, general_account_profile_fixed_count_v3,
    general_account_profile_rule_v3,
};
use dclutch_general_adapter_contract::hot_candidate_v3::{
    GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_scalar_count_v3,
};
use dclutch_general_adapter_contract::local_state_v3::{
    GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
    general_local_state_len_v3,
};
use dclutch_general_adapter_contract::release_v3::GENERAL_ACTIONS_V3;
use dclutch_general_adapter_contract::runtime_width::{
    SettlementCursorHeaderV2, SettlementCursorV2, SettlementPhaseV2, settlement_cursor_len,
};
use dclutch_general_adapter_contract::state_artifacts_v3::general_child_account_start_v3;
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::root::{GeneralRootV2, general_root_creation_tail_v2};
use dclutch_general_config_contract::v3::GENERAL_CONFIG_SCHEMA_ID_V3;
use dclutch_general_config_contract::{GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_operator::general_hot_v3::{
    CheckedGeneralHotReleaseV3, GeneralHotArtifactDigestsV3, GeneralHotInstructionV3,
    GeneralHotOperatorErrorV3, GeneralHotStateV3, GeneralObservedAccountMetaV3,
    build_general_hot_instruction_v3, canonical_general_lookup_addresses_v3,
    compile_general_hot_v0,
};
use dclutch_operator::versioned::PACKET_DATA_BYTES;
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID;
use dclutch_transition_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_PROGRAM_SCHEMA_ID;
use solana_address_lookup_table_interface::{
    program as lookup_table_program,
    state::{AddressLookupTable, LookupTableMeta},
};
use solana_hash::Hash;
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

/// Registry program which owns every finalized raw record in this snapshot.
const REGISTRY_PROGRAM: Pubkey = Pubkey::new_from_array([0xc1; 32]);
/// Core program which owns the Market account.
const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0xc2; 32]);
/// Trading program: the instruction target and the state-PDA deriving program.
const TRADING_PROGRAM: Pubkey = Pubkey::new_from_array([0xc3; 32]);
/// Admitted General accelerator deployment.
const ACCELERATOR_PROGRAM: Pubkey = Pubkey::new_from_array([0xc4; 32]);

/// Immutable Market generation carried by the root header and the envelope.
const GENERATION: u64 = 9;
/// Product width; every register, page and portfolio row follows from it.
const OUTCOME_COUNT: u32 = 4;
/// Finalized slot shared by every account in the snapshot.
const FINALIZED_SLOT: u64 = 9_001;
/// Market-selected immutable execution release set.
const RELEASE_SET: [u8; 32] = [0x31; 32];
/// Checked immutable Trading ArtifactRelease identity.
const TRADING_ARTIFACT_RELEASE: [u8; 32] = [0x32; 32];
/// Digest of the user-supplied checked multiprogram manifest.
const CHECKED_MANIFEST_DIGEST: [u8; 32] = [0x33; 32];
/// Candidate identity carried by every action whose request names one.
const CANDIDATE_ID: [u8; 32] = [0x81; 32];

/// Deliberately wrong bump witnesses handed to the builder.
///
/// `build_general_hot_instruction_v3` documents that the two request bump
/// fields are untrusted placeholders which it replaces with bumps derived from
/// the authenticated lifecycle policy. A canonical bump is found by walking
/// down from 255, so this value is not one the derivation can return here; the
/// accepted test asserts that separately rather than assuming it.
const PLACEHOLDER_BUMP: u8 = 0xEE;

/// Release-selected external widths, as the release builder would supply them.
///
/// These are the same values the joined-artifact fixture's own tests use. Each
/// one is copied into an `AccountProfileV2` rule, so the accounts this test
/// builds take their widths from the artifact rather than from here.
const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
    linked_basis_prefix: 256,
    result_domain: 192,
    rent_sysvar: 17,
    core_market: 320,
    activation_cache: 160,
    upgradeable_program: 36,
    trading_programdata_prefix: 45,
    claims_programdata_prefix: 45,
    core_programdata_prefix: 45,
    realm_record: 112,
    rent_credit: 48,
};

/// One finalized Registry raw record and its vacant staging cursor.
struct RecordPairV3 {
    /// Finalized content-addressed raw record owned by the Registry.
    raw: ObservedAccount,
    /// Vacant System-owned staging cursor for the same content.
    staging: ObservedAccount,
}

/// One complete finalized General snapshot plus the facts it was built from.
struct GeneralChainFixtureV3 {
    /// Exact operator input.
    state: GeneralHotStateV3,
    /// Generated seven-action artifact graph.
    artifacts: JoinedGeneralArtifactsV5,
    /// Selected action.
    action: Action,
    /// Core Market account.
    market: Pubkey,
    /// Composite Trading capability root.
    root: Pubkey,
    /// Exact composite root bytes, whose digest the envelope commits to.
    root_data: Vec<u8>,
    /// SHA-256 identity of the finalized Product root record.
    product_record: [u8; 32],
    /// Canonical primary action-state address.
    primary_state: Pubkey,
    /// Canonical primary action-state bump.
    primary_state_bump: u8,
    /// Canonical close-only terminal action-state address.
    terminal_state: Option<Pubkey>,
    /// Canonical close-only terminal action-state bump.
    terminal_state_bump: Option<u8>,
    /// Exact instruction account width implied by the artifact geometry.
    instruction_accounts: usize,
}

/// General's own state-seed domain, from `state_artifacts_v3.rs`.
///
/// These four literals are module-private constants in the adapter contract,
/// so the test cannot borrow them and restates them instead. That restatement
/// is self-checking: the builder refuses with `Lifecycle` unless the account
/// this file installs is exactly the address these seeds derive under the
/// Trading program, so a drifted literal fails the accepted test rather than
/// passing a weakened one.
const GENERAL_STATE_SEED_DOMAIN_V3: &[u8] = b"dclutch-general-state-v3";
/// Selection-phase state discriminator.
const SELECTION_STATE_SEED_V3: &[u8] = b"selection";
/// Settlement-phase state discriminator.
const SETTLEMENT_STATE_SEED_V3: &[u8] = b"settlement";
/// Close-only terminal-record discriminator.
const TERMINAL_STATE_SEED_V3: &[u8] = b"terminal";

/// The one finalized observation every account in the snapshot shares.
fn observation() -> Observation {
    Observation {
        slot: FINALIZED_SLOT,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

/// Product-runtime content identity, refusing the zero identity.
fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

/// Core content identity, refusing the zero identity.
fn core_content(value: [u8; 32]) -> CoreContentId {
    CoreContentId::new(value).expect("nonzero content identity")
}

/// Market identity coordinate, refusing zero.
fn identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("nonzero Market identity")
}

/// SHA-256 content digest, the identity every record coordinate is keyed by.
fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

/// One rent-exempt finalized account observation.
fn observed(key: Pubkey, owner: Pubkey, executable: bool, data: Vec<u8>) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner,
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        executable,
        data,
    }
}

/// One loader-owned executable deployment.
fn program(key: Pubkey) -> ObservedAccount {
    observed(
        key,
        bpf_loader_upgradeable::ID,
        true,
        vec![0; usize::try_from(WIDTHS.upgradeable_program).expect("program width")],
    )
}

/// One finalized raw record and vacant staging cursor at their exact Registry PDAs.
fn record_pair(schema: [u8; 32], data: Vec<u8>) -> RecordPairV3 {
    let content_digest = digest(&data);
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &content_digest],
        &REGISTRY_PROGRAM,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &content_digest],
        &REGISTRY_PROGRAM,
    )
    .0;
    RecordPairV3 {
        raw: observed(raw, REGISTRY_PROGRAM, false, data),
        staging: observed(staging, system_program::ID, false, Vec::new()),
    }
}

/// Read-only frame member: every fixed account but the root is read-only.
fn readonly(account: ObservedAccount) -> GeneralObservedAccountMetaV3 {
    GeneralObservedAccountMetaV3 {
        account,
        is_signer: false,
        is_writable: false,
    }
}

/// The exact authenticated scratch-page count for this Product width.
///
/// The bank transport is the sole authority on the page span; the operator
/// derives the same number from the effect program's register geometry.
fn scratch_pages(outcome_count: u32) -> u32 {
    let scalars = general_hot_scalar_count_v3(outcome_count).expect("General scalar count");
    match classify_bank_transport_v2(scalars, GENERAL_HOT_COMMON_IDENTITIES_V3)
        .expect("General bank transport")
    {
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => page_count,
        BankTransportV2::InlineReturnData { .. } => 1,
    }
}

/// Canonical request for one action, with both bump witnesses poisoned.
fn placeholder_request(action: Action) -> ControllerRequestV2 {
    ControllerRequestV2 {
        action,
        expected_revision: 0,
        candidate_id: (action != Action::Freeze).then_some(CANDIDATE_ID),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: PLACEHOLDER_BUMP,
        terminal_record_bump: PLACEHOLDER_BUMP,
    }
}

/// Whether the action's primary state is a selection cursor rather than settlement.
fn is_selection(action: Action) -> bool {
    matches!(action, Action::Consider | Action::Freeze)
}

/// Exact primary-state seed program for one action.
fn primary_state_seeds(action: Action, root: Pubkey) -> Vec<Vec<u8>> {
    if is_selection(action) {
        vec![
            GENERAL_STATE_SEED_DOMAIN_V3.to_vec(),
            root.to_bytes().to_vec(),
            SELECTION_STATE_SEED_V3.to_vec(),
        ]
    } else {
        vec![
            GENERAL_STATE_SEED_DOMAIN_V3.to_vec(),
            root.to_bytes().to_vec(),
            CANDIDATE_ID.to_vec(),
            SETTLEMENT_STATE_SEED_V3.to_vec(),
        ]
    }
}

/// Exact close-only terminal-record seed program.
fn terminal_state_seeds(root: Pubkey, terminal_coordinate: u64) -> Vec<Vec<u8>> {
    vec![
        GENERAL_STATE_SEED_DOMAIN_V3.to_vec(),
        root.to_bytes().to_vec(),
        CANDIDATE_ID.to_vec(),
        terminal_coordinate.to_le_bytes().to_vec(),
        TERMINAL_STATE_SEED_V3.to_vec(),
    ]
}

/// Derive one canonical Trading-owned state address from its seed program.
fn state_address(seeds: &[Vec<u8>]) -> (Pubkey, u8) {
    let refs = seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
    Pubkey::find_program_address(&refs, &TRADING_PROGRAM)
}

/// One live General local state at the exact width the profile declares.
fn live_settlement_state(bump: u8) -> Vec<u8> {
    let mut body = vec![0; settlement_cursor_len(OUTCOME_COUNT).expect("cursor width")];
    SettlementCursorV2::encode_into(
        SettlementCursorHeaderV2 {
            outcome_count: OUTCOME_COUNT,
            // `ReadyToClose` is the one phase Close may consume: every order is
            // drained (`next_order == order_count`) and no terminal coordinate
            // has been written yet.
            order_count: 1,
            next_order: 1,
            revision: 1,
            candidate_id: CANDIDATE_ID,
            quote_inventory: 0,
            complete_set_quantity: 0,
            terminal_coordinate: 0,
            phase: SettlementPhaseV2::ReadyToClose,
        },
        &vec![0; usize::try_from(OUTCOME_COUNT).expect("Product width")],
        &mut body,
    )
    .expect("canonical settlement cursor");
    let len = general_local_state_len_v3(GeneralLocalStateKindV3::Settlement, OUTCOME_COUNT)
        .expect("local state width");
    let mut scratch = vec![0; len];
    let mut output = vec![0; len];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind: GeneralLocalStateKindV3::Settlement,
            bump,
            rent_principal: Rent::default().minimum_balance(len),
            beneficiary: [0x26; 32],
        },
        &body,
        &mut scratch,
        &mut output,
    )
    .expect("canonical General local state");
    output
}

/// Build one complete finalized General snapshot for the selected action.
///
/// This stays one contiguous function because every stage consumes an identity
/// the previous stage derived: the Market is keyed by the Product digest, the
/// root header by the Market and the config digest, the state PDA by the root.
#[allow(clippy::too_many_lines)]
fn build_fixture(action: Action) -> GeneralChainFixtureV3 {
    // The Product graph first: every downstream identity -- Market, root
    // header, request -- is keyed by content that only exists once the graph
    // does.
    //
    // The linked-basis body is opaque filler. Nothing in this construction
    // path decodes a liability basis, and the profile rule asserts only a
    // nonzero prefix width; what is real is the LINK, since the record sits at
    // the Registry coordinate for the exact `liability_basis_id` the Product
    // result domain names.
    let basis_bytes =
        vec![0x6b; usize::try_from(WIDTHS.linked_basis_prefix).expect("basis prefix width")];
    let liability_basis = content(digest(&basis_bytes));
    let product_id = content([0x33; 32]);
    let representation_release = content([0x35; 32]);
    let cuts: Vec<i128> = (0..usize::try_from(OUTCOME_COUNT).expect("Product width") - 2)
        .map(|index| i128::try_from(index).expect("cut index"))
        .collect();
    let mut domain = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id,
            coordinate_domain_id: content([0x36; 32]),
            result_unit_id: content([0x37; 32]),
            liability_basis_id: liability_basis,
            representation_release_id: representation_release,
            mapping_release_id: content([0x38; 32]),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("canonical Product result domain");
    let coefficients = vec![1_u64; usize::try_from(OUTCOME_COUNT).expect("Product width")];
    let mut portfolio = vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id,
            result_domain_id: content(digest(&domain)),
            claim_basis_id: content([0x39; 32]),
            liability_basis_id: liability_basis,
            representation_release_id: representation_release,
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("canonical Product portfolio");
    let mut product = vec![0; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        product_id,
        content(digest(&domain)),
        content(digest(&portfolio)),
    )
    .encode_into(&mut product)
    .expect("canonical Product root");
    let product_record = digest(&product);

    // The artifact graph. Its `outcome_count` must equal the width the Product
    // graph above authenticates, because the builder re-authenticates the
    // bundle against the Product-derived width and nothing else.
    let artifacts = build_joined_general_artifacts_v5(JoinedGeneralArtifactInputV5 {
        capacity_profile: [0x41; 32],
        accelerator_artifact_release: digest(b"dclutch/general/accelerator-release/fixture"),
        outcome_count: OUTCOME_COUNT,
        external_widths: WIDTHS,
        token_account_bytes: 165,
        price_scale: 1_000,
        selection_policy: [0x43; 32],
        quote_surplus_beneficiary: [0x44; 32],
    })
    .expect("complete seven-action General artifact graph");
    let selected = artifacts.action(action).expect("selected action bundle");
    let config_id = digest(&artifacts.config);
    let program_set_id = digest(&artifacts.program_set);

    // The CapabilityManifest body is opaque filler for the same reason as the
    // basis: no validator on this path decodes it. Its identity is real, in
    // that the manifest coordinate holds exactly the bytes the root header's
    // execution selection names.
    let manifest_bytes = vec![0x5a; 96];
    let manifest_id = digest(&manifest_bytes);

    // Market. The identity is closed under its own PDA the way Core closes it:
    // the seeds are taken over a provisional identity, and the resulting
    // address is written back as `market_id`.
    let provisional = MarketIdentity {
        market_id: identity([0x21; 32]),
        realm_id: identity([0x22; 32]),
        product_record: identity(product_record),
        product_id: identity(product_id.to_bytes()),
        resolution_policy: identity([0x25; 32]),
        capability_manifest: identity(manifest_id),
        selected_release_set: identity(RELEASE_SET),
        registry_program: identity(REGISTRY_PROGRAM.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &CORE_PROGRAM,
    )
    .0;
    let market_data = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity(market.to_bytes()),
            ..provisional
        },
        outstanding_capabilities: 1,
        rent_beneficiary: identity([0x26; 32]),
        terminal_receipt: None,
    }
    .encode()
    .expect("canonical Open Market state");

    // The composite root: a real immutable `CapabilityRootHeaderV1` followed by
    // a real `GeneralRootV2` tail, assembled by the contract's own initializer
    // against the action-selected V4 descriptor.
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        core_content(manifest_id),
        core_content(GENERAL_CAPABILITY_KIND_ID_V1),
        core_content(program_set_id),
        core_content(config_id),
    )
    .expect("capability execution selection");
    let root_header = CapabilityRootHeaderV1::new(
        core_content(RELEASE_SET),
        market.to_bytes(),
        GENERATION,
        selection,
    )
    .expect("capability root header");
    let root = Pubkey::find_program_address(&root_header.seeds().as_slices(), &TRADING_PROGRAM).0;
    let descriptor =
        CapabilityProgramV4::decode(&selected.descriptor).expect("action-selected descriptor");
    let mut root_data = vec![
        0;
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(GENERAL_ROOT_BYTES_V2)
            .expect("root account width")
    ];
    initialize_root_account_v4(
        &mut root_data,
        root_header,
        descriptor,
        &general_root_creation_tail_v2(market.to_bytes(), config_id, GENERATION)
            .expect("General creation tail"),
    )
    .expect("composite General root");

    // Records. Every artifact this snapshot publishes sits at the exact
    // Registry raw/staging coordinate for its own bytes under its own schema,
    // so the record layer cannot silently disagree with the artifact layer.
    let manifest = record_pair(
        digest(b"dclutch/schema/capability-manifest/fixture"),
        manifest_bytes,
    );
    let program_set = record_pair(
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        artifacts.program_set.clone(),
    );
    let descriptor_record = record_pair(
        CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        selected.descriptor.clone(),
    );
    let config = record_pair(GENERAL_CONFIG_SCHEMA_ID_V3, artifacts.config.clone());
    let account_profile = record_pair(ACCOUNT_PROFILE_SCHEMA_ID, selected.account_profile.clone());
    let request_profile = record_pair(REQUEST_PROFILE_SCHEMA_ID, selected.request_profile.clone());
    let transition = record_pair(TRANSITION_PROGRAM_SCHEMA_ID, selected.transition.clone());
    let effect = record_pair(EFFECT_PROGRAM_SCHEMA_ID, selected.effect.clone());
    let lifecycle = record_pair(
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
        selected.lifecycle_policy.clone(),
    );
    let strategy = record_pair(
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        selected.strategy.clone(),
    );
    let certificate = record_pair(
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        selected.certificate.clone(),
    );
    let admission = record_pair(
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
        selected.admission.clone(),
    );
    let artifact_release = record_pair(
        digest(b"dclutch/schema/artifact-release/fixture"),
        b"dclutch/general/accelerator-release/fixture".to_vec(),
    );
    let product_pair = record_pair(PRODUCT_RECORD_SCHEMA_ID_V2, product);
    let domain_pair = record_pair(RESULT_DOMAIN_SCHEMA_ID_V2, domain);
    let portfolio_pair = record_pair(PORTFOLIO_SCHEMA_ID_V2, portfolio);
    let basis_pair = record_pair(
        digest(b"dclutch/schema/liability-basis/fixture"),
        basis_bytes,
    );

    // The fixed frame, in canonical ABI order. Pushing under an asserted length
    // keeps the coordinate constant next to the account it names, which a bare
    // index cannot do.
    let mut fixed: Vec<GeneralObservedAccountMetaV3> =
        Vec::with_capacity(HOT_FIXED_ACCOUNT_COUNT_V3);
    let push = |accounts: &mut Vec<GeneralObservedAccountMetaV3>,
                coordinate: usize,
                meta: GeneralObservedAccountMetaV3| {
        assert_eq!(accounts.len(), coordinate, "canonical Hot frame order");
        accounts.push(meta);
    };
    push(
        &mut fixed,
        HOT_MARKET_ACCOUNT_V3,
        readonly(observed(market, CORE_PROGRAM, false, market_data.to_vec())),
    );
    // The sole writable account in the fixed frame, per `validate_fixed_frame`.
    push(
        &mut fixed,
        HOT_ROOT_ACCOUNT_V3,
        GeneralObservedAccountMetaV3 {
            account: observed(root, TRADING_PROGRAM, false, root_data.clone()),
            is_signer: false,
            is_writable: true,
        },
    );
    for (coordinate, pair) in [
        (HOT_MANIFEST_RAW_ACCOUNT_V3, manifest),
        (HOT_PROGRAM_SET_RAW_ACCOUNT_V3, program_set),
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, descriptor_record),
        (HOT_CONFIG_RAW_ACCOUNT_V3, config),
        (HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, account_profile),
        (HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, request_profile),
        (HOT_TRANSITION_RAW_ACCOUNT_V3, transition),
        (HOT_EFFECT_RAW_ACCOUNT_V3, effect),
        (HOT_LIFECYCLE_RAW_ACCOUNT_V3, lifecycle),
        (HOT_STRATEGY_RAW_ACCOUNT_V3, strategy),
    ] {
        push(&mut fixed, coordinate, readonly(pair.raw));
        push(&mut fixed, coordinate + 1, readonly(pair.staging));
    }
    push(
        &mut fixed,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        readonly(observed(
            Pubkey::new_from_array([0x71; 32]),
            REGISTRY_PROGRAM,
            false,
            vec![0; usize::try_from(WIDTHS.activation_cache).expect("cache width")],
        )),
    );
    push(
        &mut fixed,
        HOT_CORE_PROGRAM_ACCOUNT_V3,
        readonly(program(CORE_PROGRAM)),
    );
    push(
        &mut fixed,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        readonly(observed(
            Pubkey::new_from_array([0x72; 32]),
            bpf_loader_upgradeable::ID,
            false,
            vec![0; usize::try_from(WIDTHS.core_programdata_prefix).expect("programdata")],
        )),
    );
    push(
        &mut fixed,
        HOT_TRADING_PROGRAM_ACCOUNT_V3,
        readonly(program(TRADING_PROGRAM)),
    );
    push(
        &mut fixed,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        readonly(observed(
            Pubkey::new_from_array([0x73; 32]),
            bpf_loader_upgradeable::ID,
            false,
            vec![0; usize::try_from(WIDTHS.trading_programdata_prefix).expect("programdata")],
        )),
    );
    push(
        &mut fixed,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        readonly(program(REGISTRY_PROGRAM)),
    );
    push(
        &mut fixed,
        HOT_RENT_SYSVAR_ACCOUNT_V3,
        readonly(observed(
            sysvar::rent::ID,
            sysvar::ID,
            false,
            vec![0; usize::try_from(WIDTHS.rent_sysvar).expect("Rent width")],
        )),
    );
    push(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        readonly(observed(
            sysvar::instructions::ID,
            sysvar::ID,
            false,
            Vec::new(),
        )),
    );
    for (coordinate, pair) in [
        (HOT_PRODUCT_RAW_ACCOUNT_V3, product_pair),
        (HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, domain_pair),
        (HOT_PORTFOLIO_RAW_ACCOUNT_V3, portfolio_pair),
        (HOT_LINKED_BASIS_RAW_ACCOUNT_V3, basis_pair),
    ] {
        push(&mut fixed, coordinate, readonly(pair.raw));
        push(&mut fixed, coordinate + 1, readonly(pair.staging));
    }
    push(
        &mut fixed,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3,
        readonly(observed(
            Pubkey::new_from_array([0x74; 32]),
            TRADING_PROGRAM,
            false,
            vec![0; 96],
        )),
    );
    assert_eq!(fixed.len(), HOT_FIXED_ACCOUNT_COUNT_V3);

    // The admitted-AOT transport suffix: the six certificate/admission/release
    // record coordinates, the accelerator deployment, its ProgramData, then one
    // release-pinned caller authority per acknowledgment chunk.
    let pages = scratch_pages(OUTCOME_COUNT);
    let mut strategy_accounts = vec![
        readonly(certificate.raw),
        readonly(certificate.staging),
        readonly(admission.raw),
        readonly(admission.staging),
        readonly(artifact_release.raw),
        readonly(artifact_release.staging),
        readonly(program(ACCELERATOR_PROGRAM)),
        readonly(observed(
            Pubkey::new_from_array([0x75; 32]),
            bpf_loader_upgradeable::ID,
            false,
            vec![0; usize::try_from(WIDTHS.trading_programdata_prefix).expect("programdata")],
        )),
    ];
    for page in 0..pages {
        let mut key = [0_u8; 32];
        key.get_mut(0..4)
            .expect("caller authority key prefix")
            .copy_from_slice(&page.to_le_bytes());
        *key.get_mut(31).expect("caller authority tag") = 0x91;
        strategy_accounts.push(readonly(observed(
            Pubkey::new_from_array(key),
            TRADING_PROGRAM,
            false,
            Vec::new(),
        )));
    }

    // The runtime suffix is generated entirely from the emitted profile: one
    // account per physical ordinal past the five injected logical coordinates,
    // with the privileges and data width the artifact declares.
    let profile =
        AccountProfileV2::decode(&selected.account_profile).expect("emitted account profile");
    let span_counts = [pages];
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(OUTCOME_COUNT, &span_counts)
        .expect("physical account count");
    let request = placeholder_request(action);
    let (primary_state, primary_state_bump) = state_address(&primary_state_seeds(action, root));
    let (terminal_state, terminal_state_bump) = if action == Action::Close {
        let terminal_coordinate = request
            .expected_revision
            .checked_add(1)
            .expect("terminal coordinate");
        let (key, bump) = state_address(&terminal_state_seeds(root, terminal_coordinate));
        (Some(key), Some(bump))
    } else {
        (None, None)
    };
    let mut runtime_suffix_accounts = Vec::new();
    for ordinal in HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3..physical_count {
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(OUTCOME_COUNT, &span_counts, ordinal)
            .expect("physical account geometry");
        let privileges = geometry.privileges();
        let data_len = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes }
            | PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable {
                minimum_bytes: bytes,
            } => bytes,
            // A lifecycle-bound coordinate is admitted vacant, which is the
            // honest prestate for an action that has not run yet.
            PhysicalAccountDataGeometryV2::VacantOrExact { .. }
            | PhysicalAccountDataGeometryV2::Opaque => 0,
        };
        let representative = geometry.logical_representative();
        let (key, owner, data) = if Some(representative) == primary_state_index(action, true) {
            if action == Action::Close {
                (
                    primary_state,
                    TRADING_PROGRAM,
                    live_settlement_state(primary_state_bump),
                )
            } else {
                (primary_state, system_program::ID, Vec::new())
            }
        } else if Some(representative) == primary_state_index(action, false) {
            (
                terminal_state.expect("close terminal state"),
                system_program::ID,
                Vec::new(),
            )
        } else {
            let mut key = [0_u8; 32];
            key.get_mut(0..8)
                .expect("runtime key prefix")
                .copy_from_slice(&u64::try_from(ordinal).expect("ordinal").to_le_bytes());
            *key.get_mut(31).expect("runtime key tag") = 0xa0;
            (
                Pubkey::new_from_array(key),
                if privileges.executable() {
                    bpf_loader_upgradeable::ID
                } else {
                    system_program::ID
                },
                vec![0; data_len],
            )
        };
        runtime_suffix_accounts.push(GeneralObservedAccountMetaV3 {
            account: observed(key, owner, privileges.executable(), data),
            is_signer: privileges.signer(),
            is_writable: privileges.writable(),
        });
    }

    let instruction_accounts = HOT_FIXED_ACCOUNT_COUNT_V3
        .checked_add(strategy_accounts.len())
        .and_then(|value| value.checked_add(runtime_suffix_accounts.len()))
        .expect("instruction account width");
    let checked = CheckedGeneralHotReleaseV3 {
        trading_program: TRADING_PROGRAM,
        trading_artifact_release: TRADING_ARTIFACT_RELEASE,
        general_artifact_release: artifacts.accelerator_artifact_release,
        checked_manifest_digest: CHECKED_MANIFEST_DIGEST,
    };
    GeneralChainFixtureV3 {
        state: GeneralHotStateV3 {
            fixed_accounts: fixed,
            strategy_accounts,
            runtime_suffix_accounts,
            release_set: RELEASE_SET,
            generation: GENERATION,
            minimum_finalized_slot: FINALIZED_SLOT,
            checked_release: Some(checked),
        },
        artifacts,
        action,
        market,
        root,
        root_data,
        product_record,
        primary_state,
        primary_state_bump,
        terminal_state,
        terminal_state_bump,
        instruction_accounts,
    }
}

/// Logical coordinate of the primary (or close-only terminal) state account.
///
/// These two coordinates are the only ones whose key the operator derives
/// rather than accepts, so the generated runtime frame has to know where they
/// are. Both come from General's own `state_artifacts_v3` coordinates.
fn primary_state_index(action: Action, primary: bool) -> Option<usize> {
    use dclutch_general_adapter_contract::state_artifacts_v3::{
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
    };
    if primary {
        Some(usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3))
    } else if action == Action::Close {
        Some(usize::from(GENERAL_TERMINAL_STATE_ACCOUNT_V3))
    } else {
        None
    }
}

/// Exact content identities the builder should report for one action bundle.
fn expected_digests(fixture: &GeneralChainFixtureV3) -> GeneralHotArtifactDigestsV3 {
    let selected = fixture
        .artifacts
        .action(fixture.action)
        .expect("selected action bundle");
    GeneralHotArtifactDigestsV3 {
        program_set: digest(&fixture.artifacts.program_set),
        descriptor: digest(&selected.descriptor),
        config: digest(&fixture.artifacts.config),
        account_profile: digest(&selected.account_profile),
        lifecycle_policy: digest(&selected.lifecycle_policy),
        request_profile: digest(&selected.request_profile),
        strategy: digest(&selected.strategy),
        certificate: digest(&selected.certificate),
        admission: digest(&selected.admission),
        transition: digest(&selected.transition),
        effect: digest(&selected.effect),
    }
}

/// Exact instruction width, derived from the rule generator rather than the
/// decoded artifact.
///
/// This is the independent half of the account-count claim. `build_fixture`
/// sizes its runtime frame from the emitted `AccountProfileV2`; this walks the
/// generator that authored those bytes, skips every aliased coordinate -- an
/// alias is a second logical name for a physical account the frame already
/// carries, so it costs no transaction account -- and adds the fixed prefix,
/// the admitted-AOT transport frame and the scratch-page span. Two independent
/// authorities, one number.
fn generated_instruction_accounts(action: Action) -> usize {
    let logical =
        general_account_profile_fixed_count_v3(action).expect("General logical account count");
    let mut physical_runtime = 0_usize;
    for coordinate in 0..logical {
        let rule =
            general_account_profile_rule_v3(action, coordinate, WIDTHS).expect("General rule");
        if matches!(rule.rule.alias, AccountAliasInputV2::Fixed(_)) {
            continue;
        }
        physical_runtime += 1;
    }
    let pages = usize::try_from(scratch_pages(OUTCOME_COUNT)).expect("bounded page count");
    physical_runtime += pages;
    HOT_FIXED_ACCOUNT_COUNT_V3 + ADMITTED_AOT_FIXED_EXTRAS_V3 + pages + physical_runtime
        - HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
}

/// Admitted-AOT transport accounts carried between Hot38 and the runtime frame.
///
/// Trading's own `ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2` minus its
/// `INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2`: the certificate, admission and
/// ArtifactRelease record pairs plus the accelerator deployment and its
/// ProgramData. Those two constants live in the Trading program crate, which
/// this detached workspace does not depend on, so the count is restated here
/// and the builder's own `StrategyGeometry` refusal is what pins it.
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = 8;

/// Build, then mutate one field of the accepted snapshot and re-run the builder.
fn refuse_after(
    action: Action,
    mutate: impl FnOnce(&mut GeneralHotStateV3),
) -> GeneralHotOperatorErrorV3 {
    let mut fixture = build_fixture(action);
    mutate(&mut fixture.state);
    let (selection, bytes) = fixture
        .artifacts
        .selected(action)
        .expect("selected artifacts");
    build_general_hot_instruction_v3(
        &fixture.state,
        selection,
        bytes,
        placeholder_request(action),
    )
    .expect_err("the mutated snapshot must refuse")
}

/// Borrow one fixed-frame coordinate for mutation.
fn fixed_at(state: &mut GeneralHotStateV3, coordinate: usize) -> &mut GeneralObservedAccountMetaV3 {
    state
        .fixed_accounts
        .get_mut(coordinate)
        .expect("fixed frame coordinate")
}

#[test]
fn the_builder_turns_one_finalized_general_snapshot_into_a_complete_hot_instruction() {
    let action = Action::Freeze;
    let fixture = build_fixture(action);
    let (selection, bytes) = fixture
        .artifacts
        .selected(action)
        .expect("selected artifacts");
    let report = build_general_hot_instruction_v3(
        &fixture.state,
        selection,
        bytes,
        placeholder_request(action),
    )
    .expect("real chain state builds one General Hot instruction");

    assert_eq!(report.instruction.program_id, TRADING_PROGRAM);
    assert_eq!(
        report.instruction.accounts.len(),
        fixture.instruction_accounts
    );
    assert_eq!(
        report.instruction.accounts.len(),
        generated_instruction_accounts(action)
    );
    assert_eq!(report.action, action);
    assert_eq!(report.outcome_count, OUTCOME_COUNT);
    assert_eq!(report.product_record, fixture.product_record);
    assert_eq!(report.artifacts, expected_digests(&fixture));
    assert_eq!(report.checked_manifest_digest, CHECKED_MANIFEST_DIGEST);
    assert_eq!(report.trading_artifact_release, TRADING_ARTIFACT_RELEASE);
    assert_eq!(
        report.general_artifact_release,
        fixture.artifacts.accelerator_artifact_release
    );
    assert_eq!(report.observation, observation());

    // The account whose digest the envelope commits to is a real composite
    // root, not a same-width filler: the immutable header decodes and the
    // family tail is exactly what General's own creation projection emits.
    let root_account = fixture
        .state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .expect("root coordinate");
    assert_eq!(root_account.account.key, fixture.root);
    let descriptor = CapabilityProgramV4::decode(
        &fixture
            .artifacts
            .action(action)
            .expect("selected action bundle")
            .descriptor,
    )
    .expect("action-selected descriptor");
    let composite = CapabilityRootAccountV4::decode(&fixture.root_data, descriptor)
        .expect("composite General root decodes");
    assert_eq!(composite.header().market(), fixture.market.to_bytes());
    assert_eq!(composite.header().generation(), GENERATION);
    assert_eq!(
        GeneralRootV2::decode(composite.state()).expect("General root tail"),
        GeneralRootV2::active(
            fixture.market.to_bytes(),
            digest(&fixture.artifacts.config),
            GENERATION
        )
        .expect("active General root")
    );

    let (envelope, request_bytes) =
        HotExecutionEnvelopeV3::split_instruction(&report.instruction.data)
            .expect("canonical hot envelope");
    assert_eq!(envelope.market(), fixture.market.to_bytes());
    assert_eq!(envelope.generation(), GENERATION);
    assert_eq!(envelope.release_set(), RELEASE_SET);
    assert_eq!(envelope.root_prestate_digest(), digest(&fixture.root_data));
    assert_eq!(request_bytes.len(), CONTROLLER_REQUEST_BYTES_V2);
    assert_eq!(report.family_request_digest, digest(request_bytes));

    // The documented contract: both bump witnesses come from the lifecycle
    // policy and the observed addresses, never from the caller.
    let decoded = ControllerRequestV2::decode(request_bytes).expect("canonical family request");
    assert_ne!(fixture.primary_state_bump, PLACEHOLDER_BUMP);
    assert_eq!(decoded.state_bump, fixture.primary_state_bump);
    assert_eq!(decoded.terminal_record_bump, 0);
    assert_eq!(
        ControllerRequestV2 {
            state_bump: PLACEHOLDER_BUMP,
            terminal_record_bump: PLACEHOLDER_BUMP,
            ..decoded
        },
        placeholder_request(action),
        "only the two bump witnesses may differ from the request that was passed in"
    );
    assert_eq!(report.lifecycle.primary_state, fixture.primary_state);
    assert_eq!(
        report.lifecycle.primary_state_bump,
        fixture.primary_state_bump
    );
    assert_eq!(report.lifecycle.terminal_state, None);
    assert_eq!(report.lifecycle.terminal_state_bump, None);
    assert_eq!(report.lifecycle.terminal_coordinate, None);

    // The only signer in the whole frame is the lifecycle payer, and the
    // AccountProfile is what says so.
    let payer = fixture
        .state
        .runtime_suffix_accounts
        .iter()
        .filter(|account| account.is_signer)
        .map(|account| account.account.key)
        .collect::<Vec<_>>();
    assert_eq!(report.required_instruction_signers, payer);
}

/// Every General action builds a complete instruction at the Product width.
///
/// This is the test that found the defect ADR 0006 section 8 item 4 was after.
/// When it was first written, six of the seven actions refused with
/// `GeneralHotOperatorErrorV3::Lifecycle`: the tail of
/// `project_general_lifecycle_v5` compared `general_child_account_start_v3`
/// against literal 8/9, which is `general_readonly_evidence_start_v3`'s table
/// copied. Children begin *after* evidence, so that equality held only for an
/// action naming no readonly evidence -- `Freeze` alone. The guard had never
/// fired because the builder had no caller. It has since been split into two
/// conjuncts with independent authors, and all seven actions build.
///
/// The loop is deliberately over `GENERAL_ACTIONS_V3` rather than a list: an
/// eighth action must either build or explain itself here.
#[test]
fn every_general_action_builds_at_the_product_authenticated_width() {
    for action in GENERAL_ACTIONS_V3 {
        let fixture = build_fixture(action);
        let (selection, bytes) = fixture
            .artifacts
            .selected(action)
            .expect("selected artifacts");
        let built = build_general_hot_instruction_v3(
            &fixture.state,
            selection,
            bytes,
            placeholder_request(action),
        );
        assert!(built.is_ok(), "{action:?} must build: {built:?}");
        let report = built.expect("checked immediately above");
        assert_eq!(
            report.instruction.accounts.len(),
            generated_instruction_accounts(action),
            "{action:?}"
        );
        assert_eq!(report.action, action, "{action:?}");
        assert_eq!(report.outcome_count, OUTCOME_COUNT, "{action:?}");
        assert_eq!(report.product_record, fixture.product_record, "{action:?}");
        assert_eq!(
            report.lifecycle.primary_state, fixture.primary_state,
            "{action:?}"
        );
        assert_eq!(
            report.lifecycle.terminal_state, fixture.terminal_state,
            "{action:?}"
        );
        // The child frame starts where General says it does, and only `Close`
        // consumes a revision into a terminal coordinate.
        assert_eq!(
            report.lifecycle.child_account_start,
            general_child_account_start_v3(action),
            "{action:?}"
        );
        assert_eq!(
            report.lifecycle.terminal_coordinate,
            (action == Action::Close).then_some(1),
            "{action:?}"
        );
        let request = ControllerRequestV2::decode(
            report
                .instruction
                .data
                .get(HOT_FAMILY_REQUEST_OFFSET_V3..)
                .expect("family request"),
        )
        .expect("canonical family request");
        assert_eq!(
            request.state_bump, fixture.primary_state_bump,
            "{action:?} state bump is derived"
        );
        assert_eq!(
            request.terminal_record_bump,
            fixture.terminal_state_bump.unwrap_or_default(),
            "{action:?} terminal bump is derived"
        );
        assert_ne!(request.state_bump, PLACEHOLDER_BUMP, "{action:?}");
        assert_ne!(request.terminal_record_bump, PLACEHOLDER_BUMP, "{action:?}");
    }
}

/// A deployment with no checked release evidence is refused before anything else.
///
/// `CheckedGeneralHotReleaseV3` is the one authority a chain reader cannot
/// manufacture from self-consistent chain state, so its absence has to be fatal
/// even though every other byte in the snapshot is the accepted one.
#[test]
fn a_snapshot_without_checked_release_evidence_is_unrecognized() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| state.checked_release = None),
        GeneralHotOperatorErrorV3::UnrecognizedRelease
    );
}

/// Exactly one fixed account may be writable, and it is the composite root.
///
/// Requesting write on the Market -- one bit, on the account whose key the
/// envelope commits to -- is the cheapest way to escalate a read-only hot
/// action, so `validate_fixed_frame` refuses it by position rather than by
/// account role.
#[test]
fn a_second_writable_account_in_the_fixed_frame_refuses() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            fixed_at(state, HOT_MARKET_ACCOUNT_V3).is_writable = true;
        }),
        GeneralHotOperatorErrorV3::FixedFrame
    );
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            fixed_at(state, HOT_ROOT_ACCOUNT_V3).is_writable = false;
        }),
        GeneralHotOperatorErrorV3::FixedFrame
    );
}

/// One account read a slot later than the rest is not the same snapshot.
///
/// The refusal differs by where the drift is: a fixed coordinate fails the
/// frame walk, while a strategy or runtime coordinate fails the snapshot join.
/// Both are one `u64` away from accepted.
#[test]
fn a_single_account_from_a_different_slot_breaks_the_snapshot() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            fixed_at(state, HOT_CONFIG_RAW_ACCOUNT_V3)
                .account
                .observation
                .slot += 1;
        }),
        GeneralHotOperatorErrorV3::FixedFrame
    );
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            state
                .runtime_suffix_accounts
                .first_mut()
                .expect("runtime frame")
                .account
                .observation
                .slot += 1;
        }),
        GeneralHotOperatorErrorV3::Snapshot
    );
}

/// A runtime account one byte off its declared width is not the declared account.
///
/// The AccountProfile rule for the lifecycle RentCredit coordinate is an exact
/// width, so a single trailing byte makes the physical frame disagree with the
/// artifact that authored it.
#[test]
fn a_runtime_account_one_byte_off_its_profile_width_refuses() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            let rent_credit = state
                .runtime_suffix_accounts
                .iter_mut()
                .find(|account| {
                    account.is_writable && !account.is_signer && !account.account.data.is_empty()
                })
                .expect("lifecycle rent credit");
            rent_credit.account.data.push(0);
        }),
        GeneralHotOperatorErrorV3::RuntimeGeometry
    );
}

/// The admitted-AOT transport frame is read-only in its entirety.
#[test]
fn a_writable_strategy_transport_account_refuses() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            state
                .strategy_accounts
                .first_mut()
                .expect("strategy frame")
                .is_writable = true;
        }),
        GeneralHotOperatorErrorV3::StrategyGeometry
    );
}

/// The action state must be the address the lifecycle policy derives.
///
/// A caller who supplies its own account here would have the Trading program
/// create or authenticate state at an address the policy never named.
#[test]
fn an_action_state_at_a_foreign_address_refuses() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            state
                .runtime_suffix_accounts
                .first_mut()
                .expect("primary action state")
                .account
                .key = Pubkey::new_from_array([0xd7; 32]);
        }),
        GeneralHotOperatorErrorV3::Lifecycle
    );
}

/// A Product staging cursor holding data is not a finalized coordinate.
#[test]
fn a_nonvacant_product_staging_cursor_refuses() {
    assert_eq!(
        refuse_after(Action::Freeze, |state| {
            fixed_at(state, HOT_PRODUCT_RAW_ACCOUNT_V3 + 1)
                .account
                .data
                .push(0);
        }),
        GeneralHotOperatorErrorV3::Product
    );
}

/// One finalized canonical lookup table for an already-built instruction.
fn canonical_lookup_table(report: &GeneralHotInstructionV3, payer: Pubkey) -> ObservedAccount {
    let addresses = canonical_general_lookup_addresses_v3(&report.instruction, payer)
        .expect("canonical lookup addresses");
    let table = AddressLookupTable {
        meta: LookupTableMeta {
            authority: Some(Pubkey::new_from_array([0xd1; 32])),
            last_extended_slot: FINALIZED_SLOT - 1,
            deactivation_slot: u64::MAX,
            ..LookupTableMeta::default()
        },
        addresses: Cow::Owned(addresses),
    };
    observed(
        Pubkey::new_from_array([0xd2; 32]),
        lookup_table_program::id(),
        false,
        table.serialize_for_tests().expect("lookup table bytes"),
    )
}

/// The real account set compiles to a packet-safe ALT-backed v0 message.
///
/// The operator's own packet evidence
/// (`every_action_is_alt_packet_safe_at_the_canonical_runtime_width`) runs on
/// ninety-one fabricated `AccountMeta`s. This is the same claim against an
/// instruction the builder actually produced from chain state: the keys are
/// the real Market, root, records, deployments, sysvars and state PDAs, and
/// the data is the real envelope plus the real controller request.
#[test]
fn the_built_instruction_compiles_into_a_packet_safe_v0_message() {
    let action = Action::Freeze;
    let fixture = build_fixture(action);
    let (selection, bytes) = fixture
        .artifacts
        .selected(action)
        .expect("selected artifacts");
    let report = build_general_hot_instruction_v3(
        &fixture.state,
        selection,
        bytes,
        placeholder_request(action),
    )
    .expect("accepted General Hot instruction");

    let payer = Pubkey::new_from_array([0xd0; 32]);
    let plan = compile_general_hot_v0(
        &report,
        payer,
        Hash::new_from_array([0x16; 32]),
        &canonical_lookup_table(&report, payer),
    )
    .expect("packet-safe ALT-backed v0 message");
    assert!(plan.message.wire_bytes <= PACKET_DATA_BYTES);
    assert!(plan.message.loaded_addresses > 0);
    // The fee payer signs, and so does the lifecycle payer the AccountProfile
    // names: two signature slots, in that order.
    assert_eq!(
        plan.required_signers,
        core::iter::once(payer)
            .chain(report.required_instruction_signers.iter().copied())
            .collect::<Vec<_>>()
    );
    assert_eq!(usize::from(plan.message.required_signatures), 2);
    assert_eq!(plan.action, action);
    assert_eq!(plan.outcome_count, OUTCOME_COUNT);
    assert_eq!(plan.product_record, fixture.product_record);
    assert_eq!(plan.lifecycle, report.lifecycle);

    // A table that is one address short of canonical is refused even though
    // Solana could still compile a message from it.
    let mut short = canonical_lookup_table(&report, payer);
    let mut addresses = AddressLookupTable::deserialize(&short.data)
        .expect("table")
        .addresses
        .into_owned();
    addresses.pop();
    short.data = AddressLookupTable {
        meta: LookupTableMeta {
            authority: Some(Pubkey::new_from_array([0xd1; 32])),
            last_extended_slot: FINALIZED_SLOT - 1,
            deactivation_slot: u64::MAX,
            ..LookupTableMeta::default()
        },
        addresses: Cow::Owned(addresses),
    }
    .serialize_for_tests()
    .expect("short table bytes");
    assert_eq!(
        compile_general_hot_v0(&report, payer, Hash::new_from_array([0x16; 32]), &short),
        Err(GeneralHotOperatorErrorV3::LookupTable)
    );
}
