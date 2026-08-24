//! Exact Pyth Receiver SDK 2.0.0 full-price-update decoding.

/// The Anchor discriminator for Pyth Receiver SDK 2.0.0 `PriceUpdateV2`.
pub const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];

/// The only accepted serialized length for a full `PriceUpdateV2`.
pub const FULL_PRICE_UPDATE_V2_LEN: usize = 134;

/// Error returned while decoding an untrusted Pyth `PriceUpdateV2` byte slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriceUpdateV2Error {
    /// The input was not exactly [`FULL_PRICE_UPDATE_V2_LEN`] bytes.
    InvalidLength {
        /// Observed number of input bytes.
        actual: usize,
    },
    /// The eight-byte Anchor account discriminator was not `PriceUpdateV2`.
    InvalidDiscriminator,
    /// The Borsh `VerificationLevel` tag was not the `Full` tag (one).
    NotFullyVerified {
        /// Observed Borsh enum tag.
        tag: u8,
    },
    /// The final allocation byte was nonzero and therefore noncanonical.
    NonzeroAllocationTail {
        /// Observed final allocation byte.
        value: u8,
    },
}

/// Result alias for exact Pyth Receiver price-update parsing.
pub type PriceUpdateV2Result<T> = core::result::Result<T, PriceUpdateV2Error>;

/// A fully verified exact view of Pyth Receiver SDK 2.0.0 `PriceUpdateV2`.
///
/// This type validates the account's fixed 134-byte full-verification shape.
/// It does not authenticate account ownership, account address, feed policy,
/// freshness, or the release that selected this ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullPriceUpdateV2 {
    write_authority: [u8; 32],
    feed_id: [u8; 32],
    price: i64,
    confidence: u64,
    exponent: i32,
    publish_time: i64,
    prev_publish_time: i64,
    ema_price: i64,
    ema_confidence: u64,
    posted_slot: u64,
}

impl FullPriceUpdateV2 {
    /// Parse one exact fully verified Pyth Receiver SDK 2.0.0 price update.
    pub fn parse(bytes: &[u8]) -> PriceUpdateV2Result<Self> {
        if bytes.len() != FULL_PRICE_UPDATE_V2_LEN {
            return Err(PriceUpdateV2Error::InvalidLength {
                actual: bytes.len(),
            });
        }
        if bytes.get(..8) != Some(&PRICE_UPDATE_V2_DISCRIMINATOR) {
            return Err(PriceUpdateV2Error::InvalidDiscriminator);
        }
        let verification_tag = byte_at(bytes, 40)?;
        if verification_tag != 1 {
            return Err(PriceUpdateV2Error::NotFullyVerified {
                tag: verification_tag,
            });
        }
        let allocation_tail = byte_at(bytes, 133)?;
        if allocation_tail != 0 {
            return Err(PriceUpdateV2Error::NonzeroAllocationTail {
                value: allocation_tail,
            });
        }
        Ok(Self {
            write_authority: array_at(bytes, 8)?,
            feed_id: array_at(bytes, 41)?,
            price: i64_at(bytes, 73)?,
            confidence: u64_at(bytes, 81)?,
            exponent: i32_at(bytes, 89)?,
            publish_time: i64_at(bytes, 93)?,
            prev_publish_time: i64_at(bytes, 101)?,
            ema_price: i64_at(bytes, 109)?,
            ema_confidence: u64_at(bytes, 117)?,
            posted_slot: u64_at(bytes, 125)?,
        })
    }

    /// Return the account write authority selected by the serialized update.
    pub const fn write_authority(&self) -> [u8; 32] {
        self.write_authority
    }

    /// Return the Pyth feed identifier.
    pub const fn feed_id(&self) -> [u8; 32] {
        self.feed_id
    }

    /// Return the raw signed price integer.
    pub const fn price(&self) -> i64 {
        self.price
    }

    /// Return the raw unsigned confidence interval half-width.
    pub const fn confidence(&self) -> u64 {
        self.confidence
    }

    /// Return the base-ten exponent for the price and confidence integers.
    pub const fn exponent(&self) -> i32 {
        self.exponent
    }

    /// Return the Unix publication timestamp.
    pub const fn publish_time(&self) -> i64 {
        self.publish_time
    }

    /// Return the predecessor publication timestamp.
    pub const fn prev_publish_time(&self) -> i64 {
        self.prev_publish_time
    }

    /// Return the raw exponential-moving-average price integer.
    pub const fn ema_price(&self) -> i64 {
        self.ema_price
    }

    /// Return the raw exponential-moving-average confidence integer.
    pub const fn ema_confidence(&self) -> u64 {
        self.ema_confidence
    }

    /// Return the Solana slot at which the update was posted.
    pub const fn posted_slot(&self) -> u64 {
        self.posted_slot
    }
}

fn byte_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })
}

fn array_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?;
    bytes
        .get(offset..end)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })
}

