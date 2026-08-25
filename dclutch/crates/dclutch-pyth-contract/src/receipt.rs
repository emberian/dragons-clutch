//! Canonical fixed-width result receipt for Pyth resolution.

use crate::{Error, Result, array, nonzero, zero};

/// Exact byte width of [`ResolutionReceiptV1`].
pub const RECEIPT_BYTES: usize = 128;
/// Receipt magic.
pub const RECEIPT_MAGIC: [u8; 8] = *b"DCLTRCP1";
/// Implemented receipt schema.
pub const RECEIPT_SCHEMA_VERSION: u16 = 1;

const KIND_EMPTY: u8 = 0;
const KIND_PRICE: u8 = 1;
const KIND_FAILURE: u8 = 2;

/// One clock observation retained for a failure receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Clock {
    /// Observed slot.
    pub slot: u64,
    /// Observed Unix timestamp.
    pub unix_timestamp: i64,
}

/// Provider facts required to make a price receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceInput {
    /// Winning outcome index.
    pub winner: u8,
    /// Slot at which the update was posted.
    pub posted_slot: u64,
    /// Slot at which the update was consumed.
    pub consumed_slot: u64,
    /// Observed consumption timestamp.
    pub consumed_unix_timestamp: i64,
    /// Provider previous publish time.
    pub previous_publish_time: i64,
    /// Provider current publish time.
    pub publish_time: i64,
    /// Provider price integer.
    pub price: i64,
    /// Provider confidence integer.
    pub confidence: u64,
    /// Provider decimal exponent.
    pub exponent: i32,
    /// SHA-256 digest of the exact Pyth `PostUpdateParams` body.
    pub post_params_body_digest: [u8; 32],
}

/// The canonical receipt kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptKind {
    /// No result was posted.
    Empty,
    /// A price result was posted.
    Price,
    /// Resolution failed with a retained clock.
    Failure,
}

/// Exact decoded resolution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionReceiptV1 {
    kind: ReceiptKind,
    winner: u8,
    posted_slot: u64,
    consumed_slot: u64,
    consumed_unix_timestamp: i64,
    previous_publish_time: i64,
    publish_time: i64,
    price: i64,
    confidence: u64,
    exponent: i32,
    post_params_body_digest: [u8; 32],
}

impl ResolutionReceiptV1 {
    /// Construct the canonical empty receipt after validating outcome count.
    pub fn empty(outcome_count: u8) -> Result<Self> {
        validate_count(outcome_count)?;
        Ok(Self {
            kind: ReceiptKind::Empty,
            winner: 0,
            posted_slot: 0,
            consumed_slot: 0,
            consumed_unix_timestamp: 0,
            previous_publish_time: 0,
            publish_time: 0,
            price: 0,
            confidence: 0,
            exponent: 0,
            post_params_body_digest: [0; 32],
        })
    }

    /// Construct a canonical price receipt.
    pub fn price(input: PriceInput, outcome_count: u8) -> Result<Self> {
        validate_count(outcome_count)?;
        if input.winner >= outcome_count {
            return Err(Error::InvalidWinner);
        }
        if !nonzero(&input.post_params_body_digest) {
            return Err(Error::ZeroIdentifier);
        }
        if input.previous_publish_time >= input.publish_time {
            return Err(Error::InvalidPublishTimes);
        }
        if input.consumed_slot != input.posted_slot {
            return Err(Error::SlotMismatch);
        }
        Ok(Self {
            kind: ReceiptKind::Price,
            winner: input.winner,
            posted_slot: input.posted_slot,
            consumed_slot: input.consumed_slot,
            consumed_unix_timestamp: input.consumed_unix_timestamp,
            previous_publish_time: input.previous_publish_time,
            publish_time: input.publish_time,
            price: input.price,
            confidence: input.confidence,
            exponent: input.exponent,
            post_params_body_digest: input.post_params_body_digest,
        })
    }

    /// Construct a canonical failure receipt.  Provider and price fields are
    /// deliberately absent; the consumption clock remains evidence.
    pub fn failure(winner: u8, outcome_count: u8, clock: Clock) -> Result<Self> {
        validate_count(outcome_count)?;
        if winner >= outcome_count {
            return Err(Error::InvalidWinner);
        }
        Ok(Self {
            kind: ReceiptKind::Failure,
            winner,
            posted_slot: 0,
            consumed_slot: clock.slot,
            consumed_unix_timestamp: clock.unix_timestamp,
            previous_publish_time: 0,
            publish_time: 0,
            price: 0,
            confidence: 0,
            exponent: 0,
            post_params_body_digest: [0; 32],
        })
    }

