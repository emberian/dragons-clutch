//! Terminal receipt for durable controller-funding expiry cleanup.
//!
//! The first cleanup transaction persists its exact close evidence in the
//! controller-funding checkpoint. The second transaction reauthenticates that
//! phase-4 or phase-5 checkpoint, closes the remaining controller ledger, then
//! closes the checkpoint. This fixed receipt binds both transactions' evidence
//! and refund arithmetic so an untrusted caller cannot splice a child receipt,
//! ledger state, checkpoint, controller order, or refund destination from a
//! different cleanup.

use crate::{
    ControllerFundingCheckpointPhaseV1, ControllerFundingCleanupOriginV1,
    ControllerFundingControllerV1, Error,
};

/// Exact terminal cleanup receipt width.
pub const CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_BYTES_V1: usize = 512;
// The width the encoded receipt is asserted to occupy, checked where it cannot
// become a no-op. It used to be an `assert!` inside `fixed_layout_round_trips_exactly`
// over two literals, which is a compile-time fact stated at runtime: it could never
// have failed a test run and clippy's `assertions_on_constants` said so.
const _: () = assert!(CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_BYTES_V1 <= 512);
/// Canonical terminal cleanup receipt magic.
pub const CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTCFR2";
/// Implemented terminal cleanup receipt schema version.
pub const CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_SCHEMA_V1: u16 = 1;

const SCHEMA_OFFSET: usize = 8;
const ORIGIN_OFFSET: usize = 10;
const FIRST_CONTROLLER_OFFSET: usize = 11;
const REMAINING_CONTROLLER_OFFSET: usize = 12;
const CHECKPOINT_PHASE_OFFSET: usize = 13;
const HEADER_RESERVED_OFFSET: usize = 14;
const HEADER_RESERVED_BYTES: usize = 2;
const PRODUCER_OFFSET: usize = 16;
const CHECKPOINT_KEY_OFFSET: usize = 48;
const CHECKPOINT_DIGEST_OFFSET: usize = 80;
const PRIOR_CHECKPOINT_DIGEST_OFFSET: usize = 112;
const FIRST_PRESTATE_DIGEST_OFFSET: usize = 144;
const FIRST_CLOSED_DIGEST_OFFSET: usize = 176;
const FIRST_CHILD_RECEIPT_DIGEST_OFFSET: usize = 208;
const REMAINING_PRESTATE_DIGEST_OFFSET: usize = 240;
const REMAINING_CLOSED_DIGEST_OFFSET: usize = 272;
const REMAINING_CHILD_RECEIPT_DIGEST_OFFSET: usize = 304;
const FUNDING_SOURCE_OFFSET: usize = 336;
const RENT_CREDIT_OFFSET: usize = 368;
const FIRST_PRINCIPAL_REFUND_OFFSET: usize = 400;
const FIRST_RENT_REFUND_OFFSET: usize = 408;
const REMAINING_PRINCIPAL_REFUND_OFFSET: usize = 416;
const REMAINING_RENT_REFUND_OFFSET: usize = 424;
const TOTAL_PRINCIPAL_REFUND_OFFSET: usize = 432;
const TOTAL_RENT_REFUND_OFFSET: usize = 440;
const FIRST_TRANSITION_SLOT_OFFSET: usize = 448;
const FINALIZED_SLOT_OFFSET: usize = 456;
const CHECKPOINT_REVISION_OFFSET: usize = 464;
const FIRST_MASK_OFFSET: usize = 472;
const REMAINING_MASK_OFFSET: usize = 474;
const BODY_RESERVED_OFFSET: usize = 476;
const BODY_RESERVED_BYTES: usize = 36;

const PREPARED_FIRST_LEDGER_CLOSED_REVISION_V1: u64 = 4;
const CUSTODY_FIRST_LEDGER_CLOSED_REVISION_V1: u64 = 5;

