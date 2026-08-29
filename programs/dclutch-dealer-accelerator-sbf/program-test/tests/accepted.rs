//! Executed caller evidence for the Dealer scenario accepted transition.
//!
//! The canonical unsplit admitted Hot instruction for this scenario resolves
//! 121 account locks against a 64-lock runtime ceiling, so it can never be
//! submitted anywhere -- devnet, mainnet, or this harness. The lock-bounded
//! checkpoint routes are the submittable form of the same transition, and this
//! campaign is the first thing that actually submits them.
//!
//! What runs here is a real caller against the real Trading ELF: the transcript
//! `dclutch_operator::dealer_scenario_checkpoint_v1::build_dealer_accepted_transcript_v4`
//! emits is signed and processed transaction by transaction, and the durable
//! journal advances only on observed success. Every instruction's account order,
//! privileges and route data come from the operator; this file states no span,
//! no bitmap, and no account order of its own.
//!
//! This is a Dealer scenario accepted-transition campaign. It selects no price,
//! quotes nothing, and holds no inventory. It is not an AMM, an order book, or a
//! quote surface.

use std::{env, vec::Vec};

use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_claims_svm::frame_spec_v1::{ClaimsFrameRoleV1, SignedDeltaFrameSpecV3};
use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketInputV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionInputV2,
    LiabilityBasisPositionViewV2, encode_liability_basis_market_into_v2,
    encode_liability_basis_position_into_v2,
};
use dclutch_fractional_atomic_program_test::narrow_fixture::{
    NarrowFixtureInputV2, NarrowFixtureV2, NarrowPositionV2, compile_narrow_fixture_v2,
};
use dclutch_claims_svm::signed_delta_v3::SignedDeltaPlanV3;
use dclutch_custody_contract::{CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CompartmentV1, CustodyVaultSeedsV1};
use dclutch_dealer_codec::{
    scenario::ClaimsInventoryObservation,
    scenario_checkpoint_v1::DEALER_SCENARIO_PREPARATION_PAGES_V1,
    scenario_custody_reservation_v1::{
        DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1, DealerScenarioCustodyEffectManifestV1,
        DealerScenarioCustodyEffectV1, DealerScenarioCustodyRequestKindV1,
        DealerScenarioReservationBatchStatusV1, DealerScenarioReservationBatchV1,
        DealerScenarioReservationStateStatusV1, DealerScenarioReservationStateV1,
    },
    scenario_membership_manifest_v1::{
        DEALER_SCENARIO_MEMBERSHIP_PAGES_V1, DealerScenarioMembershipManifestV1,
    },
    scenario_reservation_receipt_v1::{
        DEALER_SCENARIO_MAX_RESERVATIONS_V1, DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
        DealerScenarioReservationActionV1, DealerScenarioReservationReceiptV1,
    },
};
use dclutch_operator::{
    dealer_scenario_checkpoint_v1::{
        DealerScenarioCheckpointJournalV1, DealerScenarioCheckpointRouteV1,
        DealerAcceptedEvaluationAccountsV4, DealerAcceptedReservationAccountsV4,
        DealerAcceptedTranscriptInputV4, DealerScenarioCommitAccountsV1,
        DealerScenarioCommitEffectAccountsV1, DealerScenarioEvaluationBodiesV1,
        build_dealer_accepted_transcript_v4, build_dealer_scenario_checkpoint_cleanup_v1,
        build_dealer_scenario_checkpoint_create_v1, build_dealer_scenario_checkpoint_reserve_v1,
        build_dealer_scenario_commit_v1,
        build_dealer_scenario_checkpoint_evaluate_v1, build_dealer_scenario_checkpoint_page_v1,
        dealer_scenario_checkpoint_address_v1, dealer_scenario_evaluation_receipt_address_v1,
        dealer_scenario_reservation_batch_address_v1,
        dealer_scenario_membership_manifest_address_v1,
        derive_dealer_scenario_evaluation_receipt_v1,
        project_dealer_scenario_canonical_membership_pages_v1,
    },
    dealer_scenario_hot_v4::{
        DealerScenarioHotMetaStateV4, DealerScenarioSemanticStateV4,
        SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1, dealer_hot_frame_projection_v4,
        project_dealer_scenario_hot_semantics_v4, project_dealer_scenario_unsplit_topology_v4,
    },
    direct_inline_v3::ObservedAccountMetaV3,
};
use dclutch_registry_activation_auth_v1::activation_cache_address_v1;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_core_contract::ContentId;
use dclutch_trading_sbf::dealer::{
    v3_composer::{ScenarioCollateralFrameV3, ScenarioComposerContextV3},
    v3_obligation::stage_scenario_obligation_replacement_v3,
    v3_trade_profile::{
        DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DEALER_SCENARIO_PROFILE_SPANS_V4,
        DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
    },
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
    v3_trade::{
        DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
        DealerScenarioTradeRequestV3,
        ScenarioTradeChainProjectionV3, ScenarioTradeDirectionV3, ScenarioTradeIntentV3,
        build_scenario_trade_request_v3, scenario_trade_max_request_bytes_v3,
    },
};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_message::{VersionedMessage, v0};
use solana_message_v3::AddressLookupTableAccount as OperatorLookupTable;
use solana_message::AddressLookupTableAccount;
use solana_transaction::versioned::VersionedTransaction;
use solana_sdk::transaction::TransactionError;
use solana_address_lookup_table_interface::instruction::{create_lookup_table, extend_lookup_table};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

/// Release-selected Trading program the campaign installs the real ELF at.
const TRADING: Pubkey = Pubkey::new_from_array([0xd0; 32]);
/// Producer that owns the canonical membership manifest.
const MANIFEST_PRODUCER: Pubkey = Pubkey::new_from_array([0xd1; 32]);
/// Immutable rent beneficiary named at creation.
const BENEFICIARY: Pubkey = Pubkey::new_from_array([0xd2; 32]);
/// Counterparty Claims Position owner.
const COUNTERPARTY: Pubkey = Pubkey::new_from_array([0xd3; 32]);
/// Counterparty external collateral account.
const COUNTERPARTY_ACCOUNT: Pubkey = Pubkey::new_from_array([0xd4; 32]);
/// Immutable Trading child root.
const CHILD_ROOT: Pubkey = Pubkey::new_from_array([0xd5; 32]);
/// Logical Core Market.
const MARKET: Pubkey = Pubkey::new_from_array([0xd6; 32]);
/// Exact immutable Dealer request account.
const REQUEST: Pubkey = Pubkey::new_from_array([0xd7; 32]);
/// Producer-owned exact candidate register bank.
const CANDIDATE_BANK: Pubkey = Pubkey::new_from_array([0xd8; 32]);
/// Producer-owned exact candidate obligation body.
const CANDIDATE_OBLIGATION: Pubkey = Pubkey::new_from_array([0xd9; 32]);
/// Producer-owned exact expected Claims delta body.
const CLAIMS_DELTA: Pubkey = Pubkey::new_from_array([0xda; 32]);
/// Producer-owned ordered Custody-effect manifest.
const EFFECTS: Pubkey = Pubkey::new_from_array([0xdb; 32]);
/// Producer-owned single Custody effect body the manifest names.
const EFFECT_BODY: Pubkey = Pubkey::new_from_array([0xdc; 32]);
/// A real executable program the activated release set never named as Custody.
const UNACTIVATED_PRODUCER: Pubkey = Pubkey::new_from_array([0xdd; 32]);
/// Release-selected Custody program.
const CUSTODY_PROGRAM: Pubkey = Pubkey::new_from_array([0xe1; 32]);
/// Release-selected Claims program.
const CLAIMS_PROGRAM: Pubkey = Pubkey::new_from_array([0xe6; 32]);
/// Release-selected Core program.
const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0xe7; 32]);

/// The refusal Trading raises when a route's account content is not canonical.
const TRADING_CONTENT: u32 = 0x4003;
/// The refusal Trading raises when a checked data-defined transition refuses.
const TRADING_TRANSITION: u32 = 0x4004;
/// The refusal Trading raises when a projected physical mutation cannot commit.
const TRADING_COMMIT: u32 = 0x4005;
/// The refusal Trading raises when the release waist does not authenticate.
const TRADING_RELEASE: u32 = 0x4001;
/// The Claims SignedDelta route's refusal when the aggregate state does not join.
const CLAIMS_SIGNED_DELTA_STATE: u32 = 0x5204;

/// Runtime Product outcome width this scenario transitions.
const WIDTH: u32 = 3;
/// Representation coordinate the Claims graph funds and this scenario trades at.
const FUNDED_COORDINATE: usize = 0;
/// Immutable Realm identity the Claims graph is founded under.
const SCENARIO_REALM: [u8; 32] = [0xb6; 32];
/// Immutable Custody replay namespace for this Market.
const SCENARIO_CUSTODY_CONTEXT: [u8; 32] = [0xbb; 32];
/// Market generation every layer of this scenario restates.
const SCENARIO_GENERATION: u64 = 17;
/// Claims aggregate revision this scenario trades against.
///
/// The narrow fixture plants the pre-founding revision zero, which is right for
/// a founding campaign and unreachable for this one: the Dealer projection
/// refuses a Position at revision zero, because a Dealer trade is against
/// Positions that have already been transacted. The graph is re-encoded through
/// the supported Claims encoders at a live revision rather than byte-patched.
const LIVE_CLAIMS_REVISION: u64 = 4;
/// Position revision both Claims Positions carry.
const LIVE_POSITION_REVISION: u64 = 3;
/// Last slot at which this scenario's checkpoint may still be advanced.
///
/// Commit admits only a live checkpoint, and building the address lookup table
/// the commit needs costs several slots, so the preparation window has to
/// outlast its own transport.
const SCENARIO_EXPIRES_AT: u64 = 5_000;

/// The five execution roles one activated release set binds.
const ALL_ROLES: [ExecutionRoleV1; 5] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

/// Deployment slot every role in this release set is pinned to.
///
/// Genesis writes slot zero and the bank runs above it, so an immutable
/// slot-pinned release is the only generation loadable here.
const WAIST_SLOT: u64 = 0;

/// Read one real artifact the campaign is evidence about.
fn elf(name: &str) -> Vec<u8> {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required: this campaign is real-ELF evidence and refuses to run without \
         the artifacts under test",
    );
    std::fs::read(std::path::Path::new(&directory).join(format!("{name}.so")))
        .expect("the campaign refuses to run without its real artifact")
}

/// The genuine release waist the reservation route authenticates against.
///
/// This is not a stand-in. Reservation reaches the Registry-owned activation
/// cache through `authenticate_activated_role_v1`, which derives the cache
/// address from the release set, requires the Registry to own it, hostile-
/// decodes the whole fixed-width record, and then pins the Custody role to the
/// exact Loader V3 deployment slot its activation observed. Every one of those
/// facts is staged here the way the Registry writes them.
struct ReleaseWaist {
    registry: Pubkey,
    release_set_id: [u8; 32],
    custody_program: Pubkey,
    custody_programdata: Pubkey,
    claims_program: Pubkey,
    claims_programdata: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    activation_cache: Pubkey,
    cache_body: Vec<u8>,
    deployments: Vec<(&'static str, Pubkey, Vec<u8>)>,
}

/// The exact 36-byte Loader V3 Program account body.
fn loader_program_body(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0_u8; 36];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..36)
        .expect("link")
        .copy_from_slice(programdata.as_ref());
    output
}

