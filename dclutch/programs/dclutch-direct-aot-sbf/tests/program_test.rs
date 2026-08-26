//! Real-ELF execution evidence for the stateless Direct AOT accelerator.

use dclutch_core_contract::ContentId;
use dclutch_direct_aot_contract::*;
use dclutch_direct_aot_sbf::{
    DIRECT_AOT_ACCEPTED_ACK_BYTES_V1, DIRECT_AOT_BANK_BYTES_V1, DIRECT_AOT_IDENTITIES_V1,
    DIRECT_AOT_REQUEST_BYTES_V1, DIRECT_AOT_SCALARS_V1,
};
use dclutch_execution_strategy_contract::{
    AcceleratorAckV1, AcceleratorRequestV1, ExecutionDispositionV1, encode_register_bank_into,
};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::ProgramTest;
use solana_sdk::signature::Signer;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa7; 32]);

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("content")
}

fn request(fill: u64) -> [u8; DIRECT_AOT_REQUEST_BYTES_V1] {
    let mut scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    scalars[SCALAR_PHASE] = OPEN_PHASE_V2;
    scalars[SCALAR_SLOT] = 100;
    scalars[SCALAR_SELLER_FROM] = 90;
    scalars[SCALAR_SELLER_THROUGH] = 110;
    scalars[SCALAR_BUYER_FROM] = 95;
    scalars[SCALAR_BUYER_THROUGH] = 120;
    scalars[SCALAR_SELLER_SIDE] = SELL_SIDE_V2;
    scalars[SCALAR_BUYER_SIDE] = BUY_SIDE_V2;
    scalars[SCALAR_SELLER_GENERATION] = 3;
    scalars[SCALAR_BUYER_GENERATION] = 3;
    scalars[SCALAR_SELLER_OUTCOME] = 1;
    scalars[SCALAR_BUYER_OUTCOME] = 1;
    scalars[SCALAR_OUTCOME_COUNT] = 2;
    scalars[SCALAR_SELLER_LIFECYCLE] = 0;
    scalars[SCALAR_SELLER_MAXIMUM] = 2_000;
    scalars[SCALAR_BUYER_LIFECYCLE] = 0;
    scalars[SCALAR_BUYER_MAXIMUM] = 2_000;
    scalars[SCALAR_SELLER_LIMIT] = 400_000;
    scalars[SCALAR_EXECUTION_PRICE] = 500_000;
    scalars[SCALAR_BUYER_LIMIT] = 600_000;
    scalars[SCALAR_PRICE_SCALE] = 1_000_000;
    scalars[SCALAR_SELLER_FEE_BPS] = 25;
    scalars[SCALAR_BUYER_FEE_BPS] = 25;
    scalars[SCALAR_POLICY_FEE_BPS] = 25;
    scalars[SCALAR_FILL] = fill;
    scalars[SCALAR_SELLER_CLAIMS] = 5_000;
    scalars[SCALAR_BUYER_CLAIMS] = 200;
    scalars[SCALAR_BUYER_COLLATERAL] = 2_000;
    scalars[SCALAR_SELLER_COLLATERAL] = 100;
    scalars[SCALAR_VENUE_COLLATERAL] = 20;
    let identities = [[101_u8; 32], [101_u8; 32], [11_u8; 32], [12_u8; 32]];
    assert_eq!(identities.len(), DIRECT_AOT_IDENTITIES_V1);
    let mut bank = [0_u8; DIRECT_AOT_BANK_BYTES_V1];
    encode_register_bank_into(&scalars, &identities, &mut bank).expect("register bank");
    let request = AcceleratorRequestV1::new(content(1), content(2), content(3), 41, 4, &bank)
        .expect("request");
    let mut bytes = [0_u8; DIRECT_AOT_REQUEST_BYTES_V1];
    request.encode_into(&mut bytes).expect("request bytes");
    bytes
}

async fn submit(
    context: &mut solana_program_test::ProgramTestContext,
    data: Vec<u8>,
) -> (u64, Vec<u8>) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[Instruction {
            program_id: PROGRAM_ID,
            accounts: Vec::new(),
            data,
        }],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(processed.result.is_ok(), "stateless execution must commit");
    let metadata = processed.metadata.expect("transaction metadata");
    let returned = metadata.return_data.expect("accelerator return data");
    assert_eq!(returned.program_id, PROGRAM_ID);
    (metadata.compute_units_consumed, returned.data)
}

#[tokio::test]
#[ignore = "requires cargo-build-sbf output via SBF_OUT_DIR"]
async fn real_elf_acceptance_and_refusal_have_exact_return_shapes() {
    let mut test = ProgramTest::new("dclutch_direct_aot_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    let mut context = test.start_with_context().await;

    let (accepted_cu, accepted_bytes) = submit(&mut context, request(2_000).to_vec()).await;
    assert_eq!(accepted_bytes.len(), DIRECT_AOT_ACCEPTED_ACK_BYTES_V1);
    assert_eq!(
        AcceleratorAckV1::decode(&accepted_bytes)
            .expect("accepted ack")
            .disposition(),
        ExecutionDispositionV1::Accepted
    );

    let (refused_cu, refused_bytes) = submit(&mut context, request(0).to_vec()).await;
    assert_eq!(
        AcceleratorAckV1::decode(&refused_bytes)
            .expect("refusal ack")
            .disposition(),
        ExecutionDispositionV1::Refused
    );
    assert!(accepted_cu > 0);
    assert!(refused_cu > 0);
    eprintln!("Direct stateless AOT accepted CU: {accepted_cu}");
    eprintln!("Direct stateless AOT refused CU: {refused_cu}");
}
