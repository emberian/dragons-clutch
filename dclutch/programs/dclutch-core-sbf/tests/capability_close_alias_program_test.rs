//! Real-SBF proof of the exact Core-to-Trading native-close alias frame.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
};
use dclutch_custody::token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, CompartmentFundingV1, ContentId,
    FundingAmountsV1, FundingLedgerStatusV2, FundingLedgerV2, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY, capability_dependency_closure_mask_v1,
    derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
};
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, v4::CapabilityProgramV4,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
    RealmV1Input,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2,
        LifecycleRentCreditV2,
    },
};
use dclutch_market::{
    Action, CapabilityFundingHeaderV2, CapabilityRouteLayoutV1, CoreEffectActionV1,
    CoreEffectEnvelopeV1, CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    Readiness, Request, Role, STATE_BYTES, StateBumpsV1,
};
use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product::payoff::runtime_v3::BASIS_HEADER_BYTES_V3;
use dclutch_product::{DOMAIN_CUT_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_trading::{
    activation_bundle_v1::{
        DIRECT_ACTIVATION_SELECTOR_V1, direct_activation_account_profile_schema_v1,
        direct_activation_descriptor_schema_v1, direct_activation_effect_schema_v1,
        direct_activation_request_v1,
    },
    begin_retiring_bundle_v1::{
        direct_begin_retiring_account_profile_schema_v1, direct_begin_retiring_effect_schema_v1,
    },
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
    program_set_v4::{
        build_direct_inline_ordinary_lifecycle_program_set_v1,
        build_direct_inline_ordinary_native_close_program_set_v1,
    },
    retirement_v1::{
        DirectBeginRetiringReceiptV1, DirectBeginRetiringRequestV1,
        direct_begin_retiring_context_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigV1, DirectRootStateV1,
    },
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{instruction::InstructionError, signature::Signer, transaction::TransactionError};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

/// The exact close frame Core parses and `dclutch-operator`'s
/// `project_direct_native_close_coordinate_closure_v1` emits: five record and
/// Market accounts, an `F=2` physical funding slice, eleven Core-owned fixed
/// accounts, and a twenty-account Direct child tail.
///
/// The frame's WIDTH and every coordinate the hostilities aim at are read off
/// this layout rather than written down as positions. `67e96e5b` is why:
/// `open_market_program_test` carried its frame as literals, `2dc53776` moved
/// the route, and the file spent four days submitting a frame one account
/// short -- with four hostile assertions passing on the length refusal none of
/// them was about. The honest frame below is still assembled in order, because
/// it is the frame's CONTENT and no semantic owner can supply the fixture's
/// keys; the width assertion is what catches it drifting.
fn close_layout() -> CapabilityRouteLayoutV1 {
    CapabilityRouteLayoutV1::new(2, 20).expect("the F=2 twenty-tail close layout is in bounds")
}

/// `CoreSbfError::AccountFrame`, `::Release` and `::Funding`.
const CORE_ACCOUNT_FRAME: u32 = 0x3001;
const CORE_RELEASE: u32 = 0x3004;
const CORE_FUNDING: u32 = 0x3008;
/// `TradingSbfError::Root`.
const TRADING_ROOT: u32 = 0x4002;
/// What a replayed founding refuses with, derived from the enum rather than
/// typed:  pins the Market prestate digest, and the
/// founding it authorized moved that state.
///
/// The code is coarse and this file cannot make it finer:
/// publishes  for a seven-way disjunction whose
/// last term is the prestate digest. Six of the seven are fixed by the
/// instruction bytes, which the replay reuses verbatim, so the digest is the
/// only term that can have moved -- but that is the test's reasoning, not the
/// program's word, and a split discriminant is what would make it the latter.
const REPLAY_REFUSAL: u32 = dclutch_core_sbf::CoreSbfError::Instruction as u32;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc4; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc5; 32]);
const GENERATION: u64 = 9;
const CAPACITY_PROFILE: [u8; 32] = [0x44; 32];

/// Where the selected Direct entry sits in the market being closed.
///
/// The four-entry fixture this file was written against put it LAST, so the
/// Resolution-owned dependency ledger led the funding slice and the Trading one
/// was written. Devnet `GyD95eyE…` puts it FIRST, and the frame is the mirror
/// image. Both are real; neither is a position anything may type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloseShape {
    /// Selected entry at index 3, masks `0b0111` / `0b1000`.
    SelectedLast,
    /// Selected entry at index 0, masks `0x0001` / `0x000e` -- the real market.
    SelectedFirst,
}

impl CloseShape {
    const fn selected_entry_index(self) -> u16 {
        match self {
            Self::SelectedLast => 3,
            Self::SelectedFirst => 0,
        }
    }

