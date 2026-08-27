//! `CROSSING_V1` — the closing-boundary selection rule.
//!
//! Runtime port of `research/source-profile-v1/src/crossing_v1.rs` per
//! `R2_PULL_PROMOTION_PLAN.md` P0.2, with the research crate's `MODEL_*`
//! envelope replaced by the real [`clutch_accumulator`] one.
//!
//! Let the frozen grid origin be `G = 0`, `bucket_seconds = B`, and bucket `k`
//! cover `[G+kB, G+(k+1)B)`. This rule registers only the **closing** boundary
//! `T(k) = G+(k+1)B` — the source state in force when the bucket closes. The
//! opening-boundary experiment is not a second live rule and its id is refused
//! by [`super::spec::SourceSpecV2::new`].
//!
//! Admission for bucket `k` takes the unique update `U` with
//! `prev_publish_time(U) < T(k) <= publish_time(U)`:
//!
//! * a degenerate update (`prev == publish`, a failed aggregation) satisfies
//!   the predicate for no `T` at all and witnesses no boundary;
//! * an absent crossing witness is an explicit [`CrossingError::Stall`] —
//!   nothing manufactures a `Missing` record or substitutes an adjacent update;
//! * two *distinct* qualifying update bodies for one boundary is the falsifier
//!   for the entire provider selection
//!   ([`CrossingError::DoubleWitnessBoundary`]); the rule refuses rather than
//!   picks, and one demonstrated instance reopens the R2 provider choice;
//! * one update may legitimately witness consecutive boundaries, so the archive
//!   sequence (`sequence := publish_time(U)`) is monotone **non-strict**, with
//!   equality admissible only when the record bodies are byte-identical except
//!   the bucket field ([`CrossingError::EqualSequenceValueDrift`] otherwise).
//!
//! Semantic owners are disambiguated and no field doubles as another: archive
//! *source publish time* is `publish_time(U)`; archive *source publish slot* is
//! the update account's `posted_slot` (a receiver-write slot, explicitly not
//! source-native); archive *sequence* is `publish_time(U)`.
//!
//! ## Why the 64-byte record layout is the V1 one
//!
//! [`ArchiveRecordV2`] is byte-identical in field order and width to the
//! records [`crate::source_archive`] already writes (`SOURCE_ADMISSION_V1`
//! §5.3). That is deliberate and load-bearing: it is what lets a v2-authored
//! archive page be verified and folded by the *existing* sealed-archive reader,
//! so the resolution plane needs no second record decoder.
//! `the_record_layout_matches_the_v1_archive_record` pins the equality.

use clutch_accumulator::MAX_BUCKET_SECONDS;

use crate::pyth_receiver::{
    normalize_interval, selects_boundary, FullPriceUpdateV2, PythReceiverError,
};

use super::spec::GRID_ORIGIN_UNIX_SECONDS_V1;

/// Closing-boundary `CROSSING_V1`: `T(k) = G+(k+1)*B`, with `G = 0`.
///
/// Id `1` is V1's finalized-bucket rule and id `3` was the rejected
/// opening-boundary experiment; neither is admissible under the v2 domain.
pub const SELECTION_CROSSING_V1: u16 = 2;

/// Exact byte length of one archive record.
pub const ARCHIVE_RECORD_V2_BYTES: usize = 64;

/// Byte length of the leading bucket field excluded from body identity.
const BUCKET_FIELD_BYTES: usize = 8;

/// Refusals from boundary construction, witness selection, and admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossingError {
    /// The rule id is not a registered `CROSSING_V1` boundary variant.
    UnknownSelectionRule,
    /// `bucket_seconds` is zero or outside the accumulator grid envelope.
    InvalidBucketSeconds,
    /// The grid origin is not the frozen Unix-epoch origin.
    UnknownGridOrigin,
    /// `T(k)` is not representable, so no update could ever witness it. The
    /// configuration is refused rather than stalled.
    BoundaryOverflow,
    /// No presented update witnesses the boundary. The feed stalls; this is
    /// never adapted into a fabricated `Missing` record.
    Stall,
    /// Two distinct updates both witness one boundary. This falsifies the
    /// provider's documented uniqueness and reopens the R2 selection.
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
    /// the bucket field. This falsifies the non-strict equality clause, and the
    /// rule must be replaced rather than relaxed.
    EqualSequenceValueDrift,
    /// The witness value failed conservative interval normalization.
    Normalization(PythReceiverError),
}

