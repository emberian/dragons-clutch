//! Real-ELF N=258 Fractional wrap through the canonical Claims SignedDeltaV3 route.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, ProductLbv2FixtureV2,
    compile_product_lbv2_fixture_v2,
};
use dclutch_claims_svm::signed_delta_v3::{
    DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanV3, SignedDeltaV3,
};
use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    NO_TERMINAL_OUTCOME_V1,
};
use dclutch_fractional_claims_kernel::{
    FractionalSignedDeltaInputV1, FractionalSignedDeltaLoweringV1,
    fractional_signed_delta_shape_v1, lower_fractional_signed_delta_v1,
};
use dclutch_fractional_signed_delta_test_caller_sbf::FRACTIONAL_SIGNED_DELTA_TEST_WRAPPER_BYTES;
use dclutch_program_test_evidence::TransactionEvidence;
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
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_transaction::{Transaction, versioned::VersionedTransaction};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa8; 32]);
const ACTOR_OWNER: Pubkey = Pubkey::new_from_array([0xa6; 32]);
const RESERVE_OWNER: Pubkey = Pubkey::new_from_array([0xa7; 32]);
const GENERATION: u64 = 37;
const WRAP_QUANTITY: u64 = 7;

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    caller: Vec<u8>,
}

struct Fixture {
    shared: ProductLbv2FixtureV2,
    activation_cache: Pubkey,
    caller_authority: Pubkey,
    request: FractionalFamilyRequestV1,
    wrapper: Vec<u8>,
    expected: ExpectedClaimsEffect,
}

struct ExpectedClaimsEffect {
    lowering: FractionalSignedDeltaLoweringV1,
    packet: Vec<u8>,
    market: Vec<u8>,
    positions: [Vec<u8>; 2],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaimsSnapshot {
    market: Account,
    positions: [Account; 2],
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        caller: read("dclutch_fractional_signed_delta_test_caller_sbf.so"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
    );
}

fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic_seed; 32]).expect("semantic release"),
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
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
}

fn activation_cache(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x31, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims);
    let trading = release(TEST_CALLER_PROGRAM_ID, 0x33, &artifacts.caller);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(claims),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, claims),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn add_finalized(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone());
    add_account(test, record.staging, system_program::ID, Vec::new());
}

fn request(shared: &ProductLbv2FixtureV2, release_set: [u8; 32]) -> FractionalFamilyRequestV1 {
    FractionalFamilyRequestV1::new(
        FractionalActionV1::Wrap,
        FractionalFamilyRequestInputV1 {
            release_set,
            market: shared.core_market.to_bytes(),
            product_record: shared.product.digest,
            result_domain: shared.result_domain.digest,
            terms: [0x71; 32],
            token_behavior: [0x72; 32],
            owner: ACTOR_OWNER.to_bytes(),
            source_token_account: [0; 32],
            destination_token_account: [0x73; 32],
            terminal_digest: [0; 32],
            expected_revision: 0,
            quantity: WRAP_QUANTITY,
            outcome: 0,
            terminal_outcome: NO_TERMINAL_OUTCOME_V1,
        },
    )
    .expect("canonical Fractional wrap request")
}

fn lower_expected(
    shared: &ProductLbv2FixtureV2,
    request: FractionalFamilyRequestV1,
) -> ExpectedClaimsEffect {
    let input = FractionalSignedDeltaInputV1 {
        request,
        semantic_product_id: shared.product_id,
        market_account: shared.claims_market.to_bytes(),
        market_bytes: &shared.claims_market_bytes,
        linked_basis_record_digest: shared.linked_basis.digest,
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        reserve_owner: RESERVE_OWNER.to_bytes(),
        reserve_position_bytes: &shared.positions[1].bytes,
        actor_position_bytes: Some(&shared.positions[0].bytes),
        native_claims: WRAP_QUANTITY,
        collateral_atoms: 0,
        expected_post_reserve_native_claims: Some(WRAP_QUANTITY),
        retirement_native_burns: &[],
        post_fractional_revision: 1,
    };
    let shape = fractional_signed_delta_shape_v1(input).expect("runtime shape");
    assert_eq!(shape.claim_count(), 258);
    assert_eq!(shape.position_count(), 2);
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral");
    let mut aggregate = vec![neutral; usize::try_from(shape.claim_count()).expect("claim width")];
    let dummy = PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 1).expect("dummy debit"),
        },
        shape.position_count(),
        shape.claim_count(),
    )
    .expect("dummy row");
    let mut rows = vec![dummy; usize::try_from(shape.position_delta_count()).expect("row width")];
    let mut packet_scratch = vec![0; shape.packet_bytes()];
    let mut packet = vec![0; shape.packet_bytes()];
    let mut market = vec![0; shared.claims_market_bytes.len()];
    let mut first = vec![0; shared.positions[0].bytes.len()];
    let mut second = vec![0; shared.positions[1].bytes.len()];
    let lowering = lower_fractional_signed_delta_v1(
        input,
        &mut aggregate,
        &mut rows,
        &mut packet_scratch,
        &mut packet,
        &mut market,
        &mut [first.as_mut_slice(), second.as_mut_slice()],
    )
    .expect("exact Fractional lowering");
    assert_eq!(
        SignedDeltaPlanV3::decode(&packet)
            .expect("plan")
            .claim_count(),
        258
    );
    ExpectedClaimsEffect {
        lowering,
        packet,
        market,
        positions: [first, second],
    }
}

