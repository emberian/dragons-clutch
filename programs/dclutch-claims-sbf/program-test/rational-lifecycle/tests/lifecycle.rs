//! Real Token-2022 activation, retirement, and transaction-rollback evidence.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, ProductLbv2FixtureV2,
    compile_product_lbv2_fixture_v2,
};
use dclutch_claims_sbf::rational_lifecycle_v2::{
    RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2,
    RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2,
};
use dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2;
use dclutch_claims_svm::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionClaimsCapabilitySeedsV2,
    ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    ProtocolPositionSeedsV2,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{CoreState, Identity, Phase};
use dclutch_rational_representation_v2_contract::{
    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2,
    RATIONAL_STRUCTURED_CUSTODY_SEED_V2, RationalReceiptMintSeedsV2,
};
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    ABSENT_POSITION_REVISION_V2, LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2,
    LIFECYCLE_RECEIPT_BYTES_V2, LifecycleActionV2, LifecycleCoordinateV2, LifecycleHeaderV2,
    LifecycleReceiptV2, LifecycleRequestV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_rent_contract::{RefundAuthority, RentCreditV1};
use dclutch_token_svm::{
    ACCOUNT_BYTES, TOKEN_2022_CLOSEABLE_MINT_BYTES_V2, TOKEN_2022_PROGRAM_ID,
    Token2022CloseableMintProfileV2, TokenAccount,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table as create_lookup_table_instruction, extend_lookup_table,
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
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::versioned::VersionedTransaction;

const CLAIMS: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const REGISTRY: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const TRADING: Pubkey = Pubkey::new_from_array([0xc4; 32]);
const RENT_PROGRAM: Pubkey = Pubkey::new_from_array([0xc5; 32]);
const TOKEN_2022: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const GENERATION: u64 = 41;
const OUTCOME: u32 = 0;
const COEFFICIENT: u64 = 1;
const GRAPH_ID: [u8; 32] = [0xd1; 32];
const PARENT_CONTEXT: [u8; 32] = [0xd2; 32];
const TOKEN_2022_V11_ELF_DIGEST: [u8; 32] = [
    0x44, 0x7c, 0xa3, 0xc6, 0x90, 0xec, 0x00, 0x1c, 0x88, 0xca, 0xdc, 0xa3, 0x41, 0x52, 0xa4, 0xab,
    0xb7, 0x80, 0x65, 0x85, 0x52, 0xe8, 0x72, 0xd5, 0x97, 0x75, 0xcc, 0x41, 0x7e, 0x6e, 0xc2, 0x5d,
];

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    trading: Vec<u8>,
    rent: Vec<u8>,
    token_2022: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let read = |name: &str| fs::read(directory.join(name)).expect("real ELF");
    let token_2022 = read("spl_token_2022.so");
    assert_eq!(
        hash(&token_2022).to_bytes(),
        TOKEN_2022_V11_ELF_DIGEST,
        "exact Token-2022 v11 ELF"
    );
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        trading: read("dclutch_rational_lifecycle_test_caller_sbf.so"),
        rent: read("dclutch_rent_sbf.so"),
        token_2022,
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn add_exact(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>, lamports: u64) {
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

fn add_rent_exempt(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) -> u64 {
    let lamports = Rent::default().minimum_balance(data.len()).max(1);
    add_exact(test, key, owner, data, lamports);
    lamports
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_rent_exempt(
        test,
        programdata(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
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
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
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
        .expect("deployment observation"),
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
    let content = ContentId::new(id).expect("release ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("activation cache");
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
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete activation");
    (id, bytes)
}

fn add_record(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_rent_exempt(test, record.raw, record.owner, record.bytes.clone());
    add_exact(test, record.staging, system_program::ID, Vec::new(), 1);
}

fn finalized_descriptor(test: &mut ProgramTest, bytes: Vec<u8>) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            &digest,
        ],
        &REGISTRY,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            &digest,
        ],
        &REGISTRY,
    )
    .0;
    add_rent_exempt(test, raw, REGISTRY, bytes);
    add_exact(test, staging, system_program::ID, Vec::new(), 1);
    (raw, staging, digest)
}

fn descriptor_bytes(
    graph_digest: [u8; 32],
    market: Pubkey,
    release: [u8; 32],
    receipt_mint: Pubkey,
    outcome_count: u32,
) -> Vec<u8> {
    let count = usize::try_from(outcome_count).expect("outcome count");
    let mut bytes = vec![0; DESCRIPTOR_HEADER_BYTES + count * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut bytes, 8, &3_u16.to_le_bytes());
    put(&mut bytes, 16, &GRAPH_ID);
    put(&mut bytes, 48, &graph_digest);
    put(&mut bytes, 80, &[0xd3; 32]);
    put(&mut bytes, 112, market.as_ref());
    put(&mut bytes, 144, &release);
    put(&mut bytes, 176, receipt_mint.as_ref());
    put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put(&mut bytes, 240, &outcome_count.to_le_bytes());
    put(&mut bytes, 248, &1_u64.to_le_bytes());
    put(
        &mut bytes,
        DESCRIPTOR_HEADER_BYTES,
        &COEFFICIENT.to_le_bytes(),
    );
    bytes
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

struct Fixture {
    release: [u8; 32],
    cache: Pubkey,
    graph: ProductLbv2FixtureV2,
    descriptor_raw: Pubkey,
    descriptor_staging: Pubkey,
    descriptor_id: [u8; 32],
    representation_authority: Pubkey,
    receipt_mint: Pubkey,
    old_ata_scar: Pubkey,
    claims_owner: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    shard_mint: Pubkey,
    structured_custody: Pubkey,
    rent_credit: Pubkey,
    rent_credit_initial: u64,
    receipt_lamports: u64,
    shard_lamports: u64,
    structured_lamports: u64,
    position_lamports: u64,
    admission_lamports: u64,
    receipt_rent: u64,
    shard_rent: u64,
    structured_rent: u64,
    position_rent: u64,
    admission_rent: u64,
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        ("dclutch_claims_sbf", CLAIMS, artifacts.claims.as_slice()),
        (
            "dclutch_registry_sbf",
            REGISTRY,
            artifacts.registry.as_slice(),
        ),
        ("dclutch_core_sbf", CORE, artifacts.core.as_slice()),
        (
            "dclutch_rational_lifecycle_test_caller_sbf",
            TRADING,
            artifacts.trading.as_slice(),
        ),
        ("dclutch_rent_sbf", RENT_PROGRAM, artifacts.rent.as_slice()),
        (
            "spl_token_2022",
            TOKEN_2022,
            artifacts.token_2022.as_slice(),
        ),
    ] {
        add_program(&mut test, name, program, elf);
    }

    let (release, cache_data) = activation(&artifacts);
    let cache = Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release], &REGISTRY).0;
    add_rent_exempt(&mut test, cache, REGISTRY, cache_data);

    let refund = RefundAuthority::new([0xe1; 32]).expect("refund authority");
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            dclutch_rent_contract::RENT_CREDIT_PDA_DOMAIN_V1,
            &refund.to_bytes(),
        ],
        &RENT_PROGRAM,
    );
    let rent_credit_data = RentCreditV1::new(refund, rent_bump).to_bytes().to_vec();
    let rent_credit_initial =
        add_rent_exempt(&mut test, rent_credit, RENT_PROGRAM, rent_credit_data);

    let graph = compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY,
        core_program: CORE,
        claims_program: CLAIMS,
        release_set: release,
        realm_id: [0xe2; 32],
        custody_context: [0xe3; 32],
        generation: GENERATION,
        source_owner: rent_credit,
        destination_owner: Pubkey::new_from_array([0xe4; 32]),
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
    add_rent_exempt(&mut test, graph.core_market, CORE, graph.core_state.clone());
    add_rent_exempt(
        &mut test,
        graph.claims_market,
        CLAIMS,
        graph.claims_market_bytes.clone(),
    );

    let graph_digest = hash(b"rational-lifecycle-finalized-graph-v2").to_bytes();
    let receipt_seeds =
        RationalReceiptMintSeedsV2::new(graph_digest, graph.core_market.to_bytes(), release)
            .expect("receipt seeds");
    let receipt_mint = Pubkey::find_program_address(&receipt_seeds.as_slices(), &CLAIMS).0;
    let descriptor = descriptor_bytes(
        graph_digest,
        graph.core_market,
        release,
        receipt_mint,
        graph.outcome_count,
    );
    let (descriptor_raw, descriptor_staging, descriptor_id) =
        finalized_descriptor(&mut test, descriptor);
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &CLAIMS,
    )
    .0;
    add_exact(
        &mut test,
        representation_authority,
        system_program::ID,
        Vec::new(),
        1,
    );

    let outcome = OUTCOME.to_le_bytes();
    let shard_mint = Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor_id, &outcome],
        &CLAIMS,
    )
    .0;
    let structured_custody = Pubkey::find_program_address(
        &[
            RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
            &descriptor_id,
            &outcome,
        ],
        &CLAIMS,
    )
    .0;
    let owner_seeds = ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor_id, OUTCOME)
        .expect("Claims capability owner");
    let claims_owner = Pubkey::find_program_address(&owner_seeds.as_slices(), &CLAIMS).0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(graph.claims_market.to_bytes(), claims_owner.to_bytes())
            .expect("Position seeds");
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS).0;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        graph.claims_market.to_bytes(),
        claims_owner.to_bytes(),
    )
    .expect("admission seeds");
    let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &CLAIMS).0;
    add_exact(&mut test, claims_owner, system_program::ID, Vec::new(), 1);

    let rent = Rent::default();
    let receipt_rent = rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2);
    let shard_rent = receipt_rent;
    let structured_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let position_width = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
        + usize::try_from(graph.outcome_count).expect("width") * 8;
    let position_rent = rent.minimum_balance(position_width);
    let admission_rent = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    let receipt_lamports = receipt_rent + 17;
    let shard_lamports = shard_rent + 19;
    let structured_lamports = structured_rent + 23;
    let position_lamports = position_rent + 29;
    let admission_lamports = admission_rent + 31;
    for (key, lamports) in [
        (receipt_mint, receipt_lamports),
        (shard_mint, shard_lamports),
        (structured_custody, structured_lamports),
        (position, position_lamports),
        (admission, admission_lamports),
    ] {
        add_exact(&mut test, key, system_program::ID, Vec::new(), lamports);
    }
    let old_ata_scar = Pubkey::new_from_array([0xe5; 32]);
    add_exact(
        &mut test,
        old_ata_scar,
        system_program::ID,
        Vec::new(),
        receipt_lamports,
    );

    (
        test,
        Fixture {
            release,
            cache,
            graph,
            descriptor_raw,
            descriptor_staging,
            descriptor_id,
            representation_authority,
            receipt_mint,
            old_ata_scar,
            claims_owner,
            position,
            admission,
            shard_mint,
            structured_custody,
            rent_credit,
            rent_credit_initial,
            receipt_lamports,
            shard_lamports,
            structured_lamports,
            position_lamports,
            admission_lamports,
            receipt_rent,
            shard_rent,
            structured_rent,
            position_rent,
            admission_rent,
        },
    )
}

