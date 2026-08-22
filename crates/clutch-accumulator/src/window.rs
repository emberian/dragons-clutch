//! Domain-bound window evidence over the [`Summary`] monoid.
//!
//! Status: MODEL/PROPOSED. Nothing in this module authenticates anything. It
//! defines the *shape* of the evidence an eventual adapter must construct, so
//! that a bare statistic can no longer be mistaken for settlement authority.
//!
//! [`Summary`] answers "what did these buckets look like". It deliberately
//! answers that question for *any* bucket range, complete or gapped. That is
//! correct for an associative accumulator and wrong for a settlement term:
//! `docs/implementation/ADVERSARIAL_REVIEW_V0.md` §P1-D records that a caller
//! can accidentally treat an accepted-only statistic as a full-window one.
//!
//! This module closes that composition hole with types rather than prose:
//!
//! - a [`WindowDomain`] states the exact expected feed identity, grid, bucket
//!   range, maturity bound, repair generation, and registered coverage policy;
//! - a [`WindowAccumulator`] folds pages into exactly that domain and runs the
//!   open/mature/sealed state machine generalized from the vertical model's
//!   frozen host horizon; and
//! - a [`WindowResult`] is produced only by sealing such an accumulator and
//!   passing its registered coverage policy.
//!
//! No public constructor turns a [`Summary`] into a [`WindowResult`]. A
//! settlement-facing function that names `&WindowResult` therefore cannot be
//! handed a bare summary, a truncated prefix, a gapped substitute, another
//! window, or another repair generation: each is a compile error or an explicit
//! refusal, not a convention a reviewer has to notice.

use crate::{
    Coverage, CoverageState, FractionInterval, Grid, Observation, RatioInterval, StatisticError,
    Summary, SummaryError, ValueInterval, MAX_BUCKETS,
};

/// Bytes in one opaque adapter-supplied identity.
pub const IDENTITY_BYTES: usize = 32;

/// Exact length of the canonical [`WindowDomain`] byte encoding.
pub const WINDOW_DOMAIN_BYTES: usize = 144;

/// Magic prefix of the canonical [`WindowDomain`] byte encoding.
pub const WINDOW_DOMAIN_MAGIC: [u8; 8] = *b"DCWINR1\0";

/// Schema version of the canonical [`WindowDomain`] byte encoding.
pub const WINDOW_DOMAIN_SCHEMA_VERSION: u16 = 1;

/// Zero reserved bytes at the tail of the canonical encoding.
pub const WINDOW_DOMAIN_RESERVED_BYTES: usize = 6;

/// Domain-separation string an adapter must prepend when it content-addresses
/// [`WindowDomain::encode_canonical`] bytes into a `WindowId`.
///
/// This crate deliberately owns no hash primitive and computes no identity. It
/// publishes the tag and the exact preimage bytes so that a hashing adapter and
/// an independent recomputation cannot disagree about either.
pub const WINDOW_DOMAIN_TAG: &[u8] = b"dragons-clutch/window-domain/v1";

/// Registered identifier of the "every expected bucket accepted" policy.
pub const COVERAGE_POLICY_COMPLETE_REQUIRED: u16 = 1;

/// Registered identifier of the bounded-gap policy.
pub const COVERAGE_POLICY_BOUNDED_GAPS: u16 = 2;

/// Refusals from window domain construction, folding, sealing, and binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowError {
    /// An opaque 32-byte identity was all zero.
    ZeroIdentity,
    /// A source or evaluator version was zero, which names no frozen artifact.
    UnversionedIdentity,
    /// The expected bucket range is empty, reversed, or exceeds
    /// [`MAX_BUCKETS`].
    InvalidRange,
    /// The maturity bound is earlier than the window's exclusive end.
    InvalidMaturity,
    /// The coverage policy identifier is not registered in this crate.
    UnknownCoveragePolicy,
    /// The coverage policy parameter is outside its registered domain.
    InvalidPolicyParameter,
    /// A page or expectation used a different semantic grid.
    MismatchedGrid,
    /// The feed identity, source version, or evaluator version differs.
    MismatchedFeed,
    /// The registered coverage policy differs.
    MismatchedCoveragePolicy,
    /// The repair generation differs.
    MismatchedGeneration,
    /// The maturity bound differs.
    MismatchedMaturity,
    /// The bucket range differs.
    WrongWindow,
    /// A page does not begin at the accumulator's next expected bucket.
    NonContiguous,
    /// A page would extend past the window's exclusive end.
    RangeOverflow,
    /// A witnessed feed cursor moved backwards.
    NonMonotoneCursor,
    /// A page was offered after the window was sealed.
    ObservationAfterSeal,
    /// Not every expected bucket has been represented yet.
    IncompleteDomain,
    /// The authenticated feed cursor has not reached the maturity bound.
    NotMature,
    /// The window is already sealed and cannot be sealed or reopened.
    AlreadySealed,
    /// The window is not sealed, so no result exists.
    NotSealed,
    /// The observed coverage is refused by the registered coverage policy.
    CoverageRefused,
    /// A sealed window failed its internal consistency checks.
    MalformedResult,
    /// The underlying summary algebra refused.
    Summary(SummaryError),
}

impl From<SummaryError> for WindowError {
    fn from(error: SummaryError) -> Self {
        Self::Summary(error)
    }
}

/// A registered coverage policy.
///
/// This type is the crate's registry: its fields are private and only the
/// constructors below exist, so a caller cannot invent a policy identifier or
/// attach unregistered semantics to a registered one. Adding a policy is a
/// crate change with its own tests, not a call-site decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveragePolicy {
    id: u16,
    max_missing_buckets: u64,
}

impl CoveragePolicy {
    /// Every bucket in the expected range must carry an accepted observation.
    pub const COMPLETE_REQUIRED: Self = Self {
        id: COVERAGE_POLICY_COMPLETE_REQUIRED,
        max_missing_buckets: 0,
    };