/// The exact 45-byte Loader V3 ProgramData metadata span, then the ELF.
fn loader_programdata_body(slot: u64, authority: Option<[u8; 32]>, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    if let Some(authority) = authority {
        output
            .get_mut(12..13)
            .expect("option tag")
            .copy_from_slice(&[1]);
        output
            .get_mut(13..45)
            .expect("authority")
            .copy_from_slice(&authority);
    }
    output.get_mut(45..).expect("elf").copy_from_slice(elf);
    output
}

/// Loader V3 ProgramData address for one program.
fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// One slot-pinned immutable release over a real deployed artifact.
fn artifact_release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program identity"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity"),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        WAIST_SLOT,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

/// Activation input for one release, observed exactly as staged.
fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let artifact = ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
        .expect("artifact identity");
    let observation = DeploymentObservationV1::new(
        release.program().to_bytes(),
        bpf_loader_upgradeable::ID.to_bytes(),
        true,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        false,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        release.deployment_slot(),
        release.elf_digest(),
        release.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact, release, observation)
}

/// Build the multi-role activated release set this campaign executes under.
///
/// Every role is bound to a real deployed artifact at the slot its activation
/// observed. Trading is bound to the Trading program because Trading is the
/// caller inside the Claims frame; Custody to Custody because reservation
/// authenticates that role; Claims and Core because the commit frame
/// authenticates both.
fn release_waist() -> ReleaseWaist {
    release_waist_for(CUSTODY_PROGRAM)
}

/// Build one activated release set over a chosen Custody program identity.
fn release_waist_for(custody_program: Pubkey) -> ReleaseWaist {
    let registry = Pubkey::new_from_array([0xe0; 32]);
    let custody_elf = elf("dclutch_custody_sbf");
    let claims_elf = elf("dclutch_claims_sbf");
    let core_elf = elf("dclutch_core_sbf");
    let trading_elf = elf("dclutch_trading_sbf");
    let custody = artifact_release(custody_program, 0xe3, &custody_elf);
    let claims = artifact_release(CLAIMS_PROGRAM, 0xe4, &claims_elf);
    let core = artifact_release(CORE_PROGRAM, 0xe5, &core_elf);
    let trading = artifact_release(TRADING, 0xe6, &trading_elf);
    let bind = |release: ArtifactReleaseV1| {
        ExecutionRoleBindingV1::new(
            release.program(),
            ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact id"),
        )
    };
    let release_set = ExecutionReleaseSetV1::new(
        bind(core),
        bind(claims),
        bind(trading),
        bind(claims),
        bind(custody),
    )
    .expect("multi-role release set");
    let release_set_id =
        ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");
    let mut cache_body = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache_body, release_set_id).expect("initialize cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut cache_body,
            release_set_id,
            &release_set,
            role,
            &activation_input(release),
        )
        .expect("activate role");
    }
    ReleaseWaist {
        registry,
        release_set_id: release_set_id.to_bytes(),
        custody_program,
        custody_programdata: programdata_address(custody_program),
        claims_program: CLAIMS_PROGRAM,
        claims_programdata: programdata_address(CLAIMS_PROGRAM),
        core_program: CORE_PROGRAM,
        core_programdata: programdata_address(CORE_PROGRAM),
        trading_programdata: programdata_address(TRADING),
        activation_cache: activation_cache_address_v1(&registry, &release_set_id.to_bytes()),
        cache_body,
        deployments: vec![
            ("dclutch_custody_sbf", custody_program, custody_elf),
            ("dclutch_claims_sbf", CLAIMS_PROGRAM, claims_elf),
            ("dclutch_core_sbf", CORE_PROGRAM, core_elf),
            ("dclutch_trading_sbf", TRADING, trading_elf),
        ],
    }
}

/// Encode the canonical Dealer account profile for one set of common widths.
fn canonical_profile(common_data_lengths: [u32; 5]) -> Vec<u8> {
    let mut scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    let mut output = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    encode_dealer_scenario_account_profile_v4_atomic(
        DealerScenarioAccountProfileInputV4 {
            common_data_lengths,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical account profile");
    output
}

/// One frame observation at a canonical coordinate.
fn frame_meta(
    index: usize,
    bytes: usize,
    signer: bool,
    writable: bool,
    executable: bool,
) -> ObservedAccountMetaV3 {
    ObservedAccountMetaV3 {
        account: ObservedAccount {
            observation: observation(),
            key: Pubkey::new_from_array([u8::try_from(index + 1).expect("small coordinate"); 32]),
            owner: Pubkey::new_from_array([200; 32]),
            lamports: 1,
            executable,
            data: vec![0; bytes],
        },
        is_signer: signer,
        is_writable: writable,
    }
}

/// Derive the physical Dealer frame for one runtime width and span set.
///
/// Every coordinate, width and privilege here comes from the account profile
/// and from the supported frame projection. The campaign states none of them.
fn physical_frame(
    tail_count: u32,
    span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
) -> (Vec<ObservedAccountMetaV3>, Vec<ObservedAccountMetaV3>) {
    let projection = dealer_hot_frame_projection_v4();
    let common_lengths = [32_u32, 128, 48, 56, 64];
    let profile_bytes = canonical_profile(common_lengths);
    let profile = AccountProfileV2::decode(&profile_bytes).expect("decode profile");
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(tail_count, &span_counts)
        .expect("physical count");
    let mut fixed_accounts = (0..projection.fixed_account_count)
        .map(|index| frame_meta(index, 0, false, index == HOT_ROOT_ACCOUNT_V3, false))
        .collect::<Vec<_>>();
    fixed_accounts
        .get_mut(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
        .expect("profile coordinate")
        .account
        .data = profile_bytes.clone();
    for (length, index) in common_lengths
        .into_iter()
        .zip(projection.injected_physical_indices)
    {
        fixed_accounts
            .get_mut(index)
            .expect("injected coordinate")
            .account
            .data = vec![0; usize::try_from(length).expect("common width")];
    }
    let mut suffix = Vec::new();
    for ordinal in projection.injected_account_count..physical_count {
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(tail_count, &span_counts, ordinal)
            .expect("physical geometry");
        let privileges = geometry.privileges();
        let bytes = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes }
            | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                minimum_bytes
            }
            PhysicalAccountDataGeometryV2::Opaque => 7,
        };
        suffix.push(frame_meta(
            projection.fixed_account_count + ordinal,
            bytes,
            privileges.signer(),
            privileges.writable(),
            privileges.executable(),
        ));
    }
    (fixed_accounts, suffix)
}

fn observation() -> Observation {
    Observation {
        slot: 20,
        unix_timestamp: 12,
        finality: Finality::Finalized,
    }
}

fn obligation_bytes(
    market: [u8; 32],
    product: [u8; 32],
    basis: [u8; 32],
    owner: [u8; 32],
    child: [u8; 32],
    revision: u64,
    values: &[u64],
) -> Vec<u8> {
    let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
    bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
    bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
    bytes[12..16].copy_from_slice(
        &u32::try_from(values.len())
            .expect("small obligation width")
            .to_le_bytes(),
    );
    bytes[16..24].copy_from_slice(&revision.to_le_bytes());
    for (offset, value) in [
        (24, market),
        (56, product),
        (88, basis),
        (120, owner),
        (152, child),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&value);
    }
    for (index, value) in values.iter().enumerate() {
        let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn program_set_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 72];
    bytes[..8].copy_from_slice(b"DCLTCPS1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3.to_le_bytes());
    bytes[16] = 2;
    bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
    bytes[32..36].copy_from_slice(&u32::from(DEALER_SCENARIO_TRADE_ACTION_V3).to_le_bytes());
    bytes[36..68].copy_from_slice(&[42; 32]);
    bytes
}

fn data_account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