    /// Whether the selected row's own ledger leads the funding slice, which is
    /// true exactly when its mask's lowest selected index is the lower one.
    const fn selected_leads(self) -> bool {
        matches!(self, Self::SelectedFirst)
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    MissingAlias,
    ShiftedAlias,
    PairSubstitution,
    ExtraAlias,
    DependencyWritable,
    DependencyReordered,
    DependencySubstitution,
    DependencyMutated,
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
    dependency_funding: Pubkey,
    funding: Pubkey,
    rent_credit: Pubkey,
    root_lamports: u64,
    funding_lamports: u64,
    dependency_funding_data: Vec<u8>,
    dependency_funding_lamports: u64,
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

/// The exemption-scaled rate this bank charges, which is what a founding here
/// would have recorded in its ledger header. Every account this file funds is
/// priced with the same `Rent::default()`, so the ledgers' own
/// `validate_recorded_native_custody` has to agree with it.
fn funded_rent_rate(account_bytes: usize) -> u32 {
    let rent = Rent::default();
    derive_funded_rent_rate_v2(
        rent.minimum_balance(0),
        account_bytes,
        rent.minimum_balance(account_bytes),
    )
    .expect("Rent::default() is affine in the account length")
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
        dclutch_trading::ordinary_geometry_v3::DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3
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
    output[42] =
        u32::try_from(dclutch_custody::CUSTODY_REPLAY_BYTES_V1).expect("Custody replay width");
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
    build_fixture_for(CloseShape::SelectedLast, fault)
}

fn build_fixture_for(shape: CloseShape, fault: Fault) -> (ProgramTest, Fixture) {
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
    let selected_entry_index = shape.selected_entry_index();
    let selected_bit = 1_u16 << selected_entry_index;
    let dependency_mask = 0b1111_u16 & !selected_bit;
    // Every OTHER manifest index, ascending: the edge set a Direct founding
    // derives, whichever position the sort gives the selected entry.
    let mut direct_dependencies = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
    {
        let mut position = 0_usize;
        for index in 0_u8..4 {
            if u16::from(index) == selected_entry_index {
                continue;
            }
            direct_dependencies[position] = index;
            position += 1;
        }
    }
    let direct_entry = CapabilityEntryV1::new(
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
        content(ordinary_descriptor.capacity_profile().to_bytes()),
        content(ordinary_descriptor.root_schema().to_bytes()),
        content(ordinary_descriptor.derivation_policy().to_bytes()),
        ActivationPolicy::PrepaidLazy,
        u64::MAX,
        3,
        direct_dependencies,
        FundingQuoteV1::new(amounts, None).expect("funding quote"),
    )
    .expect("Direct manifest entry");
    // Companion kinds sort BELOW the Direct kind for the last-index shape and
    // above it for the first-index one, because manifest order is kind-digest
    // order and the position is what the shape is about.
    let companion_kinds = if shape.selected_leads() {
        [0xfd_u8, 0xfe, 0xff]
    } else {
        [0x10_u8, 0x11, 0x12]
    };
    let dependency_entries = companion_kinds.map(|kind| {
        CapabilityEntryV1::new(
            content([kind; 32]),
            content([kind.wrapping_add(0x10); 32]),
            content([kind.wrapping_add(0x20); 32]),
            content([kind.wrapping_add(0x30); 32]),
            content([kind.wrapping_add(0x40); 32]),
            content([kind.wrapping_add(0x50); 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("dependency funding quote"),
        )
        .expect("Resolution dependency entry")
    });
    assert_eq!(
        ordinary_descriptor.kind().to_bytes() < [companion_kinds[0]; 32],
        shape.selected_leads(),
        "the companion kinds must place the selected entry where the shape says",
    );
    let entries = if shape.selected_leads() {
        [
            direct_entry,
            dependency_entries[0],
            dependency_entries[1],
            dependency_entries[2],
        ]
    } else {
        [
            dependency_entries[0],
            dependency_entries[1],
            dependency_entries[2],
            direct_entry,
        ]
    };
    let mut manifest = vec![0; MANIFEST_HEADER_BYTES + 4 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest).expect("manifest");
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
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
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
        selected_entry_index,
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
        bumps: StateBumpsV1::UNRECORDED,
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
    let mut dependency_funding_data =
        vec![0; funding_ledger_bytes_v2(3).expect("dependency funding width")];
    let dependency_funding_rate = funded_rent_rate(dependency_funding_data.len());
    FundingLedgerV2::initialize(
        &mut dependency_funding_data,
        manifest_id,
        decoded_manifest,
        dependency_mask,
        dependency_funding_rate,
    )
    .expect("dependency funding initialize");
    for entry_index in 0_u16..4 {
        if dependency_mask & (1_u16 << entry_index) == 0 {
            continue;
        }
        FundingLedgerV2::activate_in_place(
            &mut dependency_funding_data,
            manifest_id,
            decoded_manifest,
            entry_index,
            u64::from(entry_index) + 1,
        )
        .expect("dependency funding activate");
    }
    let dependency_derivation = CapabilityFundingLedgerDerivationV2::new(
        RESOLUTION_PROGRAM_ID.to_bytes(),
        market.to_bytes(),
        GENERATION,
        manifest_id,
        FundingLedgerV2::decode(&dependency_funding_data).expect("dependency funding ledger"),
    )
    .expect("dependency funding derivation");
    let dependency_funding = Pubkey::find_program_address(
        &dependency_derivation.seed_components(),
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    let dependency_funding_lamports =
        Rent::default().minimum_balance(dependency_funding_data.len());
    let mut observed_dependency_funding_data = dependency_funding_data.clone();
    if matches!(fault, Fault::DependencyMutated) {
        *observed_dependency_funding_data
            .last_mut()
            .expect("nonempty dependency ledger") ^= 1;
    }
    add_account_with_lamports(
        &mut test,
        dependency_funding,
        RESOLUTION_PROGRAM_ID,
        observed_dependency_funding_data.clone(),
        dependency_funding_lamports,
    );

    let mut funding_data = vec![0; funding_ledger_bytes_v2(1).expect("funding width")];
    let funding_rate = funded_rent_rate(funding_data.len());
    FundingLedgerV2::initialize(
        &mut funding_data,
        manifest_id,
        decoded_manifest,
        selected_bit,
        funding_rate,
    )
    .expect("funding initialize");
    FundingLedgerV2::activate_in_place(
        &mut funding_data,
        manifest_id,
        decoded_manifest,
        selected_entry_index,
        4,
    )
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
    let funding_header = CapabilityFundingHeaderV2::new(2, 4, 0b1111).expect("funding header");
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
    ];
    // The funding slice in canonical mask order, which is the SHAPE's order and
    // not a controller's position.
    if shape.selected_leads() {
        accounts.push(AccountMeta::new(funding, false));
        accounts.push(AccountMeta::new_readonly(dependency_funding, false));
    } else {
        accounts.push(AccountMeta::new_readonly(dependency_funding, false));
        accounts.push(AccountMeta::new(funding, false));
    }
    accounts.extend([
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
    ]);
    let layout = close_layout();
    // The first of the seven exact close aliases: the Registry activation
    // cache, carried once for Core and once again inside the Direct child
    // tail. Both halves come from `close_alias_pairs()`, so a route that
    // moves its aliases moves these hostilities with it.
    let (cache_left, cache_right) = layout
        .close_alias_pairs()
        .first()
        .copied()
        .expect("the close route admits seven aliases");
    // Two child-tail coordinates that are NOT an admitted pair: the System
    // program and the Rent program. Aliasing them is the extra-alias case.
    let unpaired_left = layout.child_start() + 15;
    let unpaired_right = layout.child_start() + 18;
    match fault {
        Fault::None => {}
        Fault::MissingAlias => accounts[cache_right] = AccountMeta::new_readonly(hostile, false),
        Fault::ShiftedAlias => accounts[cache_right] = accounts[layout.core_program()].clone(),
        Fault::PairSubstitution => {
            accounts[cache_left] = AccountMeta::new_readonly(hostile, false);
            accounts[cache_right] = AccountMeta::new_readonly(hostile, false);
        }
        Fault::ExtraAlias => accounts[unpaired_left] = accounts[unpaired_right].clone(),
        Fault::DependencyWritable => accounts[layout.funding_start()].is_writable = true,
        Fault::DependencyReordered => {
            accounts.swap(layout.funding_start(), layout.funding_end() - 1)
        }
        Fault::DependencySubstitution => {
            accounts[layout.funding_start()] = AccountMeta::new_readonly(hostile, false);
        }
        Fault::DependencyMutated => {}
    }
    assert_eq!(accounts.len(), layout.account_count());
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
            dependency_funding,
            funding,
            rent_credit,
            root_lamports,
            funding_lamports,
            dependency_funding_data: observed_dependency_funding_data,
            dependency_funding_lamports,
        },
    )
}

#[derive(Clone)]
struct BeginRetiringRoute {
    seed: u8,
    instruction: Instruction,
    root: Pubkey,
    request: Vec<u8>,
    expected_post_root: Vec<u8>,
    expected_post_digest: [u8; 32],
    root_lamports: u64,
}

fn build_begin_retiring_campaign() -> (ProgramTest, Vec<BeginRetiringRoute>, [u8; 32]) {
    // Reuse the existing release-authenticated real-SBF deployment membrane;
    // the extra close fixture accounts are disjoint and never appear in these
    // top-level 20-account instructions.
    let (mut test, _) = build_fixture(Fault::None);
    let artifacts = artifacts();
    let (release_set, _) = activation(&artifacts);
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;

    let ordinary =
        build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
            account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                logical_data_lengths: &ordinary_lengths(),
            },
            capacity_profile: CAPACITY_PROFILE,
        })
        .expect("canonical ordinary bundle");
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(ordinary, CAPACITY_PROFILE)
        .expect("canonical three-selector lifecycle release");
    let ordinary_descriptor =
        CapabilityProgramV4::decode(&release.ordinary.descriptor).expect("ordinary descriptor");
    let config = DirectExecutionConfigV1::new(100, 0, [0x56; 32])
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

    let (manifest_raw, _, manifest_raw_bump, manifest_staging_bump) = add_record(
        &mut test,
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest,
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
        direct_begin_retiring_account_profile_schema_v1(),
        release.begin_retiring.account_profile.clone(),
    );
    let (effect_raw, effect_staging, _, _) = add_record(
        &mut test,
        direct_begin_retiring_effect_schema_v1(),
        release.begin_retiring.effect.clone(),
    );
    let (descriptor_raw, descriptor_staging, _, _) = add_record(
        &mut test,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        release.begin_retiring.descriptor.clone(),
    );
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        manifest_id,
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
    )
    .expect("selection")
    .with_capability_release_record_bumps(release_raw_bump, release_staging_bump);
    let record_bumps = SelectedRecordBumpsV1::new(
        manifest_raw_bump,
        manifest_staging_bump,
        config_raw_bump,
        config_staging_bump,
    );

    let mut routes = Vec::with_capacity(20);
    for seed in 0_u8..20 {
        let generation = 100_u64 + u64::from(seed);
        let mut state = CoreState {
            phase: Phase::Retiring,
            readiness: Readiness::Consumed,
            terminal_winner: u32::from(seed % 3),
            identity: MarketIdentity {
                market_id: identity([0x80_u8.wrapping_add(seed); 32]),
                realm_id: identity([0x90_u8.wrapping_add(seed); 32]),
                product_record: identity([0xa0_u8.wrapping_add(seed); 32]),
                product_id: identity([0xb0_u8.wrapping_add(seed); 32]),
                resolution_policy: identity([0xc0_u8.wrapping_add(seed); 32]),
                capability_manifest: identity(manifest_digest),
                selected_release_set: identity(release_set),
                registry_program: identity(REGISTRY_PROGRAM_ID.to_bytes()),
                generation,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: u64::MAX,
            rent_beneficiary: identity([0xd0_u8.wrapping_add(seed); 32]),
            terminal_receipt: Some(identity([0xe0_u8.wrapping_add(seed); 32])),
            bumps: StateBumpsV1::UNRECORDED,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
            &CORE_PROGRAM_ID,
        )
        .0;
        state.identity.market_id = identity(market.to_bytes());
        let market_data = state.encode().expect("Retiring Core Market");
        add_account(&mut test, market, CORE_PROGRAM_ID, market_data.to_vec());

        let header = CapabilityRootHeaderV1::new(
            content(release_set),
            market.to_bytes(),
            generation,
            selection,
            record_bumps,
        )
        .expect("root header");
        let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&DirectRootStateV1::new().encode());
        add_account_with_lamports(
            &mut test,
            root,
            TRADING_PROGRAM_ID,
            root_data.clone(),
            root_lamports,
        );
        let context = direct_begin_retiring_context_v1(
            release_set,
            market.to_bytes(),
            root.to_bytes(),
            manifest_digest,
            release.program_set_id,
            config_digest,
            generation,
            0,
        );
        let request = DirectBeginRetiringRequestV1 {
            release_set,
            market: market.to_bytes(),
            context,
            root: root.to_bytes(),
            manifest: manifest_digest,
            program_set: release.program_set_id,
            config: config_digest,
            expected_market_digest: hash(&market_data).to_bytes(),
            expected_root_digest: hash(&root_data).to_bytes(),
            generation,
            entry_index: 0,
        }
        .to_bytes()
        .expect("begin-retiring request")
        .to_vec();
        let mut expected_post_root = header.to_bytes().to_vec();
        expected_post_root.extend_from_slice(
            &DirectRootStateV1::new()
                .begin_retiring()
                .expect("Retiring root")
                .encode(),
        );
        let expected_post_digest = hash(&expected_post_root).to_bytes();
        let accounts = vec![
            AccountMeta::new(root, false),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new_readonly(manifest_raw, false),
            AccountMeta::new_readonly(program_set_raw, false),
            AccountMeta::new_readonly(program_set_staging, false),
            AccountMeta::new_readonly(descriptor_raw, false),
            AccountMeta::new_readonly(descriptor_staging, false),
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
        ];
        assert_eq!(accounts.len(), 20);
        routes.push(BeginRetiringRoute {
            seed,
            instruction: Instruction {
                program_id: TRADING_PROGRAM_ID,
                accounts,
                data: request.clone(),
            },
            root,
            request,
            expected_post_root,
            expected_post_digest,
            root_lamports,
        });
    }
    (test, routes, hash(&artifacts.trading).to_bytes())
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
}

