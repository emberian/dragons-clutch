#![no_std]
#![deny(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! A bounded, source-neutral interval-summary monoid.
//!
//! This crate deliberately owns semantic summaries only. It has no source
//! adapter, account parser, RPC client, clock policy, chain code, allocator,
//! serialization dependency, or deployment assumption. An adapter must first
//! authenticate an observation and then pass the already-normalized result to
//! [`Summary::singleton`].
//!
//! # Algebra
//!
//! A summary covers a non-empty contiguous half-open bucket range. A missing
//! bucket is a real member of that range, not an omitted value. `combine` is
//! defined only for summaries with the same [`Grid`] and adjacent ranges;
//! otherwise it returns an explicit error. [`Summary::empty`] is the identity
//! for a particular grid. All arithmetic is checked and all accepted values
//! are bounded by [`MAX_VALUE`].
//!
//! The associative fields are coverage, first/last accepted intervals,
//! extrema, and exact lower/upper time-integral numerators. The integral is
//! exact because V1 fixes one duration for every bucket in a grid; the result
//! is retained as an integer numerator rather than rounded to a midpoint.
//!
//! # Information-theoretic boundary
//!
//! This summary intentionally discards observation order except for first and
//! last accepted intervals. It cannot answer arbitrary threshold predicates,
//! crossing counts, run lengths, path-dependent drawdown, realized variance,
//! or reconstruct discarded observations. Those methods must return a
//! conservative refusal until a separately registered feature family carries
//! the required information. Equal summaries can represent streams with
//! different threshold-crossing topology; see the law tests for a concrete
//! witness.
//!
//! # Domain-bound window evidence
//!
//! A [`Summary`] is a statistic, not an authority. It answers for whatever
//! bucket range it happens to cover, complete or gapped, which is correct for
//! an accumulator and wrong for a settlement term. The window plane
//! ([`WindowDomain`], [`WindowAccumulator`], [`WindowResult`]) is the typed
//! join that fixes the composition hole recorded as §P1-D of
//! `docs/implementation/ADVERSARIAL_REVIEW_V0.md`: a [`WindowResult`] exists
//! only for an exact expected feed/grid/range/generation/maturity domain that
//! reached its maturity bound, was sealed, and satisfied a registered
//! [`CoveragePolicy`]. There is no constructor from a bare [`Summary`], so a
//! function that names `&WindowResult` refuses a prefix, a gapped substitute,
//! or another window at compile time or by explicit error.
//!
//! Nothing here authenticates anything. This crate has no source adapter, no
//! hash primitive, and no clock. `WindowResult` is honest evidence about a
//! fold, never evidence about a source.

mod window;

pub use window::{
    CoveragePolicy, FeedIdentity, WindowAccumulator, WindowDomain, WindowError, WindowPhase,
    WindowResult, COVERAGE_POLICY_BOUNDED_GAPS, COVERAGE_POLICY_COMPLETE_REQUIRED, IDENTITY_BYTES,
    WINDOW_DOMAIN_BYTES, WINDOW_DOMAIN_MAGIC, WINDOW_DOMAIN_RESERVED_BYTES,
    WINDOW_DOMAIN_SCHEMA_VERSION, WINDOW_DOMAIN_TAG,
};

/// Maximum number of buckets in one summary.
pub const MAX_BUCKETS: u64 = 1_000_000;
/// Maximum duration of one grid bucket, in the grid's declared time unit.
pub const MAX_BUCKET_SECONDS: u64 = 86_400;
/// Maximum normalized endpoint value admitted by this prototype.
pub const MAX_VALUE: u128 = 1_000_000_000_000_000_000_000_000;

/// A semantic versioned grid identity and its exact bucket duration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grid {
    family_id: u32,
    version: u16,
    bucket_seconds: u64,
}

impl Grid {
    /// Construct a grid. `family_id` and `version` are semantic identifiers;
    /// `bucket_seconds` is not a wall-clock timestamp and must be non-zero.
    pub const fn new(
        family_id: u32,
        version: u16,
        bucket_seconds: u64,
    ) -> Result<Self, SummaryError> {
        if bucket_seconds == 0 || bucket_seconds > MAX_BUCKET_SECONDS {
            return Err(SummaryError::InvalidGrid);
        }
        Ok(Self {
            family_id,
            version,
            bucket_seconds,
        })
    }