/// Everything the campaign installs and every derived fact it re-checks.
struct Scenario {
    dealer: Keypair,
    request_bytes: Vec<u8>,
    request_digest: [u8; 32],
    obligation: Pubkey,
    obligation_state: Vec<u8>,
    checkpoint: Pubkey,
    membership_manifest: Pubkey,
    manifest_bytes: Vec<u8>,
    pages: [Vec<Pubkey>; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
    membership: Vec<Pubkey>,
    frame_accounts: Vec<(Pubkey, Account)>,
    candidate_obligation_bytes: Vec<u8>,
    fixture: NarrowFixtureV2,
    live: LiveClaimsGraph,
    core_market: Pubkey,
    unsplit_account_lock_count: usize,
    waist: ReleaseWaist,
}

/// Derive one complete scenario: request, checkpoint, canonical membership.
fn scenario() -> Scenario {
    let waist = release_waist();
    let dealer = Keypair::new();
    // The Claims aggregate graph is compiled by the Fractional narrow fixture,
    // which is parameterized by outcome width precisely so a second campaign can
    // consume it. This is that second campaign; nothing here re-derives a Claims
    // coordinate of its own.
    let fixture = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: usize::try_from(WIDTH).expect("small width"),
        registry_program: waist.registry,
        core_program: waist.core_program,
        claims_program: waist.claims_program,
        release_set: waist.release_set_id,
        realm_id: SCENARIO_REALM,
        custody_context: SCENARIO_CUSTODY_CONTEXT,
        generation: SCENARIO_GENERATION,
        actor_owner: dealer.pubkey(),
        reserve_owner: COUNTERPARTY,
        funded_coordinate: FUNDED_COORDINATE,
        funded_balance: 100,
        reserve_balance: 100,
        terminal: None,
        rent_beneficiary: BENEFICIARY,
        graph_id: [0xb9; 32],
        exposure_id: [0xba; 32],
    })
    .expect("canonical Claims graph");
    let live = live_claims_graph(&fixture);
    let claims_market_view =
        LiabilityBasisMarketViewV2::decode(&live.market).expect("claims aggregate decodes");
    let dealer_inventory = live.dealer_balances.clone();
    let counterparty_inventory = live.counterparty_balances.clone();
    let dealer_owner = dealer.pubkey().to_bytes();
    let market = fixture.core_market.to_bytes();
    let product = fixture.product_id;
    let basis = fixture.semantic_basis_id;
    let child = CHILD_ROOT.to_bytes();
    let obligation_state = obligation_bytes(market, product, basis, dealer_owner, child, 7, &[
        12, 20, 10,
    ]);
    let current_obligation =
        DealerObligationProjectionV3::decode(&obligation_state).expect("canonical obligation");
    let obligation = Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child], &TRADING).0;
    let chain = ScenarioTradeChainProjectionV3 {
        trading_program: TRADING.to_bytes(),
        release_set: waist.release_set_id,
        market,
        child_root: child,
        obligation_address: obligation.to_bytes(),
        current_obligation,
        dealer_position: ClaimsInventoryObservation {
            market_id: market,
            product_id: product,
            liability_basis_id: basis,
            position_owner: dealer_owner,
            revision: LIVE_POSITION_REVISION,
            inventory: &dealer_inventory,
        },
        counterparty_position: ClaimsInventoryObservation {
            market_id: market,
            product_id: product,
            liability_basis_id: basis,
            position_owner: COUNTERPARTY.to_bytes(),
            revision: LIVE_POSITION_REVISION,
            inventory: &counterparty_inventory,
        },
        product_record_digest: fixture.product.digest,
        linked_basis_record_digest: fixture.linked_basis.digest,
        counterparty_account: COUNTERPARTY_ACCOUNT.to_bytes(),
        principal_balance: 100,
        locked_capital_floor: 0,
        claims_revision: claims_market_view.revision,
        generation: SCENARIO_GENERATION,
        now: 20,
        expires_at: SCENARIO_EXPIRES_AT,
        terminal: false,
    };
    // The narrow fixture funds ONE representation coordinate per Position, so
    // this first executed commit trades at that coordinate. Dealer scenarios are
    // not restricted to one coordinate in general; the campaign is.
    let mut acquired = vec![0_u64; usize::try_from(WIDTH).expect("small width")];
    let mut delivered = vec![0_u64; usize::try_from(WIDTH).expect("small width")];
    // Acquired and delivered must be disjoint per coordinate, and the graph
    // funds exactly one, so this trade moves value one way at that coordinate.
    *acquired.get_mut(FUNDED_COORDINATE).expect("funded coordinate") = 10;
    let intent = ScenarioTradeIntentV3 {
        direction: ScenarioTradeDirectionV3::CounterpartyPaysDealer,
        principal: 10,
        realized_fee: 1,
        acquired: &acquired,
        delivered: &delivered,
        candidate_obligations: &[10, 19, 13],
    };
    let set_bytes = program_set_bytes();
    let set = CapabilityProgramSetV1::decode(&set_bytes).expect("canonical program set");
    let mut request_bytes =
        vec![0; scenario_trade_max_request_bytes_v3(WIDTH).expect("request bound")];
    let built = build_scenario_trade_request_v3(chain, intent, set, &mut request_bytes)
        .expect("chain-derived request");
    request_bytes.truncate(built.request_bytes);
    let request_digest = hash(&request_bytes).to_bytes();
    // The candidate obligation is not a fixture choice: commit re-derives it
    // from the current body and the request's own candidate vector, and refuses
    // anything whose bytes or digest differ.
    let mut candidate_obligation_bytes = vec![0_u8; obligation_state.len()];
    stage_scenario_obligation_replacement_v3(
        current_obligation,
        intent.candidate_obligations,
        &mut candidate_obligation_bytes,
    )
    .expect("candidate obligation replacement");
    let checkpoint = dealer_scenario_checkpoint_address_v1(TRADING, request_digest);
    let membership_manifest =
        dealer_scenario_membership_manifest_address_v1(MANIFEST_PRODUCER, checkpoint, request_digest);

    // The membership transcript is the complete physical Dealer frame for this
    // scenario after alias de-duplication. Its width is the reason the split
    // exists: one instruction naming all of it cannot be submitted.
    // The physical frame is derived, not chosen. The semantic projection fixes
    // the span widths and the caller-authority count; the account profile fixes
    // every coordinate and privilege; and the unsplit topology below is the
    // proof that what the pages carry really is the 121-lock scenario the split
    // exists to make submittable.
    let vault = |context, compartment| {
        Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(market, waist.release_set_id, context, compartment)
                .as_slices(),
            &waist.custody_program,
        )
        .0
        .to_bytes()
    };
    let semantic = DealerScenarioSemanticStateV4 {
        chain,
        context: ScenarioComposerContextV3 {
            trading_program: TRADING.to_bytes(),
            custody_program: waist.custody_program.to_bytes(),
            release_set: waist.release_set_id,
            market,
            realm: SCENARIO_REALM,
            child_root: child,
            obligation_account: obligation.to_bytes(),
            mint: [0xb7; 32],
            token_program: [0xb8; 32],
            parent_request_digest: request_digest,
            generation: SCENARIO_GENERATION,
            custody_replay_revision: 7,
            locked_capital_floor: 0,
        },
        collateral: ScenarioCollateralFrameV3 {
            principal_vault: vault(child, CompartmentV1::TradingPrincipal),
            principal_balance: 100,
            fee_vault: vault(child, CompartmentV1::FeeVault),
            fee_balance: 9,
            hoard_vault: vault(market, CompartmentV1::HoardPrincipal),
            hoard_balance: 100,
            counterparty_account: COUNTERPARTY_ACCOUNT.to_bytes(),
            counterparty_owner: COUNTERPARTY.to_bytes(),
            counterparty_external_delegate: Pubkey::find_program_address(
                &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &waist.release_set_id],
                &waist.custody_program,
            )
            .0
            .to_bytes(),
            counterparty_external_delegated_amount: 11,
            counterparty_balance: 100,
        },
    };
    let projected = project_dealer_scenario_hot_semantics_v4(semantic, &request_bytes)
        .expect("semantic projection");
    let (mut fixed_accounts, suffix) =
        physical_frame(WIDTH, projected.dynamic_span_counts);
    let frame = dealer_hot_frame_projection_v4();
    fixed_accounts
        .get_mut(HOT_MARKET_ACCOUNT_V3)
        .expect("market coordinate")
        .account
        .key = fixture.core_market;
    fixed_accounts
        .get_mut(HOT_ROOT_ACCOUNT_V3)
        .expect("root coordinate")
        .account
        .key = CHILD_ROOT;
    fixed_accounts
        .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .expect("trading coordinate")
        .account
        .key = TRADING;
    let mut strategy_accounts = (0..frame.admitted_evidence_count + projected.caller_authority_count)
        .map(|index| frame_meta(200 + index, 0, false, false, index == 6))
        .collect::<Vec<_>>();
    strategy_accounts
        .get_mut(6)
        .expect("accelerator coordinate")
        .account
        .executable = true;
    let state = DealerScenarioHotMetaStateV4 {
        fixed_accounts: &fixed_accounts,
        strategy_accounts: &strategy_accounts,
        runtime_suffix_accounts: &suffix,
    };
    let unsplit = project_dealer_scenario_unsplit_topology_v4(state, semantic, &request_bytes)
        .expect("unsplit topology");
    // The frame aliases some coordinates, and the canonical partition sorts and
    // deduplicates once across the whole set, so the membership transcript is
    // the frame's distinct identities.
    let mut membership = fixed_accounts
        .iter()
        .chain(strategy_accounts.iter())
        .chain(suffix.iter())
        .map(|meta| meta.account.key)
        .collect::<Vec<_>>();
    membership.sort_unstable_by_key(Pubkey::to_bytes);
    membership.dedup();
    let mut frame_accounts = fixed_accounts
        .iter()
        .chain(strategy_accounts.iter())
        .chain(suffix.iter())
        .map(|meta| {
            (
                meta.account.key,
                Account {
                    lamports: Rent::default()
                        .minimum_balance(meta.account.data.len())
                        .max(1),
                    data: meta.account.data.clone(),
                    owner: meta.account.owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )
        })
        .collect::<Vec<_>>();
    frame_accounts.sort_by_key(|(key, _)| key.to_bytes());
    frame_accounts.dedup_by_key(|(key, _)| *key);
    let canonical = project_dealer_scenario_canonical_membership_pages_v1(
        state,
        MANIFEST_PRODUCER,
        checkpoint,
        request_digest,
    )
    .expect("canonical membership partition");
    let manifest_bytes = canonical.manifest.encode().expect("manifest encode").to_vec();
    Scenario {
        dealer,
        request_bytes,
        request_digest,
        obligation,
        obligation_state,
        checkpoint,
        membership_manifest,
        manifest_bytes,
        pages: canonical.pages,
        membership,
        frame_accounts,
        candidate_obligation_bytes,
        core_market: fixture.core_market,
        live,
        fixture,
        unsplit_account_lock_count: unsplit.unique_account_lock_count,
        waist,
    }
}

/// The compiled Claims graph re-encoded at a revision Dealer can trade against.
struct LiveClaimsGraph {
    market: Vec<u8>,
    dealer_position: Vec<u8>,
    counterparty_position: Vec<u8>,
    dealer_balances: Vec<u64>,
    counterparty_balances: Vec<u64>,
}

/// Re-encode the compiled graph at a live revision through supported encoders.
fn live_claims_graph(fixture: &NarrowFixtureV2) -> LiveClaimsGraph {
    let market_view =
        LiabilityBasisMarketViewV2::decode(&fixture.claims_market_bytes).expect("aggregate decodes");
    let supplies = (0..fixture.outcome_count)
        .map(|claim| {
            market_view
                .supply(&fixture.claims_market_bytes, claim)
                .expect("supply")
        })
        .collect::<Vec<_>>();
    let mut market = vec![0_u8; fixture.claims_market_bytes.len()];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: LIVE_CLAIMS_REVISION,
            logical_market: market_view.logical_market,
            release_set: market_view.release_set,
            registry_program: market_view.registry_program,
            product_instance_id: market_view.product_instance_id,
            basis_id: market_view.basis_id,
            realm_id: market_view.realm_id,
            custody_context: market_view.custody_context,
            generation: market_view.generation,
        },
        &supplies,
        &mut market,
    )
    .expect("live aggregate encodes");
    let relive = |position: &NarrowPositionV2| {
        let view = LiabilityBasisPositionViewV2::decode(&position.bytes).expect("position decodes");
        let balances = (0..fixture.outcome_count)
            .map(|claim| view.balance(&position.bytes, claim).expect("balance"))
            .collect::<Vec<_>>();
        let mut bytes = vec![0_u8; position.bytes.len()];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: LIVE_POSITION_REVISION,
                market_account: view.market_account,
                owner: view.owner,
                basis_id: view.basis_id,
            },
            &balances,
            &mut bytes,
        )
        .expect("live position encodes");
        (bytes, balances)
    };
    let (dealer_position, dealer_balances) = relive(&fixture.actor_position);
    let (counterparty_position, counterparty_balances) = relive(&fixture.reserve_position);
    LiveClaimsGraph {
        market,
        dealer_position,
        counterparty_position,
        dealer_balances,
        counterparty_balances,
    }
}

impl Scenario {
    /// Every account the compiled Claims graph owns, with its owner and body.
    ///
    /// A vacant staging cursor is a real protocol state: system-owned and
    /// exactly zero bytes. Installing it as anything else would make the
    /// finalized record beside it unreadable.
    fn claims_graph_accounts(&self) -> Vec<(Pubkey, Pubkey, Vec<u8>)> {
        let fixture = &self.fixture;
        let claims = self.waist.claims_program;
        let mut accounts = vec![
            (fixture.core_market, self.waist.core_program, fixture.core_state.clone()),
            (fixture.claims_market, claims, self.live.market.clone()),
            (
                fixture.actor_position.account,
                claims,
                self.live.dealer_position.clone(),
            ),
            (
                fixture.reserve_position.account,
                claims,
                self.live.counterparty_position.clone(),
            ),
        ];
        for record in [
            &fixture.product,
            &fixture.result_domain,
            &fixture.portfolio,
            &fixture.linked_basis,
            &fixture.exposure,
        ] {
            accounts.push((record.raw, record.owner, record.bytes.clone()));
            accounts.push((record.staging, system_program::ID, Vec::new()));
        }
        accounts
    }

    /// Just the identities, for exclusion from the derived membership frame.
    fn claims_graph_keys(&self) -> Vec<Pubkey> {
        self.claims_graph_accounts()
            .into_iter()
            .map(|(key, _, _)| key)
            .collect()
    }
}