    /// Admit at most `max_missing_buckets` explicit gaps in the expected range.
    ///
    /// A zero bound is refused: it would be a second identifier for
    /// [`CoveragePolicy::COMPLETE_REQUIRED`] and therefore a parallel truth.
    pub const fn bounded_gaps(max_missing_buckets: u64) -> Result<Self, WindowError> {
        if max_missing_buckets == 0 || max_missing_buckets > MAX_BUCKETS {
            return Err(WindowError::InvalidPolicyParameter);
        }
        Ok(Self {
            id: COVERAGE_POLICY_BOUNDED_GAPS,
            max_missing_buckets,
        })
    }

    /// Rebuild a policy from decoded bytes, refusing unregistered identifiers.
    pub const fn from_registry(id: u16, max_missing_buckets: u64) -> Result<Self, WindowError> {
        match id {
            COVERAGE_POLICY_COMPLETE_REQUIRED => {
                if max_missing_buckets != 0 {
                    return Err(WindowError::InvalidPolicyParameter);
                }
                Ok(Self::COMPLETE_REQUIRED)
            }
            COVERAGE_POLICY_BOUNDED_GAPS => Self::bounded_gaps(max_missing_buckets),
            _ => Err(WindowError::UnknownCoveragePolicy),
        }
    }

    /// Registered identifier.
    pub const fn id(self) -> u16 {
        self.id
    }

    /// Registered gap bound; zero for [`CoveragePolicy::COMPLETE_REQUIRED`].
    pub const fn max_missing_buckets(self) -> u64 {
        self.max_missing_buckets
    }

    /// Whether this policy admits the observed coverage of a full window.
    ///
    /// The caller must already have proved that the coverage spans exactly the
    /// expected range; this predicate only judges gaps.
    pub const fn admits(self, coverage: Coverage) -> bool {
        match self.id {
            COVERAGE_POLICY_COMPLETE_REQUIRED => {
                coverage.missing_buckets() == 0
                    && matches!(coverage.state(), CoverageState::Complete)
            }
            COVERAGE_POLICY_BOUNDED_GAPS => coverage.missing_buckets() <= self.max_missing_buckets,
            _ => false,
        }
    }
}

/// Versioned identity of the authenticated source adapter and its evaluator.
///
/// The two 32-byte values are opaque here on purpose: this crate must stay
/// source-neutral and must not learn how to parse an external account. An
/// adapter binds them to its own frozen `SourceAdapterId`/`FeedSpecId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedIdentity {
    source_adapter_id: [u8; IDENTITY_BYTES],
    feed_spec_id: [u8; IDENTITY_BYTES],
    source_version: u32,
    evaluator_version: u32,
}