/// The exact custom refusal a submission produced, or `None` if it succeeded.
///
/// Every hostile case in this file asserted only `expect_err` until
/// 2026-08-30, which is the assertion shape `67e96e5b` caught passing on the
/// wrong refusal in `open_market_program_test`. Naming the code is what makes
/// a hostility about its own conjunct rather than about whatever the program
/// happened to reach first.
fn refusal(result: Result<(), BanksClientError>) -> Option<u32> {
    match result {
        Ok(()) => None,
        Err(BanksClientError::TransactionError(error)) => transaction_refusal(Err(error)),
        Err(other) => panic!("expected a program refusal, got {other:?}"),
    }
}

/// The same reading, for the metadata-carrying submission path.
fn transaction_refusal(result: Result<(), TransactionError>) -> Option<u32> {
    match result {
        Ok(()) => None,
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => Some(code),
        Err(other) => panic!("expected a program refusal, got {other:?}"),
    }
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
    let dependency_before = account(&mut context, fixture.dependency_funding)
        .await
        .expect("Resolution dependency ledger");
    assert_eq!(dependency_before.data, fixture.dependency_funding_data);
    assert_eq!(
        dependency_before.lamports,
        fixture.dependency_funding_lamports
    );
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
    assert_eq!(
        account(&mut context, fixture.dependency_funding)
            .await
            .expect("preserved Resolution dependency ledger"),
        dependency_before
    );
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

/// Retirement's stage four on the shape a REAL market has.
///
/// `DirectCloseCapability` takes `outstanding_capabilities` to zero on a market
/// whose selected Direct entry is manifest index **0** -- devnet `GyD95eyE…`'s
/// position -- so the funding slice is Trading `0x0001` first and written,
/// Resolution `0x000e` second and preserved. That is the mirror image of the
/// fixture above, and it is the order `terminal_retirement_v1` used to have
/// written down as two fields named after controllers.
///
/// The point is not that a second order works. It is that the SAME projection
/// produces both, and the market this file could actually retire is the one it
/// was not testing.
#[tokio::test]
async fn the_high_selector_closes_a_market_whose_selected_entry_is_index_zero() {
    let (test, fixture) = build_fixture_for(CloseShape::SelectedFirst, Fault::None);
    let mut context = test.start_with_context().await;
    let credit_before = account(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit");
    let dependency_before = account(&mut context, fixture.dependency_funding)
        .await
        .expect("Resolution dependency ledger");
    assert_eq!(dependency_before.data, fixture.dependency_funding_data);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                fixture.instruction.clone(),
            ],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context
                .banks_client
                .get_latest_blockhash()
                .await
                .expect("blockhash"),
        ))
        .await
        .expect("Banks RPC");
    assert!(
        processed.result.is_ok(),
        "the index-0 close refused: {:?}",
        processed.result
    );
    println!(
        "direct close capability, selected entry index 0: {} CU top level",
        processed
            .metadata
            .expect("transaction metadata")
            .compute_units_consumed
    );
    for closed in [fixture.root, fixture.funding] {
        if let Some(account) = account(&mut context, closed).await {
            assert_eq!(account.lamports, 0);
            assert_eq!(account.owner, system_program::ID);
            assert!(account.data.is_empty());
        }
    }
    let market = account(&mut context, fixture.market).await.expect("Market");
    assert_eq!(
        CoreState::decode(&market.data)
            .expect("Core poststate")
            .outstanding_capabilities,
        0
    );
    assert_eq!(
        account(&mut context, fixture.dependency_funding)
            .await
            .expect("preserved Resolution dependency ledger"),
        dependency_before,
        "the close covers the preserved compartments and moves none of them",
    );
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
    // A dropped alias, a shifted one and a spurious one are all frame
    // geometry, and Core refuses them at `require_authenticated_suffix_aliases`
    // before it reads a byte of state -- 8.4k to 18.4k compute units.
    // Substituting BOTH halves of a pair keeps the geometry exact, so that one
    // survives the frame check and is refused where it actually differs: the
    // Registry activation cache no longer authenticates the release.
    for (fault, expected) in [
        (Fault::MissingAlias, CORE_ACCOUNT_FRAME),
        (Fault::ShiftedAlias, CORE_ACCOUNT_FRAME),
        (Fault::PairSubstitution, CORE_RELEASE),
        (Fault::ExtraAlias, CORE_ACCOUNT_FRAME),
    ] {
        let (test, fixture) = build_fixture(fault);
        let mut context = test.start_with_context().await;
        let before = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        assert_eq!(
            refusal(submit(&mut context, fixture.instruction).await),
            Some(expected),
            "{fault:?} must refuse with its own code"
        );
        let after = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        assert_eq!(after, before);
    }
}