/// Install the whole scenario and the real Trading ELF.
fn program_test(scenario: &Scenario) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_account(
        REQUEST,
        data_account(TRADING, scenario.request_bytes.clone()),
    );
    test.add_account(CHILD_ROOT, data_account(TRADING, vec![0xaa; 64]));
    test.add_account(
        scenario.obligation,
        data_account(TRADING, scenario.obligation_state.clone()),
    );
    test.add_account(
        scenario.membership_manifest,
        data_account(MANIFEST_PRODUCER, scenario.manifest_bytes.clone()),
    );
    test.add_account(BENEFICIARY, data_account(system_program::ID, Vec::new()));
    test.add_account(COUNTERPARTY, data_account(system_program::ID, Vec::new()));
    test.add_account(
        COUNTERPARTY_ACCOUNT,
        data_account(system_program::ID, Vec::new()),
    );
    add_executable(&mut test, MANIFEST_PRODUCER);
    add_executable(&mut test, UNACTIVATED_PRODUCER);
    // The genuine release waist reservation authenticates against. The Custody
    // role is installed as a real loadable deployment and its ProgramData is
    // then written to the exact slot and authority the activation pinned.
    add_executable(&mut test, scenario.waist.registry);
    for (name, program, artifact) in &scenario.waist.deployments {
        test.add_upgradeable_program_to_genesis(name, program);
        test.add_account(
            programdata_address(*program),
            data_account(
                bpf_loader_upgradeable::ID,
                loader_programdata_body(WAIST_SLOT, None, artifact),
            ),
        );
    }
    test.add_account(
        scenario.waist.activation_cache,
        data_account(scenario.waist.registry, scenario.waist.cache_body.clone()),
    );
    // Install the frame exactly as the projection observed it: same identities,
    // same owners, same widths. The pages carry these accounts, so what the
    // campaign pages is the frame itself and not a stand-in for it.
    let mut reserved = vec![
        TRADING,
        REQUEST,
        CHILD_ROOT,
        scenario.obligation,
        scenario.membership_manifest,
        scenario.waist.registry,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
        scenario.waist.claims_program,
        scenario.waist.claims_programdata,
        scenario.waist.core_program,
        scenario.waist.core_programdata,
        scenario.waist.trading_programdata,
        MANIFEST_PRODUCER,
        UNACTIVATED_PRODUCER,
        BENEFICIARY,
        COUNTERPARTY,
        COUNTERPARTY_ACCOUNT,
        scenario.core_market,
    ];
    reserved.extend(scenario.claims_graph_keys());
    for (key, account) in &scenario.frame_accounts {
        if reserved.contains(key) {
            continue;
        }
        test.add_account(*key, account.clone());
    }
    // The Claims aggregate graph, installed exactly as the fixture compiled it.
    for (key, owner, body) in scenario.claims_graph_accounts() {
        if body.is_empty() {
            test.add_account(key, Account {
                lamports: Rent::default().minimum_balance(0).max(1),
                data: Vec::new(),
                owner,
                executable: false,
                rent_epoch: 0,
            });
        } else {
            test.add_account(key, data_account(owner, body));
        }
    }
    test
}

fn add_executable(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(key, Account {
        lamports: 1,
        data: Vec::new(),
        owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: 0,
    });
}

/// Sign and process one route.
///
/// The signer set is route-derived: creation carries a wallet authority beyond
/// the payer, and every other route carries none.
async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signers: &[&Keypair],
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let payer = context.payer.insecure_clone();
    let mut signers: Vec<&Keypair> = vec![&payer];
    signers.extend_from_slice(extra_signers);
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
}

/// Build the creation route for one dealer authority.
fn create_instruction(
    scenario: &Scenario,
    payer: Pubkey,
    dealer_authority: Pubkey,
) -> (Instruction, usize) {
    let packet = build_dealer_scenario_checkpoint_create_v1(
        TRADING,
        payer,
        dealer_authority,
        BENEFICIARY,
        scenario.checkpoint,
        REQUEST,
        CHILD_ROOT,
        scenario.obligation,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        MANIFEST_PRODUCER,
        scenario.membership_manifest,
        Hash::default(),
        &[],
    )
    .expect("create packet");
    assert_eq!(packet.route, DealerScenarioCheckpointRouteV1::Create);
    (
        packet.instruction,
        packet.lock_census.unique_account_lock_count,
    )
}

/// Build one membership page route over an exact observation set.
fn page_instruction(
    scenario: &Scenario,
    payer: Pubkey,
    page_index: u8,
    page: &[Pubkey],
) -> (Instruction, usize) {
    let packet = build_dealer_scenario_checkpoint_page_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        scenario.membership_manifest,
        page_index,
        page,
        Hash::default(),
        &[],
    )
    .expect("page packet");
    (
        packet.instruction,
        packet.lock_census.unique_account_lock_count,
    )
}

/// Read the exact current checkpoint body.
async fn checkpoint_body(context: &mut ProgramTestContext, scenario: &Scenario) -> Vec<u8> {
    context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query")
        .expect("checkpoint exists")
        .data
}

/// Execute creation, which every hostile page case starts from.
async fn create_checkpoint(context: &mut ProgramTestContext, scenario: &Scenario) {
    let payer = context.payer.pubkey();
    let (instruction, _) = create_instruction(scenario, payer, scenario.dealer.pubkey());
    let dealer = scenario.dealer.insecure_clone();
    let processed = submit(context, instruction, &[&dealer])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "checkpoint creation must commit; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
}

#[tokio::test]
async fn real_trading_elf_executes_the_accepted_transition_through_reservation() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let mut journal = DealerScenarioCheckpointJournalV1::planned(TRADING, scenario.request_digest)
        .expect("planned journal");
    assert_eq!(
        journal.checkpoint, scenario.checkpoint,
        "the durable journal and the campaign name one checkpoint"
    );
    // The exact width is a property of this scenario's spans, not a constant,
    // so what is pinned is the thing that matters: the unsplit form is over the
    // ceiling and therefore unsubmittable, and the split carries the same frame.
    assert!(
        scenario.unsplit_account_lock_count > SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "the unsplit form must be unsubmittable, which is the whole reason this campaign exists: \
         observed {} locks against a {SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1}-lock ceiling",
        scenario.unsplit_account_lock_count
    );

    let payer = context.payer.pubkey();
    let (instruction, create_locks) =
        create_instruction(&scenario, payer, scenario.dealer.pubkey());
    assert!(
        create_locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "creation must be lock-bounded"
    );
    let dealer = scenario.dealer.insecure_clone();
    let processed = submit(&mut context, instruction, &[&dealer])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "checkpoint creation must commit; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let created = context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query")
        .expect("checkpoint exists after creation");
    assert_eq!(created.owner, TRADING, "the checkpoint is Trading-owned");
    journal
        .record_created(hash(&created.data).to_bytes())
        .expect("journal records creation");

    let mut peak_locks = create_locks;
    let mut carried = 0_usize;
    for (page_index, page) in scenario.pages.iter().enumerate() {
        let ordinal = u8::try_from(page_index).expect("six pages");
        let (instruction, locks) = page_instruction(&scenario, payer, ordinal, page);
        assert!(
            locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
            "page {page_index} must be lock-bounded"
        );
        peak_locks = peak_locks.max(locks);
        let processed = submit(&mut context, instruction, &[])
            .await
            .expect("ProgramTest processing");
        assert!(
            processed.result.is_ok(),
            "page {page_index} must commit; observed {:?} logs {:?}",
            processed.result,
            processed.metadata.as_ref().map(|value| &value.log_messages)
        );
        let returned = processed
            .metadata
            .as_ref()
            .and_then(|value| value.return_data.as_ref())
            .map(|value| value.data.clone())
            .expect("every page returns its receipt digest");
        let digest = <[u8; 32]>::try_from(returned.as_slice()).expect("32-byte page receipt");
        let observed = checkpoint_body(&mut context, &scenario).await;
        journal
            .record_page(ordinal, digest, hash(&observed).to_bytes())
            .expect("journal records the page it observed");
        carried += page.len();
    }
    assert_eq!(
        usize::from(journal.next_page),
        DEALER_SCENARIO_PREPARATION_PAGES_V1,
        "the whole canonical membership transcript is on chain"
    );
    assert_eq!(
        carried,
        scenario.membership.len(),
        "every member of the derived physical frame was carried exactly once"
    );
    // The evaluator seals what it read. Both prestate digests fold the complete
    // ordered transcript the checkpoint now carries, so this receipt could not
    // have existed before the pages did.
    let read_back = checkpoint_body(&mut context, &scenario).await;
    let evidence = evaluation_evidence(&scenario, &read_back);
    for (key, body) in evidence.installed.iter() {
        context.set_account(key, &AccountSharedData::from(data_account(TRADING, body.clone())));
    }
    let packet = build_dealer_scenario_checkpoint_evaluate_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        TRADING,
        evidence.receipt_address,
        CANDIDATE_BANK,
        CANDIDATE_OBLIGATION,
        CLAIMS_DELTA,
        EFFECTS,
        Hash::default(),
        &[],
    )
    .expect("evaluate packet");
    assert!(
        packet.lock_census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "evaluation must be lock-bounded"
    );
    peak_locks = peak_locks.max(packet.lock_census.unique_account_lock_count);
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "evaluation must seal; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let returned = processed
        .metadata
        .as_ref()
        .and_then(|value| value.return_data.as_ref())
        .map(|value| value.data.clone())
        .expect("evaluation returns its sealed receipt digest");
    let sealed = <[u8; 32]>::try_from(returned.as_slice()).expect("32-byte receipt digest");
    assert_eq!(
        sealed,
        hash(&evidence.receipt_body).to_bytes(),
        "the sealed digest is the receipt body the producer published"
    );
    let after_evaluation = checkpoint_body(&mut context, &scenario).await;
    assert_ne!(
        after_evaluation, read_back,
        "sealing an evaluation advances the checkpoint"
    );
    journal
        .record_evaluated(sealed, 1, hash(&after_evaluation).to_bytes())
        .expect("journal records the evaluation it observed");

    // Custody locks the value the commit will spend. Trading does not call
    // Custody to learn this: it authenticates the Custody-owned receipt out of
    // the activated release set, then re-reads the reservation account and
    // refuses any poststate the receipt did not commit.
    let reservation = reservation_evidence(
        &scenario,
        &after_evaluation,
        evidence.effects_digest,
        evidence.effect_digest,
    );
    for (key, body) in reservation.installed.iter() {
        context.set_account(
            key,
            &AccountSharedData::from(data_account(scenario.waist.custody_program, body.clone())),
        );
    }
    let packet = build_dealer_scenario_checkpoint_reserve_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
        scenario.waist.registry,
        reservation.receipt_address,
        reservation.reservation_state,
        TRADING,
        EFFECTS,
        EFFECT_BODY,
        0,
        Hash::default(),
        &[],
    )
    .expect("reserve packet");
    assert_eq!(
        packet.route,
        DealerScenarioCheckpointRouteV1::Reserve(0),
        "the reservation is the zeroth evaluated effect"
    );
    assert!(
        packet.lock_census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "reservation must be lock-bounded"
    );
    peak_locks = peak_locks.max(packet.lock_census.unique_account_lock_count);
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "the reservation must be ingested; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let after_reservation = checkpoint_body(&mut context, &scenario).await;
    assert_ne!(
        after_reservation, after_evaluation,
        "ingesting a reservation advances the checkpoint"
    );
    let returned = processed
        .metadata
        .as_ref()
        .and_then(|value| value.return_data.as_ref())
        .map(|value| value.data.clone())
        .expect("reservation returns its receipt digest");
    let reserved = <[u8; 32]>::try_from(returned.as_slice()).expect("32-byte receipt digest");
    journal
        .record_reservation(0, reserved, hash(&after_reservation).to_bytes())
        .expect("journal records the reservation it observed");

    assert!(
        peak_locks <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "the executed transcript's peak lock count is {peak_locks}, which must stay inside the \
         64-lock ceiling the unsplit 121-account instruction cannot meet"
    );
}

/// Producer-owned evaluation evidence for one checkpoint body.
struct EvaluationEvidence {
    receipt_address: Pubkey,
    receipt_body: Vec<u8>,
    effects_digest: [u8; 32],
    effect_digest: [u8; 32],
    installed: Vec<(Pubkey, Vec<u8>)>,
}

/// Publish the evaluator's bodies and derive the receipt Trading will accept.
///
/// Only the manifest is a typed protocol body here: the candidate bank, the
/// candidate obligation and the Claims delta are authenticated at this route
/// purely by the digests the receipt commits, and the effect bodies themselves
/// are authenticated later, at reservation.
fn evaluation_evidence(scenario: &Scenario, checkpoint_body: &[u8]) -> EvaluationEvidence {
    evaluation_evidence_with_delta(scenario, checkpoint_body, None)
}

