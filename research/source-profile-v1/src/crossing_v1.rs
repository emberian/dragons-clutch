//! PROPOSED `CROSSING_V1` selection-rule admission model (MODEL-ONLY).
//!
//! Executable model of `docs/design/SOURCE_PROVIDER_V1_SELECTION.md` §4.  It
//! is not a runtime transition and freezes nothing; the default ELF keeps
//! refusing `SourceReleaseUnavailable` (`0x79`).
//!
//! Let the frozen grid origin be `G = 0`, `bucket_seconds = B`, and bucket `k`
//! cover `[G+kB, G+(k+1)B)`. V1 registers only the closing boundary:
//! `T(k) = G+(k+1)B`, the source state in force when the bucket closes. The
//! opening-boundary experiment is not a second live rule.
//!
//! Admission for bucket `k` takes the unique update `U` with
//! `prev_publish_time(U) < T(k) <= publish_time(U)`.  Exactly per the design
//! doc:
//!
//! - a degenerate update (`prev == publish`, failed aggregation) satisfies
//!   the predicate for no `T` and witnesses no boundary;
//! - an absent crossing witness is an explicit [`CrossingError::Stall`] —
//!   nothing manufactures a `Missing` record or substitutes an adjacent
//!   update;
//! - two *distinct* qualifying update bodies for one boundary is the falsifier for
//!   the whole provider selection ([`CrossingError::DoubleWitnessBoundary`]);
//!   the model refuses rather than picks;
//! - one update may witness consecutive boundaries, so the archive sequence
//!   (`sequence := publish_time(U)`) is monotone **non-strict**, with
//!   equality admissible only when the 64-byte record bodies are
//!   byte-identical except the bucket field
//!   ([`CrossingError::EqualSequenceValueDrift`] otherwise).
//!
//! Semantic owners are disambiguated: archive `source publish time` is
//! `publish_time(U)`; archive `source publish slot` is the update account's
//! `posted_slot` (receiver-write slot, explicitly not source-native); archive
//! sequence is `publish_time(U)`.  No field doubles as another.

use crate::spec_v2::{GRID_ORIGIN_UNIX_SECONDS_V1, MODEL_MAX_BUCKET_SECONDS};
use crate::{normalize_interval, selects_boundary, Error, FullPriceUpdateV2};

/// Closing-boundary `CROSSING_V1`: `T(k) = G+(k+1)*B`, with `G = 0`.
pub const SELECTION_CROSSING_V1: u16 = 2;

/// Exact byte length of one archive record (`SOURCE_ADMISSION_V1` §5.3).
pub const ARCHIVE_RECORD_V2_BYTES: usize = 64;

/// Byte length of the leading bucket field excluded from body identity.
const BUCKET_FIELD_BYTES: usize = 8;

/// Refusals from boundary construction, witness selection, and admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossingError {
    /// The rule id is not a registered `CROSSING_V1` boundary variant.
    UnknownSelectionRule,
    /// `bucket_seconds` is zero or outside the model grid envelope.
    InvalidBucketSeconds,
    /// The grid origin is not the frozen Unix-epoch origin.
    UnknownGridOrigin,
    /// `T(k)` is not representable; no update could ever witness it, so the
    /// configuration is refused rather than stalled.
    BoundaryOverflow,
    /// No presented update witnesses the boundary.  The feed stalls; this is
    /// never adapted into a fabricated `Missing` record.
    Stall,
    /// Two distinct updates both witness one boundary.  This falsifies the
    /// provider's documented uniqueness and reopens the R2 selection; the
    /// model refuses rather than picks.
    DoubleWitnessBoundary,
    /// The paired update does not witness the named boundary.
    NotBoundaryWitness,
    /// The witness publish time cannot own the archive sequence field.
    InvalidPublishTime,
    /// Records must cover consecutive buckets; gaps and repeats are refused.
    NonContiguousBucket,
    /// The next exclusive archive cursor is not representable.
    BucketCursorOverflow,
    /// The archive sequence (`publish_time`) moved backwards.
    SequenceRegression,
    /// Equal sequence with record bodies that are not byte-identical except
    /// the bucket field.  This falsifies the non-strict equality clause and
    /// the rule must be replaced (e.g., by a witness-identity hash).
    EqualSequenceValueDrift,
    /// The witness value failed conservative interval normalization.
    Normalization(Error),
}

