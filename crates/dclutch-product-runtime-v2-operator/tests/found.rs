//! Runtime-width Core Found construction and hostile substitution tests.

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, EMPTY_MANIFEST_BYTES};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{Identity, MarketCoreStateSeedsV2, MarketIdentity};
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{FinalizedRecordCoordinateV2, PRODUCT_RECORD_BYTES_V2};
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, CompiledProductRecordsV2, Error, FinalizedRecordObservationV2,
    ProductCompilationInputV2, compile_product_records_v2,
    found::{
        FOUND_ACCOUNT_COUNT_V2, FinalizedReferenceObservationV2, FoundStateV2,
        build_found_instruction_v2,
    },
    lifecycle_rent_v2::{LifecycleRentCreateStateV2, build_lifecycle_rent_create_v2},
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    ProgramIdentityV1, ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use solana_program::sysvar::SysvarSerialize;
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

const SLOT: u64 = 919;
const GENERATION: u64 = 41;
const REGISTRY: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const RENT_PROGRAM: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([0xa4; 32]);

#[derive(Clone)]
struct RecordBacking {
    schema: [u8; 32],
    digest: [u8; 32],
    raw: Pubkey,
    staging: Pubkey,
    data: Vec<u8>,
}

impl RecordBacking {
    fn new(schema: [u8; 32], data: Vec<u8>) -> Self {
        let digest = hash(&data).to_bytes();
        let raw =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &REGISTRY).0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY,
        )
        .0;
        Self {
            schema,
            digest,
            raw,
            staging,
            data,
        }
    }

    fn from_coordinate(coordinate: FinalizedRecordCoordinateV2, data: Vec<u8>) -> Self {
        let record = Self::new(coordinate.schema_id.to_bytes(), data);
        assert_eq!(record.digest, coordinate.content_digest.to_bytes());
        assert_eq!(
            record.raw,
            Pubkey::new_from_array(coordinate.raw_account.to_bytes())
        );
        assert_eq!(
            record.staging,
            Pubkey::new_from_array(coordinate.staging_account.to_bytes())
        );
        record
    }

    fn observation(&self) -> FinalizedRecordObservationV2<'_> {
        let minimum = Rent::default().minimum_balance(self.data.len());
        FinalizedRecordObservationV2 {
            raw: account(self.raw, REGISTRY, minimum, false, &self.data),
            staging: account(self.staging, system_program::ID, 17, false, &[]),
            raw_rent_minimum: minimum,
        }
    }

    fn reference(&self) -> FinalizedReferenceObservationV2<'_> {
        FinalizedReferenceObservationV2 {
            schema_id: self.schema,
            record: self.observation(),
        }
    }
}

struct CompiledGraph {
    report: CompiledProductRecordsV2,
    product: RecordBacking,
    domain: RecordBacking,
    portfolio: RecordBacking,
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("fixture identity")
}

fn source_id(byte: u8) -> SourceContentId {
    SourceContentId::new([byte; 32]).expect("Source fixture identity")
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero identity")
}

