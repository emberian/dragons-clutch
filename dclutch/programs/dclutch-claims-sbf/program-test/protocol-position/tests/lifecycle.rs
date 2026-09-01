//! Real-ELF admission, rollback, replay-refusal, and rent-close evidence.
//!
//! With `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` set the campaign also emits the
//! finalized transactions the gauntlet's census folds into the execution
//! ledger. See `tools/gauntlet/claims-custody/README.md`.

use dclutch_program_test_evidence::TransactionEvidence;
use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, compile_product_lbv2_fixture_v2,
};
use dclutch_claims_sbf::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2,
    PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionCloseReceiptV2,
    ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    ProtocolPositionSbfErrorV2, ProtocolPositionSeedsV2,
};
use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_CAPABILITY_ROOT_BYTES_V4, FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4,
    FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3, FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3,
    FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3,
    FractionalRetirementActionV3, FractionalRetirementCursorInputV3, FractionalRetirementCursorV3,
    FractionalRetirementLifecycleReceiptV3, FractionalRetirementRequestInputV3,
    FractionalRetirementRequestV3, FractionalRootInputV1, FractionalRootV1,
    NO_RETIREMENT_COORDINATE_V3,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2, FractionalExposureTermsV2,
    encode_fractional_exposure_terms_v2, encode_fractional_selection_config_v1,
    fractional_exposure_terms_bytes_v2, fractional_selection_config_from_terms_v1,
};
use dclutch_fractional_claim_operator::{
    FractionalRetirementCoordinateSnapshotV3, FractionalRetirementDeploymentV3,
    FractionalRetirementRecordV3, FractionalRetirementSnapshotV3,
    plan_fractional_retirement_instruction_v3,
};
use dclutch_market_core_codec::{CoreState, Identity, Phase as CorePhase};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2,
    TokenBehaviorSelectionV2,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::versioned::VersionedTransaction;

const CLAIMS: Pubkey = Pubkey::new_from_array([0xb1; 32]);
const REGISTRY: Pubkey = Pubkey::new_from_array([0xb3; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xb4; 32]);
const TRADING: Pubkey = Pubkey::new_from_array([0xb5; 32]);
const RENT_PROGRAM: Pubkey = Pubkey::new_from_array([0xb6; 32]);
const TOKEN_PROGRAM: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const GENERATION: u64 = 23;

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    trading: Vec<u8>,
    rent: Vec<u8>,
    /// The audited Token-2022 v11 ELF, provenance-checked by the runner.
    ///
    /// Required, not optional: the ordered walk is the campaign's only proof
    /// that a fractional market can actually retire, and a fixture that
    /// silently skipped it when the ELF was missing would be evidence of
    /// nothing at all.
    token: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let read = |name: &str| fs::read(directory.join(name)).expect("real ELF");
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        trading: read("dclutch_claims_liability_basis_test_caller_sbf.so"),
        rent: read("dclutch_rent_sbf.so"),
        token: read("spl_token_2022.so"),
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("programdata state")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("programdata slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("programdata authority tag") = 0;
    bytes
        .get_mut(45..)
        .expect("programdata ELF")
        .copy_from_slice(elf);
    bytes
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>, lamports: u64) {
    test.add_account(
        key,
        Account {
            lamports: lamports
                .max(Rent::default().minimum_balance(data.len()))
                .max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
        1,
    );
}

fn identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact id")
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV1::new(
            release.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            0,
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("observation"),
    )
}

fn activation(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE, 0x51, &artifacts.core);
    let claims = release(CLAIMS, 0x52, &artifacts.claims);
    let trading = release(TRADING, 0x53, &artifacts.trading);
    let rent = release(RENT_PROGRAM, 0x54, &artifacts.rent);
    let set = ExecutionReleaseSetV1::new(
        ExecutionRoleBindingV1::new(core.program(), artifact_id(core)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(trading.program(), artifact_id(trading)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(rent.program(), artifact_id(rent)),
    )
    .expect("release set");
    let id = hash(&set.to_bytes()).to_bytes();
    let content = ContentId::new(id).expect("release id");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, rent),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &set,
            role,
            &activation_input(artifact),
        )
        .expect("activate");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (id, bytes)
}

fn add_record(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone(), 1);
    add_account(test, record.staging, system_program::ID, Vec::new(), 1);
}

fn finalized_record(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> FinalizedRecordFixtureV2 {
    let digest = hash(&bytes).to_bytes();
    FinalizedRecordFixtureV2 {
        owner,
        schema,
        digest,
        raw: Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0,
        staging: Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &owner,
        )
        .0,
        bytes,
    }
}

/// The selection-config identity a real Fractional activation writes.
///
/// The Market selects a market-free config; the terms are the market-bearing
/// record it admits. `authenticate_root` re-projects the terms and compares,
/// so a fixture that plants the terms record digest here is planting a root no
/// activation could have produced.
fn selection_config_digest(terms_bytes: &[u8]) -> [u8; 32] {
    let terms_digest: [u8; 32] = hash(terms_bytes).to_bytes();
    let terms = FractionalExposureTermsV2::decode(
        terms_bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: terms_digest,
            finalized_terms_id: terms_digest,
            recomputed_terms_digest: terms_digest,
            finalized_terms_digest: terms_digest,
            record_authenticated: true,
        },
    )
    .expect("campaign terms decode");
    let mut config = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut config,
    )
    .expect("campaign selection config");
    hash(&config).to_bytes()
}

