//! Real-SBF evidence for the non-production immutable Dealer-policy catalog.
//!
//! The laboratory profile stages a strict `DealerPolicyV1` in contiguous
//! chunks, proves replay refusal and transaction rollback, routes hostile PDA
//! prefunds under the stored rent split, and seals the exact content-addressed
//! catalog account. Production profiles refuse the same allocated action
//! before inspecting any account.

#![cfg_attr(
    not(feature = "profile-non-production-dealer-policy-catalog-lab"),
    allow(dead_code, unused_imports)
)]

use {
    clutch_dealer_runtime_contract::{
        DealerPolicyV1, FixedCodec, Id, DEALER_POLICY_BYTES_V1, MAX_OUTCOMES,
    },
    clutch_sbf::{error::ClutchError, seeds},
    clutch_solana_layout::registry::{
        DealerPolicyAction, ExtensionAction, ExtensionEnvelope, ExtensionFamily,
        DEALER_BEGIN_POLICY_PAYLOAD_BYTES, DEALER_POLICY_ACCOUNT_BYTES,
        DEALER_POLICY_ACCOUNT_HEADER_BYTES, DEALER_POLICY_BODY_BYTES, DEALER_POLICY_CHUNK_BYTES,
        DEALER_POLICY_ID_PAYLOAD_BYTES, DEALER_POLICY_STAGE_ACCOUNT_BYTES,
        DEALER_POLICY_STAGE_HEADER_BYTES, DEALER_WRITE_POLICY_PAYLOAD_BYTES,
    },
    clutch_solana_layout::{Hash32, Intent, MAX_INTENT_BYTES},
    clutch_solana_reference::ExtensionRequest,
    clutch_svm_fixture::{PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const FUNDER_LAMPORTS: u64 = 3_000_000_000;
const SINK_LAMPORTS: u64 = 1_000_000;
const PREFUND: u64 = 1;

fn id(byte: u8) -> Id {
    Id::from_bytes([byte; 32])
}

fn policy() -> DealerPolicyV1 {
    let mut unit_eggs = [0; MAX_OUTCOMES];
    unit_eggs[0] = 10;
    unit_eggs[1] = 10;
    let mut weights = [0; MAX_OUTCOMES];
    weights[0] = 1;
    weights[1] = 1;
    let mut buy = [0; MAX_OUTCOMES];
    buy[0] = 100;
    buy[1] = 100;
    let mut sell = [0; MAX_OUTCOMES];
    sell[0] = 100;
    sell[1] = 100;
    DealerPolicyV1 {
        realm_id: id(1),
        profile_id: id(2),
        market_instance_v2_id: id(3),
        claim_basis_id: id(4),
        collateral_mint: id(5),
        token_program: id(6),
        hoard_custody_semantics_id: id(7),
        relation_v2_id: id(8),
        price_measure_policy_id: id(9),
        curve_policy_id: id(10),
        curve_price_certificate_policy_id: id(11),
        fee_policy_id: id(12),
        liveness_policy_id: id(13),
        retirement_policy_id: id(14),
        neutral_sink: id(72),
        quote_authority: id(15),
        outcome_count: 2,
        payout_denominator: 10,
        capital_unit_cash_atoms: 10,
        capital_unit_eggs: unit_eggs,
        initial_price_denominator: 2,
        initial_price_weights: weights,
        depth_atoms: 1_000,
        max_net_buy: buy,
        max_net_sell: sell,
        minimum_lp_shares: 10,
        maximum_lp_shares: 100,
        funding_deadline_slot: 100,
        trading_open_slot: 100,
        trading_close_slot: 1_000,
        maturity_slot: 2_000,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 2,
        maximum_lp_pages: 4,
    }
}

fn funder() -> Keypair {
    Keypair::new_from_array([
        0x41, 0x11, 0x92, 0xa3, 0x55, 0x0d, 0x73, 0x64, 0x08, 0x7c, 0x39, 0x9e, 0x22, 0xf4, 0x57,
        0x6c, 0x72, 0x45, 0x2b, 0x18, 0x31, 0x8e, 0x3a, 0x14, 0x89, 0x6f, 0x62, 0x91, 0x75, 0x27,
        0x40, 0x5d,
    ])
}

fn donor() -> Keypair {
    Keypair::new_from_array([
        0x52, 0x17, 0x31, 0x08, 0x95, 0x6d, 0x43, 0xa4, 0x20, 0x1c, 0x19, 0x2e, 0x72, 0x84, 0x27,
        0x3c, 0x42, 0x35, 0x6b, 0x78, 0x11, 0x9e, 0x9a, 0x54, 0x29, 0x2f, 0x12, 0xb1, 0x05, 0x67,
        0x20, 0x4d,
    ])
}

fn sink() -> Address {
    Address::new_from_array([72; 32])
}

fn recipient() -> Address {
    Address::new_from_array([91; 32])
}

fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    }
}

