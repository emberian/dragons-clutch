//! Exact maximum-distinct lock and v0 packet census for Fractional V3 routes.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_operator::{FractionalFrameKindV3, fractional_frame_census_v3};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, v0};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

const PACKET_LIMIT: usize = 1_232;
const LOCK_LIMIT: usize = 64;

fn key(index: u16) -> Pubkey {
    let mut bytes = [0_u8; 32];
    bytes[..2].copy_from_slice(&index.to_le_bytes());
    bytes[31] = 1;
    Pubkey::new_from_array(bytes)
}

fn compile_maximum_distinct(kind: FractionalFrameKindV3) -> (usize, usize, usize) {
    let census = fractional_frame_census_v3(kind);
    let payer = key(1);
    let program = key(2);
    let mut accounts = Vec::with_capacity(census.unique_account_locks - 1);
    accounts.push(AccountMeta::new(payer, true));
    for index in 0..census.unique_account_locks - 2 {
        accounts.push(AccountMeta::new(
            key(u16::try_from(index + 3).unwrap()),
            false,
        ));
    }
    let instruction = Instruction {
        program_id: program,
        accounts,
        data: vec![0; census.instruction_data_bytes],
    };
    let addresses = instruction
        .accounts
        .iter()
        .filter(|meta| !meta.is_signer)
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    let table = AddressLookupTableAccount {
        key: key(500),
        addresses,
    };
    let message = v0::Message::try_compile(
        &payer,
        core::slice::from_ref(&instruction),
        core::slice::from_ref(&table),
        Hash::new_from_array([77; 32]),
    )
    .unwrap();
    let loaded = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
        .sum();
    let wire_bytes =
        1 + usize::from(message.header.num_required_signatures) * 64 + message.serialize().len();
    (
        wire_bytes,
        loaded,
        usize::from(message.header.num_required_signatures),
    )
}

#[test]
fn every_bounded_route_is_below_lock_and_packet_limits() {
    for (kind, expected_wire, expected_loaded) in [
        (FractionalFrameKindV3::WrapOrWholeUnwrap, 682, 29),
        (FractionalFrameKindV3::DirectTransfer, 222, 3),
        (FractionalFrameKindV3::TerminalRedeemOrZeroBurn, 708, 42),
        (FractionalFrameKindV3::Terminalize, 656, 16),
        (FractionalFrameKindV3::RetirementBegin, 508, 6),
        (FractionalFrameKindV3::RetirementCoordinate, 534, 19),
        (FractionalFrameKindV3::RetirementFinish, 512, 8),
    ] {
        let census = fractional_frame_census_v3(kind);
        let (wire, loaded, signatures) = compile_maximum_distinct(kind);
        assert!(census.unique_account_locks <= LOCK_LIMIT);
        assert!(wire <= PACKET_LIMIT);
        assert_eq!(wire, expected_wire);
        assert_eq!(signatures, census.required_signatures);
        assert_eq!(loaded, expected_loaded);
        assert_eq!(loaded, census.unique_account_locks - 2);
    }
}

#[test]
fn maximum_width_retirement_never_reintroduces_a_k_account_tail() {
    let step = fractional_frame_census_v3(FractionalFrameKindV3::RetirementCoordinate);
    let finish = fractional_frame_census_v3(FractionalFrameKindV3::RetirementFinish);
    assert_eq!(step.unique_account_locks, 21);
    assert_eq!(finish.unique_account_locks, 10);
    // K=256 changes the number of transactions, not either transaction frame.
    assert_eq!(
        fractional_frame_census_v3(FractionalFrameKindV3::RetirementCoordinate),
        step
    );
}
