//! Fail-closed native B-spline occupation-resolution fold.
//!
//! This module defines two statistic identities that are distinct from the
//! terminal/minimum/maximum point statistics and from TWAP:
//! [`STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06`] and
//! [`STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07`].  Both evaluate
//! every exact canonical bucket point in a sealed source archive, accumulate
//! the resulting quantized native basis vectors with the bounded associative
//! `clutch-bspline-accumulator` monoid, and finalize under the policy named by
//! the statistic identity.
//!
//! The current SourceArchive V1 wire stores conservative intervals but has no
//! authenticated missing-record kind.  Therefore archive-backed preflight
//! accepts only `low == high` and never selects a midpoint or endpoint.  The
//! lower-level [`summarize_canonical_buckets`] seam already represents gaps
//! explicitly; both finalizers preserve them as a refusal rather than dropping
//! them.  A future archive revision can map an authenticated gap record to that
//! seam without changing the occupation algebra.
//!
//! The live Resolve route selects this fold only when immutable degree-one
//! through degree-three Terms name statistic 6 or 7.  It then persists the
//! result in the distinct 383-byte occupation Resolution v4 account.  Point
//! resolution retains its separate 319-byte v3 wire and caller projection;
//! neither path infers semantics from vector contents.

use clutch_accumulator::{CoveragePolicy, FeedIdentity, Grid, WindowDomain, MAX_VALUE};
use clutch_bspline::{BasisSpec, EdgePolicy as BasisEdgePolicy};
use clutch_bspline_accumulator::{
    BasisDomain, Error as AccumulatorError, FinalWeights, FinalizationMode,
    SequentialSummaryBuilder, Summary, BASIS_EVALUATOR_VERSION, OCCUPATION_SUMMARY_VERSION,
};
pub use clutch_solana_layout::occupation_resolution::{
    STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06, STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
};
use clutch_solana_layout::{CodecError, Hash32, PayoutVectorBytes, TermsAccount};
use clutch_solana_reference::{
    AMBIG_REFUSE_01, EDGE_CLAMP_01, EDGE_REFUSE_02, FAIL_EXTENDED_WINDOW_02,
    FAIL_UNIFORM_REFUND_01, GEN_EXACT_01,
};

use crate::source_archive::{
    self, ArchiveAccountViewV1, SealedArchiveReceiptV1, SourceArchiveError,
    VerifiedSealedArchiveViewV1, SOURCE_ARCHIVE_MAX_RECORDS_V1,
};

/// Maximum canonical buckets consumed by this SourceArchive V1 preflight.
pub const NATIVE_WINDOW_MAX_BUCKETS_V1: usize = SOURCE_ARCHIVE_MAX_RECORDS_V1;

const GRID_IDENTITY_MAGIC_V1: [u8; 8] = *b"DCGRIDV1";

/// One explicit canonical bucket supplied to the source-neutral fold seam.
///
/// A conservative interval is retained as two endpoints until admission.  A
/// non-point interval refuses; no method exists that computes its midpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalBucketV1 {
    /// One source-authenticated conservative interval.
    Observation {
        /// Inclusive conservative low endpoint.
        low: u128,
        /// Inclusive conservative high endpoint.
        high: u128,
    },
    /// One authenticated missing bucket.
    Gap,
}

/// Explicit finalization policy selected by one immutable statistic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowFinalizationV1 {
    /// Every component mass must divide by coverage exactly.
    ExactOnly,
    /// Canonical largest remainder with lowest-index exact ties.
    LargestRemainderV1,
}

impl NativeWindowFinalizationV1 {
    const fn from_statistic(statistic: u16) -> Result<Self, NativeWindowError> {
        match statistic {
            STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06 => Ok(Self::ExactOnly),
            STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07 => Ok(Self::LargestRemainderV1),
            _ => Err(NativeWindowError::WrongStatistic),
        }
    }

    const fn accumulator_mode(self) -> FinalizationMode {
        match self {
            Self::ExactOnly => FinalizationMode::ExactOnly,
            Self::LargestRemainderV1 => FinalizationMode::LargestRemainderV1,
        }
    }

