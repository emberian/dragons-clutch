//! Native Ed25519 evidence, relay-owned.
//!
//! **O-018 compliance, stated before anything leans on adjacency.** Adjacency
//! is not the authority here.  The authority is the pair (release-pinned public
//! key, byte-exact message equality against the current instruction's own data);
//! adjacency only selects *which* instruction to parse.  Precompiles are
//! verified against the transaction's top-level instruction list during
//! transaction verification and are not reachable by CPI, so a post-then-consume
//! transport is unavailable and adjacency is the only carriage.
//!
//! This parser admits **exactly one** signature per precompile instruction, and
//! that is a deliberate narrowing rather than a simplification.  An m-of-n key
//! set signing the *same* message would produce m identical message slices; the
//! family therefore never batches, and uses one short seal message per signer
//! instead.  Refusing a count other than one closes the overlapping-slice
//! hazard structurally rather than by a pairwise check.
//!
//! **Convergence debt, declared rather than discovered.**
//! `dclutch-direct-contract::adapter::inspect_preceding_ed25519_batch_item_v2`
//! parses the same wire.  It cannot be reused as-is: it copies each message into
//! a `[u8; DIRECT_INTENT_BYTES_V2]` (232 bytes), and a relayed attestation runs
//! to 560.  This parser returns a *borrowed* message instead of copying, which
//! is also what keeps a 560-byte message off an SBF stack frame.  The two should
//! converge onto the borrowed shape; the Direct crate owns that move.

use crate::{
    ADDRESS_BYTES, ED25519_DESCRIPTOR_BYTES_V1, ED25519_DESCRIPTOR_START_V1,
    ED25519_PUBLIC_KEY_BYTES_V1, ED25519_SIGNATURE_BYTES_V1, Error, Result, array, slice, u16_at,
    u16_from,
};

/// Native Ed25519 program bytes pinned by `solana-sdk-ids = 3.0.0`.
pub const ED25519_PROGRAM_ID_3_0: [u8; ADDRESS_BYTES] = [
    3, 125, 70, 214, 124, 147, 251, 190, 18, 249, 66, 143, 131, 141, 64, 255, 5, 112, 116, 73, 39,
    244, 138, 100, 252, 202, 112, 68, 128, 0, 0, 0,
];

/// Descriptor sentinel meaning "this same instruction".
pub const ED25519_CURRENT_INSTRUCTION_INDEX: u16 = u16::MAX;

/// Exact data width of a one-signature Ed25519 precompile instruction.
pub const ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES: usize = ED25519_DESCRIPTOR_START_V1
    + ED25519_DESCRIPTOR_BYTES_V1
    + ED25519_PUBLIC_KEY_BYTES_V1
    + ED25519_SIGNATURE_BYTES_V1;

const PUBLIC_KEY_OFFSET: usize = ED25519_DESCRIPTOR_START_V1 + ED25519_DESCRIPTOR_BYTES_V1;
const SIGNATURE_OFFSET: usize = PUBLIC_KEY_OFFSET + ED25519_PUBLIC_KEY_BYTES_V1;

/// Authenticated instructions-sysvar projection supplied by the SBF adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ed25519InstructionViewV1<'a> {
    /// Program ID of the immediately preceding instruction.
    pub program_id: [u8; ADDRESS_BYTES],
    /// The immediately preceding native instruction's data.
    pub ed25519_data: &'a [u8],
    /// Index of that preceding instruction.
    pub preceding_index: u16,
    /// Index of the current relay instruction.
    pub current_index: u16,
    /// The current relay instruction's data, which carries the signed message.
    pub current_data: &'a [u8],
}

/// Sealed evidence for one immediately preceding native Ed25519 signature.
///
/// Cryptographic validity follows from successful execution of that preceding
/// native instruction: the runtime rejects the transaction before this program
/// runs if the signature does not verify.  This type therefore certifies *which
/// key signed which bytes*, never that the bytes are meaningful.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedSignatureV1<'a> {
    signer: [u8; ADDRESS_BYTES],
    message: &'a [u8],
}