/// One archive record, in the fixed 64-byte layout.
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
    /// Observation bucket this record closes.
    pub bucket: u64,
    /// Conservative low endpoint in normalized atoms.
    pub low: u128,
    /// Conservative high endpoint in normalized atoms.
    pub high: u128,
    /// Archive sequence; the witness publish time.
    pub sequence: u64,
    /// Receiver-write slot of the witnessing update account.
    pub publish_slot: u64,
    /// Witness publish time.
    pub publish_time: u64,
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
///
/// This is the exact predicate the non-strict sequence rule needs: one witness
/// legitimately covering two consecutive boundaries produces records that
/// differ in the bucket and in nothing else.
pub fn body_identical(left: ArchiveRecordV2, right: ArchiveRecordV2) -> bool {
    left.encode()[BUCKET_FIELD_BYTES..] == right.encode()[BUCKET_FIELD_BYTES..]
}

/// Compute the boundary instant `T(k)` for one registered variant.
///
/// The result always fits `i64` so it can be compared against publish times;
/// an unrepresentable boundary is a refused configuration, never a stall.
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
    if bucket_seconds == 0 || bucket_seconds > MAX_BUCKET_SECONDS {
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
/// No qualifying candidate is an explicit [`CrossingError::Stall`]. Two
/// candidates that qualify and differ in any field are the uniqueness falsifier
/// and are refused; byte-identical duplicates of one witness carry no selection
/// surface and collapse to that witness.
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
/// The crossing predicate is rechecked here, so a caller-paired update that
/// does not witness the named boundary is refused rather than adapted.
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
/// Buckets must be consecutive. The sequence may repeat only when the same
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

/// Start-aware cursor for one contiguous archive window.
///
/// The first append is checked against the immutable window start rather than
/// merely accepted for want of a predecessor; every successful append advances
/// exactly one bucket; an unrepresentable exclusive cursor is refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveCursorV2 {
    start_bucket: u64,
    next_bucket: u64,
    previous: Option<ArchiveRecordV2>,
}

impl ArchiveCursorV2 {
    /// Open a cursor at an immutable window start with no predecessor.
    pub const fn new(start_bucket: u64) -> Self {
        Self {
            start_bucket,
            next_bucket: start_bucket,
            previous: None,
        }
    }

    /// Reopen a cursor mid-window from authenticated archive state.
    ///
    /// `next_bucket` and `previous` must come from a verified archive account,
    /// never from caller instruction data.
    pub const fn resumed(
        start_bucket: u64,
        next_bucket: u64,
        previous: Option<ArchiveRecordV2>,
    ) -> Self {
        Self {
            start_bucket,
            next_bucket,
            previous,
        }
    }

    /// The immutable window start.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// The exact next bucket this cursor will admit.
    pub const fn next_bucket(self) -> u64 {
        self.next_bucket
    }

    /// The last admitted record, if any.
    pub const fn previous(self) -> Option<ArchiveRecordV2> {
        self.previous
    }

    /// Admit one record at the exact next bucket.
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

#[cfg(test)]
mod tests {
    use super::*;

    const BUCKET_SECONDS: u64 = 10;
    const ORIGIN: i64 = GRID_ORIGIN_UNIX_SECONDS_V1;