#[tokio::test]
async fn dependency_writable_reordered_substituted_and_mutated_refuse_with_rollback() {
    // None of these four is a frame refusal. The physical funding slice is
    // width- and privilege-checked by the FundingLedger validator, not by the
    // alias geometry, so a writable, reordered, substituted or byte-mutated
    // dependency ledger all arrive at `CoreSbfError::Funding` -- and they do
    // so at four different depths (110k to 228k compute units), which is the
    // evidence that they are four hostilities rather than one.
    for (fault, expected) in [
        (Fault::DependencyWritable, CORE_FUNDING),
        (Fault::DependencyReordered, CORE_FUNDING),
        (Fault::DependencySubstitution, CORE_FUNDING),
        (Fault::DependencyMutated, CORE_FUNDING),
    ] {
        let (test, fixture) = build_fixture(fault);
        let mut context = test.start_with_context().await;
        let before = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.dependency_funding).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        assert_eq!(
            refusal(submit(&mut context, fixture.instruction).await),
            Some(expected),
            "{fault:?} must refuse with its own code"
        );
        let after = [
            account(&mut context, fixture.market).await,
            account(&mut context, fixture.root).await,
            account(&mut context, fixture.dependency_funding).await,
            account(&mut context, fixture.funding).await,
            account(&mut context, fixture.rent_credit).await,
        ];
        assert_eq!(after, before);
    }
}

fn hex32(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("hex formatting");
    }
    output
}

