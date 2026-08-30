//! Typed Custody reservation and rollback receipt for Dealer acceptance.
//!
//! Reservation is deliberately distinct from final value delivery. Custody
//! locks one exact effect amount under a checkpoint-scoped reservation; only
//! after every reservation exists may Trading activate Claims and the
//! obligation. If the checkpoint expires first, rollback receipts release the
//! reservations in reverse order before Trading may refund checkpoint rent.

use super::{Error as CodecError, array_at, byte_at, put, put_byte, require_zero};

/// Maximum active Custody effects in one Dealer scenario fill.
pub const DEALER_SCENARIO_MAX_RESERVATIONS_V1: usize = 4;
/// Exact reservation-receipt wire width.
pub const DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1: usize = 304;
/// Canonical receipt magic.
pub const DEALER_SCENARIO_RESERVATION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDRR1";
/// Implemented receipt version.
pub const DEALER_SCENARIO_RESERVATION_RECEIPT_VERSION_V1: u16 = 1;
/// Custody-owned PDA domain for one reservation receipt.
pub const DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-reserve:v1";

const _: () = assert!(
    DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1.len()
        <= crate::scenario_custody_reservation_v1::MAX_PDA_SEED_BYTES_V1,
    "the reservation receipt domain must be a usable PDA seed"
);

const VERSION_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const ORDINAL_OFFSET: usize = 11;
const EFFECT_COUNT_OFFSET: usize = 12;
const RESERVED_OFFSET: usize = 13;
const RESERVED_BYTES: usize = 3;
const PRODUCER_OFFSET: usize = 16;
const CHECKPOINT_OFFSET: usize = 48;
const CHECKPOINT_PRESTATE_OFFSET: usize = 80;
const REQUEST_OFFSET: usize = 112;
const EFFECTS_OFFSET: usize = 144;
const RESERVATION_OFFSET: usize = 176;
const RESERVATION_PRESTATE_OFFSET: usize = 208;
const RESERVATION_POSTSTATE_OFFSET: usize = 240;
const PRIOR_RECEIPT_OFFSET: usize = 272;

/// Durable Custody reservation transition.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioReservationActionV1 {
    /// Lock one effect without delivering its final economic destination.
    Reserve = 1,
    /// Release one earlier reservation after checkpoint expiry.
    Rollback = 2,
}

impl DealerScenarioReservationActionV1 {
    fn decode(value: u8) -> Result<Self, DealerScenarioReservationReceiptErrorV1> {
        match value {
            1 => Ok(Self::Reserve),
            2 => Ok(Self::Rollback),
            _ => Err(DealerScenarioReservationReceiptErrorV1::Coordinate),
        }
    }
}

/// Exact producer-owned reservation transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservationReceiptV1 {
    /// Reserve or rollback.
    pub action: DealerScenarioReservationActionV1,
    /// Zero-based effect ordinal.
    pub effect_ordinal: u8,
    /// Exact active effect count selected by the evaluator.
    pub effect_count: u8,
    /// Current release-selected Custody program.
    pub producer_program: [u8; 32],
    /// Trading checkpoint whose effect is reserved.
    pub checkpoint: [u8; 32],
    /// Exact Trading checkpoint bytes observed by Custody.
    pub checkpoint_prestate_digest: [u8; 32],
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Commitment to the complete ordered effect bank.
    pub effects_digest: [u8; 32],
    /// Checkpoint- and ordinal-scoped Custody reservation account.
    pub reservation: [u8; 32],
    /// Reservation account prestate digest.
    pub reservation_prestate_digest: [u8; 32],
    /// Reservation account poststate digest.
    pub reservation_poststate_digest: [u8; 32],
    /// Zero for Reserve; exact Reserve receipt digest for Rollback.
    pub prior_receipt_digest: [u8; 32],
}

/// Stable hostile-decoding refusal for a reservation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioReservationReceiptErrorV1 {
    /// Fixed-layout bytes were malformed.
    Codec(CodecError),
    /// An action, ordinal, identity, or digest was not canonical.
    Coordinate,
}

