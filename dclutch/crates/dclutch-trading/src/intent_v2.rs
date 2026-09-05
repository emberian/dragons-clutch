//! Runtime-product-width signed intent for the Direct successor.
//!
//! Native Ed25519 signs [`CompactIntentV2::signed_preimage`], not the bare
//! intent bytes. The preimage begins with the SHA-256 identity of the named
//! signature domain, so neither legacy Direct bytes nor another protocol's
//! message can be replayed as this authority.

use crate::{Error, array, byte, generated_intent_v2 as generated, put, put_byte, reserved, slice};

pub use generated::{
    CANCEL_THROUGH_BYTES_V2, CANCEL_THROUGH_MAGIC_V2, CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2,
    CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2, COMPACT_INTENT_BYTES_V2, COMPACT_INTENT_MAGIC_V2,
    COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
    COMPACT_INTENT_VERSION_V2,
};

/// Named domain whose SHA-256 identity prefixes every native Ed25519 message.
pub const COMPACT_INTENT_SIGNATURE_DOMAIN_PREIMAGE_V2: &[u8] =
    b"dclutch/signature/direct-compact-intent-v2";
/// Named domain whose identity prefixes every CancelThrough V2 signature.
pub const CANCEL_THROUGH_SIGNATURE_DOMAIN_PREIMAGE_V2: &[u8] =
    b"dclutch/signature/direct-cancel-through-v2";

/// The only independently signed intent admitted by the Direct successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactIntentV2 {
    /// Seller `0` or buyer `1`.
    pub side: u8,
    /// Inline FOK `0`, inline IOC `1`, or registered `2`.
    pub lifecycle: u8,
    /// Product-V2 runtime outcome coordinate.
    pub outcome: u32,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact next gap-free maker nonce.
    pub nonce: u64,
    /// First valid trusted Clock slot.
    pub valid_from: u64,
    /// Last valid trusted Clock slot, inclusive.
    pub valid_through: u64,
    /// Maximum admitted fill.
    pub maximum_fill: u64,
    /// Seller minimum or buyer maximum at the selected config's scale.
    pub limit_price: u64,
    /// Exact cumulative floor-fee rate accepted by the maker.
    pub fee_basis_points: u16,
    /// Seller destination or buyer source token account.
    pub collateral_account: [u8; 32],
}