impl<'a> RelayedSignatureV1<'a> {
    /// The public key the native program verified against.
    pub const fn signer(self) -> [u8; ADDRESS_BYTES] {
        self.signer
    }

    /// The exact verified message bytes, borrowed from the current instruction.
    pub const fn message(self) -> &'a [u8] {
        self.message
    }
}

/// Inspect the one immediately preceding native Ed25519 signature.
///
/// `message_offset` and `message_len` name a slice of `view.current_data`; the
/// descriptor must agree with them exactly, and the message must lie inside the
/// current instruction's own data.  A caller that wants a different message has
/// to ask for a different slice — it cannot be handed one.
pub fn inspect_preceding_relay_signature_v1<'a>(
    view: Ed25519InstructionViewV1<'a>,
    message_offset: u16,
    message_len: u16,
) -> Result<RelayedSignatureV1<'a>> {
    if view.program_id != ED25519_PROGRAM_ID_3_0 {
        return Err(Error::InvalidSignatureProgram);
    }
    if view.preceding_index.checked_add(1) != Some(view.current_index) {
        return Err(Error::InvalidSignatureInstructionOrder);
    }
    if view.ed25519_data.len() != ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES
        || u16_at(view.ed25519_data, 0)? != 1
    {
        return Err(Error::InvalidSignatureInstruction);
    }
    let descriptor = ED25519_DESCRIPTOR_START_V1;
    let expected = [
        (descriptor, u16_from(SIGNATURE_OFFSET)?),
        (
            descriptor.checked_add(2).ok_or(Error::ArithmeticOverflow)?,
            ED25519_CURRENT_INSTRUCTION_INDEX,
        ),
        (
            descriptor.checked_add(4).ok_or(Error::ArithmeticOverflow)?,
            u16_from(PUBLIC_KEY_OFFSET)?,
        ),
        (
            descriptor.checked_add(6).ok_or(Error::ArithmeticOverflow)?,
            ED25519_CURRENT_INSTRUCTION_INDEX,
        ),
        (
            descriptor.checked_add(8).ok_or(Error::ArithmeticOverflow)?,
            message_offset,
        ),
        (
            descriptor
                .checked_add(10)
                .ok_or(Error::ArithmeticOverflow)?,
            message_len,
        ),
        (
            descriptor
                .checked_add(12)
                .ok_or(Error::ArithmeticOverflow)?,
            view.current_index,
        ),
    ];
    for (offset, value) in expected {
        if u16_at(view.ed25519_data, offset)? != value {
            return Err(Error::InvalidSignatureInstruction);
        }
    }
    if slice(
        view.ed25519_data,
        SIGNATURE_OFFSET,
        ED25519_SIGNATURE_BYTES_V1,
    )?
    .iter()
    .all(|byte| *byte == 0)
    {
        return Err(Error::ForgedSignature);
    }
    let message = slice(
        view.current_data,
        usize::from(message_offset),
        usize::from(message_len),
    )
    .map_err(|_| Error::SignatureMessageMismatch)?;
    Ok(RelayedSignatureV1 {
        signer: array(view.ed25519_data, PUBLIC_KEY_OFFSET)?,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::put;

    fn instruction(message_offset: u16, message_len: u16, current_index: u16) -> [u8; 112] {
        let mut data = [0u8; ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES];
        put(&mut data, 0, &1u16.to_le_bytes()).expect("count");
        let fields: [(usize, u16); 7] = [
            (2, 48),
            (4, ED25519_CURRENT_INSTRUCTION_INDEX),
            (6, 16),
            (8, ED25519_CURRENT_INSTRUCTION_INDEX),
            (10, message_offset),
            (12, message_len),
            (14, current_index),
        ];
        for (offset, value) in fields {
            put(&mut data, offset, &value.to_le_bytes()).expect("descriptor");
        }
        put(&mut data, PUBLIC_KEY_OFFSET, &[0x55; 32]).expect("key");
        put(&mut data, SIGNATURE_OFFSET, &[0x77; 64]).expect("signature");
        data
    }

    fn view<'a>(data: &'a [u8], current: &'a [u8]) -> Ed25519InstructionViewV1<'a> {
        Ed25519InstructionViewV1 {
            program_id: ED25519_PROGRAM_ID_3_0,
            ed25519_data: data,
            preceding_index: 0,
            current_index: 1,
            current_data: current,
        }
    }

    #[test]
    fn a_well_formed_adjacent_signature_yields_its_signer_and_borrowed_message() {
        let data = instruction(4, 3, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        let authorization =
            inspect_preceding_relay_signature_v1(view(&data, &current), 4, 3).expect("accepted");
        assert_eq!(authorization.signer(), [0x55; 32]);
        assert_eq!(authorization.message(), &[1, 2, 3]);
    }

    #[test]
    fn a_non_adjacent_signature_refuses() {
        let data = instruction(4, 3, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        let mut hostile = view(&data, &current);
        hostile.preceding_index = 0;
        hostile.current_index = 2;
        assert_eq!(
            inspect_preceding_relay_signature_v1(hostile, 4, 3),
            Err(Error::InvalidSignatureInstructionOrder)
        );
    }

    #[test]
    fn a_signature_from_another_program_refuses() {
        let data = instruction(4, 3, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        let mut hostile = view(&data, &current);
        hostile.program_id = [1; 32];
        assert_eq!(
            inspect_preceding_relay_signature_v1(hostile, 4, 3),
            Err(Error::InvalidSignatureProgram)
        );
    }

    #[test]
    fn an_all_zero_signature_refuses() {
        let mut data = instruction(4, 3, 1);
        put(&mut data, SIGNATURE_OFFSET, &[0u8; 64]).expect("zero");
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        assert_eq!(
            inspect_preceding_relay_signature_v1(view(&data, &current), 4, 3),
            Err(Error::ForgedSignature)
        );
    }

    #[test]
    fn a_descriptor_pointing_outside_the_current_instruction_refuses() {
        let data = instruction(4, 8, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        assert_eq!(
            inspect_preceding_relay_signature_v1(view(&data, &current), 4, 8),
            Err(Error::SignatureMessageMismatch)
        );
    }

    #[test]
    fn a_descriptor_naming_a_different_message_than_the_caller_asked_for_refuses() {
        let data = instruction(4, 3, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        assert_eq!(
            inspect_preceding_relay_signature_v1(view(&data, &current), 0, 3),
            Err(Error::InvalidSignatureInstruction)
        );
    }

    #[test]
    fn a_descriptor_naming_another_instruction_index_refuses() {
        let data = instruction(4, 3, 7);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        assert_eq!(
            inspect_preceding_relay_signature_v1(view(&data, &current), 4, 3),
            Err(Error::InvalidSignatureInstruction)
        );
    }

    #[test]
    fn a_two_signature_batch_refuses_structurally() {
        let mut data = [0u8; ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES];
        put(&mut data, 0, &2u16.to_le_bytes()).expect("count");
        let current = [0u8; 8];
        assert_eq!(
            inspect_preceding_relay_signature_v1(view(&data, &current), 0, 1),
            Err(Error::InvalidSignatureInstruction)
        );
    }

    #[test]
    fn every_truncation_of_the_precompile_instruction_refuses() {
        let data = instruction(4, 3, 1);
        let current = [9u8, 9, 9, 9, 1, 2, 3];
        for width in 0..data.len() {
            let short = data.get(..width).expect("prefix");
            assert!(
                inspect_preceding_relay_signature_v1(view(short, &current), 4, 3).is_err(),
                "a {width}-byte precompile instruction was accepted"
            );
        }
    }
}