    fn update(publish: i64, prev: i64) -> FullPriceUpdateV2 {
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

    fn record(bucket: u64, witness: FullPriceUpdateV2) -> ArchiveRecordV2 {
        record_from_witness(
            SELECTION_CROSSING_V1,
            ORIGIN,
            BUCKET_SECONDS,
            bucket,
            witness,
            8,
            2,
        )
        .expect("witness qualifies")
    }

    #[test]
    fn the_record_layout_matches_the_v1_archive_record() {
        // The v2 record must be byte-compatible with the records
        // `source_archive` already writes, or a v2-authored page could not be
        // read by the existing sealed-archive reader.
        assert_eq!(
            ARCHIVE_RECORD_V2_BYTES,
            crate::source_archive::SOURCE_ARCHIVE_RECORD_V1_BYTES
        );
        let encoded = ArchiveRecordV2 {
            bucket: 0x0102_0304_0506_0708,
            low: 9,
            high: 10,
            sequence: 11,
            publish_slot: 12,
            publish_time: 13,
        }
        .encode();
        assert_eq!(encoded[0..8], 0x0102_0304_0506_0708_u64.to_le_bytes());
        assert_eq!(encoded[8..24], 9_u128.to_le_bytes());
        assert_eq!(encoded[24..40], 10_u128.to_le_bytes());
        assert_eq!(encoded[40..48], 11_u64.to_le_bytes());
        assert_eq!(encoded[48..56], 12_u64.to_le_bytes());
        assert_eq!(encoded[56..64], 13_u64.to_le_bytes());
    }

    #[test]
    fn the_boundary_is_the_close_of_the_bucket() {
        // Bucket k covers [kB, (k+1)B); the registered boundary is its close.
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, 0),
            Ok(10)
        );
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, 169_999_999),
            Ok(1_700_000_000)
        );
    }

    #[test]
    fn unregistered_rules_origins_and_grids_are_refused() {
        for rule in [0_u16, 1, 3, u16::MAX] {
            assert_eq!(
                boundary_instant(rule, ORIGIN, BUCKET_SECONDS, 1),
                Err(CrossingError::UnknownSelectionRule)
            );
        }
        for origin in [1_i64, -1] {
            assert_eq!(
                boundary_instant(SELECTION_CROSSING_V1, origin, BUCKET_SECONDS, 1),
                Err(CrossingError::UnknownGridOrigin)
            );
        }
        for seconds in [0_u64, MAX_BUCKET_SECONDS + 1] {
            assert_eq!(
                boundary_instant(SELECTION_CROSSING_V1, ORIGIN, seconds, 1),
                Err(CrossingError::InvalidBucketSeconds)
            );
        }
    }

    #[test]
    fn an_unrepresentable_boundary_is_refused_not_stalled() {
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, u64::MAX),
            Err(CrossingError::BoundaryOverflow)
        );
        // k+1 fits but (k+1)*B does not.
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, u64::MAX - 1),
            Err(CrossingError::BoundaryOverflow)
        );
        // The product fits u64 but not i64, so it could never be compared
        // against a publish time.
        assert_eq!(
            boundary_instant(SELECTION_CROSSING_V1, ORIGIN, 2, u64::MAX / 2),
            Err(CrossingError::BoundaryOverflow)
        );
    }

    #[test]
    fn an_absent_witness_stalls_and_never_fabricates_a_record() {
        let far_past = update(5, 4);
        assert_eq!(
            select_witness(
                SELECTION_CROSSING_V1,
                ORIGIN,
                BUCKET_SECONDS,
                10,
                &[far_past]
            ),
            Err(CrossingError::Stall)
        );
        assert_eq!(
            select_witness(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, 10, &[]),
            Err(CrossingError::Stall)
        );
    }

    #[test]
    fn a_degenerate_aggregate_witnesses_nothing() {
        let degenerate = update(110, 110);
        for bucket in 0..20_u64 {
            assert_eq!(
                witnesses_boundary(
                    SELECTION_CROSSING_V1,
                    ORIGIN,
                    BUCKET_SECONDS,
                    bucket,
                    degenerate
                ),
                Ok(false)
            );
        }
    }

    #[test]
    fn two_distinct_witnesses_for_one_boundary_are_the_falsifier() {
        let first = update(112, 105);
        let mut second = first;
        second.price += 1;
        assert!(
            witnesses_boundary(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, 10, first).unwrap()
        );
        assert!(
            witnesses_boundary(SELECTION_CROSSING_V1, ORIGIN, BUCKET_SECONDS, 10, second).unwrap()
        );
        assert_eq!(
            select_witness(
                SELECTION_CROSSING_V1,
                ORIGIN,
                BUCKET_SECONDS,
                10,
                &[first, second]
            ),
            Err(CrossingError::DoubleWitnessBoundary)
        );

        // A byte-identical duplicate carries no selection surface and is not
        // the falsifier: it collapses to the single witness.
        assert_eq!(
            select_witness(
                SELECTION_CROSSING_V1,
                ORIGIN,
                BUCKET_SECONDS,
                10,
                &[first, first, first]
            ),
            Ok(first)
        );
    }

    #[test]
    fn a_non_witnessing_pair_is_refused_never_adapted() {
        let stale = update(95, 90);
        assert_eq!(
            record_from_witness(
                SELECTION_CROSSING_V1,
                ORIGIN,
                BUCKET_SECONDS,
                10,
                stale,
                8,
                2
            ),
            Err(CrossingError::NotBoundaryWitness)
        );
    }

    #[test]
    fn one_witness_may_cover_consecutive_boundaries_with_byte_identity() {
        // prev < T(10)=110 and T(11)=120 <= publish, so this single update
        // legitimately witnesses both boundaries.
        let wide = update(125, 105);
        let first = record(10, wide);
        let second = record(11, wide);
        assert_eq!(first.sequence, second.sequence);
        assert!(body_identical(first, second));
        assert_eq!(admit_after(first, second), Ok(()));
    }

    #[test]
    fn equal_sequence_with_drifting_values_is_refused() {
        let wide = update(125, 105);
        let first = record(10, wide);
        let mut drifted = record(11, wide);
        drifted.low += 1;
        assert!(!body_identical(first, drifted));
        assert_eq!(
            admit_after(first, drifted),
            Err(CrossingError::EqualSequenceValueDrift)
        );

        // A drifting publish slot is drift too: a different receiver-write
        // slot means a different qualifying witness, not a duplicate.
        let mut reposted = record(11, wide);
        reposted.publish_slot += 1;
        assert_eq!(
            admit_after(first, reposted),
            Err(CrossingError::EqualSequenceValueDrift)
        );
    }

    #[test]
    fn sequence_regression_is_refused() {
        let first = record(10, update(112, 105));
        let mut backwards = record(11, update(125, 115));
        backwards.sequence = 111;
        assert_eq!(
            admit_after(first, backwards),
            Err(CrossingError::SequenceRegression)
        );
    }

    #[test]
    fn gaps_repeats_and_reorderings_are_refused_explicitly() {
        let first = record(10, update(112, 105));
        for bucket in [10_u64, 12, 0, 9] {
            let mut next = record(11, update(125, 115));
            next.bucket = bucket;
            assert_eq!(
                admit_after(first, next),
                Err(CrossingError::NonContiguousBucket)
            );
        }
    }

    #[test]
    fn the_first_append_is_checked_against_the_window_start() {
        let cursor = ArchiveCursorV2::new(10);
        assert_eq!(cursor.next_bucket(), 10);
        assert_eq!(cursor.previous(), None);

        // A first append at any other bucket is refused even though there is
        // no predecessor to compare against.
        let wrong = record(11, update(125, 115));
        assert_eq!(cursor.admit(wrong), Err(CrossingError::NonContiguousBucket));

        let right = record(10, update(112, 105));
        let advanced = cursor.admit(right).expect("first bucket admits");
        assert_eq!(advanced.start_bucket(), 10);
        assert_eq!(advanced.next_bucket(), 11);
        assert_eq!(advanced.previous(), Some(right));
    }

    #[test]
    fn an_unrepresentable_next_cursor_is_a_named_refusal() {
        let last = ArchiveRecordV2 {
            bucket: u64::MAX,
            low: 1,
            high: 2,
            sequence: 3,
            publish_slot: 4,
            publish_time: 3,
        };
        let cursor = ArchiveCursorV2::resumed(u64::MAX, u64::MAX, None);
        assert_eq!(cursor.admit(last), Err(CrossingError::BucketCursorOverflow));
        assert_eq!(
            admit_after(last, last),
            Err(CrossingError::BucketCursorOverflow)
        );
    }

    #[test]
    fn normalization_failures_are_named_not_swallowed() {
        let mut negative = update(112, 105);
        negative.price = -1;
        assert_eq!(
            record_from_witness(
                SELECTION_CROSSING_V1,
                ORIGIN,
                BUCKET_SECONDS,
                10,
                negative,
                8,
                2
            ),
            Err(CrossingError::Normalization(
                PythReceiverError::InvalidPrice
            ))
        );
    }

    #[test]
    fn the_record_owns_each_field_with_exactly_one_meaning() {
        let witness = update(112, 105);
        let built = record(10, witness);
        assert_eq!(built.bucket, 10);
        // sequence and publish_time are both the witness publish time; the
        // publish slot is the receiver-write slot and nothing else.
        assert_eq!(built.sequence, 112);
        assert_eq!(built.publish_time, 112);
        assert_eq!(built.publish_slot, witness.posted_slot);
        assert!(built.low < built.high);
    }
}