    /// Semantic family identifier.
    pub const fn family_id(self) -> u32 {
        self.family_id
    }

    /// Version of the frozen summary/statistic family.
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Exact duration of one bucket.
    pub const fn bucket_seconds(self) -> u64 {
        self.bucket_seconds
    }
}

/// A conservative inclusive interval for one normalized observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueInterval {
    low: u128,
    high: u128,
}

impl ValueInterval {
    /// Construct an interval, rejecting reversed or out-of-range endpoints.
    pub const fn new(low: u128, high: u128) -> Result<Self, SummaryError> {
        if low > high || high > MAX_VALUE {
            return Err(SummaryError::InvalidObservation);
        }
        Ok(Self { low, high })
    }

    /// Lower endpoint.
    pub const fn low(self) -> u128 {
        self.low
    }

    /// Upper endpoint.
    pub const fn high(self) -> u128 {
        self.high
    }
}

/// Whether one bucket carries an accepted interval or an explicit gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observation {
    /// A source-authenticated interval for one bucket.
    Accepted {
        /// Canonical integer bucket index.
        bucket: u64,
        /// Conservative accepted value interval.
        value: ValueInterval,
    },
    /// An explicit source gap for one bucket.
    Missing {
        /// Canonical integer bucket index.
        bucket: u64,
    },
}

impl Observation {
    /// Construct an accepted observation. Endpoint validation is repeated by
    /// the summary constructor so this value may safely cross an adapter API.
    pub const fn accepted(bucket: u64, low: u128, high: u128) -> Self {
        Self::Accepted {
            bucket,
            value: ValueInterval { low, high },
        }
    }

    /// Construct an explicit missing bucket.
    pub const fn missing(bucket: u64) -> Self {
        Self::Missing { bucket }
    }

    /// Bucket index.
    pub const fn bucket(self) -> u64 {
        match self {
            Self::Accepted { bucket, .. } | Self::Missing { bucket } => bucket,
        }
    }
}

/// Coverage state carried by a summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageState {
    /// The identity summary, with no bucket range.
    Empty,
    /// Every bucket in the range was accepted.
    Complete,
    /// At least one bucket in the range is explicitly missing.
    Gapped,
}

/// A checked exact ratio interval. Both endpoints share `denominator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RatioInterval {
    numerator_low: u128,
    numerator_high: u128,
    denominator: u128,
}

impl RatioInterval {
    /// Lower numerator before any caller-selected presentation rounding.
    pub const fn numerator_low(self) -> u128 {
        self.numerator_low
    }

    /// Upper numerator before any caller-selected presentation rounding.
    pub const fn numerator_high(self) -> u128 {
        self.numerator_high
    }

    /// Common positive denominator.
    pub const fn denominator(self) -> u128 {
        self.denominator
    }
}

/// An interval whose endpoint fractions may require different denominators.
/// This is used for ratios of two already conservative intervals; callers
/// choose any presentation rounding only after inspecting both exact pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionInterval {
    low_numerator: u128,
    low_denominator: u128,
    high_numerator: u128,
    high_denominator: u128,
}

impl FractionInterval {
    /// Exact lower endpoint numerator.
    pub const fn low_numerator(self) -> u128 {
        self.low_numerator
    }

    /// Exact lower endpoint denominator.
    pub const fn low_denominator(self) -> u128 {
        self.low_denominator
    }

    /// Exact upper endpoint numerator.
    pub const fn high_numerator(self) -> u128 {
        self.high_numerator
    }

    /// Exact upper endpoint denominator.
    pub const fn high_denominator(self) -> u128 {
        self.high_denominator
    }
}

/// Coverage and duration counters exposed without exposing mutable fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Coverage {
    total_buckets: u64,
    accepted_buckets: u64,
    missing_buckets: u64,
    span_duration: u64,
    covered_duration: u64,
    state: CoverageState,
}

impl Coverage {
    /// Number of buckets in the summary range.
    pub const fn total_buckets(self) -> u64 {
        self.total_buckets
    }