fn wrapper_bytes(
    fail_after: bool,
    shared: &ProductLbv2FixtureV2,
    request: FractionalFamilyRequestV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(FRACTIONAL_SIGNED_DELTA_TEST_WRAPPER_BYTES);
    bytes.push(u8::from(fail_after));
    bytes.extend_from_slice(&request.to_bytes());
    bytes.extend_from_slice(&shared.product_id);
    bytes.extend_from_slice(&shared.linked_basis.digest);
    bytes.extend_from_slice(&RESERVE_OWNER.to_bytes());
    bytes.extend_from_slice(&WRAP_QUANTITY.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&WRAP_QUANTITY.to_le_bytes());
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    assert_eq!(bytes.len(), FRACTIONAL_SIGNED_DELTA_TEST_WRAPPER_BYTES);
    bytes
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_fractional_signed_delta_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    let (release_set, cache_bytes) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache,
        REGISTRY_PROGRAM_ID,
        cache_bytes,
    );
    let shared = compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set,
        realm_id: [0x61; 32],
        custody_context: [0x62; 32],
        generation: GENERATION,
        source_owner: ACTOR_OWNER,
        destination_owner: RESERVE_OWNER,
    })
    .expect("shared N=258 Product/LBV2 fixture");
    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.substituted_product,
        &shared.substituted_portfolio,
        &shared.linked_basis,
        &shared.substituted_linked_basis,
    ] {
        add_finalized(&mut test, record);
    }
    add_account(
        &mut test,
        shared.core_market,
        CORE_PROGRAM_ID,
        shared.core_state.clone(),
    );
    add_account(
        &mut test,
        shared.claims_market,
        CLAIMS_PROGRAM_ID,
        shared.claims_market_bytes.clone(),
    );
    for position in &shared.positions {
        add_account(
            &mut test,
            position.account,
            CLAIMS_PROGRAM_ID,
            position.bytes.clone(),
        );
    }
    let request = request(&shared, release_set);
    let expected = lower_expected(&shared, request);
    let plan = SignedDeltaPlanV3::decode(&expected.packet).expect("expected plan");
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        plan.market(),
        ExecutionRoleV1::Trading,
        plan.request_id(),
        hash(&expected.packet).to_bytes(),
    )
    .expect("caller authority seeds");
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &TEST_CALLER_PROGRAM_ID).0;
    add_account(&mut test, caller_authority, system_program::ID, Vec::new());
    let wrapper = wrapper_bytes(false, &shared, request);
    (
        test,
        Fixture {
            shared,
            activation_cache,
            caller_authority,
            request,
            wrapper,
            expected,
        },
    )
}

fn child_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    let shared = &fixture.shared;
    vec![
        AccountMeta::new_readonly(fixture.caller_authority, false),
        AccountMeta::new(shared.claims_market, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(TEST_CALLER_PROGRAM_ID), false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new(shared.positions[0].account, false),
        AccountMeta::new(shared.positions[1].account, false),
    ]
}