fn coordinate(f: &Fixture, vacancy: bool) -> LifecycleCoordinateV2 {
    LifecycleCoordinateV2 {
        outcome: OUTCOME,
        coefficient: COEFFICIENT,
        shard_mint: f.shard_mint.to_bytes(),
        structured_custody_account: f.structured_custody.to_bytes(),
        claims_custody_owner: f.claims_owner.to_bytes(),
        claims_custody_position: f.position.to_bytes(),
        position_admission: f.admission.to_bytes(),
        observed_shard_lamports: if vacancy { 0 } else { f.shard_lamports },
        observed_structured_lamports: if vacancy { 0 } else { f.structured_lamports },
        observed_position_lamports: if vacancy { 0 } else { f.position_lamports },
        observed_admission_lamports: if vacancy { 0 } else { f.admission_lamports },
        shard_rent_principal: if vacancy { 0 } else { f.shard_rent },
        structured_rent_principal: if vacancy { 0 } else { f.structured_rent },
        position_rent_principal: if vacancy { 0 } else { f.position_rent },
        admission_rent_principal: if vacancy { 0 } else { f.admission_rent },
        expected_shard_supply: 0,
        expected_structured_amount: 0,
        expected_position_revision: if vacancy {
            ABSENT_POSITION_REVISION_V2
        } else {
            0
        },
    }
}

