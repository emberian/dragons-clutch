//! Real-ELF admission, rollback, replay-refusal, and rent-close evidence.
//!
//! With `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` set the campaign also emits the
//! finalized transactions the gauntlet's census folds into the execution
//! ledger. See `tools/gauntlet/claims-custody/README.md`.

use dclutch_program_test_evidence::TransactionEvidence;
use std::{env, fs, path::PathBuf, vec::Vec};


use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, compile_product_lbv2_fixture_v2,
};
use dclutch_claims_sbf::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2,
    PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionCloseReceiptV2,
    ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    ProtocolPositionSeedsV2,
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
use solana_account::Account;
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
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::versioned::VersionedTransaction;

const CLAIMS: Pubkey = Pubkey::new_from_array([0xb1; 32]);
const REGISTRY: Pubkey = Pubkey::new_from_array([0xb3; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xb4; 32]);
const TRADING: Pubkey = Pubkey::new_from_array([0xb5; 32]);
const RENT_PROGRAM: Pubkey = Pubkey::new_from_array([0xb6; 32]);
const GENERATION: u64 = 23;

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
        trading: read("dclutch_claims_liability_basis_test_caller_sbf.so"),
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
    graph: dclutch_claims_affine_batch_program_test::fixture::ProductLbv2FixtureV2,
}

fn fixture() -> (ProgramTest, Fixture) {
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
    let (release, cache_bytes) = activation(&artifacts);
    let cache = Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release], &REGISTRY).0;
    add_account(&mut test, cache, REGISTRY, cache_bytes, 1);
    let owner = Pubkey::new_from_array([0xd1; 32]);
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
    add_account(&mut test, owner, TRADING, vec![1], 1);
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
            graph,
        },
    )
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

async fn process_legacy(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
) {
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
    let failure = processed.result.clone().err().map(|error| format!("{error:?}"));
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
