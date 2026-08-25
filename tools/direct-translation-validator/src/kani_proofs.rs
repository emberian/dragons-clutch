//! Bit-precise universal proof targets for Kani.
//!
//! These harnesses are inert in ordinary Rust builds. They become proof units
//! only when an independently pinned `cargo kani` toolchain injects `cfg(kani)`
//! and the `kani` crate.

use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, CompactIntentV1, ControllerInstructionV1, Error,
    REGISTERED_TERMINAL_INSTRUCTION_BYTES, RegisteredTerminalAction,
    RegisteredTerminalInstructionV1,
};

fn arbitrary_intent() -> CompactIntentV1 {
    CompactIntentV1 {
        side: kani::any(),
        outcome: kani::any(),
        lifecycle: kani::any(),
        market: kani::any(),
        generation: kani::any(),
        nonce: kani::any(),
        valid_from: kani::any(),
        valid_through: kani::any(),
        maximum_fill: kani::any(),
        limit_price: kani::any(),
        fee_basis_points: kani::any(),
        collateral_account: kani::any(),
    }
}

#[kani::proof]
fn every_fixed_width_intent_round_trips() {
    let intent = arbitrary_intent();
    let encoded = intent.encode();
    assert!(encoded.is_ok());
    if let Ok(bytes) = encoded {
        assert_eq!(CompactIntentV1::decode(&bytes), Ok(intent));
    }
}

#[kani::proof]
fn every_fixed_width_controller_round_trips() {
    let instruction = ControllerInstructionV1 {
        controller_bump: kani::any(),
        seller_replay_bump: kani::any(),
        buyer_replay_bump: kani::any(),
        seller_position_bump: kani::any(),
        buyer_position_bump: kani::any(),
        fill: kani::any(),
        execution_price: kani::any(),
        seller: arbitrary_intent(),
        buyer: arbitrary_intent(),
    };
    let encoded = instruction.encode();
    assert!(encoded.is_ok());
    if let Ok(bytes) = encoded {
        assert_eq!(ControllerInstructionV1::decode(&bytes), Ok(instruction));
    }
}

#[kani::proof]
fn every_fixed_width_terminal_controller_round_trips() {
    let cancel: bool = kani::any();
    let instruction = RegisteredTerminalInstructionV1 {
        action: if cancel {
            RegisteredTerminalAction::Cancel
        } else {
            RegisteredTerminalAction::Expire
        },
        controller_bump: kani::any(),
        registration_bump: kani::any(),
        expected_sequence: kani::any(),
    };
    let encoded = instruction.encode();
    assert!(encoded.is_ok());
    if let Ok(bytes) = encoded {
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&bytes),
            Ok(instruction)
        );
    }
}

#[kani::proof]
fn every_short_terminal_controller_is_refused() {
    let bytes: [u8; REGISTERED_TERMINAL_INSTRUCTION_BYTES] = kani::any();
    let length: usize = kani::any();
    kani::assume(length < REGISTERED_TERMINAL_INSTRUCTION_BYTES);
    assert_eq!(
        RegisteredTerminalInstructionV1::decode(&bytes[..length]),
        Err(Error::InvalidLength)
    );
}

#[kani::proof]
fn every_short_intent_is_refused() {
    let bytes: [u8; COMPACT_INTENT_BYTES] = kani::any();
    let length: usize = kani::any();
    kani::assume(length < COMPACT_INTENT_BYTES);
    assert_eq!(
        CompactIntentV1::decode(&bytes[..length]),
        Err(Error::InvalidLength)
    );
}

#[kani::proof]
fn any_nonzero_reserved_intent_byte_is_refused() {
    let intent = arbitrary_intent();
    let encoded = intent.encode();
    assert!(encoded.is_ok());
    if let Ok(mut bytes) = encoded {
        let choose_second_span: bool = kani::any();
        let index: usize = kani::any();
        let value: u8 = kani::any();
        kani::assume(value != 0);
        let offset = if choose_second_span {
            kani::assume(index < 6);
            98 + index
        } else {
            kani::assume(index < 3);
            13 + index
        };
        bytes[offset] = value;
        assert_eq!(CompactIntentV1::decode(&bytes), Err(Error::NonzeroReserved));
    }
}