    /// Number of accepted buckets.
    pub const fn accepted_buckets(self) -> u64 {
        self.accepted_buckets
    }

    /// Number of explicit gaps.
    pub const fn missing_buckets(self) -> u64 {
        self.missing_buckets
    }

    /// Exact elapsed duration represented by the range.
    pub const fn span_duration(self) -> u64 {
        self.span_duration
    }

    /// Exact duration represented by accepted observations.
    pub const fn covered_duration(self) -> u64 {
        self.covered_duration
    }

    /// Coverage state.
    pub const fn state(self) -> CoverageState {
        self.state
    }
}

/// Errors from grid, observation, monoid, and checked arithmetic operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummaryError {
    /// A grid has zero or excessive bucket duration.
    InvalidGrid,
    /// An interval is reversed or exceeds [`MAX_VALUE`].
    InvalidObservation,
    /// A bucket index cannot advance to its exclusive end.
    BucketOverflow,
    /// A summary would exceed [`MAX_BUCKETS`].
    SpanTooLarge,
    /// A checked aggregate operation overflowed.
    ArithmeticOverflow,
    /// The two operands use different semantic grids.
    MismatchedGrid,
    /// The right range does not begin at the left exclusive end.
    NonAdjacent,
    /// A summary failed its internal consistency checks.
    MalformedSummary,
}

/// Refusals from closed statistic evaluators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticError {
    /// No accepted observation exists for this statistic.
    NoAcceptedCoverage,
    /// The statistic needs information not present in the summary family.
    UnsupportedPredicate,
    /// An interval denominator includes zero, so division would not be
    /// conservative without a registered sign/refusal policy.
    AmbiguousDenominator,
    /// A derived ratio endpoint could not be represented in the frozen width.
    ArithmeticOverflow,
}

/// An immutable associative summary over one contiguous bucket range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Summary {
    grid: Grid,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    accepted_count: u64,
    missing_count: u64,
    span_duration: u64,
    covered_duration: u64,
    has_first: bool,
    first: ValueInterval,
    has_last: bool,
    last: ValueInterval,
    has_extrema: bool,
    min_low: u128,
    min_high: u128,
    max_low: u128,
    max_high: u128,
    price_time_integral_low: u128,
    price_time_integral_high: u128,
    state: CoverageState,
}

impl Summary {
    /// Return the identity summary for `grid`.
    pub const fn empty(grid: Grid) -> Self {
        Self {
            grid,
            start_bucket: 0,
            end_bucket_exclusive: 0,
            accepted_count: 0,
            missing_count: 0,
            span_duration: 0,
            covered_duration: 0,
            has_first: false,
            first: ValueInterval { low: 0, high: 0 },
            has_last: false,
            last: ValueInterval { low: 0, high: 0 },
            has_extrema: false,
            min_low: 0,
            min_high: 0,
            max_low: 0,
            max_high: 0,
            price_time_integral_low: 0,
            price_time_integral_high: 0,
            state: CoverageState::Empty,
        }
    }

    /// Build a one-bucket summary after validating the observation.
    pub fn singleton(grid: Grid, observation: Observation) -> Result<Self, SummaryError> {
        validate_grid(grid)?;
        let end = observation
            .bucket()
            .checked_add(1)
            .ok_or(SummaryError::BucketOverflow)?;
        let span_duration = grid.bucket_seconds;
        match observation {
            Observation::Missing { .. } => Ok(Self {
                grid,
                start_bucket: observation.bucket(),
                end_bucket_exclusive: end,
                accepted_count: 0,
                missing_count: 1,
                span_duration,
                covered_duration: 0,
                has_first: false,
                first: zero_interval(),
                has_last: false,
                last: zero_interval(),
                has_extrema: false,
                min_low: 0,
                min_high: 0,
                max_low: 0,
                max_high: 0,
                price_time_integral_low: 0,
                price_time_integral_high: 0,
                state: CoverageState::Gapped,
            }),
            Observation::Accepted { value, .. } => {
                let value = ValueInterval::new(value.low, value.high)?;
                let duration = u128::from(grid.bucket_seconds);
                let integral_low = value
                    .low
                    .checked_mul(duration)
                    .ok_or(SummaryError::ArithmeticOverflow)?;
                let integral_high = value
                    .high
                    .checked_mul(duration)
                    .ok_or(SummaryError::ArithmeticOverflow)?;
                Ok(Self {
                    grid,
                    start_bucket: observation.bucket(),
                    end_bucket_exclusive: end,
                    accepted_count: 1,
                    missing_count: 0,
                    span_duration,
                    covered_duration: span_duration,
                    has_first: true,
                    first: value,
                    has_last: true,
                    last: value,
                    has_extrema: true,
                    min_low: value.low,
                    min_high: value.high,
                    max_low: value.low,
                    max_high: value.high,
                    price_time_integral_low: integral_low,
                    price_time_integral_high: integral_high,
                    state: CoverageState::Complete,
                })
            }
        }
    }