fn retirement_mint_bytes(controller: Pubkey) -> Vec<u8> {
    const TLV_START: usize = 166;
    let mut bytes = vec![0_u8; TLV_START];
    bytes
        .get_mut(0..4)
        .expect("Mint authority tag")
        .copy_from_slice(&1_u32.to_le_bytes());
    bytes
        .get_mut(4..36)
        .expect("Mint authority")
        .copy_from_slice(controller.as_ref());
    *bytes.get_mut(45).expect("Mint initialized") = 1;
    *bytes.get_mut(165).expect("Mint account type") = 1;
    for extension in [3_u16, 28_u16] {
        bytes.extend_from_slice(&extension.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(controller.as_ref());
    }
    bytes
}

struct Fixture {
    release: [u8; 32],
    cache: Pubkey,
    core_market: Pubkey,
    market: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    owner: Pubkey,
    wrong_owner: Pubkey,
    rent_credit: Pubkey,
    position_lamports: u64,
    admission_lamports: u64,
    retirement_authority: Pubkey,
    root: Pubkey,
    cursor: Pubkey,
    terms: FinalizedRecordFixtureV2,
    token_behavior: FinalizedRecordFixtureV2,
    mint: Pubkey,
    begin: FractionalRetirementRequestV3,
    retirement: FractionalRetirementRequestV3,
    finish: FractionalRetirementRequestV3,
    cursor_rent: u64,
    graph: dclutch_claims_affine_batch_program_test::fixture::ProductLbv2FixtureV2,
}

/// Which of the two campaigns a fixture is being stood up for.
///
/// They differ in exactly three planted facts, and each one is load-bearing
/// for one campaign and fatal to the other.
#[derive(Clone, Copy, Eq, PartialEq)]
enum CampaignV1 {
    /// A stub at the canonical Token address that refuses every instruction,
    /// so the late Mint-close CPI is observable as a rollback boundary. The
    /// cursor is planted, because this campaign never runs `Begin`.
    LateTokenRefusal,
    /// The audited Token-2022 v11 ELF and a vacant cursor address ALREADY
    /// OVER-FUNDED past its own rent minimum -- the two things an ordered walk
    /// needs in order to be driven end to end by the real routes rather than
    /// around them, plus the griefing case: a stranger's donation must make
    /// begin's bill smaller rather than making begin impossible, and the stray
    /// lamports must come back out again at finish instead of being burned.
    OrderedWalk,
    /// The same, with the cursor address funded BELOW its rent minimum, so
    /// begin has to top it up out of the payer rather than finding it ready.
    OrderedWalkUnderfundedCursor,
}

impl CampaignV1 {
    const fn walks(self) -> bool {
        matches!(self, Self::OrderedWalk | Self::OrderedWalkUnderfundedCursor)
    }
}

/// Lamports a stranger left on the cursor address before anyone began.
const CURSOR_STRAY_LAMPORTS: u64 = 4_242;

/// Resolve the Market between transactions, the way Resolution would.
///
/// The order matters and is not a convenience. `Admit` requires Core to be
/// exactly `Open` and `Begin` requires it to be Terminal-or-Retiring, so a
/// campaign that drives BOTH with the real routes has to move the Market
/// between them -- which is what actually happens on chain, one resolution
/// apart. Everything but the phase and the terminal receipt is left exactly as
/// the shared Product/LBV2 fixture compiled it.
async fn resolve_market(context: &mut ProgramTestContext, core_market: Pubkey) {
    let account = context
        .banks_client
        .get_account(core_market)
        .await
        .expect("Core market")
        .expect("Core market exists");
    let mut state = CoreState::decode(&account.data).expect("Core state");
    assert_eq!(
        state.phase,
        CorePhase::Open,
        "the walk resolves an Open Market"
    );
    state.phase = CorePhase::Terminal;
    state.terminal_winner = 0;
    state.terminal_receipt = Some(Identity::new([0x7c; 32]).expect("terminal receipt"));
    let mut resolved = account.clone();
    resolved.data = state.encode().expect("terminal Core state").to_vec();
    context.set_account(&core_market, &AccountSharedData::from(resolved));
}

fn fixture() -> (ProgramTest, Fixture) {
    fixture_for(CampaignV1::LateTokenRefusal)
}

fn fixture_for(campaign: CampaignV1) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, id, elf) in [
        ("dclutch_claims_sbf", CLAIMS, artifacts.claims.as_slice()),
        (
            "dclutch_registry_sbf",
            REGISTRY,
            artifacts.registry.as_slice(),
        ),
        ("dclutch_core_sbf", CORE, artifacts.core.as_slice()),
        (
            "dclutch_claims_liability_basis_test_caller_sbf",
            TRADING,
            artifacts.trading.as_slice(),
        ),
        ("dclutch_rent_sbf", RENT_PROGRAM, artifacts.rent.as_slice()),
    ] {
        add_program(&mut test, name, id, elf);
    }
    if campaign.walks() {
        add_program(
            &mut test,
            "spl_token_2022",
            TOKEN_PROGRAM,
            artifacts.token.as_slice(),
        );
    } else {
        add_program(
            &mut test,
            "dclutch_claims_liability_basis_test_caller_sbf",
            TOKEN_PROGRAM,
            artifacts.trading.as_slice(),
        );
    }
    let (release, cache_bytes) = activation(&artifacts);
    let cache = Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release], &REGISTRY).0;
    add_account(&mut test, cache, REGISTRY, cache_bytes, 1);
    let wrong_owner = Pubkey::new_from_array([0xd2; 32]);
    let graph = compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY,
        core_program: CORE,
        claims_program: CLAIMS,
        release_set: release,
        realm_id: [0x61; 32],
        custody_context: [0x62; 32],
        generation: GENERATION,
        source_owner: Pubkey::new_from_array([0xa1; 32]),
        destination_owner: Pubkey::new_from_array([0xa2; 32]),
    })
    .expect("Product/LBV2 fixture");
    for record in [
        &graph.product,
        &graph.result_domain,
        &graph.portfolio,
        &graph.linked_basis,
    ] {
        add_record(&mut test, record);
    }
    add_account(
        &mut test,
        graph.core_market,
        CORE,
        graph.core_state.clone(),
        1,
    );
    add_account(
        &mut test,
        graph.claims_market,
        CLAIMS,
        graph.claims_market_bytes.clone(),
        1,
    );
    let refund = RefundAuthority::new([0x71; 32]).expect("refund authority");
    let (rent_credit, bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            graph.core_market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM,
    );
    let rent_credit_data = LifecycleRentCreditV2::new(
        refund,
        LifecycleAccountIdV2::new(graph.core_market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release).expect("release set"),
        GENERATION,
        bump,
    )
    .expect("lifecycle RentCredit")
    .to_bytes()
    .to_vec();
    add_account(&mut test, rent_credit, RENT_PROGRAM, rent_credit_data, 1);

    let token_behavior_bytes = TokenBehaviorSelectionV2::new([0x61; 32], release)
        .expect("Token behavior")
        .to_bytes()
        .to_vec();
    let token_behavior = finalized_record(
        REGISTRY,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        token_behavior_bytes,
    );
    add_record(&mut test, &token_behavior);
    let mint = Pubkey::new_from_array([0xe1; 32]);
    let mut terms_scratch = vec![0_u8; fractional_exposure_terms_bytes_v2(1).expect("terms width")];
    let mut terms_bytes = terms_scratch.clone();
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: graph.core_market.to_bytes(),
            product_record: graph.product.digest,
            result_domain: graph.result_domain.digest,
            release_set: release,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: token_behavior.digest,
            exposure_id: [0x63; 32],
            product_basis: graph.linked_basis.digest,
            representation_basis: [0x64; 32],
            graph_id: [0x65; 32],
            product_width: 258,
            denominator: 10,
            shard_mints: &[mint.to_bytes()],
        },
        &mut terms_scratch,
        &mut terms_bytes,
    )
    .expect("Fractional terms");
    let terms = finalized_record(
        REGISTRY,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        terms_bytes,
    );
    add_record(&mut test, &terms);
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([0x66; 32]).expect("manifest"),
        ContentId::new([0x67; 32]).expect("kind"),
        ContentId::new([0x68; 32]).expect("capability release"),
        // The digest of the market-free selection config the terms project
        // to, which is what a real activation writes -- NOT the terms record
        // digest. Planting the record digest here made `authenticate_root`'s
        // config split refuse at `0x500B`, which had left this campaign's only
        // retirement test red since that gate landed (`4630ad77` updated the
        // fractional-atomic fixture and not this one).
        ContentId::new(selection_config_digest(&terms.bytes)).expect("terms"),
    )
    .expect("selection");
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(release).expect("release"),
        graph.core_market.to_bytes(),
        1,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("root header");
    let (root, root_bump) =
        Pubkey::find_program_address(&root_header.seeds().as_slices(), &TRADING);
    let root_lamports = Rent::default().minimum_balance(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4);
    let root_state = FractionalRootV1::new(FractionalRootInputV1 {
        bump: root_bump,
        terms: terms.digest,
        market: graph.core_market.to_bytes(),
        rent_beneficiary: rent_credit.to_bytes(),
        // The root revision a real `Begin` would have consumed, which is the
        // one `begin` is called with below. This used to be 5 -- one ahead of
        // the value the cursor was begun from -- because the handler compared
        // the frozen root revision to the CURSOR's current one, and coordinate
        // 0 is the only step where those can both be satisfied. A width-2 walk
        // was unreachable and nothing said so. The cursor now derives the root
        // anchor instead, and this fixture states the root a real begin leaves.
        revision: 4,
        historical_rent_principal: root_lamports,
    })
    .expect("root state");
    let mut root_bytes = vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_BYTES_V4];
    root_bytes
        .get_mut(..FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4)
        .expect("root header bytes")
        .copy_from_slice(&root_header.to_bytes());
    root_bytes
        .get_mut(FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4..)
        .expect("root state bytes")
        .copy_from_slice(&root_state.to_bytes());
    add_account(&mut test, root, TRADING, root_bytes, root_lamports);
    let owner = root;
    add_account(&mut test, wrong_owner, system_program::ID, Vec::new(), 1);
    let position_seeds =
        ProtocolPositionSeedsV2::new(graph.claims_market.to_bytes(), owner.to_bytes())
            .expect("position seeds");
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS).0;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(graph.claims_market.to_bytes(), owner.to_bytes())
            .expect("admission seeds");
    let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &CLAIMS).0;
    let position_lamports = Rent::default().minimum_balance(128 + 8 * 258) + 17;
    let admission_lamports =
        Rent::default().minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2) + 19;
    add_account(
        &mut test,
        position,
        system_program::ID,
        Vec::new(),
        position_lamports,
    );
    add_account(
        &mut test,
        admission,
        system_program::ID,
        Vec::new(),
        admission_lamports,
    );

    let terms_view = FractionalExposureTermsV2::decode(
        &terms.bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: terms.digest,
            finalized_terms_id: terms.digest,
            recomputed_terms_digest: terms.digest,
            finalized_terms_digest: terms.digest,
            record_authenticated: true,
        },
    )
    .expect("terms view");
    let cursor_rent = Rent::default().minimum_balance(FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3);
    let (cursor, cursor_bump) = Pubkey::find_program_address(
        &[FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, root.as_ref()],
        &CLAIMS,
    );
    let retirement_input = |revision, coordinate| FractionalRetirementRequestInputV3 {
        release_set: release,
        market: graph.core_market.to_bytes(),
        terms: terms.digest,
        token_program: TOKEN_2022_PROGRAM_ID,
        token_behavior: token_behavior.digest,
        exposure: [0x63; 32],
        root: root.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        expected_revision: revision,
        representation_coordinate: coordinate,
    };
    let begin = FractionalRetirementRequestV3::new(
        FractionalRetirementActionV3::Begin,
        retirement_input(4, NO_RETIREMENT_COORDINATE_V3),
    )
    .expect("begin");
    let cursor_state = FractionalRetirementCursorV3::begin(
        terms_view,
        begin,
        FractionalRetirementCursorInputV3 {
            bump: cursor_bump,
            pre_revision: 4,
            historical_rent_principal: cursor_rent,
        },
    )
    .expect("cursor");
    match campaign {
        // The walk creates this account with the real `Begin`, so the address
        // must be VACANT -- system-owned and empty. What it holds is the
        // variable, and both values are hostile in their own way.
        CampaignV1::OrderedWalk => add_account(
            &mut test,
            cursor,
            system_program::ID,
            Vec::new(),
            cursor_rent
                .checked_add(CURSOR_STRAY_LAMPORTS)
                .expect("over-funded cursor"),
        ),
        // `add_account` floors an empty account at the rent minimum for zero
        // bytes, which is far below what 296 bytes costs, so this really is
        // underfunded and begin really does have to transfer.
        CampaignV1::OrderedWalkUnderfundedCursor => {
            add_account(&mut test, cursor, system_program::ID, Vec::new(), 1)
        }
        CampaignV1::LateTokenRefusal => add_account(
            &mut test,
            cursor,
            CLAIMS,
            cursor_state.to_bytes().expect("cursor bytes").to_vec(),
            cursor_rent,
        ),
    }
    let retirement = FractionalRetirementRequestV3::new(
        FractionalRetirementActionV3::RetireCoordinate,
        retirement_input(5, 0),
    )
    .expect("retirement");
    // Each act consumes exactly one cursor revision: begin at the root's own
    // 4, the single coordinate at 5, finish at 6. The root stays at 4 the
    // whole time, which is the relation `root_revision_anchor` states.
    let finish = FractionalRetirementRequestV3::new(
        FractionalRetirementActionV3::Finish,
        retirement_input(6, NO_RETIREMENT_COORDINATE_V3),
    )
    .expect("finish");
    let retirement_bytes = retirement.to_bytes().expect("retirement bytes");
    let retirement_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release).expect("release"),
        graph.core_market.to_bytes(),
        ExecutionRoleV1::Trading,
        terms.digest,
        hash(&retirement_bytes).to_bytes(),
    )
    .expect("retirement authority");
    let retirement_authority =
        Pubkey::find_program_address(&retirement_seeds.as_slices(), &TRADING).0;
    add_account(
        &mut test,
        retirement_authority,
        system_program::ID,
        Vec::new(),
        1,
    );
    let mint_bytes = retirement_mint_bytes(root);
    Token2022BehaviorProfileV2::check_mint(
        TOKEN_2022_PROGRAM_ID,
        mint.to_bytes(),
        &mint_bytes,
        root.to_bytes(),
        0,
    )
    .expect("retirement Mint profile");
    add_account(&mut test, mint, TOKEN_PROGRAM, mint_bytes, 1);
    (
        test,
        Fixture {
            release,
            cache,
            core_market: graph.core_market,
            market: graph.claims_market,
            position,
            admission,
            owner,
            wrong_owner,
            rent_credit,
            position_lamports,
            admission_lamports,
            retirement_authority,
            root,
            cursor,
            terms,
            token_behavior,
            mint,
            begin,
            retirement,
            finish,
            cursor_rent,
            graph,
        },
    )
}

