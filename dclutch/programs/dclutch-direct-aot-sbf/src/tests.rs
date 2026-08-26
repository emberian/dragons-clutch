extern crate std;

use dclutch_direct_aot_contract::*;
use dclutch_execution_strategy_contract::{
    AcceleratorAckV1, AcceleratorRequestV1, ExecutionDispositionV1, decode_register_bank_into,
    encode_register_bank_into,
};
use solana_hash::Hash;
use solana_message::{VersionedMessage, v0};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey};
use std::vec;

use super::*;

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("content")
}

fn example_bank(fill: u64) -> [u8; DIRECT_AOT_BANK_BYTES_V1] {
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
    let mut bank = [0_u8; DIRECT_AOT_BANK_BYTES_V1];
    encode_register_bank_into(&scalars, &identities, &mut bank).expect("bank");
    bank
}

fn request(bank: &[u8]) -> [u8; DIRECT_AOT_REQUEST_BYTES_V1] {
    let request = AcceleratorRequestV1::new(content(1), content(2), content(3), 41, 4, bank)
        .expect("request");
    let mut bytes = [0_u8; DIRECT_AOT_REQUEST_BYTES_V1];
    request.encode_into(&mut bytes).expect("request bytes");
    bytes
}

#[test]
fn accepted_request_returns_exact_candidate_ack() {
    let bytes = request(&example_bank(2_000));
    let mut output = [0_u8; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1];
    assert_eq!(evaluate_into(&bytes, &mut output), Ok(616));
    let ack = AcceleratorAckV1::decode(&output).expect("ack");
    assert_eq!(ack.disposition(), ExecutionDispositionV1::Accepted);
    assert_eq!(ack.request_digest().as_bytes(), &hash(&bytes).to_bytes());
    assert_eq!(
        ack.bank_digest().expect("bank digest").as_bytes(),
        &hash(ack.bank()).to_bytes()
    );
    let mut scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    let mut identities = [[0_u8; 32]; DIRECT_AOT_IDENTITIES_V1];
    decode_register_bank_into(ack.bank(), &mut scalars, &mut identities).expect("output bank");
    assert_eq!(scalars[SCALAR_GROSS_OUTPUT], 1_000);
    assert_eq!(scalars[SCALAR_FEE_OUTPUT], 2);
    assert_eq!(scalars[SCALAR_SELLER_NONCE_OUTPUT], 1);
    assert_eq!(scalars[SCALAR_BUYER_NONCE_OUTPUT], 1);
}

#[test]
fn semantic_refusal_returns_ack_without_candidate() {
    let bytes = request(&example_bank(0));
    let mut output = [0xa5_u8; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1];
    assert_eq!(evaluate_into(&bytes, &mut output), Ok(160));
    let ack = AcceleratorAckV1::decode(
        output
            .get(..DIRECT_AOT_REFUSED_ACK_BYTES_V1)
            .expect("refusal prefix"),
    )
    .expect("refusal ack");
    assert_eq!(ack.disposition(), ExecutionDispositionV1::Refused);
    assert!(ack.bank().is_empty());
    assert_eq!(ack.bank_digest(), None);
}

#[test]
fn malformed_request_refuses_before_evaluation() {
    let mut bytes = request(&example_bank(2_000));
    *bytes.first_mut().expect("magic") ^= 1;
    let mut output = [0xa5_u8; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1];
    assert_eq!(
        evaluate_into(&bytes, &mut output),
        Err(DirectAotSbfError::InvalidRequest.into())
    );
    assert_eq!(output, [0xa5; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1]);
}

#[test]
fn standalone_v0_packet_is_measured_from_pinned_sdk() {
    let payer = Pubkey::new_from_array([1; 32]);
    let program_id = Pubkey::new_from_array([2; 32]);
    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: request(&example_bank(2_000)).to_vec(),
    };
    let message =
        v0::Message::try_compile(&payer, &[instruction], &[], Hash::new_from_array([3; 32]))
            .expect("v0 message");
    assert_eq!(message.header.num_required_signatures, 1);
    assert_eq!(message.account_keys.len(), 2);
    let wire_bytes = 1
        + usize::from(message.header.num_required_signatures) * 64
        + VersionedMessage::V0(message).serialize().len();
    assert_eq!(wire_bytes, DIRECT_AOT_STANDALONE_V0_WIRE_BYTES_V1);
    assert!(wire_bytes <= SOLANA_PACKET_DATA_BYTES_V1);
}

#[test]
fn stateless_frame_has_zero_persistent_rent_and_no_accounts() {
    let instruction = Instruction {
        program_id: Pubkey::new_from_array([2; 32]),
        accounts: vec![],
        data: request(&example_bank(2_000)).to_vec(),
    };
    assert!(instruction.accounts.is_empty());
    let persistent_account_data_bytes = 0_usize;
    let persistent_rent_lamports = 0_u64;
    assert_eq!(persistent_account_data_bytes, 0);
    assert_eq!(persistent_rent_lamports, 0);
}