fn instruction(fixture: &Fixture, fail_after: bool) -> Instruction {
    let mut accounts = Vec::with_capacity(23);
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(child_accounts(fixture));
    let mut data = fixture.wrapper.clone();
    *data.first_mut().expect("failure flag") = u8::from(fail_after);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn substitute_child(instruction: &mut Instruction, child_index: usize, key: Pubkey) {
    let outer_index = child_index.checked_add(1).expect("outer child offset");
    instruction
        .accounts
        .get_mut(outer_index)
        .expect("child coordinate")
        .pubkey = key;
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> ClaimsSnapshot {
    ClaimsSnapshot {
        market: account(context, fixture.shared.claims_market).await,
        positions: [
            account(context, fixture.shared.positions[0].account).await,
            account(context, fixture.shared.positions[1].account).await,
        ],
    }
}

fn lookup_addresses(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer && !addresses.contains(&instruction.program_id) {
            addresses.push(instruction.program_id);
        }
        for account in &instruction.accounts {
            if account.pubkey != payer && !addresses.contains(&account.pubkey) {
                addresses.push(account.pubkey);
            }
        }
    }
    addresses
}

const PACKET_DATA_BYTES: usize = 1_232;

/// The extent of a legacy or v0 message once signed, checked against Solana's
/// packet maximum. `solana-program-test` submits no packet and cannot enforce
/// this itself -- Found31 was ten bytes over and survived every fixture test --
/// so the campaign measures it directly.
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
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
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
    process_legacy(
        context,
        create,
        "claims fractional-signed-delta: create lookup table",
    )
    .await;
    for (index, chunk) in addresses.chunks(20).enumerate() {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            &format!("claims fractional-signed-delta: extend lookup table {index}"),
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

async fn submit_v0(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<(bool, u64, Vec<String>), BanksClientError> {
    let blockhash: Hash = context.banks_client.get_latest_blockhash().await?;
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
        VersionedTransaction::try_new(message, &[&context.payer]).expect("signed v0 transaction");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = wire_extent(transaction.signatures.len(), &transaction.message.serialize());
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
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (compute, logs) = processed
        .metadata
        .map(|metadata| (metadata.compute_units_consumed, metadata.log_messages))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Ok((accepted, compute, logs))
}

#[tokio::test]
async fn real_sbf_fractional_wrap_lowers_n258_and_rolls_back_after_late_refusal() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let direct = instruction(&fixture, false);
    let late = instruction(&fixture, true);
    let mut substituted_basis = direct.clone();
    substitute_child(
        &mut substituted_basis,
        2,
        fixture.shared.substituted_linked_basis.raw,
    );
    substitute_child(
        &mut substituted_basis,
        3,
        fixture.shared.substituted_linked_basis.staging,
    );
    let instructions = [direct.clone(), late.clone(), substituted_basis.clone()];
    let addresses = lookup_addresses(context.payer.pubkey(), &instructions);
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let before = snapshot(&mut context, &fixture).await;

    let (accepted, _, _) = submit_v0(
        &mut context,
        substituted_basis,
        table,
        &addresses,
        "claims fractional-signed-delta: wrap against a substituted linked basis",
    )
    .await
    .expect("substituted-basis transaction");
    assert!(
        !accepted,
        "same-width different-Product linked basis must refuse"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);

    let (accepted, late_compute, logs) = submit_v0(
        &mut context,
        late,
        table,
        &addresses,
        "claims fractional-signed-delta: caller refuses after a complete wrap",
    )
    .await
    .expect("late-refusal transaction");
    assert!(
        !accepted,
        "caller must deliberately refuse after Claims success"
    );
    assert!(
        logs.iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims must return before late refusal: {logs:#?}"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);

    let (accepted, success_compute, logs) = submit_v0(
        &mut context,
        direct.clone(),
        table,
        &addresses,
        "claims fractional-signed-delta: canonical wrap commits",
    )
    .await
    .expect("canonical Fractional transaction");
    assert!(
        accepted,
        "canonical real-SBF Fractional wrap must commit: {logs:#?}"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(after.market.data, fixture.expected.market);
    assert_eq!(after.positions[0].data, fixture.expected.positions[0]);
    assert_eq!(after.positions[1].data, fixture.expected.positions[1]);
    assert_eq!(fixture.expected.lowering.native_claims(), WRAP_QUANTITY);
    assert_eq!(fixture.expected.lowering.collateral_atoms(), 0);

    let (accepted, _, _) = submit_v0(
        &mut context,
        direct,
        table,
        &addresses,
        "claims fractional-signed-delta: stale wrap refuses",
    )
    .await
    .expect("stale Fractional transaction");
    assert!(!accepted, "stale Claims revisions must refuse");
    assert_eq!(snapshot(&mut context, &fixture).await, after);
    println!(
        "fractional SignedDeltaV3 N=258 compute units: success={success_compute}, late_rollback={late_compute}, packet_bytes={}",
        fixture.expected.packet.len()
    );
    let _ = fixture.request;
}