impl CompactIntentV2 {
    /// Hostile-decode one exact canonical intent, excluding signature evidence.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != COMPACT_INTENT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(input, generated::COMPACT_INTENT_MAGIC_OFFSET_V2, 8)? != COMPACT_INTENT_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(input, generated::COMPACT_INTENT_VERSION_OFFSET_V2)?)
            != COMPACT_INTENT_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(input, generated::COMPACT_INTENT_RESERVED_A_OFFSET_V2, 4)?;
        reserved(input, generated::COMPACT_INTENT_RESERVED_B_OFFSET_V2, 6)?;
        Ok(Self {
            side: byte(input, generated::COMPACT_INTENT_SIDE_OFFSET_V2)?,
            lifecycle: byte(input, generated::COMPACT_INTENT_LIFECYCLE_OFFSET_V2)?,
            outcome: u32::from_le_bytes(array(input, generated::COMPACT_INTENT_OUTCOME_OFFSET_V2)?),
            market: array(input, generated::COMPACT_INTENT_MARKET_OFFSET_V2)?,
            generation: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_GENERATION_OFFSET_V2,
            )?),
            nonce: u64::from_le_bytes(array(input, generated::COMPACT_INTENT_NONCE_OFFSET_V2)?),
            valid_from: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            )?),
            valid_through: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            )?),
            maximum_fill: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            )?),
            limit_price: u64::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            )?),
            fee_basis_points: u16::from_le_bytes(array(
                input,
                generated::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            )?),
            collateral_account: array(
                input,
                generated::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            )?,
        })
    }

    /// Encode the exact canonical persisted intent bytes.
    pub fn encode(self) -> Result<[u8; COMPACT_INTENT_BYTES_V2], Error> {
        let mut output = [0_u8; COMPACT_INTENT_BYTES_V2];
        put(
            &mut output,
            generated::COMPACT_INTENT_MAGIC_OFFSET_V2,
            &COMPACT_INTENT_MAGIC_V2,
        )?;
        put(
            &mut output,
            generated::COMPACT_INTENT_VERSION_OFFSET_V2,
            &COMPACT_INTENT_VERSION_V2.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated::COMPACT_INTENT_SIDE_OFFSET_V2,
            self.side,
        )?;
        put_byte(
            &mut output,
            generated::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            self.lifecycle,
        )?;
        for (offset, value) in [
            (
                generated::COMPACT_INTENT_OUTCOME_OFFSET_V2,
                self.outcome.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_GENERATION_OFFSET_V2,
                self.generation.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_NONCE_OFFSET_V2,
                self.nonce.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
                self.valid_from.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
                self.valid_through.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
                self.maximum_fill.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
                self.limit_price.to_le_bytes().as_slice(),
            ),
            (
                generated::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
                self.fee_basis_points.to_le_bytes().as_slice(),
            ),
        ] {
            put(&mut output, offset, value)?;
        }
        put(
            &mut output,
            generated::COMPACT_INTENT_MARKET_OFFSET_V2,
            &self.market,
        )?;
        put(
            &mut output,
            generated::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            &self.collateral_account,
        )?;
        Ok(output)
    }

    /// Construct the exact native-Ed25519 message.
    pub fn signed_preimage(self) -> Result<[u8; COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2], Error> {
        let intent = self.encode()?;
        let mut output = [0_u8; COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2];
        put(&mut output, 0, &COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2)?;
        put(
            &mut output,
            COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2.len(),
            &intent,
        )?;
        Ok(output)
    }

    /// Hostile-decode the exact message authenticated by native Ed25519.
    pub fn decode_signed_preimage(input: &[u8]) -> Result<Self, Error> {
        if input.len() != COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(input, 0, COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2.len())?
            != COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2
        {
            return Err(Error::InvalidMagic);
        }
        Self::decode(slice(
            input,
            COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2.len(),
            COMPACT_INTENT_BYTES_V2,
        )?)
    }
}

/// Sole maker-signed O(1) invalidation threshold admitted by Direct V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancelThroughV2 {
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Lowest registered nonce that remains live.
    pub minimum_live_nonce: u64,
}

