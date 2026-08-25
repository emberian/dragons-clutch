//! Hostile adapter dispatch and exact transaction-envelope tests.

extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_general_config_contract::GENERAL_ACTIVATION_REQUEST_BYTES_V2;
use dclutch_market_core_codec::{
    Action, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CapabilityFundingHeaderV1, CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, REQUEST_BYTES,
    Request, Role,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    CAPABILITY_PREFIX_BYTES_V1, CAPABILITY_ROLE_PREFIX_BYTES_V1, CoreSbfError, process_instruction,
};

const PACKET_DATA_BYTES: usize = 1_232;
const MAX_FUNDING_ACCOUNTS: usize = 16;
const STANDARD_GENERAL_CHILD_TAIL_ACCOUNTS: usize = 3;
const GENERIC_FIXED_ACCOUNTS: usize = 14;

fn identity(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("nonzero identity")
}

fn valid_capability_instruction() -> Vec<u8> {
    let market = identity(1);
    let request = Request::administrative(Action::ActivateCapability, 7, market)
        .encode()
        .expect("request");
    let selection =
        CapabilityExecutionSelectionV1::from_bytes(0, [2; 32], [3; 32], [4; 32], [5; 32])
            .expect("selection")
            .to_bytes();
    let header = CapabilityFundingHeaderV1::new(1).expect("header").encode();
    let family_request = [42];
    let role_request_bytes =
        u32::try_from(selection.len() + header.len() + family_request.len()).expect("role width");
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        identity(6),
        identity(7),
        identity(8),
        market,
        identity(9),
        identity(10),
        identity(11),
        7,
        0,
        0,
        role_request_bytes,
    )
    .expect("envelope")
    .encode()
    .expect("envelope bytes");
    let mut output = Vec::with_capacity(
        request.len() + envelope.len() + selection.len() + header.len() + family_request.len(),
    );
    output.extend_from_slice(&request);
    output.extend_from_slice(&envelope);
    output.extend_from_slice(&selection);
    output.extend_from_slice(&header);
    output.extend_from_slice(&family_request);
    output
}

fn account(key: Pubkey) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(0)),
        Box::leak(Vec::new().into_boxed_slice()),
        Box::leak(Box::new(Pubkey::default())),
        false,
    )
}

#[test]
fn truncated_instruction_refuses_before_account_access() {
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &[0; 71]),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn noncanonical_funding_header_refuses_before_account_access() {
    let mut instruction = valid_capability_instruction();
    let header_start = CAPABILITY_PREFIX_BYTES_V1 + CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
    let reserved = header_start + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1 - 1;
    let byte = instruction.get_mut(reserved).expect("reserved header byte");
    *byte = 1;
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &instruction),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn aliased_outer_accounts_refuse_before_state_or_child_access() {
    let duplicate = Pubkey::new_unique();
    let accounts = vec![account(duplicate), account(duplicate)];
    assert_eq!(
        process_instruction(
            &Pubkey::new_unique(),
            &accounts,
            &valid_capability_instruction()
        ),
        Err(ProgramError::Custom(CoreSbfError::AccountFrame as u32))
    );
}

#[test]
fn maximum_profile_general_activation_fits_one_lookup_v0_packet() {
    let payer = Pubkey::new_from_array([1; 32]);
    let program_id = Pubkey::new_from_array([2; 32]);
    let account_count =
        GENERIC_FIXED_ACCOUNTS + MAX_FUNDING_ACCOUNTS + STANDARD_GENERAL_CHILD_TAIL_ACCOUNTS;
    assert_eq!(account_count, 33);
    let addresses = (0..account_count)
        .map(|index| Pubkey::new_from_array([u8::try_from(index + 3).expect("key"); 32]))
        .collect::<Vec<_>>();
    let accounts = addresses
        .iter()
        .map(|key| AccountMeta::new_readonly(*key, false))
        .collect::<Vec<_>>();
    let role_bytes = CAPABILITY_ROLE_PREFIX_BYTES_V1 + GENERAL_ACTIVATION_REQUEST_BYTES_V2;
    assert_eq!(role_bytes, 416);
    let instruction_bytes = REQUEST_BYTES + CORE_EFFECT_ENVELOPE_BYTES_V1 + role_bytes;
    assert_eq!(instruction_bytes, 768);
    let instruction = Instruction {
        program_id,
        accounts,
        data: vec![0; instruction_bytes],
    };
    let blockhash = Hash::new_from_array([255; 32]);
    let uncompressed =
        v0::Message::try_compile(&payer, core::slice::from_ref(&instruction), &[], blockhash)
            .expect("uncompressed v0 message");
    let uncompressed_bytes = 1
        + usize::from(uncompressed.header.num_required_signatures) * 64
        + VersionedMessage::V0(uncompressed).serialize().len();
    let compressed = v0::Message::try_compile(
        &payer,
        &[instruction],
        &[AddressLookupTableAccount {
            key: Pubkey::new_from_array([254; 32]),
            addresses,
        }],
        blockhash,
    )
    .expect("lookup v0 message");
    let compressed_bytes = 1
        + usize::from(compressed.header.num_required_signatures) * 64
        + VersionedMessage::V0(compressed).serialize().len();
    assert_eq!(uncompressed_bytes, 2_029);
    assert_eq!(compressed_bytes, 1_040);
    assert!(compressed_bytes <= PACKET_DATA_BYTES);
}