async fn observed_account(
    context: &mut ProgramTestContext,
    key: Pubkey,
    observation: Observation,
) -> ObservedAccount {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("planner account read")
        .unwrap_or_else(|| {
            assert_eq!(key, system_program::ID, "required planner account {key}");
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: solana_sdk_ids::native_loader::ID,
                executable: true,
                rent_epoch: 0,
            }
        });
    ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn retirement_deployment(
    context: &mut ProgramTestContext,
    program: Pubkey,
    observation: Observation,
) -> FractionalRetirementDeploymentV3 {
    FractionalRetirementDeploymentV3 {
        program: observed_account(context, program, observation).await,
        programdata: observed_account(context, programdata(program), observation).await,
    }
}

/// Reacquire the complete semantic graph from the live ProgramTest bank.
///
/// The fixture supplies only addresses. Every byte, owner, balance, deployment
/// and cursor decision consumed by the public planner is read back after the
/// preceding real transaction, so this is the same entrance an RPC adapter
/// uses rather than a parallel request builder.
async fn retirement_snapshot(
    context: &mut ProgramTestContext,
    f: &Fixture,
    payer: Pubkey,
    include_coordinate: bool,
) -> FractionalRetirementSnapshotV3 {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("planner clock");
    let observation = Observation {
        slot: clock.slot,
        unix_timestamp: clock.unix_timestamp,
        finality: Finality::Finalized,
    };
    let coordinate = if include_coordinate {
        Some(FractionalRetirementCoordinateSnapshotV3 {
            position: observed_account(context, f.position, observation).await,
            admission: observed_account(context, f.admission, observation).await,
            shard_mint: observed_account(context, f.mint, observation).await,
        })
    } else {
        None
    };
    FractionalRetirementSnapshotV3 {
        payer,
        core_market: observed_account(context, f.core_market, observation).await,
        claims_market: observed_account(context, f.market, observation).await,
        activation_cache: observed_account(context, f.cache, observation).await,
        registry_program: observed_account(context, REGISTRY, observation).await,
        core: retirement_deployment(context, CORE, observation).await,
        claims: retirement_deployment(context, CLAIMS, observation).await,
        trading: retirement_deployment(context, TRADING, observation).await,
        rent: retirement_deployment(context, RENT_PROGRAM, observation).await,
        root: observed_account(context, f.root, observation).await,
        rent_credit: observed_account(context, f.rent_credit, observation).await,
        cursor: observed_account(context, f.cursor, observation).await,
        terms: FractionalRetirementRecordV3 {
            raw: observed_account(context, f.terms.raw, observation).await,
            staging: observed_account(context, f.terms.staging, observation).await,
        },
        token_behavior: FractionalRetirementRecordV3 {
            raw: observed_account(context, f.token_behavior.raw, observation).await,
            staging: observed_account(context, f.token_behavior.staging, observation).await,
        },
        rent_sysvar: observed_account(context, sysvar::rent::ID, observation).await,
        system_program: observed_account(context, system_program::ID, observation).await,
        token_program: observed_account(context, TOKEN_PROGRAM, observation).await,
        coordinate,
    }
}