    /// Canonical Resolution-v4 wire discriminator for this finalizer.
    pub const fn wire_id(self) -> u8 {
        match self {
            Self::ExactOnly => 1,
            Self::LargestRemainderV1 => 2,
        }
    }
}

/// Deterministic refusal from occupation-domain admission or archive preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowError {
    /// The immutable Terms account was malformed or not self-certifying.
    Terms(CodecError),
    /// The bounded occupation accumulator refused the domain, fold, or finalizer.
    Accumulator(AccumulatorError),
    /// The sealed archive receipt could not read the exact committed record.
    Archive(SourceArchiveError),
    /// Occupation resolution is defined only for native degrees one through three.
    WrongBasisDegree,
    /// The Terms statistic is not either registered occupation identity.
    WrongStatistic,
    /// Terms selected an unregistered ambiguity, repair, failure, or edge policy.
    UnsupportedPolicy,
    /// Terms selected an evaluator version other than the frozen basis evaluator.
    WrongEvaluatorVersion,
    /// The Terms window could not be reconstructed canonically.
    InvalidWindowDomain,
    /// The sealed receipt does not bind the exact Terms feed/window/range/maturity.
    ArchiveBindingMismatch,
    /// The requested span is empty or exceeds the 32-record SourceArchive V1 bound.
    WindowTooLarge,
    /// A canonical record did not occupy its exact expected bucket.
    NonCanonicalBucket,
    /// An observation was a genuine interval, so no canonical point exists.
    NonPointObservation,
}

impl From<CodecError> for NativeWindowError {
    fn from(error: CodecError) -> Self {
        Self::Terms(error)
    }
}

impl From<AccumulatorError> for NativeWindowError {
    fn from(error: AccumulatorError) -> Self {
        Self::Accumulator(error)
    }
}

impl From<SourceArchiveError> for NativeWindowError {
    fn from(error: SourceArchiveError) -> Self {
        Self::Archive(error)
    }
}

/// Non-authoritative result of replaying one exact sealed archive.
///
/// The fields bind the candidate to immutable Terms and to both the canonical
/// window-domain identity and exact page commitment.  This value is not a
/// Resolution account and cannot authorize a kernel transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowPreflightV1 {
    terms: Hash32,
    feed: Hash32,
    window: Hash32,
    archive_commitment: Hash32,
    statistic: u16,
    finalization: NativeWindowFinalizationV1,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    sealed_feed_cursor: u64,
    repair_generation: u64,
    sample_count: u64,
    coverage_count: u64,
    gap_count: u64,
    basis_evaluator_version: u16,
    occupation_summary_version: u16,
    vector: PayoutVectorBytes,
}

impl NativeWindowPreflightV1 {
    /// Digest of the exact immutable Terms account.
    pub const fn terms(self) -> Hash32 {
        self.terms
    }

    /// Canonical feed/source-spec identity.
    pub const fn feed(self) -> Hash32 {
        self.feed
    }

    /// Canonical immutable window-domain identity.
    pub const fn window(self) -> Hash32 {
        self.window
    }

    /// Commitment of the exact sealed archive page replayed by preflight.
    pub const fn archive_commitment(self) -> Hash32 {
        self.archive_commitment
    }

    /// Distinct registered occupation statistic identity.
    pub const fn statistic(self) -> u16 {
        self.statistic
    }

    /// Explicit final averaging policy.
    pub const fn finalization(self) -> NativeWindowFinalizationV1 {
        self.finalization
    }

    /// Inclusive first canonical bucket.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Exclusive final canonical bucket.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }

    /// Feed cursor authenticated when the canonical archive was sealed.
    pub const fn sealed_feed_cursor(self) -> u64 {
        self.sealed_feed_cursor
    }

    /// Exact repair generation selected by immutable Terms.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// Number of canonical buckets, including explicit gaps.
    pub const fn sample_count(self) -> u64 {
        self.sample_count
    }

    /// Number of buckets with admitted exact points.
    pub const fn coverage_count(self) -> u64 {
        self.coverage_count
    }

    /// Number of explicit gaps retained by the summary.
    pub const fn gap_count(self) -> u64 {
        self.gap_count
    }