    /// Append one next bucket using the same grid.
    pub fn append(self, observation: Observation) -> Result<Self, SummaryError> {
        let singleton = Self::singleton(self.grid, observation)?;
        self.combine(singleton)
    }

    /// Combine adjacent summaries, preserving exact integer aggregates.
    pub fn combine(self, rhs: Self) -> Result<Self, SummaryError> {
        validate_grid(self.grid)?;
        validate_grid(rhs.grid)?;
        self.validate()?;
        rhs.validate()?;
        if self.grid != rhs.grid {
            return Err(SummaryError::MismatchedGrid);
        }
        if self.state == CoverageState::Empty {
            return Ok(rhs);
        }
        if rhs.state == CoverageState::Empty {
            return Ok(self);
        }
        if self.end_bucket_exclusive != rhs.start_bucket {
            return Err(SummaryError::NonAdjacent);
        }
        let left_len = self.range_len()?;
        let right_len = rhs.range_len()?;
        let range_len = left_len
            .checked_add(right_len)
            .ok_or(SummaryError::SpanTooLarge)?;
        if range_len > MAX_BUCKETS {
            return Err(SummaryError::SpanTooLarge);
        }
        let accepted_count = self
            .accepted_count
            .checked_add(rhs.accepted_count)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let missing_count = self
            .missing_count
            .checked_add(rhs.missing_count)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let span_duration = self
            .span_duration
            .checked_add(rhs.span_duration)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let covered_duration = self
            .covered_duration
            .checked_add(rhs.covered_duration)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let integral_low = self
            .price_time_integral_low
            .checked_add(rhs.price_time_integral_low)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let integral_high = self
            .price_time_integral_high
            .checked_add(rhs.price_time_integral_high)
            .ok_or(SummaryError::ArithmeticOverflow)?;
        let (has_first, first) = if self.has_first {
            (true, self.first)
        } else {
            (rhs.has_first, rhs.first)
        };
        let (has_last, last) = if rhs.has_last {
            (true, rhs.last)
        } else {
            (self.has_last, self.last)
        };
        let (has_extrema, min_low, min_high, max_low, max_high) = combine_extrema(self, rhs);
        let state = if missing_count == 0 {
            CoverageState::Complete
        } else {
            CoverageState::Gapped
        };
        let result = Self {
            grid: self.grid,
            start_bucket: self.start_bucket,
            end_bucket_exclusive: rhs.end_bucket_exclusive,
            accepted_count,
            missing_count,
            span_duration,
            covered_duration,
            has_first,
            first,
            has_last,
            last,
            has_extrema,
            min_low,
            min_high,
            max_low,
            max_high,
            price_time_integral_low: integral_low,
            price_time_integral_high: integral_high,
            state,
        };
        result.validate()?;
        Ok(result)
    }