/// The same evidence, optionally over a delta the evaluator chose to publish.
fn evaluation_evidence_with_delta(
    scenario: &Scenario,
    checkpoint_body: &[u8],
    published_delta: Option<Vec<u8>>,
) -> EvaluationEvidence {
    let effect_body = DealerScenarioCustodyEffectV1 {
        kind: DealerScenarioCustodyRequestKindV1::Canonical,
        ordinal: 0,
        effect_count: 1,
        producer_program: TRADING.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        source_after: 90,
        destination_after: 110,
        request_payload: [0_u8; DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1],
    }
    .encode()
    .expect("canonical effect body encodes")
    .to_vec();
    let manifest = DealerScenarioCustodyEffectManifestV1 {
        effect_count: 1,
        producer_program: TRADING.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        effect_accounts: core::array::from_fn(|index| {
            if index == 0 {
                EFFECT_BODY.to_bytes()
            } else {
                [0_u8; 32]
            }
        }),
        effect_digests: core::array::from_fn(|index| {
            if index == 0 {
                hash(&effect_body).to_bytes()
            } else {
                [0_u8; 32]
            }
        }),
    };
    assert_eq!(
        DEALER_SCENARIO_MAX_RESERVATIONS_V1, 4,
        "the manifest carries a fixed reservation width"
    );
    let effects_body = manifest.encode().expect("effect manifest encodes").to_vec();
    let candidate_bank = vec![0xc1; 64];
    let candidate_obligation = scenario.candidate_obligation_bytes.clone();
    // Commit requires this body to be byte-identical to the SignedDelta packet
    // carried inside the request, so the evaluator cannot promise Claims one
    // delta and publish another.
    let claims_delta = published_delta.unwrap_or_else(|| {
        DealerScenarioTradeRequestV3::decode(&scenario.request_bytes)
            .expect("canonical request decodes")
            .claims_packet()
            .to_vec()
    });
    let receipt = derive_dealer_scenario_evaluation_receipt_v1(
        TRADING,
        scenario.checkpoint,
        checkpoint_body,
        DealerScenarioEvaluationBodiesV1 {
            candidate_bank: &candidate_bank,
            candidate_obligation: &candidate_obligation,
            claims_delta: &claims_delta,
            effects: &effects_body,
        },
        1,
    )
    .expect("producer derives its receipt from the checkpoint it read");
    let receipt_body = receipt.encode().expect("receipt encodes").to_vec();
    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    EvaluationEvidence {
        receipt_address,
        receipt_body: receipt_body.clone(),
        effects_digest: hash(&effects_body).to_bytes(),
        effect_digest: hash(&effect_body).to_bytes(),
        installed: vec![
            (receipt_address, receipt_body),
            (CANDIDATE_BANK, candidate_bank),
            (CANDIDATE_OBLIGATION, candidate_obligation),
            (CLAIMS_DELTA, claims_delta),
            (EFFECTS, effects_body),
            (EFFECT_BODY, effect_body),
        ],
    }
}

#[tokio::test]
async fn a_substituted_membership_member_refuses_and_the_checkpoint_does_not_advance() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let before = checkpoint_body(&mut context, &scenario).await;

    // Same page ordinal, same width, one substituted member. The manifest
    // committed this page's digest at creation, so the substitution cannot pass.
    let mut substituted = scenario.pages.first().expect("page zero").clone();
    let replacement = *scenario
        .pages
        .get(1)
        .and_then(|page| page.last())
        .expect("page one is not empty");
    *substituted.last_mut().expect("page zero is not empty") = replacement;
    let payer = context.payer.pubkey();
    let (instruction, _) = page_instruction(&scenario, payer, 0, &substituted);
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a substituted member must refuse on content; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused page must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_wrong_dealer_authority_cannot_create_the_checkpoint() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;

    // A real signature from the wrong wallet. The request names its dealer
    // owner, so signing is necessary and never sufficient.
    let impostor = Keypair::new();
    let payer = context.payer.pubkey();
    let (instruction, _) = create_instruction(&scenario, payer, impostor.pubkey());
    let processed = submit(&mut context, instruction, &[&impostor])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a foreign dealer authority must refuse on content; observed {:?}",
        processed.result
    );
    assert!(
        context
            .banks_client
            .get_account(scenario.checkpoint)
            .await
            .expect("checkpoint query")
            .is_none(),
        "a refused creation must leave the checkpoint address vacant"
    );
}

#[tokio::test]
async fn a_malformed_membership_manifest_refuses_every_page() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let before = checkpoint_body(&mut context, &scenario).await;

    // The manifest keeps its PDA, its owner, its width and its structural
    // validity; only the order of two committed page digests changes. What
    // refuses is the body the checkpoint bound at creation, nothing shallower.
    let mut manifest = DealerScenarioMembershipManifestV1::decode(&scenario.manifest_bytes)
        .expect("canonical manifest decodes");
    manifest.page_membership_digests.swap(4, 5);
    let substituted = manifest.encode().expect("substituted manifest encodes");
    assert_ne!(
        substituted.as_slice(),
        scenario.manifest_bytes.as_slice(),
        "the substitution must actually change the body"
    );
    context.set_account(
        &scenario.membership_manifest,
        &AccountSharedData::from(data_account(MANIFEST_PRODUCER, substituted.to_vec())),
    );

    let payer = context.payer.pubkey();
    let (instruction, _) =
        page_instruction(&scenario, payer, 0, scenario.pages.first().expect("page zero"));
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_CONTENT),
        "a substituted manifest body must refuse on content; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused page must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_replayed_page_ordinal_refuses_after_it_already_committed() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let page = scenario.pages.first().expect("page zero");

    let (instruction, _) = page_instruction(&scenario, payer, 0, page);
    submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing")
        .result
        .expect("the first page must commit");
    let after_first = checkpoint_body(&mut context, &scenario).await;

    // Byte-identical replay of a page the checkpoint already carries.
    let (instruction, _) = page_instruction(&scenario, payer, 0, page);
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_err(),
        "a replayed page ordinal must fail closed; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        after_first,
        "a refused replay must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_substituted_candidate_body_cannot_seal_an_evaluation() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    create_checkpoint(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    for (page_index, page) in scenario.pages.iter().enumerate() {
        let ordinal = u8::try_from(page_index).expect("six pages");
        let (instruction, _) = page_instruction(&scenario, payer, ordinal, page);
        submit(&mut context, instruction, &[])
            .await
            .expect("ProgramTest processing")
            .result
            .expect("every page must commit before evaluation");
    }
    let read_back = checkpoint_body(&mut context, &scenario).await;
    let evidence = evaluation_evidence(&scenario, &read_back);
    for (key, body) in evidence.installed.iter() {
        context.set_account(key, &AccountSharedData::from(data_account(TRADING, body.clone())));
    }

    // The receipt is the one the producer really derived. What changes is the
    // candidate bank it promised: same account, same owner, same width, a
    // different body -- so nothing shallower than the digest can answer it.
    context.set_account(
        &CANDIDATE_BANK,
        &AccountSharedData::from(data_account(TRADING, vec![0xc4; 64])),
    );
    let packet = build_dealer_scenario_checkpoint_evaluate_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        TRADING,
        evidence.receipt_address,
        CANDIDATE_BANK,
        CANDIDATE_OBLIGATION,
        CLAIMS_DELTA,
        EFFECTS,
        Hash::default(),
        &[],
    )
    .expect("evaluate packet");
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "a candidate body the receipt did not commit must refuse; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        read_back,
        "a refused evaluation must not advance the checkpoint"
    );
}