/// Exact facts bound by one terminal controller-funding cleanup receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFundingCleanupTerminalReceiptInputV1 {
    /// Origin of the durable cleanup sequence.
    pub origin: ControllerFundingCleanupOriginV1,
    /// Controller whose ledger closed in the first cleanup transaction.
    pub first_controller: ControllerFundingControllerV1,
    /// Controller whose ledger closed in the terminal cleanup transaction.
    pub remaining_controller: ControllerFundingControllerV1,
    /// Durable checkpoint phase authenticated by the terminal transaction.
    pub checkpoint_phase: ControllerFundingCheckpointPhaseV1,
    /// Trading program that produced these terminal receipt bytes.
    pub producer: [u8; 32],
    /// Controller-funding checkpoint account closed by the terminal transaction.
    pub checkpoint_key: [u8; 32],
    /// Digest of the exact phase-4 or phase-5 checkpoint account data.
    pub checkpoint_digest: [u8; 32],
    /// Digest of the exact phase-1 or phase-3 checkpoint consumed by step one.
    pub prior_checkpoint_digest: [u8; 32],
    /// Exact first-ledger account-state digest before its close.
    pub first_prestate_digest: [u8; 32],
    /// Exact first-ledger account-state digest after its close.
    pub first_closed_digest: [u8; 32],
    /// Digest of the first controller's exact child close receipt.
    pub first_child_receipt_digest: [u8; 32],
    /// Exact remaining-ledger account-state digest before its close.
    pub remaining_prestate_digest: [u8; 32],
    /// Exact remaining-ledger account-state digest after its close.
    pub remaining_closed_digest: [u8; 32],
    /// Digest of the remaining controller's exact child close receipt.
    pub remaining_child_receipt_digest: [u8; 32],
    /// Immutable destination of both principal refunds.
    pub funding_source: [u8; 32],
    /// Immutable destination of both Rent refunds.
    pub rent_credit: [u8; 32],
    /// Principal refunded by the first cleanup transaction.
    pub first_principal_refund_lamports: u64,
    /// Rent refunded by the first cleanup transaction.
    pub first_rent_refund_lamports: u64,
    /// Principal refunded by the terminal cleanup transaction.
    pub remaining_principal_refund_lamports: u64,
    /// Rent refunded by the terminal cleanup transaction.
    pub remaining_rent_refund_lamports: u64,
    /// Exact sum of both principal refunds.
    pub total_principal_refund_lamports: u64,
    /// Exact sum of both Rent refunds.
    pub total_rent_refund_lamports: u64,
    /// Finalized slot of the persisted first-ledger close.
    pub first_transition_slot: u64,
    /// Finalized slot of the terminal cleanup transaction.
    pub finalized_slot: u64,
    /// Revision of the exact phase-4 or phase-5 checkpoint.
    pub checkpoint_revision: u64,
    /// Exact manifest mask owned by the first controller.
    pub first_mask: u16,
    /// Exact manifest mask owned by the remaining controller.
    pub remaining_mask: u16,
}

/// Validated terminal controller-funding cleanup receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFundingCleanupTerminalReceiptV1 {
    input: ControllerFundingCleanupTerminalReceiptInputV1,
}

impl ControllerFundingCleanupTerminalReceiptV1 {
    /// Construct and validate one terminal cleanup receipt.
    pub fn new(input: ControllerFundingCleanupTerminalReceiptInputV1) -> Result<Self, Error> {
        validate(input)?;
        Ok(Self { input })
    }

