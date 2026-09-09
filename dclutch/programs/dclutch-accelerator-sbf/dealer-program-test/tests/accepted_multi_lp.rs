//! Current real-ELF acceptance for the retained Dealer multi-LP capability.
//!
//! This campaign uses only the current eight-selector Dealer SetV2 and the
//! family-neutral Hot outer.  It opens two independently funded LP positions,
//! contributes and withdraws all capital through selector P0, then closes both
//! positions.  The hostile control substitutes a late LP evidence account and
//! proves transaction-wide rollback by exact refusal and exact poststate.

use std::{env, vec::Vec};

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
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketInputV2, LiabilityBasisMarketViewV2,
    LiabilityBasisPositionViewV2, encode_liability_basis_market_into_v2,
    put_liability_basis_market_bump_v2, put_liability_basis_position_bump_v2,
};
use dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::ACCOUNT_BYTES as TOKEN_ACCOUNT_BYTES;
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1,
    DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_fractional_atomic_program_test::narrow_fixture::{
    NarrowFixtureInputV2, NarrowFixtureV2, NarrowPositionV2, compile_narrow_fixture_v2,
};
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityManifestV1, CompartmentFundingV1, ContentId as ManifestContentId,
    FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    set_v1::CapabilityProgramSetV1,
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, CapabilityProgramSetV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_V4,
    },
};
use dclutch_market::execution_strategy::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_OUTPUT_PAGE_ACK_SCHEMA_ID_V3,
    ACCELERATOR_OUTPUT_PAGE_REQUEST_SCHEMA_ID_V3, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
    StrategyDispositionV2,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2,
        LifecycleRentCreditV2,
    },
};
use dclutch_market::{CoreState, Identity as CoreIdentity};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV2,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_trading::{
    dealer::{
        Phase,
        config_v4::DealerConfigV4,
        root_tail::{ROOT_TAIL_BYTES, RootTail},
    },
    dealer_scenario::ClaimsInventoryObservation,
};
use dclutch_trading_sbf::{
    TradingSbfError,
    dealer::{
        equity::{
            PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3, PoolEquityPlanV3,
            PoolEquityRedemptionV3, preflight_pool_equity_v3,
        },
        equity_accelerator::DealerEquityAcceleratorErrorV4,
        equity_artifacts::{
            dealer_equity_request_profile_bytes_v3, dealer_equity_transition_bytes_v3,
            encode_dealer_equity_request_profile_v3, encode_dealer_equity_transition_v3,
        },
        equity_effect::{
            dealer_equity_evidence_owner_identity_register_v3, dealer_equity_identity_count_v3,
            dealer_equity_scalar_count_v3, project_dealer_equity_hot_registers_v3,
        },
        equity_profile::{
            DealerEquityAccountProfileInputV3, dealer_equity_logical_account_count_v3,
            encode_dealer_equity_account_profile_v3,
        },
        equity_release::{
            DEALER_EQUITY_LIFECYCLE_BYTES_V5, DealerEquityFinalizedArtifactsV4,
            dealer_equity_effect_bytes_v4, encode_dealer_equity_effect_v4,
            encode_dealer_equity_lifecycle_v5, finalize_dealer_equity_descriptor_v4,
        },
        equity_request::{
            DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3, DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3,
            DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3, DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
            DEALER_EQUITY_REDEEM_P1_SELECTOR_V3, DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
            DEALER_EQUITY_SELECTOR_OFFSET_V3, DealerEquityBumpSeedsV3, DealerEquityRequestV3,
            EquityPoolChainProjectionV3, EquityRequestIntentV3, build_equity_request_v3,
            mine_dealer_equity_bumps_v3, prepare_equity_request_v3,
        },
        lp_artifacts::{
            DEALER_LP_OBLIGATION_ACCOUNT_V3, DealerLpAccountProfileInputV3,
            dealer_lp_account_count_v3, dealer_lp_transition_bytes_v3,
            encode_dealer_lp_account_profile_v3, encode_dealer_lp_request_profile_v3,
            encode_dealer_lp_transition_v3,
        },
        lp_release::{
            DEALER_LP_LIFECYCLE_BYTES_V5, DealerLpFinalizedArtifactsV4, dealer_lp_effect_bytes_v4,
            encode_dealer_lp_effect_v4, encode_dealer_lp_lifecycle_v5,
            finalize_dealer_lp_descriptor_v4,
        },
        lp_request::{MultiLpChainProjectionV3, MultiLpRequestActionV3},
        lp_set_request::{build_close_lp_v4, build_open_lp_v4},
        multi_lp::{
            DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpPositionV3,
            MAX_MULTI_LP_CUSTODY_EFFECTS_V3, MultiLpActionV3, MultiLpBumpHintsV3,
            MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpCustodyRequestV3,
        },
        obligation::{
            DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
            DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
            DealerObligationProjectionV3,
        },
    },
};
use dclutch_vm::account_profile::v2::AccountProfileV2;
use dclutch_vm::request_profile::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_V1;
use solana_account::{Account, AccountSharedData};
use solana_clock::Clock;
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent};
use solana_program_option::COption as ProgramCOption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::{Transaction, TransactionError};
use spl_token_interface::state::{Account as SplTokenAccount, AccountState as SplAccountState};

const TRADING: Pubkey = Pubkey::new_from_array([0xd0; 32]);
const CUSTODY_PROGRAM: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const CLAIMS_PROGRAM: Pubkey = Pubkey::new_from_array([0xe6; 32]);
const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0xe7; 32]);
const ACCELERATOR: Pubkey = Pubkey::new_from_array([0xe8; 32]);
const BENEFICIARY: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const WIDTH: u32 = 1;
const FUNDED_COORDINATE: usize = 0;
const GENERATION: u64 = 17;
const POSITION_REVISION: u64 = 3;
const WAIST_SLOT: u64 = 0;

fn elf(name: &str) -> Vec<u8> {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real-ELF evidence");
    std::fs::read(std::path::Path::new(&directory).join(format!("{name}.so")))
        .expect("required real ELF")
}