    /// Decode the exact canonical receipt using its external outcome count.
    pub fn decode(bytes: &[u8], outcome_count: u8) -> Result<Self> {
        validate_count(outcome_count)?;
        if bytes.len() != RECEIPT_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != RECEIPT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != RECEIPT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if !zero(bytes.get(12..16).ok_or(Error::InvalidLength)?)
            || !zero(bytes.get(76..80).ok_or(Error::InvalidLength)?)
            || !zero(bytes.get(112..128).ok_or(Error::InvalidLength)?)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let winner = *bytes.get(11).ok_or(Error::InvalidLength)?;
        let posted_slot = u64::from_le_bytes(array(bytes, 16)?);
        let consumed_slot = u64::from_le_bytes(array(bytes, 24)?);
        let consumed_unix_timestamp = i64::from_le_bytes(array(bytes, 32)?);
        let previous_publish_time = i64::from_le_bytes(array(bytes, 40)?);
        let publish_time = i64::from_le_bytes(array(bytes, 48)?);
        let price = i64::from_le_bytes(array(bytes, 56)?);
        let confidence = u64::from_le_bytes(array(bytes, 64)?);
        let exponent = i32::from_le_bytes(array(bytes, 72)?);
        let post_params_body_digest = array(bytes, 80)?;
        match *bytes.get(10).ok_or(Error::InvalidLength)? {
            KIND_EMPTY => {
                let receipt = Self::empty(outcome_count)?;
                if winner != 0
                    || posted_slot != 0
                    || consumed_slot != 0
                    || consumed_unix_timestamp != 0
                    || previous_publish_time != 0
                    || publish_time != 0
                    || price != 0
                    || confidence != 0
                    || exponent != 0
                    || !zero(&post_params_body_digest)
                {
                    return Err(Error::NonCanonicalReceipt);
                }
                Ok(receipt)
            }
            KIND_PRICE => Self::price(
                PriceInput {
                    winner,
                    posted_slot,
                    consumed_slot,
                    consumed_unix_timestamp,
                    previous_publish_time,
                    publish_time,
                    price,
                    confidence,
                    exponent,
                    post_params_body_digest,
                },
                outcome_count,
            ),
            KIND_FAILURE => {
                let receipt = Self::failure(
                    winner,
                    outcome_count,
                    Clock {
                        slot: consumed_slot,
                        unix_timestamp: consumed_unix_timestamp,
                    },
                )?;
                if posted_slot != 0
                    || previous_publish_time != 0
                    || publish_time != 0
                    || price != 0
                    || confidence != 0
                    || exponent != 0
                    || !zero(&post_params_body_digest)
                {
                    return Err(Error::NonCanonicalReceipt);
                }
                Ok(receipt)
            }
            _ => Err(Error::InvalidReceiptKind),
        }
    }

    /// Encode this receipt into its exact canonical fixed-width bytes.
    pub fn to_bytes(self) -> [u8; RECEIPT_BYTES] {
        let mut out = [0; RECEIPT_BYTES];
        out[..8].copy_from_slice(&RECEIPT_MAGIC);
        out[8..10].copy_from_slice(&RECEIPT_SCHEMA_VERSION.to_le_bytes());
        out[10] = match self.kind {
            ReceiptKind::Empty => KIND_EMPTY,
            ReceiptKind::Price => KIND_PRICE,
            ReceiptKind::Failure => KIND_FAILURE,
        };
        out[11] = self.winner;
        out[16..24].copy_from_slice(&self.posted_slot.to_le_bytes());
        out[24..32].copy_from_slice(&self.consumed_slot.to_le_bytes());
        out[32..40].copy_from_slice(&self.consumed_unix_timestamp.to_le_bytes());
        out[40..48].copy_from_slice(&self.previous_publish_time.to_le_bytes());
        out[48..56].copy_from_slice(&self.publish_time.to_le_bytes());
        out[56..64].copy_from_slice(&self.price.to_le_bytes());
        out[64..72].copy_from_slice(&self.confidence.to_le_bytes());
        out[72..76].copy_from_slice(&self.exponent.to_le_bytes());
        out[80..112].copy_from_slice(&self.post_params_body_digest);
        out
    }