fn new_bank(prefund_accounts: &[Address]) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    test.add_account(funder().pubkey(), system_account(FUNDER_LAMPORTS));
    test.add_account(donor().pubkey(), system_account(FUNDER_LAMPORTS));
    test.add_account(sink(), system_account(SINK_LAMPORTS));
    test.add_account(recipient(), system_account(10));
    for address in prefund_accounts {
        test.add_account(*address, system_account(PREFUND));
    }
    test
}

fn request(sequence: u64, action: DealerPolicyAction, payload: &[u8]) -> Vec<u8> {
    let request = ExtensionRequest {
        sequence,
        envelope: ExtensionEnvelope {
            family: ExtensionFamily::Dealer,
            action: ExtensionAction::DealerPolicy(action),
            payload,
        },
    };
    let mut bytes = vec![0; 13 + 402];
    let written = request.encode(&mut bytes).unwrap();
    bytes.truncate(written);
    bytes
}

fn legacy_split_request() -> Vec<u8> {
    let intent = Intent::Split {
        market: Hash32::from_bytes([21; 32]),
        owner: Hash32::from_bytes([22; 32]),
        quantity: 1,
    };
    let mut intent_bytes = [0; MAX_INTENT_BYTES];
    let intent_len = intent.encode(&mut intent_bytes).unwrap();
    let mut bytes = vec![0; 13 + intent_len];
    bytes[0] = 0xd1;
    bytes[1] = 1;
    bytes[10] = 1;
    bytes[11..13].copy_from_slice(&(intent_len as u16).to_le_bytes());
    bytes[13..].copy_from_slice(&intent_bytes[..intent_len]);
    bytes
}

fn begin_payload(
    policy_id: [u8; 32],
    expires_slot: u64,
) -> [u8; DEALER_BEGIN_POLICY_PAYLOAD_BYTES] {
    let mut payload = [0; DEALER_BEGIN_POLICY_PAYLOAD_BYTES];
    payload[..32].copy_from_slice(&policy_id);
    payload[32..64].copy_from_slice(sink().as_ref());
    payload[64..72].copy_from_slice(&expires_slot.to_le_bytes());
    payload
}

fn write_payload(
    policy_id: [u8; 32],
    cursor: usize,
    bytes: &[u8],
) -> [u8; DEALER_WRITE_POLICY_PAYLOAD_BYTES] {
    let mut payload = [0; DEALER_WRITE_POLICY_PAYLOAD_BYTES];
    payload[..32].copy_from_slice(&policy_id);
    payload[32..34].copy_from_slice(&(cursor as u16).to_le_bytes());
    payload[34..36].copy_from_slice(&(bytes.len() as u16).to_le_bytes());
    payload[36..36 + bytes.len()].copy_from_slice(bytes);
    payload
}

fn id_payload(policy_id: [u8; 32]) -> [u8; DEALER_POLICY_ID_PAYLOAD_BYTES] {
    policy_id
}