const fn is_zero_identity(bytes: &[u8; IDENTITY_BYTES]) -> bool {
    let mut index = 0;
    while index < IDENTITY_BYTES {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

impl FeedIdentity {
    /// Construct a feed identity, refusing zero identities and zero versions.
    pub const fn new(
        source_adapter_id: [u8; IDENTITY_BYTES],
        feed_spec_id: [u8; IDENTITY_BYTES],
        source_version: u32,
        evaluator_version: u32,
    ) -> Result<Self, WindowError> {
        if is_zero_identity(&source_adapter_id) || is_zero_identity(&feed_spec_id) {
            return Err(WindowError::ZeroIdentity);
        }
        if source_version == 0 || evaluator_version == 0 {
            return Err(WindowError::UnversionedIdentity);
        }
        Ok(Self {
            source_adapter_id,
            feed_spec_id,
            source_version,
            evaluator_version,
        })
    }

    /// Frozen source-adapter identity.
    pub const fn source_adapter_id(&self) -> [u8; IDENTITY_BYTES] {
        self.source_adapter_id
    }

    /// Frozen feed-specification identity.
    pub const fn feed_spec_id(&self) -> [u8; IDENTITY_BYTES] {
        self.feed_spec_id
    }

    /// Version of the source adapter that produced the observations.
    pub const fn source_version(&self) -> u32 {
        self.source_version
    }

    /// Version of the statistic evaluator admitted for this window.
    pub const fn evaluator_version(&self) -> u32 {
        self.evaluator_version
    }
}

/// The exact domain a window result is bound to.
///
/// Two results are interchangeable only when their domains are equal. Every
/// field is part of that equality: swapping a grid, a bucket range, a maturity
/// bound, a repair generation, or a coverage policy produces a different
/// domain, and [`WindowResult::check_domain`] names which field differed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowDomain {
    feed: FeedIdentity,
    grid: Grid,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    maturity_bucket_exclusive: u64,
    generation: u64,
    coverage: CoveragePolicy,
}

impl WindowDomain {
    /// Construct an expected domain.
    ///
    /// `maturity_bucket_exclusive` is the first bucket at which the feed's
    /// repair interval for this window has closed. It is at least
    /// `end_bucket_exclusive`; any excess is the frozen repair grace. This
    /// generalizes the vertical model's single `MATURITY_BUCKETS` host constant
    /// into a per-window bound that a Market's immutable terms can name.
    pub const fn new(
        feed: FeedIdentity,
        grid: Grid,
        start_bucket: u64,
        end_bucket_exclusive: u64,
        maturity_bucket_exclusive: u64,
        generation: u64,
        coverage: CoveragePolicy,
    ) -> Result<Self, WindowError> {
        if end_bucket_exclusive <= start_bucket {
            return Err(WindowError::InvalidRange);
        }
        let range_len = end_bucket_exclusive - start_bucket;
        if range_len > MAX_BUCKETS {
            return Err(WindowError::InvalidRange);
        }
        if maturity_bucket_exclusive < end_bucket_exclusive {
            return Err(WindowError::InvalidMaturity);
        }
        Ok(Self {
            feed,
            grid,
            start_bucket,
            end_bucket_exclusive,
            maturity_bucket_exclusive,
            generation,
            coverage,
        })
    }

    /// Feed, source, and evaluator identity.
    pub const fn feed(&self) -> FeedIdentity {
        self.feed
    }

    /// Semantic grid identity and exact bucket duration.
    pub const fn grid(&self) -> Grid {
        self.grid
    }

    /// Inclusive first expected bucket.
    pub const fn start_bucket(&self) -> u64 {
        self.start_bucket
    }

    /// Exclusive last expected bucket.
    pub const fn end_bucket_exclusive(&self) -> u64 {
        self.end_bucket_exclusive
    }

    /// First bucket at which the repair interval for this window has closed.
    pub const fn maturity_bucket_exclusive(&self) -> u64 {
        self.maturity_bucket_exclusive
    }

    /// Repair generation of the pages this domain admits.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Registered coverage policy.
    pub const fn coverage_policy(&self) -> CoveragePolicy {
        self.coverage
    }

    /// Number of expected buckets.
    pub const fn range_len(&self) -> u64 {
        self.end_bucket_exclusive - self.start_bucket
    }

    /// Write the canonical fixed-length preimage of this domain.
    ///
    /// The encoding is total for any constructed domain and never allocates.
    /// It is a preimage only: this crate does not hash it. An adapter that
    /// content-addresses a window must hash [`WINDOW_DOMAIN_TAG`] followed by
    /// exactly these [`WINDOW_DOMAIN_BYTES`] bytes.
    pub fn encode_canonical(&self, out: &mut [u8; WINDOW_DOMAIN_BYTES]) {
        let mut at = 0usize;
        write_field(out, &mut at, &WINDOW_DOMAIN_MAGIC);
        write_field(out, &mut at, &WINDOW_DOMAIN_SCHEMA_VERSION.to_le_bytes());
        write_field(out, &mut at, &self.coverage.id().to_le_bytes());
        write_field(
            out,
            &mut at,
            &self.coverage.max_missing_buckets().to_le_bytes(),
        );
        write_field(out, &mut at, &self.feed.source_adapter_id());
        write_field(out, &mut at, &self.feed.feed_spec_id());
        write_field(out, &mut at, &self.feed.source_version().to_le_bytes());
        write_field(out, &mut at, &self.feed.evaluator_version().to_le_bytes());
        write_field(out, &mut at, &self.grid.family_id().to_le_bytes());
        write_field(out, &mut at, &self.grid.version().to_le_bytes());
        write_field(out, &mut at, &self.grid.bucket_seconds().to_le_bytes());
        write_field(out, &mut at, &self.start_bucket.to_le_bytes());
        write_field(out, &mut at, &self.end_bucket_exclusive.to_le_bytes());
        write_field(out, &mut at, &self.maturity_bucket_exclusive.to_le_bytes());
        write_field(out, &mut at, &self.generation.to_le_bytes());
        write_field(out, &mut at, &[0u8; WINDOW_DOMAIN_RESERVED_BYTES]);
        debug_assert!(at == WINDOW_DOMAIN_BYTES);
    }

    /// Compare against another expected domain, naming the first difference.
    pub const fn check_against(&self, expected: &Self) -> Result<(), WindowError> {
        if !identity_eq(&self.feed, &expected.feed) {
            return Err(WindowError::MismatchedFeed);
        }
        if !grid_eq(self.grid, expected.grid) {
            return Err(WindowError::MismatchedGrid);
        }
        if self.start_bucket != expected.start_bucket
            || self.end_bucket_exclusive != expected.end_bucket_exclusive
        {
            return Err(WindowError::WrongWindow);
        }
        if self.maturity_bucket_exclusive != expected.maturity_bucket_exclusive {
            return Err(WindowError::MismatchedMaturity);
        }
        if self.generation != expected.generation {
            return Err(WindowError::MismatchedGeneration);
        }
        if self.coverage.id() != expected.coverage.id()
            || self.coverage.max_missing_buckets() != expected.coverage.max_missing_buckets()
        {
            return Err(WindowError::MismatchedCoveragePolicy);
        }
        Ok(())
    }
}

fn write_field(out: &mut [u8; WINDOW_DOMAIN_BYTES], at: &mut usize, bytes: &[u8]) {
    out[*at..*at + bytes.len()].copy_from_slice(bytes);
    *at += bytes.len();
}

const fn bytes_eq(left: &[u8; IDENTITY_BYTES], right: &[u8; IDENTITY_BYTES]) -> bool {
    let mut index = 0;
    while index < IDENTITY_BYTES {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn identity_eq(left: &FeedIdentity, right: &FeedIdentity) -> bool {
    bytes_eq(&left.source_adapter_id, &right.source_adapter_id)
        && bytes_eq(&left.feed_spec_id, &right.feed_spec_id)
        && left.source_version == right.source_version
        && left.evaluator_version == right.evaluator_version
}

const fn grid_eq(left: Grid, right: Grid) -> bool {
    left.family_id() == right.family_id()
        && left.version() == right.version()
        && left.bucket_seconds() == right.bucket_seconds()
}

/// Lifecycle phase of one window's evidence.
///
/// This is the generalization of the vertical model's host semantics: that
/// model froze one `MATURITY_BUCKETS` constant and one `sealed` flag for one
/// market. Here the horizon is a per-window field of the immutable domain, the
/// cursor is an explicitly witnessed feed fact, and sealing is terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowPhase {
    /// Expected buckets are still missing, or no maturity witness is present.
    Open,
    /// Every expected bucket is represented and a maturity witness is present.
    Mature,
    /// The window is sealed. No further page is admitted, ever.
    Sealed,
}

/// Folds authenticated pages into exactly one [`WindowDomain`].
///
/// The state machine is `Open -> Mature -> Sealed`, and only a sealed
/// accumulator yields a [`WindowResult`].
///
/// A normal feed cursor witness matures the window only at
/// [`WindowDomain::maturity_bucket_exclusive`]. Some source generations instead
/// authenticate the window's closing boundary directly; those adapters must
/// call [`WindowAccumulator::witness_authenticated_closing_boundary`] after
/// verifying that source-specific rule. Merely appending the final observation
/// never creates that witness.
///
/// ```
/// use clutch_accumulator::{
///     CoveragePolicy, FeedIdentity, Grid, Observation, ValueInterval, WindowAccumulator,
///     WindowDomain, WindowError, WindowPhase,
/// };
///
/// let feed = FeedIdentity::new([1; 32], [2; 32], 1, 1).expect("nonzero identity");
/// let grid = Grid::new(7, 1, 60).expect("valid grid");
/// // Buckets 100..103 must all be accepted; repair closes one bucket later.
/// let domain = WindowDomain::new(
///     feed,
///     grid,
///     100,
///     103,
///     104,
///     0,
///     CoveragePolicy::COMPLETE_REQUIRED,
/// )
/// .expect("valid domain");
///
/// let mut window = WindowAccumulator::open(domain);
/// for bucket in 100..103 {
///     window.observe(Observation::accepted(bucket, 40, 41)).expect("in domain");
/// }
/// // Every expected bucket is present, but the repair grace has not elapsed.
/// assert_eq!(window.phase(), WindowPhase::Open);
/// assert_eq!(window.seal(), Err(WindowError::NotMature));
///
/// window.witness_feed_cursor(104).expect("monotone cursor");
/// assert_eq!(window.phase(), WindowPhase::Mature);
/// window.seal().expect("mature and complete");
///
/// let result = window.result().expect("sealed");
/// assert_eq!(
///     result.terminal().expect("accepted coverage"),
///     ValueInterval::new(40, 41).expect("valid interval"),
/// );
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowAccumulator {
    domain: WindowDomain,
    summary: Summary,
    cursor: u64,
    feed_cursor: u64,
    closing_boundary_witnessed: bool,
    sealed: bool,
    sealed_cursor: u64,
}

impl WindowAccumulator {
    /// Open an empty accumulator for `domain`.
    pub fn open(domain: WindowDomain) -> Self {
        Self {
            domain,
            summary: Summary::empty(domain.grid()),
            cursor: domain.start_bucket(),
            feed_cursor: domain.start_bucket(),
            closing_boundary_witnessed: false,
            sealed: false,
            sealed_cursor: 0,
        }
    }

    /// The domain this accumulator is bound to.
    pub const fn domain(&self) -> WindowDomain {
        self.domain
    }

    /// Next expected bucket inside the window.
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Highest witnessed authenticated feed cursor.
    pub const fn feed_cursor(&self) -> u64 {
        self.feed_cursor
    }

    /// Current lifecycle phase.
    pub const fn phase(&self) -> WindowPhase {
        if self.sealed {
            WindowPhase::Sealed
        } else if self.cursor == self.domain.end_bucket_exclusive()
            && (self.feed_cursor >= self.domain.maturity_bucket_exclusive()
                || self.closing_boundary_witnessed)
        {
            WindowPhase::Mature
        } else {
            WindowPhase::Open
        }
    }

    /// Absorb one already-folded adjacent page summary.
    ///
    /// The identity summary is admitted as a no-op. Any other page must use the
    /// domain's grid, begin at [`WindowAccumulator::cursor`], and end at or
    /// before the domain's exclusive end.
    pub fn absorb(&mut self, page: Summary) -> Result<(), WindowError> {
        if self.sealed {
            return Err(WindowError::ObservationAfterSeal);
        }
        page.validate()?;
        if !grid_eq(page.grid(), self.domain.grid()) {
            return Err(WindowError::MismatchedGrid);
        }
        let (start, end) = match (page.start_bucket(), page.end_bucket_exclusive()) {
            (Some(start), Some(end)) => (start, end),
            _ => return Ok(()),
        };
        if start != self.cursor {
            return Err(WindowError::NonContiguous);
        }
        if end > self.domain.end_bucket_exclusive() {
            return Err(WindowError::RangeOverflow);
        }
        self.summary = self.summary.combine(page)?;
        self.cursor = end;
        if self.feed_cursor < self.cursor {
            self.feed_cursor = self.cursor;
        }
        Ok(())
    }

    /// Absorb exactly one bucket.
    pub fn observe(&mut self, observation: Observation) -> Result<(), WindowError> {
        let page = Summary::singleton(self.domain.grid(), observation)?;
        self.absorb(page)
    }

    /// Witness the authenticated next-bucket cursor of the underlying feed.
    ///
    /// This is the only input that can prove the maturity bound was reached
    /// when the bound exceeds the window's own end. It is monotone: a source
    /// that appears to move backwards is refused rather than believed.
    pub fn witness_feed_cursor(&mut self, next_bucket: u64) -> Result<(), WindowError> {
        if self.sealed {
            return Err(WindowError::ObservationAfterSeal);
        }
        if next_bucket < self.feed_cursor {
            return Err(WindowError::NonMonotoneCursor);
        }
        self.feed_cursor = next_bucket;
        Ok(())
    }

    /// Witness that an authenticated source generation proved the window's
    /// closing boundary is final.
    ///
    /// This is a pure, source-neutral state transition; it does not verify any
    /// signature, clock, proof, account, or archive. Callers may use it only
    /// after their own adapter has proved that `next_bucket` is a valid
    /// generation-specific maturity witness for this domain's exclusive end.
    /// The witnessed cursor is retained verbatim and becomes the
    /// [`WindowResult::sealed_cursor`] once the accumulator is sealed.
    pub fn witness_authenticated_closing_boundary(
        &mut self,
        next_bucket: u64,
    ) -> Result<(), WindowError> {
        if self.sealed {
            return Err(WindowError::ObservationAfterSeal);
        }
        if next_bucket < self.feed_cursor {
            return Err(WindowError::NonMonotoneCursor);
        }
        if next_bucket < self.domain.end_bucket_exclusive() {
            return Err(WindowError::NotMature);
        }
        self.feed_cursor = next_bucket;
        self.closing_boundary_witnessed = true;
        Ok(())
    }

    /// Seal the window. Sealing is terminal and cannot be undone.
    pub fn seal(&mut self) -> Result<(), WindowError> {
        if self.sealed {
            return Err(WindowError::AlreadySealed);
        }
        if self.cursor != self.domain.end_bucket_exclusive() {
            return Err(WindowError::IncompleteDomain);
        }
        if self.feed_cursor < self.domain.maturity_bucket_exclusive()
            && !self.closing_boundary_witnessed
        {
            return Err(WindowError::NotMature);
        }
        self.sealed = true;
        self.sealed_cursor = self.feed_cursor;
        Ok(())
    }

    /// Whether the window is sealed.
    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Evaluate the sealed window against its registered coverage policy.
    ///
    /// This is the only constructor of a [`WindowResult`] in the crate.
    pub fn result(&self) -> Result<WindowResult, WindowError> {
        if !self.sealed {
            return Err(WindowError::NotSealed);
        }
        self.summary.validate()?;
        if self.summary.start_bucket() != Some(self.domain.start_bucket())
            || self.summary.end_bucket_exclusive() != Some(self.domain.end_bucket_exclusive())
        {
            return Err(WindowError::MalformedResult);
        }
        let coverage = self.summary.coverage();
        if coverage.total_buckets() != self.domain.range_len() {
            return Err(WindowError::MalformedResult);
        }
        if !self.domain.coverage_policy().admits(coverage) {
            return Err(WindowError::CoverageRefused);
        }
        Ok(WindowResult {
            domain: self.domain,
            summary: self.summary,
            sealed_cursor: self.sealed_cursor,
        })
    }
}

/// An immutable, domain-bound, sealed window evaluation.
///
/// A `WindowResult` exists only if some [`WindowAccumulator`] covered exactly
/// its domain's bucket range, reached its maturity bound, was sealed, and
/// satisfied its registered coverage policy. There is no other constructor, so
/// a settlement-shaped function that names this type cannot be handed a bare
/// [`Summary`]:
///
/// ```compile_fail
/// use clutch_accumulator::{Grid, Summary, WindowResult};
///
/// fn payout_index(_evidence: &WindowResult) -> u8 { 0 }
///
/// let grid = Grid::new(7, 1, 60).expect("valid grid");
/// let summary = Summary::empty(grid);
/// // A bare summary carries no feed, range, generation, maturity, or seal.
/// let _ = payout_index(&summary);
/// ```
///
/// The statistics below are the same closed evaluators as on [`Summary`], but
/// reaching them required the domain binding. This crate still authenticates
/// nothing: a `WindowResult` is honest evidence of a fold, not of a source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowResult {
    domain: WindowDomain,
    summary: Summary,
    sealed_cursor: u64,
}

impl WindowResult {
    /// The exact domain this result is bound to.
    pub const fn domain(&self) -> WindowDomain {
        self.domain
    }

    /// Always [`WindowPhase::Sealed`]; a result cannot exist in another phase.
    pub const fn phase(&self) -> WindowPhase {
        WindowPhase::Sealed
    }

    /// Feed cursor witnessed at the moment of sealing.
    pub const fn sealed_cursor(&self) -> u64 {
        self.sealed_cursor
    }

    /// Coverage counters and exact durations of the sealed window.
    pub const fn coverage(&self) -> Coverage {
        self.summary.coverage()
    }

    /// Refuse unless this result is bound to exactly `expected`.
    ///
    /// An adapter holding a Market's immutable expected domain calls this
    /// before reading any statistic. The returned error names which field
    /// differed, so a wrong window, generation, maturity, or coverage policy is
    /// a distinguishable refusal rather than one opaque failure.
    pub const fn check_domain(&self, expected: &WindowDomain) -> Result<(), WindowError> {
        self.domain.check_against(expected)
    }

    /// The underlying summary, for diagnostics and independent recomputation.
    ///
    /// The name is a warning. The returned value is no longer domain bound: it
    /// carries no feed identity, expected range, generation, maturity, or seal.
    /// No settlement, payout, or resolution API may accept this type; those
    /// must name [`WindowResult`].
    pub const fn unbound_summary(&self) -> Summary {
        self.summary
    }

    /// Terminal accepted interval of the sealed window.
    pub const fn terminal(&self) -> Result<ValueInterval, StatisticError> {
        self.summary.terminal()
    }

    /// Exact lower and upper integral numerators over accepted durations.
    pub fn price_time_integral(&self) -> Option<RatioInterval> {
        self.summary.price_time_integral()
    }

    /// Exact TWAP ratio interval of the sealed window.
    pub fn twap(&self) -> Result<RatioInterval, StatisticError> {
        self.summary.twap()
    }

    /// Conservative sampled minimum interval of the sealed window.
    pub const fn sampled_min(&self) -> Option<ValueInterval> {
        self.summary.sampled_min()
    }

    /// Conservative sampled maximum interval of the sealed window.
    pub const fn sampled_max(&self) -> Option<ValueInterval> {
        self.summary.sampled_max()
    }

    /// Conservative terminal-to-TWAP ratio interval of the sealed window.
    pub fn relative_terminal_to_twap(&self) -> Result<FractionInterval, StatisticError> {
        self.summary.relative_terminal_to_twap()
    }

    /// Refuse threshold-crossing predicates: topology was not retained.
    pub const fn threshold_crossings(&self, threshold: u128) -> Result<u64, StatisticError> {
        self.summary.threshold_crossings(threshold)
    }

    /// Refuse path-dependent drawdown: extrema do not retain the path.
    pub const fn maximum_drawdown(&self) -> Result<ValueInterval, StatisticError> {
        self.summary.maximum_drawdown()
    }

    /// Refuse realized variance: the summary family carries no return terms.
    pub const fn realized_variance(&self) -> Result<ValueInterval, StatisticError> {
        self.summary.realized_variance()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn feed() -> FeedIdentity {
        FeedIdentity::new([1; IDENTITY_BYTES], [2; IDENTITY_BYTES], 3, 4).expect("valid identity")
    }

    fn grid() -> Grid {
        Grid::new(7, 1, 60).expect("valid grid")
    }

    fn domain(coverage: CoveragePolicy) -> WindowDomain {
        WindowDomain::new(feed(), grid(), 100, 103, 104, 0, coverage).expect("valid domain")
    }

    fn complete_window() -> WindowAccumulator {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        window
            .observe(Observation::accepted(100, 10, 11))
            .expect("bucket 100");
        window
            .observe(Observation::accepted(101, 20, 21))
            .expect("bucket 101");
        window
            .observe(Observation::accepted(102, 30, 31))
            .expect("bucket 102");
        window.witness_feed_cursor(104).expect("monotone");
        window
    }

    #[test]
    fn sealed_complete_window_produces_domain_bound_result() {
        let mut window = complete_window();
        assert_eq!(window.phase(), WindowPhase::Mature);
        window.seal().expect("mature");
        let result = window.result().expect("sealed and complete");
        assert_eq!(result.phase(), WindowPhase::Sealed);
        assert_eq!(result.sealed_cursor(), 104);
        assert_eq!(result.coverage().total_buckets(), 3);
        assert_eq!(result.coverage().accepted_buckets(), 3);
        assert_eq!(result.coverage().state(), CoverageState::Complete);
        assert_eq!(
            result.terminal().expect("accepted coverage"),
            ValueInterval::new(30, 31).expect("valid interval")
        );
        assert_eq!(
            result.check_domain(&domain(CoveragePolicy::COMPLETE_REQUIRED)),
            Ok(())
        );
    }

    #[test]
    fn truncated_prefix_cannot_seal_or_yield_a_result() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        window
            .observe(Observation::accepted(100, 10, 11))
            .expect("bucket 100");
        window
            .observe(Observation::accepted(101, 20, 21))
            .expect("bucket 101");
        window.witness_feed_cursor(104).expect("monotone");
        // The prefix statistic exists on the bare summary; the window refuses.
        assert_eq!(window.phase(), WindowPhase::Open);
        assert_eq!(window.seal(), Err(WindowError::IncompleteDomain));
        assert_eq!(window.result(), Err(WindowError::NotSealed));
    }

    #[test]
    fn early_window_refuses_before_the_maturity_bound() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        for bucket in 100..103 {
            window
                .observe(Observation::accepted(bucket, 10, 11))
                .expect("in domain");
        }
        assert_eq!(window.feed_cursor(), 103);
        assert_eq!(window.phase(), WindowPhase::Open);
        assert_eq!(window.seal(), Err(WindowError::NotMature));
        window.witness_feed_cursor(103).expect("equal is monotone");
        assert_eq!(window.seal(), Err(WindowError::NotMature));
        window.witness_feed_cursor(104).expect("advance");
        window.seal().expect("mature");
    }

    #[test]
    fn authenticated_closing_boundary_witness_can_seal_with_the_closing_cursor() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        for bucket in 100..103 {
            window
                .observe(Observation::accepted(bucket, 10, 11))
                .expect("in domain");
        }
        assert_eq!(window.feed_cursor(), 103);
        assert_eq!(window.phase(), WindowPhase::Open);
        assert_eq!(window.seal(), Err(WindowError::NotMature));

        window
            .witness_authenticated_closing_boundary(103)
            .expect("authenticated source generation proved the closing boundary");
        assert_eq!(window.phase(), WindowPhase::Mature);
        window.seal().expect("closing-boundary witness matures");
        let result = window.result().expect("sealed");
        assert_eq!(result.sealed_cursor(), 103);
        assert_eq!(
            result.check_domain(&domain(CoveragePolicy::COMPLETE_REQUIRED)),
            Ok(())
        );
    }

    #[test]
    fn closing_boundary_witness_is_not_a_prefix_or_backwards_escape() {
        let mut prefix = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        prefix
            .observe(Observation::accepted(100, 10, 11))
            .expect("in domain");
        assert_eq!(
            prefix.witness_authenticated_closing_boundary(102),
            Err(WindowError::NotMature)
        );
        assert_eq!(prefix.seal(), Err(WindowError::IncompleteDomain));

        let mut advanced = complete_window();
        advanced
            .witness_feed_cursor(104)
            .expect("ordinary maturity");
        assert_eq!(
            advanced.witness_authenticated_closing_boundary(103),
            Err(WindowError::NonMonotoneCursor)
        );
    }

    #[test]
    fn gap_tolerant_substitution_is_refused_by_the_registered_policy() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        window
            .observe(Observation::accepted(100, 10, 11))
            .expect("bucket 100");
        window.observe(Observation::missing(101)).expect("gap");
        window
            .observe(Observation::accepted(102, 30, 31))
            .expect("bucket 102");
        window.witness_feed_cursor(104).expect("monotone");
        window.seal().expect("complete range and mature");
        // The bare summary still answers `terminal`; the window does not.
        assert_eq!(
            window.summary.terminal().expect("accepted coverage"),
            ValueInterval::new(30, 31).expect("valid interval")
        );
        assert_eq!(window.result(), Err(WindowError::CoverageRefused));
    }

    #[test]
    fn a_bounded_gap_result_cannot_stand_in_for_a_complete_one() {
        let tolerant = CoveragePolicy::bounded_gaps(1).expect("registered policy");
        let mut window = WindowAccumulator::open(domain(tolerant));
        window
            .observe(Observation::accepted(100, 10, 11))
            .expect("bucket 100");
        window.observe(Observation::missing(101)).expect("gap");
        window
            .observe(Observation::accepted(102, 30, 31))
            .expect("bucket 102");
        window.witness_feed_cursor(104).expect("monotone");
        window.seal().expect("mature");
        let result = window.result().expect("policy admits one gap");
        assert_eq!(result.coverage().missing_buckets(), 1);
        assert_eq!(result.check_domain(&domain(tolerant)), Ok(()));
        assert_eq!(
            result.check_domain(&domain(CoveragePolicy::COMPLETE_REQUIRED)),
            Err(WindowError::MismatchedCoveragePolicy)
        );
        let wider = CoveragePolicy::bounded_gaps(2).expect("registered policy");
        assert_eq!(
            result.check_domain(&domain(wider)),
            Err(WindowError::MismatchedCoveragePolicy)
        );
    }

    #[test]
    fn a_fully_missing_bounded_gap_window_still_refuses_statistics() {
        let tolerant = CoveragePolicy::bounded_gaps(3).expect("registered policy");
        let mut window = WindowAccumulator::open(domain(tolerant));
        for bucket in 100..103 {
            window.observe(Observation::missing(bucket)).expect("gap");
        }
        window.witness_feed_cursor(104).expect("monotone");
        window.seal().expect("mature");
        let result = window.result().expect("policy admits three gaps");
        assert_eq!(result.terminal(), Err(StatisticError::NoAcceptedCoverage));
        assert_eq!(result.twap(), Err(StatisticError::NoAcceptedCoverage));
        assert_eq!(result.price_time_integral(), None);
    }

    #[test]
    fn wrong_window_generation_maturity_and_feed_are_distinguishable_refusals() {
        let mut window = complete_window();
        window.seal().expect("mature");
        let result = window.result().expect("sealed");
        let expected = domain(CoveragePolicy::COMPLETE_REQUIRED);

        let other_range = WindowDomain::new(
            feed(),
            grid(),
            200,
            203,
            204,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid domain");
        assert_eq!(
            result.check_domain(&other_range),
            Err(WindowError::WrongWindow)
        );

        let other_generation = WindowDomain::new(
            feed(),
            grid(),
            100,
            103,
            104,
            1,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid domain");
        assert_eq!(
            result.check_domain(&other_generation),
            Err(WindowError::MismatchedGeneration)
        );

        let other_maturity = WindowDomain::new(
            feed(),
            grid(),
            100,
            103,
            110,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid domain");
        assert_eq!(
            result.check_domain(&other_maturity),
            Err(WindowError::MismatchedMaturity)
        );

        let other_evaluator = FeedIdentity::new([1; IDENTITY_BYTES], [2; IDENTITY_BYTES], 3, 5)
            .expect("valid identity");
        let other_feed = WindowDomain::new(
            other_evaluator,
            grid(),
            100,
            103,
            104,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid domain");
        assert_eq!(
            result.check_domain(&other_feed),
            Err(WindowError::MismatchedFeed)
        );

        let other_grid = Grid::new(7, 2, 60).expect("valid grid");
        let regridded = WindowDomain::new(
            feed(),
            other_grid,
            100,
            103,
            104,
            0,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .expect("valid domain");
        assert_eq!(
            result.check_domain(&regridded),
            Err(WindowError::MismatchedGrid)
        );

        assert_eq!(result.check_domain(&expected), Ok(()));
    }

    #[test]
    fn a_sealed_window_cannot_be_reopened_or_resealed() {
        let mut window = complete_window();
        window.seal().expect("mature");
        assert_eq!(window.phase(), WindowPhase::Sealed);
        assert_eq!(window.seal(), Err(WindowError::AlreadySealed));
        assert_eq!(
            window.observe(Observation::accepted(103, 1, 1)),
            Err(WindowError::ObservationAfterSeal)
        );
        assert_eq!(
            window.witness_feed_cursor(200),
            Err(WindowError::ObservationAfterSeal)
        );
        let first = window.result().expect("sealed");
        let second = window.result().expect("sealed");
        assert_eq!(first, second);
        assert_eq!(first.sealed_cursor(), 104);
    }

    #[test]
    fn pages_must_be_contiguous_in_grid_and_range() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        assert_eq!(
            window.observe(Observation::accepted(101, 1, 1)),
            Err(WindowError::NonContiguous)
        );
        let other_grid = Grid::new(9, 1, 60).expect("valid grid");
        let foreign = Summary::singleton(other_grid, Observation::accepted(100, 1, 1))
            .expect("valid singleton");
        assert_eq!(window.absorb(foreign), Err(WindowError::MismatchedGrid));
        // The identity page is admitted and changes nothing.
        window
            .absorb(Summary::empty(grid()))
            .expect("identity absorb");
        assert_eq!(window.cursor(), 100);

        let mut overrun = Summary::empty(grid());
        for bucket in 100..105 {
            overrun = overrun
                .append(Observation::accepted(bucket, 1, 1))
                .expect("valid append");
        }
        assert_eq!(window.absorb(overrun), Err(WindowError::RangeOverflow));

        let mut page = Summary::empty(grid());
        for bucket in 100..103 {
            page = page
                .append(Observation::accepted(bucket, 5, 6))
                .expect("valid append");
        }
        window.absorb(page).expect("exact page");
        assert_eq!(window.cursor(), 103);
        assert_eq!(window.feed_cursor(), 103);
    }

    #[test]
    fn a_witnessed_feed_cursor_cannot_move_backwards() {
        let mut window = WindowAccumulator::open(domain(CoveragePolicy::COMPLETE_REQUIRED));
        window.witness_feed_cursor(150).expect("advance");
        assert_eq!(
            window.witness_feed_cursor(149),
            Err(WindowError::NonMonotoneCursor)
        );
        assert_eq!(window.feed_cursor(), 150);
    }

    #[test]
    fn domain_construction_refuses_degenerate_identity_range_and_maturity() {
        assert_eq!(
            FeedIdentity::new([0; IDENTITY_BYTES], [2; IDENTITY_BYTES], 1, 1),
            Err(WindowError::ZeroIdentity)
        );
        assert_eq!(
            FeedIdentity::new([1; IDENTITY_BYTES], [0; IDENTITY_BYTES], 1, 1),
            Err(WindowError::ZeroIdentity)
        );
        assert_eq!(
            FeedIdentity::new([1; IDENTITY_BYTES], [2; IDENTITY_BYTES], 0, 1),
            Err(WindowError::UnversionedIdentity)
        );
        assert_eq!(
            FeedIdentity::new([1; IDENTITY_BYTES], [2; IDENTITY_BYTES], 1, 0),
            Err(WindowError::UnversionedIdentity)
        );
        assert_eq!(
            WindowDomain::new(
                feed(),
                grid(),
                100,
                100,
                100,
                0,
                CoveragePolicy::COMPLETE_REQUIRED
            ),
            Err(WindowError::InvalidRange)
        );
        assert_eq!(
            WindowDomain::new(
                feed(),
                grid(),
                0,
                MAX_BUCKETS + 1,
                MAX_BUCKETS + 1,
                0,
                CoveragePolicy::COMPLETE_REQUIRED
            ),
            Err(WindowError::InvalidRange)
        );
        assert_eq!(
            WindowDomain::new(
                feed(),
                grid(),
                100,
                103,
                102,
                0,
                CoveragePolicy::COMPLETE_REQUIRED
            ),
            Err(WindowError::InvalidMaturity)
        );
    }

    #[test]
    fn the_coverage_registry_is_closed_and_has_no_parallel_identifiers() {
        assert_eq!(
            CoveragePolicy::bounded_gaps(0),
            Err(WindowError::InvalidPolicyParameter)
        );
        assert_eq!(
            CoveragePolicy::bounded_gaps(MAX_BUCKETS + 1),
            Err(WindowError::InvalidPolicyParameter)
        );
        assert_eq!(
            CoveragePolicy::from_registry(COVERAGE_POLICY_COMPLETE_REQUIRED, 0),
            Ok(CoveragePolicy::COMPLETE_REQUIRED)
        );
        assert_eq!(
            CoveragePolicy::from_registry(COVERAGE_POLICY_COMPLETE_REQUIRED, 1),
            Err(WindowError::InvalidPolicyParameter)
        );
        assert_eq!(
            CoveragePolicy::from_registry(COVERAGE_POLICY_BOUNDED_GAPS, 0),
            Err(WindowError::InvalidPolicyParameter)
        );
        assert_eq!(
            CoveragePolicy::from_registry(3, 0),
            Err(WindowError::UnknownCoveragePolicy)
        );
        assert_eq!(
            CoveragePolicy::from_registry(0, 0),
            Err(WindowError::UnknownCoveragePolicy)
        );
    }

    #[test]
    fn canonical_domain_bytes_separate_every_field() {
        let base = domain(CoveragePolicy::COMPLETE_REQUIRED);
        let mut bytes = [0u8; WINDOW_DOMAIN_BYTES];
        base.encode_canonical(&mut bytes);
        assert_eq!(&bytes[..8], &WINDOW_DOMAIN_MAGIC);
        assert_eq!(
            &bytes[WINDOW_DOMAIN_BYTES - WINDOW_DOMAIN_RESERVED_BYTES..],
            &[0u8; WINDOW_DOMAIN_RESERVED_BYTES]
        );

        let variants = [
            WindowDomain::new(
                feed(),
                grid(),
                100,
                103,
                104,
                1,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .expect("valid"),
            WindowDomain::new(
                feed(),
                grid(),
                100,
                103,
                110,
                0,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .expect("valid"),
            WindowDomain::new(
                feed(),
                grid(),
                101,
                103,
                104,
                0,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .expect("valid"),
            WindowDomain::new(
                feed(),
                grid(),
                100,
                103,
                104,
                0,
                CoveragePolicy::bounded_gaps(1).expect("registered"),
            )
            .expect("valid"),
            WindowDomain::new(
                FeedIdentity::new([1; IDENTITY_BYTES], [2; IDENTITY_BYTES], 3, 5)
                    .expect("valid identity"),
                grid(),
                100,
                103,
                104,
                0,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .expect("valid"),
            WindowDomain::new(
                feed(),
                Grid::new(7, 1, 61).expect("valid grid"),
                100,
                103,
                104,
                0,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .expect("valid"),
        ];
        for variant in variants {
            let mut other = [0u8; WINDOW_DOMAIN_BYTES];
            variant.encode_canonical(&mut other);
            assert_ne!(bytes, other);
        }
    }

    #[test]
    fn window_results_do_not_invent_unsupported_statistics() {
        let mut window = complete_window();
        window.seal().expect("mature");
        let result = window.result().expect("sealed");
        assert_eq!(
            result.threshold_crossings(20),
            Err(StatisticError::UnsupportedPredicate)
        );
        assert_eq!(
            result.maximum_drawdown(),
            Err(StatisticError::UnsupportedPredicate)
        );
        assert_eq!(
            result.realized_variance(),
            Err(StatisticError::UnsupportedPredicate)
        );
        assert!(result.sampled_min().is_some());
        assert!(result.sampled_max().is_some());
        assert!(result.relative_terminal_to_twap().is_ok());
    }
}