    /// Encode into an exact-width caller buffer without changing it on refusal.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != RECEIPT_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return receipt kind.
    pub const fn kind(&self) -> ReceiptKind {
        self.kind
    }
    /// Return winner index.
    pub const fn winner(&self) -> u8 {
        self.winner
    }
    /// Return posted slot.
    pub const fn posted_slot(&self) -> u64 {
        self.posted_slot
    }
    /// Return consumed slot.
    pub const fn consumed_slot(&self) -> u64 {
        self.consumed_slot
    }
    /// Return consumption Unix timestamp.
    pub const fn consumed_unix_timestamp(&self) -> i64 {
        self.consumed_unix_timestamp
    }
    /// Return previous provider publish time.
    pub const fn previous_publish_time(&self) -> i64 {
        self.previous_publish_time
    }
    /// Return provider publish time.
    pub const fn publish_time(&self) -> i64 {
        self.publish_time
    }
    /// Return provider price.
    pub const fn price_value(&self) -> i64 {
        self.price
    }
    /// Return provider confidence.
    pub const fn confidence(&self) -> u64 {
        self.confidence
    }
    /// Return provider exponent.
    pub const fn exponent(&self) -> i32 {
        self.exponent
    }
    /// Return SHA-256 of the exact Pyth `PostUpdateParams` body.
    pub const fn post_params_body_digest(&self) -> &[u8; 32] {
        &self.post_params_body_digest
    }
}

fn validate_count(outcome_count: u8) -> Result<()> {
    if !(2..=16).contains(&outcome_count) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PriceInput {
        PriceInput {
            winner: 1,
            posted_slot: 0,
            consumed_slot: 0,
            consumed_unix_timestamp: -2,
            previous_publish_time: 3,
            publish_time: 4,
            price: -5,
            confidence: 6,
            exponent: -7,
            post_params_body_digest: [1; 32],
        }
    }

    #[test]
    fn all_kinds_round_trip() {
        let price = ResolutionReceiptV1::price(input(), 2).expect("valid price");
        assert_eq!(ResolutionReceiptV1::decode(&price.to_bytes(), 2), Ok(price));
        let failure = ResolutionReceiptV1::failure(
            1,
            2,
            Clock {
                slot: 0,
                unix_timestamp: -1,
            },
        )
        .expect("valid failure");
        assert_eq!(
            ResolutionReceiptV1::decode(&failure.to_bytes(), 2),
            Ok(failure)
        );
        let empty = ResolutionReceiptV1::empty(2).expect("valid empty");
        assert_eq!(ResolutionReceiptV1::decode(&empty.to_bytes(), 2), Ok(empty));
    }

    #[test]
    fn hostile_receipts_refuse() {
        let receipt = ResolutionReceiptV1::price(input(), 2).expect("valid price");
        let bytes = receipt.to_bytes();
        for length in 0..RECEIPT_BYTES {
            if let Some(short) = bytes.get(..length) {
                assert_eq!(
                    ResolutionReceiptV1::decode(short, 2),
                    Err(Error::InvalidLength)
                );
            }
        }
        assert_eq!(
            ResolutionReceiptV1::decode(&[0; 129], 2),
            Err(Error::InvalidLength)
        );
        assert_eq!(
            ResolutionReceiptV1::empty(1),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            ResolutionReceiptV1::empty(17),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            ResolutionReceiptV1::price(
                PriceInput {
                    winner: 2,
                    ..input()
                },
                2
            ),
            Err(Error::InvalidWinner)
        );
        assert_eq!(
            ResolutionReceiptV1::price(
                PriceInput {
                    post_params_body_digest: [0; 32],
                    ..input()
                },
                2
            ),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            ResolutionReceiptV1::price(
                PriceInput {
                    previous_publish_time: 4,
                    ..input()
                },
                2
            ),
            Err(Error::InvalidPublishTimes)
        );
        assert_eq!(
            ResolutionReceiptV1::price(
                PriceInput {
                    consumed_slot: 1,
                    ..input()
                },
                2
            ),
            Err(Error::SlotMismatch)
        );
        let mut changed = bytes;
        if let Some(slot) = changed.get_mut(10) {
            *slot = 9;
        }
        assert_eq!(
            ResolutionReceiptV1::decode(&changed, 2),
            Err(Error::InvalidReceiptKind)
        );
        let mut changed = ResolutionReceiptV1::empty(2)
            .expect("valid empty")
            .to_bytes();
        if let Some(slot) = changed.get_mut(12) {
            *slot = 1;
        }
        assert_eq!(
            ResolutionReceiptV1::decode(&changed, 2),
            Err(Error::NonCanonicalReservedBytes)
        );
    }
}
