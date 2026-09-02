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

use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_claims_svm::frame_spec_v1::{ClaimsFrameRoleV1, SignedDeltaFrameSpecV3};
use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketInputV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    encode_liability_basis_market_into_v2,
};
use dclutch_claims_svm::signed_delta_v3::SignedDeltaPlanV3;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CompartmentV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_dealer_accelerator_program_test::custody_delivery::{
    DealerDeliveryInputV1, DealerDeliveryRealmV1, DealerDeliveryV1, dealer_delivery_realm_v1,
    dealer_delivery_token_account_bytes, mint_total_supply, stage_dealer_delivery_v1,
    token_account_amount,
};
use dclutch_dealer_codec::{
    scenario::ClaimsInventoryObservation,
    scenario_checkpoint_v1::DEALER_SCENARIO_PREPARATION_PAGES_V1,
    scenario_custody_reservation_v1::{
        DealerScenarioActivationReceiptV1, DealerScenarioReservationBatchStatusV1,
        DealerScenarioReservationBatchV1, DealerScenarioReservationStateStatusV1,
        DealerScenarioReservationStateV1,
    },
    scenario_membership_manifest_v1::{
        DEALER_SCENARIO_MEMBERSHIP_PAGES_V1, DealerScenarioMembershipManifestV1,
    },
    scenario_reservation_receipt_v1::{
        DEALER_SCENARIO_MAX_RESERVATIONS_V1, DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
        DealerScenarioReservationActionV1, DealerScenarioReservationReceiptV1,
    },
};
use dclutch_fractional_atomic_program_test::narrow_fixture::{
    NarrowFixtureInputV2, NarrowFixtureV2, NarrowPositionV2, compile_narrow_fixture_v2,
};
use dclutch_operator::{
    dealer_scenario_checkpoint_v1::{
        DealerAcceptedEvaluationAccountsV4, DealerAcceptedReservationAccountsV4,
        DealerAcceptedTranscriptInputV4, DealerScenarioActivationAccountsV1,
        DealerScenarioActivationEffectAccountsV1, DealerScenarioCheckpointJournalV1,
        DealerScenarioCheckpointRouteV1, DealerScenarioCommitAccountsV1,
        DealerScenarioCommitEffectAccountsV1, DealerScenarioEvaluationBodiesV1,
        DealerScenarioReservationAccountsV1, DealerScenarioReservationBundlePacketV1,
        build_dealer_accepted_transcript_v4, build_dealer_scenario_activation_v1,
        build_dealer_scenario_checkpoint_cleanup_v1, build_dealer_scenario_checkpoint_create_v1,
        build_dealer_scenario_checkpoint_evaluate_v1, build_dealer_scenario_checkpoint_page_v1,
        build_dealer_scenario_checkpoint_reserve_v1, build_dealer_scenario_commit_v1,
        build_dealer_scenario_reservation_bundle_v1, dealer_scenario_checkpoint_address_v1,
        dealer_scenario_evaluation_receipt_address_v1,
        dealer_scenario_membership_manifest_address_v1,
        dealer_scenario_reservation_batch_address_v1, derive_dealer_scenario_evaluation_receipt_v1,
        encode_dealer_scenario_custody_effect_artifacts_v1,
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
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_trading_sbf::dealer::{
    v3_composer::{ScenarioCollateralFrameV3, ScenarioComposerContextV3, ScenarioCustodyEffectV3},
    v3_multi_lp::MultiLpCustodyRequestV3,
    v3_obligation::stage_scenario_obligation_replacement_v3,
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
    v3_trade::{
        DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
        DealerScenarioTradeRequestV3, ScenarioTradeChainProjectionV3, ScenarioTradeDirectionV3,
        ScenarioTradeIntentV3, build_scenario_trade_request_v3,
        scenario_trade_max_request_bytes_v3,
    },
    v3_trade_profile::{
        DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DEALER_SCENARIO_PROFILE_SPANS_V4,
        DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
    },
};
use dclutch_trading_sbf::{
    TradingSbfError, dealer_scenario_checkpoint_v1::DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::AddressLookupTableAccount;
use solana_message::{VersionedMessage, v0};
use solana_message_v3::AddressLookupTableAccount as OperatorLookupTable;
use solana_program::{
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;
use solana_transaction::versioned::VersionedTransaction;

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
// Custody's codes, derived from the REGISTERED BAND rather than written out.
//
// `CustodySbfError` lives in `dclutch-custody-sbf`, and a program-test taking a
// dependency on the program it drives as a real ELF is the wrong direction --
// decision 0007 names the registry base as the alternative for exactly this
// case. What was here before were three bare literals, and a bare refusal
// number is how a code drifts out of its band without anything going red.
/// Custody's refusal when a replay PDA, owner, bytes or revision does not join.
const CUSTODY_REPLAY: u32 = dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x005;
/// Custody's refusal when a vault PDA, token state or authority policy refuses.
const CUSTODY_TOKEN_STATE: u32 = dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x006;
/// Custody's refusal when a reservation is rolled back before it can expire.
const CUSTODY_EXPIRY: u32 = dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x00B;

/// Runtime Product outcome width this scenario transitions.
const WIDTH: u32 = 3;
/// Representation coordinate the Claims graph funds and this scenario trades at.
const FUNDED_COORDINATE: usize = 0;
/// External collateral token account the delivered effect credits.
const DEALER_COLLATERAL_ACCOUNT: Pubkey = Pubkey::new_from_array([0xde; 32]);
/// Collateral atoms the reservation locked and the delivery moves.
const DELIVERY_AMOUNT: u64 = 10;
/// Source-vault balance after the reservation debited it.
const DELIVERY_SOURCE_AFTER: u64 = 90;
/// Destination balance before the delivery credits it.
const DELIVERY_DESTINATION_BEFORE: u64 = 100;
/// Revision the standard Custody replay cursor stands at before delivery.
const DELIVERY_REPLAY_REVISION: u64 = 7;
/// Immutable Custody replay namespace for this Market.
const SCENARIO_CUSTODY_CONTEXT: [u8; 32] = [0xbb; 32];
/// Market generation every layer of this scenario restates.
const SCENARIO_GENERATION: u64 = 17;
/// Claims aggregate revision this scenario trades against.
///
/// The aggregate is re-encoded to it through the supported Claims encoder
/// rather than byte-patched; the fixture does not parameterize this one.
const LIVE_CLAIMS_REVISION: u64 = 4;
/// Position revision both Claims Positions carry.
///
/// The Dealer projection refuses a Position at revision zero, because a Dealer
/// trade is against Positions that have already been transacted. The narrow
/// fixture takes this as an input and plants both Positions at it.
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
    let artifact =
        ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact identity");
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
    child_root: Pubkey,
    counterparty: Pubkey,
    counterparty_account: Pubkey,
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
    realm: DealerDeliveryRealmV1,
    delivery: DealerDeliveryV1,
    custody_effect: ScenarioCustodyEffectV3,
}

/// Derive one complete scenario: request, checkpoint, canonical membership.
fn scenario() -> Scenario {
    let waist = release_waist();
    // A Realm is content-addressed by its own body, so the scenario cannot name
    // one: it builds the record first and every layer below restates the digest.
    let realm = dealer_delivery_realm_v1(waist.registry);
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
        realm_id: realm.digest,
        custody_context: SCENARIO_CUSTODY_CONTEXT,
        generation: SCENARIO_GENERATION,
        actor_owner: dealer.pubkey(),
        reserve_owner: COUNTERPARTY,
        funded_coordinate: FUNDED_COORDINATE,
        funded_balance: 100,
        // A Dealer trade is against Positions that have already been transacted,
        // and the projection refuses revision zero outright. The fixture plants
        // the live revision itself, so no Dealer file re-encodes a Position.
        position_revision: LIVE_POSITION_REVISION,
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
    let obligation_state = obligation_bytes(
        market,
        product,
        basis,
        dealer_owner,
        child,
        7,
        &[12, 20, 10],
    );
    let current_obligation =
        DealerObligationProjectionV3::decode(&obligation_state).expect("canonical obligation");
    let obligation =
        Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child], &TRADING).0;
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
        basis_scale: 1,
    };
    // The narrow fixture funds ONE representation coordinate per Position, so
    // this first executed commit trades at that coordinate. Dealer scenarios are
    // not restricted to one coordinate in general; the campaign is.
    let mut acquired = vec![0_u64; usize::try_from(WIDTH).expect("small width")];
    let delivered = vec![0_u64; usize::try_from(WIDTH).expect("small width")];
    // Acquired and delivered must be disjoint per coordinate, and the graph
    // funds exactly one, so this trade moves value one way at that coordinate.
    *acquired
        .get_mut(FUNDED_COORDINATE)
        .expect("funded coordinate") = 10;
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
    // The collateral graph the later delivery executes against. Every address in
    // it is derived from the request the effect will carry, in the direction
    // Custody derives it, so the escrow is reachable only through the
    // reservation that owns it.
    let delivery = stage_dealer_delivery_v1(DealerDeliveryInputV1 {
        custody_program: waist.custody_program,
        trading_program: TRADING,
        release_set: waist.release_set_id,
        market: fixture.core_market,
        realm: realm.digest,
        // The composer names the child root as both the Custody replay namespace
        // and the trading-principal vault context, so the delivery debits the
        // very vault the scenario's own collateral frame carries and advances
        // the very cursor its effects were planned against.
        context: child,
        source_vault_context: child,
        generation: SCENARIO_GENERATION,
        checkpoint,
        request_digest,
        destination: DEALER_COLLATERAL_ACCOUNT,
        destination_owner: dealer.pubkey(),
        replay_rent_refund: BENEFICIARY,
        amount: DELIVERY_AMOUNT,
        source_after: DELIVERY_SOURCE_AFTER,
        destination_before: DELIVERY_DESTINATION_BEFORE,
        replay_revision: DELIVERY_REPLAY_REVISION,
    });
    let membership_manifest = dealer_scenario_membership_manifest_address_v1(
        MANIFEST_PRODUCER,
        checkpoint,
        request_digest,
    );
    let custody_effect = ScenarioCustodyEffectV3 {
        request: MultiLpCustodyRequestV3::Canonical(delivery.request),
        source_after: DELIVERY_SOURCE_AFTER,
        destination_after: DELIVERY_DESTINATION_BEFORE
            .checked_add(DELIVERY_AMOUNT)
            .expect("destination after"),
    };

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
            realm: realm.digest,
            child_root: child,
            obligation_account: obligation.to_bytes(),
            // The census names the collateral pair the executed delivery moves.
            // A scenario naming two different mints is the shape that hides a
            // defect, even where nothing executed joins the two.
            mint: delivery.mint.to_bytes(),
            token_program: delivery.token_program.to_bytes(),
            parent_request_digest: request_digest,
            generation: SCENARIO_GENERATION,
            custody_replay_revision: 7,
            locked_capital_floor: 0,
            basis_scale: 1,
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
                &[
                    CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
                    &market,
                    &waist.release_set_id,
                ],
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
    let (mut fixed_accounts, suffix) = physical_frame(WIDTH, projected.dynamic_span_counts);
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
    let mut strategy_accounts = (0..frame.admitted_evidence_count
        + projected.caller_authority_count)
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
    let manifest_bytes = canonical
        .manifest
        .encode()
        .expect("manifest encode")
        .to_vec();
    Scenario {
        dealer,
        child_root: CHILD_ROOT,
        counterparty: COUNTERPARTY,
        counterparty_account: COUNTERPARTY_ACCOUNT,
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
        realm,
        delivery,
        custody_effect,
    }
}

/// The compiled Claims graph re-encoded at a revision Dealer can trade against.
#[derive(Clone)]
struct LiveClaimsGraph {
    market: Vec<u8>,
    dealer_position: Vec<u8>,
    counterparty_position: Vec<u8>,
    dealer_balances: Vec<u64>,
    counterparty_balances: Vec<u64>,
}

/// Re-encode the compiled graph at a live revision through supported encoders.
fn live_claims_graph(fixture: &NarrowFixtureV2) -> LiveClaimsGraph {
    let market_view = LiabilityBasisMarketViewV2::decode(&fixture.claims_market_bytes)
        .expect("aggregate decodes");
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
    // The Positions are taken as the fixture planted them: it opens both at
    // `position_revision`, so the campaign only has to read their coordinates.
    let observe = |position: &NarrowPositionV2| {
        let view = LiabilityBasisPositionViewV2::decode(&position.bytes).expect("position decodes");
        assert_eq!(
            view.revision, LIVE_POSITION_REVISION,
            "fixture Positions open at the revision the campaign trades against"
        );
        let balances = (0..fixture.outcome_count)
            .map(|claim| view.balance(&position.bytes, claim).expect("balance"))
            .collect::<Vec<_>>();
        (position.bytes.clone(), balances)
    };
    let (dealer_position, dealer_balances) = observe(&fixture.actor_position);
    let (counterparty_position, counterparty_balances) = observe(&fixture.reserve_position);
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
            (
                fixture.core_market,
                self.waist.core_program,
                fixture.core_state.clone(),
            ),
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
    program_test_configured(scenario, true)
}

/// Install the same scenario while allowing transaction ComputeBudget requests.
fn program_test_with_transaction_compute(scenario: &Scenario) -> ProgramTest {
    program_test_configured(scenario, false)
}

fn program_test_configured(scenario: &Scenario, fixed_compute_override: bool) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    if fixed_compute_override {
        test.set_compute_max_units(1_400_000);
    }
    test.add_account(
        REQUEST,
        data_account(TRADING, scenario.request_bytes.clone()),
    );
    test.add_account(scenario.child_root, data_account(TRADING, vec![0xaa; 64]));
    test.add_account(
        scenario.obligation,
        data_account(TRADING, scenario.obligation_state.clone()),
    );
    test.add_account(
        scenario.membership_manifest,
        data_account(MANIFEST_PRODUCER, scenario.manifest_bytes.clone()),
    );
    test.add_account(BENEFICIARY, data_account(system_program::ID, Vec::new()));
    test.add_account(
        scenario.counterparty,
        data_account(system_program::ID, Vec::new()),
    );
    test.add_account(
        scenario.counterparty_account,
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
    // The collateral graph the delivery leg executes against: the Realm at the
    // address its own digest derives, its vacant staging cursor beside it, the
    // Mint the Realm selects, and the three token accounts the reservation left.
    // The token program itself is not installed here -- ProgramTest genesis
    // already carries the real one at the address the Realm's adapter names.
    let delivery = &scenario.delivery;
    test.add_account(
        scenario.realm.raw,
        data_account(scenario.waist.registry, scenario.realm.bytes.clone()),
    );
    test.add_account(
        scenario.realm.staging,
        Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        delivery.mint,
        data_account(delivery.token_program, delivery.mint_bytes.clone()),
    );
    for (key, body) in [
        (delivery.source, &delivery.source_bytes),
        (delivery.escrow, &delivery.escrow_bytes),
        (delivery.destination, &delivery.destination_bytes),
    ] {
        test.add_account(key, data_account(delivery.token_program, body.clone()));
    }
    test.add_account(
        delivery.replay,
        data_account(
            scenario.waist.custody_program,
            delivery.replay_bytes.clone(),
        ),
    );
    // Install the frame exactly as the projection observed it: same identities,
    // same owners, same widths. The pages carry these accounts, so what the
    // campaign pages is the frame itself and not a stand-in for it.
    let mut reserved = vec![
        TRADING,
        REQUEST,
        scenario.child_root,
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
        scenario.counterparty,
        scenario.counterparty_account,
        scenario.core_market,
        scenario.realm.raw,
        scenario.realm.staging,
        delivery.mint,
        delivery.token_program,
        delivery.source,
        delivery.escrow,
        delivery.destination,
        delivery.replay,
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
            test.add_account(
                key,
                Account {
                    lamports: Rent::default().minimum_balance(0).max(1),
                    data: Vec::new(),
                    owner,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        } else {
            test.add_account(key, data_account(owner, body));
        }
    }
    test
}

fn add_executable(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(
        key,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
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
    let instruction_data = instruction.data.clone();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    // One short-vector signature count byte, one signature, and the canonical
    // message bytes. ProgramTest submits no packet, so a fast lane that wants to
    // say anything about the 1,232-byte maximum has to MEASURE, not ask the
    // runtime to enforce.
    let wire_bytes = 1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
        .expect("bounded transaction wire");
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    record_campaign_transaction(&signature, slot, Some(wire_bytes), &instruction_data, &processed);
    Ok(processed)
}

/// The checkpoint route this instruction dispatches to, named as the inventory
/// names it.
///
/// Read off the submitted bytes with the PROGRAM'S OWN predicates -- the same
/// `is_*` functions `process_dealer_family_v1` dispatches on -- so a campaign
/// label cannot name a route other than the one Trading will take. A binding
/// key derived any other way is a second author for the route claim, and the
/// census exists to catch exactly that.
///
/// `None` for anything that is not a checkpoint stage: the Hot execution
/// envelope, the activation and commit banks, and Address Lookup Table
/// maintenance. Those bindings are authored from the logs instead.
fn dealer_checkpoint_route_v1(data: &[u8]) -> Option<&'static str> {
    use dclutch_trading_sbf::dealer_scenario_checkpoint_v1 as checkpoint;
    const PREFIX: &str = "trading/dealer_scenario_checkpoint_v1::";
    if checkpoint::is_dealer_scenario_checkpoint_create_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_create_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_page_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_page_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_evaluate_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_evaluate_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_reserve_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_reserve_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_rollback_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_rollback_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_commit_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_commit_v1"
        ));
    }
    if checkpoint::is_dealer_scenario_checkpoint_cleanup_v1(data) {
        return Some(concat!(
            "trading/dealer_scenario_checkpoint_v1::",
            "process_dealer_scenario_checkpoint_cleanup_v1"
        ));
    }
    let _ = PREFIX;
    None
}