fn account(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: &[u8],
) -> AccountObservationV2<'_> {
    AccountObservationV2 {
        slot: SLOT,
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

fn compile_graph(cuts: &[i128], coefficient: u64) -> CompiledGraph {
    let outcome_count = cuts.len().checked_add(2).expect("outcome count");
    let coefficients = vec![coefficient; outcome_count];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain bytes")];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(outcome_count).expect("portfolio bytes")];
    let report = compile_product_records_v2(
        REGISTRY,
        ProductCompilationInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            cut_denominator: 1,
            cuts,
            portfolio_denominator: 9,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("canonical Product graph");
    CompiledGraph {
        product: RecordBacking::from_coordinate(report.receipt.product, product.to_vec()),
        domain: RecordBacking::from_coordinate(report.receipt.result_domain, domain),
        portfolio: RecordBacking::from_coordinate(report.receipt.portfolio, portfolio),
        report,
    }
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn artifact_release(program: Pubkey, programdata: Pubkey, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata.to_bytes(),
        CoreContentId::new([0xb1; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        71,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact identity")
}

fn activation(release: ArtifactReleaseV1) -> (ExecutionReleaseSetV1, Pubkey, Vec<u8>) {
    let binding = ExecutionRoleBindingV1::new(release.program(), artifact_id(release));
    let release_set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
        .expect("release set");
    let release_set_digest = hash(&release_set.to_bytes()).to_bytes();
    let release_set_id = CoreContentId::new(release_set_digest).expect("release-set identity");
    let mut bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, release_set_id).expect("activation cache");
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
    let input = ArtifactActivationInputV1::new(artifact_id(release), release, observation);
    for role in [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ] {
        activate_execution_role_into_v1(&mut bytes, release_set_id, &release_set, role, &input)
            .expect("role activation");
    }
    let cache =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_digest], &REGISTRY).0;
    (release_set, cache, bytes)
}

fn loader_program_data(programdata: Pubkey) -> Vec<u8> {
    let mut data = vec![0_u8; 36];
    put(&mut data, 0, &2_u32.to_le_bytes());
    put(&mut data, 4, programdata.as_ref());
    data
}

fn loader_programdata_data(elf: &[u8]) -> Vec<u8> {
    let mut data = vec![0_u8; 45 + elf.len()];
    put(&mut data, 0, &3_u32.to_le_bytes());
    put(&mut data, 4, &71_u64.to_le_bytes());
    put(&mut data, 12, &[0]);
    put(&mut data, 45, elf);
    data
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
    let end = offset.checked_add(bytes.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture output")
        .copy_from_slice(bytes);
}

fn rent_data() -> Vec<u8> {
    let rent = Rent::default();
    let mut lamports = 1;
    let mut data = vec![0_u8; Rent::size_of()];
    let key = sysvar::rent::ID;
    let owner = sysvar::ID;
    let mut info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
    rent.to_account_info(&mut info)
        .expect("serialize canonical Rent");
    data
}

struct Fixture {
    graph: CompiledGraph,
    realm: RecordBacking,
    source: RecordBacking,
    manifest: RecordBacking,
    release_set: RecordBacking,
    activation_cache: Pubkey,
    activation_data: Vec<u8>,
    core_programdata: Pubkey,
    core_program_data: Vec<u8>,
    core_programdata_data: Vec<u8>,
    registry_artifact: RecordBacking,
    registry_programdata: Pubkey,
    registry_program_data: Vec<u8>,
    registry_programdata_data: Vec<u8>,
    rent_artifact: RecordBacking,
    rent_programdata: Pubkey,
    rent_program_data: Vec<u8>,
    rent_programdata_data: Vec<u8>,
    infrastructure_profile: Pubkey,
    infrastructure_profile_data: Vec<u8>,
    rent_credit: Pubkey,
    rent_credit_data: Vec<u8>,
    rent_data: Vec<u8>,
    market: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let cuts: Vec<i128> = (-128_i128..128).collect();
        let graph = compile_graph(&cuts, 7);
        assert_eq!(graph.report.outcome_count, 258);
        let realm = RealmV1::new(RealmV1Input {
            token_program: [0xc1; 32],
            collateral_mint: [0xc2; 32],
            collateral_adapter_release_id: [0xc3; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm");
        let realm = RecordBacking::new(REALM_SCHEMA_RELEASE_ID_V1, realm.to_bytes().to_vec());
        let source = SourceMaterialV2::new(
            SourceContentId::new(graph.product.digest).expect("Product record digest"),
            source_id(0xd1),
            source_id(0xd2),
            source_id(0xd3),
            None,
            source_id(0xd4),
        );
        let source = RecordBacking::new(
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
            source.to_bytes().to_vec(),
        );
        let manifest = RecordBacking::new(
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            EMPTY_MANIFEST_BYTES.to_vec(),
        );
        let core_programdata =
            Pubkey::find_program_address(&[CORE.as_ref()], &bpf_loader_upgradeable::ID).0;
        let core_elf = b"dclutch-core-found-v2-fixture";
        let core_release = artifact_release(CORE, core_programdata, core_elf);
        let (release_set, activation_cache, activation_data) = activation(core_release);
        let release_set = RecordBacking::new(
            EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            release_set.to_bytes().to_vec(),
        );
        let registry_programdata =
            Pubkey::find_program_address(&[REGISTRY.as_ref()], &bpf_loader_upgradeable::ID).0;
        let registry_elf = b"dclutch-registry-v1-fixture";
        let registry_release = artifact_release(REGISTRY, registry_programdata, registry_elf);
        let registry_artifact = RecordBacking::new(
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            registry_release.to_bytes().to_vec(),
        );
        let rent_programdata =
            Pubkey::find_program_address(&[RENT_PROGRAM.as_ref()], &bpf_loader_upgradeable::ID).0;
        let rent_elf = b"dclutch-rent-v1-fixture";
        let rent_release = artifact_release(RENT_PROGRAM, rent_programdata, rent_elf);
        let rent_artifact = RecordBacking::new(
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            rent_release.to_bytes().to_vec(),
        );
        let profile = ProtocolInfrastructureProfileV1::new(
            ExecutionRoleBindingV1::new(registry_release.program(), artifact_id(registry_release)),
            ExecutionRoleBindingV1::new(rent_release.program(), artifact_id(rent_release)),
        )
        .expect("infrastructure profile");
        let infrastructure_profile =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &CORE).0;
        let market_identity = MarketIdentity {
            market_id: identity([0xff; 32]),
            realm_id: identity(realm.digest),
            product_record: identity(graph.product.digest),
            product_id: identity(id(1).to_bytes()),
            resolution_policy: identity(source.digest),
            capability_manifest: identity(manifest.digest),
            selected_release_set: identity(release_set.digest),
            registry_program: identity(REGISTRY.to_bytes()),
            generation: GENERATION,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
            &CORE,
        )
        .0;
        let authority = RefundAuthority::new(PAYER.to_bytes()).expect("refund authority");
        let (rent_credit, bump) = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                market.as_ref(),
                &GENERATION.to_le_bytes(),
            ],
            &RENT_PROGRAM,
        );
        let rent_credit_data = LifecycleRentCreditV2::new(
            authority,
            LifecycleAccountIdV2::new(market.to_bytes()).expect("market"),
            LifecycleAccountIdV2::new(release_set.digest).expect("release set"),
            GENERATION,
            bump,
        )
        .expect("lifecycle credit")
        .to_bytes()
        .to_vec();
        Self {
            graph,
            realm,
            source,
            manifest,
            release_set,
            activation_cache,
            activation_data,
            core_programdata,
            core_program_data: loader_program_data(core_programdata),
            core_programdata_data: loader_programdata_data(core_elf),
            registry_artifact,
            registry_programdata,
            registry_program_data: loader_program_data(registry_programdata),
            registry_programdata_data: loader_programdata_data(registry_elf),
            rent_artifact,
            rent_programdata,
            rent_program_data: loader_program_data(rent_programdata),
            rent_programdata_data: loader_programdata_data(rent_elf),
            infrastructure_profile,
            infrastructure_profile_data: profile.to_bytes().to_vec(),
            rent_credit,
            rent_credit_data,
            rent_data: rent_data(),
            market,
        }
    }

    fn state(&self) -> FoundStateV2<'_> {
        FoundStateV2 {
            payer: account(PAYER, system_program::ID, 10_000_000_000, false, &[]),
            market: account(self.market, system_program::ID, 0, false, &[]),
            rent_credit: account(
                self.rent_credit,
                RENT_PROGRAM,
                1_000_000,
                false,
                &self.rent_credit_data,
            ),
            rent_program: account(
                RENT_PROGRAM,
                bpf_loader_upgradeable::ID,
                1,
                true,
                &self.rent_program_data,
            ),
            realm: self.realm.reference(),
            product: self.graph.product.observation(),
            result_domain: self.graph.domain.observation(),
            portfolio: self.graph.portfolio.observation(),
            source_material: self.source.reference(),
            capability_manifest: self.manifest.reference(),
            execution_release_set: self.release_set.reference(),
            activation_cache: account(
                self.activation_cache,
                REGISTRY,
                1_000_000,
                false,
                &self.activation_data,
            ),
            core_program: account(
                CORE,
                bpf_loader_upgradeable::ID,
                1,
                true,
                &self.core_program_data,
            ),
            core_programdata: account(
                self.core_programdata,
                bpf_loader_upgradeable::ID,
                1,
                false,
                &self.core_programdata_data,
            ),
            registry_program: account(
                REGISTRY,
                bpf_loader_upgradeable::ID,
                1,
                true,
                &self.registry_program_data,
            ),
            rent: account(sysvar::rent::ID, sysvar::ID, 1, false, &self.rent_data),
            system_program: account(system_program::ID, native_loader::ID, 1, true, &[]),
            infrastructure_profile: account(
                self.infrastructure_profile,
                CORE,
                Rent::default().minimum_balance(self.infrastructure_profile_data.len()),
                false,
                &self.infrastructure_profile_data,
            ),
            registry_artifact: self.registry_artifact.observation(),
            registry_programdata: account(
                self.registry_programdata,
                bpf_loader_upgradeable::ID,
                1,
                false,
                &self.registry_programdata_data,
            ),
            rent_artifact: self.rent_artifact.observation(),
            rent_programdata: account(
                self.rent_programdata,
                bpf_loader_upgradeable::ID,
                1,
                false,
                &self.rent_programdata_data,
            ),
        }
    }
}

#[test]
fn valid_found31_exports_runtime_width_instruction() {
    let fixture = Fixture::new();
    assert_eq!(fixture.graph.report.outcome_count, 258);
    assert_eq!(FOUND_ACCOUNT_COUNT_V2, 31);
    let plan =
        build_found_instruction_v2(GENERATION, fixture.state()).expect("Found instruction plan");
    assert_eq!(plan.outcome_count, 258);
    assert_eq!(plan.instruction.accounts.len(), 31);
    assert_eq!(plan.market_address, fixture.market);
    assert_eq!(
        plan.product.product_record_digest.to_bytes(),
        fixture.graph.product.digest
    );
    assert_eq!(
        plan.instruction
            .accounts
            .get(24)
            .expect("profile meta")
            .pubkey,
        fixture.infrastructure_profile
    );
    assert_eq!(
        plan.instruction
            .accounts
            .get(25)
            .expect("Registry artifact meta")
            .pubkey,
        fixture.registry_artifact.raw
    );
    assert_eq!(
        plan.instruction
            .accounts
            .get(27)
            .expect("Registry ProgramData meta")
            .pubkey,
        fixture.registry_programdata
    );
    assert_eq!(
        plan.instruction
            .accounts
            .get(28)
            .expect("Rent artifact meta")
            .pubkey,
        fixture.rent_artifact.raw
    );
    assert_eq!(
        plan.instruction
            .accounts
            .get(30)
            .expect("Rent ProgramData meta")
            .pubkey,
        fixture.rent_programdata
    );
}

#[test]
fn lifecycle_credit_creation_is_derived_from_the_found_projection() {
    let fixture = Fixture::new();
    let found = build_found_instruction_v2(GENERATION, fixture.state()).expect("Found plan");
    let plan = build_lifecycle_rent_create_v2(
        &found,
        LifecycleRentCreateStateV2 {
            payer: account(PAYER, system_program::ID, 10_000_000_000, false, &[]),
            credit_destination: account(fixture.rent_credit, system_program::ID, 0, false, &[]),
            refund_wallet: account(PAYER, system_program::ID, 10_000_000_000, false, &[]),
            rent_program: fixture.state().rent_program,
            system_program: fixture.state().system_program,
            rent: fixture.state().rent,
        },
    )
    .expect("lifecycle credit create");
    assert_eq!(plan.credit, fixture.rent_credit);
    assert_eq!(plan.state.market().to_bytes(), fixture.market.to_bytes());
    assert_eq!(plan.state.generation(), GENERATION);
    assert_eq!(plan.instruction.accounts.len(), 4);

    let hostile = build_lifecycle_rent_create_v2(
        &found,
        LifecycleRentCreateStateV2 {
            payer: account(PAYER, system_program::ID, 10_000_000_000, false, &[]),
            credit_destination: account(
                Pubkey::new_from_array([0xee; 32]),
                system_program::ID,
                0,
                false,
                &[],
            ),
            refund_wallet: account(PAYER, system_program::ID, 10_000_000_000, false, &[]),
            rent_program: fixture.state().rent_program,
            system_program: fixture.state().system_program,
            rent: fixture.state().rent,
        },
    );
    assert_eq!(hostile, Err(dclutch_product_runtime_v2_operator::lifecycle_rent_v2::LifecycleRentOperatorErrorV2::InvalidCredit));
}

#[test]
fn same_width_domain_and_portfolio_substitution_refuse() {
    let fixture = Fixture::new();
    let shifted_cuts: Vec<i128> = (-127_i128..129).collect();
    let hostile_domain = compile_graph(&shifted_cuts, 7).domain;
    assert_eq!(hostile_domain.data.len(), fixture.graph.domain.data.len());
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                result_domain: hostile_domain.observation(),
                ..fixture.state()
            }
        ),
        Err(Error::InvalidRecord)
    );

    let original_cuts: Vec<i128> = (-128_i128..128).collect();
    let hostile_portfolio = compile_graph(&original_cuts, 8).portfolio;
    assert_eq!(
        hostile_portfolio.data.len(),
        fixture.graph.portfolio.data.len()
    );
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                portfolio: hostile_portfolio.observation(),
                ..fixture.state()
            }
        ),
        Err(Error::InvalidRecord)
    );
}