fn request(f: &Fixture, action: ProtocolPositionActionV2) -> ProtocolPositionRequestV2 {
    ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: f.release,
        market: f.core_market.to_bytes(),
        position_owner: f.owner.to_bytes(),
        parent_request_digest: if action == ProtocolPositionActionV2::Admit {
            [0x81; 32]
        } else {
            [0x82; 32]
        },
        rent_credit: f.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM.to_bytes(),
        generation: GENERATION,
        expected_market_revision: 0,
        expected_position_revision: 0,
        observed_position_lamports: f.position_lamports,
        observed_admission_lamports: f.admission_lamports,
        position_rent_principal: Rent::default().minimum_balance(128 + 8 * 258),
        admission_rent_principal: Rent::default()
            .minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
}

fn wrapped(
    f: &Fixture,
    request: ProtocolPositionRequestV2,
    fail_after: bool,
    owner: Pubkey,
) -> Instruction {
    let bytes = request.to_bytes().expect("request");
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.position_owner,
        hash(&bytes).to_bytes(),
    )
    .expect("authority seeds");
    let authority = Pubkey::find_program_address(&seeds.as_slices(), &TRADING).0;
    let forwarded = match request.action {
        ProtocolPositionActionV2::Admit => vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(f.market, false),
            AccountMeta::new(f.position, false),
            AccountMeta::new(f.admission, false),
            AccountMeta::new_readonly(f.graph.linked_basis.raw, false),
            AccountMeta::new_readonly(f.graph.linked_basis.staging, false),
            AccountMeta::new_readonly(f.graph.product.raw, false),
            AccountMeta::new_readonly(f.graph.product.staging, false),
            AccountMeta::new_readonly(f.graph.result_domain.raw, false),
            AccountMeta::new_readonly(f.graph.result_domain.staging, false),
            AccountMeta::new_readonly(f.graph.portfolio.raw, false),
            AccountMeta::new_readonly(f.graph.portfolio.staging, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(f.core_market, false),
            AccountMeta::new_readonly(f.cache, false),
            AccountMeta::new_readonly(REGISTRY, false),
            AccountMeta::new_readonly(TRADING, false),
            AccountMeta::new_readonly(programdata(TRADING), false),
            AccountMeta::new_readonly(CLAIMS, false),
            AccountMeta::new_readonly(programdata(CLAIMS), false),
            AccountMeta::new_readonly(CORE, false),
            AccountMeta::new_readonly(programdata(CORE), false),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new_readonly(f.rent_credit, false),
            AccountMeta::new_readonly(RENT_PROGRAM, false),
        ],
        ProtocolPositionActionV2::Close => vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(f.market, false),
            AccountMeta::new(f.position, false),
            AccountMeta::new(f.admission, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(f.cache, false),
            AccountMeta::new_readonly(REGISTRY, false),
            AccountMeta::new_readonly(TRADING, false),
            AccountMeta::new_readonly(programdata(TRADING), false),
            AccountMeta::new_readonly(CLAIMS, false),
            AccountMeta::new_readonly(programdata(CLAIMS), false),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(f.rent_credit, false),
            AccountMeta::new_readonly(RENT_PROGRAM, false),
        ],
    };
    assert_eq!(
        forwarded.len(),
        if request.action == ProtocolPositionActionV2::Admit {
            PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2
        } else {
            PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2
        }
    );
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS, false)];
    accounts.extend(forwarded);
    let mut data = vec![u8::from(fail_after)];
    data.extend_from_slice(&bytes);
    Instruction {
        program_id: TRADING,
        accounts,
        data,
    }
}

/// One direct Claims `Begin`. No wrapper and no caller authority: begin is
/// permissionless, because everything it writes is determined by the terms and
/// the root it just authenticated.
fn begin_instruction(f: &Fixture, payer: Pubkey) -> Instruction {
    let accounts = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(f.market, false),
        AccountMeta::new_readonly(f.core_market, false),
        AccountMeta::new_readonly(CORE, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(f.root, false),
        AccountMeta::new_readonly(f.rent_credit, false),
        AccountMeta::new(f.cursor, false),
        AccountMeta::new_readonly(f.terms.raw, false),
        AccountMeta::new_readonly(f.terms.staging, false),
        AccountMeta::new_readonly(f.token_behavior.raw, false),
        AccountMeta::new_readonly(f.token_behavior.staging, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    assert_eq!(accounts.len(), FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3);
    Instruction {
        program_id: CLAIMS,
        accounts,
        data: f.begin.to_bytes().expect("begin bytes").to_vec(),
    }
}

/// One direct Claims `Finish`, likewise unprivileged.
fn finish_instruction(f: &Fixture) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(f.market, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(f.root, false),
        AccountMeta::new(f.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM, false),
        AccountMeta::new(f.cursor, false),
        AccountMeta::new_readonly(f.terms.raw, false),
        AccountMeta::new_readonly(f.terms.staging, false),
        AccountMeta::new_readonly(f.token_behavior.raw, false),
        AccountMeta::new_readonly(f.token_behavior.staging, false),
    ];
    assert_eq!(
        accounts.len(),
        FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3
    );
    Instruction {
        program_id: CLAIMS,
        accounts,
        data: f.finish.to_bytes().expect("finish bytes").to_vec(),
    }
}

fn retirement_wrapped(f: &Fixture) -> Instruction {
    let forwarded = vec![
        AccountMeta::new_readonly(f.retirement_authority, false),
        AccountMeta::new_readonly(f.market, false),
        AccountMeta::new(f.position, false),
        AccountMeta::new(f.admission, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(f.cache, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new(f.root, false),
        AccountMeta::new(f.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM, false),
        AccountMeta::new(f.cursor, false),
        AccountMeta::new_readonly(f.terms.raw, false),
        AccountMeta::new_readonly(f.terms.staging, false),
        AccountMeta::new_readonly(f.token_behavior.raw, false),
        AccountMeta::new_readonly(f.token_behavior.staging, false),
        AccountMeta::new(f.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM, false),
    ];
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS, false)];
    accounts.extend(forwarded);
    let mut data = vec![0_u8];
    data.extend_from_slice(&f.retirement.to_bytes().expect("retirement bytes"));
    Instruction {
        program_id: TRADING,
        accounts,
        data,
    }
}

/// Solana's legacy packet maximum. ProgramTest submits no packet and therefore
/// cannot enforce it, so this campaign MEASURES every transaction against it:
/// Found31 was a frame ten bytes past this limit and it survived every fixture
/// test in the tree.
const PACKET_DATA_BYTES: usize = 1_232;

/// The exact wire extent of one signed transaction.
///
/// One shortvec byte for the signature count, 64 bytes per signature, then the
/// serialised message. This is what a validator would receive.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    let extent = 1 + signatures * 64 + message.len();
    assert!(
        extent <= PACKET_DATA_BYTES,
        "the transaction serialises to {extent} bytes, past Solana's {PACKET_DATA_BYTES}-byte packet maximum"
    );
    extent
}

async fn process_legacy(context: &mut ProgramTestContext, instruction: Instruction, label: &str) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("legacy blockhash");
    let transaction = solana_transaction::Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed ALT transaction")
        .to_string();
    let wire_bytes = wire_extent(transaction.signatures.len(), &transaction.message_data());
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT lifecycle processing");
    let accepted = processed.result.is_ok();
    // The refusal is rendered from what the RUNTIME returned, never from what
    // the campaign expected.
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    assert!(accepted, "ALT lifecycle must commit");
}