/// Emit one census observation, when the gauntlet asked for evidence.
///
/// A no-op in an ordinary `cargo test` run, because
/// `dclutch_program_test_evidence::record` returns `Ok(())` with the evidence
/// directory unset.
///
/// The label is derived, not hand-written, and that is deliberate. A campaign
/// label is the key a `bindings.json` row matches on, so a hand-written one can
/// drift from the transaction it names while both still look right. This one is
/// the libtest thread name -- which is the test's own path, because every case
/// here is a current-thread `#[tokio::test]` -- plus that test's submission
/// ordinal. It cannot name a transaction other than the one it came from, and
/// re-running the campaign reproduces it exactly.
fn record_campaign_transaction(
    signature: &str,
    slot: u64,
    wire_bytes: Option<usize>,
    instruction_data: &[u8],
    processed: &solana_program_test::BanksTransactionResultWithMetadata,
) {
    if dclutch_program_test_evidence::evidence_directory().is_none() {
        return;
    }
    let case = std::thread::current().name().unwrap_or("unnamed").to_owned();
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let logs = processed
        .metadata
        .as_ref()
        .map_or_else(Vec::new, |metadata| metadata.log_messages.clone());
    // THE DISPOSITION IS PART OF THE KEY, and it has to be.
    //
    // A binding declares one outcome and the census checks it against the
    // chain. Seven cases here submit the same route twice on purpose -- once
    // accepted, once refused -- so a key of (test, route) alone would put two
    // outcomes under one binding and the census would refuse the campaign
    // rather than record half of it. The disposition is READ BACK from what the
    // runtime reported, never asserted: `executed` is a null error, and a
    // refusal carries the custom code out of the chain's own failure line, so a
    // refusal that changes code changes binding rather than quietly reusing one.
    let disposition = match custom_code(&processed.result) {
        Some(code) => format!("refused 0x{code:x}"),
        None if failure.is_some() => "refused".to_owned(),
        None => "executed".to_owned(),
    };
    let label = match dealer_checkpoint_route_v1(instruction_data) {
        Some(route) => format!("dealer accepted: {case} -- {route} -- {disposition}"),
        None => format!("dealer accepted: {case} -- {disposition}"),
    };
    let compute_units = processed
        .metadata
        .as_ref()
        .map(|metadata| metadata.compute_units_consumed);
    dclutch_program_test_evidence::record(&dclutch_program_test_evidence::TransactionEvidence {
        label: &label,
        signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: compute_units,
        wire_bytes,
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
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
        scenario.child_root,
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
        context.set_account(
            key,
            &AccountSharedData::from(data_account(TRADING, body.clone())),
        );
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
    // The effect bank is built by the semantic owner from the scenario's own
    // Custody effect, not restated here. The nested request is real: activation
    // decodes it and joins its release set, Market, Realm, caller program,
    // parent request digest, generation, transfer index, Mint and token program
    // against the batch, so a zero payload reserves and can never deliver.
    let effects = [Some(scenario.custody_effect), None, None, None];
    let artifacts = encode_dealer_scenario_custody_effect_artifacts_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
        core::array::from_fn(|index| {
            if index == 0 {
                EFFECT_BODY
            } else {
                Pubkey::default()
            }
        }),
        &effects,
        1,
    )
    .expect("evaluator-owned effect bank");
    assert_eq!(
        DEALER_SCENARIO_MAX_RESERVATIONS_V1, 4,
        "the manifest carries a fixed reservation width"
    );
    let effect_body = artifacts
        .effect_bodies
        .first()
        .copied()
        .flatten()
        .expect("the zeroth effect body")
        .encode()
        .expect("canonical effect body encodes")
        .to_vec();
    let effects_body = artifacts
        .manifest
        .encode()
        .expect("effect manifest encodes")
        .to_vec();
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
    let (instruction, _) = page_instruction(
        &scenario,
        payer,
        0,
        scenario.pages.first().expect("page zero"),
    );
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
        context.set_account(
            key,
            &AccountSharedData::from(data_account(TRADING, body.clone())),
        );
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
fn lookup_table(
    scenario: &Scenario,
    commit: &DealerScenarioCommitAccountsV1,
) -> OperatorLookupTable {
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
        root: scenario.child_root,
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
        root: scenario.child_root,
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
        context.set_account(
            key,
            &AccountSharedData::from(data_account(TRADING, body.clone())),
        );
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
    assert!(
        rent_held > 0,
        "the checkpoint holds the rent it must return"
    );
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
    let delivery = &scenario.delivery;
    let batch = dealer_scenario_reservation_batch_address_v1(custody, scenario.checkpoint);
    let reservation_state = delivery.reservation_state;
    let state_body = DealerScenarioReservationStateV1 {
        status: DealerScenarioReservationStateStatusV1::Active,
        ordinal: 0,
        effect_count: 1,
        batch: batch.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        effects_digest,
        effect_digest,
        // Every coordinate here is the chain's, not the campaign's: activation
        // re-reads the escrow and the destination and refuses unless the
        // reservation's digests are what those accounts actually hold.
        source: delivery.source.to_bytes(),
        destination: delivery.destination.to_bytes(),
        escrow: delivery.escrow.to_bytes(),
        mint: delivery.mint.to_bytes(),
        token_program: delivery.token_program.to_bytes(),
        source_prestate_digest: [0xf8; 32],
        destination_prestate_digest: delivery.destination_digest(),
        effect_poststate_digest: delivery.escrow_digest(),
        source_poststate_digest: [0xfb; 32],
        amount: DELIVERY_AMOUNT,
        source_after: DELIVERY_SOURCE_AFTER,
        destination_before: DELIVERY_DESTINATION_BEFORE,
        escrow_after: DELIVERY_AMOUNT,
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
    let receipt_body = receipt
        .encode()
        .expect("reservation receipt encodes")
        .to_vec();
    let batch_body = DealerScenarioReservationBatchV1 {
        status: DealerScenarioReservationBatchStatusV1::Reserved,
        effect_count: 1,
        reserved_count: 1,
        rollback_count: 0,
        release_set: scenario.waist.release_set_id,
        market: scenario.core_market.to_bytes(),
        realm: scenario.realm.digest,
        trading_program: TRADING.to_bytes(),
        checkpoint: scenario.checkpoint.to_bytes(),
        request_digest: scenario.request_digest,
        effects_digest,
        replay: delivery.replay.to_bytes(),
        replay_prestate_digest: delivery.replay_digest(),
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

/// The Custody-derived address of one reservation receipt.
fn reservation_receipt_address(scenario: &Scenario, ordinal: u8) -> Pubkey {
    reservation_receipt_address_for(scenario, ordinal, DealerScenarioReservationActionV1::Reserve)
}

/// The receipt address for one ordinal AND ONE ACTION.
///
/// The action is a PDA seed (`require_reservation_identities`), so a reserve and
/// the rollback that reverses it write two different receipts -- and the
/// rollback's must be vacant when it runs, because the route creates it. A
/// helper that hard-wired `Reserve` therefore could not express a rollback at
/// all: handing the reserve's receipt to a rollback refuses `AccountFrame`
/// (0x6001), which is what the first draft of the accepting-rollback case did.
fn reservation_receipt_address_for(
    scenario: &Scenario,
    ordinal: u8,
    action: DealerScenarioReservationActionV1,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            scenario.checkpoint.as_ref(),
            &scenario.request_digest,
            &[action as u8],
            &[ordinal],
        ],
        &scenario.waist.custody_program,
    )
    .0
}

/// Make CUSTODY produce the reservation, instead of the campaign publishing it.
///
/// The reservation evidence was the campaign's last staged protocol body. It was
/// real in shape -- `reservation_evidence` builds the receipt, state and batch
/// through the supported encoders, and every route downstream authenticates them
/// exactly as it would authenticate Custody's own output -- but Custody's reserve
/// route had never written them. Four of the state's coordinates could not be
/// real, because only the chain knows them: the source vault's prestate and
/// poststate digests, and the batch's last-prestate digest, were literals.
///
/// This drives the route instead. `build_dealer_scenario_reservation_bundle_v1`
/// is the operator's atomic producer-then-ingest pair and had no consumer in the
/// tree: instruction one is Custody's own `Reserve`, which moves real collateral
/// out of the trading-principal vault into an escrow it creates, and writes the
/// batch, the state and the typed receipt; instruction two is Trading's ingest,
/// which must immediately join that receipt to the checkpoint. They share one
/// transaction, so a producer whose receipt Trading refuses is rolled back whole.
///
/// Two prestates have to be corrected for the route to have anything to do. The
/// staged campaign installed the vault as the reservation LEFT it and the escrow
/// already funded, because nothing was going to debit anything. Here the vault
/// is installed as the reservation FINDS it and the escrow is vacant, because
/// Custody creates it. Those two staged bodies do not disappear -- they become
/// the poststate the caller asserts the chain reached.
/// The operator's atomic Custody-producer plus Trading-ingest pair for ordinal zero.
///
/// Every coordinate is the scenario's own; nothing here is a second derivation.
fn reservation_bundle(
    scenario: &Scenario,
    payer: Pubkey,
) -> DealerScenarioReservationBundlePacketV1 {
    reservation_bundle_for(scenario, payer, DealerScenarioReservationActionV1::Reserve)
}

/// The same pair for either action.
///
/// The builder already knows the difference: Rollback swaps the Trading ingest
/// magic to `DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1` and makes the refund
/// beneficiary writable, because that is the account the escrow's rent goes back
/// to. Nothing here restates either fact.
fn reservation_bundle_for(
    scenario: &Scenario,
    payer: Pubkey,
    action: DealerScenarioReservationActionV1,
) -> DealerScenarioReservationBundlePacketV1 {
    let delivery = &scenario.delivery;
    build_dealer_scenario_reservation_bundle_v1(
        action,
        0,
        DealerScenarioReservationAccountsV1 {
            custody_program: scenario.waist.custody_program,
            market: scenario.core_market,
            activation_cache: scenario.waist.activation_cache,
            registry_program: scenario.waist.registry,
            trading_program: TRADING,
            trading_programdata: scenario.waist.trading_programdata,
            realm: scenario.realm.raw,
            realm_staging: scenario.realm.staging,
            custody_replay: delivery.replay,
            checkpoint: scenario.checkpoint,
            effect_producer: TRADING,
            effect_manifest: EFFECTS,
            effect_body: EFFECT_BODY,
            batch: dealer_scenario_reservation_batch_address_v1(
                scenario.waist.custody_program,
                scenario.checkpoint,
            ),
            reservation_state: delivery.reservation_state,
            reservation_receipt: reservation_receipt_address_for(scenario, 0, action),
            source: delivery.source,
            destination: delivery.destination,
            escrow: delivery.escrow,
            mint: delivery.mint,
            custody_authority: delivery.custody_authority,
            token_program: delivery.token_program,
            payer,
            refund_beneficiary: BENEFICIARY,
            clock: sysvar::clock::ID,
            rent: sysvar::rent::ID,
            system_program: system_program::ID,
            custody_programdata: scenario.waist.custody_programdata,
        },
        Hash::default(),
        &[],
    )
    .expect("reservation bundle")
}

/// Submit one Custody-producer plus Trading-ingest pair, and record it.
///
/// The three bundle sites shared this verbatim. Instruction ONE is Trading's
/// ingest and names the checkpoint route, so the route half of the label is read
/// off it with the program's own predicates exactly as the single-instruction
/// paths do. Instruction ZERO is Custody's own producer, which is why the
/// binding for these rows names `custody`: it is the program the chain's logs
/// corroborate first.
async fn submit_reservation_bundle(
    context: &mut ProgramTestContext,
    bundle: &DealerScenarioReservationBundlePacketV1,
    payer: Pubkey,
) -> solana_program_test::BanksTransactionResultWithMetadata {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let signer = context.payer.insecure_clone();
    let transaction = Transaction::new_signed_with_payer(
        &bundle.instructions,
        Some(&payer),
        &[&signer],
        blockhash,
    );
    let wire_bytes = 1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
        .expect("bounded transaction wire");
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ProgramTest processing");
    record_campaign_transaction(
        &signature,
        slot,
        Some(wire_bytes),
        bundle
            .instructions
            .get(1)
            .map_or(&[][..], |instruction| instruction.data.as_slice()),
        &processed,
    );
    processed
}

async fn reserve_through_custody(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> ReservationEvidence {
    let delivery = &scenario.delivery;
    let payer = context.payer.pubkey();
    context.set_account(
        &delivery.source,
        &AccountSharedData::from(data_account(
            delivery.token_program,
            delivery.source_prereservation_bytes(),
        )),
    );
    context.set_account(
        &delivery.escrow,
        &AccountSharedData::from(Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        }),
    );

    let receipt_address = reservation_receipt_address(scenario, 0);
    let batch = dealer_scenario_reservation_batch_address_v1(
        scenario.waist.custody_program,
        scenario.checkpoint,
    );
    let bundle = reservation_bundle(scenario, payer);
    assert!(
        bundle.lock_census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
        "the producer-plus-ingest pair must stay lock-bounded: {}",
        bundle.lock_census.unique_account_lock_count
    );

    // Custody produces and Trading ingests in one transaction. If the second
    // instruction refuses, SVM rolls the token movement and all three created
    // reservation records back with it. The independently ingestible route
    // remains the recovery shape for an RPC-response loss, not the honest
    // campaign's substitute for atomicity.
    let processed = submit_reservation_bundle(context, &bundle, payer).await;
    assert!(
        processed.result.is_ok(),
        "atomic Custody reservation plus Trading ingest must commit: {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let logs = processed
        .metadata
        .as_ref()
        .map(|value| &value.log_messages)
        .expect("accepted transaction metadata");
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {} success", scenario.waist.custody_program)),
        "the atomic pair must execute Custody: {logs:?}"
    );
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {TRADING} success")),
        "the atomic pair must execute Trading ingest: {logs:?}"
    );

    // What the reservation DID, asserted rather than assumed. The three bodies
    // this used to publish are now read back off the chain, and the two token
    // bodies the campaign used to stage are now poststates the route had to
    // reach on its own.
    let custody = scenario.waist.custody_program;
    let escrow_after = account_body(context, delivery.escrow)
        .await
        .expect("Custody created the escrow it locks into");
    assert_eq!(
        escrow_after, delivery.escrow_bytes,
        "the escrow Custody created and funded is exactly the escrow the campaign used to stage"
    );
    let source_after = account_body(context, delivery.source)
        .await
        .expect("the trading-principal vault");
    assert_eq!(
        source_after, delivery.source_bytes,
        "the vault the reservation debited is exactly the vault the campaign used to stage"
    );
    assert_eq!(
        token_account_amount(&source_after) + token_account_amount(&escrow_after),
        token_account_amount(&delivery.source_prereservation_bytes()),
        "collateral is conserved across the lock: what left the vault is in the escrow"
    );

    let state_body = account_body(context, delivery.reservation_state)
        .await
        .expect("Custody wrote the reservation state");
    let state = DealerScenarioReservationStateV1::decode(&state_body)
        .expect("the reservation state Custody wrote decodes");
    assert_eq!(state.status, DealerScenarioReservationStateStatusV1::Active);
    assert_eq!(
        state.amount,
        scenario.custody_effect.request.custody().amount
    );
    assert_eq!(state.source_after, scenario.custody_effect.source_after);
    assert_eq!(
        state.escrow_after,
        scenario.custody_effect.request.custody().amount
    );
    // The four coordinates the staged body could not have known, because only
    // the chain knows them. They were literals -- 0xf8, 0xfb, 0xfe and a
    // `hash(&[])` standing in for Custody's own vacancy digest -- and a literal
    // is exactly as authentic as whatever wrote it.
    assert_eq!(
        state.source_prestate_digest,
        hash(&delivery.source_prereservation_bytes()).to_bytes(),
        "the source prestate is the vault the reservation actually found"
    );
    assert_eq!(
        state.source_poststate_digest,
        hash(&source_after).to_bytes(),
        "the source poststate is the vault the reservation actually left"
    );
    assert_eq!(
        state.effect_poststate_digest,
        hash(&escrow_after).to_bytes(),
        "the effect poststate is the escrow the reservation actually created"
    );

    let receipt_body = account_body(context, receipt_address)
        .await
        .expect("Custody wrote the reservation receipt");
    let written = DealerScenarioReservationReceiptV1::decode(&receipt_body)
        .expect("the receipt Custody wrote decodes");
    assert_eq!(written.producer_program, custody.to_bytes());
    assert_eq!(written.reservation, delivery.reservation_state.to_bytes());
    assert_eq!(
        written.reservation_poststate_digest,
        hash(&state_body).to_bytes(),
        "the receipt commits to the state body the chain holds"
    );

    let batch_body = account_body(context, batch)
        .await
        .expect("Custody wrote the reservation batch");
    let written_batch = DealerScenarioReservationBatchV1::decode(&batch_body)
        .expect("the batch Custody wrote decodes");
    assert_eq!(
        written_batch.status,
        DealerScenarioReservationBatchStatusV1::Reserved
    );
    assert_eq!(written_batch.reserved_count, 1);
    assert_eq!(
        written_batch.receipt_digests.first().copied(),
        Some(hash(&receipt_body).to_bytes()),
        "the batch records the digest of the receipt Custody actually wrote"
    );
    assert_eq!(
        written_batch.replay_prestate_digest,
        delivery.replay_digest(),
        "and it pins the replay cursor the chain holds"
    );

    ReservationEvidence {
        receipt_address,
        reservation_state: delivery.reservation_state,
        batch,
        // Nothing was published. The whole point of this path is that the three
        // bodies downstream authenticates are the ones Custody wrote.
        installed: Vec::new(),
    }
}

/// A late Trading refusal rolls the preceding Custody reservation back whole.
///
/// The honest atomic pair succeeds above. This hostile keeps the exact same
/// The reservation the checkpoint took out, given back.
///
/// `..._rollback_v1` was the one checkpoint stage with no ACCEPTING driver
/// anywhere in the tree: `tools/gauntlet/dealer-checkpoint` observed it exactly
/// once, refused, from the hostile case below, and
/// `DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1` occurred exactly once in the
/// whole campaign. A stage witnessed only by its own refusal is a stage nobody
/// has shown works.
///
/// This is the on-chain twin of the dealer-codec unit test
/// `outstanding_reservations_require_reverse_order_rollback_before_cleanup`:
/// reserve, expire, roll back, and assert the collateral came home.
///
/// Three conjuncts have to hold at once for Custody to accept it, and each is
/// asserted here rather than assumed:
///
/// - REVERSE ORDER. Custody computes the expected ordinal as
///   `reservation_count - rollback_count - 1`. One outstanding reservation means
///   ordinal zero, and any other ordinal refuses `Expiry`.
/// - EXPIRY. Rollback returns value to its ORIGINAL SOURCE, which is only
///   legitimate once the reservation can no longer be activated, so Custody
///   refuses while `slot <= checkpoint.input.expires_at`. The warp below is
///   load-bearing, not scene-setting.
/// - PHASE. The checkpoint must be `Reserved` or already `RollingBack`.
///
/// What it proves beyond reachability is conservation: the escrow drains to
/// zero and the vault is byte-for-byte the body the reservation first found, so
/// the lock and its release are exact inverses rather than merely both green.
#[tokio::test]
async fn an_expired_reservation_rolls_back_in_reverse_order_and_returns_the_collateral() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    prepare_through_evaluation(&mut context, &scenario).await;
    let reservation = reserve_through_custody(&mut context, &scenario).await;
    let delivery = &scenario.delivery;
    let payer = context.payer.pubkey();

    // The prestate this case exists to reverse, read off the chain rather than
    // staged: the reserve really moved collateral into a real escrow.
    let locked = scenario.custody_effect.request.custody().amount;
    let escrow_locked = account_body(&mut context, delivery.escrow)
        .await
        .expect("the reserve created the escrow");
    assert_eq!(
        token_account_amount(&escrow_locked),
        locked,
        "there is nothing to roll back unless the reserve locked something"
    );
    let checkpoint_reserved = checkpoint_body(&mut context, &scenario).await;

    // Rollback is expiry-gated. Before the warp Custody refuses `Expiry`, and
    // that refusal is the control for the acceptance below: without it a green
    // rollback would not distinguish "the gate passed" from "there is no gate".
    let premature = reservation_bundle_for(
        &scenario,
        payer,
        DealerScenarioReservationActionV1::Rollback,
    );
    let refused = submit_reservation_bundle(&mut context, &premature, payer).await;
    assert_eq!(
        custom_code(&refused.result),
        Some(CUSTODY_EXPIRY),
        "an unexpired reservation must not be rollable: {:?}",
        refused.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        checkpoint_reserved,
        "a refused rollback must leave the checkpoint byte-for-byte alone"
    );

    context
        .warp_to_slot(SCENARIO_EXPIRES_AT + 8)
        .expect("warp past expiry");

    let bundle = reservation_bundle_for(
        &scenario,
        payer,
        DealerScenarioReservationActionV1::Rollback,
    );
    let processed = submit_reservation_bundle(&mut context, &bundle, payer).await;
    assert!(
        processed.result.is_ok(),
        "an expired reservation must roll back: {:?} logs {:?}",
        processed.result,
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let logs = processed
        .metadata
        .as_ref()
        .map(|value| &value.log_messages)
        .expect("accepted transaction metadata");
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {} success", scenario.waist.custody_program)),
        "the rollback pair must execute Custody: {logs:?}"
    );
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {TRADING} success")),
        "the rollback pair must execute Trading's ingest: {logs:?}"
    );

    // Conservation, both directions. The escrow is empty and the vault is the
    // exact body the reservation found -- not merely the right amount.
    let escrow_after = account_body(&mut context, delivery.escrow).await;
    let escrow_amount = escrow_after
        .as_ref()
        .map_or(0, |body| token_account_amount(body));
    assert_eq!(escrow_amount, 0, "the escrow must not still hold the lock");
    let source_after = account_body(&mut context, delivery.source)
        .await
        .expect("the trading-principal vault");
    assert_eq!(
        source_after,
        delivery.source_prereservation_bytes(),
        "the vault is byte-for-byte the body the reservation first found"
    );

    // The reservation record says what happened, and it is not the reserve's
    // word for it: `RolledBack`, written by the route this case drives.
    let state = DealerScenarioReservationStateV1::decode(
        &account_body(&mut context, reservation.reservation_state)
            .await
            .expect("the reservation state survives its own rollback"),
    )
    .expect("the reservation state decodes");
    assert_eq!(
        state.status,
        DealerScenarioReservationStateStatusV1::RolledBack,
        "the state Custody rewrote must say the value went back to its source"
    );
    assert_ne!(
        checkpoint_body(&mut context, &scenario).await,
        checkpoint_reserved,
        "Trading's ingest must have moved the checkpoint"
    );
}

/// Custody producer instruction, but asks Trading to ingest the resulting
/// Reserve receipt as a Rollback receipt. Custody therefore runs to completion
/// before Trading reaches the action join and refuses. The absent escrow,
/// reservation records, and checkpoint mutation prove transaction-wide rollback
/// at the seam this atomic shape exists to protect.
#[tokio::test]
async fn a_late_trading_refusal_rolls_the_atomic_reservation_back_whole() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    prepare_through_evaluation(&mut context, &scenario).await;
    let delivery = &scenario.delivery;
    let payer = context.payer.pubkey();
    context.set_account(
        &delivery.source,
        &AccountSharedData::from(data_account(
            delivery.token_program,
            delivery.source_prereservation_bytes(),
        )),
    );
    context.set_account(
        &delivery.escrow,
        &AccountSharedData::from(Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let mut bundle = reservation_bundle(&scenario, payer);
    bundle
        .instructions
        .get_mut(1)
        .expect("Trading ingest instruction")
        .data = DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1.to_vec();
    let checkpoint_before = checkpoint_body(&mut context, &scenario).await;
    let receipt = reservation_receipt_address(&scenario, 0);
    let batch = dealer_scenario_reservation_batch_address_v1(
        scenario.waist.custody_program,
        scenario.checkpoint,
    );

    let processed = submit_reservation_bundle(&mut context, &bundle, payer).await;

    // Instruction ONE, not instruction zero: Custody's producer accepted and
    // Trading refused the contradictory action only after authenticating the
    // Custody-owned receipt.
    assert_eq!(
        processed.result,
        Err(TransactionError::InstructionError(
            1,
            solana_sdk::instruction::InstructionError::Custom(TradingSbfError::Transition as u32),
        )),
        "TradingSbfError::Transition, at the ingest, not the producer: {:?}",
        processed.metadata.as_ref().map(|value| &value.log_messages)
    );
    let logs = processed
        .metadata
        .as_ref()
        .map(|value| value.log_messages.clone())
        .unwrap_or_default();
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {} success", scenario.waist.custody_program)),
        "Custody must have produced the reservation before Trading refused it: {logs:?}"
    );
    // And the transaction rolled back whole, which is the property the atomic
    // shape exists for: no escrow, no reservation.
    assert!(
        account_body(&mut context, delivery.escrow).await.is_none(),
        "a refused bundle leaves no escrow behind"
    );
    assert!(
        account_body(&mut context, delivery.reservation_state)
            .await
            .is_none(),
        "a refused bundle leaves no reservation behind"
    );
    assert!(
        account_body(&mut context, receipt).await.is_none(),
        "a refused bundle leaves no reservation receipt behind"
    );
    assert!(
        account_body(&mut context, batch).await.is_none(),
        "a refused bundle leaves no reservation batch behind"
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        checkpoint_before,
        "a refused ingest leaves the checkpoint byte-for-byte unchanged"
    );
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
) -> (
    ReservationEvidence,
    Vec<u8>,
    DealerScenarioCheckpointJournalV1,
) {
    evaluated_with_published_delta(context, scenario, None).await
}

/// The same, over a delta the evaluator chose to publish and seal.
async fn evaluated_with_published_delta(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    published_delta: Option<Vec<u8>>,
) -> (
    ReservationEvidence,
    Vec<u8>,
    DealerScenarioCheckpointJournalV1,
) {
    let journal =
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
    let (reservation, _, _) = evaluated_with_reservation_evidence(&mut context, &scenario).await;
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
        root: scenario.child_root,
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
        scenario.child_root,
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
    let dealer_after = position_balances(
        &mut context,
        &scenario,
        scenario.fixture.actor_position.account,
    )
    .await;
    let counterparty_after = position_balances(
        &mut context,
        &scenario,
        scenario.fixture.reserve_position.account,
    )
    .await;
    let acquired = 10_u64;
    for claim in 0..usize::try_from(WIDTH).expect("small width") {
        let dealer_before = *scenario.live.dealer_balances.get(claim).expect("before");
        let counterparty_before = *scenario
            .live
            .counterparty_balances
            .get(claim)
            .expect("before");
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
            assert_eq!(
                dealer_now, dealer_before,
                "coordinate {claim} must not move"
            );
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
    let instruction_data = instruction.data.clone();
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
    // A v0 transaction with an Address Lookup Table has no serializer in this
    // package's dependency set, and `wire_bytes: None` is the emitter's own word
    // for "this campaign did not measure". It is NOT a claim that the frame
    // fits, which is why the packet witness excludes the unmeasured rather than
    // taking a max that would silently drop them.
    let wire_bytes = None;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    record_campaign_transaction(&signature, slot, wire_bytes, &instruction_data, &processed);
    Ok(processed)
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
fn commit_table_addresses(bank: &DealerScenarioCommitAccountsV1, payer: Pubkey) -> Vec<Pubkey> {
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
    for proof in bank
        .effect_accounts
        .iter()
        .take(usize::from(bank.effect_count))
    {
        addresses.extend([proof.reservation_receipt, proof.reservation_state]);
    }
    addresses.retain(|key| *key != payer && *key != TRADING);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

/// Build, table and submit one commit, returning what the chain reported.
async fn submit_commit(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    bank: DealerScenarioCommitAccountsV1,
) -> solana_program_test::BanksTransactionResultWithMetadata {
    let _ = scenario;
    let payer = context.payer.pubkey();
    let addresses = commit_table_addresses(&bank, payer);
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
    let mut batch =
        DealerScenarioReservationBatchV1::decode(&original.data).expect("canonical batch decodes");
    batch.receipt_digests =
        core::array::from_fn(|index| if index == 0 { [0x7c; 32] } else { [0_u8; 32] });
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

/// Drive one scenario all the way to a committed checkpoint.
///
/// Returns the commit bank and the live table it was carried on, so a case can
/// resubmit the very same transaction against the new state.
async fn drive_to_committed(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> (DealerScenarioCommitAccountsV1, Pubkey, Vec<Pubkey>) {
    // The reservation every committed case is built on is CUSTODY-PRODUCED: the
    // batch, state and receipt commit reads are the bodies Custody's own reserve
    // route wrote, and the collateral it spends was really moved into escrow by
    // that route rather than staged there.
    prepare_through_evaluation(context, scenario).await;
    let reservation = reserve_through_custody(context, scenario).await;
    let payer = context.payer.pubkey();
    let receipt_address = dealer_scenario_evaluation_receipt_address_v1(
        TRADING,
        scenario.checkpoint,
        scenario.request_digest,
    );
    let bank = commit_bank(scenario, payer, receipt_address, &reservation);
    let addresses = commit_table_addresses(&bank, payer);
    let packet = build_dealer_scenario_commit_v1(
        bank.clone(),
        Hash::default(),
        &[OperatorLookupTable {
            key: Pubkey::new_from_array([0x7b; 32]),
            addresses: addresses.clone(),
        }],
    )
    .expect("commit packet");
    let table = create_live_lookup_table(context, &addresses).await;
    submit_v0(context, packet.instruction, table, &addresses)
        .await
        .expect("ProgramTest processing")
        .result
        .expect("the commit must land");
    (bank, table, addresses)
}

#[tokio::test]
async fn a_committed_checkpoint_is_never_cleaned_back_to_its_beneficiary() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    drive_to_committed(&mut context, &scenario).await;
    let payer = context.payer.pubkey();
    let committed = checkpoint_body(&mut context, &scenario).await;

    // Expiry is reached and the beneficiary is the immutable one named at
    // creation, so nothing about the cleanup frame is wrong. What refuses is
    // the phase: a committed checkpoint is not abandoned state whose rent can
    // be swept, because Custody delivery is a later permissionless effect that
    // still refers to it. The abandonment path and the committed path are two
    // different endings, and only one of them returns the rent.
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
        "a committed checkpoint must refuse cleanup; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        committed,
        "a refused cleanup must leave the committed checkpoint intact"
    );
    assert!(
        context
            .banks_client
            .get_account(scenario.checkpoint)
            .await
            .expect("checkpoint query")
            .is_some_and(|account| account.lamports > 0),
        "the committed checkpoint keeps its rent"
    );
}

#[tokio::test]
async fn a_committed_checkpoint_refuses_a_replayed_commit() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let (bank, table, addresses) = drive_to_committed(&mut context, &scenario).await;
    let committed = checkpoint_body(&mut context, &scenario).await;
    let dealer_after = position_balances(
        &mut context,
        &scenario,
        scenario.fixture.actor_position.account,
    )
    .await;

    // Byte-identical replay of the transaction that just landed. Nothing about
    // its frame has changed; the checkpoint has.
    let packet = build_dealer_scenario_commit_v1(
        bank,
        Hash::default(),
        &[OperatorLookupTable {
            key: Pubkey::new_from_array([0x7b; 32]),
            addresses: addresses.clone(),
        }],
    )
    .expect("commit packet");
    let processed = submit_v0(&mut context, packet.instruction, table, &addresses)
        .await
        .expect("ProgramTest processing");
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_TRANSITION),
        "a replayed commit must refuse on the phase; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        committed,
        "a refused replay must leave the checkpoint intact"
    );
    assert_eq!(
        position_balances(
            &mut context,
            &scenario,
            scenario.fixture.actor_position.account
        )
        .await,
        dealer_after,
        "a refused replay must not move the Claims Positions a second time"
    );
}

/// Reach a reserved checkpoint and hand back the bank a commit would use.
async fn reserved_commit_bank(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> DealerScenarioCommitAccountsV1 {
    let (reservation, receipt_address, _) = reserved_with_commit_inputs(context, scenario).await;
    let payer = context.payer.pubkey();
    commit_bank(scenario, payer, receipt_address, &reservation)
}

#[tokio::test]
async fn a_commit_refuses_a_caller_authority_from_another_request() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let mut bank = reserved_commit_bank(&mut context, &scenario).await;
    let reserved = checkpoint_body(&mut context, &scenario).await;

    // A real Trading PDA, derived through the real seed constructor, differing
    // only in the claims-packet digest it commits to. A random key would have
    // been refused for not being a program address at all; this one is a
    // perfectly good authority for a request that is not this one.
    let request = DealerScenarioTradeRequestV3::decode(&scenario.request_bytes)
        .expect("canonical request decodes");
    let plan = SignedDeltaPlanV3::decode(request.claims_packet()).expect("claims plan decodes");
    let mut foreign_packet = request.claims_packet().to_vec();
    *foreign_packet.last_mut().expect("packet is not empty") ^= 0xff;
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).expect("release set"),
        plan.market(),
        ExecutionRoleV1::Trading,
        plan.request_id(),
        hash(&foreign_packet).to_bytes(),
    )
    .expect("caller authority seeds");
    let foreign = Pubkey::find_program_address(&seeds.as_slices(), &TRADING).0;
    *bank
        .claims_accounts
        .first_mut()
        .expect("caller authority coordinate") = AccountMeta::new_readonly(foreign, false);

    let processed = submit_commit(&mut context, &scenario, bank).await;
    assert_eq!(
        custom_code(&processed.result),
        Some(TRADING_RELEASE),
        "an authority bound to another request must refuse; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        reserved,
        "a refused commit must not advance the checkpoint"
    );
}

#[tokio::test]
async fn a_commit_refuses_a_claims_position_table_out_of_canonical_order() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let mut bank = reserved_commit_bank(&mut context, &scenario).await;
    let reserved = checkpoint_body(&mut context, &scenario).await;
    let dealer_before = position_balances(
        &mut context,
        &scenario,
        scenario.fixture.actor_position.account,
    )
    .await;

    // Both Positions are the real ones this request names, with their real
    // bodies and privileges. Only the order changes. Claims recomputes the
    // table sorted by owner, so the frame it is handed must already be in that
    // order and cannot be permuted by a caller.
    let count = bank.claims_accounts.len();
    bank.claims_accounts.swap(count - 2, count - 1);

    let processed = submit_commit(&mut context, &scenario, bank).await;
    assert!(
        processed.result.is_err(),
        "a permuted Position table must fail closed; observed {:?}",
        processed.result
    );
    assert_eq!(
        checkpoint_body(&mut context, &scenario).await,
        reserved,
        "a refused commit must not advance the checkpoint"
    );
    assert_eq!(
        position_balances(
            &mut context,
            &scenario,
            scenario.fixture.actor_position.account
        )
        .await,
        dealer_before,
        "a refused commit must not move the Claims Positions"
    );
}

/// Read one account's exact body, or nothing when it does not exist.
async fn account_body(context: &mut ProgramTestContext, key: Pubkey) -> Option<Vec<u8>> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .map(|account| account.data)
}

/// Read one account's lamports, treating an absent account as zero.
async fn account_lamports(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .map_or(0, |account| account.lamports)
}

/// Drive the whole accepted transition to a committed checkpoint.
///
/// Delivery is not reachable any other way: `activate_batch` refuses every
/// checkpoint whose phase is not `Committed`, so the delivery leg is chained
/// after the executed commit rather than staged beside it.
async fn committed_with_delivery_inputs(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
) -> ReservationEvidence {
    let (reservation, receipt_address, _reserved) =
        reserved_with_commit_inputs(context, scenario).await;
    let bank = commit_bank(
        scenario,
        context.payer.pubkey(),
        receipt_address,
        &reservation,
    );
    let processed = submit_commit(context, scenario, bank).await;
    processed
        .result
        .expect("the commit must land before anything can be delivered");
    reservation
}

/// The exact activation account bank for this scenario's single effect.
fn activation_bank(
    scenario: &Scenario,
    reservation: &ReservationEvidence,
    payer: Pubkey,
) -> DealerScenarioActivationAccountsV1 {
    let delivery = &scenario.delivery;
    DealerScenarioActivationAccountsV1 {
        custody_program: scenario.waist.custody_program,
        market: scenario.core_market,
        activation_cache: scenario.waist.activation_cache,
        registry_program: scenario.waist.registry,
        trading_program: TRADING,
        trading_programdata: scenario.waist.trading_programdata,
        realm: scenario.realm.raw,
        realm_staging: scenario.realm.staging,
        custody_replay: delivery.replay,
        checkpoint: scenario.checkpoint,
        effect_producer: TRADING,
        effect_manifest: EFFECTS,
        batch: reservation.batch,
        activation_receipt: delivery.activation_receipt,
        mint: delivery.mint,
        custody_authority: delivery.custody_authority,
        token_program: delivery.token_program,
        payer,
        refund_beneficiary: BENEFICIARY,
        rent: sysvar::rent::ID,
        system_program: system_program::ID,
        effects: core::array::from_fn(|index| {
            if index == 0 {
                DealerScenarioActivationEffectAccountsV1 {
                    effect_body: EFFECT_BODY,
                    reservation_state: delivery.reservation_state,
                    escrow: delivery.escrow,
                    destination: delivery.destination,
                }
            } else {
                DealerScenarioActivationEffectAccountsV1::default()
            }
        }),
        effect_count: 1,
    }
}

