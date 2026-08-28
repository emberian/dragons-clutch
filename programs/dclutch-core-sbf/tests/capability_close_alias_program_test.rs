//! Real-SBF proof of the exact Core-to-Trading native-close alias frame.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, CompartmentFundingV1, ContentId,
    FundingAmountsV1, FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, v4::CapabilityProgramV4,
};
use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
};
use dclutch_direct_codec::{
    native_close_bundle_v1::{
        DIRECT_NATIVE_CLOSE_SELECTOR_V1, direct_native_close_account_profile_schema_v1,
        direct_native_close_effect_schema_v1, direct_native_close_request_v1,
    },
    ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleInputV4, build_direct_inline_ordinary_hot_bundle_v4,
    },
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
    },
    ordinary_geometry_v3::DirectOrdinaryGeometryV3,
    program_set_v4::build_direct_inline_ordinary_native_close_program_set_v1,
    successor::{
        DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigV1, DirectRootStateV1,
    },
};
use dclutch_market_core_codec::{
    Action, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, Request, Role, STATE_BYTES,
};
use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_HEADER_BYTES_V3;
use dclutch_product_runtime_v2::{
    DOMAIN_CUT_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
    RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2,
        LifecycleRentCreditV2,
    },
};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc4; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc5; 32]);
const GENERATION: u64 = 9;
const CAPACITY_PROFILE: [u8; 32] = [0x44; 32];

#[derive(Clone, Copy)]
enum Fault {
    None,
    MissingAlias,
    ShiftedAlias,
    PairSubstitution,
    ExtraAlias,
}

struct Artifacts {
    core: Vec<u8>,
    trading: Vec<u8>,
    registry: Vec<u8>,
}

struct Fixture {
    instruction: Instruction,
    market: Pubkey,
    root: Pubkey,
    funding: Pubkey,
    rent_credit: Pubkey,
    root_lamports: u64,
    funding_lamports: u64,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        trading: fs::read(directory.join("dclutch_trading_sbf.so")).expect("Trading ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
    }
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("identity")
}

fn content(bytes: [u8; 32]) -> ContentId {
    ContentId::new(bytes).expect("content identity")
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&0_u64.to_le_bytes());
    bytes[12] = 0;
    bytes[45..].copy_from_slice(elf);
    bytes
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    artifact_name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(artifact_name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
    );
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        content([semantic; 32]),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
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
            release.deployment_slot(),
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("deployment observation"),
    )
}

fn activation(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let releases = [
        release(CORE_PROGRAM_ID, 0x51, &artifacts.core),
        release(RESOLUTION_PROGRAM_ID, 0x52, &artifacts.core),
        release(TRADING_PROGRAM_ID, 0x53, &artifacts.trading),
        release(RESOLUTION_PROGRAM_ID, 0x52, &artifacts.core),
        release(RESOLUTION_PROGRAM_ID, 0x52, &artifacts.core),
    ];
    let release_set = ExecutionReleaseSetV1::new(
        binding(releases[0]),
        binding(releases[1]),
        binding(releases[2]),
        binding(releases[3]),
        binding(releases[4]),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let release_set_content = content(release_set_id);
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, release_set_content).expect("activation cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, releases[0]),
        (ExecutionRoleV1::Claims, releases[1]),
        (ExecutionRoleV1::Trading, releases[2]),
        (ExecutionRoleV1::Resolution, releases[3]),
        (ExecutionRoleV1::Custody, releases[4]),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            release_set_content,
            &release_set,
            role,
            &activation_input(release),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete activation cache");
    (release_set_id, bytes)
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    let lamports = Rent::default().minimum_balance(data.len()).max(1);
    add_account_with_lamports(test, key, owner, data, lamports);
}

fn add_account_with_lamports(
    test: &mut ProgramTest,
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    lamports: u64,
) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    bytes: Vec<u8>,
) -> (Pubkey, Pubkey, u8, u8) {
    let digest = hash(&bytes).to_bytes();
    let (raw, raw_bump) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    );
    let (staging, staging_bump) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    );
    add_account(test, raw, REGISTRY_PROGRAM_ID, bytes);
    add_account(test, staging, system_program::ID, Vec::new());
    (raw, staging, raw_bump, staging_bump)
}