#[tokio::test]
async fn begin_direct_retiring_m61_twenty_seed_real_sbf_campaign() {
    use std::fmt::Write as _;

    let (test, routes, trading_elf_digest) = build_begin_retiring_campaign();
    assert_eq!(routes.len(), 20);
    let mut context = test.start_with_context().await;
    let mut units = Vec::with_capacity(routes.len());
    let mut evidence = String::from("seed\tgeneration\tcompute_units\troot\tpost_digest\n");

    for route in &routes {
        let pre = account(&mut context, route.root)
            .await
            .expect("Open Direct root");
        assert_eq!(pre.lamports, route.root_lamports);
        let blockhash = context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("blockhash");
        let transaction = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                route.instruction.clone(),
            ],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
        );
        let processed = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await
            .expect("Banks RPC");
        assert!(
            processed.result.is_ok(),
            "seed {} refused: {:?}",
            route.seed,
            processed.result
        );
        let metadata = processed.metadata.expect("transaction metadata");
        let returned = metadata.return_data.expect("begin-retiring receipt");
        assert_eq!(returned.program_id, TRADING_PROGRAM_ID);
        let receipt = DirectBeginRetiringReceiptV1::decode(&returned.data)
            .expect("receipt wire")
            .authenticate_for_request(
                &route.request,
                route.expected_post_digest,
                TRADING_PROGRAM_ID.to_bytes(),
            )
            .expect("receipt/request/poststate join");
        assert_eq!(receipt.post_root_digest, route.expected_post_digest);
        let post = account(&mut context, route.root)
            .await
            .expect("Retiring Direct root");
        assert_eq!(post.owner, TRADING_PROGRAM_ID);
        assert_eq!(post.lamports, route.root_lamports);
        assert_eq!(post.data, route.expected_post_root);
        units.push(metadata.compute_units_consumed);
        writeln!(
            &mut evidence,
            "{}\t{}\t{}\t{}\t{}",
            route.seed,
            100_u64 + u64::from(route.seed),
            metadata.compute_units_consumed,
            route.root,
            hex32(route.expected_post_digest),
        )
        .expect("evidence row");
    }

    let pass_count = units.len();
    let sum = units
        .iter()
        .try_fold(0_u64, |total, value| total.checked_add(*value))
        .expect("CU sum");
    let mean = sum / u64::try_from(pass_count).expect("pass count");
    let minimum = units.iter().copied().min().expect("minimum CU");
    let maximum = units.iter().copied().max().expect("maximum CU");
    assert_eq!(pass_count, 20);
    println!(
        "begin-direct-retiring M-61 pass={pass_count}/20 mean={mean} min={minimum} max={maximum} trading_elf_sha256={}",
        hex32(trading_elf_digest),
    );
    writeln!(
        &mut evidence,
        "summary\tpass={pass_count}/20\tmean={mean}\tmin={minimum}\tmax={maximum}\ttrading_elf_sha256={}",
        hex32(trading_elf_digest),
    )
    .expect("evidence summary");
    if let Ok(directory) = env::var("DCLUTCH_RETIRE_CANDIDATE_DIR") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).expect("candidate directory");
        fs::write(directory.join("m61.tsv"), evidence).expect("candidate M-61 evidence");
    }

    // The same exact request now names the old Open preimage. Replay must
    // refuse without changing one byte or lamport of the Retiring root.
    let replay = routes.first().expect("first route");
    let before = account(&mut context, replay.root)
        .await
        .expect("Retiring root before replay");
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("replay blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            replay.instruction.clone(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("replay Banks RPC");
    // The refusal must be Trading's, about the root it names: the request
    // still carries the Open preimage that the first pass consumed, so the
    // immutable child root no longer matches. `is_err()` alone would also
    // have accepted a Core frame refusal, which would mean this replay never
    // reached the statement it is here to exercise.
    assert_eq!(
        transaction_refusal(processed.result),
        Some(TRADING_ROOT),
        "replay must refuse at the Trading root"
    );
    let after = account(&mut context, replay.root)
        .await
        .expect("Retiring root after replay");
    assert_eq!(after, before);
}

/// The exact activation frame Core parses, and the frame the successor
/// bootstrap's `devnet-direct-capability-activation-v1` driver builds
/// (`tools/local-validator/bootstrap/successor/src/direct_capability_activation.rs`):
/// five record and Market accounts, an `F=1` physical funding slice, eleven
/// Core-owned fixed accounts, and an eighteen-account Direct child tail.
///
/// It is one account narrower on both ends than the close frame: activation
/// carries only the Trading ledger it is about, and its child tail has no
/// Rent program and no rent credit, because a root being CREATED has no rent
/// to refund.
fn activation_layout(funding_count: u8) -> CapabilityRouteLayoutV1 {
    CapabilityRouteLayoutV1::new(funding_count, 18).expect("activation route layout")
}

/// `TradingSbfError::ActivationLedgerCount`, derived and never typed.
const TRADING_ACTIVATION_LEDGER_COUNT: u32 =
    dclutch_trading_sbf::TradingSbfError::ActivationLedgerCount as u32;

/// Which market shape an activation fixture founds under.
///
/// The two-ledger shapes are the REAL market's: devnet `GyD95eyE…` founded on
/// 2026-09-05 with the selected Direct entry at manifest index 0, dependency
/// edges `[1, 2, 3]`, and the two funding ledgers `0x0001` (Trading, selected)
/// and `0x000e` (Resolution, preserved) its closure requires. The zero-edge
/// shape is every cohort before it, and is here as the control: the same
/// release, the same artifacts, the same bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationShape {
    /// One entry, no edges, one ledger.
    ZeroEdge,
    /// Four entries, selected at index 0, two ledgers.
    TwoLedger,
    /// The two-ledger market with the dependency ledger withheld.
    MissingDependencyLedger,
    /// The two-ledger market with a third ledger overlapping the dependency's
    /// rows, which is "more ledgers than the closure has".
    OverlappingDependencyLedger,
}

impl ActivationShape {
    /// Manifest entries this shape founds under.
    const fn entry_count(self) -> u16 {
        match self {
            Self::ZeroEdge => 1,
            _ => 4,
        }
    }

    /// The selected entry's own manifest index.
    const fn selected_entry_index(self) -> u16 {
        0
    }

    /// Whether the market seats a foreign dependency ledger at all.
    const fn has_dependency_ledger(self) -> bool {
        !matches!(self, Self::ZeroEdge)
    }

    /// Physical ledgers the instruction PRESENTS, which is what the funding
    /// header declares and what the hostiles move.
    const fn presented_ledgers(self) -> u8 {
        match self {
            Self::ZeroEdge | Self::MissingDependencyLedger => 1,
            Self::TwoLedger => 2,
            Self::OverlappingDependencyLedger => 3,
        }
    }
}

struct ActivationFixture {
    instruction: Instruction,
    market: Pubkey,
    root: Pubkey,
    funding: Pubkey,
    root_lamports: u64,
    funding_lamports: u64,
    expected_root: Vec<u8>,
    manifest: Vec<u8>,
    manifest_id: ContentId,
    selected_entry_index: u16,
    dependency_funding: Option<Pubkey>,
    dependency_funding_data: Vec<u8>,
    dependency_funding_lamports: u64,
}