/// Every address the activation packet resolves, minus the payer's own.
fn activation_table_addresses(
    bank: &DealerScenarioActivationAccountsV1,
    payer: Pubkey,
) -> Vec<Pubkey> {
    let mut addresses = vec![
        bank.market,
        bank.activation_cache,
        bank.registry_program,
        bank.trading_program,
        bank.trading_programdata,
        bank.realm,
        bank.realm_staging,
        bank.custody_replay,
        bank.checkpoint,
        bank.effect_manifest,
        bank.batch,
        bank.activation_receipt,
        bank.mint,
        bank.custody_authority,
        bank.token_program,
        bank.refund_beneficiary,
        bank.rent,
        bank.system_program,
    ];
    for effect in bank.effects.iter().take(usize::from(bank.effect_count)) {
        addresses.extend([
            effect.effect_body,
            effect.reservation_state,
            effect.escrow,
            effect.destination,
        ]);
    }
    addresses.retain(|key| *key != payer);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

/// Build, table and submit one delivery, returning what the chain reported.
async fn submit_activation(
    context: &mut ProgramTestContext,
    bank: DealerScenarioActivationAccountsV1,
) -> solana_program_test::BanksTransactionResultWithMetadata {
    let payer = context.payer.pubkey();
    let addresses = activation_table_addresses(&bank, payer);
    let packet = build_dealer_scenario_activation_v1(
        bank,
        Hash::default(),
        &[OperatorLookupTable {
            key: Pubkey::new_from_array([0x7c; 32]),
            addresses: addresses.clone(),
        }],
    )
    .expect("activation packet");
    let table = create_live_lookup_table(context, &addresses).await;
    submit_v0(context, packet.instruction, table, &addresses)
        .await
        .expect("ProgramTest processing")
}

#[tokio::test]
async fn the_delivery_moves_the_locked_collateral_and_closes_its_escrow() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let reservation = committed_with_delivery_inputs(&mut context, &scenario).await;
    let delivery = &scenario.delivery;

    // What the chain holds before delivery, read back rather than assumed.
    let escrow_before = account_body(&mut context, delivery.escrow)
        .await
        .expect("the escrow exists while value is locked");
    let destination_before = account_body(&mut context, delivery.destination)
        .await
        .expect("the destination exists");
    let source_before = account_body(&mut context, delivery.source)
        .await
        .expect("the source vault exists");
    let source_before_bytes = source_before.clone();
    let mint_supply = mint_total_supply(
        &account_body(&mut context, delivery.mint)
            .await
            .expect("the collateral Mint exists"),
    );
    let escrow_rent = account_lamports(&mut context, delivery.escrow).await;
    let beneficiary_before = account_lamports(&mut context, BENEFICIARY).await;
    assert_eq!(token_account_amount(&escrow_before), DELIVERY_AMOUNT);
    assert_eq!(
        token_account_amount(&destination_before),
        DELIVERY_DESTINATION_BEFORE
    );
    let replay_before = CustodyReplayV1::decode(
        &account_body(&mut context, delivery.replay)
            .await
            .expect("the replay cursor exists"),
    )
    .expect("canonical replay cursor");
    assert_eq!(replay_before.next_revision, DELIVERY_REPLAY_REVISION);
    assert!(
        account_body(&mut context, delivery.activation_receipt)
            .await
            .is_none_or(|body| body.is_empty()),
        "the activation receipt must be vacant before delivery"
    );

    let payer = context.payer.pubkey();
    let processed = submit_activation(
        &mut context,
        activation_bank(&scenario, &reservation, payer),
    )
    .await;
    processed
        .result
        .as_ref()
        .expect("the delivery must execute against the real Custody ELF");

    // Conservation: exactly what left the escrow arrived at the destination,
    // the escrow is gone, and the source vault the reservation already debited
    // is untouched by the delivery.
    let destination_after = account_body(&mut context, delivery.destination)
        .await
        .expect("the destination survives delivery");
    assert_eq!(
        token_account_amount(&destination_after),
        DELIVERY_DESTINATION_BEFORE
            .checked_add(DELIVERY_AMOUNT)
            .expect("credited destination"),
        "the destination is credited exactly the locked amount"
    );
    assert_eq!(
        account_body(&mut context, delivery.escrow).await,
        None,
        "the escrow is closed, not merely emptied"
    );
    assert_eq!(
        account_body(&mut context, delivery.source).await,
        Some(source_before),
        "delivery does not touch the source vault the reservation debited"
    );
    assert_eq!(
        account_lamports(&mut context, BENEFICIARY).await,
        beneficiary_before
            .checked_add(escrow_rent)
            .expect("refunded rent"),
        "every lamport of escrow rent reaches the beneficiary fixed at reservation"
    );
    // The whole claim in one line: no collateral was created and none was
    // destroyed. Every atom the Mint issued is still held by an account this
    // scenario names, before and after.
    let held_before = DELIVERY_SOURCE_AFTER
        .checked_add(DELIVERY_AMOUNT)
        .and_then(|value| value.checked_add(DELIVERY_DESTINATION_BEFORE))
        .expect("collateral held before delivery");
    let held_after = token_account_amount(&source_before_bytes)
        .checked_add(token_account_amount(&destination_after))
        .expect("collateral held after delivery");
    assert_eq!(
        held_before, held_after,
        "delivery moves collateral between accounts and creates none"
    );
    assert_eq!(
        held_after, mint_supply,
        "and every atom the Mint issued is still held by an account this scenario names"
    );

    // The replay cursor advanced exactly once, and the batch is terminal.
    let replay_after = CustodyReplayV1::decode(
        &account_body(&mut context, delivery.replay)
            .await
            .expect("the replay cursor survives delivery"),
    )
    .expect("canonical replay cursor");
    assert_eq!(
        replay_after.next_revision,
        DELIVERY_REPLAY_REVISION
            .checked_add(1)
            .expect("advanced revision"),
        "one delivered effect advances the cursor exactly one revision"
    );
    let batch_after = DealerScenarioReservationBatchV1::decode(
        &account_body(&mut context, reservation.batch)
            .await
            .expect("the batch survives delivery"),
    )
    .expect("canonical batch");
    assert_eq!(
        batch_after.status,
        DealerScenarioReservationBatchStatusV1::Activated,
        "a delivered batch is terminal"
    );
    let state_after = DealerScenarioReservationStateV1::decode(
        &account_body(&mut context, delivery.reservation_state)
            .await
            .expect("the reservation state survives delivery"),
    )
    .expect("canonical reservation state");
    assert_eq!(
        state_after.status,
        DealerScenarioReservationStateStatusV1::Activated
    );
    assert_eq!(state_after.escrow_after, 0);

    // And the receipt exists, naming the checkpoint it delivered.
    let receipt = DealerScenarioActivationReceiptV1::decode(
        &account_body(&mut context, delivery.activation_receipt)
            .await
            .expect("delivery creates its receipt"),
    )
    .expect("canonical activation receipt");
    assert_eq!(receipt.checkpoint, scenario.checkpoint.to_bytes());
    assert_eq!(receipt.request_digest, scenario.request_digest);
    assert_eq!(receipt.batch, reservation.batch.to_bytes());
}

#[tokio::test]
async fn a_replayed_delivery_refuses_and_the_collateral_does_not_move_twice() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let reservation = committed_with_delivery_inputs(&mut context, &scenario).await;
    let delivery = &scenario.delivery;
    let payer = context.payer.pubkey();

    submit_activation(
        &mut context,
        activation_bank(&scenario, &reservation, payer),
    )
    .await
    .result
    .expect("the first delivery executes");
    let destination_once = account_body(&mut context, delivery.destination)
        .await
        .expect("the destination survives the first delivery");
    let replay_once = account_body(&mut context, delivery.replay)
        .await
        .expect("the cursor survives the first delivery");
    let batch_once = account_body(&mut context, reservation.batch)
        .await
        .expect("the batch survives the first delivery");

    // Byte-identical resubmission. The activation receipt is no longer vacant,
    // which is the anti-replay gate for the whole route; behind it the batch is
    // already Activated and the cursor has already advanced, so the case would
    // refuse three times over. It reaches the first of them.
    let processed = submit_activation(
        &mut context,
        activation_bank(&scenario, &reservation, payer),
    )
    .await;
    assert!(
        processed.result.is_err(),
        "a replayed delivery must fail closed; observed {:?}",
        processed.result
    );
    assert_eq!(
        account_body(&mut context, delivery.destination).await,
        Some(destination_once),
        "a refused delivery must not credit the destination a second time"
    );
    assert_eq!(
        account_body(&mut context, delivery.replay).await,
        Some(replay_once),
        "a refused delivery must not advance the replay cursor"
    );
    assert_eq!(
        account_body(&mut context, reservation.batch).await,
        Some(batch_once),
        "a refused delivery must not touch the batch it already delivered"
    );
    assert_eq!(
        account_body(&mut context, delivery.escrow).await,
        None,
        "the escrow stays closed"
    );
}

/// One Custody-owned body from the staged reservation evidence.
fn staged_body(reservation: &ReservationEvidence, key: Pubkey) -> Vec<u8> {
    reservation
        .installed
        .iter()
        .find(|(installed, _)| *installed == key)
        .map(|(_, body)| body.clone())
        .expect("the reservation staged this body")
}

/// Re-seal the locked batch around a lie, through its own codec.
///
/// A hostile that edits a body and leaves the digests that commit to it alone is
/// answered by the shallower digest check and never reaches the check it claims
/// to be about. Every case below re-seals, so the case is the case it names.
fn resealed_batch(
    reservation: &ReservationEvidence,
    edit: impl FnOnce(&mut DealerScenarioReservationBatchV1),
) -> Vec<u8> {
    let mut batch =
        DealerScenarioReservationBatchV1::decode(&staged_body(reservation, reservation.batch))
            .expect("canonical batch");
    edit(&mut batch);
    batch.encode().expect("the batch re-encodes").to_vec()
}

/// Re-seal the reservation state around a lie, through its own codec.
fn resealed_state(
    reservation: &ReservationEvidence,
    edit: impl FnOnce(&mut DealerScenarioReservationStateV1),
) -> Vec<u8> {
    let mut state = DealerScenarioReservationStateV1::decode(&staged_body(
        reservation,
        reservation.reservation_state,
    ))
    .expect("canonical reservation state");
    edit(&mut state);
    state.encode().expect("the state re-encodes").to_vec()
}

/// Overwrite one Custody-owned account with an exact body.
fn restage(context: &mut ProgramTestContext, custody: Pubkey, key: Pubkey, body: Vec<u8>) {
    context.set_account(&key, &AccountSharedData::from(data_account(custody, body)));
}

#[tokio::test]
async fn a_replay_cursor_at_the_wrong_revision_refuses_inside_the_delivery() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let reservation = committed_with_delivery_inputs(&mut context, &scenario).await;
    let delivery = &scenario.delivery;
    let custody = scenario.waist.custody_program;

    // The lie: the cursor stands one revision ahead of what the effect's own
    // request expects. Sealed: the batch's pinned replay prestate digest is
    // recomputed over the tampered cursor, so the batch check that would
    // otherwise answer this case passes, and the refusal has to come from
    // CustodyReplayV1::advance comparing the revision to the request itself.
    let mut cursor = CustodyReplayV1::decode(&delivery.replay_bytes).expect("canonical cursor");
    cursor.next_revision = DELIVERY_REPLAY_REVISION
        .checked_add(1)
        .expect("advanced revision");
    let cursor_bytes = cursor.to_bytes().expect("the cursor re-encodes").to_vec();
    assert_ne!(
        cursor_bytes, delivery.replay_bytes,
        "the lie must actually change the cursor"
    );
    restage(&mut context, custody, delivery.replay, cursor_bytes.clone());

    // Control, so the seal below is demonstrably load-bearing rather than
    // decorative: with the cursor changed and the batch's digest left alone, the
    // batch answers the case and the token program is never invoked. This is the
    // shallow version of this hostile, and it is worthless.
    let payer = context.payer.pubkey();
    let unsealed = submit_activation(
        &mut context,
        activation_bank(&scenario, &reservation, payer),
    )
    .await;
    assert!(unsealed.result.is_err(), "the unsealed lie also refuses");
    assert!(
        !invoked_programs(&unsealed).contains(&delivery.token_program),
        "the unsealed lie is answered by the batch, before any collateral moves"
    );

    restage(
        &mut context,
        custody,
        reservation.batch,
        resealed_batch(&reservation, |batch| {
            batch.replay_prestate_digest = hash(&cursor_bytes).to_bytes();
        }),
    );

    let destination_before = account_body(&mut context, delivery.destination)
        .await
        .expect("the destination exists");
    let processed = submit_activation(
        &mut context,
        activation_bank(&scenario, &reservation, payer),
    )
    .await;
    assert!(
        processed.result.is_err(),
        "a cursor at the wrong revision must fail closed; observed {:?}",
        processed.result
    );
    assert_eq!(
        custom_code(&processed.result.clone().map(|_| ())),
        Some(CUSTODY_REPLAY),
        "the refusal is a replay refusal"
    );
    // Depth, not just refusal: the token program actually ran. Every shallower
    // gate -- the batch's replay prestate digest, the activation identities, the
    // effect join, the reservation's own coordinates -- sits before the
    // transfer, so a case answered by any of them could never have reached it.
    // Only `advance` refuses after the collateral has already moved.
    assert!(
        invoked_programs(&processed).contains(&scenario.delivery.token_program),
        "the case must reach past the transfer to be about the cursor at all"
    );
    // The transfer happens before the cursor advances, so this refusal unwinds a
    // token movement the runtime had already made. Nothing survives it.
    assert_eq!(
        account_body(&mut context, delivery.destination).await,
        Some(destination_before),
        "a refused delivery must not credit the destination"
    );
    assert!(
        account_body(&mut context, delivery.escrow).await.is_some(),
        "a refused delivery must not close the escrow"
    );
    assert_eq!(
        account_body(&mut context, delivery.replay).await,
        Some(cursor_bytes),
        "a refused delivery leaves the cursor exactly as it found it"
    );
}

#[tokio::test]
async fn a_substituted_destination_refuses_on_the_owner_the_request_names() {
    let scenario = scenario();
    let mut context = program_test(&scenario).start_with_context().await;
    let reservation = committed_with_delivery_inputs(&mut context, &scenario).await;
    let delivery = &scenario.delivery;
    let custody = scenario.waist.custody_program;

    // A third distinct identity, per the craft note: a substitution the frame
    // answers with a key comparison is not a substitution of anything. This is a
    // real token account, of the Realm's own Mint, at the destination's exact
    // balance -- differing only in who owns it.
    let intruder_owner = Pubkey::new_from_array([0xe9; 32]);
    let intruder = Pubkey::new_from_array([0xea; 32]);
    let intruder_bytes = dealer_delivery_token_account_bytes(
        delivery.mint,
        intruder_owner,
        DELIVERY_DESTINATION_BEFORE,
    );
    context.set_account(
        &intruder,
        &AccountSharedData::from(data_account(delivery.token_program, intruder_bytes.clone())),
    );
    // Sealed: the reservation names the intruder and commits to its prestate, so
    // neither the reservation's key comparison nor its destination prestate
    // digest can answer this case. What is left is the only thing that should
    // refuse it -- the external destination owner the request itself names.
    restage(
        &mut context,
        custody,
        reservation.reservation_state,
        resealed_state(&reservation, |state| {
            state.destination = intruder.to_bytes();
            state.destination_prestate_digest = hash(&intruder_bytes).to_bytes();
        }),
    );

    let payer = context.payer.pubkey();
    let mut bank = activation_bank(&scenario, &reservation, payer);
    bank.effects
        .get_mut(0)
        .expect("the zeroth effect")
        .destination = intruder;
    let processed = submit_activation(&mut context, bank).await;
    assert!(
        processed.result.is_err(),
        "a destination the request does not name must fail closed; observed {:?}",
        processed.result
    );
    // Depth: every reservation-join refusal is coded Replay, so a token-state
    // refusal proves the case cleared them all and was answered by the external
    // destination owner the request itself names.
    assert_eq!(
        custom_code(&processed.result.clone().map(|_| ())),
        Some(CUSTODY_TOKEN_STATE),
        "the refusal is the token-state gate, not a shallower reservation join"
    );
    assert_eq!(
        account_body(&mut context, intruder).await,
        Some(intruder_bytes),
        "a refused delivery must not credit the substituted account"
    );
    assert_eq!(
        token_account_amount(
            &account_body(&mut context, delivery.destination)
                .await
                .expect("the real destination survives")
        ),
        DELIVERY_DESTINATION_BEFORE,
        "nor the real one"
    );
    assert!(
        account_body(&mut context, delivery.escrow).await.is_some(),
        "and the escrow stays locked"
    );
}

/// Every program the runtime actually invoked, read out of the transaction log.
fn invoked_programs(
    processed: &solana_program_test::BanksTransactionResultWithMetadata,
) -> Vec<Pubkey> {
    processed
        .metadata
        .as_ref()
        .map(|metadata| metadata.log_messages.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|line| {
            line.strip_prefix("Program ")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|key| key.parse::<Pubkey>().ok())
        })
        .collect()
}