fn ordinary_lengths() -> [u32; DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3 as usize] {
    let geometry = DirectOrdinaryGeometryV3::CANONICAL;
    let outcomes = usize::try_from(geometry.outcome_count()).expect("outcome count");
    let mut output = [0_u32; DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3 as usize];
    output[0] = u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)
        .expect("root width");
    output[1] = u32::try_from(DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config width");
    output[2] = u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("product width");
    output[3] = u32::try_from(PORTFOLIO_HEADER_BYTES + outcomes * PORTFOLIO_COEFFICIENT_BYTES)
        .expect("portfolio width");
    output[4] = u32::try_from(BASIS_HEADER_BYTES_V3).expect("basis width");
    output[5] = u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker width");
    output[7] = u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width");
    output[8] = output[5];
    output[10] = u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("program width");
    output[13] = u32::try_from(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + outcomes * 8)
        .expect("Claims Market width");
    output[14] = output[4];
    output[16] = output[2];
    output[18] = u32::try_from(
        dclutch_direct_codec::ordinary_geometry_v3::DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3
            + outcomes * DOMAIN_CUT_BYTES,
    )
    .expect("domain width");
    output[20] = output[3];
    output[22] = 17;
    output[23] = u32::try_from(STATE_BYTES).expect("Core state width");
    output[24] = u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("cache width");
    for coordinate in [25_usize, 26, 28, 30] {
        output[coordinate] = u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("program width");
    }
    for coordinate in [27_usize, 29, 31] {
        output[coordinate] = 1_024;
    }
    output[32] = u32::try_from(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + outcomes * 8)
        .expect("Claims position width");
    output[33] = output[32];
    output[35] = output[23];
    output[36] = output[24];
    output[37] = output[25];
    output[38] = output[26];
    output[39] = output[27];
    output[40] = u32::try_from(REALM_BYTES).expect("Realm width");
    output[42] = u32::try_from(dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1)
        .expect("Custody replay width");
    output[43] = 82;
    output[44] = 165;
    output[45] = 165;
    output[47] = u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("token program width");
    output[73] = 165;
    for (account, representative) in [
        (49, 23),
        (50, 24),
        (51, 25),
        (52, 26),
        (53, 27),
        (54, 40),
        (55, 41),
        (56, 42),
        (57, 43),
        (58, 44),
        (59, 45),
        (60, 46),
        (61, 47),
        (63, 23),
        (64, 24),
        (65, 25),
        (66, 26),
        (67, 27),
        (68, 40),
        (69, 41),
        (70, 42),
        (71, 43),
        (72, 44),
        (74, 46),
        (75, 47),
        (77, 23),
        (78, 24),
        (79, 25),
        (80, 26),
        (81, 27),
        (82, 40),
        (83, 41),
        (84, 42),
        (85, 43),
        (86, 44),
        (87, 73),
        (88, 46),
        (89, 47),
    ] {
        output[account] = output[representative];
    }
    output[usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3)] =
        u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Custody program width");
    output
}