    /// Frozen native point-evaluator semantic version.
    pub const fn basis_evaluator_version(self) -> u16 {
        self.basis_evaluator_version
    }

    /// Frozen occupation-summary semantic version.
    pub const fn occupation_summary_version(self) -> u16 {
        self.occupation_summary_version
    }

    /// Candidate native payout vector.  The future resolution account is its
    /// sole permitted persisted owner.
    pub const fn vector(self) -> PayoutVectorBytes {
        self.vector
    }
}

/// Construct the exact source-neutral occupation domain from immutable Terms.
///
/// `terms.terms` binds the whole basis and mode.  The grid identity is a
/// collision-free fixed encoding of its family, version, and duration rather
/// than a caller-provided label.
#[inline(never)]
pub fn occupation_domain(terms: &TermsAccount) -> Result<BasisDomain, NativeWindowError> {
    terms.validate()?;
    validate_registered_terms(terms)?;
    let spec = basis_spec(terms)?;
    BasisDomain::new(
        terms.terms.bytes(),
        canonical_grid_identity_v1(
            terms.grid_family_id,
            terms.grid_version,
            terms.bucket_seconds,
        ),
        terms.bucket_seconds,
        spec,
    )
    .map_err(Into::into)
}

/// Fold an exact bounded sequence while retaining every explicit gap.
///
/// This seam authenticates nothing.  Its production caller must derive each
/// element from a verified canonical archive.  It exists separately so the
/// future archive gap kind has exactly one mapping and tests can prove that a
/// gap increments `sample_count` without incrementing `coverage_count`.
#[inline(never)]
pub fn summarize_canonical_buckets(
    domain: BasisDomain,
    start_bucket: u64,
    buckets: &[CanonicalBucketV1],
) -> Result<Summary, NativeWindowError> {
    if buckets.is_empty() || buckets.len() > NATIVE_WINDOW_MAX_BUCKETS_V1 {
        return Err(NativeWindowError::WindowTooLarge);
    }
    let mut summary = Summary::empty(domain)?;
    let mut index = 0_usize;
    while index < buckets.len() {
        let bucket = start_bucket
            .checked_add(u64::try_from(index).map_err(|_| NativeWindowError::WindowTooLarge)?)
            .ok_or(NativeWindowError::WindowTooLarge)?;
        summary = append_bucket(summary, domain, bucket, buckets[index])?;
        index += 1;
    }
    Ok(summary)
}

/// Replay one already-authenticated sealed SourceArchive V1 into an occupation
/// payout candidate.
///
/// `receipt` is a capability: its fields are private and the source/archive
/// module constructs it only after checking the canonical key, program owner,
/// exact source spec/window lineage, seal, record order, and page commitment.
/// This function rechecks the Terms-facing feed/window/range/maturity join and
/// rechecks account key/owner/content on every record read.
#[inline(never)]
pub fn preflight_sealed_archive(
    terms: &TermsAccount,
    receipt: SealedArchiveReceiptV1,
    archive: ArchiveAccountViewV1<'_>,
) -> Result<NativeWindowPreflightV1, NativeWindowError> {
    let (domain, finalization, span) = validate_archive_binding(terms, receipt)?;
    let summary = summarize_archive(domain, receipt, archive, span)?;
    finalize_preflight(terms, receipt, finalization, summary)
}