impl From<CodecError> for DealerScenarioReservationReceiptErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl DealerScenarioReservationReceiptV1 {
    /// Hostile-decode one exact receipt body.
    pub fn decode(bytes: &[u8]) -> Result<Self, DealerScenarioReservationReceiptErrorV1> {
        if bytes.len() != DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1 {
            return Err(DealerScenarioReservationReceiptErrorV1::Codec(
                CodecError::InvalidLength,
            ));
        }
        if bytes.get(..8) != Some(DEALER_SCENARIO_RESERVATION_RECEIPT_MAGIC_V1.as_slice()) {
            return Err(DealerScenarioReservationReceiptErrorV1::Codec(
                CodecError::InvalidMagic,
            ));
        }
        let version = bytes
            .get(VERSION_OFFSET..VERSION_OFFSET + 2)
            .ok_or(CodecError::InvalidLength)?;
        if version != DEALER_SCENARIO_RESERVATION_RECEIPT_VERSION_V1.to_le_bytes() {
            return Err(DealerScenarioReservationReceiptErrorV1::Codec(
                CodecError::UnsupportedVersion,
            ));
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let receipt = Self {
            action: DealerScenarioReservationActionV1::decode(byte_at(bytes, ACTION_OFFSET)?)?,
            effect_ordinal: byte_at(bytes, ORDINAL_OFFSET)?,
            effect_count: byte_at(bytes, EFFECT_COUNT_OFFSET)?,
            producer_program: array_at(bytes, PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, CHECKPOINT_OFFSET)?,
            checkpoint_prestate_digest: array_at(bytes, CHECKPOINT_PRESTATE_OFFSET)?,
            request_digest: array_at(bytes, REQUEST_OFFSET)?,
            effects_digest: array_at(bytes, EFFECTS_OFFSET)?,
            reservation: array_at(bytes, RESERVATION_OFFSET)?,
            reservation_prestate_digest: array_at(bytes, RESERVATION_PRESTATE_OFFSET)?,
            reservation_poststate_digest: array_at(bytes, RESERVATION_POSTSTATE_OFFSET)?,
            prior_receipt_digest: array_at(bytes, PRIOR_RECEIPT_OFFSET)?,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Encode one exact receipt body.
    pub fn encode(
        self,
    ) -> Result<
        [u8; DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1],
        DealerScenarioReservationReceiptErrorV1,
    > {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1];
        put(&mut bytes, 0, &DEALER_SCENARIO_RESERVATION_RECEIPT_MAGIC_V1)?;
        put(
            &mut bytes,
            VERSION_OFFSET,
            &DEALER_SCENARIO_RESERVATION_RECEIPT_VERSION_V1.to_le_bytes(),
        )?;
        put_byte(&mut bytes, ACTION_OFFSET, self.action as u8)?;
        put_byte(&mut bytes, ORDINAL_OFFSET, self.effect_ordinal)?;
        put_byte(&mut bytes, EFFECT_COUNT_OFFSET, self.effect_count)?;
        for (offset, value) in [
            (PRODUCER_OFFSET, self.producer_program),
            (CHECKPOINT_OFFSET, self.checkpoint),
            (CHECKPOINT_PRESTATE_OFFSET, self.checkpoint_prestate_digest),
            (REQUEST_OFFSET, self.request_digest),
            (EFFECTS_OFFSET, self.effects_digest),
            (RESERVATION_OFFSET, self.reservation),
            (
                RESERVATION_PRESTATE_OFFSET,
                self.reservation_prestate_digest,
            ),
            (
                RESERVATION_POSTSTATE_OFFSET,
                self.reservation_poststate_digest,
            ),
            (PRIOR_RECEIPT_OFFSET, self.prior_receipt_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        Ok(bytes)
    }

    fn validate(self) -> Result<(), DealerScenarioReservationReceiptErrorV1> {
        if self.effect_count == 0
            || usize::from(self.effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || self.effect_ordinal >= self.effect_count
            || [
                self.producer_program,
                self.checkpoint,
                self.checkpoint_prestate_digest,
                self.request_digest,
                self.effects_digest,
                self.reservation,
                self.reservation_prestate_digest,
                self.reservation_poststate_digest,
            ]
            .contains(&[0; 32])
            || match self.action {
                DealerScenarioReservationActionV1::Reserve => self.prior_receipt_digest != [0; 32],
                DealerScenarioReservationActionV1::Rollback => self.prior_receipt_digest == [0; 32],
            }
        {
            Err(DealerScenarioReservationReceiptErrorV1::Coordinate)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(action: DealerScenarioReservationActionV1) -> DealerScenarioReservationReceiptV1 {
        DealerScenarioReservationReceiptV1 {
            action,
            effect_ordinal: 1,
            effect_count: 3,
            producer_program: [1; 32],
            checkpoint: [2; 32],
            checkpoint_prestate_digest: [3; 32],
            request_digest: [4; 32],
            effects_digest: [5; 32],
            reservation: [6; 32],
            reservation_prestate_digest: [7; 32],
            reservation_poststate_digest: [8; 32],
            prior_receipt_digest: if action == DealerScenarioReservationActionV1::Rollback {
                [9; 32]
            } else {
                [0; 32]
            },
        }
    }

    #[test]
    fn reserve_and_rollback_are_distinct_and_exact() {
        for action in [
            DealerScenarioReservationActionV1::Reserve,
            DealerScenarioReservationActionV1::Rollback,
        ] {
            let value = receipt(action);
            let bytes = value.encode().expect("receipt");
            assert_eq!(
                DealerScenarioReservationReceiptV1::decode(&bytes),
                Ok(value)
            );
        }
        let mut hostile = receipt(DealerScenarioReservationActionV1::Rollback);
        hostile.prior_receipt_digest = [0; 32];
        assert_eq!(
            hostile.encode(),
            Err(DealerScenarioReservationReceiptErrorV1::Coordinate)
        );
    }
}