/// The founding of a Direct capability root, on real Core and Trading ELFs.
///
/// The close fixture above PLANTS the root it closes, which is the only reason
/// it can start. This one starts where a real market starts: the root
/// coordinate is vacant, its rent is parked in a Pending Trading funding
/// ledger, and the only thing that can create it is Core's `ActivateCapability`
/// CPI into Trading's `process_activation`.
///
/// Every coordinate is derived the way the devnet driver derives it, so the two
/// cannot drift: the selection from the manifest entry, the root from the
/// selection's own header seeds, the closure mask from the manifest, and the
/// role request as selection bytes, `CapabilityFundingHeaderV2::new(1, 1, mask)`
/// and `direct_activation_request_v1()` concatenated.
fn build_activation_fixture(shape: ActivationShape) -> (ProgramTest, ActivationFixture) {
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
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(ordinary, CAPACITY_PROFILE)
        .expect("canonical ordinary/lifecycle release");
    // The activation request selects its own entry out of that set, and the
    // selector is read from the request rather than typed here.
    assert_eq!(
        u32::from_le_bytes(
            direct_activation_request_v1()[12..16]
                .try_into()
                .expect("selector")
        ),
        DIRECT_ACTIVATION_SELECTOR_V1
    );
    let ordinary_descriptor =
        CapabilityProgramV4::decode(&release.ordinary.descriptor).expect("ordinary descriptor");
    let config = DirectExecutionConfigV1::new(100, 0, [0x55; 32])
        .expect("Direct config")
        .encode();
    let config_digest = hash(&config).to_bytes();

    let root_space = CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1;
    let root_lamports = Rent::default().minimum_balance(root_space);
    // Exactly the rent of the account this row owns, which is how
    // `selected_manifest_entry_v1` quotes it in production. A quote below the
    // root's own exemption cannot found it at all.
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(root_lamports).expect("native rent quote"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding amounts");
    // The selected entry names EVERY other manifest index, which is the edge
    // set a Direct founding derives and the only one whose closure the close
    // frame's two ledger masks can partition.
    let mut direct_dependencies = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
    let dependency_count = u8::try_from(shape.entry_count() - 1).expect("dependency count");
    for position in 0..usize::from(dependency_count) {
        direct_dependencies[position] = u8::try_from(position + 1).expect("dependency index");
    }
    let direct_entry = CapabilityEntryV1::new(
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
        content(ordinary_descriptor.capacity_profile().to_bytes()),
        content(ordinary_descriptor.root_schema().to_bytes()),
        content(ordinary_descriptor.derivation_policy().to_bytes()),
        ActivationPolicy::PrepaidLazy,
        u64::MAX,
        dependency_count,
        direct_dependencies,
        FundingQuoteV1::new(amounts, None).expect("funding quote"),
    )
    .expect("Direct manifest entry");
    // Manifest order is kind-digest order, and the real market's selected entry
    // sits FIRST. The companions are given kinds above the Direct kind so this
    // fixture reproduces that position rather than the four-entry fixture's
    // index 3 -- and the assertion is what says the digest did not land above
    // them, rather than a comment hoping it did not.
    let companion_kinds = [0xfd_u8, 0xfe, 0xff];
    assert!(
        ordinary_descriptor.kind().to_bytes() < [companion_kinds[0]; 32],
        "the Direct kind must sort below the companions for the selected entry to be index 0",
    );
    let mut entries = vec![direct_entry];
    if shape.entry_count() > 1 {
        for kind in companion_kinds {
            entries.push(
                CapabilityEntryV1::new(
                    content([kind; 32]),
                    content([kind.wrapping_sub(0x10); 32]),
                    content([kind.wrapping_sub(0x20); 32]),
                    content([kind.wrapping_sub(0x30); 32]),
                    content([kind.wrapping_sub(0x40); 32]),
                    content([kind.wrapping_sub(0x50); 32]),
                    ActivationPolicy::RequiredAtFounding,
                    0,
                    0,
                    [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                    FundingQuoteV1::new(amounts, None).expect("dependency funding quote"),
                )
                .expect("Resolution dependency entry"),
            );
        }
    }
    let mut manifest = vec![0; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest).expect("manifest");
    let manifest_digest = hash(&manifest).to_bytes();
    let manifest_id = content(manifest_digest);
    let decoded_manifest = CapabilityManifestV1::decode(&manifest).expect("decoded manifest");
    let selected_entry_index = shape.selected_entry_index();
    // Discovered, not chosen: the closure is what the funding header must
    // declare and what the presented ledgers must partition.
    let closure_mask =
        capability_dependency_closure_mask_v1(decoded_manifest, selected_entry_index)
            .expect("dependency closure");
    assert_eq!(
        closure_mask,
        match shape.entry_count() {
            1 => 0b0001,
            _ => 0b1111,
        }
    );
    let selected_bit = 1_u16 << selected_entry_index;
    let dependency_mask = closure_mask & !selected_bit;

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
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
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
        direct_activation_account_profile_schema_v1(),
        release.activation.account_profile.clone(),
    );
    let (effect_raw, effect_staging, _, _) = add_record(
        &mut test,
        direct_activation_effect_schema_v1(),
        release.activation.effect.clone(),
    );
    let (descriptor_raw, descriptor_staging, _, _) = add_record(
        &mut test,
        direct_activation_descriptor_schema_v1(),
        release.activation.descriptor.clone(),
    );

    let wire_selection = CapabilityExecutionSelectionV1::new(
        selected_entry_index,
        manifest_id,
        content(ordinary_descriptor.kind().to_bytes()),
        content(release.program_set_id),
        content(config_digest),
    )
    .expect("selection");
    let persisted_selection =
        wire_selection.with_capability_release_record_bumps(release_raw_bump, release_staging_bump);

    let mut state = CoreState {
        phase: Phase::Open,
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
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity([0x75; 32]),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    state.identity.market_id = identity(market.to_bytes());
    let state_bytes = state.encode().expect("Core state");
    add_account(&mut test, market, CORE_PROGRAM_ID, state_bytes.to_vec());

    // The root coordinate, derived exactly as `direct_execution_root_v1`
    // derives it. NOT added to the bank: this is what the route creates.
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
    let mut expected_root = root_header.to_bytes().to_vec();
    expected_root.extend_from_slice(&DirectRootStateV1::new().encode());

    // Pending, and holding the root's rent as well as its own: the founding
    // moves the parked quote into the account it creates.
    let mut funding_data = vec![0; funding_ledger_bytes_v2(1).expect("funding width")];
    let funding_rate = funded_rent_rate(funding_data.len());
    FundingLedgerV2::initialize(
        &mut funding_data,
        manifest_id,
        decoded_manifest,
        selected_bit,
        funding_rate,
    )
    .expect("funding initialize");
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
        funding_lamports + root_lamports,
    );

    // The foreign controller's ledger: every dependency row, already Active,
    // Resolution-owned, and read-only in the frame. The activation must not
    // move one byte or lamport of it.
    let mut dependency_funding_data = Vec::new();
    let mut dependency_funding_lamports = 0_u64;
    let dependency_funding = if shape.has_dependency_ledger() {
        let slot_count = u16::try_from(dependency_mask.count_ones()).expect("dependency rows");
        dependency_funding_data = vec![0; funding_ledger_bytes_v2(slot_count).expect("width")];
        let rate = funded_rent_rate(dependency_funding_data.len());
        FundingLedgerV2::initialize(
            &mut dependency_funding_data,
            manifest_id,
            decoded_manifest,
            dependency_mask,
            rate,
        )
        .expect("dependency funding initialize");
        for entry_index in 0..decoded_manifest.entry_count() {
            if dependency_mask & (1_u16 << entry_index) == 0 {
                continue;
            }
            FundingLedgerV2::activate_in_place(
                &mut dependency_funding_data,
                manifest_id,
                decoded_manifest,
                entry_index,
                u64::from(entry_index) + 1,
            )
            .expect("dependency funding activate");
        }
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            RESOLUTION_PROGRAM_ID.to_bytes(),
            market.to_bytes(),
            GENERATION,
            manifest_id,
            FundingLedgerV2::decode(&dependency_funding_data).expect("dependency ledger"),
        )
        .expect("dependency funding derivation");
        let key =
            Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0;
        dependency_funding_lamports =
            Rent::default().minimum_balance(dependency_funding_data.len());
        add_account_with_lamports(
            &mut test,
            key,
            RESOLUTION_PROGRAM_ID,
            dependency_funding_data.clone(),
            dependency_funding_lamports,
        );
        Some(key)
    } else {
        None
    };

    // "More ledgers than the closure has": a real, distinct, Resolution-owned
    // ledger whose rows the dependency ledger already covers.
    let overlapping_funding = if shape == ActivationShape::OverlappingDependencyLedger {
        let overlap_mask = 1_u16 << 1;
        let mut bytes = vec![0; funding_ledger_bytes_v2(1).expect("overlap width")];
        let rate = funded_rent_rate(bytes.len());
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            decoded_manifest,
            overlap_mask,
            rate,
        )
        .expect("overlap funding initialize");
        FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, decoded_manifest, 1, 1)
            .expect("overlap funding activate");
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            RESOLUTION_PROGRAM_ID.to_bytes(),
            market.to_bytes(),
            GENERATION,
            manifest_id,
            FundingLedgerV2::decode(&bytes).expect("overlap ledger"),
        )
        .expect("overlap funding derivation");
        let key =
            Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0;
        let lamports = Rent::default().minimum_balance(bytes.len());
        add_account_with_lamports(&mut test, key, RESOLUTION_PROGRAM_ID, bytes, lamports);
        Some(key)
    } else {
        None
    };

    let family_request = direct_activation_request_v1();
    let funding_header = CapabilityFundingHeaderV2::new(
        shape.presented_ledgers(),
        u8::try_from(closure_mask.count_ones()).expect("logical count"),
        closure_mask,
    )
    .expect("funding header");
    let mut role_request = wire_selection.to_bytes().to_vec();
    role_request.extend_from_slice(&funding_header.encode());
    role_request.extend_from_slice(&family_request);
    let role_digest = hash(&role_request).to_bytes();
    let context = [0x82; 32];
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
        CoreEffectActionV1::ActivateCapability,
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
        Action::ActivateCapability,
        GENERATION,
        identity(market.to_bytes()),
    );
    let mut data = request.encode().expect("Core request").to_vec();
    data.extend_from_slice(&envelope.encode().expect("Core envelope bytes"));
    data.extend_from_slice(&role_request);

    let mut accounts = vec![
        AccountMeta::new(market, false),
        AccountMeta::new_readonly(realm_raw, false),
        AccountMeta::new_readonly(realm_staging, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(manifest_staging, false),
    ];
    // The physical funding slice, in the order
    // `validate_funding_ledger_masks_v2` requires: by each ledger's lowest
    // selected manifest index. The selected entry is index 0 here, so the
    // Trading ledger LEADS -- which is the real market's order and the reverse
    // of the four-entry close fixture's.
    accounts.push(AccountMeta::new(funding, false));
    if let Some(extra) = overlapping_funding {
        accounts.push(AccountMeta::new_readonly(extra, false));
    }
    if shape != ActivationShape::MissingDependencyLedger {
        if let Some(dependency) = dependency_funding {
            accounts.push(AccountMeta::new_readonly(dependency, false));
        }
    }
    accounts.extend([
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
    ]);
    assert_eq!(
        accounts.len(),
        activation_layout(shape.presented_ledgers()).account_count()
    );
    (
        test,
        ActivationFixture {
            instruction: Instruction {
                program_id: CORE_PROGRAM_ID,
                accounts,
                data,
            },
            market,
            root,
            funding,
            root_lamports,
            funding_lamports,
            expected_root,
            manifest,
            manifest_id,
            selected_entry_index,
            dependency_funding,
            dependency_funding_data,
            dependency_funding_lamports,
        },
    )
}

