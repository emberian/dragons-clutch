#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout codecs for Lean-owned compiled Direct data.

mod generated_layout;

/// Bytes in one independently signed compact intent.
pub const COMPACT_INTENT_BYTES: usize = generated_layout::COMPACT_INTENT_BYTES_VALUE;
/// Bytes in one controller instruction containing two compact intents.
pub const CONTROLLER_INSTRUCTION_BYTES: usize =
    generated_layout::CONTROLLER_INSTRUCTION_BYTES_VALUE;
/// Current compiled Direct ABI version.
pub const VERSION: u16 = generated_layout::ABI_VERSION;
/// Semantic release selected by a Market for this compiled inline controller.
///
/// SHA-256 of `dclutch/release/direct-compiled-controller-v1`. A checked
/// release manifest separately binds this semantic coordinate to exact ELF and
/// Loader evidence; it is not itself an artifact digest.
pub const COMPILED_DIRECT_RELEASE_ID_V1: [u8; 32] = [
    0x79, 0xfa, 0xd2, 0xf0, 0x4f, 0x8d, 0x9c, 0xe0, 0x7d, 0x76, 0xc8, 0x09, 0xfe, 0x11, 0x6d, 0xb8,
    0xef, 0x93, 0x74, 0xad, 0xbe, 0xb1, 0x5e, 0x62, 0xf6, 0x03, 0x23, 0x5c, 0x3a, 0x2b, 0x96, 0xb9,
];
/// Measured compiled inline capacity coordinate for `N = 2..=16`.
pub const COMPILED_DIRECT_CAPACITY_ID_V1: [u8; 32] = [
    0x2e, 0xaf, 0xb1, 0x44, 0x84, 0x0a, 0x9d, 0xc3, 0x1c, 0xed, 0x73, 0xac, 0x19, 0x9b, 0xa1, 0xcf,
    0x49, 0x16, 0x15, 0x28, 0x47, 0x02, 0x05, 0x14, 0x37, 0xb1, 0xa5, 0x9d, 0xf5, 0xd2, 0x93, 0x7d,
];
/// Compiled replay/Position child-state schema coordinate.
pub const COMPILED_DIRECT_CHILD_SCHEMA_ID_V1: [u8; 32] = [
    0x97, 0x67, 0xa1, 0x54, 0x54, 0x9c, 0x1f, 0x06, 0xa1, 0xe0, 0xc7, 0x41, 0xc1, 0x84, 0xac, 0xc0,
    0xd9, 0xbd, 0x18, 0xa4, 0x21, 0x19, 0xfa, 0xac, 0x14, 0x0c, 0x4d, 0xd7, 0xf1, 0x55, 0xfc, 0xe7,
];
/// Compiled replay/Position PDA-derivation coordinate.
pub const COMPILED_DIRECT_DERIVATION_ID_V1: [u8; 32] = [
    0x2d, 0x00, 0xc7, 0x72, 0x68, 0xf9, 0x0c, 0x56, 0xc2, 0xf4, 0x3d, 0xb5, 0x43, 0x74, 0x54, 0x92,
    0xd0, 0x5e, 0x36, 0x9a, 0xef, 0xd5, 0xd3, 0x86, 0x9a, 0x6a, 0xa6, 0xae, 0xe7, 0xc9, 0xc5, 0x8d,
];

const INTENT_MAGIC: &[u8; 8] = &generated_layout::INTENT_MAGIC_BYTES;
const CONTROLLER_MAGIC: &[u8; 8] = &generated_layout::CONTROLLER_MAGIC_BYTES;

/// Strict codec refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input width differed from the exact schema width.
    InvalidLength,
    /// Domain-separating magic was not exact.
    InvalidMagic,
    /// Schema version is not implemented.
    UnsupportedVersion,
    /// A reserved byte was nonzero.
    NonzeroReserved,
    /// A fixed output field could not be written.
    Output,
}

/// One independently signed reusable Direct limit intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactIntentV1 {
    /// Seller `0` or buyer `1`; transition admission rejects other tags.
    pub side: u8,
    /// Product-owned outcome coordinate.
    pub outcome: u8,
    /// Fill-or-kill `0` or immediate-or-cancel `1`.
    pub lifecycle: u8,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Replay generation selected by the immutable Market identity.
    pub generation: u64,
    /// Exact next maker replay nonce.
    pub nonce: u64,
    /// First valid Clock slot.
    pub valid_from: u64,
    /// Last valid Clock slot.
    pub valid_through: u64,
    /// Maximum admitted fill.
    pub maximum_fill: u64,
    /// Seller minimum or buyer maximum price at the profile scale.
    pub limit_price: u64,
    /// Exact maker-accepted cumulative floor-fee rate.
    pub fee_basis_points: u16,
    /// Seller destination or buyer source token account.
    pub collateral_account: [u8; 32],
}

impl CompactIntentV1 {
    /// Strictly decode one canonical compact intent.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, COMPACT_INTENT_BYTES)?;
        exact_magic(input, INTENT_MAGIC)?;
        exact_version(input)?;
        reserved(
            input,
            generated_layout::INTENT_RESERVED_A_OFFSET,
            generated_layout::INTENT_RESERVED_A_WIDTH,
        )?;
        reserved(
            input,
            generated_layout::INTENT_RESERVED_B_OFFSET,
            generated_layout::INTENT_RESERVED_B_WIDTH,
        )?;
        Ok(generated_layout::decode_compact_intent_fields!(input))
    }

    /// Encode one canonical compact intent.
    pub fn encode(self) -> Result<[u8; COMPACT_INTENT_BYTES], Error> {
        let mut output = [0_u8; COMPACT_INTENT_BYTES];
        put(&mut output, generated_layout::MAGIC_OFFSET, INTENT_MAGIC)?;
        put(
            &mut output,
            generated_layout::VERSION_OFFSET,
            &VERSION.to_le_bytes(),
        )?;
        generated_layout::encode_compact_intent_fields!(output, self);
        Ok(output)
    }
}