fn lookup_addresses(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer && !addresses.contains(&instruction.program_id) {
            addresses.push(instruction.program_id);
        }
        for meta in &instruction.accounts {
            if meta.pubkey != payer && !addresses.contains(&meta.pubkey) {
                addresses.push(meta.pubkey);
            }
        }
    }
    addresses
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
) -> Pubkey {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    process_legacy(context, create, "claims position: create lookup table").await;
    for (index, chunk) in addresses.chunks(20).enumerate() {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            &format!("claims position: extend lookup table {index}"),
        )
        .await;
    }
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup addresses");
    table
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<(bool, Vec<String>, Option<(Pubkey, Vec<u8>)>, u64), BanksClientError> {
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
    let transaction =
        VersionedTransaction::try_new(message, &[&context.payer]).expect("transaction");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let accepted = processed.result.is_ok();
    // The refusal is rendered from what the RUNTIME returned, never from what
    // the campaign expected.
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (logs, returned, compute_units) = processed
        .metadata
        .map(|metadata| {
            (
                metadata.log_messages,
                metadata
                    .return_data
                    .map(|value| (value.program_id, value.data)),
                metadata.compute_units_consumed,
            )
        })
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Ok((accepted, logs, returned, compute_units))
}

#[tokio::test]
async fn real_sbf_admit_rolls_back_and_zero_close_reclaims_both_accounts() {
    let (test, f) = fixture();
    let mut context = test.start_with_context().await;
    let before_position = context
        .banks_client
        .get_account(f.position)
        .await
        .expect("read")
        .expect("position");
    let before_admission = context
        .banks_client
        .get_account(f.admission)
        .await
        .expect("read")
        .expect("admission");

    let hostile = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        false,
        f.wrong_owner,
    );
    let late = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        true,
        f.owner,
    );
    let admit = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        false,
        f.owner,
    );
    let replay = admit.clone();
    let close = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Close),
        false,
        f.owner,
    );
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        &[hostile.clone(), late.clone(), admit.clone(), close.clone()],
    );
    let table = create_live_lookup_table(&mut context, &addresses).await;

    let (accepted, _, _, _) = submit(
        &mut context,
        hostile,
        table,
        &addresses,
        "claims position: admit under a substituted Position owner",
    )
    .await
    .expect("hostile");
    assert!(!accepted);
    assert_eq!(
        context
            .banks_client
            .get_account(f.position)
            .await
            .expect("read")
            .expect("position"),
        before_position
    );

    let (accepted, logs, _, _) = submit(
        &mut context,
        late,
        table,
        &addresses,
        "claims position: caller refuses after a complete admission",
    )
    .await
    .expect("late");
    assert!(!accepted);
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {CLAIMS} success"))
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.admission)
            .await
            .expect("read")
            .expect("admission"),
        before_admission
    );

    let (accepted, _, returned, admit_compute_units) = submit(
        &mut context,
        admit,
        table,
        &addresses,
        "claims position: admit",
    )
    .await
    .expect("admit");
    assert!(accepted);
    assert!(admit_compute_units <= 1_400_000);
    let (producer, bytes) = returned.expect("admit receipt");
    assert_eq!(producer, CLAIMS);
    let admission = ProtocolPositionAdmissionV2::decode_receipt(&bytes).expect("receipt");
    assert_eq!(admission.outcome_count(), 258);
    let position = context
        .banks_client
        .get_account(f.position)
        .await
        .expect("read")
        .expect("position");
    assert_eq!(position.owner, CLAIMS);
    assert!(
        position
            .data
            .get(128..)
            .expect("position vector")
            .iter()
            .all(|byte| *byte == 0)
    );

    let (accepted, _, _, _) = submit(
        &mut context,
        replay,
        table,
        &addresses,
        "claims position: admit an already admitted Position",
    )
    .await
    .expect("replay");
    assert!(!accepted);

    let rent_before = context
        .banks_client
        .get_account(f.rent_credit)
        .await
        .expect("read")
        .expect("rent")
        .lamports;
    let (accepted, _, returned, close_compute_units) = submit(
        &mut context,
        close,
        table,
        &addresses,
        "claims position: close a zero Position",
    )
    .await
    .expect("close");
    assert!(accepted);
    assert!(close_compute_units <= 1_400_000);
    let (_, bytes) = returned.expect("close receipt");
    ProtocolPositionCloseReceiptV2::decode(&bytes).expect("close receipt");
    let closed_position = context
        .banks_client
        .get_account(f.position)
        .await
        .expect("read");
    let closed_admission = context
        .banks_client
        .get_account(f.admission)
        .await
        .expect("read");
    assert!(closed_position.is_none() && closed_admission.is_none());
    let rent_after = context
        .banks_client
        .get_account(f.rent_credit)
        .await
        .expect("read")
        .expect("rent")
        .lamports;
    assert_eq!(
        rent_after,
        rent_before + f.position_lamports + f.admission_lamports
    );
    println!(
        "runtime-width LBV2 protocol Position CU: admit={admit_compute_units}, close={close_compute_units}"
    );
}

#[tokio::test]
async fn real_sbf_late_token_refusal_rolls_back_fractional_position_mint_and_cursor() {
    let (test, f) = fixture();
    let mut context = test.start_with_context().await;
    let admit = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        false,
        f.owner,
    );
    let retirement = retirement_wrapped(&f);
    let addresses = lookup_addresses(context.payer.pubkey(), &[admit.clone(), retirement.clone()]);
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, _, _, _) = submit(
        &mut context,
        admit,
        table,
        &addresses,
        "claims fractional retirement: admit zero root Position",
    )
    .await
    .expect("admit");
    assert!(accepted);

    let before_position = context
        .banks_client
        .get_account(f.position)
        .await
        .expect("position");
    let before_admission = context
        .banks_client
        .get_account(f.admission)
        .await
        .expect("admission");
    let before_rent = context
        .banks_client
        .get_account(f.rent_credit)
        .await
        .expect("RentCredit");
    let before_cursor = context
        .banks_client
        .get_account(f.cursor)
        .await
        .expect("cursor");
    let before_mint = context
        .banks_client
        .get_account(f.mint)
        .await
        .expect("Mint");
    let (accepted, logs, _, _) = submit(
        &mut context,
        retirement,
        table,
        &addresses,
        "claims fractional retirement: canonical Token program refuses at late Mint close CPI",
    )
    .await
    .expect("retirement refusal");
    assert!(!accepted, "the test Token processor must refuse Mint close");
    assert!(
        logs.iter()
            .any(|line| line.contains(&format!("Program {TOKEN_PROGRAM} invoke"))),
        "the refusal must occur after Claims reaches the canonical-ID test Token SBF: {logs:#?}"
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.position)
            .await
            .expect("position"),
        before_position
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.admission)
            .await
            .expect("admission"),
        before_admission
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.rent_credit)
            .await
            .expect("RentCredit"),
        before_rent
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor"),
        before_cursor
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.mint)
            .await
            .expect("Mint"),
        before_mint
    );
}

/// Assert a refusal carried an exact program error code.
///
/// A hostile that only asserts "the transaction failed" passes on any failure,
/// including one raised by a gate long before the check the hostile claims to
/// be about. The code is what makes the claim checkable.
fn refused_with(logs: &[String], code: u32) -> bool {
    let needle = format!("custom program error: {code:#x}");
    logs.iter().any(|line| line.contains(&needle))
}

/// Refusal code for `ProtocolPositionSbfErrorV2::Position`.
///
/// Written literally so a code read out of a validator log is greppable to
/// here; the program's own `const _: () = assert!` band pins the enum.
const POSITION_REFUSAL: u32 = 0x5145;

