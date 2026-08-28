//! Real-SBF two-party admission, signer refusal, replay, and atomic-rent evidence.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, compile_product_lbv2_fixture_v2,
};
use dclutch_claims_svm::{
    liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_core_contract::ContentId;
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
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_user_position_admission_contract::{
    ProtocolPositionActionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
    USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1, UserPositionAdmissionRequestV1,
};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
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
const GENERATION: u64 = 23;
const SEED_COUNT: usize = 20;
const COMPUTE_LIMIT: u64 = 1_400_000;
const PACKET_DATA_BYTES: usize = 1_232;

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    trading: Vec<u8>,
    rent: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let read = |name: &str| fs::read(directory.join(name)).expect("real ELF");
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        trading: read("dclutch_trading_sbf.so"),
        rent: read("dclutch_rent_sbf.so"),
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

struct UserRoute {
    owner: Keypair,
    position: Pubkey,
    admission: Pubkey,
}

struct Fixture {
    release: [u8; 32],
    cache: Pubkey,
    core_market: Pubkey,
    market: Pubkey,
    rent_credit: Pubkey,
    position_rent: u64,
    admission_rent: u64,
    graph: dclutch_claims_affine_batch_program_test::fixture::ProductLbv2FixtureV2,
    users: Vec<UserRoute>,
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(COMPUTE_LIMIT);
    for (name, id, elf) in [
        ("dclutch_claims_sbf", CLAIMS, artifacts.claims.as_slice()),
        (
            "dclutch_registry_sbf",
            REGISTRY,
            artifacts.registry.as_slice(),
        ),
        ("dclutch_core_sbf", CORE, artifacts.core.as_slice()),
        ("dclutch_trading_sbf", TRADING, artifacts.trading.as_slice()),
        ("dclutch_rent_sbf", RENT_PROGRAM, artifacts.rent.as_slice()),
    ] {
        add_program(&mut test, name, id, elf);
    }
    let (release, cache_bytes) = activation(&artifacts);
    let cache = Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release], &REGISTRY).0;
    add_account(&mut test, cache, REGISTRY, cache_bytes, 1);
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

    let position_bytes = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
        .checked_add(
            usize::try_from(graph.outcome_count)
                .expect("outcome width")
                .checked_mul(8)
                .expect("position vector"),
        )
        .expect("position width");
    let position_rent = Rent::default().minimum_balance(position_bytes);
    let admission_rent = Rent::default().minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    let mut users = Vec::with_capacity(SEED_COUNT);
    for seed in 0..SEED_COUNT {
        let byte = u8::try_from(seed + 1).expect("bounded seed");
        let owner = Keypair::new_from_array([byte; 32]);
        add_account(&mut test, owner.pubkey(), system_program::ID, Vec::new(), 1);
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(
                graph.claims_market.to_bytes(),
                owner.pubkey().to_bytes(),
            )
            .expect("position seeds")
            .as_slices(),
            &CLAIMS,
        )
        .0;
        let admission = Pubkey::find_program_address(
            &ProtocolPositionAdmissionSeedsV2::new(
                graph.claims_market.to_bytes(),
                owner.pubkey().to_bytes(),
            )
            .expect("admission seeds")
            .as_slices(),
            &CLAIMS,
        )
        .0;
        users.push(UserRoute {
            owner,
            position,
            admission,
        });
    }
    (
        test,
        Fixture {
            release,
            cache,
            core_market: graph.core_market,
            market: graph.claims_market,
            rent_credit,
            position_rent,
            admission_rent,
            graph,
            users,
        },
    )
}

fn request(fixture: &Fixture, user: &UserRoute) -> ProtocolPositionRequestV2 {
    let parent_request_digest = hashv(&[
        b"dclutch/program-test/user-position-admission/v1",
        user.owner.pubkey().as_ref(),
    ])
    .to_bytes();
    ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: fixture.release,
        market: fixture.core_market.to_bytes(),
        position_owner: user.owner.pubkey().to_bytes(),
        parent_request_digest,
        rent_credit: fixture.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM.to_bytes(),
        generation: GENERATION,
        expected_market_revision: 0,
        expected_position_revision: 0,
        observed_position_lamports: fixture.position_rent,
        observed_admission_lamports: fixture.admission_rent,
        position_rent_principal: fixture.position_rent,
        admission_rent_principal: fixture.admission_rent,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
}