fn begin_ix(funder: Address, stage: Address, policy_id: [u8; 32]) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &request(
            0,
            DealerPolicyAction::BeginPolicy,
            &begin_payload(policy_id, 10_000),
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn write_ix(
    funder: Address,
    stage: Address,
    policy_id: [u8; 32],
    cursor: usize,
    bytes: &[u8],
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &request(
            cursor as u64,
            DealerPolicyAction::WritePolicy,
            &write_payload(policy_id, cursor, bytes),
        ),
        vec![
            AccountMeta::new_readonly(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn seal_ix(
    funder: Address,
    stage: Address,
    final_account: Address,
    policy_id: [u8; 32],
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &request(
            DEALER_POLICY_BODY_BYTES as u64,
            DealerPolicyAction::SealPolicy,
            &id_payload(policy_id),
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(final_account, false),
            AccountMeta::new(sink(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn abort_ix(caller: Address, stage: Address, funder: Address, policy_id: [u8; 32]) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &request(0, DealerPolicyAction::AbortPolicy, &id_payload(policy_id)),
        vec![
            AccountMeta::new_readonly(caller, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(funder, false),
            AccountMeta::new(sink(), false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

async fn try_send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap()
        .result
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) {
    try_send(context, instructions, signers)
        .await
        .expect("transaction must succeed");
}

async fn account(banks: &mut BanksClient, address: Address) -> Account {
    banks
        .get_account(address)
        .await
        .unwrap()
        .expect("account must exist")
}

fn assert_image_eq(actual: &Account, expected: &Account) {
    assert_eq!(actual.lamports, expected.lamports);
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.owner, expected.owner);
    assert_eq!(actual.executable, expected.executable);
    assert_eq!(actual.rent_epoch, expected.rent_epoch);
}

fn assert_custom(result: Result<(), TransactionError>, index: u8, error: ClutchError) {
    assert_eq!(
        result,
        Err(TransactionError::InstructionError(
            index,
            InstructionError::Custom(error as u32),
        ))
    );
}

#[cfg(not(feature = "profile-non-production-dealer-policy-catalog-lab"))]
#[tokio::test]
async fn production_profile_refuses_allocated_dealer_action_before_accounts() {
    let mut context = new_bank(&[]).start_with_context().await;
    let policy_id = policy().policy_id().unwrap().bytes();
    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &request(
            0,
            DealerPolicyAction::BeginPolicy,
            &begin_payload(policy_id, 10_000),
        ),
        vec![],
    );
    assert_custom(
        try_send(&mut context, &[instruction], &[]).await,
        0,
        ClutchError::UnsupportedInstruction,
    );
}

#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
#[tokio::test]
async fn real_sbf_catalog_is_resumable_rent_exact_and_replay_safe() {
    let uploader = funder();
    let mut body = [0; DEALER_POLICY_BYTES_V1];
    let policy = policy();
    policy.encode_into(&mut body).unwrap();
    let policy_id = policy.policy_id().unwrap().bytes();
    let (stage, _) = Address::find_program_address(
        &[
            seeds::SEED_DEALER_POLICY_STAGE,
            uploader.pubkey().as_ref(),
            &policy_id,
        ],
        &PROGRAM_ID,
    );
    let (final_account, _) =
        Address::find_program_address(&[seeds::SEED_DEALER_POLICY, &policy_id], &PROGRAM_ID);
    // Hostile pre-existing System-owned PDA balances are installed before the
    // bank starts. A one-lamport transfer cannot create a rent-paying account
    // under the current SVM rent-transition rules, but the resulting account
    // image is exactly the hostile prefund state the handler must normalize.
    let mut context = new_bank(&[stage, final_account]).start_with_context().await;
    assert_custom(
        try_send(
            &mut context,
            &[Instruction::new_with_bytes(
                PROGRAM_ID,
                &legacy_split_request(),
                vec![],
            )],
            &[],
        )
        .await,
        0,
        ClutchError::UnsupportedInstruction,
    );

    let stage_before_begin = account(&mut context.banks_client, stage).await;
    let mut wrong_rent = begin_ix(uploader.pubkey(), stage, policy_id);
    wrong_rent.accounts[3] = AccountMeta::new_readonly(CLOCK_SYSVAR, false);
    assert_custom(
        try_send(&mut context, &[wrong_rent], &[&uploader]).await,
        0,
        ClutchError::WrongRentSysvar,
    );
    assert_image_eq(
        &account(&mut context.banks_client, stage).await,
        &stage_before_begin,
    );
    send(
        &mut context,
        &[begin_ix(uploader.pubkey(), stage, policy_id)],
        &[&uploader],
    )
    .await;
    let stage_after_begin = account(&mut context.banks_client, stage).await;
    assert_eq!(stage_after_begin.owner, PROGRAM_ID);
    assert_eq!(
        stage_after_begin.data.len(),
        DEALER_POLICY_STAGE_ACCOUNT_BYTES
    );
    assert_eq!(
        stage_after_begin.lamports,
        Rent::default().minimum_balance(DEALER_POLICY_STAGE_ACCOUNT_BYTES) + PREFUND
    );

    let first = write_ix(
        uploader.pubkey(),
        stage,
        policy_id,
        0,
        &body[..DEALER_POLICY_CHUNK_BYTES],
    );
    send(&mut context, &[first], &[&uploader]).await;
    let stage_after_first = account(&mut context.banks_client, stage).await;
    let mut conflicting_first_chunk = body[..DEALER_POLICY_CHUNK_BYTES].to_vec();
    conflicting_first_chunk[0] ^= 1;
    assert_custom(
        try_send(
            &mut context,
            &[write_ix(
                uploader.pubkey(),
                stage,
                policy_id,
                0,
                &conflicting_first_chunk,
            )],
            &[&uploader],
        )
        .await,
        0,
        ClutchError::Replay,
    );
    assert_image_eq(
        &account(&mut context.banks_client, stage).await,
        &stage_after_first,
    );
    let final_before_owner_swap = account(&mut context.banks_client, final_account).await;
    assert_custom(
        try_send(
            &mut context,
            &[write_ix(
                uploader.pubkey(),
                final_account,
                policy_id,
                DEALER_POLICY_CHUNK_BYTES,
                &body[DEALER_POLICY_CHUNK_BYTES..2 * DEALER_POLICY_CHUNK_BYTES],
            )],
            &[&uploader],
        )
        .await,
        0,
        ClutchError::WrongProgramOwner,
    );
    assert_image_eq(
        &account(&mut context.banks_client, final_account).await,
        &final_before_owner_swap,
    );

    // A successful System transfer before a stale Dealer write must roll back
    // with it. The donor is not the transaction fee payer.
    let donor = donor();
    let donor_before = account(&mut context.banks_client, donor.pubkey()).await;
    let recipient_before = account(&mut context.banks_client, recipient()).await;
    let rollback = vec![
        system_instruction::transfer(&donor.pubkey(), &recipient(), 17),
        write_ix(
            uploader.pubkey(),
            stage,
            policy_id,
            0,
            &body[..DEALER_POLICY_CHUNK_BYTES],
        ),
    ];
    assert_custom(
        try_send(&mut context, &rollback, &[&donor, &uploader]).await,
        1,
        ClutchError::Replay,
    );
    assert_image_eq(
        &account(&mut context.banks_client, donor.pubkey()).await,
        &donor_before,
    );
    assert_image_eq(
        &account(&mut context.banks_client, recipient()).await,
        &recipient_before,
    );
    assert_image_eq(
        &account(&mut context.banks_client, stage).await,
        &stage_after_first,
    );

    let mut cursor = DEALER_POLICY_CHUNK_BYTES;
    while cursor < body.len() {
        let end = core::cmp::min(cursor + DEALER_POLICY_CHUNK_BYTES, body.len());
        send(
            &mut context,
            &[write_ix(
                uploader.pubkey(),
                stage,
                policy_id,
                cursor,
                &body[cursor..end],
            )],
            &[&uploader],
        )
        .await;
        cursor = end;
    }
    let completed = account(&mut context.banks_client, stage).await;
    assert_eq!(&completed.data[DEALER_POLICY_STAGE_HEADER_BYTES..], &body);

    let sink_before = account(&mut context.banks_client, sink()).await;
    let mut wrong_sink = seal_ix(uploader.pubkey(), stage, final_account, policy_id);
    wrong_sink.accounts[3] = AccountMeta::new(recipient(), false);
    assert_custom(
        try_send(&mut context, &[wrong_sink], &[&uploader]).await,
        0,
        ClutchError::DealerPolicyUploadMismatch,
    );
    assert_image_eq(&account(&mut context.banks_client, stage).await, &completed);
    assert_image_eq(
        &account(&mut context.banks_client, final_account).await,
        &final_before_owner_swap,
    );
    send(
        &mut context,
        &[seal_ix(uploader.pubkey(), stage, final_account, policy_id)],
        &[&uploader],
    )
    .await;

    let final_image = account(&mut context.banks_client, final_account).await;
    assert_eq!(final_image.owner, PROGRAM_ID);
    assert_eq!(final_image.data.len(), DEALER_POLICY_ACCOUNT_BYTES);
    assert_eq!(
        final_image.lamports,
        Rent::default().minimum_balance(DEALER_POLICY_ACCOUNT_BYTES)
    );
    assert_eq!(
        &final_image.data[DEALER_POLICY_ACCOUNT_HEADER_BYTES..],
        &body
    );
    assert_eq!(
        u64::from_le_bytes(final_image.data[48..56].try_into().unwrap()),
        PREFUND
    );
    assert_eq!(
        account(&mut context.banks_client, sink()).await.lamports,
        sink_before.lamports + (2 * PREFUND),
    );
    assert!(context
        .banks_client
        .get_account(stage)
        .await
        .unwrap()
        .is_none());

    let before_replay = final_image.clone();
    assert!(try_send(
        &mut context,
        &[seal_ix(uploader.pubkey(), stage, final_account, policy_id)],
        &[&uploader],
    )
    .await
    .is_err());
    assert_image_eq(
        &account(&mut context.banks_client, final_account).await,
        &before_replay,
    );

    let mut abandoned = policy;
    abandoned.quote_authority = id(16);
    let abandoned_id = abandoned.policy_id().unwrap().bytes();
    let (abandoned_stage, _) = Address::find_program_address(
        &[
            seeds::SEED_DEALER_POLICY_STAGE,
            uploader.pubkey().as_ref(),
            &abandoned_id,
        ],
        &PROGRAM_ID,
    );
    send(
        &mut context,
        &[begin_ix(uploader.pubkey(), abandoned_stage, abandoned_id)],
        &[&uploader],
    )
    .await;
    send(
        &mut context,
        &[abort_ix(
            uploader.pubkey(),
            abandoned_stage,
            uploader.pubkey(),
            abandoned_id,
        )],
        &[&uploader],
    )
    .await;
    assert!(context
        .banks_client
        .get_account(abandoned_stage)
        .await
        .unwrap()
        .is_none());
}