/// A STRANGER'S ONE LAMPORT CANNOT BLOCK ADMISSION OR CLOSE, AND UNDERFUNDING STILL REFUSES.
///
/// Census row R13. The Position and admission accounts are keyless, off-curve,
/// system-owned PDAs, so anyone on the network may send them lamports at any
/// time and nothing can stop them. Admission compared their LIVE balance to a
/// balance the caller had *declared* one slot earlier, by exact equality, so a
/// single lamport arriving in between refused the transaction -- repeatable by
/// anyone, against any pending admission, for about one lamport plus a fee,
/// every slot, forever. That is not a delay a retry outlasts: retrying costs
/// strictly more than attacking.
///
/// This test executes the attack with a real transfer from a wallet that is
/// nobody, then admits and closes the position using requests whose bytes were
/// fixed before the attack. It pins both safety halves: an admit may not
/// over-declare its prepaid balance, and a close may not substitute the
/// immutable balance baseline recorded by admission. The close receipt and
/// RentCredit use the authenticated LIVE balances, so the donation is neither
/// a liveness veto nor a burned/unclassified lamport.
#[tokio::test]
async fn a_strangers_lamport_cannot_block_admission_and_underfunding_still_refuses() {
    let (test, f) = fixture();
    let mut context = test.start_with_context().await;

    // The admit whose bytes never change: it declares the balance the caller
    // read BEFORE the attack, which is the only balance a caller can ever know.
    let admit = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        false,
        f.owner,
    );
    // The negative control. Claiming the accounts hold more than they do is
    // underfunding, and underfunding is exactly what this check exists to
    // refuse. It must still refuse after the relaxation, or the relaxation
    // removed a guard instead of widening one.
    let mut overdeclared = request(&f, ProtocolPositionActionV2::Admit);
    overdeclared.observed_position_lamports = f
        .position_lamports
        .checked_add(1_000)
        .expect("over-declaration");
    let underfunded = wrapped(&f, overdeclared, false, f.owner);
    // The close is authored before the attack and carries the immutable
    // admission baseline. A hostile close that substitutes the donated live
    // balance must refuse against the persisted admission instead.
    let baseline_close_request = request(&f, ProtocolPositionActionV2::Close);
    let baseline_close = wrapped(&f, baseline_close_request, false, f.owner);
    let mut substituted = request(&f, ProtocolPositionActionV2::Close);
    substituted.observed_position_lamports = f
        .position_lamports
        .checked_add(1)
        .expect("substituted balance");
    let substituted_close = wrapped(&f, substituted, false, f.owner);

    let addresses = lookup_addresses(
        context.payer.pubkey(),
        &[
            underfunded.clone(),
            admit.clone(),
            baseline_close.clone(),
            substituted_close.clone(),
        ],
    );
    let table = create_live_lookup_table(&mut context, &addresses).await;

    // THE ATTACK. A wallet that holds no role, is named nowhere in this market
    // and signs nothing but its own transfer, sends one lamport to the Position
    // PDA. It needs no permission because the PDA is system-owned and has no
    // key: this is the cheapest griefing verb in the tree.
    let griefer = Keypair::new();
    let griefer_stake = Rent::default()
        .minimum_balance(0)
        .checked_mul(64)
        .expect("griefer funding");
    let funder = context.payer.pubkey();
    process_legacy(
        &mut context,
        transfer(&funder, &griefer.pubkey(), griefer_stake),
        "claims position: fund a stranger",
    )
    .await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    context
        .banks_client
        .process_transaction(solana_transaction::Transaction::new_signed_with_payer(
            &[transfer(&griefer.pubkey(), &f.position, 1)],
            Some(&griefer.pubkey()),
            &[&griefer],
            blockhash,
        ))
        .await
        .expect("one lamport, from anybody, needs nobody's permission");
    let attacked = context
        .banks_client
        .get_account(f.position)
        .await
        .expect("read")
        .expect("position")
        .lamports;
    assert_eq!(
        attacked,
        f.position_lamports + 1,
        "the attack must actually have landed, or nothing below is a test"
    );

    // The negative control refuses, at the code, on the attacked state.
    let (accepted, logs, _, _) = submit(
        &mut context,
        underfunded,
        table,
        &addresses,
        "claims position: an over-declared prepayment is underfunding and still refuses",
    )
    .await
    .expect("underfunded");
    assert!(!accepted, "over-declaration must never be admissible");
    assert!(
        refused_with(&logs, POSITION_REFUSAL),
        "underfunding must refuse at {POSITION_REFUSAL:#x}, not merely fail:\n{}",
        logs.join("\n")
    );
    assert!(
        context
            .banks_client
            .get_account(f.position)
            .await
            .expect("read")
            .expect("position")
            .data
            .is_empty(),
        "a refused admission allocates nothing"
    );

    // THE WELD. Same request bytes, one stranger's lamport later.
    let (accepted, logs, _, _) = submit(
        &mut context,
        admit,
        table,
        &addresses,
        "claims position: admission survives a one-lamport front-run",
    )
    .await
    .expect("admit");
    assert!(
        accepted,
        "a stranger's lamport must not be able to block this admission:\n{}",
        logs.join("\n")
    );

    // A caller cannot make the donated live balance a new signed baseline.
    // Admission owns that immutable fact, so this substitution refuses before
    // mutation at the exact admission conjunct.
    let (accepted, logs, _, _) = submit(
        &mut context,
        substituted_close,
        table,
        &addresses,
        "claims position: close refuses a substituted admission baseline",
    )
    .await
    .expect("substituted close");
    assert!(!accepted);
    assert!(
        refused_with(&logs, ProtocolPositionSbfErrorV2::Admission as u32),
        "the substituted immutable baseline must refuse at Admission, not merely fail:\n{}",
        logs.join("\n")
    );

    // And conservation over the TRUE balance, dust included: every lamport the
    // stranger donated reaches the Market's rent credit rather than stranding.
    let rent_before = context
        .banks_client
        .get_account(f.rent_credit)
        .await
        .expect("read")
        .expect("rent")
        .lamports;
    let (accepted, logs, returned, _) = submit(
        &mut context,
        baseline_close,
        table,
        &addresses,
        "claims position: baseline close sweeps authenticated live donation",
    )
    .await
    .expect("honest close");
    assert!(accepted, "close refusal logs:\n{}", logs.join("\n"));
    let (_, bytes) = returned.expect("close receipt");
    let receipt = ProtocolPositionCloseReceiptV2::decode(&bytes).expect("close receipt");
    receipt
        .validate_request(
            baseline_close_request,
            hash(&baseline_close_request.to_bytes().expect("request bytes")).to_bytes(),
            CLAIMS.to_bytes(),
        )
        .expect("receipt binds the pre-attack request");
    assert_eq!(receipt.position_lamports(), f.position_lamports + 1);
    assert_eq!(receipt.admission_lamports(), f.admission_lamports);
    assert_eq!(
        receipt.total_credit(),
        f.position_lamports + 1 + f.admission_lamports
    );
    let rent_after = context
        .banks_client
        .get_account(f.rent_credit)
        .await
        .expect("read")
        .expect("rent")
        .lamports;
    assert_eq!(
        rent_after,
        rent_before + f.position_lamports + 1 + f.admission_lamports,
        "the stranger's lamport is swept with the rest, not stranded"
    );
    assert!(
        context
            .banks_client
            .get_account(f.position)
            .await
            .expect("read")
            .is_none()
            && context
                .banks_client
                .get_account(f.admission)
                .await
                .expect("read")
                .is_none()
    );
}

/// The refusal code the runtime actually returned, read out of its own logs.
fn refusal_code(logs: &[String]) -> Option<u32> {
    logs.iter()
        .find_map(|line| line.split("failed: custom program error: 0x").nth(1))
        .and_then(|code| u32::from_str_radix(code.trim(), 16).ok())
}

async fn lamports_of(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account")
        .map_or(0, |account| account.lamports)
}

/// A replay must be a genuinely new transaction, or the runtime deduplicates
/// it and the campaign observes nothing.
async fn advance_blockhash(context: &mut ProgramTestContext) {
    context
        .get_new_latest_blockhash()
        .await
        .expect("new blockhash");
}

