#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fixed-width occupation summaries for native B-spline payout vectors.
//!
//! Each accepted equal-duration bucket contributes the exact integer
//! [`clutch_bspline::WeightVector`] returned by the frozen basis evaluator.
//! Missing buckets are explicit members of the covered span and contribute no
//! mass. Summaries combine only across adjacent spans with exactly equal
//! [`BasisDomain`] values.
//!
//! This crate authenticates nothing. [`BasisDomain::spec_digest`] is an opaque
//! content identity supplied by an adapter; this crate binds it, an opaque
//! equal-duration grid identity, the exact [`BasisSpec`], and both semantic
//! versions into every summary and refuses a join mismatch. It does not claim
//! either opaque identity is authentic or recompute a hash.

use clutch_bspline::{BasisSpec, Error as BasisError, ValidatedBasisSpec, MAX_OUTCOMES};

/// Implemented point-evaluator semantic version.
pub const BASIS_EVALUATOR_VERSION: u16 = 1;
/// Implemented occupation-summary semantic version.
pub const OCCUPATION_SUMMARY_VERSION: u16 = 1;
/// Number of bytes in an opaque basis-spec digest.
pub const SPEC_DIGEST_BYTES: usize = 32;
/// Number of bytes in an opaque canonical-grid identity.
pub const GRID_IDENTITY_BYTES: usize = 32;

/// Result alias for occupation operations.
pub type Result<T> = core::result::Result<T, Error>;

/// A deterministic refusal from domain admission, accumulation, or finalize.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The basis evaluator refused the exact spec or point.
    Basis(BasisError),
    /// A version, opaque identity, or equal bucket duration is not canonical.
    InvalidDomain,
    /// A bucket's exclusive end could not be represented.
    BucketOverflow,
    /// Two non-empty spans are not exactly adjacent in the requested order.
    NonAdjacent,
    /// Two summaries name different basis, grid, duration, or versions.
    DomainMismatch,
    /// A checked count, mass, or span operation overflowed.
    ArithmeticOverflow,
    /// A summary or output violates its fixed-width canonical invariants.
    InvalidSummary,
    /// No accepted bucket exists to average.
    NoCoverage,
    /// One or more explicit gaps exist; this crate has no policy to ignore them.
    IncompleteCoverage,
    /// The exact average cannot be represented at the frozen denominator.
    InexactAverage,
}

impl From<BasisError> for Error {
    fn from(value: BasisError) -> Self {
        Self::Basis(value)
    }
}

/// Immutable identity for one occupation algebra.
///
/// Both identities are opaque here. A caller should compute and authenticate
/// them over the canonical basis and grid artifacts before constructing this
/// value. Keeping the exact spec and duration beside them makes accidental or
/// adversarial cross-domain joins fail even if a caller reuses an identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisDomain {
    evaluator_version: u16,
    summary_version: u16,
    spec_digest: [u8; SPEC_DIGEST_BYTES],
    grid_identity: [u8; GRID_IDENTITY_BYTES],
    bucket_duration: u64,
    evaluator: ValidatedBasisSpec,
}

impl BasisDomain {
    /// Admit one exact basis and equal-duration grid identity.
    pub fn new(
        spec_digest: [u8; SPEC_DIGEST_BYTES],
        grid_identity: [u8; GRID_IDENTITY_BYTES],
        bucket_duration: u64,
        spec: BasisSpec,
    ) -> Result<Self> {
        let evaluator = spec.validated()?;
        if spec_digest == [0; SPEC_DIGEST_BYTES]
            || grid_identity == [0; GRID_IDENTITY_BYTES]
            || bucket_duration == 0
        {
            return Err(Error::InvalidDomain);
        }
        Ok(Self {
            evaluator_version: BASIS_EVALUATOR_VERSION,
            summary_version: OCCUPATION_SUMMARY_VERSION,
            spec_digest,
            grid_identity,
            bucket_duration,
            evaluator,
        })
    }

    /// Frozen point-evaluator semantic version.
    pub const fn evaluator_version(self) -> u16 {
        self.evaluator_version
    }

    /// Frozen occupation-summary semantic version.
    pub const fn summary_version(self) -> u16 {
        self.summary_version
    }

    /// Opaque caller-supplied content identity for the canonical spec artifact.
    pub const fn spec_digest(self) -> [u8; SPEC_DIGEST_BYTES] {
        self.spec_digest
    }

    /// Opaque caller-supplied identity of the canonical equal-duration grid.
    pub const fn grid_identity(self) -> [u8; GRID_IDENTITY_BYTES] {
        self.grid_identity
    }

    /// Exact nonzero duration shared by every canonical bucket, in grid units.
    pub const fn bucket_duration(self) -> u64 {
        self.bucket_duration
    }

    /// Exact validated native basis spec used for every accepted point.
    pub const fn spec(self) -> BasisSpec {
        self.evaluator.spec()
    }

    fn validated_spec(self) -> Result<ValidatedBasisSpec> {
        if self.evaluator_version != BASIS_EVALUATOR_VERSION
            || self.summary_version != OCCUPATION_SUMMARY_VERSION
            || self.spec_digest == [0; SPEC_DIGEST_BYTES]
            || self.grid_identity == [0; GRID_IDENTITY_BYTES]
            || self.bucket_duration == 0
        {
            return Err(Error::InvalidDomain);
        }
        Ok(self.evaluator)
    }

    fn validate(self) -> Result<()> {
        self.validated_spec().map(|_| ())
    }
}