/// One modeled archive record with the §5.3 fixed 64-byte layout.
///
/// | Offset | Bytes | Field |
/// | ---: | ---: | --- |
/// | 0 | 8 | bucket |
/// | 8 | 16 | conservative low endpoint |
/// | 24 | 16 | conservative high endpoint |
/// | 40 | 8 | source-native sequence (`:= publish_time`) |
/// | 48 | 8 | source publish slot (`:= posted_slot`) |
/// | 56 | 8 | source publish time |
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveRecordV2 {
    pub bucket: u64,
    pub low: u128,
    pub high: u128,
    pub sequence: u64,
    pub publish_slot: u64,
    pub publish_time: u64,
}

/// Immutable normalization projection from one authenticated SourceSpec v2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPolicyV1 {
    pub target_decimals: u8,
    pub confidence_multiplier: u16,
}

impl ArchiveRecordV2 {
    /// Encode the exact fixed record layout.
    pub fn encode(self) -> [u8; ARCHIVE_RECORD_V2_BYTES] {
        let mut out = [0_u8; ARCHIVE_RECORD_V2_BYTES];
        out[0..8].copy_from_slice(&self.bucket.to_le_bytes());
        out[8..24].copy_from_slice(&self.low.to_le_bytes());
        out[24..40].copy_from_slice(&self.high.to_le_bytes());
        out[40..48].copy_from_slice(&self.sequence.to_le_bytes());
        out[48..56].copy_from_slice(&self.publish_slot.to_le_bytes());
        out[56..64].copy_from_slice(&self.publish_time.to_le_bytes());
        out
    }
}

/// True when two records are byte-identical except the bucket field.
pub fn body_identical(a: ArchiveRecordV2, b: ArchiveRecordV2) -> bool {
    a.encode()[BUCKET_FIELD_BYTES..] == b.encode()[BUCKET_FIELD_BYTES..]
}

/// Start-aware cursor for one contiguous archive window.
///
/// The first append is checked against the immutable window start, not merely
/// accepted because there is no predecessor. Every successful append advances
/// one exact bucket, and an unrepresentable exclusive cursor is refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveCursorV2 {
    start_bucket: u64,
    next_bucket: u64,
    previous: Option<ArchiveRecordV2>,
}

impl ArchiveCursorV2 {
    pub const fn new(start_bucket: u64) -> Self {
        Self {
            start_bucket,
            next_bucket: start_bucket,
            previous: None,
        }
    }

    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    pub const fn next_bucket(self) -> u64 {
        self.next_bucket
    }

    pub const fn previous(self) -> Option<ArchiveRecordV2> {
        self.previous
    }

    pub fn admit(self, record: ArchiveRecordV2) -> Result<Self, CrossingError> {
        if record.bucket != self.next_bucket {
            return Err(CrossingError::NonContiguousBucket);
        }
        if let Some(previous) = self.previous {
            admit_after(previous, record)?;
        }
        let next_bucket = self
            .next_bucket
            .checked_add(1)
            .ok_or(CrossingError::BucketCursorOverflow)?;
        Ok(Self {
            start_bucket: self.start_bucket,
            next_bucket,
            previous: Some(record),
        })
    }
}

/// Compute the boundary instant `T(k)` for one registered variant.
///
/// The result always fits `i64`, so it can be compared against publish
/// times; an unrepresentable boundary is a refused configuration.
pub fn boundary_instant(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    bucket: u64,
) -> Result<u64, CrossingError> {
    if rule != SELECTION_CROSSING_V1 {
        return Err(CrossingError::UnknownSelectionRule);
    }
    if grid_origin_unix_seconds != GRID_ORIGIN_UNIX_SECONDS_V1 {
        return Err(CrossingError::UnknownGridOrigin);
    }
    if bucket_seconds == 0 || bucket_seconds > MODEL_MAX_BUCKET_SECONDS {
        return Err(CrossingError::InvalidBucketSeconds);
    }
    let steps = bucket
        .checked_add(1)
        .ok_or(CrossingError::BoundaryOverflow)?;
    let boundary = steps
        .checked_mul(bucket_seconds)
        .ok_or(CrossingError::BoundaryOverflow)?;
    if i64::try_from(boundary).is_err() {
        return Err(CrossingError::BoundaryOverflow);
    }
    Ok(boundary)
}