fn build_fixture(fault: Fault) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_trading_sbf",
            TRADING_PROGRAM_ID,
            artifacts.trading.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            RESOLUTION_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            RENT_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }

    let (release_set, cache_data) = activation(&artifacts);
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, cache, REGISTRY_PROGRAM_ID, cache_data);

    let ordinary =
        build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
            account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                logical_data_lengths: &ordinary_lengths(),
            },
            capacity_profile: CAPACITY_PROFILE,
        })
        .expect("canonical ordinary bundle");
    let release =
        build_direct_inline_ordinary_native_close_program_set_v1(ordinary, CAPACITY_PROFILE)
            .expect("canonical ordinary/native-close release");
    assert_eq!(
        u32::from_le_bytes(
            direct_native_close_request_v1()[12..16]
                .try_into()
                .expect("selector")
        ),
        DIRECT_NATIVE_CLOSE_SELECTOR_V1
    );
    let ordinary_descriptor =
        CapabilityProgramV4::decode(&release.ordinary.descriptor).expect("ordinary descriptor");
    let config = DirectExecutionConfigV1::new(100, 0, [0x55; 32])
        .expect("Direct config")
        .encode();
    let config_digest = hash(&config).to_bytes();

    let root_space = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;
    let root_lamports = Rent::default().minimum_balance(root_space);
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(root_lamports.checked_sub(1).expect("rent quote"))
            .expect("native rent quote"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding amounts");
    let entry = CapabilityEntryV1::new(
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
        content(ordinary_descriptor.capacity_profile().to_bytes()),
        content(ordinary_descriptor.root_schema().to_bytes()),
        content(ordinary_descriptor.derivation_policy().to_bytes()),
        ActivationPolicy::PrepaidLazy,
        u64::MAX,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("funding quote"),
    )
    .expect("manifest entry");
    let mut manifest = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
    let manifest_digest = hash(&manifest).to_bytes();
    let manifest_id = content(manifest_digest);

    let adapter = PRODUCTION_ADAPTER_RELEASES[0];
    let realm_data = RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: [0x61; 32],
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm")
    .to_bytes()
    .to_vec();
    let realm_digest = hash(&realm_data).to_bytes();
    let (realm_raw, realm_staging, _, _) =
        add_record(&mut test, REALM_SCHEMA_RELEASE_ID_V1, realm_data);
    let (manifest_raw, manifest_staging, manifest_raw_bump, manifest_staging_bump) = add_record(
        &mut test,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest.clone(),
    );
    let (program_set_raw, program_set_staging, release_raw_bump, release_staging_bump) = add_record(
        &mut test,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        release.program_set.clone(),
    );
    let (config_raw, config_staging, config_raw_bump, config_staging_bump) = add_record(
        &mut test,
        ordinary_descriptor.config_schema().to_bytes(),
        config.to_vec(),
    );
    let (profile_raw, profile_staging, _, _) = add_record(
        &mut test,
        direct_native_close_account_profile_schema_v1(),
        release.native_close.account_profile.clone(),
    );
    let (effect_raw, effect_staging, _, _) = add_record(
        &mut test,
        direct_native_close_effect_schema_v1(),
        release.native_close.effect.clone(),
    );
    let (descriptor_raw, descriptor_staging, _, _) = add_record(
        &mut test,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        release.native_close.descriptor.clone(),
    );

    let wire_selection = CapabilityExecutionSelectionV1::new(
        0,
        manifest_id,
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
    )
    .expect("selection");
    let persisted_selection =
        wire_selection.with_capability_release_record_bumps(release_raw_bump, release_staging_bump);

    let mut state = CoreState {
        phase: Phase::Retiring,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity([0x71; 32]),
            realm_id: identity(realm_digest),
            product_record: identity([0x72; 32]),
            product_id: identity([0x73; 32]),
            resolution_policy: identity([0x74; 32]),
            capability_manifest: identity(manifest_digest),
            selected_release_set: identity(release_set),
            registry_program: identity(REGISTRY_PROGRAM_ID.to_bytes()),
            generation: GENERATION,
        },
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity([0x75; 32]),
        terminal_receipt: Some(identity([0x77; 32])),
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    state.identity.market_id = identity(market.to_bytes());
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    state.rent_beneficiary = identity(rent_credit.to_bytes());
    let state_bytes = state.encode().expect("Core state");
    add_account(&mut test, market, CORE_PROGRAM_ID, state_bytes.to_vec());
    let rent_credit_data = LifecycleRentCreditV2::new(
        RefundAuthority::new([0x76; 32]).expect("refund"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        GENERATION,
        rent_credit_bump,
    )
    .expect("RentCredit")
    .to_bytes()
    .to_vec();
    add_account(&mut test, rent_credit, RENT_PROGRAM_ID, rent_credit_data);

    let root_header = CapabilityRootHeaderV1::new(
        content(release_set),
        market.to_bytes(),
        GENERATION,
        persisted_selection,
        SelectedRecordBumpsV1::new(
            manifest_raw_bump,
            manifest_staging_bump,
            config_raw_bump,
            config_staging_bump,
        ),
    )
    .expect("root header");
    let root =
        Pubkey::find_program_address(&root_header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    let mut root_data = root_header.to_bytes().to_vec();
    root_data.extend_from_slice(
        &DirectRootStateV1::new()
            .begin_retiring()
            .expect("Retiring root")
            .encode(),
    );
    add_account_with_lamports(
        &mut test,
        root,
        TRADING_PROGRAM_ID,
        root_data,
        root_lamports,
    );

    let decoded_manifest = CapabilityManifestV1::decode(&manifest).expect("decoded manifest");
    let mut funding_data = vec![0; funding_ledger_bytes_v2(1).expect("funding width")];
    FundingLedgerV2::initialize(&mut funding_data, manifest_id, decoded_manifest, 1)
        .expect("funding initialize");
    FundingLedgerV2::activate_in_place(&mut funding_data, manifest_id, decoded_manifest, 0, 1)
        .expect("funding activate");
    let funding_derivation = CapabilityFundingLedgerDerivationV2::new(
        TRADING_PROGRAM_ID.to_bytes(),
        market.to_bytes(),
        GENERATION,
        manifest_id,
        FundingLedgerV2::decode(&funding_data).expect("funding ledger"),
    )
    .expect("funding derivation");
    let funding =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &TRADING_PROGRAM_ID).0;
    let funding_lamports = Rent::default().minimum_balance(funding_data.len());
    add_account_with_lamports(
        &mut test,
        funding,
        TRADING_PROGRAM_ID,
        funding_data,
        funding_lamports,
    );

    let family_request = direct_native_close_request_v1();
    let funding_header = CapabilityFundingHeaderV2::new(1, 1, 1).expect("funding header");
    let mut role_request = wire_selection.to_bytes().to_vec();
    role_request.extend_from_slice(&funding_header.encode());
    role_request.extend_from_slice(&family_request);
    let role_digest = hash(&role_request).to_bytes();
    let context = [0x81; 32];
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        role_digest,
    )
    .expect("caller seeds");
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &CORE_PROGRAM_ID).0;
    add_account(&mut test, caller, system_program::ID, Vec::new());
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::CloseCapability,
        Role::Trading,
        identity(CORE_PROGRAM_ID.to_bytes()),
        identity(caller.to_bytes()),
        identity(release_set),
        identity(market.to_bytes()),
        identity(context),
        identity(hash(&state_bytes).to_bytes()),
        identity(role_digest),
        GENERATION,
        0,
        0,
        u32::try_from(role_request.len()).expect("role request width"),
    )
    .expect("Core envelope");
    let request = Request::administrative(
        Action::CloseCapability,
        GENERATION,
        identity(market.to_bytes()),
    );
    let mut data = request.encode().expect("Core request").to_vec();
    data.extend_from_slice(&envelope.encode().expect("Core envelope bytes"));
    data.extend_from_slice(&role_request);

    let hostile = Pubkey::new_unique();
    add_account(&mut test, hostile, system_program::ID, Vec::new());
    let mut accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new_readonly(realm_raw, false),
        AccountMeta::new_readonly(realm_staging, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(manifest_staging, false),
        AccountMeta::new(funding, false),
        AccountMeta::new(root, false),
        AccountMeta::new_readonly(cache, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(TRADING_PROGRAM_ID), false),
        AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(RESOLUTION_PROGRAM_ID), false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new_readonly(program_set_raw, false),
        AccountMeta::new_readonly(program_set_staging, false),
        AccountMeta::new_readonly(config_raw, false),
        AccountMeta::new_readonly(config_staging, false),
        AccountMeta::new_readonly(profile_raw, false),
        AccountMeta::new_readonly(profile_staging, false),
        AccountMeta::new_readonly(effect_raw, false),
        AccountMeta::new_readonly(effect_staging, false),
        AccountMeta::new_readonly(cache, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(TRADING_PROGRAM_ID), false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(descriptor_raw, false),
        AccountMeta::new_readonly(descriptor_staging, false),
        AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
        AccountMeta::new(rent_credit, false),
    ];
    match fault {
        Fault::None => {}
        Fault::MissingAlias => accounts[25] = AccountMeta::new_readonly(hostile, false),
        Fault::ShiftedAlias => accounts[25] = accounts[8].clone(),
        Fault::PairSubstitution => {
            accounts[7] = AccountMeta::new_readonly(hostile, false);
            accounts[25] = AccountMeta::new_readonly(hostile, false);
        }
        Fault::ExtraAlias => accounts[32] = accounts[35].clone(),
    }
    assert_eq!(accounts.len(), 37);
    (
        test,
        Fixture {
            instruction: Instruction {
                program_id: CORE_PROGRAM_ID,
                accounts,
                data,
            },
            market,
            root,
            funding,
            rent_credit,
            root_lamports,
            funding_lamports,
        },
    )
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            instruction,
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

#[tokio::test]
async fn canonical_high_selector_closes_through_real_core_and_trading() {
    let (test, fixture) = build_fixture(Fault::None);
    let mut context = test.start_with_context().await;
    let credit_before = account(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit");
    submit(&mut context, fixture.instruction.clone())
        .await
        .expect("Core-to-Trading native close");
    for closed in [fixture.root, fixture.funding] {
        if let Some(account) = account(&mut context, closed).await {
            assert_eq!(account.lamports, 0);
            assert_eq!(account.owner, system_program::ID);
            assert!(account.data.is_empty());
        }
    }
    let market = account(&mut context, fixture.market).await.expect("Market");
    let state = CoreState::decode(&market.data).expect("Core poststate");
    assert_eq!(state.outstanding_capabilities, 0);
    let credit = account(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit poststate");
    assert_eq!(
        credit.lamports,
        credit_before
            .lamports
            .checked_add(fixture.root_lamports)
            .and_then(|value| value.checked_add(fixture.funding_lamports))
            .expect("classified close refund")
    );
}

#[tokio::test]
async fn shifted_substituted_and_extra_aliases_refuse_with_rollback() {
    for fault in [
        Fault::MissingAlias,
        Fault::ShiftedAlias,
        Fault::PairSubstitution,
        Fault::ExtraAlias,
    ] {
        let (test, fixture) = build_fixture(fault);
        let mut context = test.start_with_context().await;
        let before = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        submit(&mut context, fixture.instruction)
            .await
            .expect_err("hostile alias frame refuses");
        let after = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        assert_eq!(after, before);
    }
}