/// Explicit final-average policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalizationMode {
    /// Require every accumulated mass to divide by coverage exactly.
    ExactOnly,
    /// Floor each average, then award residual atoms to the largest
    /// remainders, with lowest outcome index winning exact ties.
    LargestRemainderV1,
}

/// Fixed-width payout-vector-like output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalWeights {
    active_len: u8,
    denominator: u64,
    weights: [u64; MAX_OUTCOMES],
}

impl FinalWeights {
    /// Number of active outcome weights.
    pub const fn active_len(self) -> u8 {
        self.active_len
    }

    /// Frozen common payout denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Active weights followed by canonical zero padding.
    pub const fn weights(self) -> [u64; MAX_OUTCOMES] {
        self.weights
    }

    /// Validate length, padding, component bounds, and exact partition unity.
    pub fn validate(self) -> Result<()> {
        let active = usize::from(self.active_len);
        if !(2..=MAX_OUTCOMES).contains(&active) || self.denominator == 0 {
            return Err(Error::InvalidSummary);
        }
        let mut sum = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < active {
                if weight > self.denominator {
                    return Err(Error::InvalidSummary);
                }
                sum = sum
                    .checked_add(u128::from(weight))
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if weight != 0 {
                return Err(Error::InvalidSummary);
            }
            index += 1;
        }
        if sum != u128::from(self.denominator) {
            return Err(Error::InvalidSummary);
        }
        Ok(())
    }
}

/// Fixed-width summary of canonical quantized basis occupation.
///
/// `sample_count` is the number of canonical buckets in the contiguous span;
/// `coverage_count` is the accepted subset. Therefore their difference is the
/// explicit gap count. Every accepted bucket has the same duration frozen in
/// [`BasisDomain`]; this crate does not read a clock or infer durations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Summary {
    domain: BasisDomain,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    sample_count: u64,
    coverage_count: u64,
    masses: [u128; MAX_OUTCOMES],
}

/// Validated append-only constructor for one contiguous occupation summary.
///
/// Construction validates the complete domain and captures a private
/// [`ValidatedBasisSpec`]. Each append can therefore evaluate a point and add
/// its exact vector without rebuilding and validating singleton summaries.
/// The fields are intentionally private: callers cannot forge the validated
/// precondition or mutate accumulated state around the checked append methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequentialSummaryBuilder {
    domain: BasisDomain,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    sample_count: u64,
    coverage_count: u64,
    masses: [u128; MAX_OUTCOMES],
}

impl SequentialSummaryBuilder {
    /// Validate one immutable domain and create an empty append-only builder.
    pub fn new(domain: BasisDomain) -> Result<Self> {
        domain.validate()?;
        Ok(Self {
            domain,
            start_bucket: 0,
            end_bucket_exclusive: 0,
            sample_count: 0,
            coverage_count: 0,
            masses: [0; MAX_OUTCOMES],
        })
    }

    /// Restore an append-only builder from one fully validated summary.
    ///
    /// This is the safe persistence boundary for resumable accumulation.  It
    /// revalidates every semantic field before copying state, so hostile or
    /// stale account bytes must first pass [`Summary::from_canonical_parts`]
    /// and cannot smuggle an impossible cursor, count, padding component, or
    /// accumulated partition mass into a later append.
    pub fn resume(summary: Summary) -> Result<Self> {
        summary.validate()?;
        Ok(Self {
            domain: summary.domain,
            start_bucket: summary.start_bucket,
            end_bucket_exclusive: summary.end_bucket_exclusive,
            sample_count: summary.sample_count,
            coverage_count: summary.coverage_count,
            masses: summary.masses,
        })
    }