    /// Decode and validate one exact fixed-width terminal cleanup receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_SCHEMA_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        require_zero(bytes, BODY_RESERVED_OFFSET, BODY_RESERVED_BYTES)?;
        Self::new(ControllerFundingCleanupTerminalReceiptInputV1 {
            origin: ControllerFundingCleanupOriginV1::try_from(read_u8(bytes, ORIGIN_OFFSET)?)?,
            first_controller: ControllerFundingControllerV1::try_from(read_u8(
                bytes,
                FIRST_CONTROLLER_OFFSET,
            )?)?,
            remaining_controller: ControllerFundingControllerV1::try_from(read_u8(
                bytes,
                REMAINING_CONTROLLER_OFFSET,
            )?)?,
            checkpoint_phase: ControllerFundingCheckpointPhaseV1::try_from(read_u8(
                bytes,
                CHECKPOINT_PHASE_OFFSET,
            )?)?,
            producer: read_array(bytes, PRODUCER_OFFSET)?,
            checkpoint_key: read_array(bytes, CHECKPOINT_KEY_OFFSET)?,
            checkpoint_digest: read_array(bytes, CHECKPOINT_DIGEST_OFFSET)?,
            prior_checkpoint_digest: read_array(bytes, PRIOR_CHECKPOINT_DIGEST_OFFSET)?,
            first_prestate_digest: read_array(bytes, FIRST_PRESTATE_DIGEST_OFFSET)?,
            first_closed_digest: read_array(bytes, FIRST_CLOSED_DIGEST_OFFSET)?,
            first_child_receipt_digest: read_array(bytes, FIRST_CHILD_RECEIPT_DIGEST_OFFSET)?,
            remaining_prestate_digest: read_array(bytes, REMAINING_PRESTATE_DIGEST_OFFSET)?,
            remaining_closed_digest: read_array(bytes, REMAINING_CLOSED_DIGEST_OFFSET)?,
            remaining_child_receipt_digest: read_array(
                bytes,
                REMAINING_CHILD_RECEIPT_DIGEST_OFFSET,
            )?,
            funding_source: read_array(bytes, FUNDING_SOURCE_OFFSET)?,
            rent_credit: read_array(bytes, RENT_CREDIT_OFFSET)?,
            first_principal_refund_lamports: read_u64(bytes, FIRST_PRINCIPAL_REFUND_OFFSET)?,
            first_rent_refund_lamports: read_u64(bytes, FIRST_RENT_REFUND_OFFSET)?,
            remaining_principal_refund_lamports: read_u64(
                bytes,
                REMAINING_PRINCIPAL_REFUND_OFFSET,
            )?,
            remaining_rent_refund_lamports: read_u64(bytes, REMAINING_RENT_REFUND_OFFSET)?,
            total_principal_refund_lamports: read_u64(bytes, TOTAL_PRINCIPAL_REFUND_OFFSET)?,
            total_rent_refund_lamports: read_u64(bytes, TOTAL_RENT_REFUND_OFFSET)?,
            first_transition_slot: read_u64(bytes, FIRST_TRANSITION_SLOT_OFFSET)?,
            finalized_slot: read_u64(bytes, FINALIZED_SLOT_OFFSET)?,
            checkpoint_revision: read_u64(bytes, CHECKPOINT_REVISION_OFFSET)?,
            first_mask: read_u16(bytes, FIRST_MASK_OFFSET)?,
            remaining_mask: read_u16(bytes, REMAINING_MASK_OFFSET)?,
        })
    }

    /// Encode this receipt into its exact canonical fixed layout.
    pub fn encode(self) -> [u8; CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_BYTES_V1] {
        let input = self.input;
        let mut output = [0_u8; CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_BYTES_V1];
        put_array(
            &mut output,
            0,
            CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_MAGIC_V1,
        );
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            CONTROLLER_FUNDING_CLEANUP_TERMINAL_RECEIPT_SCHEMA_V1,
        );
        output[ORIGIN_OFFSET] = input.origin as u8;
        output[FIRST_CONTROLLER_OFFSET] = input.first_controller as u8;
        output[REMAINING_CONTROLLER_OFFSET] = input.remaining_controller as u8;
        output[CHECKPOINT_PHASE_OFFSET] = input.checkpoint_phase as u8;
        for (offset, value) in [
            (PRODUCER_OFFSET, input.producer),
            (CHECKPOINT_KEY_OFFSET, input.checkpoint_key),
            (CHECKPOINT_DIGEST_OFFSET, input.checkpoint_digest),
            (
                PRIOR_CHECKPOINT_DIGEST_OFFSET,
                input.prior_checkpoint_digest,
            ),
            (FIRST_PRESTATE_DIGEST_OFFSET, input.first_prestate_digest),
            (FIRST_CLOSED_DIGEST_OFFSET, input.first_closed_digest),
            (
                FIRST_CHILD_RECEIPT_DIGEST_OFFSET,
                input.first_child_receipt_digest,
            ),
            (
                REMAINING_PRESTATE_DIGEST_OFFSET,
                input.remaining_prestate_digest,
            ),
            (
                REMAINING_CLOSED_DIGEST_OFFSET,
                input.remaining_closed_digest,
            ),
            (
                REMAINING_CHILD_RECEIPT_DIGEST_OFFSET,
                input.remaining_child_receipt_digest,
            ),
            (FUNDING_SOURCE_OFFSET, input.funding_source),
            (RENT_CREDIT_OFFSET, input.rent_credit),
        ] {
            put_array(&mut output, offset, value);
        }
        for (offset, value) in [
            (
                FIRST_PRINCIPAL_REFUND_OFFSET,
                input.first_principal_refund_lamports,
            ),
            (FIRST_RENT_REFUND_OFFSET, input.first_rent_refund_lamports),
            (
                REMAINING_PRINCIPAL_REFUND_OFFSET,
                input.remaining_principal_refund_lamports,
            ),
            (
                REMAINING_RENT_REFUND_OFFSET,
                input.remaining_rent_refund_lamports,
            ),
            (
                TOTAL_PRINCIPAL_REFUND_OFFSET,
                input.total_principal_refund_lamports,
            ),
            (TOTAL_RENT_REFUND_OFFSET, input.total_rent_refund_lamports),
            (FIRST_TRANSITION_SLOT_OFFSET, input.first_transition_slot),
            (FINALIZED_SLOT_OFFSET, input.finalized_slot),
            (CHECKPOINT_REVISION_OFFSET, input.checkpoint_revision),
        ] {
            put_u64(&mut output, offset, value);
        }
        put_u16(&mut output, FIRST_MASK_OFFSET, input.first_mask);
        put_u16(&mut output, REMAINING_MASK_OFFSET, input.remaining_mask);
        output
    }

    /// Return every authenticated fact carried by this receipt.
    pub const fn input(self) -> ControllerFundingCleanupTerminalReceiptInputV1 {
        self.input
    }

    /// Require every fact to equal the independently derived expected receipt.
    pub fn authenticate_exact(
        self,
        expected: ControllerFundingCleanupTerminalReceiptInputV1,
    ) -> Result<(), Error> {
        validate(expected)?;
        if self.input != expected {
            return Err(Error::InvalidControllerFundingCheckpointTransition);
        }
        Ok(())
    }
}

