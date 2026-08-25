#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout codecs for Lean-owned compiled Direct data.

/// Bytes in one independently signed compact intent.
pub const COMPACT_INTENT_BYTES: usize = 136;
/// Bytes in one controller instruction containing two compact intents.
pub const CONTROLLER_INSTRUCTION_BYTES: usize = 304;
/// Bytes in one experimental execution profile.
pub const MARKET_PROFILE_BYTES: usize = 136;
/// Current compiled Direct ABI version.
pub const VERSION: u16 = 1;

const INTENT_MAGIC: &[u8; 8] = b"DCLTDIR3";
const CONTROLLER_MAGIC: &[u8; 8] = b"DCLTCTL1";
const PROFILE_MAGIC: &[u8; 8] = b"DCLTPRF1";

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
    /// Immutable execution-profile account identity.
    pub execution_profile: [u8; 32],
    /// Replay generation selected by the execution profile.
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
        reserved(input, 13, 3)?;
        reserved(input, 98, 6)?;
        Ok(Self {
            side: byte(input, 10)?,
            outcome: byte(input, 11)?,
            lifecycle: byte(input, 12)?,
            execution_profile: array(input, 16)?,
            generation: u64_at(input, 48)?,
            nonce: u64_at(input, 56)?,
            valid_from: u64_at(input, 64)?,
            valid_through: u64_at(input, 72)?,
            maximum_fill: u64_at(input, 80)?,
            limit_price: u64_at(input, 88)?,
            fee_basis_points: u16_at(input, 96)?,
            collateral_account: array(input, 104)?,
        })
    }

    /// Encode one canonical compact intent.
    pub fn encode(self) -> Result<[u8; COMPACT_INTENT_BYTES], Error> {
        let mut output = [0_u8; COMPACT_INTENT_BYTES];
        put(&mut output, 0, INTENT_MAGIC)?;
        put(&mut output, 8, &VERSION.to_le_bytes())?;
        put_byte(&mut output, 10, self.side)?;
        put_byte(&mut output, 11, self.outcome)?;
        put_byte(&mut output, 12, self.lifecycle)?;
        put(&mut output, 16, &self.execution_profile)?;
        put(&mut output, 48, &self.generation.to_le_bytes())?;
        put(&mut output, 56, &self.nonce.to_le_bytes())?;
        put(&mut output, 64, &self.valid_from.to_le_bytes())?;
        put(&mut output, 72, &self.valid_through.to_le_bytes())?;
        put(&mut output, 80, &self.maximum_fill.to_le_bytes())?;
        put(&mut output, 88, &self.limit_price.to_le_bytes())?;
        put(&mut output, 96, &self.fee_basis_points.to_le_bytes())?;
        put(&mut output, 104, &self.collateral_account)?;
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
        reserved(input, 15, 1)?;
        Ok(Self {
            controller_bump: byte(input, 10)?,
            seller_replay_bump: byte(input, 11)?,
            buyer_replay_bump: byte(input, 12)?,
            seller_position_bump: byte(input, 13)?,
            buyer_position_bump: byte(input, 14)?,
            fill: u64_at(input, 16)?,
            execution_price: u64_at(input, 24)?,
            seller: CompactIntentV1::decode(slice(input, 32, COMPACT_INTENT_BYTES)?)?,
            buyer: CompactIntentV1::decode(slice(input, 168, COMPACT_INTENT_BYTES)?)?,
        })
    }

    /// Encode one canonical controller instruction.
    pub fn encode(self) -> Result<[u8; CONTROLLER_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; CONTROLLER_INSTRUCTION_BYTES];
        put(&mut output, 0, CONTROLLER_MAGIC)?;
        put(&mut output, 8, &VERSION.to_le_bytes())?;
        put_byte(&mut output, 10, self.controller_bump)?;
        put_byte(&mut output, 11, self.seller_replay_bump)?;
        put_byte(&mut output, 12, self.buyer_replay_bump)?;
        put_byte(&mut output, 13, self.seller_position_bump)?;
        put_byte(&mut output, 14, self.buyer_position_bump)?;
        put(&mut output, 16, &self.fill.to_le_bytes())?;
        put(&mut output, 24, &self.execution_price.to_le_bytes())?;
        put(&mut output, 32, &self.seller.encode()?)?;
        put(&mut output, 168, &self.buyer.encode()?)?;
        Ok(output)
    }
}

/// Read-only execution policy selected by both signed intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketProfileV1 {
    /// Current Market phase tag.
    pub phase: u8,
    /// Product-owned outcome count.
    pub outcome_count: u8,
    /// Current replay generation.
    pub generation: u64,
    /// Exact Product-owned price scale.
    pub price_scale: u64,
    /// Exact cumulative floor-fee rate.
    pub fee_basis_points: u16,
    /// Realm-selected token-program identity.
    pub token_program: [u8; 32],
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Immutable fee-recipient token account.
    pub fee_recipient: [u8; 32],
}

impl MarketProfileV1 {
    /// Strictly decode one canonical execution profile.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, MARKET_PROFILE_BYTES)?;
        exact_magic(input, PROFILE_MAGIC)?;
        exact_version(input)?;
        reserved(input, 12, 4)?;
        reserved(input, 34, 6)?;
        Ok(Self {
            phase: byte(input, 10)?,
            outcome_count: byte(input, 11)?,
            generation: u64_at(input, 16)?,
            price_scale: u64_at(input, 24)?,
            fee_basis_points: u16_at(input, 32)?,
            token_program: array(input, 40)?,
            collateral_mint: array(input, 72)?,
            fee_recipient: array(input, 104)?,
        })
    }

    /// Encode one canonical execution profile.
    pub fn encode(self) -> Result<[u8; MARKET_PROFILE_BYTES], Error> {
        let mut output = [0_u8; MARKET_PROFILE_BYTES];
        put(&mut output, 0, PROFILE_MAGIC)?;
        put(&mut output, 8, &VERSION.to_le_bytes())?;
        put_byte(&mut output, 10, self.phase)?;
        put_byte(&mut output, 11, self.outcome_count)?;
        put(&mut output, 16, &self.generation.to_le_bytes())?;
        put(&mut output, 24, &self.price_scale.to_le_bytes())?;
        put(&mut output, 32, &self.fee_basis_points.to_le_bytes())?;
        put(&mut output, 40, &self.token_program)?;
        put(&mut output, 72, &self.collateral_mint)?;
        put(&mut output, 104, &self.fee_recipient)?;
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
    if input.get(..8) == Some(expected.as_slice()) {
        Ok(())
    } else {
        Err(Error::InvalidMagic)
    }
}

fn exact_version(input: &[u8]) -> Result<(), Error> {
    if u16_at(input, 8)? == VERSION {
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
            execution_profile: [4; 32],
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
        let profile = MarketProfileV1 {
            phase: 1,
            outcome_count: 2,
            generation: 3,
            price_scale: 1_000_000,
            fee_basis_points: 25,
            token_program: [7; 32],
            collateral_mint: [8; 32],
            fee_recipient: [9; 32],
        };
        for (name, encoded) in [
            ("seller_intent", seller.encode().map(Vec::from)),
            ("buyer_intent", buyer.encode().map(Vec::from)),
            ("controller", controller.encode().map(Vec::from)),
            ("market_profile", profile.encode().map(Vec::from)),
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
        assert_eq!(
            MarketProfileV1::decode(&profile.encode().expect("profile encoding")),
            Ok(profile)
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
        encoded[13] = 1;
        assert_eq!(
            CompactIntentV1::decode(&encoded),
            Err(Error::NonzeroReserved)
        );
    }
}