impl CancelThroughV2 {
    /// Hostile-decode one exact canonical kill-switch message.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != CANCEL_THROUGH_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(input, generated::CANCEL_THROUGH_MAGIC_OFFSET_V2, 8)? != CANCEL_THROUGH_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(input, generated::CANCEL_THROUGH_VERSION_OFFSET_V2)?)
            != COMPACT_INTENT_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(input, generated::CANCEL_THROUGH_RESERVED_OFFSET_V2, 6)?;
        Ok(Self {
            market: array(input, generated::CANCEL_THROUGH_MARKET_OFFSET_V2)?,
            generation: u64::from_le_bytes(array(
                input,
                generated::CANCEL_THROUGH_GENERATION_OFFSET_V2,
            )?),
            minimum_live_nonce: u64::from_le_bytes(array(
                input,
                generated::CANCEL_THROUGH_MINIMUM_LIVE_NONCE_OFFSET_V2,
            )?),
        })
    }

    /// Encode the exact canonical kill-switch bytes.
    pub fn encode(self) -> Result<[u8; CANCEL_THROUGH_BYTES_V2], Error> {
        let mut output = [0_u8; CANCEL_THROUGH_BYTES_V2];
        put(
            &mut output,
            generated::CANCEL_THROUGH_MAGIC_OFFSET_V2,
            &CANCEL_THROUGH_MAGIC_V2,
        )?;
        put(
            &mut output,
            generated::CANCEL_THROUGH_VERSION_OFFSET_V2,
            &COMPACT_INTENT_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CANCEL_THROUGH_MARKET_OFFSET_V2,
            &self.market,
        )?;
        put(
            &mut output,
            generated::CANCEL_THROUGH_GENERATION_OFFSET_V2,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated::CANCEL_THROUGH_MINIMUM_LIVE_NONCE_OFFSET_V2,
            &self.minimum_live_nonce.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Construct the exact native-Ed25519 kill-switch message.
    pub fn signed_preimage(self) -> Result<[u8; CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2], Error> {
        let message = self.encode()?;
        let mut output = [0_u8; CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2];
        put(&mut output, 0, &CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2)?;
        put(
            &mut output,
            CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2.len(),
            &message,
        )?;
        Ok(output)
    }

    /// Hostile-decode the exact kill-switch message authenticated by native Ed25519.
    pub fn decode_signed_preimage(input: &[u8]) -> Result<Self, Error> {
        if input.len() != CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if slice(input, 0, CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2.len())?
            != CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2
        {
            return Err(Error::InvalidMagic);
        }
        Self::decode(slice(
            input,
            CANCEL_THROUGH_SIGNATURE_DOMAIN_ID_V2.len(),
            CANCEL_THROUGH_BYTES_V2,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompactIntentV1, generated_intent_v2 as generated};

    fn fixture() -> CompactIntentV2 {
        CompactIntentV2 {
            side: 1,
            lifecycle: 2,
            outcome: 70_000,
            market: [0x21; 32],
            generation: 9,
            nonce: 12,
            valid_from: 100,
            valid_through: 200,
            maximum_fill: 5_000,
            limit_price: 600_000,
            fee_basis_points: 25,
            collateral_account: [0x45; 32],
        }
    }

    #[test]
    fn lean_owned_wide_outcome_and_signed_preimage_round_trip() {
        let intent = fixture();
        let bytes = intent.encode().expect("intent");
        assert_eq!(bytes, generated::COMPACT_INTENT_EXAMPLE_V2);
        assert_eq!(CompactIntentV2::decode(&bytes), Ok(intent));
        let signed = intent.signed_preimage().expect("signed preimage");
        assert_eq!(signed, generated::COMPACT_INTENT_SIGNED_EXAMPLE_V2);
        assert_eq!(CompactIntentV2::decode_signed_preimage(&signed), Ok(intent));
        assert_eq!(intent.outcome, 70_000);
    }

    #[test]
    fn legacy_and_substituted_signature_domains_refuse() {
        let legacy = CompactIntentV1 {
            side: 1,
            outcome: 1,
            lifecycle: 2,
            market: [0x21; 32],
            generation: 9,
            nonce: 12,
            valid_from: 100,
            valid_through: 200,
            maximum_fill: 5_000,
            limit_price: 600_000,
            fee_basis_points: 25,
            collateral_account: [0x45; 32],
        }
        .encode()
        .expect("legacy fixture");
        assert_eq!(CompactIntentV2::decode(&legacy), Err(Error::InvalidLength));

        let mut signed = fixture().signed_preimage().expect("signed preimage");
        signed[0] ^= 1;
        assert_eq!(
            CompactIntentV2::decode_signed_preimage(&signed),
            Err(Error::InvalidMagic)
        );
        let mut bytes = fixture().encode().expect("intent");
        bytes[generated::COMPACT_INTENT_RESERVED_A_OFFSET_V2] = 1;
        assert_eq!(CompactIntentV2::decode(&bytes), Err(Error::NonzeroReserved));
    }

    #[test]
    fn cancel_through_has_one_separate_v2_signature_domain() {
        let message = CancelThroughV2 {
            market: [0x21; 32],
            generation: 9,
            minimum_live_nonce: 13,
        };
        let bytes = message.encode().expect("kill switch");
        assert_eq!(bytes, generated::CANCEL_THROUGH_EXAMPLE_V2);
        assert_eq!(CancelThroughV2::decode(&bytes), Ok(message));
        let signed = message.signed_preimage().expect("signed kill switch");
        assert_eq!(signed, generated::CANCEL_THROUGH_SIGNED_EXAMPLE_V2);
        assert_eq!(
            CancelThroughV2::decode_signed_preimage(&signed),
            Ok(message)
        );

        let mut wrong_domain = signed;
        wrong_domain[0] ^= 1;
        assert_eq!(
            CancelThroughV2::decode_signed_preimage(&wrong_domain),
            Err(Error::InvalidMagic)
        );
        let mut reserved_bytes = bytes;
        reserved_bytes[generated::CANCEL_THROUGH_RESERVED_OFFSET_V2] = 1;
        assert_eq!(
            CancelThroughV2::decode(&reserved_bytes),
            Err(Error::NonzeroReserved)
        );
    }
}