    /// Check all executable consistency conditions of this summary.
    pub fn validate(&self) -> Result<(), SummaryError> {
        validate_grid(self.grid)?;
        if self.state == CoverageState::Empty {
            if self.start_bucket != 0
                || self.end_bucket_exclusive != 0
                || self.accepted_count != 0
                || self.missing_count != 0
                || self.span_duration != 0
                || self.covered_duration != 0
                || self.has_first
                || self.has_last
                || self.has_extrema
                || self.price_time_integral_low != 0
                || self.price_time_integral_high != 0
            {
                return Err(SummaryError::MalformedSummary);
            }
            return Ok(());
        }
        if self.end_bucket_exclusive <= self.start_bucket {
            return Err(SummaryError::MalformedSummary);
        }
        let buckets = self.range_len()?;
        if buckets == 0 || buckets > MAX_BUCKETS {
            return Err(SummaryError::MalformedSummary);
        }
        if self.accepted_count > buckets || self.missing_count > buckets {
            return Err(SummaryError::MalformedSummary);
        }
        if self.accepted_count + self.missing_count != buckets {
            return Err(SummaryError::MalformedSummary);
        }
        let expected_span = buckets
            .checked_mul(self.grid.bucket_seconds)
            .ok_or(SummaryError::MalformedSummary)?;
        let expected_covered = self
            .accepted_count
            .checked_mul(self.grid.bucket_seconds)
            .ok_or(SummaryError::MalformedSummary)?;
        if self.span_duration != expected_span || self.covered_duration != expected_covered {
            return Err(SummaryError::MalformedSummary);
        }
        if (self.accepted_count != 0) != self.has_first
            || (self.accepted_count != 0) != self.has_last
            || (self.accepted_count != 0) != self.has_extrema
        {
            return Err(SummaryError::MalformedSummary);
        }
        if self.missing_count == 0 {
            if self.state != CoverageState::Complete {
                return Err(SummaryError::MalformedSummary);
            }
        } else if self.state != CoverageState::Gapped {
            return Err(SummaryError::MalformedSummary);
        }
        if self.has_first {
            validate_value(self.first)?;
            validate_value(self.last)?;
            validate_value(ValueInterval {
                low: self.min_low,
                high: self.min_high,
            })?;
            validate_value(ValueInterval {
                low: self.max_low,
                high: self.max_high,
            })?;
            if self.min_low > self.max_low || self.min_high > self.max_high {
                return Err(SummaryError::MalformedSummary);
            }
        } else if self.price_time_integral_low != 0 || self.price_time_integral_high != 0 {
            return Err(SummaryError::MalformedSummary);
        }
        if self.price_time_integral_low > self.price_time_integral_high {
            return Err(SummaryError::MalformedSummary);
        }
        Ok(())
    }

    fn range_len(&self) -> Result<u64, SummaryError> {
        self.end_bucket_exclusive
            .checked_sub(self.start_bucket)
            .ok_or(SummaryError::MalformedSummary)
    }

    /// Grid identity.
    pub const fn grid(self) -> Grid {
        self.grid
    }

    /// Inclusive first bucket, or `None` for the identity.
    pub const fn start_bucket(self) -> Option<u64> {
        match self.state {
            CoverageState::Empty => None,
            CoverageState::Complete | CoverageState::Gapped => Some(self.start_bucket),
        }
    }

    /// Exclusive end bucket, or `None` for the identity.
    pub const fn end_bucket_exclusive(self) -> Option<u64> {
        match self.state {
            CoverageState::Empty => None,
            CoverageState::Complete | CoverageState::Gapped => Some(self.end_bucket_exclusive),
        }
    }

    /// Coverage counters and exact durations.
    pub const fn coverage(self) -> Coverage {
        Coverage {
            total_buckets: self.end_bucket_exclusive - self.start_bucket,
            accepted_buckets: self.accepted_count,
            missing_buckets: self.missing_count,
            span_duration: self.span_duration,
            covered_duration: self.covered_duration,
            state: self.state,
        }
    }

    /// First accepted interval, if any.
    pub const fn first_interval(self) -> Option<ValueInterval> {
        if self.has_first {
            Some(self.first)
        } else {
            None
        }
    }

    /// Last accepted interval, if any.
    pub const fn last_interval(self) -> Option<ValueInterval> {
        if self.has_last {
            Some(self.last)
        } else {
            None
        }
    }

    /// Conservative sampled minimum interval, if any accepted bucket exists.
    pub const fn sampled_min(self) -> Option<ValueInterval> {
        if self.has_extrema {
            Some(ValueInterval {
                low: self.min_low,
                high: self.min_high,
            })
        } else {
            None
        }
    }