fn loader_programdata_body(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output[..4].copy_from_slice(&3_u32.to_le_bytes());
    output[4..12].copy_from_slice(&slot.to_le_bytes());
    output[45..].copy_from_slice(elf);
    output
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn artifact_release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV2 {
    dclutch_fractional_atomic_program_test::campaign_support::release(program, semantic, elf)
}

fn activation_input(release: ArtifactReleaseV2) -> ArtifactActivationInputV1 {
    dclutch_fractional_atomic_program_test::campaign_support::activation_input(release)
}

struct ReleaseWaist {
    registry: Pubkey,
    release_set_id: [u8; 32],
    custody_programdata: Pubkey,
    claims_programdata: Pubkey,
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    activation_cache: Pubkey,
    cache_body: Vec<u8>,
    deployments: Vec<(&'static str, Pubkey, Vec<u8>)>,
}

fn release_waist() -> ReleaseWaist {
    let registry = Pubkey::new_from_array([0xe0; 32]);
    let custody_elf = elf("dclutch_custody_sbf");
    let claims_elf = elf("dclutch_claims_sbf");
    let core_elf = elf("dclutch_core_sbf");
    let trading_elf = elf("dclutch_trading_sbf");
    let custody = artifact_release(CUSTODY_PROGRAM, 0xe3, &custody_elf);
    let claims = artifact_release(CLAIMS_PROGRAM, 0xe4, &claims_elf);
    let core = artifact_release(CORE_PROGRAM, 0xe5, &core_elf);
    let trading = artifact_release(TRADING, 0xe6, &trading_elf);
    let bind = |release: ArtifactReleaseV2| {
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
    .expect("release set");
    let release_set_id =
        ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");
    let mut cache_body = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache_body, release_set_id).expect("cache init");
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
        custody_programdata: programdata_address(CUSTODY_PROGRAM),
        claims_programdata: programdata_address(CLAIMS_PROGRAM),
        core_programdata: programdata_address(CORE_PROGRAM),
        trading_programdata: programdata_address(TRADING),
        activation_cache: Pubkey::find_program_address(
            &[
                dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
                &release_set_id.to_bytes(),
            ],
            &registry,
        )
        .0,
        cache_body,
        deployments: vec![
            ("dclutch_custody_sbf", CUSTODY_PROGRAM, custody_elf),
            ("dclutch_claims_sbf", CLAIMS_PROGRAM, claims_elf),
            ("dclutch_core_sbf", CORE_PROGRAM, core_elf),
            ("dclutch_trading_sbf", TRADING, trading_elf),
        ],
    }
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
    bytes[12..16].copy_from_slice(&u32::try_from(values.len()).expect("width").to_le_bytes());
    bytes[16..24].copy_from_slice(&revision.to_le_bytes());
    for (offset, identity) in [
        (24, market),
        (56, product),
        (88, basis),
        (120, owner),
        (152, child),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&identity);
    }
    for (index, value) in values.iter().enumerate() {
        let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[derive(Clone)]
struct LiveClaimsGraph {
    market: Vec<u8>,
    dealer_position: Vec<u8>,
    counterparty_position: Vec<u8>,
    dealer_balances: Vec<u64>,
    counterparty_balances: Vec<u64>,
}

fn live_claims_graph(fixture: &NarrowFixtureV2) -> LiveClaimsGraph {
    let market_view =
        LiabilityBasisMarketViewV2::decode(&fixture.claims_market_bytes).expect("aggregate");
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
            revision: 4,
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
    .expect("aggregate encode");
    put_liability_basis_market_bump_v2(
        &mut market,
        Pubkey::find_program_address(
            &[LIABILITY_BASIS_MARKET_SEED_V2, fixture.core_market.as_ref()],
            &CLAIMS_PROGRAM,
        )
        .1,
    )
    .expect("aggregate bump");
    let observe = |position: &NarrowPositionV2| {
        let view = LiabilityBasisPositionViewV2::decode(&position.bytes).expect("position");
        let balances = (0..fixture.outcome_count)
            .map(|claim| view.balance(&position.bytes, claim).expect("balance"))
            .collect::<Vec<_>>();
        let mut bytes = position.bytes.clone();
        let seeds = ProtocolPositionSeedsV2::new(
            fixture.claims_market.to_bytes(),
            position.owner.to_bytes(),
        )
        .expect("position seeds");
        put_liability_basis_position_bump_v2(
            &mut bytes,
            Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM).1,
        )
        .expect("position bump");
        (bytes, balances)
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

struct Scenario {
    dealer: Keypair,
    fixture: NarrowFixtureV2,
    live: LiveClaimsGraph,
    waist: ReleaseWaist,
    realm_digest: [u8; 32],
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    realm_bytes: Vec<u8>,
    mint: Pubkey,
    mint_bytes: Vec<u8>,
    token_program: Pubkey,
}

fn scenario() -> Scenario {
    let waist = release_waist();
    let dealer = Keypair::new();
    let mint = Pubkey::new_from_array([0x74; 32]);
    let token_program =
        dclutch_fractional_atomic_program_test::campaign_support::token_program_id();
    let adapter = dclutch_custody::token_svm::PRODUCTION_ADAPTER_RELEASES[1];
    let realm_bytes = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm")
    .to_bytes()
    .to_vec();
    let realm_digest = hash(&realm_bytes).to_bytes();
    let realm_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &waist.registry,
    )
    .0;
    let realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &waist.registry,
    )
    .0;
    let fixture = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: usize::try_from(WIDTH).expect("small width"),
        registry_program: waist.registry,
        core_program: CORE_PROGRAM,
        claims_program: CLAIMS_PROGRAM,
        release_set: waist.release_set_id,
        realm_id: realm_digest,
        custody_context: [0x99; 32],
        generation: GENERATION,
        actor_owner: dealer.pubkey(),
        reserve_owner: Pubkey::new_from_array([0x44; 32]),
        funded_coordinate: 0,
        funded_balance: 100,
        position_revision: POSITION_REVISION,
        reserve_balance: 0,
        terminal: None,
        rent_beneficiary: BENEFICIARY,
        graph_id: [0xb9; 32],
        exposure_id: [0xba; 32],
    })
    .expect("canonical narrow graph");
    let live = live_claims_graph(&fixture);
    Scenario {
        dealer,
        fixture,
        live,
        waist,
        realm_digest,
        realm_raw,
        realm_staging,
        realm_bytes,
        mint,
        mint_bytes: dclutch_fractional_atomic_program_test::campaign_support::collateral_mint_bytes(
            200, 0,
        ),
        token_program,
    }
}

fn add_executable(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(36).max(1),
            data: {
                let mut data = vec![0_u8; 36];
                data[..4].copy_from_slice(&2_u32.to_le_bytes());
                data[4..36].copy_from_slice(programdata_address(key).as_ref());
                data
            },
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn program_test(scenario: &Scenario) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in &scenario.waist.deployments {
        dclutch_fractional_atomic_program_test::campaign_support::add_upgradeable_program(
            &mut test, name, *program, elf,
        );
    }
    test.add_program("spl_token_2022", scenario.token_program, None);
    add_executable(&mut test, scenario.waist.registry);
    test.add_account(
        scenario.waist.activation_cache,
        data_account(scenario.waist.registry, scenario.waist.cache_body.clone()),
    );
    test.add_account(BENEFICIARY, data_account(system_program::ID, Vec::new()));
    test.add_account(
        scenario.realm_raw,
        data_account(scenario.waist.registry, scenario.realm_bytes.clone()),
    );
    test.add_account(
        scenario.realm_staging,
        data_account(system_program::ID, Vec::new()),
    );
    test.add_account(
        scenario.mint,
        data_account(scenario.token_program, scenario.mint_bytes.clone()),
    );
    test
}