/// Matcher coordinates and two independently signed compact intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInstructionV1 {
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Seller replay-root PDA bump.
    pub seller_replay_bump: u8,
    /// Buyer replay-root PDA bump.
    pub buyer_replay_bump: u8,
    /// Seller maker/outcome Position PDA bump.
    pub seller_position_bump: u8,
    /// Buyer maker/outcome Position PDA bump.
    pub buyer_position_bump: u8,
    /// Matcher-selected fill checked against both intents.
    pub fill: u64,
    /// Matcher-selected execution price checked against both limits.
    pub execution_price: u64,
    /// Seller's independently signed intent.
    pub seller: CompactIntentV1,
    /// Buyer's independently signed intent.
    pub buyer: CompactIntentV1,
}

impl ControllerInstructionV1 {
    /// Strictly decode one canonical controller instruction.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, CONTROLLER_INSTRUCTION_BYTES)?;
        exact_magic(input, CONTROLLER_MAGIC)?;
        exact_version(input)?;
        reserved(
            input,
            generated_layout::CONTROLLER_RESERVED_OFFSET,
            generated_layout::CONTROLLER_RESERVED_WIDTH,
        )?;
        Ok(generated_layout::decode_controller_fields!(input))
    }

    /// Encode one canonical controller instruction.
    pub fn encode(self) -> Result<[u8; CONTROLLER_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; CONTROLLER_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_layout::MAGIC_OFFSET,
            CONTROLLER_MAGIC,
        )?;
        put(
            &mut output,
            generated_layout::VERSION_OFFSET,
            &VERSION.to_le_bytes(),
        )?;
        generated_layout::encode_controller_fields!(output, self);
        Ok(output)
    }
}

fn exact_width(input: &[u8], expected: usize) -> Result<(), Error> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact_magic(input: &[u8], expected: &[u8; 8]) -> Result<(), Error> {
    if slice(input, generated_layout::MAGIC_OFFSET, expected.len())? == expected {
        Ok(())
    } else {
        Err(Error::InvalidMagic)
    }
}

fn exact_version(input: &[u8]) -> Result<(), Error> {
    if u16_at(input, generated_layout::VERSION_OFFSET)? == VERSION {
        Ok(())
    } else {
        Err(Error::UnsupportedVersion)
    }
}

fn reserved(input: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    if slice(input, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonzeroReserved)
    }
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    input.get(offset..end).ok_or(Error::InvalidLength)
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Output)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Output)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::Output)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used)]

    extern crate std;

    use super::*;
    use std::{string::String, vec::Vec};

    const LEAN_VECTORS: &str =
        include_str!("../../../formal/dclutch-semantics/vectors/direct-controller-v1.txt");

    fn fixture_intent(side: u8) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle: 0,
            market: [4; 32],
            generation: 3,
            nonce: 0,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: [if side == 0 { 5 } else { 6 }; 32],
        }
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(DIGITS[usize::from(*byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
        }
        output
    }

    fn vector(name: &str) -> String {
        let prefix = String::from(name) + "=";
        LEAN_VECTORS
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .map(String::from)
            .unwrap_or_default()
    }

    #[test]
    fn encoders_exactly_match_lean_vectors_and_round_trip() {
        let seller = fixture_intent(0);
        let buyer = fixture_intent(1);
        let controller = ControllerInstructionV1 {
            controller_bump: 1,
            seller_replay_bump: 2,
            buyer_replay_bump: 3,
            seller_position_bump: 4,
            buyer_position_bump: 5,
            fill: 2_000,
            execution_price: 500_000,
            seller,
            buyer,
        };
        for (name, encoded) in [
            ("seller_intent", seller.encode().map(Vec::from)),
            ("buyer_intent", buyer.encode().map(Vec::from)),
            ("controller", controller.encode().map(Vec::from)),
        ] {
            let encoded = encoded.expect("fixed encoder");
            assert_eq!(hex(&encoded), vector(name));
        }
        assert_eq!(
            CompactIntentV1::decode(&seller.encode().expect("seller encoding")),
            Ok(seller)
        );
        assert_eq!(
            ControllerInstructionV1::decode(&controller.encode().expect("controller encoding")),
            Ok(controller)
        );
    }

    #[test]
    fn hostile_width_magic_version_and_reserved_bytes_refuse() {
        let mut encoded = fixture_intent(0).encode().expect("intent encoding");
        assert_eq!(
            CompactIntentV1::decode(&encoded[..135]),
            Err(Error::InvalidLength)
        );
        encoded[0] ^= 1;
        assert_eq!(CompactIntentV1::decode(&encoded), Err(Error::InvalidMagic));
        encoded = fixture_intent(0).encode().expect("intent encoding");
        encoded[8] = 2;
        assert_eq!(
            CompactIntentV1::decode(&encoded),
            Err(Error::UnsupportedVersion)
        );
        encoded = fixture_intent(0).encode().expect("intent encoding");
        encoded[generated_layout::INTENT_RESERVED_A_OFFSET] = 1;
        assert_eq!(
            CompactIntentV1::decode(&encoded),
            Err(Error::NonzeroReserved)
        );
    }
}