    /// Conservative sampled maximum interval, if any accepted bucket exists.
    pub const fn sampled_max(self) -> Option<ValueInterval> {
        if self.has_extrema {
            Some(ValueInterval {
                low: self.max_low,
                high: self.max_high,
            })
        } else {
            None
        }
    }

    /// Exact lower and upper integral numerators over accepted durations.
    pub fn price_time_integral(self) -> Option<RatioInterval> {
        if self.accepted_count == 0 {
            None
        } else {
            Some(RatioInterval {
                numerator_low: self.price_time_integral_low,
                numerator_high: self.price_time_integral_high,
                denominator: u128::from(self.covered_duration),
            })
        }
    }

    /// Exact TWAP ratio interval. No display rounding is performed here.
    pub fn twap(self) -> Result<RatioInterval, StatisticError> {
        match self.price_time_integral() {
            Some(ratio) => Ok(ratio),
            None => Err(StatisticError::NoAcceptedCoverage),
        }
    }

    /// Return the conservative interval for terminal value divided by TWAP.
    ///
    /// Both accepted value endpoints are non-negative by the normalized V1
    /// domain. If the TWAP lower numerator is zero, the denominator interval
    /// includes zero and this method refuses rather than inventing a finite
    /// ratio. Endpoint products remain checked even though the frozen bounds
    /// make overflow unreachable for admitted summaries.
    pub fn relative_terminal_to_twap(self) -> Result<FractionInterval, StatisticError> {
        let terminal = self.terminal()?;
        let integral = self
            .price_time_integral()
            .ok_or(StatisticError::NoAcceptedCoverage)?;
        if integral.numerator_low == 0 || integral.numerator_high == 0 {
            return Err(StatisticError::AmbiguousDenominator);
        }
        let duration = u128::from(self.covered_duration);
        let low_numerator = terminal
            .low
            .checked_mul(duration)
            .ok_or(StatisticError::ArithmeticOverflow)?;
        let high_numerator = terminal
            .high
            .checked_mul(duration)
            .ok_or(StatisticError::ArithmeticOverflow)?;
        Ok(FractionInterval {
            low_numerator,
            low_denominator: integral.numerator_high,
            high_numerator,
            high_denominator: integral.numerator_low,
        })
    }

    /// Terminal accepted interval, or a refusal when no accepted value exists.
    pub const fn terminal(self) -> Result<ValueInterval, StatisticError> {
        match self.last_interval() {
            Some(value) => Ok(value),
            None => Err(StatisticError::NoAcceptedCoverage),
        }
    }

    /// Refuse threshold-crossing predicates: topology was not retained.
    pub const fn threshold_crossings(self, _threshold: u128) -> Result<u64, StatisticError> {
        let _ = self;
        Err(StatisticError::UnsupportedPredicate)
    }

    /// Refuse path-dependent drawdown: extrema do not retain the path.
    pub const fn maximum_drawdown(self) -> Result<ValueInterval, StatisticError> {
        let _ = self;
        Err(StatisticError::UnsupportedPredicate)
    }

    /// Refuse realized variance: the V1 summary does not carry synchronized
    /// return terms or their conservative interval composition.
    pub const fn realized_variance(self) -> Result<ValueInterval, StatisticError> {
        let _ = self;
        Err(StatisticError::UnsupportedPredicate)
    }
}

const fn zero_interval() -> ValueInterval {
    ValueInterval { low: 0, high: 0 }
}

fn validate_grid(grid: Grid) -> Result<(), SummaryError> {
    if grid.bucket_seconds == 0 || grid.bucket_seconds > MAX_BUCKET_SECONDS {
        Err(SummaryError::InvalidGrid)
    } else {
        Ok(())
    }
}

fn validate_value(value: ValueInterval) -> Result<(), SummaryError> {
    if value.low > value.high || value.high > MAX_VALUE {
        Err(SummaryError::MalformedSummary)
    } else {
        Ok(())
    }
}