/// Extract the exact program refusal code from a processed transaction.
fn custom_code(result: &Result<(), TransactionError>) -> Option<u32> {
    match result {
        Err(TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}

/// The exact Claims SignedDelta frame the final commit carries.
///
/// Every privilege and every role comes from the Claims frame specification;
/// the campaign only supplies identities, and the one identity it is not free
/// to choose is the caller program.
fn claims_frame(trading_program: Pubkey) -> Vec<AccountMeta> {
    let spec = SignedDeltaFrameSpecV3::new(2).expect("two-position Claims frame");
    let count = spec.account_count().expect("frame width");
    (0..count)
        .map(|index| {
            let account = spec.account(index).expect("frame account");
            let key = if matches!(account.role(), ClaimsFrameRoleV1::CallerProgram) {
                trading_program
            } else {
                let mut seed = [0_u8; 32];
                seed[0] = 0x5c;
                seed[1] = u8::try_from(index).expect("small frame index");
                Pubkey::new_from_array(seed)
            };
            if account.privileges().writable() {
                AccountMeta::new(key, false)
            } else {
                AccountMeta::new_readonly(key, false)
            }
        })
        .collect()
}

/// A distinct campaign-owned identity for a route the transcript names.
fn named(tag: u8) -> Pubkey {
    let mut seed = [0_u8; 32];
    seed[0] = 0x6d;
    seed[1] = tag;
    Pubkey::new_from_array(seed)
}

/// The address table a caller resolves the wide routes through.
///
/// The transition does not fit the 1,232-byte packet ceiling as static
/// addresses, which is a fact about the transition and not about this harness:
/// the operator refuses to emit a transcript whose legs cannot be sent.
fn lookup_table(scenario: &Scenario, commit: &DealerScenarioCommitAccountsV1) -> OperatorLookupTable {
    let mut addresses = scenario.membership.clone();
    addresses.extend(commit.claims_accounts.iter().map(|meta| meta.pubkey));
    addresses.extend([
        scenario.membership_manifest,
        commit.evaluation_receipt,
        CANDIDATE_BANK,
        CANDIDATE_OBLIGATION,
        CLAIMS_DELTA,
        EFFECTS,
        EFFECT_BODY,
        MANIFEST_PRODUCER,
        BENEFICIARY,
        commit.custody_program,
        commit.custody_programdata,
        commit.batch,
        named(3),
        named(4),
        named(5),
        named(6),
    ]);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    OperatorLookupTable {
        key: Pubkey::new_from_array([0x7b; 32]),
        addresses,
    }
}

/// Build the complete accepted-transition transcript for this scenario.
fn transcript(
    scenario: &Scenario,
    payer: Pubkey,
    receipt_address: Pubkey,
) -> dclutch_operator::dealer_scenario_checkpoint_v1::DealerAcceptedTranscriptV4 {
    let reservations = [DealerAcceptedReservationAccountsV4 {
        custody_program: named(1),
        custody_programdata: named(2),
        activation_cache: named(3),
        registry_program: named(4),
        reservation_receipt: named(5),
        reservation_state: named(6),
        effect_producer: TRADING,
        effect_manifest: EFFECTS,
        effect_body: EFFECT_BODY,
    }];
    let commit = DealerScenarioCommitAccountsV1 {
        trading_program: TRADING,
        payer,
        checkpoint: scenario.checkpoint,
        clock: sysvar::clock::ID,
        request: REQUEST,
        evaluation_receipt: receipt_address,
        candidate_bank: CANDIDATE_BANK,
        candidate_obligation: CANDIDATE_OBLIGATION,
        claims_delta: CLAIMS_DELTA,
        effects: EFFECTS,
        root: CHILD_ROOT,
        obligation: scenario.obligation,
        custody_program: named(1),
        custody_programdata: named(2),
        batch: named(7),
        claims_accounts: claims_frame(TRADING),
        effect_accounts: core::array::from_fn(|index| {
            if index == 0 {
                DealerScenarioCommitEffectAccountsV1 {
                    reservation_receipt: named(5),
                    reservation_state: named(6),
                }
            } else {
                DealerScenarioCommitEffectAccountsV1::default()
            }
        }),
        effect_count: 1,
    };
    let tables = [lookup_table(scenario, &commit)];
    build_dealer_accepted_transcript_v4(DealerAcceptedTranscriptInputV4 {
        trading_program: TRADING,
        payer,
        dealer_authority: scenario.dealer.pubkey(),
        refund_beneficiary: BENEFICIARY,
        request: REQUEST,
        request_digest: scenario.request_digest,
        root: CHILD_ROOT,
        obligation: scenario.obligation,
        clock: sysvar::clock::ID,
        rent: sysvar::rent::ID,
        system_program: system_program::ID,
        manifest_producer: MANIFEST_PRODUCER,
        membership_manifest: scenario.membership_manifest,
        pages: &scenario.pages,
        evaluation: DealerAcceptedEvaluationAccountsV4 {
            producer_program: TRADING,
            evaluation_receipt: receipt_address,
            candidate_bank: CANDIDATE_BANK,
            candidate_obligation: CANDIDATE_OBLIGATION,
            claims_delta: CLAIMS_DELTA,
            effects: EFFECTS,
        },
        reservations: &reservations,
        commit: &commit,
        recent_blockhash: Hash::default(),
        lookup_tables: &tables,
    })
    .expect("the whole accepted transition is one lock-bounded transcript")
}

#[tokio::test]
async fn the_accepted_transcript_is_ordered_and_wholly_lock_bounded() {
    let scenario = scenario();
    let payer = Pubkey::new_from_array([0x7a; 32]);
    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    let transcript = transcript(&scenario, payer, receipt_address);
    assert_eq!(transcript.checkpoint, scenario.checkpoint);
    assert_eq!(
        transcript.routes(),
        vec![
            DealerScenarioCheckpointRouteV1::Create,
            DealerScenarioCheckpointRouteV1::Page(0),
            DealerScenarioCheckpointRouteV1::Page(1),
            DealerScenarioCheckpointRouteV1::Page(2),
            DealerScenarioCheckpointRouteV1::Page(3),
            DealerScenarioCheckpointRouteV1::Page(4),
            DealerScenarioCheckpointRouteV1::Page(5),
            DealerScenarioCheckpointRouteV1::Evaluate,
            DealerScenarioCheckpointRouteV1::Reserve(0),
            DealerScenarioCheckpointRouteV1::Commit,
        ],
        "the accepted transition has exactly one canonical route order"
    );
    assert!(
        transcript.peak_account_lock_count() <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "the whole transition fits the ceiling the unsplit 121-lock instruction cannot: peak {}",
        transcript.peak_account_lock_count()
    );
    assert_eq!(
        transcript.journal.next_page, 0,
        "the transcript hands back a journal positioned before its first submission"
    );
}


/// Drive creation, the whole membership transcript, and the sealed evaluation.
///
/// This is the same route order the executed campaign pins; the cases below use
/// it to reach a prepared-but-uncommitted checkpoint, which is the state the
/// abandonment path exists for.
async fn prepare_through_evaluation(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> DealerScenarioCheckpointJournalV1 {
    prepare_through_evaluation_with_delta(context, scenario, None).await
}

/// The same, over a delta the evaluator chose to publish and seal.
async fn prepare_through_evaluation_with_delta(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    published_delta: Option<Vec<u8>>,
) -> DealerScenarioCheckpointJournalV1 {
    let mut journal = DealerScenarioCheckpointJournalV1::planned(TRADING, scenario.request_digest)
        .expect("planned journal");
    create_checkpoint(context, scenario).await;
    journal
        .record_created(hash(&checkpoint_body(context, scenario).await).to_bytes())
        .expect("journal records creation");
    let payer = context.payer.pubkey();
    for (page_index, page) in scenario.pages.iter().enumerate() {
        let ordinal = u8::try_from(page_index).expect("six pages");
        let (instruction, _) = page_instruction(scenario, payer, ordinal, page);
        let processed = submit(context, instruction, &[])
            .await
            .expect("ProgramTest processing");
        processed.result.expect("every page must commit");
        let returned = processed
            .metadata
            .as_ref()
            .and_then(|value| value.return_data.as_ref())
            .map(|value| value.data.clone())
            .expect("page receipt digest");
        let digest = <[u8; 32]>::try_from(returned.as_slice()).expect("32-byte page receipt");
        journal
            .record_page(
                ordinal,
                digest,
                hash(&checkpoint_body(context, scenario).await).to_bytes(),
            )
            .expect("journal records the page");
    }
    let read_back = checkpoint_body(context, scenario).await;
    let evidence = evaluation_evidence_with_delta(scenario, &read_back, published_delta);
    for (key, body) in evidence.installed.iter() {
        context.set_account(key, &AccountSharedData::from(data_account(TRADING, body.clone())));
    }
    let packet = build_dealer_scenario_checkpoint_evaluate_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        TRADING,
        evidence.receipt_address,
        CANDIDATE_BANK,
        CANDIDATE_OBLIGATION,
        CLAIMS_DELTA,
        EFFECTS,
        Hash::default(),
        &[],
    )
    .expect("evaluate packet");
    let processed = submit(context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    processed.result.expect("evaluation must seal");
    journal
        .record_evaluated(
            hash(&evidence.receipt_body).to_bytes(),
            1,
            hash(&checkpoint_body(context, scenario).await).to_bytes(),
        )
        .expect("journal records the evaluation");
    journal
}

#[tokio::test]
async fn an_expired_uncommitted_checkpoint_closes_to_its_immutable_beneficiary() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let mut journal = prepare_through_evaluation(&mut context, &scenario).await;
    let payer = context.payer.pubkey();

    let rent_held = context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query")
        .expect("checkpoint exists")
        .lamports;
    assert!(rent_held > 0, "the checkpoint holds the rent it must return");
    let beneficiary_before = context
        .banks_client
        .get_account(BENEFICIARY)
        .await
        .expect("beneficiary query")
        .expect("beneficiary exists")
        .lamports;

    // Abandonment is a time transition, not an authority: nobody signs for it,
    // and the rent can only ever go to the beneficiary named at creation.
    context
        .warp_to_slot(SCENARIO_EXPIRES_AT + 8)
        .expect("warp past expiry");
    let packet = build_dealer_scenario_checkpoint_cleanup_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        BENEFICIARY,
        sysvar::clock::ID,
        Hash::default(),
        &[],
    )
    .expect("cleanup packet");
    assert_eq!(packet.route, DealerScenarioCheckpointRouteV1::Cleanup);
    assert!(
        packet.lock_census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "cleanup must be lock-bounded"
    );
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert!(
        processed.result.is_ok(),
        "an expired uncommitted checkpoint must close; observed {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );

    let closed = context
        .banks_client
        .get_account(scenario.checkpoint)
        .await
        .expect("checkpoint query");
    assert!(
        closed.is_none_or(|account| account.data.is_empty() && account.lamports == 0),
        "a cleaned checkpoint keeps neither state nor rent"
    );
    let beneficiary_after = context
        .banks_client
        .get_account(BENEFICIARY)
        .await
        .expect("beneficiary query")
        .expect("beneficiary exists")
        .lamports;
    assert_eq!(
        beneficiary_after,
        beneficiary_before + rent_held,
        "every lamport the checkpoint held reaches the immutable beneficiary"
    );
    journal.record_cleaned().expect("journal records the close");
}

#[tokio::test]
async fn a_checkpoint_cannot_be_closed_before_it_expires() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    prepare_through_evaluation(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let before = checkpoint_body(&mut context, &scenario).await;

    let packet = build_dealer_scenario_checkpoint_cleanup_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        BENEFICIARY,
        sysvar::clock::ID,
        Hash::default(),
        &[],
    )
    .expect("cleanup packet");
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "closing a live checkpoint must refuse on transition; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused close must leave the checkpoint intact"
    );
}

#[tokio::test]
async fn an_expired_checkpoint_refuses_a_substituted_rent_beneficiary() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    prepare_through_evaluation(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    context
        .warp_to_slot(SCENARIO_EXPIRES_AT + 8)
        .expect("warp past expiry");
    let before = checkpoint_body(&mut context, &scenario).await;

    // Expiry is reached, so the only thing left to get wrong is where the rent
    // goes. The beneficiary was fixed at creation and is not a caller choice.
    let thief = Pubkey::new_from_array([0x9e; 32]);
    let packet = build_dealer_scenario_checkpoint_cleanup_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        thief,
        sysvar::clock::ID,
        Hash::default(),
        &[],
    )
    .expect("cleanup packet");
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_COMMIT),
        "a substituted rent beneficiary must refuse; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused close must leave the checkpoint intact"
    );
}


/// Custody-owned reservation evidence for one evaluated effect.
struct ReservationEvidence {
    receipt_address: Pubkey,
    reservation_state: Pubkey,
    batch: Pubkey,
    installed: Vec<(Pubkey, Vec<u8>)>,
}

/// Publish the Custody reservation receipt, state and locked batch.
///
/// These are real protocol bodies, not opaque blobs: reservation ingests the
/// receipt, and commit later decodes the state and the batch and cross-checks
/// all three against the checkpoint. The order is forced by what commits to
/// what -- the state names the batch, the receipt commits the state's poststate,
/// and the batch records the receipt's digest -- which is exactly why the state
/// cannot carry its own receipt digest.
fn reservation_evidence(
    scenario: &Scenario,
    checkpoint_body: &[u8],
    effects_digest: [u8; 32],
    effect_digest: [u8; 32],
) -> ReservationEvidence {
    let custody = scenario.waist.custody_program;
    let batch = dealer_scenario_reservation_batch_address_v1(custody, scenario.checkpoint);
    let reservation_state = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
            scenario.checkpoint.as_ref(),
            &[0],
        ],
        &custody,
    )
    .0;
    let state_body = DealerScenarioReservationStateV1 {
        status: DealerScenarioReservationStateStatusV1::Active,
        ordinal: 0,
        effect_count: 1,
        batch: batch.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        effects_digest,
        effect_digest,
        source: [0xf3; 32],
        destination: [0xf4; 32],
        escrow: [0xf5; 32],
        mint: [0xf6; 32],
        token_program: [0xf7; 32],
        source_prestate_digest: [0xf8; 32],
        destination_prestate_digest: [0xf9; 32],
        effect_poststate_digest: [0xfa; 32],
        source_poststate_digest: [0xfb; 32],
        amount: 10,
        source_after: 90,
        destination_before: 100,
        escrow_after: 10,
    }
    .encode()
    .expect("reservation state encodes")
    .to_vec();
    let receipt = DealerScenarioReservationReceiptV1 {
        action: DealerScenarioReservationActionV1::Reserve,
        effect_ordinal: 0,
        effect_count: 1,
        producer_program: custody.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        checkpoint_prestate_digest: hash(checkpoint_body).to_bytes(),
        request_digest: scenario.request_digest,
        effects_digest,
        reservation: reservation_state.to_bytes(),
        reservation_prestate_digest: hash(&[] as &[u8]).to_bytes(),
        reservation_poststate_digest: hash(&state_body).to_bytes(),
        prior_receipt_digest: [0_u8; 32],
    };
    let receipt_address = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            scenario.checkpoint.as_ref(),
            &scenario.request_digest,
            &[DealerScenarioReservationActionV1::Reserve as u8],
            &[0],
        ],
        &custody,
    )
    .0;
    let receipt_body = receipt.encode().expect("reservation receipt encodes").to_vec();
    let batch_body = DealerScenarioReservationBatchV1 {
        status: DealerScenarioReservationBatchStatusV1::Reserved,
        effect_count: 1,
        reserved_count: 1,
        rollback_count: 0,
        release_set: scenario.waist.release_set_id,
        market: scenario.core_market.to_bytes(),
        realm: SCENARIO_REALM,
        trading_program: TRADING.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        effects_digest,
        replay: [0xfc; 32],
        replay_prestate_digest: [0xfd; 32],
        refund_beneficiary: BENEFICIARY.to_bytes(),
        expires_at: SCENARIO_EXPIRES_AT,
        generation: SCENARIO_GENERATION,
        reservation_states: core::array::from_fn(|index| {
            if index == 0 {
                reservation_state.to_bytes()
            } else {
                [0_u8; 32]
            }
        }),
        receipt_digests: core::array::from_fn(|index| {
            if index == 0 {
                hash(&receipt_body).to_bytes()
            } else {
                [0_u8; 32]
            }
        }),
        last_prestate_digest: [0xfe; 32],
    }
    .encode()
    .expect("reservation batch encodes")
    .to_vec();
    ReservationEvidence {
        receipt_address,
        reservation_state,
        batch,
        installed: vec![
            (receipt_address, receipt_body),
            (reservation_state, state_body),
            (batch, batch_body),
        ],
    }
}