#[test]
fn stale_snapshot_and_caller_supplied_rent_projection_refuse() {
    let fixture = Fixture::new();
    let state = fixture.state();
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                result_domain: FinalizedRecordObservationV2 {
                    raw: AccountObservationV2 {
                        slot: SLOT + 1,
                        ..state.result_domain.raw
                    },
                    ..state.result_domain
                },
                ..state
            }
        ),
        Err(Error::ObservationMismatch)
    );
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                realm: FinalizedReferenceObservationV2 {
                    record: FinalizedRecordObservationV2 {
                        raw_rent_minimum: state.realm.record.raw_rent_minimum + 1,
                        ..state.realm.record
                    },
                    ..state.realm
                },
                ..state
            }
        ),
        Err(Error::ObservationMismatch)
    );
}

#[test]
fn substituted_profile_and_mutable_infrastructure_refuse() {
    let fixture = Fixture::new();
    let mut hostile_profile = fixture.infrastructure_profile_data.clone();
    *hostile_profile.get_mut(48).expect("artifact byte") ^= 1;
    let state = fixture.state();
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                infrastructure_profile: AccountObservationV2 {
                    data: &hostile_profile,
                    ..state.infrastructure_profile
                },
                ..state
            }
        ),
        Err(Error::CrossRecordMismatch)
    );

    let mutable_release = ArtifactReleaseV1::new(
        program_identity(REGISTRY),
        program_identity(bpf_loader_upgradeable::ID),
        fixture.registry_programdata.to_bytes(),
        CoreContentId::new([0xb1; 32]).expect("semantic release"),
        hash(b"dclutch-registry-v1-fixture").to_bytes(),
        71,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some([0xee; 32]),
    )
    .expect("mutable artifact");
    let mutable_artifact = RecordBacking::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        mutable_release.to_bytes().to_vec(),
    );
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                registry_artifact: mutable_artifact.observation(),
                ..fixture.state()
            }
        ),
        Err(Error::CrossRecordMismatch)
    );
}

