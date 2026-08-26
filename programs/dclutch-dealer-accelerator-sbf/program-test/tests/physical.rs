//! Real-ELF transport refusal and rollback evidence for the Dealer accelerator.

use std::vec::Vec;

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_dealer_accelerator_test_caller_sbf::dealer_accelerator_test_caller_authority_v1;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorRequestV2, RequestTransportV2,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const ACCELERATOR: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CALLER: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REQUEST_ACCOUNT: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const OBSERVED: Pubkey = Pubkey::new_from_array([0xd4; 32]);
const DUMMY: Pubkey = Pubkey::new_from_array([0xd5; 32]);

fn content(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero fixture content")
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

fn malformed_frame_fixture() -> (ProgramTest, Instruction, Vec<u8>) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("dclutch_dealer_accelerator_sbf", ACCELERATOR, None);
    test.add_program("dclutch_dealer_accelerator_test_caller_sbf", CALLER, None);
    let observed = vec![0x5a; 96];
    add_account(&mut test, OBSERVED, CALLER, observed.clone());
    add_account(&mut test, DUMMY, system_program::ID, Vec::new());

    let bank = [0_u8; 8];
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        content(5),
        1,
        1,
        0,
        0,
        &bank,
    )
    .expect("canonical request");
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + bank.len()];
    request
        .encode_into(&mut request_bytes)
        .expect("request encoding");

    let family = [0_u8; 4];
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family.len()).expect("family width"),
        [1; 32],
        [2; 32],
        1,
        hash(&observed).to_bytes(),
    )
    .expect("Hot envelope");
    let mut top_level_data = envelope.to_bytes().to_vec();
    top_level_data.extend_from_slice(&family);
    let (authority, _, _) = dealer_accelerator_test_caller_authority_v1(
        &CALLER,
        &top_level_data,
        &OBSERVED,
        &request_bytes,
    )
    .expect("canonical caller authority");
    add_account(&mut test, authority, system_program::ID, Vec::new());
    add_account(&mut test, REQUEST_ACCOUNT, CALLER, request_bytes);
    let instruction = Instruction {
        program_id: CALLER,
        accounts: vec![
            AccountMeta::new_readonly(REQUEST_ACCOUNT, false),
            AccountMeta::new_readonly(ACCELERATOR, false),
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(DUMMY, false),
            AccountMeta::new_readonly(OBSERVED, false),
        ],
        data: top_level_data,
    };
    (test, instruction, observed)
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<solana_program_test::BanksTransactionResultWithMetadata, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
}

#[tokio::test]
async fn real_elf_rejects_a_truncated_hot_frame_without_mutation() {
    let (test, instruction, observed_before) = malformed_frame_fixture();
    let mut context = test.start_with_context().await;
    let result = submit(&mut context, instruction)
        .await
        .expect("ProgramTest processing");
    assert!(result.result.is_err(), "truncated frame must fail closed");
    let observed_after = context
        .banks_client
        .get_account(OBSERVED)
        .await
        .expect("observed account query")
        .expect("observed account");
    assert_eq!(observed_after.data, observed_before);
}