/// Build one reservation route over a caller-supplied release waist.
///
/// The producer and cache are parameters so a case can present a waist that is
/// individually well-formed and still wrong for this checkpoint.
fn reserve_instruction(
    scenario: &Scenario,
    payer: Pubkey,
    reservation: &ReservationEvidence,
    producer: Pubkey,
    programdata: Pubkey,
    activation_cache: Pubkey,
) -> Instruction {
    build_dealer_scenario_checkpoint_reserve_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        sysvar::clock::ID,
        producer,
        programdata,
        activation_cache,
        scenario.waist.registry,
        reservation.receipt_address,
        reservation.reservation_state,
        TRADING,
        EFFECTS,
        EFFECT_BODY,
        0,
        Hash::default(),
        &[],
    )
    .expect("reserve packet")
    .instruction
}

/// Reach a sealed evaluation and publish the Custody reservation evidence.
async fn evaluated_with_reservation_evidence(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> (ReservationEvidence, Vec<u8>, DealerScenarioCheckpointJournalV1) {
    evaluated_with_published_delta(context, scenario, None).await
}

/// The same, over a delta the evaluator chose to publish and seal.
async fn evaluated_with_published_delta(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    published_delta: Option<Vec<u8>>,
) -> (ReservationEvidence, Vec<u8>, DealerScenarioCheckpointJournalV1) {
    let mut journal =
        prepare_through_evaluation_with_delta(context, scenario, published_delta.clone()).await;
    let after_evaluation = checkpoint_body(context, scenario).await;
    let evidence = evaluation_evidence_with_delta(scenario, &after_evaluation, published_delta);
    let reservation = reservation_evidence(
        scenario,
        &after_evaluation,
        evidence.effects_digest,
        evidence.effect_digest,
    );
    for (key, body) in reservation.installed.iter() {
        context.set_account(
            key,
            &AccountSharedData::from(data_account(scenario.waist.custody_program, body.clone())),
        );
    }
    (reservation, after_evaluation, journal)
}

#[tokio::test]
async fn a_reservation_from_an_unactivated_producer_refuses_on_the_release_waist() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (reservation, before, _) =
        evaluated_with_reservation_evidence(&mut context, &scenario).await;
    let payer = context.payer.pubkey();

    // A real executable program that the activated release set never named as
    // Custody. Being a program is not the same as holding the role.
    //
    // The receipt and reservation move to that program too, so this reaches the
    // release waist instead of stopping at the ownership frame: the point of the
    // case is that a coherent Custody-shaped story still cannot buy the role.
    for (key, body) in reservation.installed.iter() {
        context.set_account(
            key,
            &AccountSharedData::from(data_account(UNACTIVATED_PRODUCER, body.clone())),
        );
    }
    let instruction = reserve_instruction(
        &scenario,
        payer,
        &reservation,
        UNACTIVATED_PRODUCER,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
    );
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_RELEASE),
        "an unactivated producer must refuse on the release waist; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused reservation must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_valid_activation_cache_for_another_release_set_refuses() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (reservation, before, _) =
        evaluated_with_reservation_evidence(&mut context, &scenario).await;
    let payer = context.payer.pubkey();

    // A perfectly well-formed activation cache -- correct owner, correct width,
    // every role activated -- belonging to a different release generation, and
    // written at this generation's address. The header is the thing that must
    // refuse it, not the shape.
    let other = release_waist_for(Pubkey::new_from_array([0xf1; 32]));
    assert_ne!(
        other.release_set_id, scenario.waist.release_set_id,
        "the substituted cache must belong to another generation"
    );
    context.set_account(
        &scenario.waist.activation_cache,
        &AccountSharedData::from(data_account(scenario.waist.registry, other.cache_body)),
    );
    let instruction = reserve_instruction(
        &scenario,
        payer,
        &reservation,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
    );
    let processed = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_RELEASE),
        "a cross-generation cache must refuse on the release waist; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused reservation must not advance the checkpoint"
    );
}

#[tokio::test]
async fn locked_value_blocks_the_abandonment_path_until_it_is_rolled_back() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (reservation, _, _) =
        evaluated_with_reservation_evidence(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let instruction = reserve_instruction(
        &scenario,
        payer,
        &reservation,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
    );
    submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing")
        .result
        .expect("the reservation must be ingested");
    let reserved = checkpoint_body(&mut context, &scenario).await;

    // Expiry alone must not release a checkpoint that is still holding locked
    // Custody value: the rent is not the caller's to reclaim while a
    // reservation stands unrolled-back.
    context
        .warp_to_slot(SCENARIO_EXPIRES_AT + 8)
        .expect("warp past expiry");
    let packet = build_dealer_scenario_checkpoint_cleanup_v1(
        TRADING,
        payer,
        scenario.checkpoint,
        BENEFICIARY,
        sysvar::clock::ID,
        Hash::default(),
        &[],
    )
    .expect("cleanup packet");
    let processed = submit(&mut context, packet.instruction, &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "an expired checkpoint holding a reservation must refuse to close; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        reserved,
        "a refused close must leave the reserved checkpoint intact"
    );
}


#[tokio::test]
async fn a_commit_refuses_before_any_value_is_locked() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    prepare_through_evaluation(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let before = checkpoint_body(&mut context, &scenario).await;

    // The checkpoint is evaluated but nothing has been reserved. Commit is the
    // route that spends locked value, so the phase is the first thing it reads
    // and the whole of the rest of its frame never gets a say.
    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    let transcript = transcript(&scenario, payer, receipt_address);
    let commit = transcript
        .packets
        .last()
        .expect("the transcript ends in its commit");
    assert_eq!(commit.route, DealerScenarioCheckpointRouteV1::Commit);
    let processed = submit(&mut context, commit.instruction.clone(), &[])
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "committing an unreserved checkpoint must refuse on transition; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        before,
        "a refused commit must not advance the checkpoint"
    );
}


/// Derive the request-scoped Trading caller authority the Claims frame requires.
///
/// Commit refuses unless the frame's first account is exactly this PDA, so the
/// authority is a consequence of the request rather than a caller's choice.
fn claims_caller_authority(scenario: &Scenario) -> Pubkey {
    let request = DealerScenarioTradeRequestV3::decode(&scenario.request_bytes)
        .expect("canonical request decodes");
    let plan = SignedDeltaPlanV3::decode(request.claims_packet()).expect("claims plan decodes");
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).expect("release set"),
        plan.market(),
        ExecutionRoleV1::Trading,
        plan.request_id(),
        hash(request.claims_packet()).to_bytes(),
    )
    .expect("caller authority seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &TRADING).0
}

/// The exact Claims SignedDelta frame this scenario commits through.
///
/// Roles the campaign can already satisfy for real are bound to their real
/// accounts: the request-scoped caller authority, the rent sysvar, and every
/// release-waist coordinate. The Claims aggregate, the Core Market and the four
/// finalized record pairs remain campaign identities until the Product graph is
/// staged, which is the named next wall.
fn commit_claims_frame(scenario: &Scenario) -> Vec<AccountMeta> {
    let spec = SignedDeltaFrameSpecV3::new(2).expect("two-position Claims frame");
    let count = spec.account_count().expect("frame width");
    let authority = claims_caller_authority(scenario);
    (0..count)
        .map(|index| {
            let account = spec.account(index).expect("frame account");
            let fixture = &scenario.fixture;
            let ordered = fixture.ordered_positions();
            let key = match account.role() {
                ClaimsFrameRoleV1::CallerAuthority => authority,
                ClaimsFrameRoleV1::ClaimsMarket => fixture.claims_market,
                ClaimsFrameRoleV1::BasisRecord => fixture.linked_basis.raw,
                ClaimsFrameRoleV1::BasisStaging => fixture.linked_basis.staging,
                ClaimsFrameRoleV1::ProductRecord => fixture.product.raw,
                ClaimsFrameRoleV1::ProductStaging => fixture.product.staging,
                ClaimsFrameRoleV1::ResultDomainRecord => fixture.result_domain.raw,
                ClaimsFrameRoleV1::ResultDomainStaging => fixture.result_domain.staging,
                ClaimsFrameRoleV1::PortfolioRecord => fixture.portfolio.raw,
                ClaimsFrameRoleV1::PortfolioStaging => fixture.portfolio.staging,
                ClaimsFrameRoleV1::RentSysvar => sysvar::rent::ID,
                ClaimsFrameRoleV1::CoreMarket => fixture.core_market,
                ClaimsFrameRoleV1::ActivationCache => scenario.waist.activation_cache,
                ClaimsFrameRoleV1::RegistryProgram => scenario.waist.registry,
                ClaimsFrameRoleV1::CallerProgram => TRADING,
                ClaimsFrameRoleV1::CallerProgramData => scenario.waist.trading_programdata,
                ClaimsFrameRoleV1::ClaimsProgram => scenario.waist.claims_program,
                ClaimsFrameRoleV1::ClaimsProgramData => scenario.waist.claims_programdata,
                ClaimsFrameRoleV1::CoreProgram => scenario.waist.core_program,
                ClaimsFrameRoleV1::CoreProgramData => scenario.waist.core_programdata,
                ClaimsFrameRoleV1::SignedDeltaPosition(position) => {
                    ordered
                        .get(usize::from(position))
                        .expect("the frame names exactly the Position table")
                        .account
                }
                other => panic!("the signed-delta frame named an unexpected role: {other:?}"),
            };
            if account.privileges().writable() {
                AccountMeta::new(key, false)
            } else {
                AccountMeta::new_readonly(key, false)
            }
        })
        .collect()
}

/// The complete commit bank for this scenario's locked value.
fn commit_bank(
    scenario: &Scenario,
    payer: Pubkey,
    receipt_address: Pubkey,
    reservation: &ReservationEvidence,
) -> DealerScenarioCommitAccountsV1 {
    DealerScenarioCommitAccountsV1 {
        trading_program: TRADING,
        payer,
        checkpoint: scenario.checkpoint,
        clock: sysvar::clock::ID,
        request: REQUEST,
        evaluation_receipt: receipt_address,
        candidate_bank: CANDIDATE_BANK,
        candidate_obligation: CANDIDATE_OBLIGATION,
        claims_delta: CLAIMS_DELTA,
        effects: EFFECTS,
        root: CHILD_ROOT,
        obligation: scenario.obligation,
        custody_program: scenario.waist.custody_program,
        custody_programdata: scenario.waist.custody_programdata,
        batch: reservation.batch,
        claims_accounts: commit_claims_frame(scenario),
        effect_accounts: core::array::from_fn(|index| {
            if index == 0 {
                DealerScenarioCommitEffectAccountsV1 {
                    reservation_receipt: reservation.receipt_address,
                    reservation_state: reservation.reservation_state,
                }
            } else {
                DealerScenarioCommitEffectAccountsV1::default()
            }
        }),
        effect_count: 1,
    }
}