fn lifecycle_receipt(
    returned: Option<(Pubkey, Vec<u8>)>,
    request: FractionalRetirementRequestV3,
) -> FractionalRetirementLifecycleReceiptV3 {
    let (program, bytes) = returned.expect("the act must emit a receipt");
    assert_eq!(program, CLAIMS);
    let receipt =
        FractionalRetirementLifecycleReceiptV3::decode(&bytes).expect("lifecycle receipt decodes");
    receipt
        .verify_for(
            request,
            hash(&request.to_bytes().expect("request bytes")).to_bytes(),
        )
        .expect("the receipt must bind to the request that produced it");
    receipt
}

/// The whole point of the family: a fractional market retires, end to end.
///
/// Four real transactions against real ELFs -- the audited Token-2022 v11
/// included -- with no planted cursor and no planted intermediate state. Begin
/// creates the cursor; the single coordinate closes the reserve Position, its
/// admission and the zero-supply shard Mint; Finish closes the cursor. Before
/// this lane none of the three retirement acts could run: `Begin` and `Finish`
/// were refused outright, and even a planted cursor could not have advanced
/// twice.
///
/// The conservation story is asserted at every step, in lamports:
///   - begin tops a vacant address up to rent exemption and settles nothing;
///   - the coordinate step moves the Position's and admission's whole balances
///     to the RentCredit and leaves the cursor's alone;
///   - finish moves the cursor's ENTIRE balance -- the principal plus the
///     stray lamport the fixture planted on the address beforehand -- to the
///     same RentCredit;
///   - and the RentCredit's final balance is exactly its opening balance plus
///     what the three closed Claims accounts held between them.
#[tokio::test]
async fn a_fractional_market_retires_end_to_end_from_begin_through_finish() {
    let (test, f) = fixture_for(CampaignV1::OrderedWalk);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();

    let admit = wrapped(
        &f,
        request(&f, ProtocolPositionActionV2::Admit),
        false,
        f.owner,
    );
    let addresses = lookup_addresses(payer, std::slice::from_ref(&admit));
    let table = create_live_lookup_table(&mut context, &addresses).await;

    let (accepted, logs, ..) = submit(
        &mut context,
        admit,
        table,
        &addresses,
        "ordered walk: admit the zero root reserve Position",
    )
    .await
    .expect("admit");
    assert!(accepted, "admit must be accepted: {logs:#?}");
    resolve_market(&mut context, f.core_market).await;

    let donated = lamports_of(&mut context, f.cursor).await;
    assert_eq!(
        donated,
        f.cursor_rent
            .checked_add(CURSOR_STRAY_LAMPORTS)
            .expect("over-funded cursor"),
        "this walk begins on a vacant address a stranger has already over-funded"
    );
    let rent_credit_before = lamports_of(&mut context, f.rent_credit).await;
    let position_lamports = lamports_of(&mut context, f.position).await;
    let admission_lamports = lamports_of(&mut context, f.admission).await;
    // Token-2022 closes the shard Mint to the same beneficiary, so the
    // coordinate step's conservation set is three accounts, not two.
    let mint_lamports = lamports_of(&mut context, f.mint).await;

    // ---- Begin -----------------------------------------------------------
    let begin_plan = plan_fractional_retirement_instruction_v3(
        &retirement_snapshot(&mut context, &f, payer, false).await,
    )
    .expect("public planner selects begin");
    assert_eq!(begin_plan.action, FractionalRetirementActionV3::Begin);
    assert_eq!(begin_plan.coordinate, None);
    assert_eq!(begin_plan.request, f.begin);
    let begin = begin_plan.instruction;
    let addresses = lookup_addresses(payer, std::slice::from_ref(&begin));
    let table = create_live_lookup_table(&mut context, &addresses).await;
    // ALT creation is transport setup and legitimately debits its rent from
    // the payer. Measure immediately before the retirement act so this wall
    // distinguishes transaction fees from an illicit cursor top-up.
    let payer_before = lamports_of(&mut context, payer).await;
    let (accepted, logs, returned, begin_units) = submit(
        &mut context,
        begin,
        table,
        &addresses,
        "ordered walk: begin creates the cursor on a terminal Market",
    )
    .await
    .expect("begin");
    assert!(accepted, "begin must be accepted: {logs:#?}");
    assert!(begin_units > 10_000, "{begin_units} compute units");
    let receipt = lifecycle_receipt(returned, f.begin);
    assert_eq!(receipt.action(), FractionalRetirementActionV3::Begin);
    assert_eq!(receipt.cursor(), f.cursor.to_bytes());
    assert_eq!(receipt.lamports_settled(), 0, "begin settles nothing");
    assert_eq!(receipt.revision(), 5);

    let cursor_account = context
        .banks_client
        .get_account(f.cursor)
        .await
        .expect("cursor")
        .expect("begin must create the cursor");
    assert_eq!(cursor_account.owner, CLAIMS);
    assert_eq!(
        cursor_account.data.len(),
        FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3
    );
    let created = FractionalRetirementCursorV3::decode(&cursor_account.data).expect("cursor state");
    assert_eq!((created.next_coordinate(), created.revision()), (0, 5));
    assert_eq!(created.representation_width(), 1);
    // The relation this lane exists to make true.
    assert_eq!(created.root_revision_anchor(), Ok(4));
    assert_eq!(created.historical_rent_principal(), f.cursor_rent);
    // A donation makes begin's bill SMALLER, never impossible: the cursor is
    // already past its minimum, so nothing is transferred and the stranger has
    // funded the retirement they meant to obstruct.
    assert_eq!(cursor_account.lamports, donated);
    assert!(
        lamports_of(&mut context, payer).await >= payer_before.saturating_sub(100_000),
        "an already-funded cursor must not be topped up out of the payer"
    );
    assert_eq!(
        lamports_of(&mut context, f.rent_credit).await,
        rent_credit_before
    );

    // ---- The single coordinate -------------------------------------------
    let step_plan = plan_fractional_retirement_instruction_v3(
        &retirement_snapshot(&mut context, &f, payer, true).await,
    )
    .expect("public planner selects exact next coordinate");
    assert_eq!(
        step_plan.action,
        FractionalRetirementActionV3::RetireCoordinate
    );
    assert_eq!(step_plan.coordinate, Some(0));
    assert_eq!(step_plan.request, f.retirement);
    let step = step_plan.instruction;
    let addresses = lookup_addresses(payer, std::slice::from_ref(&step));
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, _, step_units) = submit(
        &mut context,
        step,
        table,
        &addresses,
        "ordered walk: retire the sole coordinate against real Token-2022",
    )
    .await
    .expect("coordinate");
    assert!(accepted, "the coordinate step must be accepted: {logs:#?}");
    assert!(step_units > 30_000, "{step_units} compute units");
    let advanced = FractionalRetirementCursorV3::decode(
        &context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor")
            .expect("the cursor survives its own coordinate")
            .data,
    )
    .expect("advanced cursor");
    assert_eq!((advanced.next_coordinate(), advanced.revision()), (1, 6));
    // Frozen root, moving cursor, constant anchor. Under the comparison this
    // route used to carry, the walk could not have taken a second step, and
    // this is the only place that fact is observable on chain.
    assert_eq!(advanced.root_revision_anchor(), Ok(4));
    for closed in [f.position, f.admission, f.mint] {
        assert!(
            context
                .banks_client
                .get_account(closed)
                .await
                .expect("closed account")
                .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
            "the coordinate must close the Position, its admission and the Mint"
        );
    }
    let rent_credit_after_step = lamports_of(&mut context, f.rent_credit).await;
    assert_eq!(
        rent_credit_after_step,
        rent_credit_before
            .checked_add(position_lamports)
            .and_then(|sum| sum.checked_add(admission_lamports))
            .and_then(|sum| sum.checked_add(mint_lamports))
            .expect("coordinate rent conservation"),
        "the Position, admission and Mint rent lands whole in the RentCredit"
    );

    // ---- Finish ----------------------------------------------------------
    let cursor_lamports = lamports_of(&mut context, f.cursor).await;
    let finish_plan = plan_fractional_retirement_instruction_v3(
        &retirement_snapshot(&mut context, &f, payer, false).await,
    )
    .expect("public planner selects finish only after width coordinates");
    assert_eq!(finish_plan.action, FractionalRetirementActionV3::Finish);
    assert_eq!(finish_plan.coordinate, None);
    assert_eq!(finish_plan.request, f.finish);
    let finish = finish_plan.instruction;
    let addresses = lookup_addresses(payer, std::slice::from_ref(&finish));
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, returned, finish_units) = submit(
        &mut context,
        finish,
        table,
        &addresses,
        "ordered walk: finish closes the completed cursor",
    )
    .await
    .expect("finish");
    assert!(accepted, "finish must be accepted: {logs:#?}");
    assert!(finish_units > 10_000, "{finish_units} compute units");
    let receipt = lifecycle_receipt(returned, f.finish);
    assert_eq!(receipt.action(), FractionalRetirementActionV3::Finish);
    assert_eq!(receipt.next_coordinate(), receipt.representation_width());
    assert_eq!(receipt.revision(), 7);
    assert_eq!(
        receipt.lamports_settled(),
        cursor_lamports,
        "the receipt must name what actually moved"
    );
    assert!(
        receipt.lamports_settled() > f.cursor_rent,
        "the stray lamport is settled with the principal, not stranded"
    );
    assert!(
        context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor")
            .is_none_or(|account| account.lamports == 0
                && account.data.is_empty()
                && account.owner == system_program::ID),
        "finish must leave a vacant, system-owned address behind"
    );
    // Nothing created, nothing destroyed, across the whole walk.
    assert_eq!(
        lamports_of(&mut context, f.rent_credit).await,
        rent_credit_before
            .checked_add(position_lamports)
            .and_then(|sum| sum.checked_add(admission_lamports))
            .and_then(|sum| sum.checked_add(mint_lamports))
            .and_then(|sum| sum.checked_add(cursor_lamports))
            .expect("whole-walk conservation")
    );
    println!(
        "fractional ordered retirement CU: begin={begin_units}, coordinate={step_units}, finish={finish_units}"
    );
}