fn combine_extrema(left: Summary, right: Summary) -> (bool, u128, u128, u128, u128) {
    match (left.has_extrema, right.has_extrema) {
        (false, false) => (false, 0, 0, 0, 0),
        (true, false) => (
            true,
            left.min_low,
            left.min_high,
            left.max_low,
            left.max_high,
        ),
        (false, true) => (
            true,
            right.min_low,
            right.min_high,
            right.max_low,
            right.max_high,
        ),
        (true, true) => (
            true,
            min(left.min_low, right.min_low),
            min(left.min_high, right.min_high),
            max(left.max_low, right.max_low),
            max(left.max_high, right.max_high),
        ),
    }
}

const fn min(left: u128, right: u128) -> u128 {
    if left < right {
        left
    } else {
        right
    }
}

const fn max(left: u128, right: u128) -> u128 {
    if left > right {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    fn grid() -> Grid {
        Grid::new(7, 1, 60).expect("valid test grid")
    }

    fn fold_left(observations: &[Observation], grid: Grid) -> Summary {
        observations
            .iter()
            .copied()
            .try_fold(Summary::empty(grid), |summary, observation| {
                summary.append(observation)
            })
            .expect("test observations are valid")
    }

    fn fold_split(observations: &[Observation], grid: Grid) -> Summary {
        if observations.is_empty() {
            return Summary::empty(grid);
        }
        if observations.len() == 1 {
            return Summary::singleton(grid, observations[0]).expect("valid singleton");
        }
        let split = observations.len() / 2;
        fold_split(&observations[..split], grid)
            .combine(fold_split(&observations[split..], grid))
            .expect("adjacent split")
    }

    #[test]
    fn identity_is_two_sided() {
        let observations = [
            Observation::accepted(30, 10, 12),
            Observation::missing(31),
            Observation::accepted(32, 9, 20),
        ];
        let summary = fold_left(&observations, grid());
        assert_eq!(Summary::empty(grid()).combine(summary), Ok(summary));
        assert_eq!(summary.combine(Summary::empty(grid())), Ok(summary));
    }

    #[test]
    fn associativity_holds_for_mixed_gaps_and_intervals() {
        let observations = [
            Observation::accepted(100, 10, 11),
            Observation::missing(101),
            Observation::accepted(102, 5, 8),
            Observation::accepted(103, 50, 60),
            Observation::missing(104),
            Observation::accepted(105, 2, 3),
        ];
        let left = fold_left(&observations, grid());
        let right = fold_split(&observations, grid());
        assert_eq!(left, right);
        assert_eq!(left.validate(), Ok(()));
    }

    #[test]
    fn deterministic_property_parenthesizations() {
        let test_grid = grid();
        let mut seed = 0x9e37_79b9_u64;
        let mut observations = Vec::new();
        for bucket in 4_000..4_080 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            if seed.is_multiple_of(7) {
                observations.push(Observation::missing(bucket));
            } else {
                let low = u128::from(seed % 1_000);
                let width = u128::from((seed >> 12) % 17);
                observations.push(Observation::accepted(bucket, low, low + width));
            }
        }
        let canonical = fold_left(&observations, test_grid);
        for split in 1..observations.len() {
            let candidate = fold_left(&observations[..split], test_grid)
                .combine(fold_left(&observations[split..], test_grid))
                .expect("every split is adjacent");
            assert_eq!(candidate, canonical, "split {split}");
        }
        assert_eq!(canonical.coverage().total_buckets(), 80);
        assert_eq!(
            canonical.coverage().total_buckets(),
            canonical.coverage().accepted_buckets() + canonical.coverage().missing_buckets()
        );
    }

    #[test]
    fn exact_integral_and_twap_keep_integer_boundary() {
        let test_grid = Grid::new(1, 9, 10).expect("valid grid");
        let summary = fold_left(
            &[
                Observation::accepted(8, 2, 4),
                Observation::accepted(9, 6, 8),
            ],
            test_grid,
        );
        let integral = summary.price_time_integral().expect("coverage");
        assert_eq!(integral.numerator_low(), 80);
        assert_eq!(integral.numerator_high(), 120);
        assert_eq!(integral.denominator(), 20);
        assert_eq!(summary.twap(), Ok(integral));
    }

    #[test]
    fn relative_terminal_to_twap_retains_both_exact_endpoints() {
        let test_grid = Grid::new(1, 9, 10).expect("valid grid");
        let summary = fold_left(
            &[
                Observation::accepted(8, 2, 4),
                Observation::accepted(9, 6, 8),
            ],
            test_grid,
        );
        let ratio = summary.relative_terminal_to_twap().expect("positive TWAP");
        // Terminal [6, 8] / TWAP [4, 6] = [6/6, 8/4].
        assert_eq!(ratio.low_numerator(), 120);
        assert_eq!(ratio.low_denominator(), 120);
        assert_eq!(ratio.high_numerator(), 160);
        assert_eq!(ratio.high_denominator(), 80);
    }

    #[test]
    fn relative_ratio_refuses_zero_denominator() {
        let summary = fold_left(
            &[
                Observation::accepted(8, 0, 0),
                Observation::accepted(9, 0, 0),
            ],
            grid(),
        );
        assert_eq!(
            summary.relative_terminal_to_twap(),
            Err(StatisticError::AmbiguousDenominator)
        );
    }

    #[test]
    fn extrema_and_first_last_are_conservative_intervals() {
        let summary = fold_left(
            &[
                Observation::accepted(9, 100, 110),
                Observation::accepted(10, 20, 90),
                Observation::accepted(11, 50, 60),
            ],
            grid(),
        );
        assert_eq!(summary.first_interval(), ValueInterval::new(100, 110).ok());
        assert_eq!(summary.last_interval(), ValueInterval::new(50, 60).ok());
        assert_eq!(summary.sampled_min(), ValueInterval::new(20, 60).ok());
        assert_eq!(summary.sampled_max(), ValueInterval::new(100, 110).ok());
    }

    #[test]
    fn invalid_joins_and_arithmetic_refuse() {
        let test_grid = grid();
        let left = Summary::singleton(test_grid, Observation::missing(1)).expect("valid");
        let non_adjacent = Summary::singleton(test_grid, Observation::missing(3)).expect("valid");
        assert_eq!(left.combine(non_adjacent), Err(SummaryError::NonAdjacent));
        let other_grid = Grid::new(8, 1, 60).expect("valid grid");
        let right = Summary::singleton(other_grid, Observation::missing(2)).expect("valid");
        assert_eq!(left.combine(right), Err(SummaryError::MismatchedGrid));
        let invalid = Summary::singleton(
            test_grid,
            Observation::accepted(2, MAX_VALUE + 1, MAX_VALUE + 1),
        );
        assert_eq!(invalid, Err(SummaryError::InvalidObservation));
    }

    #[test]
    fn unsupported_predicates_refuse_without_false_precision() {
        let summary = fold_left(
            &[
                Observation::accepted(0, 1, 1),
                Observation::accepted(1, 9, 9),
            ],
            grid(),
        );
        assert_eq!(
            summary.threshold_crossings(5),
            Err(StatisticError::UnsupportedPredicate)
        );
        assert_eq!(
            summary.maximum_drawdown(),
            Err(StatisticError::UnsupportedPredicate)
        );
        assert_eq!(
            summary.realized_variance(),
            Err(StatisticError::UnsupportedPredicate)
        );
    }

    #[test]
    fn equal_summaries_can_hide_different_threshold_topology() {
        let grid = Grid::new(3, 1, 1).expect("valid grid");
        let clustered = [1_u128, 1, 9, 9, 1, 9];
        let alternating = [1_u128, 9, 1, 9, 1, 9];
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (index, value) in clustered.iter().copied().enumerate() {
            let bucket = u64::try_from(index).expect("small test index");
            left.push(Observation::accepted(bucket, value, value));
        }
        for (index, value) in alternating.iter().copied().enumerate() {
            let bucket = u64::try_from(index).expect("small test index");
            right.push(Observation::accepted(bucket, value, value));
        }
        let left_summary = fold_left(&left, grid);
        let right_summary = fold_left(&right, grid);
        assert_eq!(left_summary, right_summary);
        // Above threshold 5, the hidden path has 3 versus 5 crossings.
        // The summary correctly refuses this statistic rather than choosing.
        assert_eq!(
            left_summary.threshold_crossings(5),
            Err(StatisticError::UnsupportedPredicate)
        );
    }
}