fn validate(input: ControllerFundingCleanupTerminalReceiptInputV1) -> Result<(), Error> {
    for value in [
        input.producer,
        input.checkpoint_key,
        input.checkpoint_digest,
        input.prior_checkpoint_digest,
        input.first_prestate_digest,
        input.first_closed_digest,
        input.first_child_receipt_digest,
        input.remaining_prestate_digest,
        input.remaining_closed_digest,
        input.remaining_child_receipt_digest,
        input.funding_source,
        input.rent_credit,
    ] {
        if value == [0; 32] {
            return Err(Error::ZeroIdentifier);
        }
    }
    let (expected_phase, expected_revision) = match input.origin {
        ControllerFundingCleanupOriginV1::Prepared => (
            ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed,
            PREPARED_FIRST_LEDGER_CLOSED_REVISION_V1,
        ),
        ControllerFundingCleanupOriginV1::CustodyStaged => (
            ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
            CUSTODY_FIRST_LEDGER_CLOSED_REVISION_V1,
        ),
    };
    let canonical_first =
        if input.first_mask.trailing_zeros() < input.remaining_mask.trailing_zeros() {
            input.first_controller
        } else {
            input.remaining_controller
        };
    let total_principal = input
        .first_principal_refund_lamports
        .checked_add(input.remaining_principal_refund_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let total_rent = input
        .first_rent_refund_lamports
        .checked_add(input.remaining_rent_refund_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    if input.first_controller == input.remaining_controller
        || canonical_first != input.first_controller
        || input.checkpoint_phase != expected_phase
        || input.checkpoint_revision != expected_revision
        || input.funding_source == input.rent_credit
        || input.first_mask == 0
        || input.remaining_mask == 0
        || input.first_mask & input.remaining_mask != 0
        || input.first_rent_refund_lamports == 0
        || input.remaining_rent_refund_lamports == 0
        || input.total_principal_refund_lamports != total_principal
        || input.total_rent_refund_lamports != total_rent
        || input.first_transition_slot == 0
        || input.finalized_slot < input.first_transition_slot
    {
        return Err(Error::InvalidControllerFundingCheckpointTransition);
    }
    Ok(())
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, Error> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    if bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

/// Write one fixed-width field into the encoder's own exactly-sized buffer.
///
/// The slicing panic here is deliberate and is kept as a panic.
///
/// This takes no caller data. `output` is the buffer this module just allocated
/// at the record's exact encoded width, and `offset` is one of this file's own
/// layout constants. An out-of-range write is therefore not a malformed input to
/// refuse — it is this encoder disagreeing with its own layout, which would mean
/// every record it produced was already wrong.
///
/// So there is no refusal to convert to. `get_mut(..)` with the write skipped
/// would emit a short, partly zero record that still hashes to a plausible
/// identity, and a fabricated `Err` variant would add a refusal path no caller
/// can trigger. Panicking stops the transaction, which is the correct response
/// to an encoder that cannot encode.
#[allow(clippy::indexing_slicing)]
fn put_array<const N: usize>(output: &mut [u8], offset: usize, value: [u8; N]) {
    output[offset..offset + N].copy_from_slice(&value);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put_array(output, offset, value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put_array(output, offset, value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn input() -> ControllerFundingCleanupTerminalReceiptInputV1 {
        ControllerFundingCleanupTerminalReceiptInputV1 {
            origin: ControllerFundingCleanupOriginV1::Prepared,
            first_controller: ControllerFundingControllerV1::Trading,
            remaining_controller: ControllerFundingControllerV1::Resolution,
            checkpoint_phase: ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed,
            producer: [1; 32],
            checkpoint_key: [2; 32],
            checkpoint_digest: [3; 32],
            prior_checkpoint_digest: [4; 32],
            first_prestate_digest: [5; 32],
            first_closed_digest: [6; 32],
            first_child_receipt_digest: [7; 32],
            remaining_prestate_digest: [9; 32],
            remaining_closed_digest: [10; 32],
            remaining_child_receipt_digest: [11; 32],
            funding_source: [12; 32],
            rent_credit: [13; 32],
            first_principal_refund_lamports: 14,
            first_rent_refund_lamports: 15,
            remaining_principal_refund_lamports: 16,
            remaining_rent_refund_lamports: 17,
            total_principal_refund_lamports: 30,
            total_rent_refund_lamports: 32,
            first_transition_slot: 17,
            finalized_slot: 18,
            checkpoint_revision: PREPARED_FIRST_LEDGER_CLOSED_REVISION_V1,
            first_mask: 0b0001,
            remaining_mask: 0b1110,
        }
    }

    fn assert_substitution_rejected(hostile: ControllerFundingCleanupTerminalReceiptInputV1) {
        let honest = ControllerFundingCleanupTerminalReceiptV1::new(input()).expect("honest");
        assert!(honest.authenticate_exact(hostile).is_err());
    }

    #[test]
    fn fixed_layout_round_trips_exactly() {
        let receipt = ControllerFundingCleanupTerminalReceiptV1::new(input()).expect("receipt");
        let encoded = receipt.encode();
        assert_eq!(encoded.len(), 512);
        assert_eq!(
            ControllerFundingCleanupTerminalReceiptV1::decode(&encoded),
            Ok(receipt)
        );
        assert_eq!(receipt.authenticate_exact(input()), Ok(()));

        let mut custody = input();
        custody.origin = ControllerFundingCleanupOriginV1::CustodyStaged;
        custody.checkpoint_phase = ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed;
        custody.checkpoint_revision = CUSTODY_FIRST_LEDGER_CLOSED_REVISION_V1;
        let custody = ControllerFundingCleanupTerminalReceiptV1::new(custody).expect("custody");
        assert_eq!(
            ControllerFundingCleanupTerminalReceiptV1::decode(&custody.encode()),
            Ok(custody)
        );
    }

    #[test]
    fn controller_phase_mask_total_and_shape_substitutions_refuse() {
        let honest = input();
        for hostile in [
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_controller: honest.first_controller,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                checkpoint_phase: ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                checkpoint_revision: CUSTODY_FIRST_LEDGER_CLOSED_REVISION_V1,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_mask: 0b0100,
                remaining_mask: 0b0010,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                total_principal_refund_lamports: 31,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                total_rent_refund_lamports: 33,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_rent_refund_lamports: 0,
                total_rent_refund_lamports: 17,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                rent_credit: honest.funding_source,
                ..honest
            },
        ] {
            assert!(ControllerFundingCleanupTerminalReceiptV1::new(hostile).is_err());
        }
    }

    #[test]
    fn every_bound_identity_digest_refund_and_slot_substitution_refuses() {
        let honest = input();
        for hostile in [
            ControllerFundingCleanupTerminalReceiptInputV1 {
                origin: ControllerFundingCleanupOriginV1::CustodyStaged,
                checkpoint_phase: ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
                checkpoint_revision: CUSTODY_FIRST_LEDGER_CLOSED_REVISION_V1,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_controller: ControllerFundingControllerV1::Resolution,
                remaining_controller: ControllerFundingControllerV1::Trading,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                producer: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                checkpoint_key: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                checkpoint_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                prior_checkpoint_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_prestate_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_closed_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_child_receipt_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_prestate_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_closed_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_child_receipt_digest: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                funding_source: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                rent_credit: [21; 32],
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_principal_refund_lamports: 15,
                total_principal_refund_lamports: 31,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_rent_refund_lamports: 16,
                total_rent_refund_lamports: 33,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_principal_refund_lamports: 17,
                total_principal_refund_lamports: 31,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_rent_refund_lamports: 18,
                total_rent_refund_lamports: 33,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_transition_slot: 16,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                finalized_slot: 19,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                first_mask: 0b0011,
                ..honest
            },
            ControllerFundingCleanupTerminalReceiptInputV1 {
                remaining_mask: 0b1100,
                ..honest
            },
        ] {
            assert_substitution_rejected(hostile);
        }
    }

    #[test]
    fn wire_tag_reserved_zero_and_arithmetic_substitutions_refuse() {
        let receipt = ControllerFundingCleanupTerminalReceiptV1::new(input()).expect("receipt");
        let mut bytes = receipt.encode();
        for offset in [
            0,
            SCHEMA_OFFSET,
            HEADER_RESERVED_OFFSET,
            BODY_RESERVED_OFFSET,
        ] {
            let old = bytes[offset];
            bytes[offset] ^= 1;
            assert!(ControllerFundingCleanupTerminalReceiptV1::decode(&bytes).is_err());
            bytes[offset] = old;
        }
        bytes[FIRST_CONTROLLER_OFFSET] = 9;
        assert!(ControllerFundingCleanupTerminalReceiptV1::decode(&bytes).is_err());

        let honest = input();
        assert_eq!(
            ControllerFundingCleanupTerminalReceiptV1::new(
                ControllerFundingCleanupTerminalReceiptInputV1 {
                    first_principal_refund_lamports: u64::MAX,
                    total_principal_refund_lamports: 0,
                    ..honest
                }
            ),
            Err(Error::ArithmeticOverflow)
        );
    }
}