/// C-04's founding clause: the Direct capability root is CREATED by Trading
/// under its own authority, through Core's `ActivateCapability`, on real ELFs.
///
/// Until this test, nothing offline created one. The devnet/loopback driver
/// (`direct_capability_activation.rs`) is the only other frame that reaches
/// `process_activation`, and it needs a cluster; the Trading program-test that
/// exercises the outer drives it from `dclutch_trading_core_caller_test_program`
/// with a Registry stub, so neither Core's capability route nor the real
/// Registry activation was ever in front of it.
#[tokio::test]
async fn canonical_activation_creates_the_direct_root_through_real_core_and_trading() {
    let (test, fixture) = build_activation_fixture(ActivationShape::ZeroEdge);
    let mut context = test.start_with_context().await;
    assert!(
        account(&mut context, fixture.root).await.is_none(),
        "the root coordinate must be vacant before the founding",
    );
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            fixture.instruction.clone(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(
        processed.result.is_ok(),
        "the founding refused: {:?}",
        processed.result
    );
    let units = processed
        .metadata
        .expect("transaction metadata")
        .compute_units_consumed;
    println!("direct capability activation: {units} CU top level");

    let root = account(&mut context, fixture.root)
        .await
        .expect("the founding created the root");
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert_eq!(root.data, fixture.expected_root);
    assert_eq!(root.lamports, fixture.root_lamports);

    let funding = account(&mut context, fixture.funding)
        .await
        .expect("the ledger survives its own spend");
    assert_eq!(funding.lamports, fixture.funding_lamports);
    let manifest = CapabilityManifestV1::decode(&fixture.manifest).expect("manifest");
    let authenticated = FundingLedgerV2::decode(&funding.data)
        .expect("funding poststate")
        .authenticate(fixture.manifest_id, manifest)
        .expect("authenticated funding poststate");
    let slot = authenticated
        .slot(fixture.selected_entry_index)
        .expect("selected slot");
    assert_eq!(slot.status(), FundingLedgerStatusV2::Active);
    assert!(slot.activation_slot() > 0);
    assert_eq!(slot.remaining().rent().amount(), 0);

    let market = account(&mut context, fixture.market)
        .await
        .expect("Market state");
    assert_eq!(
        CoreState::decode(&market.data)
            .expect("Core poststate")
            .outstanding_capabilities,
        1,
    );

    // A founding is once. The same exact instruction replayed must refuse
    // without moving one byte or lamport of the root it already created --
    // and it must refuse at TRADING, about the root, rather than anywhere
    // upstream: a Core-frame refusal would mean the replay never reached the
    // statement this case is about.
    context.warp_to_slot(64).expect("replay slot");
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("replay blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            fixture.instruction.clone(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let replayed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("replay Banks RPC");
    assert_eq!(
        transaction_refusal(replayed.result),
        Some(REPLAY_REFUSAL),
        "the replayed founding must refuse at its own conjunct",
    );
    let after = account(&mut context, fixture.root)
        .await
        .expect("the root after the replay");
    assert_eq!(after.data, fixture.expected_root);
    assert_eq!(after.lamports, fixture.root_lamports);
}

/// The wall cohort-16 met, on real ELFs: a market whose selected entry names
/// its dependency edges, funded by the two ledgers its closure requires.
///
/// This is devnet market `GyD95eyE…`'s shape. At the deployed release it
/// refused `Content 0x4003` at 108,180 CU, and the reading at the time was that
/// the Direct activation bundle had to declare three accounts. It cannot:
/// `AccountProfileV1` refuses `UnanchoredAccount` for a rule no requirement
/// operation names, and no seam-seeded identity names a foreign controller. So
/// the interpreted frame is the root and the SELECTED ledger, exactly as the
/// native close already built it, and the dependency ledger is authenticated
/// outside it. The artifacts are byte-identical to the zero-edge release's --
/// the release id, the manifest entry and the Market address do not move.
#[tokio::test]
async fn canonical_activation_admits_the_selected_entrys_two_ledger_closure() {
    let (test, fixture) = build_activation_fixture(ActivationShape::TwoLedger);
    let dependency = fixture
        .dependency_funding
        .expect("the two-ledger shape seats a dependency ledger");
    let mut context = test.start_with_context().await;
    assert!(
        account(&mut context, fixture.root).await.is_none(),
        "the root coordinate must be vacant before the founding",
    );
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            fixture.instruction.clone(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(
        processed.result.is_ok(),
        "the two-ledger founding refused: {:?}",
        processed.result
    );
    let units = processed
        .metadata
        .expect("transaction metadata")
        .compute_units_consumed;
    println!("direct capability activation, F=2: {units} CU top level");

    let root = account(&mut context, fixture.root)
        .await
        .expect("the founding created the root");
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert_eq!(root.data, fixture.expected_root);
    assert_eq!(root.lamports, fixture.root_lamports);

    let funding = account(&mut context, fixture.funding)
        .await
        .expect("the selected ledger survives its own spend");
    assert_eq!(funding.lamports, fixture.funding_lamports);
    let manifest = CapabilityManifestV1::decode(&fixture.manifest).expect("manifest");
    let slot = FundingLedgerV2::decode(&funding.data)
        .expect("funding poststate")
        .authenticate(fixture.manifest_id, manifest)
        .expect("authenticated funding poststate")
        .slot(fixture.selected_entry_index)
        .expect("selected slot");
    assert_eq!(slot.status(), FundingLedgerStatusV2::Active);
    assert!(slot.activation_slot() > 0);
    assert_eq!(slot.remaining().rent().amount(), 0);

    // The whole point of admitting it: it is authenticated and it is UNTOUCHED.
    let preserved = account(&mut context, dependency)
        .await
        .expect("the dependency ledger");
    assert_eq!(preserved.data, fixture.dependency_funding_data);
    assert_eq!(preserved.lamports, fixture.dependency_funding_lamports);
    assert_eq!(preserved.owner, RESOLUTION_PROGRAM_ID);

    let market = account(&mut context, fixture.market)
        .await
        .expect("Market state");
    assert_eq!(
        CoreState::decode(&market.data)
            .expect("Core poststate")
            .outstanding_capabilities,
        1,
    );
}

/// A ledger SET that is not the selected entry's closure refuses by name, at
/// CORE, before the CPI.
///
/// Both directions of the one accusation: a dependency ledger withheld, and one
/// presented twice. Both are `CoreSbfError::Funding`, which is where the
/// partition is owned -- `capability.rs`'s `validate_funding_header` requires
/// the header's mask to EQUAL the selected entry's dependency closure and
/// `validate_funding_ledger_masks_v2` requires the presented ledgers to
/// partition it.
///
/// MEASURED, and it is why `TradingSbfError::ActivationLedgerCount` is
/// documented as unreached: Trading restates the same partition for a caller it
/// does not trust, and a Core-routed frame cannot get past Core to exercise it.
/// Naming Core's code here rather than Trading's is the difference between a
/// test that says what happened and one that assumes.
#[tokio::test]
async fn an_activation_whose_ledgers_are_not_the_closure_refuses_by_name() {
    for shape in [
        ActivationShape::MissingDependencyLedger,
        ActivationShape::OverlappingDependencyLedger,
    ] {
        let (test, fixture) = build_activation_fixture(shape);
        let mut context = test.start_with_context().await;
        let blockhash = context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("blockhash");
        let transaction = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                fixture.instruction.clone(),
            ],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
        );
        let refused = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await
            .expect("Banks RPC");
        assert_eq!(
            transaction_refusal(refused.result),
            Some(CORE_FUNDING),
            "{shape:?} must refuse at the funding-partition conjunct",
        );
        assert_ne!(
            CORE_FUNDING, TRADING_ACTIVATION_LEDGER_COUNT,
            "the two sides of the CPI own separate discriminants",
        );
        assert!(
            account(&mut context, fixture.root).await.is_none(),
            "{shape:?} must not have created a root",
        );
    }
}