/// Replay a lifetime-bound, once-verified sealed SourceArchive V1 into an
/// occupation payout candidate.
///
/// This is the production fold seam.  The archive capability's private
/// constructor has already checked the complete account, release, lineage,
/// seal, and page commitment, and its immutable borrow prevents page mutation
/// during this fold.  Each indexed read therefore checks only its bounded
/// record index; it does not rehash the 2,560-byte page for every bucket.
#[inline(never)]
pub fn preflight_verified_archive(
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Result<NativeWindowPreflightV1, NativeWindowError> {
    let receipt = archive.receipt();
    let (domain, finalization, span) = validate_archive_binding(terms, receipt)?;
    let summary = summarize_verified_archive(domain, receipt, archive, span)?;
    finalize_preflight(terms, receipt, finalization, summary)
}

#[inline(never)]
fn validate_archive_binding(
    terms: &TermsAccount,
    receipt: SealedArchiveReceiptV1,
) -> Result<(BasisDomain, NativeWindowFinalizationV1, u64), NativeWindowError> {
    let domain = occupation_domain(terms)?;
    let finalization = NativeWindowFinalizationV1::from_statistic(terms.statistic_id)?;
    let expected_window = occupation_window(terms)?;
    let expected_window_id = source_archive::canonical_window_id(expected_window);
    if receipt.feed() != terms.feed
        || receipt.window() != expected_window_id
        || receipt.start_bucket() != terms.expected_start_bucket
        || receipt.end_bucket_exclusive() != terms.expected_end_bucket_exclusive
        || receipt.sealed_feed_cursor() < expected_window.maturity_bucket_exclusive()
        || receipt.page_commitment() == Hash32::ZERO
    {
        return Err(NativeWindowError::ArchiveBindingMismatch);
    }

    let span = receipt
        .end_bucket_exclusive()
        .checked_sub(receipt.start_bucket())
        .ok_or(NativeWindowError::ArchiveBindingMismatch)?;
    if span == 0 || span > NATIVE_WINDOW_MAX_BUCKETS_V1 as u64 {
        return Err(NativeWindowError::WindowTooLarge);
    }
    Ok((domain, finalization, span))
}

#[inline(never)]
fn finalize_preflight(
    terms: &TermsAccount,
    receipt: SealedArchiveReceiptV1,
    finalization: NativeWindowFinalizationV1,
    summary: Summary,
) -> Result<NativeWindowPreflightV1, NativeWindowError> {
    let finalized = summary.finalize(finalization.accumulator_mode())?;
    let vector = vector_from_final(finalized)?;
    Ok(NativeWindowPreflightV1 {
        terms: terms.terms,
        feed: terms.feed,
        window: receipt.window(),
        archive_commitment: receipt.page_commitment(),
        statistic: terms.statistic_id,
        finalization,
        start_bucket: summary.start_bucket(),
        end_bucket_exclusive: summary.end_bucket_exclusive(),
        sealed_feed_cursor: receipt.sealed_feed_cursor(),
        repair_generation: terms.repair_generation,
        sample_count: summary.sample_count(),
        coverage_count: summary.coverage_count(),
        gap_count: summary.gap_count(),
        basis_evaluator_version: BASIS_EVALUATOR_VERSION,
        occupation_summary_version: OCCUPATION_SUMMARY_VERSION,
        vector,
    })
}

#[inline(never)]
fn summarize_archive(
    domain: BasisDomain,
    receipt: SealedArchiveReceiptV1,
    archive: ArchiveAccountViewV1<'_>,
    span: u64,
) -> Result<Summary, NativeWindowError> {
    let mut summary = Summary::empty(domain)?;
    let mut index = 0_u64;
    while index < span {
        let archived = source_archive::archived_observation(
            receipt,
            archive,
            usize::try_from(index).map_err(|_| NativeWindowError::WindowTooLarge)?,
        )?;
        let expected_bucket = receipt
            .start_bucket()
            .checked_add(index)
            .ok_or(NativeWindowError::NonCanonicalBucket)?;
        if archived.bucket != expected_bucket {
            return Err(NativeWindowError::NonCanonicalBucket);
        }
        summary = append_bucket(
            summary,
            domain,
            expected_bucket,
            CanonicalBucketV1::Observation {
                low: archived.low,
                high: archived.high,
            },
        )?;
        index += 1;
    }
    Ok(summary)
}

#[inline(never)]
fn summarize_verified_archive(
    domain: BasisDomain,
    receipt: SealedArchiveReceiptV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
    span: u64,
) -> Result<Summary, NativeWindowError> {
    let mut summary = SequentialSummaryBuilder::new(domain)?;
    let mut index = 0_u64;
    while index < span {
        let archived = archive.archived_observation(
            usize::try_from(index).map_err(|_| NativeWindowError::WindowTooLarge)?,
        )?;
        let expected_bucket = receipt
            .start_bucket()
            .checked_add(index)
            .ok_or(NativeWindowError::NonCanonicalBucket)?;
        if archived.bucket != expected_bucket {
            return Err(NativeWindowError::NonCanonicalBucket);
        }
        if archived.low != archived.high {
            return Err(NativeWindowError::NonPointObservation);
        }
        summary.append_accepted(expected_bucket, archived.low)?;
        index += 1;
    }
    Ok(summary.finish())
}

#[inline(never)]
fn append_bucket(
    summary: Summary,
    domain: BasisDomain,
    bucket: u64,
    sample: CanonicalBucketV1,
) -> Result<Summary, NativeWindowError> {
    let singleton = match sample {
        CanonicalBucketV1::Observation { low, high } => {
            if low != high {
                return Err(NativeWindowError::NonPointObservation);
            }
            Summary::accepted(domain, bucket, low)?
        }
        CanonicalBucketV1::Gap => Summary::missing(domain, bucket)?,
    };
    summary.combine(singleton).map_err(Into::into)
}

#[inline(never)]
fn vector_from_final(finalized: FinalWeights) -> Result<PayoutVectorBytes, NativeWindowError> {
    finalized.validate()?;
    let vector = PayoutVectorBytes {
        denominator: finalized.denominator(),
        weights: finalized.weights(),
    };
    vector.validate_active(finalized.active_len(), finalized.denominator())?;
    Ok(vector)
}

fn validate_registered_terms(terms: &TermsAccount) -> Result<(), NativeWindowError> {
    if !(1..=3).contains(&terms.basis_degree) {
        return Err(NativeWindowError::WrongBasisDegree);
    }
    NativeWindowFinalizationV1::from_statistic(terms.statistic_id)?;
    if terms.ambiguity_policy_id != AMBIG_REFUSE_01
        || terms.repair_policy_id != u32::from(GEN_EXACT_01)
        || (terms.failure_policy_id != u32::from(FAIL_UNIFORM_REFUND_01)
            && terms.failure_policy_id != u32::from(FAIL_EXTENDED_WINDOW_02))
        || (terms.edge_policy_id != EDGE_CLAMP_01 && terms.edge_policy_id != EDGE_REFUSE_02)
    {
        return Err(NativeWindowError::UnsupportedPolicy);
    }
    if terms.evaluator_version != u32::from(BASIS_EVALUATOR_VERSION) {
        return Err(NativeWindowError::WrongEvaluatorVersion);
    }
    Ok(())
}

fn basis_spec(terms: &TermsAccount) -> Result<BasisSpec, NativeWindowError> {
    let edge_policy = match terms.edge_policy_id {
        EDGE_CLAMP_01 => BasisEdgePolicy::Clamp,
        EDGE_REFUSE_02 => BasisEdgePolicy::Refuse,
        _ => return Err(NativeWindowError::UnsupportedPolicy),
    };
    let spec = BasisSpec {
        outcome_count: terms.outcome_count,
        degree: terms.basis_degree,
        knot_count: terms.knot_count,
        uniform_log2_spacing: terms.uniform_log2_spacing,
        denominator: terms.payouts[0].denominator,
        domain_max: MAX_VALUE,
        edge_policy,
        knots: terms.knots,
    };
    Ok(spec)
}

/// Construct the exact immutable archive window selected by validated Terms.
///
/// Account adapters use this to derive the canonical SourceArchive PDA before
/// constructing a verified sealed-archive capability. It performs the same
/// checked maturity and coverage-policy mapping as monolithic resolution.
pub fn occupation_window(terms: &TermsAccount) -> Result<WindowDomain, NativeWindowError> {
    let coverage_id = u16::try_from(terms.coverage_policy_id)
        .map_err(|_| NativeWindowError::InvalidWindowDomain)?;
    let coverage = CoveragePolicy::from_registry(coverage_id, terms.coverage_policy_parameter)
        .map_err(|_| NativeWindowError::InvalidWindowDomain)?;
    let feed = FeedIdentity::new(
        terms.source_adapter_id.bytes(),
        terms.feed.bytes(),
        terms.source_version,
        terms.evaluator_version,
    )
    .map_err(|_| NativeWindowError::InvalidWindowDomain)?;
    let grid = Grid::new(
        terms.grid_family_id,
        terms.grid_version,
        terms.bucket_seconds,
    )
    .map_err(|_| NativeWindowError::InvalidWindowDomain)?;
    let maturity = terms
        .expected_start_bucket
        .checked_add(terms.maturity_horizon_buckets)
        .ok_or(NativeWindowError::InvalidWindowDomain)?;
    WindowDomain::new(
        feed,
        grid,
        terms.expected_start_bucket,
        terms.expected_end_bucket_exclusive,
        maturity,
        terms.repair_generation,
        coverage,
    )
    .map_err(|_| NativeWindowError::InvalidWindowDomain)
}

/// Exact fixed-width identity of the occupation grid.
///
/// The 22-byte active prefix is `DCGRIDV1 || family_le || version_le ||
/// bucket_seconds_le`; the ten-byte suffix is canonical zero padding.  This is
/// collision-free for the represented fields and avoids a second grid hash
/// algorithm inside the preflight.
pub const fn canonical_grid_identity_v1(
    family_id: u32,
    version: u16,
    bucket_seconds: u64,
) -> [u8; 32] {
    let mut out = [0_u8; 32];
    let family = family_id.to_le_bytes();
    let version = version.to_le_bytes();
    let duration = bucket_seconds.to_le_bytes();
    let mut index = 0_usize;
    while index < GRID_IDENTITY_MAGIC_V1.len() {
        out[index] = GRID_IDENTITY_MAGIC_V1[index];
        index += 1;
    }
    index = 0;
    while index < family.len() {
        out[8 + index] = family[index];
        index += 1;
    }
    index = 0;
    while index < version.len() {
        out[12 + index] = version[index];
        index += 1;
    }
    index = 0;
    while index < duration.len() {
        out[14 + index] = duration[index];
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        PayoutVectorBytes, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
    };

    fn hash(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn terms(statistic: u16) -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut unit = [0_u64; MAX_OUTCOMES];
        unit[0] = 7;
        payouts[0] = PayoutVectorBytes {
            denominator: 7,
            weights: unit,
        };
        let mut knots = [0_u128; MAX_KNOTS];
        knots[..3].copy_from_slice(&[0, 8, 16]);
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: hash(1),
            profile: hash(2),
            feed: hash(3),
            price_grid: hash(4),
            outcome_count: 4,
            payout_count: 1,
            payouts,
            grid_family_id: 5,
            grid_version: 2,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 102,
            maturity_horizon_buckets: 3,
            coverage_policy_id: u32::from(CoveragePolicy::COMPLETE_REQUIRED.id()),
            repair_policy_id: u32::from(GEN_EXACT_01),
            failure_policy_id: u32::from(FAIL_UNIFORM_REFUND_01),
            statistic_id: statistic,
            ambiguity_policy_id: AMBIG_REFUSE_01,
            edge_policy_id: EDGE_CLAMP_01,
            basis_degree: 2,
            knot_count: 3,
            uniform_log2_spacing: 3,
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: 6,
            source_version: 7,
            evaluator_version: u32::from(BASIS_EVALUATOR_VERSION),
            source_adapter_id: hash(5),
            payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
            knots,
            collateral_cap: 1_000,
            stored_bump: 9,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().unwrap();
        value
    }

    #[test]
    fn occupation_statistics_are_distinct_from_point_and_twap() {
        assert_ne!(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06, 1);
        assert_ne!(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06, 4);
        assert_ne!(
            STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
            STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07
        );
        assert_eq!(
            NativeWindowFinalizationV1::from_statistic(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06),
            Ok(NativeWindowFinalizationV1::ExactOnly)
        );
        assert_eq!(
            NativeWindowFinalizationV1::from_statistic(
                STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07
            ),
            Ok(NativeWindowFinalizationV1::LargestRemainderV1)
        );
    }

    #[test]
    fn occupation_domain_is_native_for_every_degree_one_through_three() {
        for (degree, outcomes) in [(1_u8, 3_u8), (2, 4), (3, 5)] {
            let mut value = terms(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
            value.basis_degree = degree;
            value.outcome_count = outcomes;
            value.knot_count = 3;
            value.terms = Hash32::ZERO;
            value.terms = value.recomputed_terms_digest().unwrap();
            let domain = occupation_domain(&value).unwrap();
            assert_eq!(domain.spec().degree, degree);
            assert_eq!(domain.spec().outcome_count, outcomes);
        }
    }

    #[test]
    fn nonpoint_interval_refuses_instead_of_selecting_a_midpoint() {
        let domain =
            occupation_domain(&terms(STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07))
                .unwrap();
        assert_eq!(
            summarize_canonical_buckets(
                domain,
                100,
                &[CanonicalBucketV1::Observation { low: 0, high: 8 }]
            ),
            Err(NativeWindowError::NonPointObservation)
        );
    }

    #[test]
    fn explicit_gap_survives_the_associative_fold_and_both_finalizers_refuse() {
        let domain =
            occupation_domain(&terms(STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07))
                .unwrap();
        let buckets = [
            CanonicalBucketV1::Observation { low: 0, high: 0 },
            CanonicalBucketV1::Gap,
            CanonicalBucketV1::Observation { low: 4, high: 4 },
        ];
        let summary = summarize_canonical_buckets(domain, 100, &buckets).unwrap();
        assert_eq!(summary.sample_count(), 3);
        assert_eq!(summary.coverage_count(), 2);
        assert_eq!(summary.gap_count(), 1);
        assert_eq!(
            summary.finalize(FinalizationMode::ExactOnly),
            Err(AccumulatorError::IncompleteCoverage)
        );
        assert_eq!(
            summary.finalize(FinalizationMode::LargestRemainderV1),
            Err(AccumulatorError::IncompleteCoverage)
        );

        let left = summarize_canonical_buckets(domain, 100, &buckets[..1]).unwrap();
        let right = summarize_canonical_buckets(domain, 101, &buckets[1..]).unwrap();
        assert_eq!(left.combine(right).unwrap(), summary);
    }

    #[test]
    fn exact_and_largest_remainder_are_separate_finalization_boundaries() {
        let exact_terms = terms(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        let exact_domain = occupation_domain(&exact_terms).unwrap();
        let samples = [
            CanonicalBucketV1::Observation { low: 0, high: 0 },
            CanonicalBucketV1::Observation { low: 4, high: 4 },
        ];
        let exact = summarize_canonical_buckets(exact_domain, 100, &samples).unwrap();
        assert_eq!(
            exact.finalize(FinalizationMode::ExactOnly),
            Err(AccumulatorError::InexactAverage)
        );

        let rounded_terms = terms(STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07);
        let rounded_domain = occupation_domain(&rounded_terms).unwrap();
        let rounded = summarize_canonical_buckets(rounded_domain, 100, &samples)
            .unwrap()
            .finalize(FinalizationMode::LargestRemainderV1)
            .unwrap();
        assert_eq!(&rounded.weights()[..4], &[5, 2, 0, 0]);
    }

    #[test]
    fn degree_zero_and_point_statistic_terms_refuse() {
        let mut categorical = terms(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        categorical.basis_degree = 0;
        assert_eq!(
            validate_registered_terms(&categorical),
            Err(NativeWindowError::WrongBasisDegree)
        );

        let mut point = terms(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        point.statistic_id = 1;
        assert_eq!(
            validate_registered_terms(&point),
            Err(NativeWindowError::WrongStatistic)
        );
    }

    #[test]
    fn grid_identity_is_fixed_width_and_field_complete() {
        let baseline = canonical_grid_identity_v1(5, 2, 60);
        assert_eq!(&baseline[..8], b"DCGRIDV1");
        assert_eq!(&baseline[22..], &[0_u8; 10]);
        assert_ne!(baseline, canonical_grid_identity_v1(6, 2, 60));
        assert_ne!(baseline, canonical_grid_identity_v1(5, 3, 60));
        assert_ne!(baseline, canonical_grid_identity_v1(5, 2, 61));
    }
}