fn core_id(bytes: [u8; 32]) -> dclutch_core_contract::ContentId {
    dclutch_core_contract::ContentId::new(bytes).expect("nonzero core identity")
}

fn manifest_id(bytes: [u8; 32]) -> ManifestContentId {
    ManifestContentId::new(bytes).expect("nonzero manifest identity")
}

fn record_bumps(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (u8, u8) {
    (
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).1,
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).1,
    )
}

fn loader_program_body(programdata: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES];
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
    encode_dealer_lp_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle).expect("LP lifecycle");
    let mut request_scratch =
        vec![0_u8; dclutch_trading_sbf::dealer::lp_artifacts::DEALER_LP_REQUEST_PROFILE_BYTES_V3];
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
        core_id(dclutch_vm::v3::SCHEMA_RELEASE_ID),
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
        core_id(dclutch_vm::v3::SCHEMA_RELEASE_ID),
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
    output_page: BuiltAccountV1,
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
            generation: GENERATION,
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
    let transition_bytes =
        dealer_equity_transition_bytes_v3(signed_position_count).expect("equity transition width");
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
    let effect_bytes =
        dealer_equity_effect_bytes_v4(action, signed_position_count).expect("equity effect width");
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
            dclutch_vm::request_profile::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
        }),
        core_id(hash(&request_profile).to_bytes()),
        core_id(dclutch_vm::v3::SCHEMA_RELEASE_ID),
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
        core_id(dclutch_vm::v3::SCHEMA_RELEASE_ID),
        core_id(hash(&transition).to_bytes()),
        core_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
        Some(certificate_id),
        core_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
        Some(core_id(hash(&admission).to_bytes())),
        // THE EQUITY ROUTE IS THE OUTPUT-PAGE ROUTE. Its candidate bank is
        // 1,392 bytes -- two 880-byte chunks, and this test failed on the
        // second one at `ComputationalBudgetExceeded` for as long as the
        // pair below named the chunked transport. One page, one caller
        // authority, one CPI.
        core_id(ACCELERATOR_OUTPUT_PAGE_REQUEST_SCHEMA_ID_V3),
        core_id(ACCELERATOR_OUTPUT_PAGE_ACK_SCHEMA_ID_V3),
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
    let bank_bytes = dealer_equity_scalar_count_v3(action)
        .expect("equity scalar count")
        .checked_mul(8)
        .and_then(|bytes| {
            dealer_equity_identity_count_v3(action)
                .expect("equity identity count")
                .checked_mul(32)
                .and_then(|identity_bytes| bytes.checked_add(identity_bytes))
        })
        .expect("equity bank width");
    let mut page_key = [0xf4_u8; 32];
    page_key[0] = match action {
        MultiLpActionV3::Add => 0xa1,
        MultiLpActionV3::Remove => 0xa2,
    };
    page_key[1] = u8::try_from(signed_position_count).expect("bounded signed positions");
    let output_page = built_data_account(
        &Rent::default(),
        Pubkey::new_from_array(page_key),
        ACCELERATOR,
        vec![0_u8; bank_bytes],
    );
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
        output_page,
    }
}