    /// Append one accepted point at the next canonical bucket.
    ///
    /// The first bucket may have any representable index. Every later bucket
    /// must equal the prior exclusive end. Refusals leave the builder intact.
    pub fn append_accepted(&mut self, bucket: u64, point: u128) -> Result<()> {
        let end_bucket_exclusive = bucket.checked_add(1).ok_or(Error::BucketOverflow)?;
        self.require_next_bucket(bucket)?;
        let vector = self.domain.evaluator.evaluate_point(point)?;
        let sample_count = self
            .sample_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let coverage_count = self
            .coverage_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut masses = self.masses;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            masses[index] = masses[index]
                .checked_add(u128::from(vector.weights[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        if self.sample_count == 0 {
            self.start_bucket = bucket;
        }
        self.end_bucket_exclusive = end_bucket_exclusive;
        self.sample_count = sample_count;
        self.coverage_count = coverage_count;
        self.masses = masses;
        Ok(())
    }

    /// Append one explicit missing bucket at the next canonical position.
    ///
    /// Missing buckets advance the covered range and sample count but add no
    /// payout mass. Refusals leave the builder intact.
    pub fn append_missing(&mut self, bucket: u64) -> Result<()> {
        let end_bucket_exclusive = bucket.checked_add(1).ok_or(Error::BucketOverflow)?;
        self.require_next_bucket(bucket)?;
        let sample_count = self
            .sample_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.sample_count == 0 {
            self.start_bucket = bucket;
        }
        self.end_bucket_exclusive = end_bucket_exclusive;
        self.sample_count = sample_count;
        Ok(())
    }

    /// Consume the validated builder and return its canonical summary.
    pub fn finish(self) -> Summary {
        Summary {
            domain: self.domain,
            start_bucket: self.start_bucket,
            end_bucket_exclusive: self.end_bucket_exclusive,
            sample_count: self.sample_count,
            coverage_count: self.coverage_count,
            masses: self.masses,
        }
    }

    fn require_next_bucket(&self, bucket: u64) -> Result<()> {
        if self.sample_count != 0 && self.end_bucket_exclusive != bucket {
            return Err(Error::NonAdjacent);
        }
        Ok(())
    }
}

impl Summary {
    /// Domain-bound empty identity. Its zero range is not a real bucket span.
    pub fn empty(domain: BasisDomain) -> Result<Self> {
        domain.validate()?;
        Ok(Self {
            domain,
            start_bucket: 0,
            end_bucket_exclusive: 0,
            sample_count: 0,
            coverage_count: 0,
            masses: [0; MAX_OUTCOMES],
        })
    }

    /// Reconstruct a persisted summary from its complete canonical parts.
    ///
    /// No field is inferred or repaired. Empty state has exactly the unique
    /// zero representation; non-empty state must have an exact half-open span,
    /// accepted coverage no larger than the span, canonical inactive padding,
    /// and total active mass equal to `denominator * coverage_count`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_canonical_parts(
        domain: BasisDomain,
        start_bucket: u64,
        end_bucket_exclusive: u64,
        sample_count: u64,
        coverage_count: u64,
        masses: [u128; MAX_OUTCOMES],
    ) -> Result<Self> {
        let result = Self {
            domain,
            start_bucket,
            end_bucket_exclusive,
            sample_count,
            coverage_count,
            masses,
        };
        result.validate()?;
        Ok(result)
    }

    /// Construct one accepted canonical bucket from an authenticated point.
    ///
    /// Authentication and point/confidence admission must already have
    /// happened. This function only invokes the native basis evaluator.
    pub fn accepted(domain: BasisDomain, bucket: u64, point: u128) -> Result<Self> {
        domain.validate()?;
        let end = bucket.checked_add(1).ok_or(Error::BucketOverflow)?;
        let vector = domain.evaluator.evaluate_point(point)?;
        let mut masses = [0_u128; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            masses[index] = u128::from(vector.weights[index]);
            index += 1;
        }
        let result = Self {
            domain,
            start_bucket: bucket,
            end_bucket_exclusive: end,
            sample_count: 1,
            coverage_count: 1,
            masses,
        };
        result.validate()?;
        Ok(result)
    }

    /// Construct one explicit missing bucket.
    pub fn missing(domain: BasisDomain, bucket: u64) -> Result<Self> {
        domain.validate()?;
        let end = bucket.checked_add(1).ok_or(Error::BucketOverflow)?;
        let result = Self {
            domain,
            start_bucket: bucket,
            end_bucket_exclusive: end,
            sample_count: 1,
            coverage_count: 0,
            masses: [0; MAX_OUTCOMES],
        };
        result.validate()?;
        Ok(result)
    }

    /// Exact basis identity shared by every bucket in this summary.
    pub const fn domain(self) -> BasisDomain {
        self.domain
    }

    /// Inclusive first bucket, or zero for the empty identity.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Exclusive final bucket, or zero for the empty identity.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }

    /// Number of canonical buckets, accepted plus explicit gaps.
    pub const fn sample_count(self) -> u64 {
        self.sample_count
    }

    /// Number of accepted canonical buckets.
    pub const fn coverage_count(self) -> u64 {
        self.coverage_count
    }

    /// Number of explicit missing buckets.
    pub const fn gap_count(self) -> u64 {
        self.sample_count - self.coverage_count
    }

    /// Per-outcome sums of canonical quantized native weights.
    pub const fn masses(self) -> [u128; MAX_OUTCOMES] {
        self.masses
    }

    /// Whether this is the unique domain-bound empty identity.
    pub const fn is_empty(self) -> bool {
        self.sample_count == 0
    }

    /// Validate range/count agreement, padding, and exact accumulated unity.
    pub fn validate(self) -> Result<()> {
        self.domain.validate()?;
        if self.sample_count == 0 {
            if self.start_bucket != 0
                || self.end_bucket_exclusive != 0
                || self.coverage_count != 0
                || self.masses != [0; MAX_OUTCOMES]
            {
                return Err(Error::InvalidSummary);
            }
            return Ok(());
        }
        let span = self
            .end_bucket_exclusive
            .checked_sub(self.start_bucket)
            .ok_or(Error::InvalidSummary)?;
        if span != self.sample_count || self.coverage_count > self.sample_count {
            return Err(Error::InvalidSummary);
        }
        let spec = self.domain.spec();
        let active = usize::from(spec.outcome_count);
        let mut mass_sum = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let mass = self.masses[index];
            if index < active {
                mass_sum = mass_sum
                    .checked_add(mass)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if mass != 0 {
                return Err(Error::InvalidSummary);
            }
            index += 1;
        }
        let expected = u128::from(spec.denominator)
            .checked_mul(u128::from(self.coverage_count))
            .ok_or(Error::ArithmeticOverflow)?;
        if mass_sum != expected {
            return Err(Error::InvalidSummary);
        }
        Ok(())
    }

    /// Combine equal-domain adjacent ranges with checked componentwise addition.
    pub fn combine(self, rhs: Self) -> Result<Self> {
        self.validate()?;
        rhs.validate()?;
        if self.domain != rhs.domain {
            return Err(Error::DomainMismatch);
        }
        if self.is_empty() {
            return Ok(rhs);
        }
        if rhs.is_empty() {
            return Ok(self);
        }
        if self.end_bucket_exclusive != rhs.start_bucket {
            return Err(Error::NonAdjacent);
        }
        let sample_count = self
            .sample_count
            .checked_add(rhs.sample_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let coverage_count = self
            .coverage_count
            .checked_add(rhs.coverage_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut masses = [0_u128; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            masses[index] = self.masses[index]
                .checked_add(rhs.masses[index])
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        let result = Self {
            domain: self.domain,
            start_bucket: self.start_bucket,
            end_bucket_exclusive: rhs.end_bucket_exclusive,
            sample_count,
            coverage_count,
            masses,
        };
        result.validate()?;
        Ok(result)
    }

    /// Finalize a complete non-empty span under one explicit average rule.
    pub fn finalize(self, mode: FinalizationMode) -> Result<FinalWeights> {
        self.validate()?;
        if self.coverage_count == 0 {
            return Err(Error::NoCoverage);
        }
        if self.coverage_count != self.sample_count {
            return Err(Error::IncompleteCoverage);
        }
        let divisor = u128::from(self.coverage_count);
        let spec = self.domain.spec();
        let active = usize::from(spec.outcome_count);
        let mut weights = [0_u64; MAX_OUTCOMES];
        let mut remainders = [0_u128; MAX_OUTCOMES];
        let mut floor_sum = 0_u64;
        let mut index = 0_usize;
        while index < active {
            let floor = self.masses[index] / divisor;
            let weight = u64::try_from(floor).map_err(|_| Error::ArithmeticOverflow)?;
            weights[index] = weight;
            remainders[index] = self.masses[index] % divisor;
            floor_sum = floor_sum
                .checked_add(weight)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }

        let denominator = spec.denominator;
        let residual = denominator
            .checked_sub(floor_sum)
            .ok_or(Error::InvalidSummary)?;
        match mode {
            FinalizationMode::ExactOnly => {
                if residual != 0 || remainders[..active].iter().any(|value| *value != 0) {
                    return Err(Error::InexactAverage);
                }
            }
            FinalizationMode::LargestRemainderV1 => {
                if residual > u64::from(spec.outcome_count - 1) {
                    return Err(Error::InvalidSummary);
                }
                let mut awarded = [false; MAX_OUTCOMES];
                let mut remaining = residual;
                while remaining > 0 {
                    let mut best: Option<usize> = None;
                    let mut candidate = 0_usize;
                    while candidate < active {
                        if !awarded[candidate] && remainders[candidate] != 0 {
                            let replace = match best {
                                None => true,
                                Some(current) => remainders[candidate] > remainders[current],
                            };
                            if replace {
                                best = Some(candidate);
                            }
                        }
                        candidate += 1;
                    }
                    let selected = best.ok_or(Error::InvalidSummary)?;
                    weights[selected] = weights[selected]
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                    awarded[selected] = true;
                    remaining -= 1;
                }
            }
        }

        let output = FinalWeights {
            active_len: spec.outcome_count,
            denominator,
            weights,
        };
        output.validate()?;
        Ok(output)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_bspline::{EdgePolicy, UNIFORM_SPACING_NONE};
    use std::{vec, vec::Vec};

    fn knots(values: &[u128]) -> [u128; clutch_bspline::MAX_KNOTS] {
        let mut result = [0; clutch_bspline::MAX_KNOTS];
        result[..values.len()].copy_from_slice(values);
        result
    }

    fn spec(degree: u8, denominator: u64) -> BasisSpec {
        match degree {
            0 => BasisSpec {
                outcome_count: 3,
                degree,
                knot_count: 2,
                uniform_log2_spacing: UNIFORM_SPACING_NONE,
                denominator,
                domain_max: 24,
                edge_policy: EdgePolicy::Clamp,
                knots: knots(&[8, 16]),
            },
            1 => BasisSpec {
                outcome_count: 3,
                degree,
                knot_count: 3,
                uniform_log2_spacing: 3,
                denominator,
                domain_max: 16,
                edge_policy: EdgePolicy::Clamp,
                knots: knots(&[0, 8, 16]),
            },
            2 | 3 => BasisSpec {
                outcome_count: 2 + degree,
                degree,
                knot_count: 3,
                uniform_log2_spacing: 3,
                denominator,
                domain_max: 16,
                edge_policy: EdgePolicy::Clamp,
                knots: knots(&[0, 8, 16]),
            },
            _ => unreachable!(),
        }
    }

    fn domain(degree: u8, denominator: u64) -> BasisDomain {
        let mut digest = [degree + 1; SPEC_DIGEST_BYTES];
        digest[31] = denominator as u8;
        BasisDomain::new(
            digest,
            [0x47; GRID_IDENTITY_BYTES],
            60,
            spec(degree, denominator),
        )
        .unwrap()
    }

    fn fold(domain: BasisDomain, start: u64, points: &[Option<u128>]) -> Summary {
        let mut result = Summary::empty(domain).unwrap();
        for (offset, point) in points.iter().enumerate() {
            let bucket = start + offset as u64;
            let item = match point {
                Some(value) => Summary::accepted(domain, bucket, *value).unwrap(),
                None => Summary::missing(domain, bucket).unwrap(),
            };
            result = result.combine(item).unwrap();
        }
        result
    }

    fn sequential_fold(domain: BasisDomain, start: u64, points: &[Option<u128>]) -> Summary {
        let mut builder = SequentialSummaryBuilder::new(domain).unwrap();
        for (offset, point) in points.iter().enumerate() {
            let bucket = start + offset as u64;
            match point {
                Some(value) => builder.append_accepted(bucket, *value).unwrap(),
                None => builder.append_missing(bucket).unwrap(),
            }
        }
        builder.finish()
    }

    fn every_parenthesization(
        domain: BasisDomain,
        start: u64,
        points: &[Option<u128>],
    ) -> Vec<Summary> {
        if points.len() == 1 {
            return vec![fold(domain, start, points)];
        }
        let mut results = Vec::new();
        for split in 1..points.len() {
            let left = every_parenthesization(domain, start, &points[..split]);
            let right = every_parenthesization(domain, start + split as u64, &points[split..]);
            for lhs in &left {
                for rhs in &right {
                    results.push(lhs.combine(*rhs).unwrap());
                }
            }
        }
        results
    }

    #[test]
    fn sequential_append_matches_every_generic_parenthesization() {
        let alphabet = [None, Some(0), Some(7), Some(16)];
        for degree in 0..=3 {
            let domain = domain(degree, 257);
            for a in alphabet {
                for b in alphabet {
                    for c in alphabet {
                        for d in alphabet {
                            let points = [a, b, c, d];
                            let sequential = sequential_fold(domain, 37, &points);
                            sequential.validate().unwrap();
                            for generic in every_parenthesization(domain, 37, &points) {
                                assert_eq!(sequential, generic);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn persisted_resume_matches_uninterrupted_and_every_parenthesization() {
        let alphabet = [None, Some(0), Some(7), Some(16)];
        for degree in 0..=3 {
            let domain = domain(degree, 257);
            for a in alphabet {
                for b in alphabet {
                    for c in alphabet {
                        for d in alphabet {
                            let points = [a, b, c, d];
                            let mut first = SequentialSummaryBuilder::new(domain).unwrap();
                            for (offset, point) in points[..2].iter().enumerate() {
                                let bucket = 37 + offset as u64;
                                match point {
                                    Some(value) => {
                                        first.append_accepted(bucket, *value).unwrap();
                                    }
                                    None => first.append_missing(bucket).unwrap(),
                                }
                            }
                            let persisted = first.finish();
                            let restored = Summary::from_canonical_parts(
                                persisted.domain(),
                                persisted.start_bucket(),
                                persisted.end_bucket_exclusive(),
                                persisted.sample_count(),
                                persisted.coverage_count(),
                                persisted.masses(),
                            )
                            .unwrap();
                            let mut resumed = SequentialSummaryBuilder::resume(restored).unwrap();
                            for (offset, point) in points[2..].iter().enumerate() {
                                let bucket = 39 + offset as u64;
                                match point {
                                    Some(value) => {
                                        resumed.append_accepted(bucket, *value).unwrap();
                                    }
                                    None => resumed.append_missing(bucket).unwrap(),
                                }
                            }
                            let resumed = resumed.finish();
                            assert_eq!(resumed, sequential_fold(domain, 37, &points));
                            for generic in every_parenthesization(domain, 37, &points) {
                                assert_eq!(resumed, generic);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn resume_accepts_only_the_unique_empty_representation() {
        let domain = domain(2, 257);
        let empty = Summary::from_canonical_parts(domain, 0, 0, 0, 0, [0; MAX_OUTCOMES]).unwrap();
        let mut resumed = SequentialSummaryBuilder::resume(empty).unwrap();
        resumed.append_accepted(91, 7).unwrap();
        assert_eq!(resumed.finish(), Summary::accepted(domain, 91, 7).unwrap());

        let mut nonzero_mass = [0; MAX_OUTCOMES];
        nonzero_mass[0] = 1;
        for invalid in [
            Summary::from_canonical_parts(domain, 1, 1, 0, 0, [0; MAX_OUTCOMES]),
            Summary::from_canonical_parts(domain, 0, 1, 0, 0, [0; MAX_OUTCOMES]),
            Summary::from_canonical_parts(domain, 0, 0, 0, 1, [0; MAX_OUTCOMES]),
            Summary::from_canonical_parts(domain, 0, 0, 0, 0, nonzero_mass),
        ] {
            assert_eq!(invalid, Err(Error::InvalidSummary));
        }
    }

    #[test]
    fn resume_rejects_hostile_range_count_mass_and_padding_parts() {
        let domain = domain(2, 257);
        let valid = sequential_fold(domain, 40, &[Some(0), None, Some(7)]);
        let masses = valid.masses();
        assert_eq!(
            Summary::from_canonical_parts(domain, 43, 40, 3, 2, masses),
            Err(Error::InvalidSummary)
        );
        assert_eq!(
            Summary::from_canonical_parts(domain, 40, 44, 3, 2, masses),
            Err(Error::InvalidSummary)
        );
        assert_eq!(
            Summary::from_canonical_parts(domain, 40, 43, 3, 4, masses),
            Err(Error::InvalidSummary)
        );

        let mut wrong_mass = masses;
        wrong_mass[0] += 1;
        assert_eq!(
            Summary::from_canonical_parts(domain, 40, 43, 3, 2, wrong_mass),
            Err(Error::InvalidSummary)
        );

        let mut noncanonical_padding = masses;
        noncanonical_padding[usize::from(domain.spec().outcome_count)] = 257;
        assert_eq!(
            Summary::from_canonical_parts(domain, 40, 43, 3, 2, noncanonical_padding),
            Err(Error::InvalidSummary)
        );

        let mut overflowing = [0; MAX_OUTCOMES];
        overflowing[0] = u128::MAX;
        overflowing[1] = 1;
        assert_eq!(
            Summary::from_canonical_parts(domain, 40, 41, 1, 1, overflowing),
            Err(Error::ArithmeticOverflow)
        );
    }

    #[test]
    fn resumed_append_refusals_are_atomic_and_finalization_is_identical() {
        let domain = domain(3, 257);
        let prefix = sequential_fold(domain, 9, &[Some(0), Some(4)]);
        let mut resumed = SequentialSummaryBuilder::resume(prefix).unwrap();
        let before = resumed.clone();
        assert_eq!(resumed.append_missing(12), Err(Error::NonAdjacent));
        assert_eq!(resumed, before);
        assert_eq!(
            resumed.append_accepted(u64::MAX, 4),
            Err(Error::BucketOverflow)
        );
        assert_eq!(resumed, before);

        resumed.append_accepted(11, 16).unwrap();
        let resumed = resumed.finish();
        let uninterrupted = sequential_fold(domain, 9, &[Some(0), Some(4), Some(16)]);
        assert_eq!(resumed, uninterrupted);
        for mode in [
            FinalizationMode::ExactOnly,
            FinalizationMode::LargestRemainderV1,
        ] {
            assert_eq!(resumed.finalize(mode), uninterrupted.finalize(mode));
        }
    }

    #[test]
    fn sequential_refusals_match_generic_and_are_atomic() {
        let domain = domain(3, 257);
        let mut builder = SequentialSummaryBuilder::new(domain).unwrap();
        builder.append_accepted(9, 4).unwrap();

        let before = builder.clone();
        let generic_nonadjacent = Summary::accepted(domain, 9, 4)
            .unwrap()
            .combine(Summary::missing(domain, 11).unwrap());
        assert_eq!(builder.append_missing(11), generic_nonadjacent.map(|_| ()));
        assert_eq!(builder, before);
        assert_eq!(
            builder.append_accepted(u64::MAX, 4),
            Err(Error::BucketOverflow)
        );
        assert_eq!(builder, before);
        assert_eq!(
            Summary::accepted(domain, u64::MAX, 4),
            Err(Error::BucketOverflow)
        );
        assert_eq!(
            Summary::missing(domain, u64::MAX),
            Err(Error::BucketOverflow)
        );

        let mut refusing_spec = spec(3, 257);
        refusing_spec.edge_policy = EdgePolicy::Refuse;
        let refusing = BasisDomain::new(
            [0x31; SPEC_DIGEST_BYTES],
            [0x47; GRID_IDENTITY_BYTES],
            60,
            refusing_spec,
        )
        .unwrap();
        let mut refusing_builder = SequentialSummaryBuilder::new(refusing).unwrap();
        let refusing_before = refusing_builder.clone();
        assert_eq!(
            refusing_builder.append_accepted(0, 17),
            Err(Error::Basis(BasisError::ValueOutOfRange))
        );
        assert_eq!(refusing_builder, refusing_before);
        assert_eq!(
            Summary::accepted(refusing, 0, 17),
            Err(Error::Basis(BasisError::ValueOutOfRange))
        );
    }

    #[test]
    fn sequential_finish_preserves_domain_mismatch_and_gap_semantics() {
        let first = domain(2, 7);
        let second = domain(2, 8);
        let first_summary = sequential_fold(first, 4, &[Some(1), None, Some(9)]);
        let second_summary = sequential_fold(second, 7, &[Some(1)]);
        assert_eq!(first_summary.gap_count(), 1);
        assert_eq!(
            first_summary.finalize(FinalizationMode::LargestRemainderV1),
            Err(Error::IncompleteCoverage)
        );
        assert_eq!(
            first_summary.combine(second_summary),
            Err(Error::DomainMismatch)
        );
    }

    #[test]
    fn every_degree_accumulates_endpoints_with_exact_partition_mass() {
        for degree in 0..=3 {
            let domain = domain(degree, 257);
            let summary = fold(domain, 10, &[Some(0), Some(8), Some(16), Some(24)]);
            assert_eq!(summary.sample_count(), 4);
            assert_eq!(summary.coverage_count(), 4);
            assert_eq!(summary.gap_count(), 0);
            assert_eq!(
                summary.masses().iter().sum::<u128>(),
                4 * u128::from(domain.spec().denominator)
            );
            summary.validate().unwrap();
            summary
                .finalize(FinalizationMode::LargestRemainderV1)
                .unwrap()
                .validate()
                .unwrap();
        }
    }

    #[test]
    fn endpoint_singletons_preserve_native_closed_edge_vectors() {
        for degree in 0..=3 {
            let domain = domain(degree, 257);
            let low_point = if degree == 0 {
                0
            } else {
                domain.spec().knots[0]
            };
            let high_point = if degree == 0 {
                domain.spec().domain_max
            } else {
                domain.spec().knots[usize::from(domain.spec().knot_count) - 1]
            };
            let low = Summary::accepted(domain, 0, low_point)
                .unwrap()
                .finalize(FinalizationMode::ExactOnly)
                .unwrap();
            let high = Summary::accepted(domain, 0, high_point)
                .unwrap()
                .finalize(FinalizationMode::ExactOnly)
                .unwrap();
            assert_eq!(low.weights()[0], 257);
            assert_eq!(
                high.weights()[usize::from(domain.spec().outcome_count) - 1],
                257
            );
        }
    }

    #[test]
    fn combine_is_associative_for_accepted_and_missing_buckets() {
        for degree in 0..=3 {
            let domain = domain(degree, 31);
            let a = fold(domain, 100, &[Some(0), None, Some(3)]);
            let b = fold(domain, 103, &[Some(7), Some(11)]);
            let c = fold(domain, 105, &[None, Some(16), Some(2)]);
            assert_eq!(
                a.combine(b).unwrap().combine(c).unwrap(),
                a.combine(b.combine(c).unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn associativity_is_exhaustive_over_small_degree_zero_to_three_alphabet() {
        let alphabet = [None, Some(0), Some(7), Some(16)];
        for degree in 0..=3 {
            let domain = domain(degree, 17);
            for left in alphabet {
                for middle in alphabet {
                    for right in alphabet {
                        let a = fold(domain, 50, &[left]);
                        let b = fold(domain, 51, &[middle]);
                        let c = fold(domain, 52, &[right]);
                        assert_eq!(
                            a.combine(b).unwrap().combine(c).unwrap(),
                            a.combine(b.combine(c).unwrap()).unwrap()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn exhaustive_parenthesizations_match_on_small_streams() {
        let domain = domain(3, 17);
        let values = [Some(0), Some(3), None, Some(9), Some(16)];
        let whole = fold(domain, 20, &values);
        for left_end in 1..values.len() {
            let left = fold(domain, 20, &values[..left_end]);
            let right = fold(domain, 20 + left_end as u64, &values[left_end..]);
            assert_eq!(left.combine(right).unwrap(), whole);
        }
    }

    #[test]
    fn empty_is_identity_but_wrong_domain_still_refuses() {
        let first = domain(2, 7);
        let second = domain(2, 8);
        let item = Summary::accepted(first, 4, 3).unwrap();
        assert_eq!(Summary::empty(first).unwrap().combine(item).unwrap(), item);
        assert_eq!(item.combine(Summary::empty(first).unwrap()).unwrap(), item);
        assert_eq!(
            Summary::empty(second).unwrap().combine(item),
            Err(Error::DomainMismatch)
        );
    }

    #[test]
    fn overlaps_reverse_order_and_holes_between_summaries_refuse() {
        let domain = domain(2, 7);
        let a = Summary::accepted(domain, 5, 3).unwrap();
        let overlap = Summary::accepted(domain, 5, 4).unwrap();
        let late = Summary::accepted(domain, 7, 4).unwrap();
        assert_eq!(a.combine(overlap), Err(Error::NonAdjacent));
        assert_eq!(late.combine(a), Err(Error::NonAdjacent));
        assert_eq!(a.combine(late), Err(Error::NonAdjacent));
    }

    #[test]
    fn explicit_gap_is_counted_and_both_finalize_modes_refuse_it() {
        let domain = domain(2, 7);
        let summary = fold(domain, 0, &[Some(0), None, Some(4)]);
        assert_eq!(summary.sample_count(), 3);
        assert_eq!(summary.coverage_count(), 2);
        assert_eq!(summary.gap_count(), 1);
        assert_eq!(
            summary.finalize(FinalizationMode::ExactOnly),
            Err(Error::IncompleteCoverage)
        );
        assert_eq!(
            summary.finalize(FinalizationMode::LargestRemainderV1),
            Err(Error::IncompleteCoverage)
        );
    }

    #[test]
    fn no_coverage_is_distinct_from_incomplete_coverage() {
        let domain = domain(3, 7);
        let summary = fold(domain, 0, &[None, None]);
        assert_eq!(
            summary.finalize(FinalizationMode::LargestRemainderV1),
            Err(Error::NoCoverage)
        );
    }

    #[test]
    fn exact_mode_accepts_representable_average() {
        let domain = domain(3, 31);
        let summary = fold(domain, 0, &[Some(9), Some(9), Some(9)]);
        let output = summary.finalize(FinalizationMode::ExactOnly).unwrap();
        let point = domain.spec().evaluate(9).unwrap();
        assert_eq!(output.weights(), point.weights);
        assert_eq!(output.denominator(), 31);
    }

    #[test]
    fn largest_remainder_mode_is_separate_and_deterministic() {
        // Exact research fixture: quadratic knots (0,8,16), D=7, path 0 then 4.
        let domain = domain(2, 7);
        let summary = fold(domain, 0, &[Some(0), Some(4)]);
        assert_eq!(
            summary.finalize(FinalizationMode::ExactOnly),
            Err(Error::InexactAverage)
        );
        let output = summary
            .finalize(FinalizationMode::LargestRemainderV1)
            .unwrap();
        assert_eq!(&output.weights()[..4], &[5, 2, 0, 0]);
        output.validate().unwrap();
    }

    #[test]
    fn largest_remainder_exact_tie_chooses_lowest_index() {
        let domain = domain(0, 1);
        let summary = fold(domain, 0, &[Some(0), Some(8)]);
        let output = summary
            .finalize(FinalizationMode::LargestRemainderV1)
            .unwrap();
        assert_eq!(&output.weights()[..3], &[1, 0, 0]);
    }

    #[test]
    fn digest_and_exact_spec_both_participate_in_domain_identity() {
        let original = domain(2, 7);
        let mut other_digest = original.spec_digest();
        other_digest[0] ^= 0x80;
        let digest_changed = BasisDomain::new(
            other_digest,
            original.grid_identity(),
            original.bucket_duration(),
            original.spec(),
        )
        .unwrap();
        let spec_changed = domain(2, 8);
        let grid_changed = BasisDomain::new(
            original.spec_digest(),
            [0x48; GRID_IDENTITY_BYTES],
            original.bucket_duration(),
            original.spec(),
        )
        .unwrap();
        let duration_changed = BasisDomain::new(
            original.spec_digest(),
            original.grid_identity(),
            original.bucket_duration() + 1,
            original.spec(),
        )
        .unwrap();
        let item = Summary::accepted(original, 0, 4).unwrap();
        assert_eq!(
            item.combine(Summary::accepted(digest_changed, 1, 4).unwrap()),
            Err(Error::DomainMismatch)
        );
        assert_eq!(
            item.combine(Summary::accepted(spec_changed, 1, 4).unwrap()),
            Err(Error::DomainMismatch)
        );
        assert_eq!(
            item.combine(Summary::accepted(grid_changed, 1, 4).unwrap()),
            Err(Error::DomainMismatch)
        );
        assert_eq!(
            item.combine(Summary::accepted(duration_changed, 1, 4).unwrap()),
            Err(Error::DomainMismatch)
        );
    }

    #[test]
    fn zero_digest_and_basis_arithmetic_overflow_refuse_at_domain_admission() {
        assert_eq!(
            BasisDomain::new(
                [0; SPEC_DIGEST_BYTES],
                [1; GRID_IDENTITY_BYTES],
                60,
                spec(2, 7),
            ),
            Err(Error::InvalidDomain)
        );
        assert_eq!(
            BasisDomain::new(
                [1; SPEC_DIGEST_BYTES],
                [0; GRID_IDENTITY_BYTES],
                60,
                spec(2, 7),
            ),
            Err(Error::InvalidDomain)
        );
        assert_eq!(
            BasisDomain::new(
                [1; SPEC_DIGEST_BYTES],
                [1; GRID_IDENTITY_BYTES],
                0,
                spec(2, 7),
            ),
            Err(Error::InvalidDomain)
        );
        let mut hostile = spec(3, u64::MAX);
        hostile.uniform_log2_spacing = 127;
        hostile.knots = knots(&[0, 1_u128 << 127]);
        hostile.knot_count = 2;
        hostile.outcome_count = 4;
        hostile.domain_max = 1_u128 << 127;
        assert!(matches!(
            BasisDomain::new(
                [1; SPEC_DIGEST_BYTES],
                [1; GRID_IDENTITY_BYTES],
                60,
                hostile,
            ),
            Err(Error::Basis(BasisError::ArithmeticBound))
        ));
    }

    #[test]
    fn maximum_bucket_refuses_instead_of_wrapping() {
        let domain = domain(1, 7);
        assert_eq!(
            Summary::accepted(domain, u64::MAX, 4),
            Err(Error::BucketOverflow)
        );
        assert_eq!(
            Summary::missing(domain, u64::MAX),
            Err(Error::BucketOverflow)
        );
    }

    #[test]
    fn corrupted_mass_count_padding_and_range_mutants_are_killed() {
        let domain = domain(2, 7);
        let valid = fold(domain, 5, &[Some(0), Some(4)]);

        let mut mass = valid;
        mass.masses[0] += 1;
        assert_eq!(mass.validate(), Err(Error::InvalidSummary));

        let mut overflowing_mass_sum = valid;
        overflowing_mass_sum.masses = [0; MAX_OUTCOMES];
        overflowing_mass_sum.masses[0] = u128::MAX;
        overflowing_mass_sum.masses[1] = 1;
        assert_eq!(
            overflowing_mass_sum.validate(),
            Err(Error::ArithmeticOverflow)
        );

        let mut padding = valid;
        padding.masses[MAX_OUTCOMES - 1] = 1;
        assert_eq!(padding.validate(), Err(Error::InvalidSummary));

        let mut count = valid;
        count.coverage_count = 3;
        assert_eq!(count.validate(), Err(Error::InvalidSummary));

        let mut range = valid;
        range.end_bucket_exclusive += 1;
        assert_eq!(range.validate(), Err(Error::InvalidSummary));

        let mut version = valid;
        version.domain.summary_version += 1;
        assert_eq!(version.validate(), Err(Error::InvalidDomain));
    }

    #[test]
    fn weight_output_mutants_are_killed() {
        let domain = domain(1, 7);
        let valid = fold(domain, 0, &[Some(4)])
            .finalize(FinalizationMode::ExactOnly)
            .unwrap();
        valid.validate().unwrap();

        let mut wrong_sum = valid;
        wrong_sum.weights[0] += 1;
        assert_eq!(wrong_sum.validate(), Err(Error::InvalidSummary));

        let mut padding = valid;
        padding.weights[MAX_OUTCOMES - 1] = 1;
        assert_eq!(padding.validate(), Err(Error::InvalidSummary));
    }

    #[test]
    fn summary_and_domain_are_fixed_width_and_allocation_free() {
        assert!(core::mem::size_of::<BasisDomain>() > core::mem::size_of::<BasisSpec>());
        assert!(core::mem::size_of::<Summary>() >= 16 * core::mem::size_of::<u128>());
        assert_eq!(core::mem::size_of::<FinalWeights>(), 144);
    }

    #[test]
    fn imported_weight_vector_shape_matches_final_output_contract() {
        let domain = domain(3, 257);
        let source: clutch_bspline::WeightVector = domain.spec().evaluate(7).unwrap();
        let output = Summary::accepted(domain, 0, 7)
            .unwrap()
            .finalize(FinalizationMode::ExactOnly)
            .unwrap();
        assert_eq!(source.active_len, output.active_len());
        assert_eq!(source.denominator, output.denominator());
        assert_eq!(source.weights, output.weights());
    }
}