#[tokio::test]
async fn the_commit_lands_the_signed_delta_and_moves_the_claims_positions() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (reservation, after_evaluation, mut journal) =
        evaluated_with_reservation_evidence(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let instruction = reserve_instruction(
        &scenario,
        payer,
        &reservation,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
    );
    let ingested = submit(&mut context, instruction, &[])
        .await
        .expect("ProgramTest processing");
    ingested
        .result
        .as_ref()
        .expect("the reservation must be ingested");
    let reserved = checkpoint_body(&mut context, &scenario).await;
    assert_ne!(reserved, after_evaluation, "the checkpoint is now reserved");
    let reserved_receipt = ingested
        .metadata
        .as_ref()
        .and_then(|value| value.return_data.as_ref())
        .map(|value| value.data.clone())
        .expect("reservation returns its receipt digest");
    journal
        .record_reservation(
            0,
            <[u8; 32]>::try_from(reserved_receipt.as_slice()).expect("32-byte receipt digest"),
            hash(&reserved).to_bytes(),
        )
        .expect("journal records the reservation it observed");

    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    let bank = commit_bank(&scenario, payer, receipt_address, &reservation);
    // Everything the commit names except the fee payer and the invoked program
    // resolves through the table, which is the only shape that fits a packet.
    let mut addresses = bank
        .claims_accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    addresses.extend([
        scenario.checkpoint,
        sysvar::clock::ID,
        REQUEST,
        receipt_address,
        CANDIDATE_BANK,
        CANDIDATE_OBLIGATION,
        CLAIMS_DELTA,
        EFFECTS,
        CHILD_ROOT,
        scenario.obligation,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        reservation.batch,
        reservation.receipt_address,
        reservation.reservation_state,
    ]);
    addresses.retain(|key| *key != payer && *key != TRADING);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    let packet = build_dealer_scenario_commit_v1(
        bank,
        Hash::default(),
        &[OperatorLookupTable {
            key: Pubkey::new_from_array([0x7b; 32]),
            addresses: addresses.clone(),
        }],
    )
    .expect("commit packet");
    assert!(
        packet.lock_census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "commit must be lock-bounded"
    );
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let processed = submit_v0(&mut context, packet.instruction, table, &addresses)
        .await
        .expect("ProgramTest processing");

    // The commit succeeds, which means Trading authenticated the whole frame and
    // the real Claims program accepted the SignedDelta at CPI depth two.
    let logs = processed
        .metadata
        .as_ref()
        .map(|value| value.log_messages.clone())
        .unwrap_or_default();
    assert!(
        processed.result.is_ok(),
        "the commit must land; observed {:?} logs {logs:?}",
        processed.result
    );
    assert!(
        logs.iter().any(|line| line.contains("invoke [2]")),
        "the SignedDelta must be executed by the Claims program itself; logs {logs:?}"
    );

    // Conservation, not acceptance. The trade acquires at the funded
    // coordinate, so exactly that much leaves the counterparty and arrives at
    // the dealer, every other coordinate is untouched, and the two sides net to
    // zero. A transaction that merely succeeded would tell us none of this.
    let dealer_after = position_balances(&mut context, &scenario, scenario.fixture.actor_position.account).await;
    let counterparty_after =
        position_balances(&mut context, &scenario, scenario.fixture.reserve_position.account).await;
    let acquired = 10_u64;
    for claim in 0..usize::try_from(WIDTH).expect("small width") {
        let dealer_before = *scenario.live.dealer_balances.get(claim).expect("before");
        let counterparty_before =
            *scenario.live.counterparty_balances.get(claim).expect("before");
        let dealer_now = *dealer_after.get(claim).expect("after");
        let counterparty_now = *counterparty_after.get(claim).expect("after");
        if claim == FUNDED_COORDINATE {
            assert_eq!(
                dealer_now,
                dealer_before + acquired,
                "the dealer must receive exactly what it acquired"
            );
            assert_eq!(
                counterparty_now,
                counterparty_before - acquired,
                "the counterparty must part with exactly that much"
            );
        } else {
            assert_eq!(dealer_now, dealer_before, "coordinate {claim} must not move");
            assert_eq!(
                counterparty_now, counterparty_before,
                "coordinate {claim} must not move"
            );
        }
        assert_eq!(
            dealer_now + counterparty_now,
            dealer_before + counterparty_before,
            "the two Positions must conserve value at coordinate {claim}"
        );
    }

    // The obligation is replaced by exactly the candidate the request named.
    let obligation_after = context
        .banks_client
        .get_account(scenario.obligation)
        .await
        .expect("obligation query")
        .expect("obligation exists")
        .data;
    assert_eq!(
        obligation_after, scenario.candidate_obligation_bytes,
        "the committed obligation is the candidate the request committed to"
    );

    // And the checkpoint is terminal for this route.
    let committed = checkpoint_body(&mut context, &scenario).await;
    assert_ne!(committed, reserved, "the commit advances the checkpoint");
    journal
        .record_committed(hash(&committed).to_bytes())
        .expect("journal records the commit it observed");
}


/// Create a real on-chain address lookup table and activate its addresses.
///
/// This is not harness convenience. The commit route resolves more addresses
/// than a static message can carry, so a caller that cannot build a table
/// cannot commit at all; creating one here is part of the evidence.
async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
) -> Pubkey {
    let clock = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make the lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    submit(context, create, &[])
        .await
        .expect("ProgramTest processing")
        .result
        .expect("create the lookup table");
    for chunk in addresses.chunks(20) {
        submit(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            &[],
        )
        .await
        .expect("ProgramTest processing")
        .result
        .expect("extend the lookup table");
    }
    let extended = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extended.slot + 1)
        .expect("activate the lookup addresses");
    table
}

/// Submit one route as a v0 transaction resolved through a live table.
async fn submit_v0(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let payer = context.payer.insecure_clone();
    let transaction =
        VersionedTransaction::try_new(message, &[&payer]).expect("signed v0 transaction");
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
}


/// Drive one scenario to a reserved checkpoint and return its commit inputs.
async fn reserved_with_commit_inputs(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> (ReservationEvidence, Pubkey, Vec<u8>) {
    reserved_with_published_delta(context, scenario, None).await
}

/// The same, over a delta the evaluator chose to publish and seal.
async fn reserved_with_published_delta(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    published_delta: Option<Vec<u8>>,
) -> (ReservationEvidence, Pubkey, Vec<u8>) {
    let (reservation, _, _journal) =
        evaluated_with_published_delta(context, scenario, published_delta).await;
    let payer = context.payer.pubkey();
    let instruction = reserve_instruction(
        scenario,
        payer,
        &reservation,
        scenario.waist.custody_program,
        scenario.waist.custody_programdata,
        scenario.waist.activation_cache,
    );
    submit(context, instruction, &[])
        .await
        .expect("ProgramTest processing")
        .result
        .expect("the reservation must be ingested");
    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    let reserved = checkpoint_body(context, scenario).await;
    (reservation, receipt_address, reserved)
}

/// Build, table and submit one commit, returning what the chain reported.
async fn submit_commit(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    bank: DealerScenarioCommitAccountsV1,
) -> solana_program_test::BanksTransactionResultWithMetadata {
    let payer = context.payer.pubkey();
    let mut addresses = bank
        .claims_accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    addresses.extend([
        bank.checkpoint,
        bank.clock,
        bank.request,
        bank.evaluation_receipt,
        bank.candidate_bank,
        bank.candidate_obligation,
        bank.claims_delta,
        bank.effects,
        bank.root,
        bank.obligation,
        bank.custody_program,
        bank.custody_programdata,
        bank.batch,
    ]);
    for proof in bank.effect_accounts.iter().take(usize::from(bank.effect_count)) {
        addresses.extend([proof.reservation_receipt, proof.reservation_state]);
    }
    addresses.retain(|key| *key != payer && *key != TRADING);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    let packet = build_dealer_scenario_commit_v1(
        bank,
        Hash::default(),
        &[OperatorLookupTable {
            key: Pubkey::new_from_array([0x7b; 32]),
            addresses: addresses.clone(),
        }],
    )
    .expect("commit packet");
    let table = create_live_lookup_table(context, &addresses).await;
    let _ = scenario;
    submit_v0(context, packet.instruction, table, &addresses)
        .await
        .expect("ProgramTest processing")
}

#[tokio::test]
async fn a_commit_refuses_a_claims_delta_that_is_not_the_request_packet() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;

    // A consistent lie, not a torn one. The evaluator publishes a delta that is
    // not the request's packet and seals that same body in its receipt, so
    // every digest agrees with itself and only the packet identity can refuse
    // it. Substituting the body after evaluation would have been answered one
    // check earlier, by the receipt digest, and would not have reached this at
    // all -- the same early-stop trap the reservation producer case taught.
    let mut published = DealerScenarioTradeRequestV3::decode(&scenario.request_bytes)
        .expect("canonical request decodes")
        .claims_packet()
        .to_vec();
    *published.last_mut().expect("packet is not empty") ^= 0xff;
    let (reservation, receipt_address, reserved) =
        reserved_with_published_delta(&mut context, &scenario, Some(published)).await;
    let payer = context.payer.pubkey();

    let bank = commit_bank(&scenario, payer, receipt_address, &reservation);
    let processed = submit_commit(&mut context, &scenario, bank).await;
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "a delta that is not the request's packet must refuse; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        reserved,
        "a refused commit must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_commit_refuses_a_locked_batch_that_names_another_receipt() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (reservation, receipt_address, reserved) =
        reserved_with_commit_inputs(&mut context, &scenario).await;
    let payer = context.payer.pubkey();

    // The batch keeps its PDA, its Custody owner, its width and its structural
    // validity. Only the receipt digest it records changes, so the checkpoint
    // and the batch now disagree about which reservation was ingested.
    let original = context
        .banks_client
        .get_account(reservation.batch)
        .await
        .expect("batch query")
        .expect("batch exists");
    let mut batch = DealerScenarioReservationBatchV1::decode(&original.data)
        .expect("canonical batch decodes");
    batch.receipt_digests = core::array::from_fn(|index| {
        if index == 0 { [0x7c; 32] } else { [0_u8; 32] }
    });
    let substituted = batch.encode().expect("substituted batch encodes").to_vec();
    assert_eq!(
        substituted.len(),
        original.data.len(),
        "the substitution must keep the batch width"
    );
    context.set_account(
        &reservation.batch,
        &AccountSharedData::from(data_account(scenario.waist.custody_program, substituted)),
    );

    let bank = commit_bank(&scenario, payer, receipt_address, &reservation);
    let processed = submit_commit(&mut context, &scenario, bank).await;
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "a batch naming another receipt must refuse; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        reserved,
        "a refused commit must not advance the checkpoint"
    );
}


/// Read one Claims Position's balance vector back off the chain.
async fn position_balances(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    position: Pubkey,
) -> Vec<u64> {
    let account = context
        .banks_client
        .get_account(position)
        .await
        .expect("position query")
        .expect("position exists");
    let view = LiabilityBasisPositionViewV2::decode(&account.data).expect("position decodes");
    (0..scenario.fixture.outcome_count)
        .map(|claim| view.balance(&account.data, claim).expect("balance"))
        .collect()
}
