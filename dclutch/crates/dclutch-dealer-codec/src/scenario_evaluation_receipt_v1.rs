//! Fixed receipt for one paged Dealer scenario evaluation.
//!
//! The receipt is deliberately a commitment, not an assertion that the
//! candidate is semantically valid. Its producer must still be authenticated
//! through the release-selected admitted-accelerator chain by the Solana
//! adapter. Once that authority is established, this wire prevents the
//! producer, checkpoint, transcript, or candidate resources from being mixed.

use super::{Error as CodecError, array_at, byte_at, put, put_byte, require_zero};
use crate::scenario_reservation_receipt_v1::DEALER_SCENARIO_MAX_RESERVATIONS_V1;

/// Canonical evaluation-receipt magic.
pub const DEALER_SCENARIO_EVALUATION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDER1";
/// Implemented evaluation-receipt schema version.
pub const DEALER_SCENARIO_EVALUATION_RECEIPT_VERSION_V1: u16 = 1;
/// Exact fixed receipt width.
pub const DEALER_SCENARIO_EVALUATION_RECEIPT_BYTES_V1: usize = 336;
/// Producer-owned PDA domain for one checkpoint-scoped receipt.
pub const DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-eval:v1";

const VERSION_OFFSET: usize = 8;
const EFFECT_COUNT_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 11;
const RESERVED_BYTES: usize = 5;
const PRODUCER_OFFSET: usize = 16;
const CHECKPOINT_OFFSET: usize = 48;
const CHECKPOINT_PRESTATE_OFFSET: usize = 80;
const REQUEST_OFFSET: usize = 112;
const CLAIMS_PRESTATE_OFFSET: usize = 144;
const CUSTODY_PRESTATE_OFFSET: usize = 176;
const CANDIDATE_BANK_OFFSET: usize = 208;
const CANDIDATE_OBLIGATION_OFFSET: usize = 240;
const CLAIMS_DELTA_OFFSET: usize = 272;
const EFFECTS_OFFSET: usize = 304;

/// Exact producer- and transcript-bound evaluation commitments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioEvaluationReceiptV1 {
    /// Number of canonical Custody effects requiring reservation.
    pub custody_effect_count: u8,
    /// Release-authenticated evaluator Program expected to own this receipt.
    pub producer_program: [u8; 32],
    /// Trading-owned checkpoint this receipt evaluates.
    pub checkpoint: [u8; 32],
    /// SHA-256 of the exact checkpoint body evaluated by the producer.
    pub checkpoint_prestate_digest: [u8; 32],
    /// Exact Dealer request digest bound at checkpoint creation.
    pub request_digest: [u8; 32],
    /// Claims-domain digest of the complete ordered membership transcript.
    pub claims_prestate_digest: [u8; 32],
    /// Custody-domain digest of the complete ordered membership transcript.
    pub custody_prestate_digest: [u8; 32],
    /// Exact selected scalar-then-identity candidate bank digest.
    pub candidate_bank_digest: [u8; 32],
    /// Exact candidate obligation body digest.
    pub candidate_obligation_digest: [u8; 32],
    /// Exact expected Claims delta body digest.
    pub claims_delta_digest: [u8; 32],
    /// Exact ordered Custody-effects body digest.
    pub effects_digest: [u8; 32],
}

/// Stable hostile-decoding refusal for an evaluation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioEvaluationReceiptErrorV1 {
    /// Fixed-layout bytes were malformed.
    Codec(CodecError),
    /// A required identity or digest was zero.
    Coordinate,
}