/// The writability exemption is real, and this is what would have been dead
/// without it.
///
/// `Finish` must take the RentCredit writable; `Begin` only reads it. Solana
/// merges privileges across the instructions of one transaction, so a builder
/// that batches the two acts -- or that simply reuses one meta list -- hands
/// `Begin` a writable RentCredit. Under an exact readonly pin that transaction
/// is dead, and the two ends of one walk can never share a transaction. Here
/// the same `Begin` is submitted with the RentCredit marked writable and must
/// be accepted, producing the identical cursor.
#[tokio::test]
async fn begin_admits_a_readonly_coordinate_that_the_callers_other_instruction_writes() {
    let (test, f) = fixture_for(CampaignV1::OrderedWalk);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    resolve_market(&mut context, f.core_market).await;

    let mut begin = begin_instruction(&f, payer);
    let rent_credit = begin
        .accounts
        .get_mut(9)
        .expect("the begin frame's RentCredit coordinate");
    assert_eq!(rent_credit.pubkey, f.rent_credit);
    assert!(!rent_credit.is_writable, "begin itself only reads it");
    rent_credit.is_writable = true;

    let addresses = lookup_addresses(payer, std::slice::from_ref(&begin));
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, returned, _) = submit(
        &mut context,
        begin,
        table,
        &addresses,
        "privilege exemption: begin accepts a RentCredit its neighbour writes",
    )
    .await
    .expect("begin with a writable RentCredit");
    assert!(
        accepted,
        "a readonly pin here would forbid the caller's transaction, not protect this one: {logs:#?}"
    );
    let receipt = lifecycle_receipt(returned, f.begin);
    assert_eq!(receipt.cursor(), f.cursor.to_bytes());
    assert_eq!(
        receipt.lamports_settled(),
        0,
        "and it still settles nothing"
    );
    let created = FractionalRetirementCursorV3::decode(
        &context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor")
            .expect("begin must create the cursor")
            .data,
    )
    .expect("cursor state");
    assert_eq!((created.next_coordinate(), created.revision()), (0, 5));
    assert_eq!(created.root_revision_anchor(), Ok(4));
}

/// Finish is not available to a walk that has not walked.
#[tokio::test]
async fn an_incomplete_walk_cannot_be_finished_and_the_cursor_survives_the_attempt() {
    let (test, f) = fixture_for(CampaignV1::OrderedWalk);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    resolve_market(&mut context, f.core_market).await;
    let begin = begin_instruction(&f, payer);
    // At the revision begin leaves behind, which is the only one a caller
    // could try before the coordinate runs. It must still refuse, because the
    // cursor still owes coordinate 0.
    let premature = Instruction {
        program_id: CLAIMS,
        accounts: finish_instruction(&f).accounts,
        data: FractionalRetirementRequestV3::new(
            FractionalRetirementActionV3::Finish,
            FractionalRetirementRequestInputV3 {
                expected_revision: 5,
                ..f.finish.input()
            },
        )
        .expect("premature finish")
        .to_bytes()
        .expect("premature finish bytes")
        .to_vec(),
    };
    let addresses = lookup_addresses(payer, &[begin.clone(), premature.clone()]);
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, ..) = submit(
        &mut context,
        begin,
        table,
        &addresses,
        "premature finish: begin the walk",
    )
    .await
    .expect("begin");
    assert!(accepted, "begin must be accepted: {logs:#?}");
    let before = context
        .banks_client
        .get_account(f.cursor)
        .await
        .expect("cursor");

    let (accepted, logs, ..) = submit(
        &mut context,
        premature,
        table,
        &addresses,
        "premature finish: a cursor that still owes a coordinate refuses",
    )
    .await
    .expect("premature finish");
    assert!(!accepted, "an incomplete walk must not finish");
    assert_eq!(
        refusal_code(&logs),
        Some(0x5008),
        "the completeness gate is a Representation refusal: {logs:#?}"
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor"),
        before,
        "the refused finish must leave the cursor exactly as it found it"
    );
}

/// A second begin finds a cursor that already exists, and there is no counter.
#[tokio::test]
async fn a_second_begin_refuses_on_the_cursors_own_existence() {
    let (test, f) = fixture_for(CampaignV1::OrderedWalkUnderfundedCursor);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    resolve_market(&mut context, f.core_market).await;
    let begin = begin_instruction(&f, payer);
    let addresses = lookup_addresses(payer, std::slice::from_ref(&begin));
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, ..) = submit(
        &mut context,
        begin.clone(),
        table,
        &addresses,
        "replay: the first begin",
    )
    .await
    .expect("begin");
    assert!(accepted, "begin must be accepted: {logs:#?}");
    // This fixture's cursor was underfunded, so begin took the transfer path
    // and landed it on exactly the rent minimum for 296 bytes -- no more.
    assert_eq!(lamports_of(&mut context, f.cursor).await, f.cursor_rent);
    let created = context
        .banks_client
        .get_account(f.cursor)
        .await
        .expect("cursor");

    advance_blockhash(&mut context).await;
    let (accepted, logs, ..) = submit(
        &mut context,
        begin,
        table,
        &addresses,
        "replay: the second begin refuses on the cursor's own vacancy",
    )
    .await
    .expect("second begin");
    assert!(!accepted, "a cursor that exists is a walk already begun");
    assert_eq!(
        refusal_code(&logs),
        Some(0x5008),
        "anti-replay is the account's own existence: {logs:#?}"
    );
    assert_eq!(
        context
            .banks_client
            .get_account(f.cursor)
            .await
            .expect("cursor"),
        created,
        "the refused begin must not disturb the cursor it found"
    );
}