fn request(f: &Fixture, action: LifecycleActionV2, rent_before: u64) -> Vec<u8> {
    let coordinate_credit = f
        .shard_lamports
        .checked_add(f.structured_lamports)
        .and_then(|value| value.checked_add(f.position_lamports))
        .and_then(|value| value.checked_add(f.admission_lamports))
        .expect("coordinate credit");
    let credit = match action {
        LifecycleActionV2::RetireCoordinate => coordinate_credit,
        LifecycleActionV2::RetireReceipt => f.receipt_lamports,
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::ActivateCoordinate => 0,
    };
    let rows = match action {
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            vec![coordinate(f, false)]
        }
        LifecycleActionV2::RetireReceipt => vec![coordinate(f, true)],
        LifecycleActionV2::ActivateReceipt => Vec::new(),
    };
    let mut row_bytes = vec![0; rows.len() * LIFECYCLE_COORDINATE_BYTES_V2];
    for (index, row) in rows.iter().copied().enumerate() {
        let start = index * LIFECYCLE_COORDINATE_BYTES_V2;
        let end = start + LIFECYCLE_COORDINATE_BYTES_V2;
        row.encode_into(row_bytes.get_mut(start..end).expect("coordinate row"))
            .expect("coordinate encoding");
    }
    let header = LifecycleHeaderV2 {
        action,
        release_set: f.release,
        market: f.graph.core_market.to_bytes(),
        graph_id: GRAPH_ID,
        descriptor_id: f.descriptor_id,
        parent_context: PARENT_CONTEXT,
        representation_authority: f.representation_authority.to_bytes(),
        receipt_mint: f.receipt_mint.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        rent_credit: f.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM.to_bytes(),
        generation: GENERATION,
        expected_claims_market_revision: 0,
        observed_receipt_lamports: f.receipt_lamports,
        receipt_rent_principal: f.receipt_rent,
        expected_receipt_supply: 0,
        outcome_count: f.graph.outcome_count,
        coordinate_count: u32::try_from(rows.len()).expect("coordinate count"),
        rent_credit_before: rent_before,
        rent_credit_after: rent_before.checked_add(credit).expect("rent after"),
    };
    let lifecycle = LifecycleRequestV2::new(header, &row_bytes).expect("lifecycle request");
    let mut output = vec![0; LIFECYCLE_HEADER_BYTES_V2 + row_bytes.len()];
    lifecycle
        .encode_into(&mut output)
        .expect("request encoding");
    output
}