fn i64_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<i64> {
    Ok(i64::from_le_bytes(eight_at(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<u64> {
    Ok(u64::from_le_bytes(eight_at(bytes, offset)?))
}

fn i32_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<i32> {
    let end = offset
        .checked_add(4)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?;
    let field: [u8; 4] = bytes
        .get(offset..end)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?;
    Ok(i32::from_le_bytes(field))
}

fn eight_at(bytes: &[u8], offset: usize) -> PriceUpdateV2Result<[u8; 8]> {
    let end = offset
        .checked_add(8)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?;
    bytes
        .get(offset..end)
        .ok_or(PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| PriceUpdateV2Error::InvalidLength {
            actual: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put<const N: usize>(
        bytes: &mut [u8],
        offset: usize,
        value: [u8; N],
    ) -> PriceUpdateV2Result<()> {
        let actual = bytes.len();
        let end = offset
            .checked_add(N)
            .ok_or(PriceUpdateV2Error::InvalidLength { actual })?;
        bytes
            .get_mut(offset..end)
            .ok_or(PriceUpdateV2Error::InvalidLength { actual })?
            .copy_from_slice(&value);
        Ok(())
    }

    fn fixture() -> PriceUpdateV2Result<[u8; FULL_PRICE_UPDATE_V2_LEN]> {
        let mut bytes = [0_u8; FULL_PRICE_UPDATE_V2_LEN];
        put(&mut bytes, 0, PRICE_UPDATE_V2_DISCRIMINATOR)?;
        put(&mut bytes, 8, [0x11; 32])?;
        put(&mut bytes, 40, [1])?;
        put(&mut bytes, 41, [0x22; 32])?;
        put(&mut bytes, 73, (-123_456_789_i64).to_le_bytes())?;
        put(&mut bytes, 81, 987_654_321_u64.to_le_bytes())?;
        put(&mut bytes, 89, (-8_i32).to_le_bytes())?;
        put(&mut bytes, 93, 1_700_000_000_i64.to_le_bytes())?;
        put(&mut bytes, 101, 1_699_999_999_i64.to_le_bytes())?;
        put(&mut bytes, 109, (-222_333_i64).to_le_bytes())?;
        put(&mut bytes, 117, 444_555_u64.to_le_bytes())?;
        put(&mut bytes, 125, 123_456_789_u64.to_le_bytes())?;
        Ok(bytes)
    }

    #[test]
    fn exact_offsets_little_endian_and_round_trip_fixture() -> PriceUpdateV2Result<()> {
        let bytes = fixture()?;
        let parsed = FullPriceUpdateV2::parse(&bytes)?;
        assert_eq!(parsed.write_authority(), [0x11; 32]);
        assert_eq!(parsed.feed_id(), [0x22; 32]);
        assert_eq!(parsed.price(), -123_456_789);
        assert_eq!(parsed.confidence(), 987_654_321);
        assert_eq!(parsed.exponent(), -8);
        assert_eq!(parsed.publish_time(), 1_700_000_000);
        assert_eq!(parsed.prev_publish_time(), 1_699_999_999);
        assert_eq!(parsed.ema_price(), -222_333);
        assert_eq!(parsed.ema_confidence(), 444_555);
        assert_eq!(parsed.posted_slot(), 123_456_789);
        Ok(())
    }

    #[test]
    fn every_truncation_and_a_long_input_refuse() -> PriceUpdateV2Result<()> {
        let bytes = fixture()?;
        for length in 0..FULL_PRICE_UPDATE_V2_LEN {
            let truncated = bytes
                .get(..length)
                .ok_or(PriceUpdateV2Error::InvalidLength { actual: length })?;
            assert_eq!(
                FullPriceUpdateV2::parse(truncated),
                Err(PriceUpdateV2Error::InvalidLength { actual: length })
            );
        }
        let mut long = [0_u8; FULL_PRICE_UPDATE_V2_LEN + 1];
        put(&mut long, 0, bytes)?;
        assert_eq!(
            FullPriceUpdateV2::parse(&long),
            Err(PriceUpdateV2Error::InvalidLength {
                actual: FULL_PRICE_UPDATE_V2_LEN + 1
            })
        );
        Ok(())
    }

    #[test]
    fn discriminator_verification_tag_and_tail_are_hostile_inputs() -> PriceUpdateV2Result<()> {
        let mut discriminator = fixture()?;
        put(&mut discriminator, 0, [0_u8; 8])?;
        assert_eq!(
            FullPriceUpdateV2::parse(&discriminator),
            Err(PriceUpdateV2Error::InvalidDiscriminator)
        );

        let mut partial = fixture()?;
        put(&mut partial, 40, [0])?;
        assert_eq!(
            FullPriceUpdateV2::parse(&partial),
            Err(PriceUpdateV2Error::NotFullyVerified { tag: 0 })
        );
        let mut unknown = fixture()?;
        put(&mut unknown, 40, [2])?;
        assert_eq!(
            FullPriceUpdateV2::parse(&unknown),
            Err(PriceUpdateV2Error::NotFullyVerified { tag: 2 })
        );

        let mut tail = fixture()?;
        put(&mut tail, 133, [1])?;
        assert_eq!(
            FullPriceUpdateV2::parse(&tail),
            Err(PriceUpdateV2Error::NonzeroAllocationTail { value: 1 })
        );
        Ok(())
    }
}