/// Whether one update witnesses bucket `k` under one registered variant.
pub fn witnesses_boundary(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    bucket: u64,
    update: FullPriceUpdateV2,
) -> Result<bool, CrossingError> {
    let boundary = boundary_instant(rule, grid_origin_unix_seconds, bucket_seconds, bucket)?;
    Ok(selects_boundary(update, boundary))
}

/// Select the unique crossing witness for bucket `k` among candidates.
///
/// No qualifying candidate is an explicit [`CrossingError::Stall`].  Two
/// candidates that qualify and differ in any field are the uniqueness
/// falsifier and are refused; byte-identical duplicates of one witness carry
/// no selection surface and collapse to that witness.
pub fn select_witness(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    bucket: u64,
    candidates: &[FullPriceUpdateV2],
) -> Result<FullPriceUpdateV2, CrossingError> {
    let boundary = boundary_instant(rule, grid_origin_unix_seconds, bucket_seconds, bucket)?;
    let mut witness: Option<FullPriceUpdateV2> = None;
    for candidate in candidates {
        if !selects_boundary(*candidate, boundary) {
            continue;
        }
        match witness {
            None => witness = Some(*candidate),
            Some(existing) if existing == *candidate => {}
            Some(_) => return Err(CrossingError::DoubleWitnessBoundary),
        }
    }
    witness.ok_or(CrossingError::Stall)
}

/// Build the archive record owned by one admitted witness.
///
/// The crossing predicate is rechecked here; a caller-paired update that does
/// not witness the boundary is refused, never adapted.
pub fn record_from_witness(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    bucket: u64,
    witness: FullPriceUpdateV2,
    target_decimals: u8,
    confidence_multiplier: u16,
) -> Result<ArchiveRecordV2, CrossingError> {
    let boundary = boundary_instant(rule, grid_origin_unix_seconds, bucket_seconds, bucket)?;
    if !selects_boundary(witness, boundary) {
        return Err(CrossingError::NotBoundaryWitness);
    }
    let interval = normalize_interval(witness, target_decimals, confidence_multiplier)
        .map_err(CrossingError::Normalization)?;
    let publish_time =
        u64::try_from(witness.publish_time).map_err(|_| CrossingError::InvalidPublishTime)?;
    Ok(ArchiveRecordV2 {
        bucket,
        low: interval.low,
        high: interval.high,
        sequence: publish_time,
        publish_slot: witness.posted_slot,
        publish_time,
    })
}

/// Admit `next` directly after `prev` under the non-strict sequence rule.
///
/// Buckets must be consecutive.  Sequence may repeat only when the same
/// witness legitimately covers both boundaries, i.e. the record bodies are
/// byte-identical except the bucket field.
pub fn admit_after(prev: ArchiveRecordV2, next: ArchiveRecordV2) -> Result<(), CrossingError> {
    let expected_bucket = prev
        .bucket
        .checked_add(1)
        .ok_or(CrossingError::BucketCursorOverflow)?;
    if next.bucket != expected_bucket {
        return Err(CrossingError::NonContiguousBucket);
    }
    if next.sequence < prev.sequence {
        return Err(CrossingError::SequenceRegression);
    }
    if next.sequence == prev.sequence && !body_identical(prev, next) {
        return Err(CrossingError::EqualSequenceValueDrift);
    }
    Ok(())
}

/// Select, normalize, and admit the record for one bucket in one step.
pub fn advance(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    prev: Option<ArchiveRecordV2>,
    bucket: u64,
    candidates: &[FullPriceUpdateV2],
    policy: RecordPolicyV1,
) -> Result<ArchiveRecordV2, CrossingError> {
    let witness = select_witness(
        rule,
        grid_origin_unix_seconds,
        bucket_seconds,
        bucket,
        candidates,
    )?;
    let record = record_from_witness(
        rule,
        grid_origin_unix_seconds,
        bucket_seconds,
        bucket,
        witness,
        policy.target_decimals,
        policy.confidence_multiplier,
    )?;
    if let Some(prev) = prev {
        admit_after(prev, record)?;
    }
    Ok(record)
}