fn child_authority(f: &Fixture, lifecycle_bytes: &[u8]) -> Pubkey {
    let lifecycle = LifecycleRequestV2::decode(lifecycle_bytes).expect("lifecycle");
    let header = lifecycle.header();
    let row = lifecycle
        .coordinates()
        .next()
        .expect("coordinate")
        .expect("coordinate bytes");
    let action = if header.action == LifecycleActionV2::ActivateCoordinate {
        ProtocolPositionActionV2::Admit
    } else {
        ProtocolPositionActionV2::Close
    };
    let child = ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: header.release_set,
        market: header.market,
        position_owner: f.claims_owner.to_bytes(),
        parent_request_digest: hash(lifecycle_bytes).to_bytes(),
        rent_credit: header.rent_credit,
        rent_program: header.rent_program,
        generation: header.generation,
        expected_market_revision: header.expected_claims_market_revision,
        expected_position_revision: row.expected_position_revision,
        observed_position_lamports: row.observed_position_lamports,
        observed_admission_lamports: row.observed_admission_lamports,
        position_rent_principal: row.position_rent_principal,
        admission_rent_principal: row.admission_rent_principal,
        capability_descriptor: header.descriptor_id,
        capability_outcome: row.outcome,
    }
    .new()
    .expect("child request");
    let bytes = child.to_bytes().expect("child bytes");
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        f.claims_owner.to_bytes(),
        hash(&bytes).to_bytes(),
    )
    .expect("child authority");
    Pubkey::find_program_address(&seeds.as_slices(), &TRADING).0
}