#[test]
fn stale_registry_elf_refuses_before_transaction_export() {
    let fixture = Fixture::new();
    let mut stale_programdata = fixture.registry_programdata_data.clone();
    let last = stale_programdata.len().saturating_sub(1);
    *stale_programdata.get_mut(last).expect("ELF byte") ^= 1;
    let state = fixture.state();
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                registry_programdata: AccountObservationV2 {
                    data: &stale_programdata,
                    ..state.registry_programdata
                },
                ..state
            }
        ),
        Err(Error::CrossRecordMismatch)
    );
}

#[test]
fn wrong_source_product_and_late_payer_failure_refuse() {
    let fixture = Fixture::new();
    let hostile_source = SourceMaterialV2::new(
        source_id(0xe1),
        source_id(0xd1),
        source_id(0xd2),
        source_id(0xd3),
        None,
        source_id(0xd4),
    );
    let hostile_source = RecordBacking::new(
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        hostile_source.to_bytes().to_vec(),
    );
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                source_material: hostile_source.reference(),
                ..fixture.state()
            }
        ),
        Err(Error::CrossRecordMismatch)
    );

    let state = fixture.state();
    assert_eq!(
        build_found_instruction_v2(
            GENERATION,
            FoundStateV2 {
                payer: AccountObservationV2 {
                    lamports: 0,
                    ..state.payer
                },
                ..state
            }
        ),
        Err(Error::InsufficientPayer)
    );
}