/// Select and append exactly at an authenticated archive cursor.
pub fn advance_cursor(
    rule: u16,
    grid_origin_unix_seconds: i64,
    bucket_seconds: u64,
    cursor: ArchiveCursorV2,
    candidates: &[FullPriceUpdateV2],
    policy: RecordPolicyV1,
) -> Result<(ArchiveCursorV2, ArchiveRecordV2), CrossingError> {
    let record = advance(
        rule,
        grid_origin_unix_seconds,
        bucket_seconds,
        cursor.previous(),
        cursor.next_bucket(),
        candidates,
        policy,
    )?;
    let next = cursor.admit(record)?;
    Ok((next, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: RecordPolicyV1 = RecordPolicyV1 {
        target_decimals: 8,
        confidence_multiplier: 2,
    };

    fn update(prev: i64, publish: i64) -> FullPriceUpdateV2 {
        FullPriceUpdateV2 {
            write_authority: [0x11; 32],
            feed_id: [0x22; 32],
            price: 123_456_789,
            confidence: 12_345,
            exponent: -8,
            publish_time: publish,
            prev_publish_time: prev,
            ema_price: 123_450_000,
            ema_confidence: 20_000,
            posted_slot: 250_000_000,
        }
    }

    #[test]
    fn closing_boundary_rule_and_zero_origin_are_exact() {
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, GRID_ORIGIN_UNIX_SECONDS_V1, 300, 0),
            Ok(300)
        );
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, GRID_ORIGIN_UNIX_SECONDS_V1, 300, 4),
            Ok(1_500)
        );

        // V1 finalized-bucket id 1, the rejected opening experiment id 3,
        // and unregistered ids are refused.
        for rule in [0_u16, 1, 3, 4, u16::MAX] {
            assert_eq!(
                boundary_instant(rule, GRID_ORIGIN_UNIX_SECONDS_V1, 300, 4),
                Err(CrossingError::UnknownSelectionRule)
            );
        }
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, 1, 300, 4),
            Err(CrossingError::UnknownGridOrigin)
        );

        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, GRID_ORIGIN_UNIX_SECONDS_V1, 0, 4),
            Err(CrossingError::InvalidBucketSeconds)
        );
        assert_eq!(
            boundary_instant(
                SELECTION_CROSSING_V1,
                GRID_ORIGIN_UNIX_SECONDS_V1,
                MODEL_MAX_BUCKET_SECONDS + 1,
                4,
            ),
            Err(CrossingError::InvalidBucketSeconds)
        );

        // Overflow of k+1, of the product, and of the i64 comparison domain.
        assert_eq!(
            boundary_instant(
                SELECTION_CROSSING_V1,
                GRID_ORIGIN_UNIX_SECONDS_V1,
                2,
                u64::MAX,
            ),
            Err(CrossingError::BoundaryOverflow)
        );
        assert_eq!(
            boundary_instant(
                SELECTION_CROSSING_V1,
                GRID_ORIGIN_UNIX_SECONDS_V1,
                2,
                4_999_999_999_999_999_999
            ),
            Err(CrossingError::BoundaryOverflow)
        );
    }

    #[test]
    fn single_crossing_witness_is_selected() {
        // Closing variant, B = 30, bucket 3, T = 120.
        let crossing = update(100, 125);
        let candidates = [update(10, 90), crossing, update(125, 300)];
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, 0, 30, 3, &candidates),
            Ok(crossing)
        );
        assert_eq!(
            witnesses_boundary(SELECTION_CROSSING_V1, 0, 30, 3, crossing),
            Ok(true)
        );
        assert_eq!(
            witnesses_boundary(SELECTION_CROSSING_V1, 0, 30, 3, update(125, 300)),
            Ok(false)
        );
    }

    #[test]
    fn absent_witness_is_an_explicit_stall_never_a_missing_record() {
        // Closing variant, B = 30, bucket 3, T = 120: a fresh-but-late
        // update, a stale update, and a degenerate update all witness
        // nothing; the model stalls instead of adapting any of them.
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, 0, 30, 3, &[]),
            Err(CrossingError::Stall)
        );
        let candidates = [update(120, 130), update(50, 100), update(120, 120)];
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, 0, 30, 3, &candidates),
            Err(CrossingError::Stall)
        );
    }

    /// §4 uniqueness falsifier: two distinct updates both satisfying the
    /// crossing predicate for one `T` must be refused, never picked between.
    #[test]
    fn falsifier_double_witness_boundary() {
        // Closing variant, B = 30, bucket 3, T = 120.
        let first = update(90, 130); // 90 < 120 <= 130
        let second = update(100, 120); // 100 < 120 <= 120
        assert_ne!(first, second);
        assert_eq!(
            witnesses_boundary(SELECTION_CROSSING_V1, 0, 30, 3, first),
            Ok(true)
        );
        assert_eq!(
            witnesses_boundary(SELECTION_CROSSING_V1, 0, 30, 3, second),
            Ok(true)
        );
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, 0, 30, 3, &[first, second]),
            Err(CrossingError::DoubleWitnessBoundary)
        );
        assert_eq!(
            advance(
                SELECTION_CROSSING_V1,
                0,
                30,
                None,
                3,
                &[first, second],
                POLICY,
            ),
            Err(CrossingError::DoubleWitnessBoundary)
        );
    }

    #[test]
    fn duplicate_identical_candidates_are_one_witness() {
        let crossing = update(100, 125);
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, 0, 30, 3, &[crossing, crossing]),
            Ok(crossing)
        );

        // Collapse means exact decoded account-body identity. A repost with a
        // different wrapper authority or posted slot is distinct and refuses;
        // the model never broadens identity to price-message fields alone.
        let mut different_authority = crossing;
        different_authority.write_authority[0] ^= 1;
        assert_eq!(
            select_witness(
                SELECTION_CROSSING_V1,
                0,
                30,
                3,
                &[crossing, different_authority],
            ),
            Err(CrossingError::DoubleWitnessBoundary)
        );
        let mut different_posted_slot = crossing;
        different_posted_slot.posted_slot += 1;
        assert_eq!(
            select_witness(
                SELECTION_CROSSING_V1,
                0,
                30,
                3,
                &[crossing, different_posted_slot],
            ),
            Err(CrossingError::DoubleWitnessBoundary)
        );
    }

    /// §4 degenerate rule: `prev == publish` (failed aggregation) satisfies
    /// the predicate for no `T` and admits nothing.
    #[test]
    fn degenerate_update_witnesses_no_boundary() {
        let degenerate = update(150, 150);
        assert!(!selects_boundary(degenerate, 149));
        assert!(!selects_boundary(degenerate, 150));
        assert!(!selects_boundary(degenerate, 151));
        for bucket in 0..8 {
            assert_eq!(
                select_witness(SELECTION_CROSSING_V1, 0, 30, bucket, &[degenerate]),
                Err(CrossingError::Stall)
            );
        }
    }

    /// §4 witness reuse: with no update between `T(k)` and `T(k+1)`, the same
    /// update witnesses both buckets; equal sequence with byte-identical
    /// bodies (except the bucket field) is admitted.
    #[test]
    fn same_witness_admits_two_consecutive_buckets() {
        // Closing variant, B = 30: U = (100, 200) crosses T(3) = 120 and
        // T(4) = 150.
        let witness = update(100, 200);
        let third = advance(SELECTION_CROSSING_V1, 0, 30, None, 3, &[witness], POLICY)
            .expect("bucket 3 admits the crossing witness");
        let fourth = advance(
            SELECTION_CROSSING_V1,
            0,
            30,
            Some(third),
            4,
            &[witness],
            POLICY,
        )
        .expect("bucket 4 legitimately reuses the same witness");
        assert_eq!(third.bucket, 3);
        assert_eq!(fourth.bucket, 4);
        assert_eq!(third.sequence, 200);
        assert_eq!(fourth.sequence, 200);
        assert!(body_identical(third, fourth));
        assert_ne!(third.encode(), fourth.encode());
    }

    /// §4 sequence-rule falsifier: two records for different buckets with
    /// equal sequence and differing endpoints must be refused — otherwise
    /// the equality clause admits value drift.
    #[test]
    fn falsifier_equal_sequence_with_differing_endpoints_refuses() {
        let witness = update(100, 200);
        let third = advance(SELECTION_CROSSING_V1, 0, 30, None, 3, &[witness], POLICY)
            .expect("bucket 3 admits the crossing witness");

        // A same-publish-time update with different confidence yields equal
        // sequence but different endpoints for bucket 4.
        let mut drifted = witness;
        drifted.confidence = 20_000;
        assert_eq!(
            advance(
                SELECTION_CROSSING_V1,
                0,
                30,
                Some(third),
                4,
                &[drifted],
                POLICY,
            ),
            Err(CrossingError::EqualSequenceValueDrift)
        );

        // The same refusal on bare records.
        let mut hostile = third;
        hostile.bucket = 4;
        hostile.low += 1;
        assert_eq!(
            admit_after(third, hostile),
            Err(CrossingError::EqualSequenceValueDrift)
        );
    }

    #[test]
    fn sequence_regression_and_bucket_gaps_are_refused() {
        let witness = update(100, 200);
        let third = advance(SELECTION_CROSSING_V1, 0, 30, None, 3, &[witness], POLICY)
            .expect("bucket 3 admits the crossing witness");

        let mut regressed = third;
        regressed.bucket = 4;
        regressed.sequence = 199;
        assert_eq!(
            admit_after(third, regressed),
            Err(CrossingError::SequenceRegression)
        );

        let mut gapped = third;
        gapped.bucket = 5;
        assert_eq!(
            admit_after(third, gapped),
            Err(CrossingError::NonContiguousBucket)
        );
        let mut repeated = third;
        repeated.bucket = 3;
        assert_eq!(
            admit_after(third, repeated),
            Err(CrossingError::NonContiguousBucket)
        );

        let mut saturated = third;
        saturated.bucket = u64::MAX;
        assert_eq!(
            admit_after(saturated, third),
            Err(CrossingError::BucketCursorOverflow)
        );
    }

    #[test]
    fn archive_cursor_pins_the_first_bucket_and_checked_exclusive_end() {
        let witness = update(100, 200);
        let cursor = ArchiveCursorV2::new(3);
        let (cursor, third) =
            advance_cursor(SELECTION_CROSSING_V1, 0, 30, cursor, &[witness], POLICY)
                .expect("frozen start bucket admits");
        assert_eq!(cursor.start_bucket(), 3);
        assert_eq!(cursor.next_bucket(), 4);
        assert_eq!(cursor.previous(), Some(third));

        let wrong_first = ArchiveRecordV2 { bucket: 2, ..third };
        assert_eq!(
            ArchiveCursorV2::new(3).admit(wrong_first),
            Err(CrossingError::NonContiguousBucket)
        );
        let saturated = ArchiveRecordV2 {
            bucket: u64::MAX,
            ..third
        };
        assert_eq!(
            ArchiveCursorV2::new(u64::MAX).admit(saturated),
            Err(CrossingError::BucketCursorOverflow)
        );
    }

    #[test]
    fn record_layout_and_semantic_owners_are_exact() {
        // Endpoints match the V1 normalization fixture: price 123_456_789,
        // confidence 12_345, exponent -8, 8 decimals, multiplier 2.
        let witness = update(100, 200);
        let record = record_from_witness(SELECTION_CROSSING_V1, 0, 30, 3, witness, 8, 2)
            .expect("crossing witness admits");
        assert_eq!(record.bucket, 3);
        assert_eq!(record.low, 123_432_099);
        assert_eq!(record.high, 123_481_479);
        // Distinct semantic owners: sequence and publish time are the
        // witness publish time; publish slot is the receiver-write slot.
        assert_eq!(record.sequence, 200);
        assert_eq!(record.publish_time, 200);
        assert_eq!(record.publish_slot, 250_000_000);

        let bytes = record.encode();
        assert_eq!(bytes[0..8], 3_u64.to_le_bytes());
        assert_eq!(bytes[8..24], 123_432_099_u128.to_le_bytes());
        assert_eq!(bytes[24..40], 123_481_479_u128.to_le_bytes());
        assert_eq!(bytes[40..48], 200_u64.to_le_bytes());
        assert_eq!(bytes[48..56], 250_000_000_u64.to_le_bytes());
        assert_eq!(bytes[56..64], 200_u64.to_le_bytes());
    }

    #[test]
    fn paired_non_crossing_or_invalid_witness_is_refused() {
        // update(125, 300) does not cross T(3) = 120.
        assert_eq!(
            record_from_witness(SELECTION_CROSSING_V1, 0, 30, 3, update(125, 300), 8, 2),
            Err(CrossingError::NotBoundaryWitness)
        );

        // Normalization refusals propagate; the witness is never admitted
        // with a fabricated interval.
        let mut hostile = update(100, 200);
        hostile.price = -1;
        assert_eq!(
            record_from_witness(SELECTION_CROSSING_V1, 0, 30, 3, hostile, 8, 2),
            Err(CrossingError::Normalization(Error::InvalidPrice))
        );
    }
}