/// Real-ELF selector-7/8 lifecycle evidence built from the sole mixed Dealer
/// ProgramSet. Kept as a private campaign module so the older scenario
/// transcript stays readable while both paths share the same waist and Product.
mod lp_lifecycle {
    use super::*;

    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        CompartmentFundingV1, ContentId as ManifestContentId, FundingAmountsV1, FundingQuoteV1,
        MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_capability_program_contract::{
        CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
        hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        set_v2::{
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
            CapabilityProgramSetV2,
        },
        v4::{
            CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
            SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_V4,
        },
    };
    use dclutch_chain_bundle_builder::{
        BuilderError, WaistFactsV1,
        admitted::AdmittedAotInputV1,
        artifacts::{ArtifactSetV1, DerivedRecordV1},
        bundle::{
            BundleInputV1, FixedCorpusV1, ScenarioV1, build_admitted_bundle,
            build_admitted_bundle_with_candidate_v1,
        },
        frame::{BuiltAccountV1, data_account as built_data_account, program, vacant},
        registers::DerivedInvocationV1,
        routes::derive_authority,
    };
    use dclutch_custody_contract::{
        CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1,
        CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplaySeedsV1, CustodyReplayV1,
        CustodyRequestV1, CustodyVaultSeedsV1, DelegatedCustodyRequestV2, OperationV1,
    };
    use dclutch_dealer_codec::{
        Phase,
        config_v4::DealerConfigV4,
        root_tail::{ROOT_TAIL_BYTES, RootTail},
    };
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
        StrategyDispositionV2,
    };
    use dclutch_market_core_codec::{CoreState, Identity as CoreIdentity};
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_release_set_contract::{ArtifactReleaseIdV1, CapabilityExecutionSelectionV1};
    use dclutch_rent_contract::{
        RefundAuthority,
        lifecycle_v2::{
            LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            LifecycleAccountIdV2, LifecycleRentCreditV2,
        },
    };
    use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_V1;
    use dclutch_token_svm::ACCOUNT_BYTES as TOKEN_ACCOUNT_BYTES;
    use dclutch_trading_sbf::dealer::{
        v3_artifacts::{
            dealer_equity_request_profile_bytes_v3, dealer_equity_transition_bytes_v3,
            encode_dealer_equity_request_profile_v3, encode_dealer_equity_transition_v3,
        },
        v3_equity::{
            PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3, PoolEquityPlanV3,
            PoolEquityRedemptionV3, preflight_pool_equity_v3,
        },
        v3_equity_operator::{
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3, DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3,
            DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3, DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
            DEALER_EQUITY_REDEEM_P1_SELECTOR_V3, DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
            DealerEquityRequestV3, EquityPoolChainProjectionV3, EquityRequestIntentV3,
            build_equity_request_v3, prepare_equity_request_v3,
        },
        v3_hot_artifact::{
            dealer_equity_evidence_owner_identity_register_v3, dealer_equity_identity_count_v3,
            dealer_equity_scalar_count_v3, project_dealer_equity_hot_registers_v3,
        },
        v3_lp_artifacts::{
            DEALER_LP_OBLIGATION_ACCOUNT_V3, DealerLpAccountProfileInputV3,
            dealer_lp_account_count_v3, dealer_lp_transition_bytes_v3,
            encode_dealer_lp_account_profile_v3, encode_dealer_lp_request_profile_v3,
            encode_dealer_lp_transition_v3,
        },
        v3_multi_lp::{
            DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpPositionV3,
            MAX_MULTI_LP_CUSTODY_EFFECTS_V3, MultiLpActionV3, MultiLpCollateralFrameV3,
            MultiLpContextV3, MultiLpCustodyRequestV3,
        },
        v3_obligation::{
            DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3,
            stage_equity_share_supply_v3,
        },
        v3_operator::{MultiLpChainProjectionV3, MultiLpRequestActionV3},
        v3_profile::{
            DealerEquityAccountProfileInputV3, dealer_equity_logical_account_count_v3,
            encode_dealer_equity_account_profile_v3,
        },
        v3_trade_artifacts::{
            DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4, DEALER_SCENARIO_TRANSITION_BYTES_V4,
            dealer_scenario_base_effect_program_bytes_v4, dealer_scenario_effect_program_bytes_v4,
            encode_dealer_scenario_base_effect_program_v4,
            encode_dealer_scenario_effect_program_v4, encode_dealer_scenario_request_profile_v4,
            encode_dealer_scenario_transition_v4,
        },
        v3_trade_profile::{
            DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DealerScenarioAccountProfileInputV4,
            dealer_scenario_logical_frame_v4, encode_dealer_scenario_account_profile_v4_atomic,
        },
        v4_equity_release::{
            DEALER_EQUITY_LIFECYCLE_BYTES_V5, DealerEquityFinalizedArtifactsV4,
            dealer_equity_effect_bytes_v4, encode_dealer_equity_effect_v4,
            encode_dealer_equity_lifecycle_v5, finalize_dealer_equity_descriptor_v4,
        },
        v4_lp_operator::{build_close_lp_v4, build_open_lp_v4},
        v4_lp_release::{
            DEALER_LP_LIFECYCLE_BYTES_V5, DealerLpFinalizedArtifactsV4, dealer_lp_effect_bytes_v4,
            encode_dealer_lp_effect_v4, encode_dealer_lp_lifecycle_v5,
            finalize_dealer_lp_descriptor_v4,
        },
        v4_scenario_operator::build_scenario_trade_request_v4,
        v4_scenario_release::{
            DEALER_GLOBAL_PROGRAM_SET_BYTES_V4, DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5,
            DealerDescriptorRecordV4, DealerScenarioFinalizedArtifactsV4,
            encode_dealer_global_program_set_v4, encode_dealer_scenario_empty_lifecycle_v5,
            finalize_dealer_scenario_descriptor_v4,
        },
    };
    use solana_clock::Clock;
    use solana_program_option::COption as ProgramCOption;
    use solana_program_pack::Pack;
    use spl_token_interface::state::{Account as SplTokenAccount, AccountState as SplAccountState};

    const ACCELERATOR: Pubkey = Pubkey::new_from_array([0xe8; 32]);

    fn core_id(bytes: [u8; 32]) -> dclutch_core_contract::ContentId {
        dclutch_core_contract::ContentId::new(bytes).expect("nonzero core identity")
    }

    fn manifest_id(bytes: [u8; 32]) -> ManifestContentId {
        ManifestContentId::new(bytes).expect("nonzero manifest identity")
    }

    fn record_bumps(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (u8, u8) {
        (
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).1,
            Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .1,
        )
    }

    fn loader_program_body(programdata: Pubkey) -> Vec<u8> {
        let mut bytes = vec![0_u8; dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES];
        bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        bytes[4..36].copy_from_slice(programdata.as_ref());
        bytes
    }

    fn deployment_account(key: Pubkey, data: Vec<u8>, executable: bool) -> BuiltAccountV1 {
        BuiltAccountV1 {
            key,
            account: Account {
                lamports: Rent::default().minimum_balance(data.len()).max(1),
                data,
                owner: bpf_loader_upgradeable::ID,
                executable,
                rent_epoch: 0,
            },
            observed: None,
        }
    }

    struct LpArtifacts {
        descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
        profile: Vec<u8>,
        request_profile: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        lifecycle: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
    }

    impl LpArtifacts {
        fn set<'a>(
            &'a self,
            program_set: &'a [u8],
            manifest: &'a [u8],
            config: &'a [u8],
        ) -> ArtifactSetV1<'a> {
            ArtifactSetV1 {
                descriptor: &self.descriptor,
                account_profile: &self.profile,
                request_profile: &self.request_profile,
                transition: &self.transition,
                effect: &self.effect,
                lifecycle: &self.lifecycle,
                strategy: &self.strategy,
                program_set,
                manifest,
                config,
            }
        }
    }

    fn lp_artifacts(
        action: MultiLpRequestActionV3,
        logical_lengths: &[u32],
        release_id: ArtifactReleaseIdV1,
    ) -> LpArtifacts {
        let profile_input = DealerLpAccountProfileInputV3 {
            action,
            logical_data_lengths: logical_lengths,
        };
        let profile = encode_dealer_lp_account_profile_v3(profile_input).expect("LP profile");
        let mut lifecycle_scratch = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        let mut lifecycle = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        encode_dealer_lp_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)
            .expect("LP lifecycle");
        let mut request_scratch = vec![
                0_u8;
                dclutch_trading_sbf::dealer::v3_lp_artifacts::DEALER_LP_REQUEST_PROFILE_BYTES_V3
            ];
        let mut request_profile = vec![0_u8; request_scratch.len()];
        encode_dealer_lp_request_profile_v3(action, &mut request_scratch, &mut request_profile)
            .expect("LP request profile");
        let transition_bytes = dealer_lp_transition_bytes_v3(action);
        let mut transition_scratch = vec![0_u8; transition_bytes];
        let mut transition = vec![0_u8; transition_bytes];
        encode_dealer_lp_transition_v3(action, &mut transition_scratch, &mut transition)
            .expect("LP transition");
        let effect_bytes = dealer_lp_effect_bytes_v4(action);
        let mut effect_scratch = vec![0_u8; effect_bytes];
        let mut effect = vec![0_u8; effect_bytes];
        encode_dealer_lp_effect_v4(action, &mut effect_scratch, &mut effect).expect("LP effect");
        let certificate = ExecutionStrategyCertificateV2::new(
            core_id(hash(&profile).to_bytes()),
            core_id(REQUEST_PROFILE_SCHEMA_V1),
            core_id(hash(&request_profile).to_bytes()),
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(hash(&effect).to_bytes()),
            release_id,
            core_id([0xc1; 32]),
            core_id([0xc2; 32]),
            core_id([0xc3; 32]),
        )
        .to_bytes()
        .to_vec();
        let certificate_id = core_id(hash(&certificate).to_bytes());
        let admission = ExecutionStrategyAdmissionV2::new(certificate_id)
            .to_bytes()
            .to_vec();
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            Some(certificate_id),
            core_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            Some(core_id(hash(&admission).to_bytes())),
            core_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            core_id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("admitted strategy")
        .to_bytes()
        .to_vec();
        let descriptor = finalize_dealer_lp_descriptor_v4(DealerLpFinalizedArtifactsV4 {
            account_profile_input: profile_input,
            account_profile: &profile,
            lifecycle_policy: &lifecycle,
            capacity_profile: &[1],
            effect: &effect,
            request_profile: &request_profile,
            execution_strategy: &strategy,
            transition: &transition,
        })
        .expect("LP descriptor");
        LpArtifacts {
            descriptor,
            profile,
            request_profile,
            transition,
            effect,
            lifecycle,
            strategy,
            certificate,
            admission,
        }
    }

    /// Fully finalized successor artifacts for one physical equity selector.
    ///
    /// These are not placeholder descriptor bytes: profile, request,
    /// transition, V4 effect, lifecycle and admitted strategy are all derived
    /// through the same semantic owners the accelerator authenticates. The
    /// caller supplies the profile's exact observed frame widths, so this
    /// constructor keeps no opaque V3 descriptor authority in the mixed
    /// Dealer program set.
    struct EquityArtifacts {
        descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
        profile: Vec<u8>,
        request_profile: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        lifecycle: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
    }

    impl EquityArtifacts {
        fn set<'a>(
            &'a self,
            program_set: &'a [u8],
            manifest: &'a [u8],
            config: &'a [u8],
        ) -> ArtifactSetV1<'a> {
            ArtifactSetV1 {
                descriptor: &self.descriptor,
                account_profile: &self.profile,
                request_profile: &self.request_profile,
                transition: &self.transition,
                effect: &self.effect,
                lifecycle: &self.lifecycle,
                strategy: &self.strategy,
                program_set,
                manifest,
                config,
            }
        }
    }

    struct ScenarioArtifacts {
        descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
        profile: Vec<u8>,
        request_profile: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        lifecycle: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
    }

    impl ScenarioArtifacts {
        fn set<'a>(
            &'a self,
            program_set: &'a [u8],
            manifest: &'a [u8],
            config: &'a [u8],
        ) -> ArtifactSetV1<'a> {
            ArtifactSetV1 {
                descriptor: &self.descriptor,
                account_profile: &self.profile,
                request_profile: &self.request_profile,
                transition: &self.transition,
                effect: &self.effect,
                lifecycle: &self.lifecycle,
                strategy: &self.strategy,
                program_set,
                manifest,
                config,
            }
        }
    }

    fn equity_transfer(
        source_compartment: CompartmentV1,
        destination_compartment: CompartmentV1,
        marker: u8,
    ) -> CustodyRequestV1 {
        let source_external = source_compartment == CompartmentV1::External;
        let destination_external = destination_compartment == CompartmentV1::External;
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment,
            destination_compartment,
            release_set: [0x41; 32],
            market: [0x42; 32],
            realm: [0x43; 32],
            context: [0x44; 32],
            caller_program: TRADING.to_bytes(),
            semantic: ContextV1 {
                candidate: [0x45; 32],
                source_owner: if source_external { [0x46; 32] } else { [0; 32] },
                destination_owner: if destination_external {
                    [0x47; 32]
                } else {
                    [0; 32]
                },
                order: [0x48; 32],
                parent_request_digest: [0x49; 32],
                order_nonce: 1,
                generation: SCENARIO_GENERATION,
                page_index: 0,
                execution_index: 0,
                transfer_index: u16::from(marker),
            },
            source: [marker; 32],
            destination: [marker.saturating_add(1); 32],
            source_vault_context: if source_external { [0; 32] } else { [0x4a; 32] },
            destination_vault_context: if destination_external {
                [0; 32]
            } else {
                [0x4b; 32]
            },
            mint: [0x4c; 32],
            token_program: [0x4d; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 1,
            resulting_revision: 2,
            amount: 1,
            rent_lamports: 0,
        }
    }

    fn equity_templates(action: MultiLpActionV3) -> Vec<MultiLpCustodyRequestV3> {
        match action {
            MultiLpActionV3::Add => {
                let external = equity_transfer(
                    CompartmentV1::External,
                    CompartmentV1::TradingPrincipal,
                    0x51,
                );
                vec![
                    MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
                        custody: external,
                        starts_atomic_debit: true,
                        terminal: true,
                        delegate_before: [0x52; 32],
                        delegate_after: [0; 32],
                        total_debit: external.amount,
                        allowance_before: external.amount,
                        allowance_after: 0,
                    }),
                    MultiLpCustodyRequestV3::Canonical(equity_transfer(
                        CompartmentV1::HoardPrincipal,
                        CompartmentV1::TradingPrincipal,
                        0x53,
                    )),
                ]
            }
            MultiLpActionV3::Remove => vec![
                MultiLpCustodyRequestV3::Canonical(equity_transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::HoardPrincipal,
                    0x54,
                )),
                MultiLpCustodyRequestV3::Canonical(equity_transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::External,
                    0x55,
                )),
                MultiLpCustodyRequestV3::Canonical(equity_transfer(
                    CompartmentV1::HoardPrincipal,
                    CompartmentV1::TradingPrincipal,
                    0x56,
                )),
            ],
        }
    }

    fn scenario_templates() -> [MultiLpCustodyRequestV3; 6] {
        let pairs = [
            (CompartmentV1::External, CompartmentV1::TradingPrincipal),
            (CompartmentV1::External, CompartmentV1::FeeVault),
            (CompartmentV1::TradingPrincipal, CompartmentV1::FeeVault),
            (
                CompartmentV1::TradingPrincipal,
                CompartmentV1::HoardPrincipal,
            ),
            (
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
            ),
            (CompartmentV1::TradingPrincipal, CompartmentV1::External),
        ];
        core::array::from_fn(|slot| {
            let (source, destination) = pairs[slot];
            let request = equity_transfer(
                source,
                destination,
                0x61_u8.saturating_add(u8::try_from(slot).expect("scenario route ordinal")),
            );
            if slot < 2 {
                MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
                    custody: request,
                    starts_atomic_debit: true,
                    terminal: true,
                    delegate_before: [0x71; 32],
                    delegate_after: [0; 32],
                    total_debit: request.amount,
                    allowance_before: request.amount,
                    allowance_after: 0,
                })
            } else {
                MultiLpCustodyRequestV3::Canonical(request)
            }
        })
    }

    fn scenario_artifacts(
        profile_input: DealerScenarioAccountProfileInputV4,
        release_id: ArtifactReleaseIdV1,
    ) -> ScenarioArtifacts {
        let mut profile_scratch = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        let mut profile = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        encode_dealer_scenario_account_profile_v4_atomic(
            profile_input,
            &mut profile_scratch,
            &mut profile,
        )
        .expect("scenario Profile13");

        let mut lifecycle_scratch = vec![0_u8; DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5];
        let mut lifecycle = vec![0_u8; DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5];
        encode_dealer_scenario_empty_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)
            .expect("scenario empty lifecycle");

        let mut request_scratch = vec![0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
        let mut request_profile = vec![0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
        encode_dealer_scenario_request_profile_v4(&mut request_scratch, &mut request_profile)
            .expect("scenario request profile");

        let mut transition_scratch = vec![0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        let mut transition = vec![0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        encode_dealer_scenario_transition_v4(&mut transition_scratch, &mut transition)
            .expect("scenario transition");

        let templates = scenario_templates();
        let base_bytes =
            dealer_scenario_base_effect_program_bytes_v4().expect("scenario base effect width");
        let mut base_scratch = vec![0_u8; base_bytes];
        let mut base_effect = vec![0_u8; base_bytes];
        encode_dealer_scenario_base_effect_program_v4(
            &templates,
            &mut base_scratch,
            &mut base_effect,
        )
        .expect("scenario base effect");
        let effect_bytes = dealer_scenario_effect_program_bytes_v4(base_effect.len())
            .expect("scenario effect width");
        let mut effect_scratch = vec![0_u8; effect_bytes];
        let mut effect = vec![0_u8; effect_bytes];
        encode_dealer_scenario_effect_program_v4(&base_effect, &mut effect_scratch, &mut effect)
            .expect("scenario EffectV4");

        let certificate = ExecutionStrategyCertificateV2::new(
            core_id(hash(&profile).to_bytes()),
            core_id(dclutch_request_profile_contract::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID),
            core_id(hash(&request_profile).to_bytes()),
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(hash(&effect).to_bytes()),
            release_id,
            core_id([0xc1; 32]),
            core_id([0xc2; 32]),
            core_id([0xc3; 32]),
        )
        .to_bytes()
        .to_vec();
        let certificate_id = core_id(hash(&certificate).to_bytes());
        let admission = ExecutionStrategyAdmissionV2::new(certificate_id)
            .to_bytes()
            .to_vec();
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            Some(certificate_id),
            core_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            Some(core_id(hash(&admission).to_bytes())),
            core_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            core_id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("admitted scenario strategy")
        .to_bytes()
        .to_vec();
        let descriptor =
            finalize_dealer_scenario_descriptor_v4(DealerScenarioFinalizedArtifactsV4 {
                account_profile_input: profile_input,
                account_profile: &profile,
                lifecycle_policy: &lifecycle,
                capacity_profile: &[1],
                request_profile: &request_profile,
                execution_strategy: &strategy,
                transition: &transition,
                effect: &effect,
                custody_templates: &templates,
            })
            .expect("scenario descriptor");
        ScenarioArtifacts {
            descriptor,
            profile,
            request_profile,
            transition,
            effect,
            lifecycle,
            strategy,
            certificate,
            admission,
        }
    }

    fn equity_artifacts(
        action: MultiLpActionV3,
        signed_position_count: u32,
        logical_lengths: &[u32],
        release_id: ArtifactReleaseIdV1,
    ) -> EquityArtifacts {
        let profile_input = DealerEquityAccountProfileInputV3 {
            action,
            signed_position_count,
            logical_data_lengths: logical_lengths,
        };
        let profile =
            encode_dealer_equity_account_profile_v3(profile_input).expect("equity AccountProfile");
        let mut lifecycle_scratch = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        let mut lifecycle = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        encode_dealer_equity_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)
            .expect("equity lifecycle");
        let request_bytes = dealer_equity_request_profile_bytes_v3(signed_position_count)
            .expect("equity request width");
        let mut request_scratch = vec![0_u8; request_bytes];
        let mut request_profile = vec![0_u8; request_bytes];
        encode_dealer_equity_request_profile_v3(
            action,
            signed_position_count,
            &mut request_scratch,
            &mut request_profile,
        )
        .expect("equity request profile");
        let transition_bytes = dealer_equity_transition_bytes_v3(signed_position_count)
            .expect("equity transition width");
        let mut transition_scratch = vec![0_u8; transition_bytes];
        let mut transition = vec![0_u8; transition_bytes];
        encode_dealer_equity_transition_v3(
            action,
            signed_position_count,
            &mut transition_scratch,
            &mut transition,
        )
        .expect("equity transition");
        let custody_templates = equity_templates(action);
        let effect_bytes = dealer_equity_effect_bytes_v4(action, signed_position_count)
            .expect("equity effect width");
        let mut effect_scratch = vec![0_u8; effect_bytes];
        let mut effect = vec![0_u8; effect_bytes];
        encode_dealer_equity_effect_v4(
            action,
            signed_position_count,
            &custody_templates,
            &mut effect_scratch,
            &mut effect,
        )
        .expect("equity V4 effect");
        let certificate = ExecutionStrategyCertificateV2::new(
            core_id(hash(&profile).to_bytes()),
            core_id(if signed_position_count == 0 {
                REQUEST_PROFILE_SCHEMA_V1
            } else {
                dclutch_request_profile_contract::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
            }),
            core_id(hash(&request_profile).to_bytes()),
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(hash(&effect).to_bytes()),
            release_id,
            core_id([0xb1; 32]),
            core_id([0xb2; 32]),
            core_id([0xb3; 32]),
        )
        .to_bytes()
        .to_vec();
        let certificate_id = core_id(hash(&certificate).to_bytes());
        let admission = ExecutionStrategyAdmissionV2::new(certificate_id)
            .to_bytes()
            .to_vec();
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            core_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            core_id(hash(&transition).to_bytes()),
            core_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            Some(certificate_id),
            core_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            Some(core_id(hash(&admission).to_bytes())),
            core_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            core_id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("admitted equity strategy")
        .to_bytes()
        .to_vec();
        let descriptor = finalize_dealer_equity_descriptor_v4(DealerEquityFinalizedArtifactsV4 {
            account_profile_input: profile_input,
            account_profile: &profile,
            lifecycle_policy: &lifecycle,
            capacity_profile: &[1],
            effect: &effect,
            request_profile: &request_profile,
            execution_strategy: &strategy,
            transition: &transition,
            custody_templates: &custody_templates,
        })
        .expect("equity descriptor");
        EquityArtifacts {
            descriptor,
            profile,
            request_profile,
            transition,
            effect,
            lifecycle,
            strategy,
            certificate,
            admission,
        }
    }

    fn global_set(
        equity: &[EquityArtifacts; 6],
        open: &LpArtifacts,
        close: &LpArtifacts,
        scenario: &ScenarioArtifacts,
    ) -> Vec<u8> {
        let records = [
            DealerDescriptorRecordV4::new(1, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[0].descriptor)
                .expect("selector 1"),
            DealerDescriptorRecordV4::new(2, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[1].descriptor)
                .expect("selector 2"),
            DealerDescriptorRecordV4::new(3, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[2].descriptor)
                .expect("selector 3"),
            DealerDescriptorRecordV4::new(4, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[3].descriptor)
                .expect("selector 4"),
            DealerDescriptorRecordV4::new(5, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[4].descriptor)
                .expect("selector 5"),
            DealerDescriptorRecordV4::new(6, CAPABILITY_PROGRAM_SCHEMA_V4, &equity[5].descriptor)
                .expect("selector 6"),
            DealerDescriptorRecordV4::new(7, CAPABILITY_PROGRAM_SCHEMA_V4, &open.descriptor)
                .expect("selector 7"),
            DealerDescriptorRecordV4::new(8, CAPABILITY_PROGRAM_SCHEMA_V4, &close.descriptor)
                .expect("selector 8"),
            DealerDescriptorRecordV4::new(9, CAPABILITY_PROGRAM_SCHEMA_V4, &scenario.descriptor)
                .expect("selector 9"),
        ];
        let mut output = vec![0_u8; DEALER_GLOBAL_PROGRAM_SET_BYTES_V4];
        encode_dealer_global_program_set_v4(&records, &mut output).expect("global Dealer SetV2");
        output
    }

    struct LpCampaign {
        equity: [EquityArtifacts; 6],
        open: LpArtifacts,
        close: LpArtifacts,
        scenario: ScenarioArtifacts,
        program_set: Vec<u8>,
        manifest: Vec<u8>,
        config: Vec<u8>,
        root: Pubkey,
        root_bytes: Vec<u8>,
        market_bytes: Vec<u8>,
        rent_credit: Pubkey,
        obligation: Pubkey,
        obligation_bytes: Vec<u8>,
        artifact_release: Vec<u8>,
        accelerator_program: BuiltAccountV1,
        accelerator_programdata: BuiltAccountV1,
    }

    impl LpCampaign {
        fn artifacts<'a>(&'a self, action: MultiLpRequestActionV3) -> ArtifactSetV1<'a> {
            match action {
                MultiLpRequestActionV3::Open => {
                    self.open
                        .set(&self.program_set, &self.manifest, &self.config)
                }
                MultiLpRequestActionV3::Close => {
                    self.close
                        .set(&self.program_set, &self.manifest, &self.config)
                }
            }
        }

        fn admitted(&self, action: MultiLpRequestActionV3) -> AdmittedAotInputV1<'_> {
            let artifacts = match action {
                MultiLpRequestActionV3::Open => &self.open,
                MultiLpRequestActionV3::Close => &self.close,
            };
            AdmittedAotInputV1 {
                certificate: Some(&artifacts.certificate),
                admission: Some(&artifacts.admission),
                artifact_release: Some(&self.artifact_release),
                accelerator_program: Some(&self.accelerator_program),
                accelerator_programdata: Some(&self.accelerator_programdata),
            }
        }

        fn equity_artifacts(&self, selector: u16) -> ArtifactSetV1<'_> {
            let index = usize::from(selector.saturating_sub(1));
            self.equity
                .get(index)
                .unwrap_or_else(|| panic!("equity selector {selector}"))
                .set(&self.program_set, &self.manifest, &self.config)
        }

        fn equity_admitted(&self, selector: u16) -> AdmittedAotInputV1<'_> {
            let index = usize::from(selector.saturating_sub(1));
            let artifacts = self
                .equity
                .get(index)
                .unwrap_or_else(|| panic!("equity selector {selector}"));
            AdmittedAotInputV1 {
                certificate: Some(&artifacts.certificate),
                admission: Some(&artifacts.admission),
                artifact_release: Some(&self.artifact_release),
                accelerator_program: Some(&self.accelerator_program),
                accelerator_programdata: Some(&self.accelerator_programdata),
            }
        }

        fn scenario_artifacts(&self) -> ArtifactSetV1<'_> {
            self.scenario
                .set(&self.program_set, &self.manifest, &self.config)
        }

        fn scenario_admitted(&self) -> AdmittedAotInputV1<'_> {
            AdmittedAotInputV1 {
                certificate: Some(&self.scenario.certificate),
                admission: Some(&self.scenario.admission),
                artifact_release: Some(&self.artifact_release),
                accelerator_program: Some(&self.accelerator_program),
                accelerator_programdata: Some(&self.accelerator_programdata),
            }
        }
    }

    fn manifest_and_root(
        scenario: &super::Scenario,
        program_set: &[u8],
        descriptor_bytes: &[u8],
        config: &[u8],
    ) -> (Vec<u8>, Pubkey, Vec<u8>) {
        let descriptor = CapabilityProgramV4::decode(descriptor_bytes).expect("LP descriptor");
        let program_set_digest = hash(program_set).to_bytes();
        let config_digest = hash(config).to_bytes();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("creation quote"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("funding amounts");
        let entry = CapabilityEntryV1::new(
            manifest_id(descriptor.kind().to_bytes()),
            manifest_id(program_set_digest),
            manifest_id(config_digest),
            manifest_id(descriptor.capacity_profile().to_bytes()),
            manifest_id(descriptor.root_schema().to_bytes()),
            manifest_id(descriptor.derivation_policy().to_bytes()),
            ActivationPolicy::PrepaidLazy,
            10_000,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("funding quote"),
        )
        .expect("capability entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("capability manifest");
        let manifest_digest = hash(&manifest).to_bytes();
        let program_set_bumps = record_bumps(
            scenario.waist.registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            program_set_digest,
        );
        let manifest_bumps = record_bumps(
            scenario.waist.registry,
            dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        );
        let config_bumps = record_bumps(
            scenario.waist.registry,
            descriptor.config_schema().to_bytes(),
            config_digest,
        );
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            core_id(manifest_digest),
            descriptor.kind(),
            core_id(program_set_digest),
            core_id(config_digest),
        )
        .expect("capability selection")
        .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1);
        let header = CapabilityRootHeaderV1::new(
            core_id(scenario.waist.release_set_id),
            scenario.fixture.core_market.to_bytes(),
            SCENARIO_GENERATION,
            selection,
            SelectedRecordBumpsV1::new(
                manifest_bumps.0,
                manifest_bumps.1,
                config_bumps.0,
                config_bumps.1,
            ),
        )
        .expect("Dealer root header");
        let tail = RootTail {
            phase: Phase::Open,
            active_candidate_id: [0xca; 32],
            pending_candidate_id: [0; 32],
            active_revision: 1,
            pending_revision: 0,
            state_revision: 1,
            buy_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
            sell_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
            fee_base: 0,
            active_work_remaining: 0,
            pending_work_funding: 0,
        };
        let mut root_bytes = Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1 + ROOT_TAIL_BYTES);
        root_bytes.extend_from_slice(&header.to_bytes());
        root_bytes.extend_from_slice(&tail.to_bytes().expect("Dealer root tail"));
        let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING).0;
        (manifest, root, root_bytes)
    }

    fn campaign(scenario: &super::Scenario) -> LpCampaign {
        let accelerator_elf = super::elf("dclutch_dealer_accelerator_sbf");
        let release = super::artifact_release(ACCELERATOR, 0xef, &accelerator_elf);
        let artifact_release = release.to_bytes().to_vec();
        let release_id = ArtifactReleaseIdV1::new(hash(&artifact_release).to_bytes())
            .expect("accelerator ArtifactRelease id");

        // Core persists the RentCredit account address, while the credit body
        // persists the wallet that ultimately receives returned principal.
        // The older trade-only fixture names the wallet directly because it
        // never executes lifecycle.  This lifecycle campaign installs the
        // same Market at the same PDA with the canonical successor fact.
        let generation = SCENARIO_GENERATION.to_le_bytes();
        let rent_credit = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                scenario.fixture.core_market.as_ref(),
                &generation,
            ],
            &scenario.waist.registry,
        )
        .0;
        let mut market = CoreState::decode(&scenario.fixture.core_state).expect("Core Market");
        market.rent_beneficiary =
            CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit identity");
        let market_bytes = market
            .encode()
            .expect("lifecycle-owned Core Market")
            .to_vec();

        let root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1 + ROOT_TAIL_BYTES;
        let obligation_bytes = DEALER_OBLIGATION_HEADER_BYTES_V3
            + usize::try_from(scenario.fixture.outcome_count).expect("outcome width") * 8;
        let product_bytes = scenario.fixture.product.bytes.len();
        let portfolio_bytes = scenario.fixture.portfolio.bytes.len();
        let basis_bytes = scenario.fixture.linked_basis.bytes.len();
        let credit_bytes = u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("credit width");
        let common = [
            u32::try_from(root_bytes).expect("root width"),
            u32::try_from(dclutch_dealer_codec::config_v4::DEALER_CONFIG_BYTES_V4)
                .expect("config width"),
            u32::try_from(product_bytes).expect("product width"),
            u32::try_from(portfolio_bytes).expect("portfolio width"),
            u32::try_from(basis_bytes).expect("basis width"),
            u32::try_from(obligation_bytes).expect("obligation width"),
            u32::try_from(DEALER_LP_POSITION_BYTES_V3).expect("LP width"),
        ];
        let open_lengths = [
            common[0],
            common[1],
            common[2],
            common[3],
            common[4],
            common[5],
            common[6],
            0,
            credit_bytes,
            0,
        ];
        let close_lengths = [
            common[0],
            common[1],
            common[2],
            common[3],
            common[4],
            common[5],
            common[6],
            credit_bytes,
            0,
        ];
        let open = lp_artifacts(MultiLpRequestActionV3::Open, &open_lengths, release_id);
        let close = lp_artifacts(MultiLpRequestActionV3::Close, &close_lengths, release_id);
        let equity = [
            equity_artifacts(
                MultiLpActionV3::Add,
                0,
                &equity_logical_lengths(scenario, MultiLpActionV3::Add, 0, obligation_bytes),
                release_id,
            ),
            equity_artifacts(
                MultiLpActionV3::Add,
                1,
                &equity_logical_lengths(scenario, MultiLpActionV3::Add, 1, obligation_bytes),
                release_id,
            ),
            equity_artifacts(
                MultiLpActionV3::Add,
                2,
                &equity_logical_lengths(scenario, MultiLpActionV3::Add, 2, obligation_bytes),
                release_id,
            ),
            equity_artifacts(
                MultiLpActionV3::Remove,
                0,
                &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 0, obligation_bytes),
                release_id,
            ),
            equity_artifacts(
                MultiLpActionV3::Remove,
                1,
                &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 1, obligation_bytes),
                release_id,
            ),
            equity_artifacts(
                MultiLpActionV3::Remove,
                2,
                &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 2, obligation_bytes),
                release_id,
            ),
        ];
        let scenario_release = scenario_artifacts(
            DealerScenarioAccountProfileInputV4 {
                common_data_lengths: [common[0], common[1], common[2], common[3], common[4]],
            },
            release_id,
        );
        let program_set = global_set(&equity, &open, &close, &scenario_release);
        let config = DealerConfigV4::new(
            scenario.waist.release_set_id,
            scenario.realm.digest,
            scenario.dealer.pubkey().to_bytes(),
            0,
        )
        .expect("Dealer config")
        .encode()
        .to_vec();
        let (manifest, root, root_bytes) =
            manifest_and_root(scenario, &program_set, &open.descriptor, &config);
        let obligation = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, root.as_ref()],
            &TRADING,
        )
        .0;
        let obligation_bytes = super::obligation_bytes(
            scenario.fixture.core_market.to_bytes(),
            scenario.fixture.product_id,
            scenario.fixture.semantic_basis_id,
            scenario.dealer.pubkey().to_bytes(),
            root.to_bytes(),
            7,
            &[12, 20, 10],
        );
        let accelerator_programdata_key = super::programdata_address(ACCELERATOR);
        let accelerator_program = deployment_account(
            ACCELERATOR,
            loader_program_body(accelerator_programdata_key),
            true,
        );
        let accelerator_programdata = deployment_account(
            accelerator_programdata_key,
            super::loader_programdata_body(WAIST_SLOT, None, &accelerator_elf),
            false,
        );
        LpCampaign {
            equity,
            open,
            close,
            scenario: scenario_release,
            program_set,
            manifest,
            config,
            root,
            root_bytes,
            market_bytes,
            rent_credit,
            obligation,
            obligation_bytes,
            artifact_release,
            accelerator_program,
            accelerator_programdata,
        }
    }

    /// Compile the Claims graph the equity child root actually names.
    ///
    /// The scenario transcript's original graph is deliberately bound to its
    /// own trade root. Equity cannot borrow that graph: Claims persists the
    /// Custody replay context and the accelerator verifies it equals the
    /// selected Dealer root. This fixture leaves the Product/Core identities
    /// unchanged while replacing only that immutable context and the second,
    /// independently signed LP Position owner.
    fn equity_fixture(
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        lp_owner: Pubkey,
    ) -> NarrowFixtureV2 {
        let fixture = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
            outcome_count: usize::try_from(scenario.fixture.outcome_count)
                .expect("small equity outcome width"),
            registry_program: scenario.waist.registry,
            core_program: scenario.waist.core_program,
            claims_program: scenario.waist.claims_program,
            release_set: scenario.waist.release_set_id,
            realm_id: scenario.realm.digest,
            custody_context: campaign.root.to_bytes(),
            generation: SCENARIO_GENERATION,
            actor_owner: scenario.dealer.pubkey(),
            reserve_owner: lp_owner,
            funded_coordinate: FUNDED_COORDINATE,
            funded_balance: 100,
            position_revision: LIVE_POSITION_REVISION,
            reserve_balance: 0,
            terminal: None,
            rent_beneficiary: BENEFICIARY,
            graph_id: [0xb9; 32],
            exposure_id: [0xba; 32],
        })
        .expect("root-bound equity Claims graph");
        assert_eq!(
            fixture.core_market, scenario.fixture.core_market,
            "Custody context does not create a second Core/Product Market"
        );
        fixture
    }

    /// Exact AccountProfile data widths for one root-bound equity frame.
    ///
    /// This is intentionally derived from the same fixture/deployments the
    /// validator installs. It is not a second account layout: `v3_profile`
    /// remains the sole coordinate owner.
    fn equity_logical_lengths(
        scenario: &super::Scenario,
        action: MultiLpActionV3,
        positions: u32,
        obligation_bytes: usize,
    ) -> Vec<u32> {
        let programdata = |program: Pubkey| {
            let elf_bytes = scenario
                .waist
                .deployments
                .iter()
                .find(|(_, key, _)| *key == program)
                .map(|(_, _, elf)| elf.len())
                .expect("selected deployed ELF");
            u32::try_from(45_usize.checked_add(elf_bytes).expect("ProgramData width"))
                .expect("ProgramData fits u32")
        };
        let root =
            u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + ROOT_TAIL_BYTES).expect("root width");
        let config = u32::try_from(dclutch_dealer_codec::config_v4::DEALER_CONFIG_BYTES_V4)
            .expect("config width");
        let loader = u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
            .expect("loader Program width");
        let core = u32::try_from(scenario.fixture.core_state.len()).expect("Core width");
        let activation = u32::try_from(scenario.waist.cache_body.len()).expect("activation width");
        let realm = u32::try_from(scenario.realm.bytes.len()).expect("Realm width");
        let mint = u32::try_from(scenario.delivery.mint_bytes.len()).expect("Mint width");
        let aggregate = u32::try_from(scenario.live.market.len()).expect("Claims aggregate width");
        let position =
            u32::try_from(scenario.live.dealer_position.len()).expect("Claims Position width");
        let custody = [
            0,
            core,
            activation,
            0,
            loader,
            programdata(TRADING),
            realm,
            0,
            u32::try_from(CUSTODY_REPLAY_BYTES_V1).expect("replay width"),
            mint,
            u32::try_from(TOKEN_ACCOUNT_BYTES).expect("token width"),
            u32::try_from(TOKEN_ACCOUNT_BYTES).expect("token width"),
            0,
            loader,
        ];
        let mut lengths = vec![
            root,
            config,
            u32::try_from(scenario.fixture.product.bytes.len()).expect("Product width"),
            u32::try_from(scenario.fixture.portfolio.bytes.len()).expect("Portfolio width"),
            u32::try_from(scenario.fixture.linked_basis.bytes.len()).expect("Basis width"),
        ];
        lengths.extend_from_slice(&custody);
        lengths.extend_from_slice(&[
            0,
            aggregate,
            u32::try_from(scenario.fixture.linked_basis.bytes.len()).expect("Basis width"),
            0,
            u32::try_from(scenario.fixture.product.bytes.len()).expect("Product width"),
            0,
            u32::try_from(scenario.fixture.result_domain.bytes.len()).expect("domain width"),
            0,
            u32::try_from(scenario.fixture.portfolio.bytes.len()).expect("Portfolio width"),
            0,
            17,
            core,
            activation,
            0,
            loader,
            programdata(TRADING),
            loader,
            programdata(scenario.waist.claims_program),
            loader,
            programdata(scenario.waist.core_program),
        ]);
        lengths.extend(core::iter::repeat_n(
            position,
            usize::try_from(positions).expect("P"),
        ));
        let additional_custody = match action {
            MultiLpActionV3::Add => 1,
            MultiLpActionV3::Remove => 2,
        };
        for _ in 0..additional_custody {
            lengths.extend_from_slice(&custody);
        }
        lengths.extend_from_slice(&[
            u32::try_from(obligation_bytes).expect("obligation width"),
            u32::try_from(DEALER_LP_POSITION_BYTES_V3).expect("LP width"),
            loader,
            position,
            position,
        ]);
        assert_eq!(
            lengths.len(),
            usize::from(
                dealer_equity_logical_account_count_v3(action, positions)
                    .expect("equity profile geometry"),
            )
        );
        lengths
    }

    fn derived_record(
        record: &dclutch_fractional_atomic_program_test::narrow_fixture::NarrowRecordV2,
    ) -> DerivedRecordV1 {
        let derived = dclutch_chain_bundle_builder::artifacts::derive_record(
            record.owner,
            record.schema,
            &record.bytes,
        );
        assert_eq!(derived.raw, record.raw, "raw record derivation");
        assert_eq!(derived.staging, record.staging, "staging record derivation");
        assert_eq!(derived.digest, record.digest, "record content derivation");
        derived
    }

    fn fixed_for_fixture(
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        fixture: &NarrowFixtureV2,
        rent: &Rent,
    ) -> FixedCorpusV1 {
        FixedCorpusV1 {
            market: built_data_account(
                rent,
                scenario.fixture.core_market,
                scenario.waist.core_program,
                campaign.market_bytes.clone(),
            ),
            root: built_data_account(rent, campaign.root, TRADING, campaign.root_bytes.clone()),
            product: derived_record(&fixture.product),
            result_domain: derived_record(&fixture.result_domain),
            portfolio: derived_record(&fixture.portfolio),
            linked_basis: derived_record(&fixture.linked_basis),
            core_programdata: scenario.waist.core_programdata,
            trading_programdata: scenario.waist.trading_programdata,
        }
    }

    fn fixed(scenario: &super::Scenario, campaign: &LpCampaign, rent: &Rent) -> FixedCorpusV1 {
        fixed_for_fixture(scenario, campaign, &scenario.fixture, rent)
    }

    fn waist(scenario: &super::Scenario) -> WaistFactsV1 {
        WaistFactsV1 {
            registry_program: scenario.waist.registry,
            trading_program: TRADING,
            core_program: scenario.waist.core_program,
            claims_program: scenario.waist.claims_program,
            custody_program: scenario.waist.custody_program,
            release_set: scenario.waist.release_set_id,
            activation_cache: scenario.waist.activation_cache,
            trading_semantic_release: [0xe6; 32],
        }
    }

    fn lifecycle_credit(
        scenario: &super::Scenario,
        beneficiary: Pubkey,
    ) -> (Pubkey, LifecycleRentCreditV2) {
        let generation = SCENARIO_GENERATION.to_le_bytes();
        let (key, bump) = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                scenario.fixture.core_market.as_ref(),
                &generation,
            ],
            &scenario.waist.registry,
        );
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(beneficiary.to_bytes()).expect("refund authority"),
            LifecycleAccountIdV2::new(scenario.fixture.core_market.to_bytes()).expect("market"),
            LifecycleAccountIdV2::new(scenario.waist.release_set_id).expect("release set"),
            SCENARIO_GENERATION,
            bump,
        )
        .expect("lifecycle RentCredit");
        (key, credit)
    }

    fn install_bundle(
        context: &mut ProgramTestContext,
        bundle: &dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
    ) {
        for account in &bundle.bundle.accounts {
            if bundle
                .bundle
                .externally_installed_keys
                .contains(&account.key)
            {
                continue;
            }
            context.set_account(
                &account.key,
                &AccountSharedData::from(account.account.clone()),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn build_lifecycle_bundle(
        context: &mut ProgramTestContext,
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        action: MultiLpRequestActionV3,
        lp_owner: Pubkey,
        obligation_bytes: &[u8],
        position_account: Option<Account>,
        credit: Pubkey,
        credit_account: Account,
    ) -> dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1 {
        let rent = Rent::default();
        let obligation = DealerObligationProjectionV3::decode(obligation_bytes)
            .expect("canonical Dealer obligation");
        let (position, _) = Pubkey::find_program_address(
            &[
                DEALER_LP_POSITION_PDA_DOMAIN_V3,
                campaign.root.as_ref(),
                lp_owner.as_ref(),
            ],
            &TRADING,
        );
        let decoded_position = position_account
            .as_ref()
            .map(|account| DealerLpPositionV3::decode(&account.data).expect("live LP Position"));
        let clock = context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .expect("Clock sysvar");
        let chain = MultiLpChainProjectionV3 {
            trading_program: TRADING.to_bytes(),
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            child_root: campaign.root.to_bytes(),
            lp_position_address: position.to_bytes(),
            lp_position: decoded_position,
            lp_position_bytes: position_account
                .as_ref()
                .map(|account| account.data.as_slice()),
            obligation,
            obligation_address: campaign.obligation.to_bytes(),
            generation: SCENARIO_GENERATION,
            now: clock.slot,
            expires_at: clock.slot.saturating_add(100),
            lp_position_rent_principal: rent.minimum_balance(DEALER_LP_POSITION_BYTES_V3),
            terminal: false,
        };
        let set = CapabilityProgramSetV2::decode(&campaign.program_set).expect("Dealer SetV2");
        let unsigned = match action {
            MultiLpRequestActionV3::Open => {
                build_open_lp_v4(chain, lp_owner.to_bytes(), set).expect("chain-derived LP Open")
            }
            MultiLpRequestActionV3::Close => {
                build_close_lp_v4(chain, set).expect("chain-derived LP Close")
            }
        };
        let selected = unsigned.selected_descriptor();
        let selected_artifacts = match action {
            MultiLpRequestActionV3::Open => &campaign.open,
            MultiLpRequestActionV3::Close => &campaign.close,
        };
        assert_eq!(selected.schema().to_bytes(), CAPABILITY_PROGRAM_SCHEMA_V4);
        assert_eq!(
            selected.program().to_bytes(),
            hash(&selected_artifacts.descriptor).to_bytes(),
            "the operator and physical builder select the same descriptor"
        );
        let request = unsigned.as_bytes().to_vec();
        let payer = context.payer.pubkey();
        let payer_account = context
            .banks_client
            .get_account(lp_owner)
            .await
            .expect("read LP payer")
            .expect("funded LP payer");
        let system_account = context
            .banks_client
            .get_account(system_program::ID)
            .await
            .expect("read System Program")
            .expect("System Program account");
        let mut bindings = vec![
            (
                usize::from(DEALER_LP_OBLIGATION_ACCOUNT_V3),
                built_data_account(
                    &rent,
                    campaign.obligation,
                    TRADING,
                    obligation_bytes.to_vec(),
                ),
            ),
            (
                match action {
                    MultiLpRequestActionV3::Open => 8,
                    MultiLpRequestActionV3::Close => 7,
                },
                BuiltAccountV1 {
                    key: credit,
                    account: credit_account,
                    observed: None,
                },
            ),
            (
                match action {
                    MultiLpRequestActionV3::Open => 9,
                    MultiLpRequestActionV3::Close => 8,
                },
                program(system_program::ID).with_observed(system_account),
            ),
        ];
        if action == MultiLpRequestActionV3::Open {
            bindings.push((7, vacant(lp_owner).with_observed(payer_account)));
        } else {
            let position_account = position_account.expect("Close has a live LP Position");
            bindings.push((
                6,
                BuiltAccountV1 {
                    key: position,
                    account: position_account,
                    observed: None,
                },
            ));
        }
        assert_eq!(
            bindings.len(),
            match action {
                MultiLpRequestActionV3::Open => 4,
                MultiLpRequestActionV3::Close => 4,
            }
        );
        assert_eq!(
            dealer_lp_account_count_v3(action),
            match action {
                MultiLpRequestActionV3::Open => 10,
                MultiLpRequestActionV3::Close => 9,
            }
        );
        let externally_installed = [lp_owner];
        let scenario_input = ScenarioV1 {
            family_request: &request,
            tail_count: scenario.fixture.outcome_count,
            clock_slot: clock.slot,
            generation: SCENARIO_GENERATION,
            ed25519_evidence: None,
            native_message_instruction_index: 0,
            externally_installed_extra: &externally_installed,
            payer,
        };
        build_admitted_bundle(
            &BundleInputV1 {
                set: campaign.artifacts(action),
                waist: waist(scenario),
                scenario: scenario_input,
                fixed: fixed(scenario, campaign, &rent),
                bindings: &bindings,
                rent: &rent,
            },
            campaign.admitted(action),
        )
        .unwrap_or_else(|error| panic!("physical LP bundle refused at {error:?}"))
    }

    #[derive(Clone)]
    struct EquityCollateralState {
        authority: Pubkey,
        replay: Pubkey,
        replay_bytes: Vec<u8>,
        external: Pubkey,
        external_bytes: Vec<u8>,
        principal: Pubkey,
        principal_bytes: Vec<u8>,
        hoard: Pubkey,
        hoard_bytes: Vec<u8>,
    }

    fn delegated_token_account_bytes(
        mint: Pubkey,
        owner: Pubkey,
        amount: u64,
        delegate: Pubkey,
        delegated_amount: u64,
    ) -> Vec<u8> {
        let mut bytes = vec![0_u8; SplTokenAccount::LEN];
        SplTokenAccount::pack(
            SplTokenAccount {
                mint,
                owner,
                amount,
                delegate: ProgramCOption::Some(delegate),
                state: SplAccountState::Initialized,
                is_native: ProgramCOption::None,
                delegated_amount,
                close_authority: ProgramCOption::None,
            },
            &mut bytes,
        )
        .expect("delegated LP collateral account");
        bytes
    }

    fn first_equity_collateral(
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        lp_owner: Pubkey,
        external_tag: u8,
        external_balance: u64,
        allowance: u64,
    ) -> EquityCollateralState {
        let custody = scenario.waist.custody_program;
        let market = scenario.fixture.core_market.to_bytes();
        let release_set = scenario.waist.release_set_id;
        let authority = Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::new(market, release_set).as_slices(),
            &custody,
        )
        .0;
        let replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                market,
                release_set,
                CallerRoleV1::Trading,
                campaign.root.to_bytes(),
            )
            .as_slices(),
            &custody,
        )
        .0;
        let principal = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                market,
                release_set,
                campaign.root.to_bytes(),
                CompartmentV1::TradingPrincipal,
            )
            .as_slices(),
            &custody,
        )
        .0;
        let hoard = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(market, release_set, market, CompartmentV1::HoardPrincipal)
                .as_slices(),
            &custody,
        )
        .0;
        let external = Pubkey::new_from_array([external_tag; 32]);
        let mint = scenario.delivery.mint;
        EquityCollateralState {
            authority,
            replay,
            replay_bytes: CustodyReplayV1 {
                caller_role: CallerRoleV1::Trading,
                release_set,
                market,
                realm: scenario.realm.digest,
                context: campaign.root.to_bytes(),
                caller_program: TRADING.to_bytes(),
                rent_refund: BENEFICIARY.to_bytes(),
                open_vault_count: 2,
                next_revision: 7,
                generation: SCENARIO_GENERATION,
                last_request_digest: [0xa7; 32],
                last_poststate_commitment: [0xa8; 32],
            }
            .to_bytes()
            .expect("equity Custody replay")
            .to_vec(),
            external,
            external_bytes: delegated_token_account_bytes(
                mint,
                lp_owner,
                external_balance,
                authority,
                allowance,
            ),
            principal,
            principal_bytes: super::dealer_delivery_token_account_bytes(mint, authority, 0),
            hoard,
            hoard_bytes: super::dealer_delivery_token_account_bytes(mint, authority, 0),
        }
    }

    async fn current_equity_collateral(
        context: &mut ProgramTestContext,
        collateral: &EquityCollateralState,
    ) -> EquityCollateralState {
        EquityCollateralState {
            authority: collateral.authority,
            replay: collateral.replay,
            replay_bytes: chain_account(context, collateral.replay).await.data,
            external: collateral.external,
            external_bytes: chain_account(context, collateral.external).await.data,
            principal: collateral.principal,
            principal_bytes: chain_account(context, collateral.principal).await.data,
            hoard: collateral.hoard,
            hoard_bytes: chain_account(context, collateral.hoard).await.data,
        }
    }

    async fn current_equity_claims(
        context: &mut ProgramTestContext,
        fixture: &NarrowFixtureV2,
    ) -> super::LiveClaimsGraph {
        let market = chain_account(context, fixture.claims_market).await.data;
        let dealer_position = chain_account(context, fixture.actor_position.account)
            .await
            .data;
        let counterparty_position = chain_account(context, fixture.reserve_position.account)
            .await
            .data;
        let balances = |bytes: &[u8]| {
            let view = LiabilityBasisPositionViewV2::decode(bytes).expect("live Claims Position");
            (0..fixture.outcome_count)
                .map(|coordinate| view.balance(bytes, coordinate).expect("Claims balance"))
                .collect::<Vec<_>>()
        };
        let dealer_balances = balances(&dealer_position);
        let counterparty_balances = balances(&counterparty_position);
        super::LiveClaimsGraph {
            market,
            dealer_position,
            counterparty_position,
            dealer_balances,
            counterparty_balances,
        }
    }

    /// Legacy composer adapter for a descriptor already selected from the
    /// campaign's authenticated SetV2.  This carries no independent program
    /// authority: callers must rejoin the returned request to SetV2 below.
    fn single_selection_set(selector_offset: u32, selector: u16, program_id: [u8; 32]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 72];
        bytes[..8].copy_from_slice(b"DCLTCPS1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&selector_offset.to_le_bytes());
        bytes[16] = 2;
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
        bytes[32..36].copy_from_slice(&u32::from(selector).to_le_bytes());
        bytes[36..68].copy_from_slice(&program_id);
        bytes
    }

    async fn observed_binding(context: &mut ProgramTestContext, key: Pubkey) -> BuiltAccountV1 {
        BuiltAccountV1 {
            key,
            account: chain_account(context, key).await,
            observed: None,
        }
    }

    struct BuiltEquityStep {
        admitted: dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
        plan: PoolEquityPlanV3,
        dealer_claims_after: Vec<u64>,
        lp_claims_after: Vec<u64>,
    }

    struct BuiltScenarioStep {
        admitted: dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
        semantic: dclutch_operator::dealer_scenario_hot_v4::DealerScenarioHotSemanticReportV4,
    }

    #[allow(clippy::too_many_arguments)]
    async fn build_equity_bundle(
        context: &mut ProgramTestContext,
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        fixture: &NarrowFixtureV2,
        live: &super::LiveClaimsGraph,
        lp_owner: Pubkey,
        obligation_bytes: &[u8],
        lp_account: &Account,
        collateral: &EquityCollateralState,
        intent: EquityRequestIntentV3<'_>,
        expected_selector: u16,
        lp_evidence_override: Option<(Pubkey, Vec<u8>)>,
    ) -> BuiltEquityStep {
        let rent = Rent::default();
        let clock = context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .expect("Clock sysvar");
        let obligation =
            DealerObligationProjectionV3::decode(obligation_bytes).expect("equity obligation");
        let (lp_position_address, _) = Pubkey::find_program_address(
            &[
                DEALER_LP_POSITION_PDA_DOMAIN_V3,
                campaign.root.as_ref(),
                lp_owner.as_ref(),
            ],
            &TRADING,
        );
        let lp_position = DealerLpPositionV3::decode(&lp_account.data).expect("opened LP Position");
        let action = match intent {
            EquityRequestIntentV3::Contribute { .. } => MultiLpActionV3::Add,
            EquityRequestIntentV3::Redeem { .. } => MultiLpActionV3::Remove,
        };
        let expected_position_count = match expected_selector {
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3 | DEALER_EQUITY_REDEEM_P0_SELECTOR_V3 => 0,
            DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3 | DEALER_EQUITY_REDEEM_P1_SELECTOR_V3 => 1,
            DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3 | DEALER_EQUITY_REDEEM_P2_SELECTOR_V3 => 2,
            _ => panic!("unsupported equity selector {expected_selector}"),
        };
        assert_eq!(
            matches!(action, MultiLpActionV3::Add),
            expected_selector <= DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3,
            "intent action and selected equity artifact agree"
        );
        let external_token =
            SplTokenAccount::unpack(&collateral.external_bytes).expect("LP external token");
        let claims_market =
            LiabilityBasisMarketViewV2::decode(&live.market).expect("live Claims aggregate");
        let dealer_position = LiabilityBasisPositionViewV2::decode(&live.dealer_position)
            .expect("live Dealer Claims Position");
        let lp_claims_position = LiabilityBasisPositionViewV2::decode(&live.counterparty_position)
            .expect("live LP Claims Position");
        let dealer_claims = ClaimsInventoryObservation {
            market_id: scenario.fixture.core_market.to_bytes(),
            product_id: scenario.fixture.product_id,
            liability_basis_id: scenario.fixture.semantic_basis_id,
            position_owner: scenario.dealer.pubkey().to_bytes(),
            revision: dealer_position.revision,
            inventory: &live.dealer_balances,
        };
        let lp_claims = ClaimsInventoryObservation {
            market_id: scenario.fixture.core_market.to_bytes(),
            product_id: scenario.fixture.product_id,
            liability_basis_id: scenario.fixture.semantic_basis_id,
            position_owner: lp_owner.to_bytes(),
            revision: lp_claims_position.revision,
            inventory: &live.counterparty_balances,
        };
        let pool_action = match intent {
            EquityRequestIntentV3::Contribute {
                collateral,
                claims,
                minted_shares,
            } => PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                collateral,
                claims,
                minted_shares,
            }),
            EquityRequestIntentV3::Redeem { burned_shares } => {
                PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares })
            }
        };
        let canonical_obligations = obligation.obligations().collect::<Vec<_>>();
        let plan = preflight_pool_equity_v3(PoolEquityInputV3 {
            collateral: super::token_account_amount(&collateral.principal_bytes),
            claims: &live.dealer_balances,
            obligations: &canonical_obligations,
            total_shares: obligation.total_equity_shares(),
            locked_capital_floor: 0,
            action: pool_action,
            basis_scale: 1,
        })
        .expect("canonical pool-equity preflight");
        let chain = EquityPoolChainProjectionV3 {
            trading_program: TRADING.to_bytes(),
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            child_root: campaign.root.to_bytes(),
            obligation_address: campaign.obligation.to_bytes(),
            obligation,
            lp_position_address: lp_position_address.to_bytes(),
            lp_position,
            lp_position_bytes: &lp_account.data,
            dealer_claims,
            lp_claims,
            product_record_digest: fixture.product.digest,
            linked_basis_record_digest: fixture.linked_basis.digest,
            claims_market_revision: claims_market.revision,
            collateral: MultiLpCollateralFrameV3 {
                lp_external_account: collateral.external.to_bytes(),
                lp_owner: lp_owner.to_bytes(),
                lp_external_balance: external_token.amount,
                lp_external_delegate: match external_token.delegate {
                    ProgramCOption::Some(delegate) => delegate.to_bytes(),
                    ProgramCOption::None => [0; 32],
                },
                lp_external_delegated_amount: external_token.delegated_amount,
                principal_vault: collateral.principal.to_bytes(),
                principal_balance: super::token_account_amount(&collateral.principal_bytes),
                hoard_vault: collateral.hoard.to_bytes(),
                hoard_balance: super::token_account_amount(&collateral.hoard_bytes),
            },
            locked_capital_floor: 0,
            generation: SCENARIO_GENERATION,
            now: clock.slot,
            expires_at: clock.slot.saturating_add(100),
            terminal: false,
            basis_scale: 1,
        };
        let artifact_index = usize::from(
            expected_selector
                .checked_sub(1)
                .expect("positive equity selector"),
        );
        let selected_program = hash(
            &campaign
                .equity
                .get(artifact_index)
                .expect("equity selector artifact")
                .descriptor,
        )
        .to_bytes();
        let selector_set_bytes = single_selection_set(
            dclutch_trading_sbf::dealer::v3_equity_operator::DEALER_EQUITY_SELECTOR_OFFSET_V3,
            expected_selector,
            selected_program,
        );
        let selector_set =
            CapabilityProgramSetV1::decode(&selector_set_bytes).expect("equity selection set");
        let width = usize::try_from(scenario.fixture.outcome_count).expect("small width");
        let mut request = vec![0_u8; 4096];
        let mut obligation_scratch = vec![0_u64; width];
        let mut residual_before = vec![0_u64; width];
        let mut residual_after = vec![0_u64; width];
        let mut claims_transferred = vec![0_u64; width];
        let mut dealer_after = vec![0_u64; width];
        let mut lp_after = vec![0_u64; width];
        let unsigned = build_equity_request_v3(
            chain,
            intent,
            selector_set,
            &mut request,
            &mut obligation_scratch,
            &mut residual_before,
            &mut residual_after,
            &mut claims_transferred,
            &mut dealer_after,
            &mut lp_after,
        )
        .expect("first cash-only equity contribution");
        assert_eq!(unsigned.selected_program.to_bytes(), selected_program);
        request.truncate(unsigned.request_bytes);
        let decoded = DealerEquityRequestV3::decode(&request).expect("equity request");
        assert_eq!(decoded.selector(), expected_selector);
        assert_eq!(
            decoded
                .claims_plan()
                .expect("equity Claims plan")
                .map_or(0, |plan| plan.position_count()),
            expected_position_count,
            "selector owns the exact physical Claims width"
        );

        // Re-run the canonical physical planner before the generic builder so
        // the projected child-route set is observable at this boundary.  This
        // owns no second arithmetic: it is the same request-to-plan join the
        // accelerator executes, over the same chain snapshot.
        let replay_view = CustodyReplayV1::decode(&collateral.replay_bytes)
            .expect("equity Custody replay decodes");
        let physical_context = MultiLpContextV3 {
            trading_program: TRADING.to_bytes(),
            custody_program: scenario.waist.custody_program.to_bytes(),
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            realm: scenario.realm.digest,
            child_root: campaign.root.to_bytes(),
            obligation_account: campaign.obligation.to_bytes(),
            mint: scenario.delivery.mint.to_bytes(),
            token_program: scenario.delivery.token_program.to_bytes(),
            parent_request_digest: hash(&request).to_bytes(),
            generation: SCENARIO_GENERATION,
            custody_replay_revision: replay_view.next_revision,
            locked_capital_floor: 0,
            basis_scale: 1,
        };
        let mut request_claims_scratch = vec![0_u64; width];
        let mut physical_obligation_scratch = vec![0_u64; width];
        let mut physical_residual_before = vec![0_u64; width];
        let mut physical_residual_after = vec![0_u64; width];
        let mut physical_claims_transferred = vec![0_u64; width];
        let mut physical_dealer_after = vec![0_u64; width];
        let mut physical_lp_after = vec![0_u64; width];
        let mut post_obligation = vec![0_u8; obligation_bytes.len()];
        let mut post_lp = vec![0_u8; DEALER_LP_POSITION_BYTES_V3];
        let mut custody_scratch = [None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3];
        let mut custody_output = [None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3];
        let physical_plan = prepare_equity_request_v3(
            &decoded,
            &chain,
            &physical_context,
            &mut request_claims_scratch,
            &mut physical_obligation_scratch,
            &mut physical_residual_before,
            &mut physical_residual_after,
            &mut physical_claims_transferred,
            &mut physical_dealer_after,
            &mut physical_lp_after,
            &mut post_obligation,
            &mut post_lp,
            &mut custody_scratch,
            &mut custody_output,
        )
        .expect("canonical physical equity plan");
        assert_eq!(physical_dealer_after, dealer_after);
        assert_eq!(physical_lp_after, lp_after);
        let mut projected_scalars =
            vec![0_u64; dealer_equity_scalar_count_v3(action).expect("equity scalar width")];
        let mut projected_identities = vec![
            [0_u8; 32];
            dealer_equity_identity_count_v3(action)
                .expect("equity identity width")
        ];
        let evidence_owner = dealer_equity_evidence_owner_identity_register_v3(action)
            .expect("equity evidence owner register");
        *projected_identities
            .get_mut(usize::from(evidence_owner))
            .expect("evidence owner register in bounds") = scenario.waist.claims_program.to_bytes();
        project_dealer_equity_hot_registers_v3(
            decoded,
            physical_plan,
            &custody_output,
            clock.slot,
            &mut projected_scalars,
            &mut projected_identities,
        )
        .expect("canonical equity Hot registers");
        let effect = dclutch_effect_kernel::v4::ProgramV4::decode(
            campaign.equity_artifacts(expected_selector).effect,
        )
        .expect("equity EffectV4");
        let base = effect.base();
        let canonical_route_counts = (0..base.route_count())
            .map(|route| {
                base.invocation_count(
                    route,
                    scenario.fixture.outcome_count,
                    &projected_scalars,
                    &projected_identities,
                )
                .expect("canonical equity route enable projection")
            })
            .collect::<Vec<_>>();
        eprintln!("canonical equity invocation counts {canonical_route_counts:?}");
        assert_eq!(
            canonical_route_counts.get(1).copied().unwrap_or_default(),
            u32::from(expected_position_count != 0),
            "the P0 Claims route must not yield an invocation"
        );
        if let Some(active) = custody_output.first().copied().flatten() {
            let mut active_request = vec![0_u8; active.request.encoded_len()];
            active
                .request
                .encode_into(&mut active_request)
                .expect("canonical active Custody request");
            let resolved = base
                .resolved_invocation(
                    0,
                    0,
                    scenario.fixture.outcome_count,
                    &projected_scalars,
                    &projected_identities,
                )
                .expect("active first Custody route");
            assert_eq!(
                active_request.len(),
                resolved.request_len,
                "the canonical child body has the route-owned width"
            );
            eprintln!(
                "canonical route0 bytes={} magic={:?}",
                active_request.len(),
                active_request.get(..8).expect("Custody magic")
            );
            derive_authority(
                &DerivedInvocationV1 {
                    route: 0,
                    invocation: 0,
                    resolved,
                    request: active_request,
                },
                scenario.waist.release_set_id,
                TRADING,
            )
            .expect("active Custody authority request kind")
            .expect("Custody owns a caller authority");
        }

        let registry = observed_binding(context, scenario.waist.registry).await;
        let activation = observed_binding(context, scenario.waist.activation_cache).await;
        let trading_program = observed_binding(context, TRADING).await;
        let trading_programdata =
            observed_binding(context, scenario.waist.trading_programdata).await;
        let claims_program = observed_binding(context, scenario.waist.claims_program).await;
        let claims_programdata = observed_binding(context, scenario.waist.claims_programdata).await;
        let core_program = observed_binding(context, scenario.waist.core_program).await;
        let core_programdata = observed_binding(context, scenario.waist.core_programdata).await;
        let custody_program = observed_binding(context, scenario.waist.custody_program).await;
        let token_program = observed_binding(context, scenario.delivery.token_program).await;
        let rent_sysvar = observed_binding(context, sysvar::rent::ID).await;
        let market = built_data_account(
            &rent,
            scenario.fixture.core_market,
            scenario.waist.core_program,
            campaign.market_bytes.clone(),
        );
        let replay = built_data_account(
            &rent,
            collateral.replay,
            scenario.waist.custody_program,
            collateral.replay_bytes.clone(),
        );
        let mint = built_data_account(
            &rent,
            scenario.delivery.mint,
            scenario.delivery.token_program,
            scenario.delivery.mint_bytes.clone(),
        );
        let authority = vacant(collateral.authority);
        let external = built_data_account(
            &rent,
            collateral.external,
            scenario.delivery.token_program,
            collateral.external_bytes.clone(),
        );
        let principal = built_data_account(
            &rent,
            collateral.principal,
            scenario.delivery.token_program,
            collateral.principal_bytes.clone(),
        );
        let hoard = built_data_account(
            &rent,
            collateral.hoard,
            scenario.delivery.token_program,
            collateral.hoard_bytes.clone(),
        );
        let realm_raw = built_data_account(
            &rent,
            scenario.realm.raw,
            scenario.waist.registry,
            scenario.realm.bytes.clone(),
        );
        let realm_staging = vacant(scenario.realm.staging);
        let claims_market = built_data_account(
            &rent,
            fixture.claims_market,
            scenario.waist.claims_program,
            live.market.clone(),
        );
        let dealer_evidence = built_data_account(
            &rent,
            fixture.actor_position.account,
            scenario.waist.claims_program,
            live.dealer_position.clone(),
        );
        let (lp_evidence_key, lp_evidence_bytes) = lp_evidence_override.unwrap_or_else(|| {
            (
                fixture.reserve_position.account,
                live.counterparty_position.clone(),
            )
        });
        let lp_evidence = built_data_account(
            &rent,
            lp_evidence_key,
            scenario.waist.claims_program,
            lp_evidence_bytes,
        );
        let obligation_binding = built_data_account(
            &rent,
            campaign.obligation,
            TRADING,
            obligation_bytes.to_vec(),
        );
        let lp_binding = BuiltAccountV1 {
            key: lp_position_address,
            account: lp_account.clone(),
            observed: None,
        };
        let domain_raw = built_data_account(
            &rent,
            fixture.result_domain.raw,
            fixture.result_domain.owner,
            fixture.result_domain.bytes.clone(),
        );
        let claims_start = 19_usize;
        let claims_positions_start = claims_start + 20;
        let later_custody_start = claims_positions_start
            + usize::try_from(expected_position_count).expect("small Claims position count");
        let custody_route_count = match action {
            MultiLpActionV3::Add => 2_usize,
            MultiLpActionV3::Remove => 3_usize,
        };
        let local_start = later_custody_start + (custody_route_count - 1) * 14;
        let obligation_coordinate = local_start;
        let lp_coordinate = obligation_coordinate + 1;
        let custody_program_coordinate = lp_coordinate + 1;
        let evidence_start = custody_program_coordinate + 1;
        let (first_source, first_destination) = match action {
            MultiLpActionV3::Add => (external.clone(), principal.clone()),
            MultiLpActionV3::Remove => (principal.clone(), hoard.clone()),
        };
        let mut bindings = vec![
            (6, market.clone()),
            (7, activation.clone()),
            (8, registry.clone()),
            (9, trading_program.clone()),
            (10, trading_programdata.clone()),
            (11, realm_raw.clone()),
            (12, realm_staging.clone()),
            (13, replay.clone()),
            (14, mint.clone()),
            (15, first_source),
            (16, first_destination),
            (17, authority.clone()),
            (18, token_program.clone()),
            (20, claims_market),
            (22, vacant(fixture.linked_basis.staging)),
            (24, vacant(fixture.product.staging)),
            (25, domain_raw),
            (26, vacant(fixture.result_domain.staging)),
            (28, vacant(fixture.portfolio.staging)),
            (29, rent_sysvar),
            (31, activation),
            (35, claims_program),
            (36, claims_programdata),
            (37, core_program),
            (38, core_programdata),
        ];
        if let Some(plan) = decoded.claims_plan().expect("equity Claims plan") {
            for index in 0..plan.position_count() {
                let position = plan.position(index).expect("Claims position descriptor");
                let (key, bytes) = if position.owner() == scenario.dealer.pubkey().to_bytes() {
                    (fixture.actor_position.account, live.dealer_position.clone())
                } else if position.owner() == lp_owner.to_bytes() {
                    (
                        fixture.reserve_position.account,
                        live.counterparty_position.clone(),
                    )
                } else {
                    panic!("equity packet names an unrelated Claims Position owner")
                };
                bindings.push((
                    claims_positions_start
                        + usize::try_from(index).expect("small Claims position ordinal"),
                    built_data_account(&rent, key, scenario.waist.claims_program, bytes),
                ));
            }
        }
        let later_routes = match action {
            MultiLpActionV3::Add => vec![(hoard.clone(), principal.clone())],
            MultiLpActionV3::Remove => vec![
                (principal.clone(), external.clone()),
                (hoard.clone(), principal.clone()),
            ],
        };
        for (index, (source, destination)) in later_routes.iter().enumerate() {
            let start = later_custody_start + index * 14;
            bindings.push((start + 8, replay.clone()));
            bindings.push((start + 10, source.clone()));
            bindings.push((start + 11, destination.clone()));
        }
        bindings.extend([
            (obligation_coordinate, obligation_binding),
            (lp_coordinate, lp_binding),
            (custody_program_coordinate, custody_program),
        ]);
        match expected_position_count {
            0 => bindings.extend([
                (evidence_start, dealer_evidence),
                (evidence_start + 1, lp_evidence),
            ]),
            1 => bindings.push((evidence_start + 1, lp_evidence)),
            2 => {}
            _ => unreachable!("selector position count is bounded"),
        }
        let external_programdata = [
            scenario.waist.claims_programdata,
            scenario.waist.custody_programdata,
            scenario.delivery.token_program,
        ];
        let scenario_input = ScenarioV1 {
            family_request: &request,
            tail_count: scenario.fixture.outcome_count,
            clock_slot: clock.slot,
            generation: SCENARIO_GENERATION,
            ed25519_evidence: None,
            native_message_instruction_index: 0,
            externally_installed_extra: &external_programdata,
            payer: context.payer.pubkey(),
        };
        let equity_artifacts = campaign.equity_artifacts(expected_selector);
        let profile = AccountProfileV2::decode(equity_artifacts.account_profile)
            .expect("selector-1 equity AccountProfile");
        let logical_count = profile
            .logical_account_count(scenario.fixture.outcome_count)
            .expect("selector-1 logical AccountProfile geometry");
        assert_eq!(
            logical_count,
            usize::from(
                dealer_equity_logical_account_count_v3(action, expected_position_count)
                    .expect("equity logical geometry")
            ),
            "selector owns its exact logical geometry"
        );
        let physical_count = profile
            .physical_account_count(scenario.fixture.outcome_count)
            .expect("selector-1 physical AccountProfile geometry");
        for ordinal in 0..physical_count {
            profile
                .physical_account_geometry(scenario.fixture.outcome_count, ordinal)
                .unwrap_or_else(|error| {
                    panic!("selector-1 physical ordinal {ordinal} refused: {error:?}")
                });
        }
        if action == MultiLpActionV3::Add && expected_position_count == 0 {
            let binding = |coordinate| {
                bindings
                    .iter()
                    .find(|(observed, _)| *observed == coordinate)
                    .map(|(_, account)| account)
                    .unwrap_or_else(|| {
                        panic!("missing selector-1 identity coordinate {coordinate}")
                    })
            };
            let trading_caller = binding(9);
            let obligation = binding(53);
            let lp_position = binding(54);
            let claims_program = binding(35);
            let dealer_evidence = binding(56);
            let lp_evidence = binding(57);
            eprintln!(
                "selector-1 identities caller={}/{} obligation={}/{} lp={}/{} claims={}/{} dealer-evidence={}/{} lp-evidence={}/{}",
                trading_caller.key,
                trading_caller.account.owner,
                obligation.key,
                obligation.account.owner,
                lp_position.key,
                lp_position.account.owner,
                claims_program.key,
                claims_program.account.owner,
                dealer_evidence.key,
                dealer_evidence.account.owner,
                lp_evidence.key,
                lp_evidence.account.owner,
            );
            assert_eq!(trading_caller.key, TRADING, "Custody caller identity");
            assert_eq!(obligation.account.owner, TRADING, "obligation owner");
            assert_eq!(lp_position.account.owner, TRADING, "LP Position owner");
            assert_eq!(
                claims_program.key, scenario.waist.claims_program,
                "Claims identity"
            );
            assert_eq!(
                dealer_evidence.account.owner, scenario.waist.claims_program,
                "Dealer evidence owner"
            );
            assert_eq!(
                lp_evidence.account.owner, scenario.waist.claims_program,
                "LP evidence owner"
            );
        }
        let bundle_input = BundleInputV1 {
            set: equity_artifacts,
            waist: waist(scenario),
            scenario: scenario_input,
            fixed: fixed(scenario, campaign, &rent),
            bindings: &bindings,
            rent: &rent,
        };
        let hostile_candidate = |scalars: &mut [u64], _: &mut [[u8; 32]]| {
            if let Some(first) = scalars.first_mut() {
                *first ^= 1;
            }
            Err(BuilderError::Projection("hostile-equity-candidate"))
        };
        assert!(matches!(
            build_admitted_bundle_with_candidate_v1(
                &bundle_input,
                campaign.equity_admitted(expected_selector),
                &hostile_candidate,
            ),
            Err(BuilderError::Projection("admitted-candidate"))
        ));
        let honest_candidate = |scalars: &mut [u64], identities: &mut [[u8; 32]]| {
            project_dealer_equity_hot_registers_v3(
                decoded,
                physical_plan,
                &custody_output,
                clock.slot,
                scalars,
                identities,
            )
            .map_err(|_| BuilderError::Projection("dealer-equity-candidate"))?;
            let projected_counts = (0..base.route_count())
                .map(|route| {
                    base.invocation_count(
                        route,
                        scenario.fixture.outcome_count,
                        scalars,
                        identities,
                    )
                    .map_err(|_| BuilderError::Projection("dealer-equity-route-count"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if projected_counts != canonical_route_counts {
                eprintln!(
                    "candidate route counts differ: canonical {canonical_route_counts:?}, preplan-seeded {projected_counts:?}"
                );
                return Err(BuilderError::Projection("dealer-equity-route-count"));
            }
            Ok(())
        };
        let admitted = build_admitted_bundle_with_candidate_v1(
            &bundle_input,
            campaign.equity_admitted(expected_selector),
            &honest_candidate,
        )
        .unwrap_or_else(|error| panic!("physical equity bundle refused at {error:?}"));
        BuiltEquityStep {
            admitted,
            plan,
            dealer_claims_after: dealer_after,
            lp_claims_after: lp_after,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn build_scenario_bundle(
        context: &mut ProgramTestContext,
        scenario: &super::Scenario,
        campaign: &LpCampaign,
        fixture: &NarrowFixtureV2,
        live: &super::LiveClaimsGraph,
        obligation_bytes: &[u8],
        collateral: &EquityCollateralState,
        family_request: &[u8],
        chain: ScenarioTradeChainProjectionV3<'_>,
        composer: ScenarioComposerContextV3,
        collateral_frame: ScenarioCollateralFrameV3,
    ) -> BuiltScenarioStep {
        let rent = Rent::default();
        let clock = context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .expect("scenario Clock");
        let semantic_state = DealerScenarioSemanticStateV4 {
            chain,
            context: composer,
            collateral: collateral_frame,
        };
        let semantic = project_dealer_scenario_hot_semantics_v4(semantic_state, family_request)
            .expect("canonical scenario Hot semantics");
        let frame = dealer_scenario_logical_frame_v4(semantic.dynamic_span_counts)
            .expect("canonical scenario logical frame");
        let request =
            DealerScenarioTradeRequestV3::decode(family_request).expect("scenario family request");
        assert_eq!(
            request
                .claims_plan()
                .expect("scenario Claims plan")
                .position_count(),
            2
        );
        assert_eq!(semantic.semantic_plan.custody_count, 1);

        let market = built_data_account(
            &rent,
            fixture.core_market,
            scenario.waist.core_program,
            campaign.market_bytes.clone(),
        );
        let activation = observed_binding(context, scenario.waist.activation_cache).await;
        let registry = observed_binding(context, scenario.waist.registry).await;
        let trading_program = observed_binding(context, TRADING).await;
        let trading_programdata =
            observed_binding(context, scenario.waist.trading_programdata).await;
        let claims_program = observed_binding(context, scenario.waist.claims_program).await;
        let claims_programdata = observed_binding(context, scenario.waist.claims_programdata).await;
        let core_program = observed_binding(context, scenario.waist.core_program).await;
        let core_programdata = observed_binding(context, scenario.waist.core_programdata).await;
        let custody_program = observed_binding(context, scenario.waist.custody_program).await;
        let token_program = observed_binding(context, scenario.delivery.token_program).await;
        let rent_sysvar = observed_binding(context, sysvar::rent::ID).await;
        let realm_raw = built_data_account(
            &rent,
            scenario.realm.raw,
            scenario.waist.registry,
            scenario.realm.bytes.clone(),
        );
        let realm_staging = vacant(scenario.realm.staging);
        let replay = built_data_account(
            &rent,
            collateral.replay,
            scenario.waist.custody_program,
            collateral.replay_bytes.clone(),
        );
        let mint = built_data_account(
            &rent,
            scenario.delivery.mint,
            scenario.delivery.token_program,
            scenario.delivery.mint_bytes.clone(),
        );
        let authority = vacant(collateral.authority);
        let hoard = built_data_account(
            &rent,
            collateral.hoard,
            scenario.delivery.token_program,
            collateral.hoard_bytes.clone(),
        );
        let fee = built_data_account(
            &rent,
            Pubkey::new_from_array(collateral_frame.fee_vault),
            scenario.delivery.token_program,
            super::dealer_delivery_token_account_bytes(
                scenario.delivery.mint,
                collateral.authority,
                collateral_frame.fee_balance,
            ),
        );
        let claims_market = built_data_account(
            &rent,
            fixture.claims_market,
            scenario.waist.claims_program,
            live.market.clone(),
        );
        let dealer_position = built_data_account(
            &rent,
            fixture.actor_position.account,
            scenario.waist.claims_program,
            live.dealer_position.clone(),
        );
        let lp_position = built_data_account(
            &rent,
            fixture.reserve_position.account,
            scenario.waist.claims_program,
            live.counterparty_position.clone(),
        );
        let obligation = built_data_account(
            &rent,
            campaign.obligation,
            TRADING,
            obligation_bytes.to_vec(),
        );
        let basis_raw = built_data_account(
            &rent,
            fixture.linked_basis.raw,
            fixture.linked_basis.owner,
            fixture.linked_basis.bytes.clone(),
        );
        let product_raw = built_data_account(
            &rent,
            fixture.product.raw,
            fixture.product.owner,
            fixture.product.bytes.clone(),
        );
        let domain_raw = built_data_account(
            &rent,
            fixture.result_domain.raw,
            fixture.result_domain.owner,
            fixture.result_domain.bytes.clone(),
        );
        let portfolio_raw = built_data_account(
            &rent,
            fixture.portfolio.raw,
            fixture.portfolio.owner,
            fixture.portfolio.bytes.clone(),
        );

        let mut bindings = Vec::new();
        let claims = SignedDeltaFrameSpecV3::new(2).expect("scenario SignedDelta P2 frame");
        for ordinal in 0..claims.account_count().expect("scenario Claims frame width") {
            let account = claims.account(ordinal).expect("scenario Claims role");
            let (coordinate, binding) = match account.role() {
                ClaimsFrameRoleV1::CallerAuthority => continue,
                ClaimsFrameRoleV1::ClaimsMarket => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    claims_market.clone(),
                ),
                ClaimsFrameRoleV1::BasisRecord => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    basis_raw.clone(),
                ),
                ClaimsFrameRoleV1::BasisStaging => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    vacant(fixture.linked_basis.staging),
                ),
                ClaimsFrameRoleV1::ProductRecord => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    product_raw.clone(),
                ),
                ClaimsFrameRoleV1::ProductStaging => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    vacant(fixture.product.staging),
                ),
                ClaimsFrameRoleV1::ResultDomainRecord => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    domain_raw.clone(),
                ),
                ClaimsFrameRoleV1::ResultDomainStaging => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    vacant(fixture.result_domain.staging),
                ),
                ClaimsFrameRoleV1::PortfolioRecord => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    portfolio_raw.clone(),
                ),
                ClaimsFrameRoleV1::PortfolioStaging => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    vacant(fixture.portfolio.staging),
                ),
                ClaimsFrameRoleV1::RentSysvar => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    rent_sysvar.clone(),
                ),
                ClaimsFrameRoleV1::CoreMarket => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    market.clone(),
                ),
                ClaimsFrameRoleV1::ActivationCache => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    activation.clone(),
                ),
                ClaimsFrameRoleV1::RegistryProgram => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    registry.clone(),
                ),
                ClaimsFrameRoleV1::CallerProgram => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    trading_program.clone(),
                ),
                ClaimsFrameRoleV1::CallerProgramData => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    trading_programdata.clone(),
                ),
                ClaimsFrameRoleV1::ClaimsProgram => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    claims_program.clone(),
                ),
                ClaimsFrameRoleV1::ClaimsProgramData => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    claims_programdata.clone(),
                ),
                ClaimsFrameRoleV1::CoreProgram => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    core_program.clone(),
                ),
                ClaimsFrameRoleV1::CoreProgramData => (
                    frame.claims_fixed_start + u32::from(ordinal),
                    core_programdata.clone(),
                ),
                ClaimsFrameRoleV1::SignedDeltaPosition(position) => {
                    let descriptor = request
                        .claims_plan()
                        .expect("scenario Claims plan")
                        .position(u32::from(position))
                        .expect("scenario Claims position descriptor");
                    let binding = if descriptor.owner() == scenario.dealer.pubkey().to_bytes() {
                        dealer_position.clone()
                    } else if descriptor.owner() == chain.counterparty_position.position_owner {
                        lp_position.clone()
                    } else {
                        panic!("scenario Claims packet names an unrelated Position owner")
                    };
                    (frame.claims_positions_start + u32::from(position), binding)
                }
                other => panic!("unexpected scenario SignedDelta role {other:?}"),
            };
            bindings.push((
                usize::try_from(coordinate).expect("logical coordinate"),
                binding,
            ));
        }

        let route_spans = [
            semantic.dynamic_span_counts[0],
            semantic.dynamic_span_counts[1],
            semantic.dynamic_span_counts[2],
            semantic.dynamic_span_counts[3],
            semantic.dynamic_span_counts[5],
            semantic.dynamic_span_counts[6],
        ];
        let mut effects = semantic.custody_effects.iter().copied().flatten();
        for (slot, span) in route_spans.into_iter().enumerate() {
            if span == 0 {
                continue;
            }
            let effect = effects.next().expect("active scenario Custody effect");
            let custody_request = effect.request.custody();
            let spec = CustodyFrameSpecV1::new(custody_request.operation);
            assert_eq!(span, u32::from(spec.account_count()));
            let source = built_data_account(
                &rent,
                Pubkey::new_from_array(custody_request.source),
                scenario.delivery.token_program,
                if custody_request.source == collateral.principal.to_bytes() {
                    collateral.principal_bytes.clone()
                } else if custody_request.source == collateral.hoard.to_bytes() {
                    collateral.hoard_bytes.clone()
                } else if custody_request.source == collateral.external.to_bytes() {
                    collateral.external_bytes.clone()
                } else if custody_request.source == collateral_frame.fee_vault {
                    fee.account.data.clone()
                } else {
                    panic!("scenario Custody source is outside the authenticated collateral frame")
                },
            );
            let destination = built_data_account(
                &rent,
                Pubkey::new_from_array(custody_request.destination),
                scenario.delivery.token_program,
                if custody_request.destination == collateral.principal.to_bytes() {
                    collateral.principal_bytes.clone()
                } else if custody_request.destination == collateral.hoard.to_bytes() {
                    collateral.hoard_bytes.clone()
                } else if custody_request.destination == collateral.external.to_bytes() {
                    collateral.external_bytes.clone()
                } else if custody_request.destination == collateral_frame.fee_vault {
                    fee.account.data.clone()
                } else {
                    panic!(
                        "scenario Custody destination is outside the authenticated collateral frame"
                    )
                },
            );
            for ordinal in 0..spec.account_count() {
                let role = spec.account(ordinal).expect("scenario Custody role").role();
                let binding = match role {
                    CustodyFrameRoleV1::CallerAuthority => continue,
                    CustodyFrameRoleV1::CoreMarket => market.clone(),
                    CustodyFrameRoleV1::ActivationCache => activation.clone(),
                    CustodyFrameRoleV1::RegistryProgram => registry.clone(),
                    CustodyFrameRoleV1::CallerProgram => trading_program.clone(),
                    CustodyFrameRoleV1::CallerProgramData => trading_programdata.clone(),
                    CustodyFrameRoleV1::RealmRecord => realm_raw.clone(),
                    CustodyFrameRoleV1::RealmStaging => realm_staging.clone(),
                    CustodyFrameRoleV1::Replay => replay.clone(),
                    CustodyFrameRoleV1::Mint => mint.clone(),
                    CustodyFrameRoleV1::TransferSource => source.clone(),
                    CustodyFrameRoleV1::TransferDestination => destination.clone(),
                    CustodyFrameRoleV1::CustodyAuthority => authority.clone(),
                    CustodyFrameRoleV1::TokenProgram => token_program.clone(),
                    other => panic!("unexpected Transfer frame role {other:?}"),
                };
                let coordinate = frame.custody_starts[slot] + u32::from(ordinal);
                bindings.push((
                    usize::try_from(coordinate).expect("Custody coordinate"),
                    binding,
                ));
            }
        }
        assert!(
            effects.next().is_none(),
            "every scenario effect owns one active route"
        );

        bindings.push((
            usize::try_from(frame.obligation).expect("obligation coordinate"),
            obligation,
        ));
        bindings.push((
            usize::try_from(frame.custody_program).expect("Custody program coordinate"),
            custody_program,
        ));
        let mut evidence = frame.evidence_start;
        if semantic.dynamic_span_counts[1] == 0 && semantic.dynamic_span_counts[2] == 0 {
            bindings.push((usize::try_from(evidence).expect("fee evidence"), fee));
            evidence += 1;
        }
        if semantic.dynamic_span_counts[3] == 0 && semantic.dynamic_span_counts[5] == 0 {
            bindings.push((usize::try_from(evidence).expect("Hoard evidence"), hoard));
            evidence += 1;
        }
        if request.dealer_evidence_count != 0 {
            assert_eq!(request.dealer_evidence_count, 1);
            bindings.push((
                usize::try_from(evidence).expect("Dealer evidence"),
                dealer_position,
            ));
            evidence += 1;
        }
        assert_eq!(evidence, frame.scratch_start, "exact scenario evidence row");

        let external_programdata = [
            scenario.waist.claims_programdata,
            scenario.waist.custody_programdata,
            scenario.delivery.token_program,
        ];
        let scenario_input = ScenarioV1 {
            family_request,
            tail_count: fixture.outcome_count,
            clock_slot: clock.slot,
            generation: SCENARIO_GENERATION,
            ed25519_evidence: None,
            native_message_instruction_index: 0,
            externally_installed_extra: &external_programdata,
            payer: context.payer.pubkey(),
        };
        let bundle_input = BundleInputV1 {
            set: campaign.scenario_artifacts(),
            waist: waist(scenario),
            scenario: scenario_input,
            fixed: fixed_for_fixture(scenario, campaign, fixture, &rent),
            bindings: &bindings,
            rent: &rent,
        };
        let hostile_candidate = |scalars: &mut [u64], _: &mut [[u8; 32]]| {
            if let Some(first) = scalars.first_mut() {
                *first ^= 1;
            }
            Err(BuilderError::Projection("hostile-scenario-candidate"))
        };
        assert!(matches!(
            build_admitted_bundle_with_candidate_v1(
                &bundle_input,
                campaign.scenario_admitted(),
                &hostile_candidate,
            ),
            Err(BuilderError::Projection("admitted-candidate"))
        ));
        let honest_candidate = |scalars: &mut [u64], identities: &mut [[u8; 32]]| {
            if scalars.len() != semantic.candidate_scalars.len()
                || identities.len() != semantic.candidate_identities.len()
            {
                return Err(BuilderError::Projection("dealer-scenario-candidate-width"));
            }
            scalars.copy_from_slice(&semantic.candidate_scalars);
            identities.copy_from_slice(&semantic.candidate_identities);
            Ok(())
        };
        let admitted = build_admitted_bundle_with_candidate_v1(
            &bundle_input,
            campaign.scenario_admitted(),
            &honest_candidate,
        )
        .unwrap_or_else(|error| panic!("physical scenario bundle refused at {error:?}"));
        assert_eq!(admitted.bundle.span_counts, semantic.dynamic_span_counts);
        BuiltScenarioStep { admitted, semantic }
    }

    #[allow(clippy::too_many_arguments)]
    async fn split_scenario_from_admitted_trade(
        context: &mut ProgramTestContext,
        base: &super::Scenario,
        campaign: &LpCampaign,
        fixture: &NarrowFixtureV2,
        live: &super::LiveClaimsGraph,
        obligation_state: &[u8],
        collateral: &EquityCollateralState,
        family_request: &[u8],
        chain: ScenarioTradeChainProjectionV3<'_>,
        composer: ScenarioComposerContextV3,
        collateral_frame: ScenarioCollateralFrameV3,
        physical: &BuiltScenarioStep,
    ) -> super::Scenario {
        install_bundle(context, &physical.admitted);
        let observed_clock = context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .expect("admitted scenario Clock");
        assert_eq!(observed_clock.slot, chain.now);
        let observation = Observation {
            slot: chain.now,
            unix_timestamp: observed_clock.unix_timestamp,
            finality: Finality::Finalized,
        };
        let hot_accounts = &physical.admitted.bundle.hot_instruction.accounts;
        let projection = dealer_hot_frame_projection_v4();
        let strategy_count = projection
            .admitted_evidence_count
            .checked_add(physical.semantic.caller_authority_count)
            .expect("scenario strategy width");
        let runtime_start = projection
            .fixed_account_count
            .checked_add(strategy_count)
            .expect("scenario runtime start");
        assert!(
            hot_accounts.len() >= runtime_start,
            "admitted Hot instruction carries the complete common and strategy frames"
        );
        let mut observed = Vec::with_capacity(hot_accounts.len());
        for meta in hot_accounts {
            let account = context
                .banks_client
                .get_account(meta.pubkey)
                .await
                .expect("read admitted physical account")
                .unwrap_or_default();
            observed.push(ObservedAccountMetaV3 {
                account: ObservedAccount {
                    observation,
                    key: meta.pubkey,
                    owner: account.owner,
                    lamports: account.lamports,
                    executable: account.executable,
                    data: account.data,
                },
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            });
        }
        let (fixed_accounts, tail) = observed.split_at(projection.fixed_account_count);
        let (strategy_accounts, runtime_suffix_accounts) = tail.split_at(strategy_count);
        let meta_state = DealerScenarioHotMetaStateV4 {
            fixed_accounts,
            strategy_accounts,
            runtime_suffix_accounts,
        };
        let semantic_state = DealerScenarioSemanticStateV4 {
            chain,
            context: composer,
            collateral: collateral_frame,
        };
        let unsplit =
            project_dealer_scenario_unsplit_topology_v4(meta_state, semantic_state, family_request)
                .expect("admitted scenario projects its exact unsplit topology");
        assert_eq!(
            unsplit.report.dynamic_span_counts,
            physical.semantic.dynamic_span_counts
        );
        assert!(
            unsplit.unique_account_lock_count > SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1,
            "the admitted scenario uses the split because its exact unsplit topology exceeds 64 locks"
        );

        let request_digest = hash(family_request).to_bytes();
        let checkpoint = dealer_scenario_checkpoint_address_v1(TRADING, request_digest);
        let canonical = project_dealer_scenario_canonical_membership_pages_v1(
            meta_state,
            MANIFEST_PRODUCER,
            checkpoint,
            request_digest,
        )
        .expect("admitted scenario canonical membership");
        let membership_manifest = dealer_scenario_membership_manifest_address_v1(
            MANIFEST_PRODUCER,
            checkpoint,
            request_digest,
        );
        let manifest_bytes = canonical
            .manifest
            .encode()
            .expect("scenario membership manifest encodes")
            .to_vec();
        let mut membership = observed
            .iter()
            .map(|meta| meta.account.key)
            .collect::<Vec<_>>();
        membership.sort_unstable_by_key(Pubkey::to_bytes);
        membership.dedup();
        let mut frame_accounts = observed
            .iter()
            .map(|meta| {
                (
                    meta.account.key,
                    Account {
                        lamports: meta.account.lamports,
                        data: meta.account.data.clone(),
                        owner: meta.account.owner,
                        executable: meta.account.executable,
                        rent_epoch: 0,
                    },
                )
            })
            .collect::<Vec<_>>();
        frame_accounts.sort_by_key(|(key, _)| key.to_bytes());
        frame_accounts.dedup_by_key(|(key, _)| *key);

        let effect = physical
            .semantic
            .custody_effects
            .iter()
            .copied()
            .flatten()
            .next()
            .expect("scenario owns one Custody effect");
        assert_eq!(physical.semantic.semantic_plan.custody_count, 1);
        let custody_request = effect.request.custody();
        assert_eq!(custody_request.source, collateral.principal.to_bytes());
        assert_eq!(custody_request.destination, collateral.external.to_bytes());
        let destination_before = effect
            .destination_after
            .checked_sub(custody_request.amount)
            .expect("scenario destination prestate");
        let delivery = stage_dealer_delivery_v1(DealerDeliveryInputV1 {
            custody_program: base.waist.custody_program,
            trading_program: TRADING,
            release_set: base.waist.release_set_id,
            market: fixture.core_market,
            realm: base.realm.digest,
            context: campaign.root.to_bytes(),
            source_vault_context: campaign.root.to_bytes(),
            generation: SCENARIO_GENERATION,
            checkpoint,
            request_digest,
            destination: collateral.external,
            destination_owner: Pubkey::new_from_array(chain.counterparty_position.position_owner),
            replay_rent_refund: BENEFICIARY,
            amount: custody_request.amount,
            source_after: effect.source_after,
            destination_before,
            replay_revision: custody_request.expected_revision,
        });
        assert_eq!(
            MultiLpCustodyRequestV3::Canonical(delivery.request),
            effect.request,
            "the split delivery is the admitted Hot candidate's exact Custody effect"
        );
        assert_eq!(delivery.replay, collateral.replay);
        assert_eq!(delivery.source, collateral.principal);
        assert_eq!(delivery.destination, collateral.external);

        let waist = super::release_waist();
        assert_eq!(waist.release_set_id, base.waist.release_set_id);
        assert_eq!(waist.custody_program, base.waist.custody_program);
        let split = super::Scenario {
            dealer: base.dealer.insecure_clone(),
            child_root: campaign.root,
            counterparty: Pubkey::new_from_array(chain.counterparty_position.position_owner),
            counterparty_account: collateral.external,
            request_bytes: family_request.to_vec(),
            request_digest,
            obligation: campaign.obligation,
            obligation_state: obligation_state.to_vec(),
            checkpoint,
            membership_manifest,
            manifest_bytes,
            pages: canonical.pages,
            membership,
            frame_accounts,
            candidate_obligation_bytes: physical.semantic.candidate_obligation_state.clone(),
            fixture: fixture.clone(),
            live: live.clone(),
            core_market: fixture.core_market,
            unsplit_account_lock_count: unsplit.unique_account_lock_count,
            waist,
            realm: base.realm.clone(),
            delivery,
            custody_effect: effect,
        };
        context.set_account(
            &REQUEST,
            &AccountSharedData::from(super::data_account(TRADING, family_request.to_vec())),
        );
        context.set_account(
            &split.membership_manifest,
            &AccountSharedData::from(super::data_account(
                MANIFEST_PRODUCER,
                split.manifest_bytes.clone(),
            )),
        );
        for (key, account) in &split.frame_accounts {
            context.set_account(key, &AccountSharedData::from(account.clone()));
        }
        split
    }

    async fn chain_account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
        context
            .banks_client
            .get_account(key)
            .await
            .expect("read chain account")
            .expect("chain account exists")
    }

    async fn assert_installed_logical_views(
        context: &mut ProgramTestContext,
        admitted: &dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
    ) {
        for coordinate in 0..admitted.bundle.logical.len() {
            let Some(expected) = admitted.bundle.logical.get(coordinate) else {
                continue;
            };
            let observed = context
                .banks_client
                .get_account(expected.key)
                .await
                .expect("read installed logical account")
                .unwrap_or_default();
            assert_eq!(
                observed,
                *expected.chain_view(),
                "installed logical coordinate {coordinate} at {} differs from the admitted transcript",
                expected.key
            );
        }
    }

    async fn submit_lp_hot(
        context: &mut ProgramTestContext,
        instruction: Instruction,
        extra_signers: &[&Keypair],
    ) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
        let blockhash = context.banks_client.get_latest_blockhash().await?;
        let payer = context.payer.insecure_clone();
        let mut signers: Vec<&Keypair> = vec![&payer];
        signers.extend_from_slice(extra_signers);
        let heap = solana_compute_budget_interface::ComputeBudgetInstruction::request_heap_frame(
            DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        );
        let compute =
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
                1_400_000,
            );
        let instruction_data = instruction.data.clone();
        let transaction = Transaction::new_signed_with_payer(
            &[compute, heap, instruction],
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let wire_bytes = 1_usize
            .checked_add(64)
            .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
            .expect("bounded transaction wire");
        let signature = transaction
            .signatures
            .first()
            .copied()
            .expect("a signed transaction has a signature")
            .to_string();
        let slot = context
            .banks_client
            .get_sysvar::<solana_program::clock::Clock>()
            .await
            .map_or(0, |clock| clock.slot);
        let processed = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await?;
        super::record_campaign_transaction(
            &signature,
            slot,
            Some(wire_bytes),
            &instruction_data,
            &processed,
        );
        Ok(processed)
    }

    #[test]
    fn mixed_dealer_set_selects_only_finalized_v4_equity_descriptors() {
        let scenario = super::scenario();
        let campaign = campaign(&scenario);
        let lp_owner = Keypair::new();
        let root_bound = equity_fixture(&scenario, &campaign, lp_owner.pubkey());
        let root_bound_live = super::live_claims_graph(&root_bound);
        let aggregate = LiabilityBasisMarketViewV2::decode(&root_bound_live.market)
            .expect("root-bound Claims aggregate");
        assert_eq!(aggregate.custody_context, campaign.root.to_bytes());
        assert_eq!(root_bound.reserve_position.owner, lp_owner.pubkey());
        let set = CapabilityProgramSetV2::decode(&campaign.program_set).expect("Dealer SetV2");
        for (selector, artifacts) in (1_u16..=6).zip(&campaign.equity) {
            let artifact_set =
                artifacts.set(&campaign.program_set, &campaign.manifest, &campaign.config);
            AccountProfileV2::decode(artifact_set.account_profile)
                .expect("generated Equity profile hostile-decodes");
            assert!(
                !artifact_set.request_profile.is_empty()
                    && !artifact_set.transition.is_empty()
                    && !artifact_set.effect.is_empty()
                    && !artifact_set.lifecycle.is_empty()
                    && !artifact_set.strategy.is_empty()
                    && !artifacts.certificate.is_empty()
                    && !artifacts.admission.is_empty(),
                "selector {selector} carries its complete admitted artifact closure"
            );
            let mut request = [0_u8; 12];
            request[10..12].copy_from_slice(&selector.to_le_bytes());
            let selected = set.select_entry(&request).expect("equity selector");
            assert_eq!(selected.selector(), u32::from(selector));
            assert_eq!(
                selected.descriptor().schema().to_bytes(),
                CAPABILITY_PROGRAM_SCHEMA_V4
            );
            assert_eq!(
                selected.descriptor().program().to_bytes(),
                hash(&artifacts.descriptor).to_bytes(),
                "selector {selector} is bound to its generated V4 descriptor"
            );
            CapabilityProgramV4::decode(&artifacts.descriptor)
                .expect("generated Equity descriptor hostile-decodes");
        }
    }

    #[tokio::test]
    async fn accepted_equity_selector_one_executes_real_custody_and_rolls_back_late_evidence_refusal()
     {
        let scenario = super::scenario();
        let campaign = campaign(&scenario);
        let lp_owner = Keypair::new();
        let fixture = equity_fixture(&scenario, &campaign, lp_owner.pubkey());
        let live = super::live_claims_graph(&fixture);
        assert!(live.counterparty_balances.iter().all(|value| *value == 0));
        let obligation_bytes = super::obligation_bytes(
            scenario.fixture.core_market.to_bytes(),
            scenario.fixture.product_id,
            scenario.fixture.semantic_basis_id,
            scenario.dealer.pubkey().to_bytes(),
            campaign.root.to_bytes(),
            7,
            &live.dealer_balances,
        );
        let initial_obligation =
            DealerObligationProjectionV3::decode(&obligation_bytes).expect("initial obligation");
        assert_eq!(initial_obligation.total_equity_shares(), 0);

        let mut test = super::program_test_with_transaction_compute(&scenario);
        test.add_upgradeable_program_to_genesis("dclutch_dealer_accelerator_sbf", &ACCELERATOR);
        test.add_account(
            super::programdata_address(ACCELERATOR),
            super::data_account(
                bpf_loader_upgradeable::ID,
                campaign.accelerator_programdata.account.data.clone(),
            ),
        );
        let mut context = test.start_with_context().await;
        context.set_account(
            &scenario.fixture.core_market,
            &AccountSharedData::from(super::data_account(
                scenario.waist.core_program,
                campaign.market_bytes.clone(),
            )),
        );
        context.set_account(
            &lp_owner.pubkey(),
            &AccountSharedData::from(Account {
                lamports: 10_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
        let (credit, credit_state) = lifecycle_credit(&scenario, lp_owner.pubkey());
        let credit_account =
            super::data_account(scenario.waist.registry, credit_state.to_bytes().to_vec());
        context.set_account(&credit, &AccountSharedData::from(credit_account.clone()));
        let open = build_lifecycle_bundle(
            &mut context,
            &scenario,
            &campaign,
            MultiLpRequestActionV3::Open,
            lp_owner.pubkey(),
            &obligation_bytes,
            None,
            credit,
            credit_account,
        )
        .await;
        let lp_position = open.bundle.logical.get(6).expect("LP coordinate").key;
        install_bundle(&mut context, &open);
        let opened = submit_lp_hot(
            &mut context,
            open.bundle.hot_instruction.clone(),
            &[&lp_owner],
        )
        .await
        .expect("submit LP Open");
        assert!(opened.result.is_ok(), "LP Open: {:?}", opened.result);
        let opened_lp = chain_account(&mut context, lp_position).await;

        let collateral =
            first_equity_collateral(&scenario, &campaign, lp_owner.pubkey(), 0xf4, 100, 10);
        let zero_claims = vec![
            0_u64;
            usize::try_from(scenario.fixture.outcome_count)
                .expect("small outcome width")
        ];
        let first_add = EquityRequestIntentV3::Contribute {
            collateral: 10,
            claims: &zero_claims,
            minted_shares: 10,
        };
        let honest = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &live,
            lp_owner.pubkey(),
            &obligation_bytes,
            &opened_lp,
            &collateral,
            first_add,
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
            None,
        )
        .await;
        let hostile = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &live,
            lp_owner.pubkey(),
            &obligation_bytes,
            &opened_lp,
            &collateral,
            first_add,
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
            Some((
                scenario.fixture.reserve_position.account,
                scenario.live.counterparty_position.clone(),
            )),
        )
        .await;
        install_bundle(&mut context, &hostile.admitted);
        let rollback_keys = [
            campaign.root,
            campaign.obligation,
            lp_position,
            collateral.replay,
            collateral.external,
            collateral.principal,
            collateral.hoard,
            fixture.claims_market,
            fixture.actor_position.account,
            scenario.fixture.reserve_position.account,
        ];
        let mut rollback_before = Vec::new();
        for key in rollback_keys {
            rollback_before.push((key, chain_account(&mut context, key).await));
        }
        let refused = submit_lp_hot(
            &mut context,
            hostile.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit hostile equity Add");
        assert_eq!(
            super::custom_code(&refused.result),
            Some(TradingSbfError::Content as u32),
            "a substituted Position identity must refuse the authenticated admitted transcript before candidate execution: {:?}",
            refused.result,
        );
        assert!(
            !super::invoked_programs(&refused).contains(&ACCELERATOR),
            "the admitted transcript must reject the substituted identity before the accelerator"
        );
        for (key, before) in rollback_before {
            assert_eq!(
                chain_account(&mut context, key).await,
                before,
                "rollback {key}"
            );
        }

        install_bundle(&mut context, &honest.admitted);
        assert_installed_logical_views(&mut context, &honest.admitted).await;
        let accepted = submit_lp_hot(
            &mut context,
            honest.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit equity Add");
        assert!(accepted.result.is_ok(), "equity Add: {:?}", accepted.result);
        let invoked = super::invoked_programs(&accepted);
        assert!(invoked.contains(&TRADING));
        assert!(invoked.contains(&ACCELERATOR));
        assert!(invoked.contains(&scenario.waist.custody_program));
        assert!(
            !invoked.contains(&scenario.waist.claims_program),
            "selector 1 has no SignedDelta suffix, so Claims CPI must be canonically absent"
        );

        let external_after = chain_account(&mut context, collateral.external).await;
        let principal_after = chain_account(&mut context, collateral.principal).await;
        let hoard_after = chain_account(&mut context, collateral.hoard).await;
        let external_token = SplTokenAccount::unpack(&external_after.data).expect("external token");
        let principal_token =
            SplTokenAccount::unpack(&principal_after.data).expect("principal token");
        let hoard_token = SplTokenAccount::unpack(&hoard_after.data).expect("hoard token");
        assert_eq!(external_token.amount, 90);
        assert_eq!(external_token.delegate, ProgramCOption::None);
        assert_eq!(external_token.delegated_amount, 0);
        assert_eq!(principal_token.amount, 10);
        assert_eq!(principal_token.owner, collateral.authority);
        assert_eq!(hoard_token.amount, 0);
        assert_eq!(
            super::token_account_amount(&collateral.external_bytes)
                + super::token_account_amount(&collateral.principal_bytes)
                + super::token_account_amount(&collateral.hoard_bytes),
            external_token.amount + principal_token.amount + hoard_token.amount,
            "the exact collateral atom is conserved"
        );
        let replay_after =
            CustodyReplayV1::decode(&chain_account(&mut context, collateral.replay).await.data)
                .expect("advanced replay");
        assert_eq!(replay_after.next_revision, 8);
        assert_eq!(replay_after.open_vault_count, 2);

        let mut expected_obligation = vec![0_u8; obligation_bytes.len()];
        stage_equity_share_supply_v3(
            initial_obligation,
            dclutch_trading_sbf::dealer::v3_obligation::EquityShareDeltaV3::Mint(10),
            &mut expected_obligation,
        )
        .expect("expected share supply");
        assert_eq!(
            chain_account(&mut context, campaign.obligation).await.data,
            expected_obligation
        );
        let lp_after = chain_account(&mut context, lp_position).await;
        let decoded_lp = DealerLpPositionV3::decode(&lp_after.data).expect("funded LP Position");
        assert_eq!(decoded_lp.revision, 2);
        assert_eq!(decoded_lp.equity_shares, 10);
        assert_eq!(
            chain_account(&mut context, fixture.claims_market)
                .await
                .data,
            live.market,
            "P0 does not rewrite the Claims aggregate"
        );
        assert_eq!(
            chain_account(&mut context, fixture.actor_position.account)
                .await
                .data,
            live.dealer_position,
            "P0 does not rewrite Dealer Claims"
        );
        assert_eq!(
            chain_account(&mut context, fixture.reserve_position.account)
                .await
                .data,
            live.counterparty_position,
            "P0 does not rewrite LP Claims"
        );
        assert_eq!(
            chain_account(&mut context, campaign.root).await.data,
            campaign.root_bytes
        );

        // A second independently owned LP joins only after the first Add has
        // committed. Its Open and contribution consume the actual obligation,
        // shared Custody replay and vault poststates rather than replaying the
        // first LP's fixture.
        let second_lp = Keypair::new();
        context.set_account(
            &second_lp.pubkey(),
            &AccountSharedData::from(Account {
                lamports: 10_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
        let second_fixture = equity_fixture(&scenario, &campaign, second_lp.pubkey());
        let second_live = super::live_claims_graph(&second_fixture);
        assert_eq!(second_fixture.claims_market, fixture.claims_market);
        assert_eq!(
            second_fixture.actor_position.account,
            fixture.actor_position.account
        );
        assert_ne!(
            second_fixture.reserve_position.account,
            fixture.reserve_position.account
        );
        assert_eq!(second_live.market, live.market);
        assert_eq!(second_live.dealer_position, live.dealer_position);

        let obligation_after_first = chain_account(&mut context, campaign.obligation).await.data;
        let credit_after_first = chain_account(&mut context, credit).await;
        let second_open = build_lifecycle_bundle(
            &mut context,
            &scenario,
            &campaign,
            MultiLpRequestActionV3::Open,
            second_lp.pubkey(),
            &obligation_after_first,
            None,
            credit,
            credit_after_first,
        )
        .await;
        let second_position = second_open
            .bundle
            .logical
            .get(6)
            .expect("second LP coordinate")
            .key;
        install_bundle(&mut context, &second_open);
        let second_opened = submit_lp_hot(
            &mut context,
            second_open.bundle.hot_instruction.clone(),
            &[&second_lp],
        )
        .await
        .expect("submit second LP Open");
        assert!(
            second_opened.result.is_ok(),
            "second LP Open: {:?}",
            second_opened.result
        );
        let second_opened_lp = chain_account(&mut context, second_position).await;
        assert_eq!(
            DealerLpPositionV3::decode(&second_opened_lp.data)
                .expect("second opened LP")
                .equity_shares,
            0
        );

        let add_slot = context
            .banks_client
            .get_sysvar::<Clock>()
            .await
            .expect("post-first-Add Clock")
            .slot;
        context
            .warp_to_slot(add_slot + 5)
            .expect("time-separate independent LP Add");
        let mut second_collateral =
            first_equity_collateral(&scenario, &campaign, second_lp.pubkey(), 0xf5, 100, 7);
        second_collateral.replay_bytes = chain_account(&mut context, second_collateral.replay)
            .await
            .data;
        second_collateral.principal_bytes =
            chain_account(&mut context, second_collateral.principal)
                .await
                .data;
        second_collateral.hoard_bytes = chain_account(&mut context, second_collateral.hoard)
            .await
            .data;
        let seven_claims = vec![0_u64; zero_claims.len()];
        let second_add = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &second_fixture,
            &second_live,
            second_lp.pubkey(),
            &obligation_after_first,
            &second_opened_lp,
            &second_collateral,
            EquityRequestIntentV3::Contribute {
                collateral: 7,
                claims: &seven_claims,
                minted_shares: 7,
            },
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
            None,
        )
        .await;
        assert_eq!(second_add.plan.shares_before, 10);
        assert_eq!(second_add.plan.shares_after, 17);
        assert_eq!(second_add.plan.collateral_in, 7);
        assert_eq!(second_add.plan.collateral_after, 17);
        assert_eq!(second_add.dealer_claims_after, live.dealer_balances);
        assert_eq!(
            second_add.lp_claims_after,
            second_live.counterparty_balances
        );
        install_bundle(&mut context, &second_add.admitted);
        let second_added = submit_lp_hot(
            &mut context,
            second_add.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit second independent equity Add");
        assert!(
            second_added.result.is_ok(),
            "second independent equity Add: {:?}",
            second_added.result
        );
        let second_external = SplTokenAccount::unpack(
            &chain_account(&mut context, second_collateral.external)
                .await
                .data,
        )
        .expect("second LP external token");
        assert_eq!(second_external.amount, 93);
        assert_eq!(second_external.delegate, ProgramCOption::None);
        let shared_principal = SplTokenAccount::unpack(
            &chain_account(&mut context, second_collateral.principal)
                .await
                .data,
        )
        .expect("shared principal after second Add");
        assert_eq!(shared_principal.amount, second_add.plan.collateral_after);
        let second_funded_lp =
            DealerLpPositionV3::decode(&chain_account(&mut context, second_position).await.data)
                .expect("second funded LP");
        assert_eq!(second_funded_lp.equity_shares, 7);
        let first_undiluted_lp =
            DealerLpPositionV3::decode(&chain_account(&mut context, lp_position).await.data)
                .expect("first LP after second Add");
        assert_eq!(first_undiluted_lp.equity_shares, 10);
        let obligation_after_second = chain_account(&mut context, campaign.obligation).await.data;
        assert_eq!(
            DealerObligationProjectionV3::decode(&obligation_after_second)
                .expect("post-second-Add obligation")
                .total_equity_shares(),
            second_add.plan.shares_after
        );

        // Compose the inventory-bounded trade against this exact root and
        // these exact post-Add accounts.  The real scenario composer, not the
        // campaign, chooses the Claims/Custody consequences.  Execution is
        // inserted below through the existing split checkpoint path; keeping
        // construction here makes a parallel fixture impossible.
        let trade_claims_before = current_equity_claims(&mut context, &fixture).await;
        let trade_market = LiabilityBasisMarketViewV2::decode(&trade_claims_before.market)
            .expect("pre-trade Claims aggregate");
        let trade_dealer_position =
            LiabilityBasisPositionViewV2::decode(&trade_claims_before.dealer_position)
                .expect("pre-trade Dealer Position");
        let trade_lp_position =
            LiabilityBasisPositionViewV2::decode(&trade_claims_before.counterparty_position)
                .expect("pre-trade LP Position");
        let trade_obligation = DealerObligationProjectionV3::decode(&obligation_after_second)
            .expect("pre-trade obligation");
        let trade_principal_account = chain_account(&mut context, collateral.principal).await;
        let trade_principal = SplTokenAccount::unpack(&trade_principal_account.data)
            .expect("pre-trade principal token");
        let trade_external_account = chain_account(&mut context, collateral.external).await;
        let trade_external = SplTokenAccount::unpack(&trade_external_account.data)
            .expect("pre-trade LP external token");
        let trade_chain = ScenarioTradeChainProjectionV3 {
            trading_program: TRADING.to_bytes(),
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            child_root: campaign.root.to_bytes(),
            obligation_address: campaign.obligation.to_bytes(),
            current_obligation: trade_obligation,
            dealer_position: ClaimsInventoryObservation {
                market_id: scenario.fixture.core_market.to_bytes(),
                product_id: scenario.fixture.product_id,
                liability_basis_id: scenario.fixture.semantic_basis_id,
                position_owner: scenario.dealer.pubkey().to_bytes(),
                revision: trade_dealer_position.revision,
                inventory: &trade_claims_before.dealer_balances,
            },
            counterparty_position: ClaimsInventoryObservation {
                market_id: scenario.fixture.core_market.to_bytes(),
                product_id: scenario.fixture.product_id,
                liability_basis_id: scenario.fixture.semantic_basis_id,
                position_owner: lp_owner.pubkey().to_bytes(),
                revision: trade_lp_position.revision,
                inventory: &trade_claims_before.counterparty_balances,
            },
            product_record_digest: fixture.product.digest,
            linked_basis_record_digest: fixture.linked_basis.digest,
            counterparty_account: collateral.external.to_bytes(),
            principal_balance: trade_principal.amount,
            locked_capital_floor: 0,
            claims_revision: trade_market.revision,
            generation: SCENARIO_GENERATION,
            now: context
                .banks_client
                .get_sysvar::<Clock>()
                .await
                .expect("pre-trade Clock")
                .slot,
            expires_at: SCENARIO_EXPIRES_AT,
            terminal: false,
            basis_scale: 1,
        };
        let trade_acquired = vec![0_u64; trade_claims_before.dealer_balances.len()];
        let mut trade_delivered = vec![0_u64; trade_claims_before.dealer_balances.len()];
        *trade_delivered
            .get_mut(FUNDED_COORDINATE)
            .expect("funded trade coordinate") = 5;
        let trade_candidate_obligations = trade_obligation.obligations().collect::<Vec<_>>();
        let trade_intent = ScenarioTradeIntentV3 {
            direction: ScenarioTradeDirectionV3::DealerPaysCounterparty,
            principal: 5,
            realized_fee: 0,
            acquired: &trade_acquired,
            delivered: &trade_delivered,
            candidate_obligations: &trade_candidate_obligations,
        };
        let campaign_set =
            CapabilityProgramSetV2::decode(&campaign.program_set).expect("mixed campaign SetV2");
        let scenario_reference = CapabilityDescriptorReferenceV2::new(
            core_id(CAPABILITY_PROGRAM_SCHEMA_V4),
            core_id(hash(&campaign.scenario.descriptor).to_bytes()),
        );
        let mut trade_request_bytes =
            vec![
                0_u8;
                scenario_trade_max_request_bytes_v3(scenario.fixture.outcome_count)
                    .expect("trade request bound")
            ];
        let unsigned_trade = build_scenario_trade_request_v4(
            trade_chain,
            trade_intent,
            campaign_set,
            scenario_reference,
            &mut trade_request_bytes,
        )
        .expect("same-root inventory-bounded trade request");
        trade_request_bytes.truncate(unsigned_trade.request_bytes());
        assert_eq!(
            unsigned_trade.selected_descriptor(),
            scenario_reference,
            "the canonical composer must preserve SetV2's exact schema/content authority"
        );
        let trade_request = DealerScenarioTradeRequestV3::decode(&trade_request_bytes)
            .expect("same-root trade request decodes");
        assert_eq!(
            campaign_set
                .select_descriptor(&trade_request_bytes)
                .expect("SetV2 reselects the completed scenario request"),
            scenario_reference,
            "the completed request must rejoin the same authenticated SetV2 authority"
        );
        assert_eq!(trade_request.child_root, campaign.root.to_bytes());
        assert_eq!(trade_request.obligation, campaign.obligation.to_bytes());
        assert_eq!(
            trade_request
                .claims_plan()
                .expect("same-root trade Claims plan")
                .position_count(),
            2
        );
        let trade_replay =
            CustodyReplayV1::decode(&chain_account(&mut context, collateral.replay).await.data)
                .expect("pre-trade replay");
        let fee_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                scenario.fixture.core_market.to_bytes(),
                scenario.waist.release_set_id,
                campaign.root.to_bytes(),
                CompartmentV1::FeeVault,
            )
            .as_slices(),
            &scenario.waist.custody_program,
        )
        .0;
        let trade_context = ScenarioComposerContextV3 {
            trading_program: TRADING.to_bytes(),
            custody_program: scenario.waist.custody_program.to_bytes(),
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            realm: scenario.realm.digest,
            child_root: campaign.root.to_bytes(),
            obligation_account: campaign.obligation.to_bytes(),
            mint: scenario.delivery.mint.to_bytes(),
            token_program: scenario.delivery.token_program.to_bytes(),
            parent_request_digest: hash(&trade_request_bytes).to_bytes(),
            generation: SCENARIO_GENERATION,
            custody_replay_revision: trade_replay.next_revision,
            locked_capital_floor: 0,
            basis_scale: 1,
        };
        let trade_frame = ScenarioCollateralFrameV3 {
            principal_vault: collateral.principal.to_bytes(),
            principal_balance: trade_principal.amount,
            fee_vault: fee_vault.to_bytes(),
            fee_balance: 0,
            hoard_vault: collateral.hoard.to_bytes(),
            hoard_balance: SplTokenAccount::unpack(
                &chain_account(&mut context, collateral.hoard).await.data,
            )
            .expect("pre-trade Hoard token")
            .amount,
            counterparty_account: collateral.external.to_bytes(),
            counterparty_owner: lp_owner.pubkey().to_bytes(),
            counterparty_external_delegate: match trade_external.delegate {
                ProgramCOption::Some(delegate) => delegate.to_bytes(),
                ProgramCOption::None => [0; 32],
            },
            counterparty_external_delegated_amount: trade_external.delegated_amount,
            counterparty_balance: trade_external.amount,
        };
        let mut trade_acquired_runtime = vec![0_u64; trade_acquired.len()];
        let mut trade_delivered_runtime = vec![0_u64; trade_delivered.len()];
        let mut trade_candidate_state = vec![0_u8; obligation_after_second.len()];
        let mut trade_obligations_before = vec![0_u64; trade_candidate_obligations.len()];
        let mut trade_obligations_after = vec![0_u64; trade_candidate_obligations.len()];
        let mut trade_dealer_after = vec![0_u64; trade_claims_before.dealer_balances.len()];
        let mut trade_lp_after = vec![0_u64; trade_claims_before.counterparty_balances.len()];
        let mut trade_equity_after = vec![0_i128; trade_claims_before.dealer_balances.len()];
        let mut trade_custody = [None; 4];
        let prepared_trade = dclutch_trading_sbf::dealer::v3_trade::prepare_scenario_trade_v3(
            trade_request,
            trade_chain,
            trade_context,
            trade_frame,
            &mut trade_acquired_runtime,
            &mut trade_delivered_runtime,
            &mut trade_candidate_state,
            &mut trade_obligations_before,
            &mut trade_obligations_after,
            &mut trade_dealer_after,
            &mut trade_lp_after,
            &mut trade_equity_after,
            &mut trade_custody,
        )
        .expect("same-root physical trade plan");
        let trade_plan = prepared_trade.plan;
        assert_eq!(trade_plan.custody_count, 1);
        assert_eq!(trade_plan.principal_after, 12);
        assert_eq!(trade_plan.counterparty_after, 95);
        assert_eq!(trade_dealer_after, [95, 0, 0]);
        assert_eq!(trade_lp_after, [5, 0, 0]);
        for coordinate in 0..trade_dealer_after.len() {
            assert_eq!(
                trade_dealer_after[coordinate] + trade_lp_after[coordinate],
                trade_claims_before.dealer_balances[coordinate]
                    + trade_claims_before.counterparty_balances[coordinate],
                "the bounded trade conserves Claims coordinate {coordinate}"
            );
        }
        let trade_collateral_state = current_equity_collateral(&mut context, &collateral).await;
        let physical_trade = build_scenario_bundle(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &trade_claims_before,
            &obligation_after_second,
            &trade_collateral_state,
            &trade_request_bytes,
            trade_chain,
            trade_context,
            trade_frame,
        )
        .await;
        assert_eq!(
            physical_trade.semantic.semantic_plan, trade_plan,
            "the admitted Hot candidate and the host preflight share one semantic plan"
        );
        assert_eq!(
            physical_trade.semantic.candidate_obligation_state, trade_candidate_state,
            "the admitted candidate carries the canonical post-trade obligation"
        );
        assert_eq!(
            physical_trade.admitted.bundle.span_counts,
            vec![0, 0, 0, 0, 2, 0, 14, 2, 6],
            "P2 Dealer-pays-counterparty derives route 5 plus fee/Hoard evidence"
        );
        let trade_scenario = split_scenario_from_admitted_trade(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &trade_claims_before,
            &obligation_after_second,
            &trade_collateral_state,
            &trade_request_bytes,
            trade_chain,
            trade_context,
            trade_frame,
            &physical_trade,
        )
        .await;
        let reservation =
            super::committed_with_delivery_inputs(&mut context, &trade_scenario).await;
        let activation_payer = context.payer.pubkey();
        let delivered_trade = super::submit_activation(
            &mut context,
            super::activation_bank(&trade_scenario, &reservation, activation_payer),
        )
        .await;
        assert!(
            delivered_trade.result.is_ok(),
            "same-root selector-9 Custody delivery: {:?}",
            delivered_trade.result
        );
        let post_trade_principal = chain_account(&mut context, collateral.principal).await;
        let post_trade_external = chain_account(&mut context, collateral.external).await;
        assert_eq!(
            super::token_account_amount(&post_trade_principal.data),
            trade_plan.principal_after
        );
        assert_eq!(
            super::token_account_amount(&post_trade_external.data),
            trade_plan.counterparty_after
        );
        assert_eq!(
            chain_account(&mut context, campaign.obligation).await.data,
            trade_candidate_state,
            "commit-last installs the exact scenario candidate obligation"
        );
        let claims_after_trade = current_equity_claims(&mut context, &fixture).await;
        assert_eq!(claims_after_trade.dealer_balances, trade_dealer_after);
        assert_eq!(claims_after_trade.counterparty_balances, trade_lp_after);
        let replay_after_trade =
            CustodyReplayV1::decode(&chain_account(&mut context, collateral.replay).await.data)
                .expect("post-trade Custody replay");
        assert_eq!(
            replay_after_trade.next_revision,
            trade_replay.next_revision + 1,
            "one scenario delivery advances the shared replay exactly once"
        );

        // Replaying the exact already-activated batch is a transaction-wide
        // hostile: no account is substituted, so the terminal batch/replay
        // invariant must refuse and preserve every economic poststate.
        let replay_hostile_keys = [
            campaign.obligation,
            fixture.claims_market,
            fixture.actor_position.account,
            fixture.reserve_position.account,
            collateral.principal,
            collateral.external,
            collateral.replay,
            reservation.batch,
        ];
        let mut replay_hostile_before = Vec::new();
        for key in replay_hostile_keys {
            replay_hostile_before.push((key, chain_account(&mut context, key).await));
        }
        let replayed_delivery = super::submit_activation(
            &mut context,
            super::activation_bank(&trade_scenario, &reservation, activation_payer),
        )
        .await;
        assert!(
            replayed_delivery.result.is_err(),
            "an activated selector-9 batch must refuse exact replay"
        );
        for (key, before) in replay_hostile_before {
            assert_eq!(
                chain_account(&mut context, key).await,
                before,
                "replayed selector-9 delivery rolls back {key}"
            );
        }
        let obligation_after_trade = chain_account(&mut context, campaign.obligation).await.data;

        // Burn only part of the first LP's position. The canonical evaluator,
        // not test arithmetic, supplies the exact floor-rounded payout and the
        // residual that must remain backing the untouched second LP.
        let first_collateral_before_partial =
            current_equity_collateral(&mut context, &collateral).await;
        let first_claims_before_partial = current_equity_claims(&mut context, &fixture).await;
        let first_lp_before_partial = chain_account(&mut context, lp_position).await;
        let partial_remove = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &first_claims_before_partial,
            lp_owner.pubkey(),
            &obligation_after_trade,
            &first_lp_before_partial,
            &first_collateral_before_partial,
            EquityRequestIntentV3::Redeem { burned_shares: 3 },
            DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
            None,
        )
        .await;
        assert_eq!(partial_remove.plan.shares_before, 17);
        assert_eq!(partial_remove.plan.shares_after, 14);
        assert_eq!(partial_remove.plan.collateral_out, 1);
        assert_eq!(partial_remove.plan.minimum_complete_sets_to_split, 1);
        assert_eq!(partial_remove.plan.maximum_complete_sets_to_merge, 0);
        assert_eq!(partial_remove.plan.collateral_after, 10);
        assert_eq!(partial_remove.dealer_claims_after, [96, 0, 0]);
        assert_eq!(partial_remove.lp_claims_after, [5, 1, 1]);
        install_bundle(&mut context, &partial_remove.admitted);
        let partially_removed = submit_lp_hot(
            &mut context,
            partial_remove.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit partial equity Remove");
        assert!(
            partially_removed.result.is_ok(),
            "partial equity Remove: {:?}",
            partially_removed.result
        );
        let partial_invoked = super::invoked_programs(&partially_removed);
        assert!(partial_invoked.contains(&ACCELERATOR));
        assert!(partial_invoked.contains(&scenario.waist.custody_program));
        assert!(
            partial_invoked.contains(&scenario.waist.claims_program),
            "post-trade P2 redemption must execute the real Claims ELF"
        );
        let first_external_after_partial =
            SplTokenAccount::unpack(&chain_account(&mut context, collateral.external).await.data)
                .expect("first LP external after partial Remove");
        assert_eq!(first_external_after_partial.amount, 96);
        let principal_after_partial =
            SplTokenAccount::unpack(&chain_account(&mut context, collateral.principal).await.data)
                .expect("principal after partial Remove");
        assert_eq!(
            principal_after_partial.amount,
            partial_remove.plan.collateral_after
        );
        assert_eq!(
            SplTokenAccount::unpack(&chain_account(&mut context, collateral.hoard).await.data)
                .expect("Hoard after partial Remove")
                .amount,
            1,
            "one split complete set backs the floor-rounded Claims payout"
        );
        assert_eq!(
            DealerLpPositionV3::decode(&chain_account(&mut context, lp_position).await.data)
                .expect("first LP after partial Remove")
                .equity_shares,
            7
        );
        assert_eq!(
            DealerLpPositionV3::decode(&chain_account(&mut context, second_position).await.data)
                .expect("second LP after first partial Remove")
                .equity_shares,
            7,
            "the partial withdrawal cannot debit the unrelated LP"
        );
        let obligation_after_partial = chain_account(&mut context, campaign.obligation).await.data;
        assert_eq!(
            DealerObligationProjectionV3::decode(&obligation_after_partial)
                .expect("post-partial obligation")
                .total_equity_shares(),
            partial_remove.plan.shares_after
        );

        // Withdraw the first LP's remaining shares, then the second LP's. Each
        // step is rebuilt from the immediately preceding chain poststate. The
        // final state has no share liability or unbacked residue. Complete-set
        // collateral in Hoard is not residue: it exactly backs the Claims
        // distributed across the two independently owned LP Positions.
        let first_collateral_before_final =
            current_equity_collateral(&mut context, &collateral).await;
        let first_claims_before_final = current_equity_claims(&mut context, &fixture).await;
        let first_lp_before_final = chain_account(&mut context, lp_position).await;
        let first_final_remove = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &fixture,
            &first_claims_before_final,
            lp_owner.pubkey(),
            &obligation_after_partial,
            &first_lp_before_final,
            &first_collateral_before_final,
            EquityRequestIntentV3::Redeem { burned_shares: 7 },
            DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
            None,
        )
        .await;
        assert_eq!(first_final_remove.plan.shares_before, 14);
        assert_eq!(first_final_remove.plan.shares_after, 7);
        assert_eq!(first_final_remove.plan.collateral_out, 3);
        assert_eq!(first_final_remove.plan.minimum_complete_sets_to_split, 2);
        assert_eq!(first_final_remove.plan.maximum_complete_sets_to_merge, 0);
        assert_eq!(first_final_remove.plan.collateral_after, 5);
        assert_eq!(first_final_remove.dealer_claims_after, [98, 0, 0]);
        assert_eq!(first_final_remove.lp_claims_after, [5, 3, 3]);
        install_bundle(&mut context, &first_final_remove.admitted);
        let first_withdrawn = submit_lp_hot(
            &mut context,
            first_final_remove.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit first LP final Remove");
        assert!(
            first_withdrawn.result.is_ok(),
            "first LP final Remove: {:?}",
            first_withdrawn.result
        );
        assert_eq!(
            SplTokenAccount::unpack(&chain_account(&mut context, collateral.external).await.data)
                .expect("first external after final Remove")
                .amount,
            99
        );
        assert_eq!(
            DealerLpPositionV3::decode(&chain_account(&mut context, lp_position).await.data)
                .expect("empty first LP")
                .equity_shares,
            0
        );

        let obligation_before_second_final =
            chain_account(&mut context, campaign.obligation).await.data;
        let second_collateral_before_final =
            current_equity_collateral(&mut context, &second_collateral).await;
        let second_claims_before_final = current_equity_claims(&mut context, &second_fixture).await;
        let second_lp_before_final = chain_account(&mut context, second_position).await;
        let second_final_remove = build_equity_bundle(
            &mut context,
            &scenario,
            &campaign,
            &second_fixture,
            &second_claims_before_final,
            second_lp.pubkey(),
            &obligation_before_second_final,
            &second_lp_before_final,
            &second_collateral_before_final,
            EquityRequestIntentV3::Redeem { burned_shares: 7 },
            DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
            None,
        )
        .await;
        assert_eq!(second_final_remove.plan.shares_before, 7);
        assert_eq!(second_final_remove.plan.shares_after, 0);
        assert_eq!(second_final_remove.plan.collateral_out, 3);
        assert_eq!(second_final_remove.plan.minimum_complete_sets_to_split, 2);
        assert_eq!(second_final_remove.plan.maximum_complete_sets_to_merge, 0);
        assert_eq!(second_final_remove.plan.collateral_after, 0);
        assert_eq!(second_final_remove.dealer_claims_after, [100, 0, 0]);
        assert_eq!(second_final_remove.lp_claims_after, [0, 2, 2]);
        install_bundle(&mut context, &second_final_remove.admitted);
        let second_withdrawn = submit_lp_hot(
            &mut context,
            second_final_remove.admitted.bundle.hot_instruction.clone(),
            &[],
        )
        .await
        .expect("submit second LP final Remove");
        assert!(
            second_withdrawn.result.is_ok(),
            "second LP final Remove: {:?}",
            second_withdrawn.result
        );
        assert_eq!(
            SplTokenAccount::unpack(
                &chain_account(&mut context, second_collateral.external)
                    .await
                    .data,
            )
            .expect("second external after final Remove")
            .amount,
            96
        );
        assert_eq!(
            SplTokenAccount::unpack(
                &chain_account(&mut context, second_collateral.principal)
                    .await
                    .data,
            )
            .expect("empty shared principal")
            .amount,
            0
        );
        assert_eq!(
            SplTokenAccount::unpack(
                &chain_account(&mut context, second_collateral.hoard)
                    .await
                    .data,
            )
            .expect("complete-set-backing Hoard")
            .amount,
            5
        );
        assert_eq!(
            DealerLpPositionV3::decode(&chain_account(&mut context, second_position).await.data)
                .expect("empty second LP")
                .equity_shares,
            0
        );
        assert_eq!(
            DealerObligationProjectionV3::decode(
                &chain_account(&mut context, campaign.obligation).await.data,
            )
            .expect("zero-residue obligation")
            .total_equity_shares(),
            0
        );
        assert_eq!(
            chain_account(&mut context, fixture.actor_position.account)
                .await
                .data,
            live.dealer_position
        );
        let terminal_first = current_equity_claims(&mut context, &fixture).await;
        let terminal_second = current_equity_claims(&mut context, &second_fixture).await;
        assert_eq!(terminal_first.dealer_balances, [100, 0, 0]);
        assert_eq!(terminal_first.counterparty_balances, [5, 3, 3]);
        assert_eq!(terminal_second.counterparty_balances, [0, 2, 2]);
        for coordinate in 0..3 {
            assert_eq!(
                terminal_first.counterparty_balances[coordinate]
                    + terminal_second.counterparty_balances[coordinate],
                5,
                "the five Hoard atoms back exactly five LP complete sets at coordinate {coordinate}"
            );
        }
        assert_eq!(
            SplTokenAccount::unpack(&chain_account(&mut context, collateral.external).await.data,)
                .expect("first terminal external")
                .amount
                + SplTokenAccount::unpack(
                    &chain_account(&mut context, second_collateral.external)
                        .await
                        .data,
                )
                .expect("second terminal external")
                .amount
                + 5,
            200,
            "both LP withdrawals plus exact Hoard backing conserve all collateral atoms"
        );
    }

    #[tokio::test]
    async fn accepted_lp_open_close_uses_real_admitted_elf_and_rolls_back_late_refusal() {
        let scenario = super::scenario();
        let campaign = campaign(&scenario);
        let mut test = super::program_test_with_transaction_compute(&scenario);
        test.add_upgradeable_program_to_genesis("dclutch_dealer_accelerator_sbf", &ACCELERATOR);
        test.add_account(
            super::programdata_address(ACCELERATOR),
            super::data_account(
                bpf_loader_upgradeable::ID,
                campaign.accelerator_programdata.account.data.clone(),
            ),
        );
        let mut context = test.start_with_context().await;

        // `program_test_with_transaction_compute` installs the shared
        // trade-only fixture before this private LP campaign exists. Replace
        // that one account with the lifecycle-canonical state the bundle also
        // carries; all immutable Market/Product facts and the address stay
        // byte-for-byte identical.
        context.set_account(
            &scenario.fixture.core_market,
            &AccountSharedData::from(super::data_account(
                scenario.waist.core_program,
                campaign.market_bytes.clone(),
            )),
        );

        let lp_owner = Keypair::new();
        let lp_owner_before = 10_000_000_u64;
        context.set_account(
            &lp_owner.pubkey(),
            &AccountSharedData::from(Account {
                lamports: lp_owner_before,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
        let (credit, credit_state) = lifecycle_credit(&scenario, lp_owner.pubkey());
        assert_eq!(
            credit, campaign.rent_credit,
            "Core and lifecycle select the same canonical RentCredit account",
        );
        assert_eq!(
            CoreState::decode(
                &chain_account(&mut context, scenario.fixture.core_market)
                    .await
                    .data,
            )
            .expect("installed Core Market")
            .rent_beneficiary
            .to_bytes(),
            credit.to_bytes(),
            "the executed Core state owns the exact RentCredit address",
        );
        let credit_account =
            super::data_account(scenario.waist.registry, credit_state.to_bytes().to_vec());
        let credit_before = credit_account.clone();
        context.set_account(&credit, &AccountSharedData::from(credit_account.clone()));

        let open = build_lifecycle_bundle(
            &mut context,
            &scenario,
            &campaign,
            MultiLpRequestActionV3::Open,
            lp_owner.pubkey(),
            &campaign.obligation_bytes,
            None,
            credit,
            credit_account,
        )
        .await;
        let position = open.bundle.logical.get(6).expect("LP state coordinate").key;
        install_bundle(&mut context, &open);
        let open_result = submit_lp_hot(
            &mut context,
            open.bundle.hot_instruction.clone(),
            &[&lp_owner],
        )
        .await
        .expect("submit LP Open");
        assert!(
            open_result.result.is_ok(),
            "LP Open: {:?}",
            open_result.result
        );
        assert!(
            super::invoked_programs(&open_result).contains(&ACCELERATOR),
            "the accepted Open must execute the real Dealer accelerator ELF"
        );

        let opened = chain_account(&mut context, position).await;
        let rent_principal = Rent::default().minimum_balance(DEALER_LP_POSITION_BYTES_V3);
        let (_, bump) = Pubkey::find_program_address(
            &[
                DEALER_LP_POSITION_PDA_DOMAIN_V3,
                campaign.root.as_ref(),
                lp_owner.pubkey().as_ref(),
            ],
            &TRADING,
        );
        let expected_position = DealerLpPositionV3 {
            revision: 1,
            release_set: scenario.waist.release_set_id,
            market: scenario.fixture.core_market.to_bytes(),
            child_root: campaign.root.to_bytes(),
            lp_owner: lp_owner.pubkey().to_bytes(),
            rent_refund: lp_owner.pubkey().to_bytes(),
            obligation_account: campaign.obligation.to_bytes(),
            equity_shares: 0,
            generation: SCENARIO_GENERATION,
            rent_principal,
            pda_bump: u16::from(bump),
        };
        let mut expected_position_bytes = vec![0_u8; DEALER_LP_POSITION_BYTES_V3];
        expected_position
            .encode_into(&mut expected_position_bytes)
            .expect("expected LP encoding");
        assert_eq!(opened.owner, TRADING);
        assert_eq!(opened.lamports, rent_principal);
        assert_eq!(opened.data, expected_position_bytes);
        assert_eq!(
            chain_account(&mut context, lp_owner.pubkey())
                .await
                .lamports,
            lp_owner_before - rent_principal,
            "Open transfers exactly current Rent from the signer"
        );
        assert_eq!(chain_account(&mut context, credit).await, credit_before);
        assert_eq!(
            chain_account(&mut context, campaign.root).await.data,
            campaign.root_bytes,
            "LP lifecycle does not rewrite Dealer semantic state"
        );
        assert_eq!(
            chain_account(&mut context, campaign.obligation).await.data,
            campaign.obligation_bytes,
            "zero-share Open does not rewrite obligations"
        );

        // This obligation is wire-valid, rooted under the same child and used
        // to build the exact request/context, but its semantic Market differs.
        // Common Hot therefore reaches the real accelerator; Dealer semantic
        // authentication refuses there, after lifecycle planning, and the
        // transaction must roll every writable account back byte-for-byte.
        let hostile_obligation = super::obligation_bytes(
            [0xf1; 32],
            scenario.fixture.product_id,
            scenario.fixture.semantic_basis_id,
            scenario.dealer.pubkey().to_bytes(),
            campaign.root.to_bytes(),
            7,
            &[12, 20, 10],
        );
        context.set_account(
            &campaign.obligation,
            &AccountSharedData::from(super::data_account(TRADING, hostile_obligation.clone())),
        );
        let hostile_credit = chain_account(&mut context, credit).await;
        let hostile = build_lifecycle_bundle(
            &mut context,
            &scenario,
            &campaign,
            MultiLpRequestActionV3::Close,
            lp_owner.pubkey(),
            &hostile_obligation,
            Some(opened.clone()),
            credit,
            hostile_credit,
        )
        .await;
        install_bundle(&mut context, &hostile);
        let rollback_keys = [campaign.root, campaign.obligation, position, credit];
        let mut rollback_before = Vec::new();
        for key in rollback_keys {
            rollback_before.push((key, chain_account(&mut context, key).await));
        }
        let hostile_result =
            submit_lp_hot(&mut context, hostile.bundle.hot_instruction.clone(), &[])
                .await
                .expect("submit hostile LP Close");
        assert_eq!(
            super::custom_code(&hostile_result.result),
            Some(TradingSbfError::Transition as u32),
            "a cross-Market obligation must produce the accelerator's refused acknowledgement: {:?}",
            hostile_result.result,
        );
        assert!(
            super::invoked_programs(&hostile_result).contains(&ACCELERATOR),
            "the hostile case must reach the real accelerator, not a shallow gate"
        );
        for (key, before) in rollback_before {
            assert_eq!(
                chain_account(&mut context, key).await,
                before,
                "rollback {key}"
            );
        }

        context.set_account(
            &campaign.obligation,
            &AccountSharedData::from(super::data_account(
                TRADING,
                campaign.obligation_bytes.clone(),
            )),
        );
        let credit_before_close = chain_account(&mut context, credit).await;
        let owner_before_close = chain_account(&mut context, lp_owner.pubkey()).await;
        let close = build_lifecycle_bundle(
            &mut context,
            &scenario,
            &campaign,
            MultiLpRequestActionV3::Close,
            lp_owner.pubkey(),
            &campaign.obligation_bytes,
            Some(opened.clone()),
            credit,
            credit_before_close.clone(),
        )
        .await;
        install_bundle(&mut context, &close);
        let close_result = submit_lp_hot(&mut context, close.bundle.hot_instruction.clone(), &[])
            .await
            .expect("submit LP Close");
        assert!(
            close_result.result.is_ok(),
            "LP Close: {:?}",
            close_result.result
        );
        assert!(super::invoked_programs(&close_result).contains(&ACCELERATOR));
        let closed = context
            .banks_client
            .get_account(position)
            .await
            .expect("read closed LP Position");
        assert!(
            closed.is_none_or(|account| {
                account.lamports == 0
                    && account.data.is_empty()
                    && account.owner == system_program::ID
            }),
            "Close retires the whole LP account"
        );
        let credit_after_close = chain_account(&mut context, credit).await;
        assert_eq!(credit_after_close.data, credit_before_close.data);
        assert_eq!(credit_after_close.owner, credit_before_close.owner);
        assert_eq!(
            credit_after_close.lamports,
            credit_before_close.lamports + opened.lamports,
            "Close returns every lamport, including any dust, to RentCredit"
        );
        assert_eq!(
            chain_account(&mut context, lp_owner.pubkey()).await,
            owner_before_close,
            "Close is permissionless and pays only the immutable RentCredit"
        );
        assert_eq!(
            chain_account(&mut context, campaign.root).await.data,
            campaign.root_bytes
        );
        assert_eq!(
            chain_account(&mut context, campaign.obligation).await.data,
            campaign.obligation_bytes
        );
    }
}