fn wrapped(f: &Fixture, bytes: Vec<u8>, fail_after: bool, old_ata: bool) -> Instruction {
    let lifecycle = LifecycleRequestV2::decode(&bytes).expect("lifecycle");
    let header = lifecycle.header();
    let outer_seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(&bytes).to_bytes(),
    )
    .expect("outer authority");
    let outer = Pubkey::find_program_address(&outer_seeds.as_slices(), &TRADING).0;
    let receipt = if old_ata {
        f.old_ata_scar
    } else {
        f.receipt_mint
    };
    let mut forwarded = vec![
        AccountMeta::new_readonly(outer, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(f.cache, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(f.descriptor_raw, false),
        AccountMeta::new_readonly(f.descriptor_staging, false),
        AccountMeta::new_readonly(f.representation_authority, false),
        if matches!(
            header.action,
            LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
        ) {
            AccountMeta::new(receipt, false)
        } else {
            AccountMeta::new_readonly(receipt, false)
        },
        AccountMeta::new_readonly(TOKEN_2022, false),
        if header.action.retires() {
            AccountMeta::new(f.rent_credit, false)
        } else {
            AccountMeta::new_readonly(f.rent_credit, false)
        },
        AccountMeta::new_readonly(RENT_PROGRAM, false),
        AccountMeta::new_readonly(f.graph.claims_market, false),
        AccountMeta::new_readonly(f.graph.core_market, false),
        AccountMeta::new_readonly(CORE, false),
        AccountMeta::new_readonly(programdata(CORE), false),
    ];
    match header.action {
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            forwarded.extend([
                AccountMeta::new_readonly(child_authority(f, &bytes), false),
                AccountMeta::new(f.position, false),
                AccountMeta::new(f.admission, false),
                AccountMeta::new(f.shard_mint, false),
                AccountMeta::new(f.structured_custody, false),
                AccountMeta::new_readonly(f.claims_owner, false),
                AccountMeta::new_readonly(f.graph.linked_basis.raw, false),
                AccountMeta::new_readonly(f.graph.linked_basis.staging, false),
                AccountMeta::new_readonly(f.graph.product.raw, false),
                AccountMeta::new_readonly(f.graph.product.staging, false),
                AccountMeta::new_readonly(f.graph.result_domain.raw, false),
                AccountMeta::new_readonly(f.graph.result_domain.staging, false),
                AccountMeta::new_readonly(f.graph.portfolio.raw, false),
                AccountMeta::new_readonly(f.graph.portfolio.staging, false),
            ]);
            assert_eq!(
                forwarded.len(),
                RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2
            );
        }
        LifecycleActionV2::RetireReceipt => {
            forwarded.extend([
                AccountMeta::new_readonly(f.shard_mint, false),
                AccountMeta::new_readonly(f.structured_custody, false),
                AccountMeta::new_readonly(f.position, false),
                AccountMeta::new_readonly(f.admission, false),
            ]);
            assert_eq!(
                forwarded.len(),
                RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2
                    + RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2
            );
        }
        LifecycleActionV2::ActivateReceipt => {
            assert_eq!(forwarded.len(), RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2);
        }
    }
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

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("read account")
}