fn admission_instruction(
    fixture: &Fixture,
    user: &UserRoute,
    request: ProtocolPositionRequestV2,
    owner_meta: Pubkey,
) -> Instruction {
    let child = request.to_bytes().expect("child request");
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.position_owner,
        hash(&child).to_bytes(),
    )
    .expect("authority seeds");
    let authority = Pubkey::find_program_address(&authority_seeds.as_slices(), &TRADING).0;
    let data = UserPositionAdmissionRequestV1::new(request)
        .expect("outer request")
        .to_bytes()
        .expect("outer bytes")
        .to_vec();
    let accounts = vec![
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new(user.position, false),
        AccountMeta::new(user.admission, false),
        AccountMeta::new_readonly(fixture.graph.linked_basis.raw, false),
        AccountMeta::new_readonly(fixture.graph.linked_basis.staging, false),
        AccountMeta::new_readonly(fixture.graph.product.raw, false),
        AccountMeta::new_readonly(fixture.graph.product.staging, false),
        AccountMeta::new_readonly(fixture.graph.result_domain.raw, false),
        AccountMeta::new_readonly(fixture.graph.result_domain.staging, false),
        AccountMeta::new_readonly(fixture.graph.portfolio.raw, false),
        AccountMeta::new_readonly(fixture.graph.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(fixture.core_market, false),
        AccountMeta::new_readonly(fixture.cache, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new_readonly(CORE, false),
        AccountMeta::new_readonly(programdata(CORE), false),
        AccountMeta::new_readonly(owner_meta, true),
        AccountMeta::new_readonly(fixture.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM, false),
    ];
    assert_eq!(accounts.len(), USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1);
    Instruction {
        program_id: TRADING,
        accounts,
        data,
    }
}

fn atomic_admission(
    payer: Pubkey,
    fixture: &Fixture,
    user: &UserRoute,
    outer: Instruction,
) -> Vec<Instruction> {
    vec![
        transfer(&payer, &user.position, fixture.position_rent),
        transfer(&payer, &user.admission, fixture.admission_rent),
        outer,
    ]
}

fn lookup_addresses(payer: Pubkey, instruction_sets: &[Vec<Instruction>]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instruction_sets.iter().flatten() {
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
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("ALT lifecycle");
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
    process_legacy(context, create).await;
    for chunk in addresses.chunks(20) {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
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

struct Submission {
    accepted: bool,
    logs: Vec<String>,
    returned: Option<(Pubkey, Vec<u8>)>,
    compute_units: u64,
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    table: Pubkey,
    addresses: &[Pubkey],
    owner: Option<&Keypair>,
) -> Result<Submission, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            instructions,
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let signers: Vec<&dyn Signer> = match owner {
        Some(owner) => vec![&context.payer, owner],
        None => vec![&context.payer],
    };
    let transaction = VersionedTransaction::try_new(message, &signers).expect("transaction");
    let wire_extent = 1_usize
        .checked_add(
            transaction
                .signatures
                .len()
                .checked_mul(64)
                .expect("signature width"),
        )
        .and_then(|value| value.checked_add(transaction.message.serialize().len()))
        .expect("wire extent");
    assert!(
        wire_extent <= PACKET_DATA_BYTES,
        "wire extent {wire_extent}"
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let accepted = processed.result.is_ok();
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
    Ok(Submission {
        accepted,
        logs,
        returned,
        compute_units,
    })
}

async fn require_vacant(context: &mut ProgramTestContext, user: &UserRoute) {
    assert!(
        context
            .banks_client
            .get_account(user.position)
            .await
            .expect("read position")
            .is_none()
    );
    assert!(
        context
            .banks_client
            .get_account(user.admission)
            .await
            .expect("read admission")
            .is_none()
    );
}

#[tokio::test]
async fn real_trading_admits_twenty_users_and_rolls_back_every_hostile_prefund() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();

    let canonical_sets: Vec<Vec<Instruction>> = fixture
        .users
        .iter()
        .map(|user| {
            let outer =
                admission_instruction(&fixture, user, request(&fixture, user), user.owner.pubkey());
            atomic_admission(payer, &fixture, user, outer)
        })
        .collect();

    let mut missing_signature = canonical_sets.first().expect("first user").clone();
    missing_signature
        .last_mut()
        .expect("outer")
        .accounts
        .get_mut(USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1)
        .expect("owner meta")
        .is_signer = false;

    let first = fixture.users.first().expect("first user");
    let second = fixture.users.get(1).expect("second user");
    let substituted_owner_outer = admission_instruction(
        &fixture,
        first,
        request(&fixture, first),
        second.owner.pubkey(),
    );
    let substituted_owner = atomic_admission(payer, &fixture, first, substituted_owner_outer);

    let mut stale_authority = canonical_sets.first().expect("first user").clone();
    *stale_authority
        .last_mut()
        .expect("outer")
        .data
        .get_mut(8 + 112)
        .expect("parent request byte") ^= 1;

    let mut wrong_rent_request = request(&fixture, first);
    wrong_rent_request.observed_position_lamports = wrong_rent_request
        .observed_position_lamports
        .checked_add(1)
        .expect("hostile rent");
    let wrong_rent_outer =
        admission_instruction(&fixture, first, wrong_rent_request, first.owner.pubkey());
    let wrong_rent = atomic_admission(payer, &fixture, first, wrong_rent_outer);

    let mut all_sets = canonical_sets.clone();
    all_sets.extend([
        missing_signature.clone(),
        substituted_owner.clone(),
        stale_authority.clone(),
        wrong_rent.clone(),
    ]);
    let addresses = lookup_addresses(payer, &all_sets);
    let table = create_live_lookup_table(&mut context, &addresses).await;

    let missing = submit(&mut context, &missing_signature, table, &addresses, None)
        .await
        .expect("missing signature submission");
    assert!(!missing.accepted);
    require_vacant(&mut context, first).await;

    let substituted = submit(
        &mut context,
        &substituted_owner,
        table,
        &addresses,
        Some(&second.owner),
    )
    .await
    .expect("substituted owner submission");
    assert!(!substituted.accepted);
    require_vacant(&mut context, first).await;

    let stale = submit(
        &mut context,
        &stale_authority,
        table,
        &addresses,
        Some(&first.owner),
    )
    .await
    .expect("stale authority submission");
    assert!(!stale.accepted);
    require_vacant(&mut context, first).await;

    let child_refusal = submit(
        &mut context,
        &wrong_rent,
        table,
        &addresses,
        Some(&first.owner),
    )
    .await
    .expect("Claims rent refusal submission");
    assert!(!child_refusal.accepted);
    assert!(
        child_refusal
            .logs
            .iter()
            .any(|line| line.contains(&CLAIMS.to_string()))
    );
    require_vacant(&mut context, first).await;

    let mut pass_count = 0_u64;
    let mut compute_total = 0_u64;
    for (seed, (user, instructions)) in fixture.users.iter().zip(canonical_sets.iter()).enumerate()
    {
        let result = submit(
            &mut context,
            instructions,
            table,
            &addresses,
            Some(&user.owner),
        )
        .await
        .expect("canonical admission submission");
        assert!(result.accepted, "seed {seed} refused: {:?}", result.logs);
        assert!(result.compute_units <= COMPUTE_LIMIT);
        let (producer, receipt_bytes) = result.returned.expect("Claims admission receipt");
        assert_eq!(producer, CLAIMS);
        let receipt = ProtocolPositionAdmissionV2::decode_receipt(&receipt_bytes)
            .expect("canonical admission receipt");
        assert_eq!(receipt.position_owner(), user.owner.pubkey().to_bytes());
        assert_eq!(receipt.outcome_count(), fixture.graph.outcome_count);
        let position = context
            .banks_client
            .get_account(user.position)
            .await
            .expect("read position")
            .expect("admitted position");
        let admission = context
            .banks_client
            .get_account(user.admission)
            .await
            .expect("read admission")
            .expect("admission state");
        assert_eq!(position.owner, CLAIMS);
        assert_eq!(admission.owner, CLAIMS);
        assert_eq!(position.lamports, fixture.position_rent);
        assert_eq!(admission.lamports, fixture.admission_rent);
        pass_count = pass_count.checked_add(1).expect("pass count");
        compute_total = compute_total
            .checked_add(result.compute_units)
            .expect("CU total");
    }

    let before_replay = context
        .banks_client
        .get_account(first.position)
        .await
        .expect("read replay prestate")
        .expect("first Position");
    let replay_outer = canonical_sets
        .first()
        .and_then(|set| set.last())
        .expect("replay outer")
        .clone();
    let replay = submit(
        &mut context,
        &[replay_outer],
        table,
        &addresses,
        Some(&first.owner),
    )
    .await
    .expect("replay submission");
    assert!(!replay.accepted);
    assert_eq!(
        context
            .banks_client
            .get_account(first.position)
            .await
            .expect("read replay poststate")
            .expect("first Position"),
        before_replay
    );

    let mean = compute_total
        .checked_div(pass_count)
        .expect("nonzero passes");
    println!(
        "user Position admission CU: PASS {pass_count}/{SEED_COUNT}; MEAN {mean} CU (of {COMPUTE_LIMIT})"
    );
    assert_eq!(
        usize::try_from(pass_count).expect("pass count usize"),
        SEED_COUNT
    );
}