fn global_set(equity: &[EquityArtifacts; 6], open: &LpArtifacts, close: &LpArtifacts) -> Vec<u8> {
    let entries = equity
        .iter()
        .map(|value| &value.descriptor)
        .chain([&open.descriptor, &close.descriptor])
        .enumerate()
        .map(|(index, descriptor)| {
            CapabilityProgramSetEntryV2::new(
                u32::try_from(index + 1).expect("selector"),
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_SCHEMA_V4).expect("descriptor schema"),
                    ContentId::new(hash(descriptor).to_bytes()).expect("descriptor content"),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut output = vec![0_u8; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
    encode_program_set_v2(
        DEALER_EQUITY_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U16,
        &entries,
        &mut output,
    )
    .expect("global Dealer SetV2");
    output
}

struct LpCampaign {
    equity: [EquityArtifacts; 6],
    open: LpArtifacts,
    close: LpArtifacts,
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
            output_page: None,
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
            output_page: Some(&artifacts.output_page),
        }
    }
}

fn manifest_and_root(
    scenario: &Scenario,
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
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
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
        GENERATION,
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
        buy_used: [0; dclutch_trading::dealer::MAX_OUTCOMES],
        sell_used: [0; dclutch_trading::dealer::MAX_OUTCOMES],
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

fn campaign(scenario: &Scenario) -> LpCampaign {
    let accelerator_elf = elf("dclutch_accelerator_sbf");
    let release = artifact_release(ACCELERATOR, 0xef, &accelerator_elf);
    let artifact_release = release.to_bytes().to_vec();
    let release_id = ArtifactReleaseIdV1::new(hash(&artifact_release).to_bytes())
        .expect("accelerator ArtifactRelease id");

    // Core persists the RentCredit account address, while the credit body
    // persists the wallet that ultimately receives returned principal.
    // The older trade-only fixture names the wallet directly because it
    // never executes lifecycle.  This lifecycle campaign installs the
    // same Market at the same PDA with the canonical successor fact.
    let generation = GENERATION.to_le_bytes();
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
    let obligation_bytes_len = DEALER_OBLIGATION_HEADER_BYTES_V3
        + usize::try_from(scenario.fixture.outcome_count).expect("outcome width") * 8;
    let product_bytes = scenario.fixture.product.bytes.len();
    let portfolio_bytes = scenario.fixture.portfolio.bytes.len();
    let basis_bytes = scenario.fixture.linked_basis.bytes.len();
    let credit_bytes = u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("credit width");
    let common = [
        u32::try_from(root_bytes).expect("root width"),
        u32::try_from(dclutch_trading::dealer::config_v4::DEALER_CONFIG_BYTES_V4)
            .expect("config width"),
        u32::try_from(product_bytes).expect("product width"),
        u32::try_from(portfolio_bytes).expect("portfolio width"),
        u32::try_from(basis_bytes).expect("basis width"),
        u32::try_from(obligation_bytes_len).expect("obligation width"),
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
            &equity_logical_lengths(scenario, MultiLpActionV3::Add, 0, obligation_bytes_len),
            release_id,
        ),
        equity_artifacts(
            MultiLpActionV3::Add,
            1,
            &equity_logical_lengths(scenario, MultiLpActionV3::Add, 1, obligation_bytes_len),
            release_id,
        ),
        equity_artifacts(
            MultiLpActionV3::Add,
            2,
            &equity_logical_lengths(scenario, MultiLpActionV3::Add, 2, obligation_bytes_len),
            release_id,
        ),
        equity_artifacts(
            MultiLpActionV3::Remove,
            0,
            &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 0, obligation_bytes_len),
            release_id,
        ),
        equity_artifacts(
            MultiLpActionV3::Remove,
            1,
            &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 1, obligation_bytes_len),
            release_id,
        ),
        equity_artifacts(
            MultiLpActionV3::Remove,
            2,
            &equity_logical_lengths(scenario, MultiLpActionV3::Remove, 2, obligation_bytes_len),
            release_id,
        ),
    ];
    let program_set = global_set(&equity, &open, &close);
    let config = DealerConfigV4::new(
        scenario.waist.release_set_id,
        scenario.realm_digest,
        scenario.dealer.pubkey().to_bytes(),
        0,
    )
    .expect("Dealer config")
    .encode()
    .to_vec();
    let (manifest, root, root_bytes) =
        manifest_and_root(scenario, &program_set, &open.descriptor, &config);
    let obligation =
        Pubkey::find_program_address(&[DEALER_OBLIGATION_PDA_DOMAIN_V3, root.as_ref()], &TRADING).0;
    let obligation_bytes = obligation_bytes(
        scenario.fixture.core_market.to_bytes(),
        scenario.fixture.product_id,
        scenario.fixture.semantic_basis_id,
        scenario.dealer.pubkey().to_bytes(),
        root.to_bytes(),
        7,
        &[100],
    );
    let accelerator_programdata_key = programdata_address(ACCELERATOR);
    let accelerator_program = deployment_account(
        ACCELERATOR,
        loader_program_body(accelerator_programdata_key),
        true,
    );
    let accelerator_programdata = deployment_account(
        accelerator_programdata_key,
        loader_programdata_body(WAIST_SLOT, &accelerator_elf),
        false,
    );
    LpCampaign {
        equity,
        open,
        close,
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
fn equity_fixture(scenario: &Scenario, campaign: &LpCampaign, lp_owner: Pubkey) -> NarrowFixtureV2 {
    let fixture = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: usize::try_from(scenario.fixture.outcome_count)
            .expect("small equity outcome width"),
        registry_program: scenario.waist.registry,
        core_program: CORE_PROGRAM,
        claims_program: CLAIMS_PROGRAM,
        release_set: scenario.waist.release_set_id,
        realm_id: scenario.realm_digest,
        custody_context: campaign.root.to_bytes(),
        generation: GENERATION,
        actor_owner: scenario.dealer.pubkey(),
        reserve_owner: lp_owner,
        funded_coordinate: FUNDED_COORDINATE,
        funded_balance: 100,
        position_revision: POSITION_REVISION,
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
    scenario: &Scenario,
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
    let config = u32::try_from(dclutch_trading::dealer::config_v4::DEALER_CONFIG_BYTES_V4)
        .expect("config width");
    let loader = u32::try_from(dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES)
        .expect("loader Program width");
    let core = u32::try_from(scenario.fixture.core_state.len()).expect("Core width");
    let activation = u32::try_from(scenario.waist.cache_body.len()).expect("activation width");
    let realm = u32::try_from(scenario.realm_bytes.len()).expect("Realm width");
    let mint = u32::try_from(scenario.mint_bytes.len()).expect("Mint width");
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
        programdata(CLAIMS_PROGRAM),
        loader,
        programdata(CORE_PROGRAM),
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
    scenario: &Scenario,
    campaign: &LpCampaign,
    fixture: &NarrowFixtureV2,
    rent: &Rent,
) -> FixedCorpusV1 {
    FixedCorpusV1 {
        market: built_data_account(
            rent,
            scenario.fixture.core_market,
            CORE_PROGRAM,
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

fn fixed(scenario: &Scenario, campaign: &LpCampaign, rent: &Rent) -> FixedCorpusV1 {
    fixed_for_fixture(scenario, campaign, &scenario.fixture, rent)
}

fn waist(scenario: &Scenario) -> WaistFactsV1 {
    WaistFactsV1 {
        registry_program: scenario.waist.registry,
        trading_program: TRADING,
        core_program: CORE_PROGRAM,
        claims_program: CLAIMS_PROGRAM,
        custody_program: CUSTODY_PROGRAM,
        release_set: scenario.waist.release_set_id,
        activation_cache: scenario.waist.activation_cache,
        trading_semantic_release: [0xe6; 32],
    }
}

/// The market's rent sponsor: the RentCredit's one immutable refund
/// wallet, and nobody's LP owner.
///
/// This campaign used to stage the credit with the FIRST LP owner's own
/// key, and that is the only reason a per-owner Open ever passed. The
/// kernel made every created state's refund identity the credit's wallet,
/// and the Open requires that identity to equal the position's owner
/// (operation 12), so one key played both parts and a family whose request
/// type is `MultiLp` looked like it admitted many owners while admitting
/// exactly one per generation. Staging a sponsor who is nobody's owner is
/// what makes the second Open below evidence rather than a coincidence.
const MARKET_RENT_SPONSOR: Pubkey = Pubkey::new_from_array([0x5b; 32]);

fn lifecycle_credit(scenario: &Scenario, beneficiary: Pubkey) -> (Pubkey, LifecycleRentCreditV2) {
    let generation = GENERATION.to_le_bytes();
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
        GENERATION,
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
    scenario: &Scenario,
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
        generation: GENERATION,
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
        generation: GENERATION,
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
    scenario: &Scenario,
    campaign: &LpCampaign,
    lp_owner: Pubkey,
    external_tag: u8,
    external_balance: u64,
    allowance: u64,
) -> EquityCollateralState {
    let custody = CUSTODY_PROGRAM;
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
    let mint = scenario.mint;
    EquityCollateralState {
        authority,
        replay,
        replay_bytes: CustodyReplayV1 {
            caller_role: CallerRoleV1::Trading,
            release_set,
            market,
            realm: scenario.realm_digest,
            context: campaign.root.to_bytes(),
            caller_program: TRADING.to_bytes(),
            rent_refund: BENEFICIARY.to_bytes(),
            open_vault_count: 2,
            next_revision: 7,
            generation: GENERATION,
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
        principal_bytes:
            dclutch_fractional_atomic_program_test::campaign_support::token_account_bytes_for(
                mint, authority, 0,
            ),
        hoard,
        hoard_bytes:
            dclutch_fractional_atomic_program_test::campaign_support::token_account_bytes_for(
                mint, authority, 0,
            ),
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
) -> LiveClaimsGraph {
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
    LiveClaimsGraph {
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

async fn build_equity_bundle(
    context: &mut ProgramTestContext,
    scenario: &Scenario,
    campaign: &LpCampaign,
    fixture: &NarrowFixtureV2,
    live: &LiveClaimsGraph,
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
        collateral: dclutch_fractional_atomic_program_test::campaign_support::token_amount(
            &collateral.principal_bytes,
        ),
        claims: &live.dealer_balances,
        obligations: &canonical_obligations,
        total_shares: obligation.total_equity_shares(),
        locked_capital_floor: 0,
        action: pool_action,
        basis_scale: 1,
    })
    .expect("canonical pool-equity preflight");
    let request_bumps = mine_dealer_equity_bumps_v3(DealerEquityBumpSeedsV3 {
        trading_program: TRADING.to_bytes(),
        core_program: CORE_PROGRAM.to_bytes(),
        custody_program: CUSTODY_PROGRAM.to_bytes(),
        registry_program: scenario.waist.registry.to_bytes(),
        core_identity: CoreState::decode(&scenario.fixture.core_state)
            .expect("campaign Core state decodes")
            .identity,
        market: scenario.fixture.core_market.to_bytes(),
        release_set: scenario.waist.release_set_id,
        child_root: campaign.root.to_bytes(),
        realm: scenario.realm_digest,
    });
    let chain = EquityPoolChainProjectionV3 {
        trading_program: TRADING.to_bytes(),
        // Every one of these eight addresses is one this producer already
        // derived to name the account frame; the evaluator would otherwise
        // search for all eight on every invocation.
        bumps: request_bumps,
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
            principal_balance:
                dclutch_fractional_atomic_program_test::campaign_support::token_amount(
                    &collateral.principal_bytes,
                ),
            hoard_vault: collateral.hoard.to_bytes(),
            hoard_balance: dclutch_fractional_atomic_program_test::campaign_support::token_amount(
                &collateral.hoard_bytes,
            ),
        },
        locked_capital_floor: 0,
        generation: GENERATION,
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
        dclutch_trading_sbf::dealer::equity_request::DEALER_EQUITY_SELECTOR_OFFSET_V3,
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
    let replay_view =
        CustodyReplayV1::decode(&collateral.replay_bytes).expect("equity Custody replay decodes");
    let physical_context = MultiLpContextV3 {
        // The same three the on-chain evaluator relays into its own planner.
        bumps: MultiLpBumpHintsV3 {
            obligation: request_bumps.obligation(),
            principal_vault: request_bumps.principal_vault(),
            hoard_vault: request_bumps.hoard_vault(),
        },
        trading_program: TRADING.to_bytes(),
        custody_program: CUSTODY_PROGRAM.to_bytes(),
        release_set: scenario.waist.release_set_id,
        market: scenario.fixture.core_market.to_bytes(),
        realm: scenario.realm_digest,
        child_root: campaign.root.to_bytes(),
        obligation_account: campaign.obligation.to_bytes(),
        mint: scenario.mint.to_bytes(),
        token_program: scenario.token_program.to_bytes(),
        parent_request_digest: hash(&request).to_bytes(),
        generation: GENERATION,
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
    let mut projected_identities =
        vec![[0_u8; 32]; dealer_equity_identity_count_v3(action).expect("equity identity width")];
    let evidence_owner = dealer_equity_evidence_owner_identity_register_v3(action)
        .expect("equity evidence owner register");
    *projected_identities
        .get_mut(usize::from(evidence_owner))
        .expect("evidence owner register in bounds") = CLAIMS_PROGRAM.to_bytes();
    project_dealer_equity_hot_registers_v3(
        decoded,
        physical_plan,
        &custody_output,
        clock.slot,
        &mut projected_scalars,
        &mut projected_identities,
    )
    .expect("canonical equity Hot registers");
    let effect = dclutch_vm::effect::v4::ProgramV4::decode(
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
    let trading_programdata = observed_binding(context, scenario.waist.trading_programdata).await;
    let claims_program = observed_binding(context, CLAIMS_PROGRAM).await;
    let claims_programdata = observed_binding(context, scenario.waist.claims_programdata).await;
    let core_program = observed_binding(context, CORE_PROGRAM).await;
    let core_programdata = observed_binding(context, scenario.waist.core_programdata).await;
    let custody_program = observed_binding(context, CUSTODY_PROGRAM).await;
    let token_program = observed_binding(context, scenario.token_program).await;
    let rent_sysvar = observed_binding(context, sysvar::rent::ID).await;
    let market = built_data_account(
        &rent,
        scenario.fixture.core_market,
        CORE_PROGRAM,
        campaign.market_bytes.clone(),
    );
    let replay = built_data_account(
        &rent,
        collateral.replay,
        CUSTODY_PROGRAM,
        collateral.replay_bytes.clone(),
    );
    let mint = built_data_account(
        &rent,
        scenario.mint,
        scenario.token_program,
        scenario.mint_bytes.clone(),
    );
    let authority = vacant(collateral.authority);
    let external = built_data_account(
        &rent,
        collateral.external,
        scenario.token_program,
        collateral.external_bytes.clone(),
    );
    let principal = built_data_account(
        &rent,
        collateral.principal,
        scenario.token_program,
        collateral.principal_bytes.clone(),
    );
    let hoard = built_data_account(
        &rent,
        collateral.hoard,
        scenario.token_program,
        collateral.hoard_bytes.clone(),
    );
    let realm_raw = built_data_account(
        &rent,
        scenario.realm_raw,
        scenario.waist.registry,
        scenario.realm_bytes.clone(),
    );
    let realm_staging = vacant(scenario.realm_staging);
    let claims_market = built_data_account(
        &rent,
        fixture.claims_market,
        CLAIMS_PROGRAM,
        live.market.clone(),
    );
    let dealer_evidence = built_data_account(
        &rent,
        fixture.actor_position.account,
        CLAIMS_PROGRAM,
        live.dealer_position.clone(),
    );
    let (lp_evidence_key, lp_evidence_bytes) = lp_evidence_override.unwrap_or_else(|| {
        (
            fixture.reserve_position.account,
            live.counterparty_position.clone(),
        )
    });
    let lp_evidence = built_data_account(&rent, lp_evidence_key, CLAIMS_PROGRAM, lp_evidence_bytes);
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
                built_data_account(&rent, key, CLAIMS_PROGRAM, bytes),
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
        scenario.token_program,
    ];
    let scenario_input = ScenarioV1 {
        family_request: &request,
        tail_count: scenario.fixture.outcome_count,
        clock_slot: clock.slot,
        generation: GENERATION,
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
                .unwrap_or_else(|| panic!("missing selector-1 identity coordinate {coordinate}"))
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
        assert_eq!(claims_program.key, CLAIMS_PROGRAM, "Claims identity");
        assert_eq!(
            dealer_evidence.account.owner, CLAIMS_PROGRAM,
            "Dealer evidence owner"
        );
        assert_eq!(
            lp_evidence.account.owner, CLAIMS_PROGRAM,
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
    // Name what came back, and name the HOSTILE'S OWN WORD for it. The
    // builder carries a candidate projector's refusal out unaltered, and
    // this string exists nowhere in the builder -- only in the closure
    // above -- so the match proves both that the builder reached the
    // candidate phase and that the closure this test installed is what
    // refused there. Any other error is the builder refusing earlier for
    // its own reasons, which is exactly the question a bare `matches!`
    // could not answer.
    match build_admitted_bundle_with_candidate_v1(
        &bundle_input,
        campaign.equity_admitted(expected_selector),
        &hostile_candidate,
    ) {
        Err(BuilderError::Projection("hostile-equity-candidate")) => {}
        Err(other) => {
            panic!("hostile equity candidate refused as {other:?}, not with its own cause")
        }
        Ok(_) => panic!("hostile equity candidate was admitted"),
    }
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
                base.invocation_count(route, scenario.fixture.outcome_count, scalars, identities)
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
    let compute = solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
        1_400_000,
    );
    let transaction = Transaction::new_signed_with_payer(
        &[compute, heap, instruction],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
}

fn custom_code(result: &Result<(), TransactionError>) -> Option<u32> {
    match result {
        Err(TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}

fn transaction_logs(
    processed: &solana_program_test::BanksTransactionResultWithMetadata,
) -> Vec<String> {
    processed
        .metadata
        .as_ref()
        .map(|metadata| metadata.log_messages.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|line| line.strip_prefix("Program log: ").map(str::to_owned))
        .collect()
}

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

fn token_amount(account: &Account) -> u64 {
    SplTokenAccount::unpack(&account.data)
        .expect("token account")
        .amount
}

fn assert_vacant(account: Option<Account>, label: &str) {
    assert!(
        account.is_none_or(|value| {
            value.lamports == 0 && value.data.is_empty() && value.owner == system_program::ID
        }),
        "{label} must be canonical vacancy"
    );
}

#[tokio::test]
async fn accepted_multi_lp_full_exit_uses_current_real_elf_and_rolls_back_late_substitution() {
    let scenario = scenario();
    let campaign = campaign(&scenario);
    let lp_a = Keypair::new();
    let lp_b = Keypair::new();
    let fixture_a = equity_fixture(&scenario, &campaign, lp_a.pubkey());
    let fixture_b = equity_fixture(&scenario, &campaign, lp_b.pubkey());
    let live_a = live_claims_graph(&fixture_a);
    let live_b = live_claims_graph(&fixture_b);
    assert_eq!(fixture_a.claims_market, fixture_b.claims_market);
    assert_eq!(
        fixture_a.actor_position.account,
        fixture_b.actor_position.account
    );
    assert_ne!(
        fixture_a.reserve_position.account,
        fixture_b.reserve_position.account
    );
    assert_eq!(live_a.market, live_b.market);
    assert_eq!(live_a.dealer_position, live_b.dealer_position);
    assert_eq!(live_a.dealer_balances, vec![100]);
    assert_eq!(live_a.counterparty_balances, vec![0]);
    assert_eq!(live_b.counterparty_balances, vec![0]);

    let mut test = program_test(&scenario);
    dclutch_fractional_atomic_program_test::campaign_support::add_upgradeable_program(
        &mut test,
        "dclutch_accelerator_sbf",
        ACCELERATOR,
        &elf("dclutch_accelerator_sbf"),
    );
    let mut context = test.start_with_context().await;
    context.set_account(
        &scenario.fixture.core_market,
        &AccountSharedData::from(data_account(CORE_PROGRAM, campaign.market_bytes.clone())),
    );
    for owner in [&lp_a, &lp_b] {
        context.set_account(
            &owner.pubkey(),
            &AccountSharedData::from(Account {
                lamports: 10_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
    }
    let (credit, credit_state) = lifecycle_credit(&scenario, MARKET_RENT_SPONSOR);
    assert_eq!(credit, campaign.rent_credit);
    let credit_account = data_account(scenario.waist.registry, credit_state.to_bytes().to_vec());
    context.set_account(&credit, &AccountSharedData::from(credit_account));

    // Open A from the current selector-7 descriptor.
    let credit_before_open_a = chain_account(&mut context, credit).await;
    let open_a = build_lifecycle_bundle(
        &mut context,
        &scenario,
        &campaign,
        MultiLpRequestActionV3::Open,
        lp_a.pubkey(),
        &campaign.obligation_bytes,
        None,
        credit,
        credit_before_open_a,
    )
    .await;
    let position_a = open_a.bundle.logical.get(6).expect("LP A position").key;
    install_bundle(&mut context, &open_a);
    let opened_a_result = submit_lp_hot(
        &mut context,
        open_a.bundle.hot_instruction.clone(),
        &[&lp_a],
    )
    .await
    .expect("submit LP A Open");
    assert_eq!(opened_a_result.result, Ok(()), "LP A Open");
    assert!(invoked_programs(&opened_a_result).contains(&ACCELERATOR));
    let opened_a = chain_account(&mut context, position_a).await;
    let position_rent = Rent::default().minimum_balance(DEALER_LP_POSITION_BYTES_V3);
    assert_eq!(opened_a.lamports, position_rent);
    assert_eq!(
        DealerLpPositionV3::decode(&opened_a.data)
            .expect("opened LP A")
            .equity_shares,
        0
    );

    // A late, otherwise well-formed Position identity is substituted after
    // the request and current chain projection have selected cash-only P0.
    let collateral_a = first_equity_collateral(&scenario, &campaign, lp_a.pubkey(), 0xf4, 100, 40);
    let zero_claims = vec![0_u64; usize::try_from(WIDTH).expect("small width")];
    let add_a_intent = EquityRequestIntentV3::Contribute {
        collateral: 40,
        claims: &zero_claims,
        minted_shares: 40,
    };
    let honest_add_a = build_equity_bundle(
        &mut context,
        &scenario,
        &campaign,
        &fixture_a,
        &live_a,
        lp_a.pubkey(),
        &campaign.obligation_bytes,
        &opened_a,
        &collateral_a,
        add_a_intent,
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
        None,
    )
    .await;
    let hostile_add_a = build_equity_bundle(
        &mut context,
        &scenario,
        &campaign,
        &fixture_a,
        &live_a,
        lp_a.pubkey(),
        &campaign.obligation_bytes,
        &opened_a,
        &collateral_a,
        add_a_intent,
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
        Some((
            scenario.fixture.reserve_position.account,
            scenario.live.counterparty_position.clone(),
        )),
    )
    .await;
    install_bundle(&mut context, &hostile_add_a.admitted);
    let rollback_keys = [
        campaign.root,
        campaign.obligation,
        position_a,
        collateral_a.replay,
        collateral_a.external,
        collateral_a.principal,
        collateral_a.hoard,
        fixture_a.claims_market,
        fixture_a.actor_position.account,
    ];
    let mut rollback_before = Vec::new();
    for key in rollback_keys {
        rollback_before.push((key, chain_account(&mut context, key).await));
    }
    let refused = submit_lp_hot(
        &mut context,
        hostile_add_a.admitted.bundle.hot_instruction.clone(),
        &[],
    )
    .await
    .expect("submit hostile LP A Add");
    assert_eq!(
        custom_code(&refused.result),
        Some(TradingSbfError::Transition as u32),
        "late evidence substitution must refuse through the admitted accelerator"
    );
    assert!(invoked_programs(&refused).contains(&ACCELERATOR));
    let refused_logs = transaction_logs(&refused);
    assert!(
        refused_logs
            .iter()
            .any(|line| line == DealerEquityAcceleratorErrorV4::Claims.refusal_name()),
        "exact late-evidence conjunct: {refused_logs:?}"
    );
    for (key, before) in rollback_before {
        assert_eq!(
            chain_account(&mut context, key).await,
            before,
            "rollback {key}"
        );
    }

    install_bundle(&mut context, &honest_add_a.admitted);
    assert_installed_logical_views(&mut context, &honest_add_a.admitted).await;
    let add_a_result = submit_lp_hot(
        &mut context,
        honest_add_a.admitted.bundle.hot_instruction.clone(),
        &[],
    )
    .await
    .expect("submit LP A Add");
    assert_eq!(add_a_result.result, Ok(()), "LP A Add");
    assert!(invoked_programs(&add_a_result).contains(&CUSTODY_PROGRAM));
    assert!(!invoked_programs(&add_a_result).contains(&CLAIMS_PROGRAM));
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.external).await),
        60
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.principal).await),
        40
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.hoard).await),
        0
    );
    assert_eq!(
        DealerObligationProjectionV3::decode(
            &chain_account(&mut context, campaign.obligation).await.data,
        )
        .expect("post-A obligation")
        .total_equity_shares(),
        40
    );
    assert_eq!(
        DealerLpPositionV3::decode(&chain_account(&mut context, position_a).await.data)
            .expect("post-A LP")
            .equity_shares,
        40
    );
    assert_eq!(
        chain_account(&mut context, fixture_a.claims_market)
            .await
            .data,
        live_a.market
    );
    assert_eq!(
        chain_account(&mut context, fixture_a.actor_position.account)
            .await
            .data,
        live_a.dealer_position
    );
    assert_eq!(
        chain_account(&mut context, fixture_a.reserve_position.account)
            .await
            .data,
        live_a.counterparty_position
    );

    // Open B only after A's contribution has committed, then contribute from
    // B against the immediately observed shared obligation and vault state.
    let obligation_after_a = chain_account(&mut context, campaign.obligation).await.data;
    let credit_before_open_b = chain_account(&mut context, credit).await;
    let open_b = build_lifecycle_bundle(
        &mut context,
        &scenario,
        &campaign,
        MultiLpRequestActionV3::Open,
        lp_b.pubkey(),
        &obligation_after_a,
        None,
        credit,
        credit_before_open_b,
    )
    .await;
    let position_b = open_b.bundle.logical.get(6).expect("LP B position").key;
    install_bundle(&mut context, &open_b);
    let open_b_result = submit_lp_hot(
        &mut context,
        open_b.bundle.hot_instruction.clone(),
        &[&lp_b],
    )
    .await
    .expect("submit LP B Open");
    assert_eq!(open_b_result.result, Ok(()), "LP B Open");
    let opened_b = chain_account(&mut context, position_b).await;

    let mut collateral_b =
        first_equity_collateral(&scenario, &campaign, lp_b.pubkey(), 0xf5, 100, 60);
    collateral_b.replay_bytes = chain_account(&mut context, collateral_b.replay).await.data;
    collateral_b.principal_bytes = chain_account(&mut context, collateral_b.principal)
        .await
        .data;
    collateral_b.hoard_bytes = chain_account(&mut context, collateral_b.hoard).await.data;
    let add_b = build_equity_bundle(
        &mut context,
        &scenario,
        &campaign,
        &fixture_b,
        &live_b,
        lp_b.pubkey(),
        &obligation_after_a,
        &opened_b,
        &collateral_b,
        EquityRequestIntentV3::Contribute {
            collateral: 60,
            claims: &zero_claims,
            minted_shares: 60,
        },
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
        None,
    )
    .await;
    install_bundle(&mut context, &add_b.admitted);
    let add_b_result = submit_lp_hot(
        &mut context,
        add_b.admitted.bundle.hot_instruction.clone(),
        &[],
    )
    .await
    .expect("submit LP B Add");
    assert_eq!(add_b_result.result, Ok(()), "LP B Add");
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.external).await),
        40
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.principal).await),
        100
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.hoard).await),
        0
    );
    assert_eq!(
        DealerLpPositionV3::decode(&chain_account(&mut context, position_a).await.data)
            .expect("LP A after B")
            .equity_shares,
        40
    );
    assert_eq!(
        DealerLpPositionV3::decode(&chain_account(&mut context, position_b).await.data)
            .expect("LP B after B")
            .equity_shares,
        60
    );
    assert_eq!(
        DealerObligationProjectionV3::decode(
            &chain_account(&mut context, campaign.obligation).await.data,
        )
        .expect("fully funded obligation")
        .total_equity_shares(),
        100
    );

    // Full cash-only exits use selector P0 and rebuild from each immediately
    // finalized poststate. No Claims bytes or Hoard principal may move.
    let obligation_before_remove_a = chain_account(&mut context, campaign.obligation).await.data;
    let collateral_before_remove_a = current_equity_collateral(&mut context, &collateral_a).await;
    let claims_before_remove_a = current_equity_claims(&mut context, &fixture_a).await;
    let lp_before_remove_a = chain_account(&mut context, position_a).await;
    let remove_a = build_equity_bundle(
        &mut context,
        &scenario,
        &campaign,
        &fixture_a,
        &claims_before_remove_a,
        lp_a.pubkey(),
        &obligation_before_remove_a,
        &lp_before_remove_a,
        &collateral_before_remove_a,
        EquityRequestIntentV3::Redeem { burned_shares: 40 },
        DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
        None,
    )
    .await;
    assert_eq!(remove_a.plan.collateral_out, 40);
    install_bundle(&mut context, &remove_a.admitted);
    let remove_a_result = submit_lp_hot(
        &mut context,
        remove_a.admitted.bundle.hot_instruction.clone(),
        &[],
    )
    .await
    .expect("submit LP A Remove");
    assert_eq!(remove_a_result.result, Ok(()), "LP A Remove");
    assert!(!invoked_programs(&remove_a_result).contains(&CLAIMS_PROGRAM));
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.external).await),
        100
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.principal).await),
        60
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.hoard).await),
        0
    );
    assert_eq!(
        DealerLpPositionV3::decode(&chain_account(&mut context, position_a).await.data)
            .expect("empty LP A")
            .equity_shares,
        0
    );

    let obligation_before_remove_b = chain_account(&mut context, campaign.obligation).await.data;
    let collateral_before_remove_b = current_equity_collateral(&mut context, &collateral_b).await;
    let claims_before_remove_b = current_equity_claims(&mut context, &fixture_b).await;
    let lp_before_remove_b = chain_account(&mut context, position_b).await;
    let remove_b = build_equity_bundle(
        &mut context,
        &scenario,
        &campaign,
        &fixture_b,
        &claims_before_remove_b,
        lp_b.pubkey(),
        &obligation_before_remove_b,
        &lp_before_remove_b,
        &collateral_before_remove_b,
        EquityRequestIntentV3::Redeem { burned_shares: 60 },
        DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
        None,
    )
    .await;
    assert_eq!(remove_b.plan.collateral_out, 60);
    install_bundle(&mut context, &remove_b.admitted);
    let remove_b_result = submit_lp_hot(
        &mut context,
        remove_b.admitted.bundle.hot_instruction.clone(),
        &[],
    )
    .await
    .expect("submit LP B Remove");
    assert_eq!(remove_b_result.result, Ok(()), "LP B Remove");
    assert!(!invoked_programs(&remove_b_result).contains(&CLAIMS_PROGRAM));
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.external).await),
        100
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.principal).await),
        0
    );
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_b.hoard).await),
        0
    );
    assert_eq!(
        DealerLpPositionV3::decode(&chain_account(&mut context, position_b).await.data)
            .expect("empty LP B")
            .equity_shares,
        0
    );
    assert_eq!(
        DealerObligationProjectionV3::decode(
            &chain_account(&mut context, campaign.obligation).await.data,
        )
        .expect("zero-share obligation")
        .total_equity_shares(),
        0
    );
    for (fixture, live) in [(&fixture_a, &live_a), (&fixture_b, &live_b)] {
        assert_eq!(
            chain_account(&mut context, fixture.claims_market)
                .await
                .data,
            live.market
        );
        assert_eq!(
            chain_account(&mut context, fixture.actor_position.account)
                .await
                .data,
            live.dealer_position
        );
        assert_eq!(
            chain_account(&mut context, fixture.reserve_position.account)
                .await
                .data,
            live.counterparty_position
        );
    }
    assert_eq!(
        token_amount(&chain_account(&mut context, collateral_a.external).await)
            + token_amount(&chain_account(&mut context, collateral_b.external).await)
            + token_amount(&chain_account(&mut context, collateral_a.principal).await)
            + token_amount(&chain_account(&mut context, collateral_a.hoard).await),
        200,
        "all collateral atoms return to their external owners"
    );

    // Both empty LP accounts close permissionlessly. Their exact rent
    // principal returns to the Market-owned LifecycleRentCredit; neither owner
    // wallet changes during Close.
    let owner_a_before_close = chain_account(&mut context, lp_a.pubkey()).await;
    let owner_b_before_close = chain_account(&mut context, lp_b.pubkey()).await;
    let credit_before_close = chain_account(&mut context, credit).await;
    let terminal_obligation = chain_account(&mut context, campaign.obligation).await.data;
    let position_a_before_close = chain_account(&mut context, position_a).await;
    let close_a = build_lifecycle_bundle(
        &mut context,
        &scenario,
        &campaign,
        MultiLpRequestActionV3::Close,
        lp_a.pubkey(),
        &terminal_obligation,
        Some(position_a_before_close),
        credit,
        credit_before_close.clone(),
    )
    .await;
    install_bundle(&mut context, &close_a);
    let close_a_result = submit_lp_hot(&mut context, close_a.bundle.hot_instruction.clone(), &[])
        .await
        .expect("submit LP A Close");
    assert_eq!(close_a_result.result, Ok(()), "LP A Close");
    assert_vacant(
        context
            .banks_client
            .get_account(position_a)
            .await
            .expect("read LP A vacancy"),
        "LP A",
    );

    let credit_after_a = chain_account(&mut context, credit).await;
    let position_b_before_close = chain_account(&mut context, position_b).await;
    let close_b = build_lifecycle_bundle(
        &mut context,
        &scenario,
        &campaign,
        MultiLpRequestActionV3::Close,
        lp_b.pubkey(),
        &terminal_obligation,
        Some(position_b_before_close),
        credit,
        credit_after_a,
    )
    .await;
    install_bundle(&mut context, &close_b);
    let close_b_result = submit_lp_hot(&mut context, close_b.bundle.hot_instruction.clone(), &[])
        .await
        .expect("submit LP B Close");
    assert_eq!(close_b_result.result, Ok(()), "LP B Close");
    assert_vacant(
        context
            .banks_client
            .get_account(position_b)
            .await
            .expect("read LP B vacancy"),
        "LP B",
    );
    let credit_after_close = chain_account(&mut context, credit).await;
    assert_eq!(credit_after_close.data, credit_before_close.data);
    assert_eq!(credit_after_close.owner, credit_before_close.owner);
    assert_eq!(
        credit_after_close.lamports,
        credit_before_close.lamports + position_rent * 2,
        "both LP rent principals return to LifecycleRentCredit"
    );
    assert_eq!(
        chain_account(&mut context, lp_a.pubkey()).await,
        owner_a_before_close
    );
    assert_eq!(
        chain_account(&mut context, lp_b.pubkey()).await,
        owner_b_before_close
    );
}