async fn process_legacy(context: &mut ProgramTestContext, instruction: Instruction) {
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
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT transaction");
    assert!(processed.result.is_ok(), "ALT transaction commits");
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

async fn create_lookup_table(context: &mut ProgramTestContext, addresses: &[Pubkey]) -> Pubkey {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock");
    context.warp_to_slot(clock.slot + 1).expect("recent slot");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table_instruction(payer, payer, clock.slot);
    process_legacy(context, create).await;
    for chunk in addresses.chunks(20) {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
        )
        .await;
    }
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("updated Clock");
    context.warp_to_slot(clock.slot + 1).expect("activate ALT");
    table
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
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
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
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
    Ok((processed.result.is_ok(), logs, returned, compute_units))
}

fn make_core_retiring(context: &mut ProgramTestContext, f: &Fixture, mut account: Account) {
    let mut state = CoreState::decode(&account.data).expect("open Core");
    state.phase = Phase::Retiring;
    state.terminal_winner = OUTCOME;
    state.terminal_receipt = Some(Identity::new([0xf1; 32]).expect("terminal receipt"));
    account.data = state.encode().expect("retiring Core").to_vec();
    context.set_account(&f.graph.core_market, &AccountSharedData::from(account));
}

fn assert_lifecycle_receipt(
    returned: Option<(Pubkey, Vec<u8>)>,
    request: &[u8],
    action: LifecycleActionV2,
) -> LifecycleReceiptV2 {
    let (producer, bytes) = returned.expect("lifecycle receipt");
    assert_eq!(
        producer, TRADING,
        "production caller returns checked receipt"
    );
    assert_eq!(bytes.len(), LIFECYCLE_RECEIPT_BYTES_V2);
    let receipt = LifecycleReceiptV2::decode(&bytes).expect("typed receipt");
    assert_eq!(receipt.action(), action);
    assert_eq!(receipt.request_digest(), hash(request).to_bytes());
    receipt
}