impl From<CodecError> for DealerScenarioEvaluationReceiptErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl DealerScenarioEvaluationReceiptV1 {
    /// Hostile-decode one exact receipt body.
    pub fn decode(bytes: &[u8]) -> Result<Self, DealerScenarioEvaluationReceiptErrorV1> {
        if bytes.len() != DEALER_SCENARIO_EVALUATION_RECEIPT_BYTES_V1 {
            return Err(DealerScenarioEvaluationReceiptErrorV1::Codec(
                CodecError::InvalidLength,
            ));
        }
        if bytes.get(..8) != Some(DEALER_SCENARIO_EVALUATION_RECEIPT_MAGIC_V1.as_slice()) {
            return Err(DealerScenarioEvaluationReceiptErrorV1::Codec(
                CodecError::InvalidMagic,
            ));
        }
        let version = bytes
            .get(VERSION_OFFSET..VERSION_OFFSET + 2)
            .ok_or(CodecError::InvalidLength)?;
        if version != DEALER_SCENARIO_EVALUATION_RECEIPT_VERSION_V1.to_le_bytes() {
            return Err(DealerScenarioEvaluationReceiptErrorV1::Codec(
                CodecError::UnsupportedVersion,
            ));
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let value = Self {
            custody_effect_count: byte_at(bytes, EFFECT_COUNT_OFFSET)?,
            producer_program: array_at(bytes, PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, CHECKPOINT_OFFSET)?,
            checkpoint_prestate_digest: array_at(bytes, CHECKPOINT_PRESTATE_OFFSET)?,
            request_digest: array_at(bytes, REQUEST_OFFSET)?,
            claims_prestate_digest: array_at(bytes, CLAIMS_PRESTATE_OFFSET)?,
            custody_prestate_digest: array_at(bytes, CUSTODY_PRESTATE_OFFSET)?,
            candidate_bank_digest: array_at(bytes, CANDIDATE_BANK_OFFSET)?,
            candidate_obligation_digest: array_at(bytes, CANDIDATE_OBLIGATION_OFFSET)?,
            claims_delta_digest: array_at(bytes, CLAIMS_DELTA_OFFSET)?,
            effects_digest: array_at(bytes, EFFECTS_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one canonical receipt body.
    pub fn encode(
        self,
    ) -> Result<
        [u8; DEALER_SCENARIO_EVALUATION_RECEIPT_BYTES_V1],
        DealerScenarioEvaluationReceiptErrorV1,
    > {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_EVALUATION_RECEIPT_BYTES_V1];
        put(&mut bytes, 0, &DEALER_SCENARIO_EVALUATION_RECEIPT_MAGIC_V1)?;
        put(
            &mut bytes,
            VERSION_OFFSET,
            &DEALER_SCENARIO_EVALUATION_RECEIPT_VERSION_V1.to_le_bytes(),
        )?;
        put_byte(&mut bytes, EFFECT_COUNT_OFFSET, self.custody_effect_count)?;
        for (offset, value) in [
            (PRODUCER_OFFSET, self.producer_program),
            (CHECKPOINT_OFFSET, self.checkpoint),
            (CHECKPOINT_PRESTATE_OFFSET, self.checkpoint_prestate_digest),
            (REQUEST_OFFSET, self.request_digest),
            (CLAIMS_PRESTATE_OFFSET, self.claims_prestate_digest),
            (CUSTODY_PRESTATE_OFFSET, self.custody_prestate_digest),
            (CANDIDATE_BANK_OFFSET, self.candidate_bank_digest),
            (
                CANDIDATE_OBLIGATION_OFFSET,
                self.candidate_obligation_digest,
            ),
            (CLAIMS_DELTA_OFFSET, self.claims_delta_digest),
            (EFFECTS_OFFSET, self.effects_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        Ok(bytes)
    }

    fn validate(self) -> Result<(), DealerScenarioEvaluationReceiptErrorV1> {
        if usize::from(self.custody_effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || [
                self.producer_program,
                self.checkpoint,
                self.checkpoint_prestate_digest,
                self.request_digest,
                self.claims_prestate_digest,
                self.custody_prestate_digest,
                self.candidate_bank_digest,
                self.candidate_obligation_digest,
                self.claims_delta_digest,
                self.effects_digest,
            ]
            .contains(&[0; 32])
        {
            Err(DealerScenarioEvaluationReceiptErrorV1::Coordinate)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> DealerScenarioEvaluationReceiptV1 {
        DealerScenarioEvaluationReceiptV1 {
            custody_effect_count: 3,
            producer_program: [1; 32],
            checkpoint: [2; 32],
            checkpoint_prestate_digest: [3; 32],
            request_digest: [4; 32],
            claims_prestate_digest: [5; 32],
            custody_prestate_digest: [6; 32],
            candidate_bank_digest: [7; 32],
            candidate_obligation_digest: [8; 32],
            claims_delta_digest: [9; 32],
            effects_digest: [10; 32],
        }
    }

    #[test]
    fn receipt_round_trips_and_every_commitment_is_live() {
        let value = receipt();
        let bytes = value.encode().expect("encode");
        assert_eq!(DealerScenarioEvaluationReceiptV1::decode(&bytes), Ok(value));
        for offset in [
            PRODUCER_OFFSET,
            CHECKPOINT_OFFSET,
            CHECKPOINT_PRESTATE_OFFSET,
            REQUEST_OFFSET,
            CLAIMS_PRESTATE_OFFSET,
            CUSTODY_PRESTATE_OFFSET,
            CANDIDATE_BANK_OFFSET,
            CANDIDATE_OBLIGATION_OFFSET,
            CLAIMS_DELTA_OFFSET,
            EFFECTS_OFFSET,
        ] {
            let mut hostile = bytes;
            hostile[offset..offset + 32].fill(0);
            assert_eq!(
                DealerScenarioEvaluationReceiptV1::decode(&hostile),
                Err(DealerScenarioEvaluationReceiptErrorV1::Coordinate)
            );
        }
    }
}