#[tokio::test]
async fn real_token_2022_lifecycle_refuses_ata_substitution_and_rolls_back_every_late_failure() {
    let (test, f) = fixture();
    let mut context = test.start_with_context().await;
    let activate_receipt_bytes = request(
        &f,
        LifecycleActionV2::ActivateReceipt,
        f.rent_credit_initial,
    );
    let activate_coordinate_bytes = request(
        &f,
        LifecycleActionV2::ActivateCoordinate,
        f.rent_credit_initial,
    );
    let coordinate_credit = f
        .shard_lamports
        .checked_add(f.structured_lamports)
        .and_then(|value| value.checked_add(f.position_lamports))
        .and_then(|value| value.checked_add(f.admission_lamports))
        .expect("coordinate credit");
    let after_coordinate_close = f.rent_credit_initial + coordinate_credit;
    let retire_coordinate_bytes = request(
        &f,
        LifecycleActionV2::RetireCoordinate,
        f.rent_credit_initial,
    );
    let retire_receipt_bytes =
        request(&f, LifecycleActionV2::RetireReceipt, after_coordinate_close);
    let hostile_ata = wrapped(&f, activate_receipt_bytes.clone(), false, true);
    let late_receipt = wrapped(&f, activate_receipt_bytes.clone(), true, false);
    let activate_receipt = wrapped(&f, activate_receipt_bytes.clone(), false, false);
    let replay_receipt = activate_receipt.clone();
    let late_coordinate = wrapped(&f, activate_coordinate_bytes.clone(), true, false);
    let activate_coordinate = wrapped(&f, activate_coordinate_bytes.clone(), false, false);
    let late_retire_coordinate = wrapped(&f, retire_coordinate_bytes.clone(), true, false);
    let retire_coordinate = wrapped(&f, retire_coordinate_bytes.clone(), false, false);
    let late_retire_receipt = wrapped(&f, retire_receipt_bytes.clone(), true, false);
    let retire_receipt = wrapped(&f, retire_receipt_bytes.clone(), false, false);
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        &[
            hostile_ata.clone(),
            late_receipt.clone(),
            activate_receipt.clone(),
            late_coordinate.clone(),
            activate_coordinate.clone(),
            late_retire_coordinate.clone(),
            retire_coordinate.clone(),
            late_retire_receipt.clone(),
            retire_receipt.clone(),
        ],
    );
    let table = create_lookup_table(&mut context, &addresses).await;

    let initial_receipt = account(&mut context, f.receipt_mint)
        .await
        .expect("prepaid receipt");
    let initial_rent = account(&mut context, f.rent_credit)
        .await
        .expect("RentCredit");
    let (accepted, _, _, _) = submit(&mut context, hostile_ata, table, &addresses)
        .await
        .expect("ATA substitution");
    assert!(!accepted, "old ATA coordinate refuses");
    assert_eq!(
        account(&mut context, f.receipt_mint).await,
        Some(initial_receipt.clone())
    );

    let (accepted, logs, _, _) = submit(&mut context, late_receipt, table, &addresses)
        .await
        .expect("late receipt activation");
    assert!(!accepted);
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {CLAIMS} success"))
    );
    assert_eq!(
        account(&mut context, f.receipt_mint).await,
        Some(initial_receipt.clone())
    );
    assert_eq!(
        account(&mut context, f.rent_credit).await,
        Some(initial_rent.clone())
    );

    let (accepted, _, returned, activate_receipt_cu) =
        submit(&mut context, activate_receipt, table, &addresses)
            .await
            .expect("activate receipt");
    assert!(accepted);
    assert_lifecycle_receipt(
        returned,
        &activate_receipt_bytes,
        LifecycleActionV2::ActivateReceipt,
    );
    let receipt_account = account(&mut context, f.receipt_mint)
        .await
        .expect("live receipt Mint");
    assert_eq!(receipt_account.owner, TOKEN_2022);
    Token2022CloseableMintProfileV2::check_mint(
        TOKEN_2022_PROGRAM_ID,
        &receipt_account.data,
        f.representation_authority.to_bytes(),
        f.representation_authority.to_bytes(),
        0,
        0,
    )
    .expect("closeable receipt Mint profile");
    let (accepted, _, _, _) = submit(&mut context, replay_receipt, table, &addresses)
        .await
        .expect("receipt replay");
    assert!(!accepted);

    let before_coordinate = [
        account(&mut context, f.shard_mint).await,
        account(&mut context, f.structured_custody).await,
        account(&mut context, f.position).await,
        account(&mut context, f.admission).await,
    ];
    let (accepted, logs, _, _) = submit(&mut context, late_coordinate, table, &addresses)
        .await
        .expect("late coordinate activation");
    assert!(!accepted);
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {CLAIMS} success"))
    );
    assert_eq!(
        [
            account(&mut context, f.shard_mint).await,
            account(&mut context, f.structured_custody).await,
            account(&mut context, f.position).await,
            account(&mut context, f.admission).await,
        ],
        before_coordinate
    );

    let (accepted, _, returned, activate_coordinate_cu) =
        submit(&mut context, activate_coordinate, table, &addresses)
            .await
            .expect("activate coordinate");
    assert!(accepted);
    assert_lifecycle_receipt(
        returned,
        &activate_coordinate_bytes,
        LifecycleActionV2::ActivateCoordinate,
    );
    let shard = account(&mut context, f.shard_mint)
        .await
        .expect("shard Mint");
    Token2022CloseableMintProfileV2::check_mint(
        TOKEN_2022_PROGRAM_ID,
        &shard.data,
        f.representation_authority.to_bytes(),
        f.representation_authority.to_bytes(),
        0,
        0,
    )
    .expect("closeable shard Mint profile");
    let structured = account(&mut context, f.structured_custody)
        .await
        .expect("structured custody");
    let structured_token = TokenAccount::parse(&structured.data).expect("token account");
    assert_eq!(structured_token.mint, f.shard_mint.to_bytes());
    assert_eq!(
        structured_token.owner,
        f.representation_authority.to_bytes()
    );
    assert_eq!(structured_token.amount, 0);

    let open_core = account(&mut context, f.graph.core_market)
        .await
        .expect("Core Market");
    make_core_retiring(&mut context, &f, open_core);

    let before_retire = [
        account(&mut context, f.shard_mint).await,
        account(&mut context, f.structured_custody).await,
        account(&mut context, f.position).await,
        account(&mut context, f.admission).await,
        account(&mut context, f.rent_credit).await,
    ];
    let (accepted, logs, _, _) = submit(&mut context, late_retire_coordinate, table, &addresses)
        .await
        .expect("late coordinate retirement");
    assert!(!accepted);
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {CLAIMS} success"))
    );
    assert_eq!(
        [
            account(&mut context, f.shard_mint).await,
            account(&mut context, f.structured_custody).await,
            account(&mut context, f.position).await,
            account(&mut context, f.admission).await,
            account(&mut context, f.rent_credit).await,
        ],
        before_retire
    );

    let (accepted, _, returned, retire_coordinate_cu) =
        submit(&mut context, retire_coordinate, table, &addresses)
            .await
            .expect("retire coordinate");
    assert!(accepted);
    let receipt = assert_lifecycle_receipt(
        returned,
        &retire_coordinate_bytes,
        LifecycleActionV2::RetireCoordinate,
    );
    assert_eq!(receipt.credited_lamports(), coordinate_credit);
    for key in [f.shard_mint, f.structured_custody, f.position, f.admission] {
        assert!(
            account(&mut context, key).await.is_none(),
            "resource closed"
        );
    }
    let rent_after_coordinate = account(&mut context, f.rent_credit)
        .await
        .expect("RentCredit after coordinate");
    assert_eq!(rent_after_coordinate.lamports, after_coordinate_close);

    let before_receipt_close = account(&mut context, f.receipt_mint)
        .await
        .expect("receipt before close");
    let (accepted, logs, _, _) = submit(&mut context, late_retire_receipt, table, &addresses)
        .await
        .expect("late receipt retirement");
    assert!(!accepted);
    assert!(
        logs.iter()
            .any(|line| line == &format!("Program {CLAIMS} success"))
    );
    assert_eq!(
        account(&mut context, f.receipt_mint).await,
        Some(before_receipt_close)
    );
    assert_eq!(
        account(&mut context, f.rent_credit)
            .await
            .expect("RentCredit rollback")
            .lamports,
        after_coordinate_close
    );

    let (accepted, _, returned, retire_receipt_cu) =
        submit(&mut context, retire_receipt, table, &addresses)
            .await
            .expect("retire receipt");
    assert!(accepted);
    let receipt = assert_lifecycle_receipt(
        returned,
        &retire_receipt_bytes,
        LifecycleActionV2::RetireReceipt,
    );
    assert_eq!(receipt.credited_lamports(), f.receipt_lamports);
    assert!(account(&mut context, f.receipt_mint).await.is_none());
    assert_eq!(
        account(&mut context, f.rent_credit)
            .await
            .expect("final RentCredit")
            .lamports,
        after_coordinate_close + f.receipt_lamports
    );
    for compute in [
        activate_receipt_cu,
        activate_coordinate_cu,
        retire_coordinate_cu,
        retire_receipt_cu,
    ] {
        assert!(compute <= 1_400_000);
    }
    println!(
        "Rational lifecycle CU: activate_receipt={activate_receipt_cu}, activate_coordinate={activate_coordinate_cu}, retire_coordinate={retire_coordinate_cu}, retire_receipt={retire_receipt_cu}"
    );
}
