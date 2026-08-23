#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Canonical host compiler and pure recurring-Series model.
//!
//! This crate is intentionally outside the SBF dispatcher. It gives the
//! proposed Template/Instance/Series objects executable identity, binding,
//! funding, and recurrence semantics while preserving the current account
//! plane as an explicit compatibility lowering.

use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, WindowDomain, MAX_VALUE, WINDOW_DOMAIN_BYTES,
    WINDOW_DOMAIN_TAG,
};
use clutch_liquidity_policy_model::{
    LiquidityPolicyV1, NativeTermsV1, MAX_ACCOUNTING_ATOMS as LIQUIDITY_MAX_ATOMS,
};
use clutch_solana_layout::{
    canonical_market_id, canonical_outcome_id, CodecError, Hash32, MarketAccount,
    PayoutVectorBytes, PriceGridAccount, ProfileAccount, RealmAccount, TermsAccount, MAX_KNOTS,
    MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
};
use sha2::{Digest, Sha256};

/// Opaque canonical identity used by compiler artifacts.
pub type Id = [u8; 32];

/// Maximum number of Instances one V1 Series may schedule.
pub const MAX_SERIES_INSTANCES: u32 = 65_536;
/// Terminal statistic tag in the Template compiler language.
pub const STATISTIC_TERMINAL_V1: u16 = 1;
/// Maximum-drawdown statistic tag in the Template compiler language.
pub const STATISTIC_MAXIMUM_DRAWDOWN_V1: u16 = 2;
/// Summary capability bit for a conservative terminal interval.
pub const FEATURE_TERMINAL_INTERVAL: u64 = 1 << 0;
/// Summary capability bit for a conservative maximum-drawdown interval.
pub const FEATURE_MAXIMUM_DRAWDOWN_INTERVAL: u64 = 1 << 1;
/// Exact scale for maximum-drawdown results: one whole is one million ppm.
pub const DRAWDOWN_PPM_SCALE: u64 = 1_000_000;
/// Current registered failure policy whose selected vector must be uniform.
pub const FAILURE_UNIFORM_REFUND_V1: u32 = 1;

const SUMMARY_PROGRAM_DOMAIN: &[u8] = b"dragons-clutch/summary-program/v1";
const HATCHERY_PROGRAM_DOMAIN: &[u8] = b"dragons-clutch/hatchery-program/v1";
const TEMPLATE_DOMAIN: &[u8] = b"dragons-clutch/template/v1";
const TEMPLATE_PRESENTATION_DOMAIN: &[u8] = b"dragons-clutch/template-presentation/v1";
const WORK_ENVELOPE_DOMAIN: &[u8] = b"dragons-clutch/work-envelope/v1";
const LIQUIDITY_BLUEPRINT_DOMAIN: &[u8] = b"dragons-clutch/liquidity-blueprint/v1";
const SERIES_DOMAIN: &[u8] = b"dragons-clutch/series/v1";
const INSTANCE_DOMAIN: &[u8] = b"dragons-clutch/instance/v1";
const INSTANCE_EPOCH_DOMAIN: &[u8] = b"dragons-clutch/instance-epoch/v1";
const INSTANCE_OUTCOME_DOMAIN: &[u8] = b"dragons-clutch/instance-outcome/v1";
const HATCHERY_WINDOW_DOMAIN: &[u8] = b"dragons-clutch/hatchery-window/v1";
const STATISTIC_RESULT_DOMAIN: &[u8] = b"dragons-clutch/statistic-result/v1";
const LIQUIDITY_POLICY_DOMAIN: &[u8] = b"dragons-clutch/liquidity-policy/v1";

/// A checked refusal from the host compiler/model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A required content or account identity was all zero.
    ZeroIdentity,
    /// A version, count, amount, span, or policy parameter was zero or invalid.
    InvalidParameter,
    /// Fixed-width inactive entries were not canonical padding.
    NonCanonicalPadding,
    /// A statistic tag or requested feature is not registered.
    UnsupportedStatistic,
    /// A referenced content artifact does not match the supplied artifact.
    MismatchedArtifact,
    /// A checked time, nonce, funding, or count operation overflowed.
    ArithmeticOverflow,
    /// The requested Instance is not the Series' next ordinal.
    WrongOrdinal,
    /// An epoch request did not name the Instance's monotone next index.
    WrongEpoch,
    /// The next Instance is outside its permissionless creation interval.
    NotEligible,
    /// Every bounded Series ordinal has already been consumed or lapsed.
    SeriesExhausted,
    /// One or more prepaid compartments cannot cover the remaining schedule.
    InsufficientPrepayment,
    /// The current Terms/statistic registry cannot represent this valid model.
    UnsupportedCurrentLowering,
    /// Current SourceArchive/Feed state is one-window and cannot host Series.
    CurrentSourcePlaneNotRecurring,
    /// The supplied current account projection failed its canonical codec.
    CurrentLayout(CodecError),
    /// The existing liquidity-policy model refused the bound policy.
    Liquidity(clutch_liquidity_policy_model::Error),
}

impl From<CodecError> for Error {
    fn from(error: CodecError) -> Self {
        Self::CurrentLayout(error)
    }
}

impl From<clutch_liquidity_policy_model::Error> for Error {
    fn from(error: clutch_liquidity_policy_model::Error) -> Self {
        Self::Liquidity(error)
    }
}

/// Result alias for compiler/model operations.
pub type Result<T> = core::result::Result<T, Error>;

fn is_zero(id: &Id) -> bool {
    id.iter().all(|byte| *byte == 0)
}

fn check_id(id: &Id) -> Result<()> {
    if is_zero(id) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn content_id(domain: &[u8], body: &[u8]) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    hasher.finalize().into()
}

fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// The closed statistic language admitted by Template V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum StatisticProgramV1 {
    /// Conservative interval of the last accepted observation.
    TerminalInterval = STATISTIC_TERMINAL_V1,
    /// Conservative maximum peak-to-subsequent-trough drawdown interval.
    MaximumDrawdownInterval = STATISTIC_MAXIMUM_DRAWDOWN_V1,
}

/// A source-plane capability required by permissionless recurring Series.
pub const HATCHERY_SOURCE_ONLY_HEAD: u32 = 1 << 0;
/// A source-plane capability required to reuse observations across windows.
pub const HATCHERY_REUSABLE_RAW_PAGES: u32 = 1 << 1;
/// A source-plane capability required to share one feed across Realms.
pub const HATCHERY_REALM_NEUTRAL_FEED: u32 = 1 << 2;
/// A source-plane capability required to derive several statistics per window.
pub const HATCHERY_STATISTIC_RESULTS: u32 = 1 << 3;
/// Exact V1 capability set required by this recurring compiler model.
pub const HATCHERY_RECURRING_REQUIRED: u32 = HATCHERY_SOURCE_ONLY_HEAD
    | HATCHERY_REUSABLE_RAW_PAGES
    | HATCHERY_REALM_NEUTRAL_FEED
    | HATCHERY_STATISTIC_RESULTS;

/// Content-addressed source/window storage program consumed by Series.
///
/// Version 3 names the first proposed generation in which FeedHead is
/// source-only, raw pages are reusable, windows are independent immutable
/// objects, and statistic results are derived children. Current V1/V2 source
/// ingestion does not satisfy this contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HatcheryProgramV1 {
    /// Reviewed source-plane implementation/release digest.
    pub release_id: Id,
    /// Source-plane account/transition generation; recurring requires >= 3.
    pub source_plane_version: u32,
    /// Raw observation-page codec version.
    pub raw_page_version: u32,
    /// Immutable window-result codec version.
    pub window_result_version: u32,
    /// Maximum observation records one exact window may reference.
    pub max_window_records: u32,
    /// Closed capability set.
    pub capabilities: u32,
}

impl HatcheryProgramV1 {
    /// Validate the exact minimum contract needed for recurring/shared windows.
    pub fn validate_recurring(&self) -> Result<()> {
        check_id(&self.release_id)?;
        if self.source_plane_version < 3
            || self.raw_page_version == 0
            || self.window_result_version == 0
            || self.max_window_records < 2
            || self.capabilities != HATCHERY_RECURRING_REQUIRED
        {
            return Err(Error::CurrentSourcePlaneNotRecurring);
        }
        Ok(())
    }

    /// Canonical content identity of this reviewed source-plane contract.
    pub fn id(&self) -> Result<Id> {
        self.validate_recurring()?;
        let mut body = Vec::with_capacity(64);
        body.extend_from_slice(b"DCHATCV1");
        body.extend_from_slice(&self.release_id);
        push_u32(&mut body, self.source_plane_version);
        push_u32(&mut body, self.raw_page_version);
        push_u32(&mut body, self.window_result_version);
        push_u32(&mut body, self.max_window_records);
        push_u32(&mut body, self.capabilities);
        Ok(content_id(HATCHERY_PROGRAM_DOMAIN, &body))
    }
}

impl StatisticProgramV1 {
    fn required_feature(self) -> u64 {
        match self {
            Self::TerminalInterval => FEATURE_TERMINAL_INTERVAL,
            Self::MaximumDrawdownInterval => FEATURE_MAXIMUM_DRAWDOWN_INTERVAL,
        }
    }
}

/// Conservative inclusive maximum-drawdown interval in integer ppm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawdownIntervalV1 {
    /// Lower bound in `0..=1_000_000` ppm.
    pub low_ppm: u64,
    /// Upper bound in `0..=1_000_000` ppm.
    pub high_ppm: u64,
}

/// Ordered associative summary for conservative maximum peak-to-subsequent-
/// trough drawdown.
///
/// This is a distinct feature family over reusable raw observations. It does
/// not weaken `clutch_accumulator::Summary::maximum_drawdown`, which correctly
/// refuses because that older summary discarded order. Combining requires
/// adjacent ranges and is directional: `left` occurred before `right`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawdownSummaryV1 {
    start_bucket: u64,
    end_bucket_exclusive: u64,
    record_count: u64,
    maximum_low: u128,
    maximum_high: u128,
    minimum_low: u128,
    minimum_high: u128,
    drawdown_low_ppm: u64,
    drawdown_high_ppm: u64,
}

impl DrawdownSummaryV1 {
    /// Construct one ordered raw observation. A single observation has zero
    /// drawdown even when its own confidence interval is non-point evidence.
    pub fn observation(bucket: u64, low: u128, high: u128) -> Result<Self> {
        if low > high || high > MAX_VALUE || bucket == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            start_bucket: bucket,
            end_bucket_exclusive: bucket + 1,
            record_count: 1,
            maximum_low: low,
            maximum_high: high,
            minimum_low: low,
            minimum_high: high,
            drawdown_low_ppm: 0,
            drawdown_high_ppm: 0,
        })
    }

    /// Combine two adjacent summaries in chronological order.
    pub fn combine(self, later: Self) -> Result<Self> {
        if self.end_bucket_exclusive != later.start_bucket {
            return Err(Error::InvalidParameter);
        }
        let cross_low = drawdown_ppm(self.maximum_low, later.minimum_high)?;
        let cross_high = drawdown_ppm(self.maximum_high, later.minimum_low)?;
        Ok(Self {
            start_bucket: self.start_bucket,
            end_bucket_exclusive: later.end_bucket_exclusive,
            record_count: self
                .record_count
                .checked_add(later.record_count)
                .ok_or(Error::ArithmeticOverflow)?,
            maximum_low: self.maximum_low.max(later.maximum_low),
            maximum_high: self.maximum_high.max(later.maximum_high),
            minimum_low: self.minimum_low.min(later.minimum_low),
            minimum_high: self.minimum_high.min(later.minimum_high),
            drawdown_low_ppm: self
                .drawdown_low_ppm
                .max(later.drawdown_low_ppm)
                .max(cross_low),
            drawdown_high_ppm: self
                .drawdown_high_ppm
                .max(later.drawdown_high_ppm)
                .max(cross_high),
        })
    }

    /// Conservative drawdown interval of this complete ordered range.
    pub const fn result(self) -> DrawdownIntervalV1 {
        DrawdownIntervalV1 {
            low_ppm: self.drawdown_low_ppm,
            high_ppm: self.drawdown_high_ppm,
        }
    }

    /// Inclusive first bucket in the ordered range.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Exclusive end bucket in the ordered range.
    pub const fn end_bucket_exclusive(self) -> u64 {
        self.end_bucket_exclusive
    }

    /// Number of represented raw observation records.
    pub const fn record_count(self) -> u64 {
        self.record_count
    }
}

fn drawdown_ppm(peak: u128, trough: u128) -> Result<u64> {
    if peak == 0 || peak <= trough {
        return Ok(0);
    }
    let numerator = (peak - trough)
        .checked_mul(u128::from(DRAWDOWN_PPM_SCALE))
        .ok_or(Error::ArithmeticOverflow)?;
    let quotient = numerator / peak;
    let rounded = quotient
        .checked_add(u128::from(numerator % peak != 0))
        .ok_or(Error::ArithmeticOverflow)?;
    u64::try_from(rounded).map_err(|_| Error::ArithmeticOverflow)
}

/// Normalized, externally authenticated projection of one immutable SourceSpec.
///
/// The SourceSpec remains the semantic owner. This view is never persisted by
/// the compiler and cannot authenticate itself; the adapter constructing it
/// must first verify the exact SourceSpec account/body and its provider pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecViewV1 {
    /// Canonical SourceSpec/feed identity.
    pub source_spec_id: Id,
    /// Reviewed source-adapter implementation identity.
    pub source_adapter_id: Id,
    /// Source-adapter version.
    pub source_version: u32,
    /// Observation grid family.
    pub grid_family_id: u32,
    /// Observation grid version.
    pub grid_version: u16,
    /// Exact bucket width in seconds.
    pub bucket_seconds: u64,
}

impl SourceSpecViewV1 {
    /// Check the normalized projection's structural shape.
    pub fn validate(&self) -> Result<()> {
        check_id(&self.source_spec_id)?;
        check_id(&self.source_adapter_id)?;
        if self.source_version == 0 || self.grid_family_id == 0 || self.grid_version == 0 {
            return Err(Error::InvalidParameter);
        }
        Grid::new(self.grid_family_id, self.grid_version, self.bucket_seconds)
            .map_err(|_| Error::InvalidParameter)?;
        Ok(())
    }
}

/// Content-addressed summary/evaluator program and its exact feature family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummaryProgramV1 {
    /// Reviewed evaluator implementation digest.
    pub evaluator_program_id: Id,
    /// Version interpreted by the runtime evaluator registry.
    pub evaluator_version: u32,
    /// Closed capability bitset; only the two published V1 bits are admitted.
    pub feature_mask: u64,
}

impl SummaryProgramV1 {
    /// Check nonzero identity/version and the closed feature bitset.
    pub fn validate(&self) -> Result<()> {
        check_id(&self.evaluator_program_id)?;
        if self.evaluator_version == 0
            || self.feature_mask == 0
            || self.feature_mask & !(FEATURE_TERMINAL_INTERVAL | FEATURE_MAXIMUM_DRAWDOWN_INTERVAL)
                != 0
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Canonical body bytes used by [`SummaryProgramV1::id`].
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut out = Vec::with_capacity(44);
        out.extend_from_slice(b"DCSUMV1\0");
        out.extend_from_slice(&self.evaluator_program_id);
        push_u32(&mut out, self.evaluator_version);
        push_u64(&mut out, self.feature_mask);
        Ok(out)
    }

    /// Content identity of the exact summary-program body.
    pub fn id(&self) -> Result<Id> {
        Ok(content_id(SUMMARY_PROGRAM_DOMAIN, &self.canonical_bytes()?))
    }

    fn supports(&self, statistic: StatisticProgramV1) -> bool {
        self.feature_mask & statistic.required_feature() != 0
    }
}

/// Complete relative market semantics that create no time-specific liability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateV1 {
    /// Canonical SourceSpec identity; source details are not copied here.
    pub source_spec_id: Id,
    /// Recurrence-capable Hatchery/source-plane program identity.
    pub hatchery_program_id: Id,
    /// Canonical SummaryProgram identity; evaluator details are not copied here.
    pub summary_program_id: Id,
    /// Digest of the canonical human-readable presentation sidecar.
    ///
    /// It is committed by [`TemplateV1::presentation_id`] but deliberately
    /// excluded from [`TemplateV1::id`], so relabeling cannot fork otherwise
    /// identical market semantics.
    pub presentation_digest: Id,
    /// Host compiler schema/version.
    pub compiler_version: u32,
    /// Number of observation buckets in each exact Instance window.
    pub window_span_buckets: u64,
    /// Additional buckets after window end before evidence may seal.
    pub repair_grace_buckets: u64,
    /// Exact page-repair generation admitted by each WindowResult.
    pub repair_generation: u64,
    /// Registered coverage policy.
    pub coverage_policy_id: u16,
    /// Registered coverage-policy parameter.
    pub coverage_policy_parameter: u64,
    /// Closed statistic evaluator.
    pub statistic: StatisticProgramV1,
    /// Registered ambiguity policy.
    pub ambiguity_policy_id: u8,
    /// Registered edge policy.
    pub edge_policy_id: u8,
    /// Registered repair policy.
    pub repair_policy_id: u32,
    /// Registered failure policy.
    pub failure_policy_id: u32,
    /// B-spline basis degree.
    pub basis_degree: u8,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Active payout-vector count.
    pub payout_count: u8,
    /// Payout vectors with canonical zero padding.
    pub payouts: [PayoutVectorBytes; MAX_PAYOUTS],
    /// Active knot count.
    pub knot_count: u8,
    /// Uniform-spacing declaration used by current Terms.
    pub uniform_log2_spacing: u8,
    /// Failure payout-vector index.
    pub failure_payout_index: u8,
    /// Degree-zero cell-to-payout map, or all-unused for smooth bases.
    pub payout_map: [u8; MAX_OUTCOMES],
    /// Active knots followed by canonical zero padding.
    pub knots: [u128; MAX_KNOTS],
}

impl TemplateV1 {
    /// Validate content references, window recipe, coverage, and every
    /// partition/payout rule already owned by current `TermsAccount`.
    pub fn validate(&self) -> Result<()> {
        check_id(&self.source_spec_id)?;
        check_id(&self.hatchery_program_id)?;
        check_id(&self.summary_program_id)?;
        check_id(&self.presentation_digest)?;
        if self.compiler_version == 0 || self.window_span_buckets == 0 {
            return Err(Error::InvalidParameter);
        }
        let maturity = self
            .window_span_buckets
            .checked_add(self.repair_grace_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        CoveragePolicy::from_registry(self.coverage_policy_id, self.coverage_policy_parameter)
            .map_err(|_| Error::InvalidParameter)?;

        // TermsAccount is the existing semantic owner of payout/knot byte
        // shape. A harmless dummy namespace lets this host artifact reuse that
        // validator instead of creating a subtly different parallel truth.
        let mut terms = TermsAccount {
            terms: Hash32::ZERO,
            realm: Hash32::from_bytes([1; 32]),
            profile: Hash32::from_bytes([2; 32]),
            feed: Hash32::from_bytes([3; 32]),
            price_grid: Hash32::from_bytes([4; 32]),
            outcome_count: self.outcome_count,
            payout_count: self.payout_count,
            payouts: self.payouts,
            grid_family_id: 1,
            grid_version: 1,
            bucket_seconds: 1,
            expected_start_bucket: 1,
            expected_end_bucket_exclusive: self
                .window_span_buckets
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            maturity_horizon_buckets: maturity,
            coverage_policy_id: u32::from(self.coverage_policy_id),
            repair_policy_id: self.repair_policy_id,
            failure_policy_id: self.failure_policy_id,
            statistic_id: 1,
            ambiguity_policy_id: self.ambiguity_policy_id,
            edge_policy_id: self.edge_policy_id,
            basis_degree: self.basis_degree,
            knot_count: self.knot_count,
            uniform_log2_spacing: self.uniform_log2_spacing,
            failure_payout_index: self.failure_payout_index,
            coverage_policy_parameter: self.coverage_policy_parameter,
            repair_generation: self.repair_generation,
            source_version: 1,
            evaluator_version: 1,
            source_adapter_id: Hash32::from_bytes([5; 32]),
            payout_map: self.payout_map,
            knots: self.knots,
            collateral_cap: 1,
            stored_bump: 0,
            flags: 0,
        };
        terms.terms = terms.recomputed_terms_digest()?;
        terms.validate()?;
        if self.failure_policy_id == FAILURE_UNIFORM_REFUND_V1 {
            let failure = self.payouts[usize::from(self.failure_payout_index)];
            let first = failure.weights[0];
            if first == 0
                || failure.weights[..usize::from(self.outcome_count)]
                    .iter()
                    .any(|weight| *weight != first)
            {
                return Err(Error::InvalidParameter);
            }
        }
        if self.statistic == StatisticProgramV1::MaximumDrawdownInterval {
            if self.coverage_policy_id != 1 || self.coverage_policy_parameter != 0 {
                return Err(Error::InvalidParameter);
            }
            if self.knots[..usize::from(self.knot_count)]
                .iter()
                .any(|knot| *knot > u128::from(DRAWDOWN_PPM_SCALE))
            {
                return Err(Error::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Validate this Template against the exact referenced source and summary
    /// artifacts, including the required statistic feature.
    pub fn validate_bindings(
        &self,
        source: &SourceSpecViewV1,
        summary: &SummaryProgramV1,
        hatchery: &HatcheryProgramV1,
    ) -> Result<()> {
        self.validate()?;
        source.validate()?;
        summary.validate()?;
        hatchery.validate_recurring()?;
        if source.source_spec_id != self.source_spec_id
            || summary.id()? != self.summary_program_id
            || hatchery.id()? != self.hatchery_program_id
        {
            return Err(Error::MismatchedArtifact);
        }
        if !summary.supports(self.statistic) {
            return Err(Error::UnsupportedStatistic);
        }
        if self.window_span_buckets > u64::from(hatchery.max_window_records) {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Canonical body bytes used by [`TemplateV1::id`].
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut out = Vec::with_capacity(1_500);
        out.extend_from_slice(b"DCTPLV1\0");
        out.extend_from_slice(&self.source_spec_id);
        out.extend_from_slice(&self.hatchery_program_id);
        out.extend_from_slice(&self.summary_program_id);
        push_u32(&mut out, self.compiler_version);
        push_u64(&mut out, self.window_span_buckets);
        push_u64(&mut out, self.repair_grace_buckets);
        push_u64(&mut out, self.repair_generation);
        push_u16(&mut out, self.coverage_policy_id);
        push_u64(&mut out, self.coverage_policy_parameter);
        push_u16(&mut out, self.statistic as u16);
        push_u8(&mut out, self.ambiguity_policy_id);
        push_u8(&mut out, self.edge_policy_id);
        push_u32(&mut out, self.repair_policy_id);
        push_u32(&mut out, self.failure_policy_id);
        push_u8(&mut out, self.basis_degree);
        push_u8(&mut out, self.outcome_count);
        push_u8(&mut out, self.payout_count);
        for payout in self.payouts {
            push_u64(&mut out, payout.denominator);
            for weight in payout.weights {
                push_u64(&mut out, weight);
            }
        }
        push_u8(&mut out, self.knot_count);
        push_u8(&mut out, self.uniform_log2_spacing);
        push_u8(&mut out, self.failure_payout_index);
        out.extend_from_slice(&self.payout_map);
        for knot in self.knots {
            push_u128(&mut out, knot);
        }
        Ok(out)
    }

    /// Content identity of the exact immutable Template body.
    pub fn id(&self) -> Result<Id> {
        Ok(content_id(TEMPLATE_DOMAIN, &self.canonical_bytes()?))
    }

    /// Presentation-manifest identity. It commits the semantic TemplateId and
    /// human sidecar digest without changing economic equivalence.
    pub fn presentation_id(&self) -> Result<Id> {
        self.validate()?;
        let template_id = self.id()?;
        let mut body = Vec::with_capacity(72);
        body.extend_from_slice(b"DCTPRSV1");
        body.extend_from_slice(&template_id);
        body.extend_from_slice(&self.presentation_digest);
        Ok(content_id(TEMPLATE_PRESENTATION_DOMAIN, &body))
    }

    fn payout_denominator(&self) -> u64 {
        self.payouts[0].denominator
    }
}

/// Content-addressed exact costs prepaid for every scheduled Instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEnvelopeV1 {
    /// Compiler/reviewed quote version.
    pub version: u32,
    /// Exact account/rent principal allocated at Instance construction.
    pub creation_lamports: u64,
    /// Exact keeper/work budget allocated independently of future fees.
    pub liveness_lamports: u64,
}

impl WorkEnvelopeV1 {
    /// Validate finite positive version and both named compartments.
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 || self.creation_lamports == 0 || self.liveness_lamports == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Canonical content identity.
    pub fn id(&self) -> Result<Id> {
        self.validate()?;
        let mut body = Vec::with_capacity(28);
        body.extend_from_slice(b"DCWORK1\0");
        push_u32(&mut body, self.version);
        push_u64(&mut body, self.creation_lamports);
        push_u64(&mut body, self.liveness_lamports);
        Ok(content_id(WORK_ENVELOPE_DOMAIN, &body))
    }
}

/// Template-relative passive-liquidity plan.
///
/// This is the time/market-neutral predecessor of the existing
/// `LiquidityPolicyV1`, whose `NativeTermsV1` necessarily binds one actual
/// Market and Terms digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityBlueprintV1 {
    /// Template whose native coefficient basis this plan uses.
    pub template_id: Id,
    /// Admitted payoff/risk region artifact.
    pub payoff_region_digest: Id,
    /// Compiler for the per-Instance concrete quote schedule.
    pub quote_schedule_compiler_id: Id,
    /// Maximum outstanding inventory by native Egg.
    pub max_inventory: [u64; MAX_OUTCOMES],
    /// Exact fully prepaid tranche cap for each Instance.
    pub collateral_cap: u64,
    /// First native batch epoch admitted by the concrete schedule.
    pub batch_start: u64,
    /// Last native batch epoch admitted by the concrete schedule.
    pub batch_end: u64,
    /// Immutable liquidity-fee allocation policy.
    pub fee_policy_id: Id,
    /// Immutable withdrawal convention.
    pub withdrawal_policy_id: Id,
    /// Blueprint/compiler version.
    pub compiler_version: u32,
}

impl LiquidityBlueprintV1 {
    /// Validate template binding, canonical inventory padding, and the numeric
    /// envelope already required by the existing liquidity model.
    pub fn validate(&self, template: &TemplateV1) -> Result<()> {
        template.validate()?;
        if self.template_id != template.id()? {
            return Err(Error::MismatchedArtifact);
        }
        check_id(&self.payoff_region_digest)?;
        check_id(&self.quote_schedule_compiler_id)?;
        check_id(&self.fee_policy_id)?;
        check_id(&self.withdrawal_policy_id)?;
        if self.compiler_version == 0
            || self.collateral_cap == 0
            || self.collateral_cap > LIQUIDITY_MAX_ATOMS
            || self.batch_start > self.batch_end
            || self.batch_end == u64::MAX
        {
            return Err(Error::InvalidParameter);
        }
        let mut any = false;
        for (index, amount) in self.max_inventory.iter().copied().enumerate() {
            if index < usize::from(template.outcome_count) {
                if amount > LIQUIDITY_MAX_ATOMS {
                    return Err(Error::InvalidParameter);
                }
                any |= amount != 0;
            } else if amount != 0 {
                return Err(Error::NonCanonicalPadding);
            }
        }
        if !any {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Canonical body bytes.
    pub fn canonical_bytes(&self, template: &TemplateV1) -> Result<Vec<u8>> {
        self.validate(template)?;
        let mut out = Vec::with_capacity(320);
        out.extend_from_slice(b"DCLIQB1\0");
        out.extend_from_slice(&self.template_id);
        out.extend_from_slice(&self.payoff_region_digest);
        out.extend_from_slice(&self.quote_schedule_compiler_id);
        for amount in self.max_inventory {
            push_u64(&mut out, amount);
        }
        push_u64(&mut out, self.collateral_cap);
        push_u64(&mut out, self.batch_start);
        push_u64(&mut out, self.batch_end);
        out.extend_from_slice(&self.fee_policy_id);
        out.extend_from_slice(&self.withdrawal_policy_id);
        push_u32(&mut out, self.compiler_version);
        Ok(out)
    }

    /// Canonical blueprint identity.
    pub fn id(&self, template: &TemplateV1) -> Result<Id> {
        Ok(content_id(
            LIQUIDITY_BLUEPRINT_DOMAIN,
            &self.canonical_bytes(template)?,
        ))
    }

    /// Bind one concrete current market and quote-schedule digest into the
    /// existing liquidity-policy model without copying market facts into
    /// Series state.
    pub fn bind_current_policy(
        &self,
        template: &TemplateV1,
        instance: &CompiledInstanceV1,
        terms: &TermsAccount,
        quote_schedule_digest: Id,
    ) -> Result<LiquidityPolicyV1> {
        self.validate(template)?;
        check_id(&quote_schedule_digest)?;
        if instance.template_id != self.template_id
            || terms.terms.bytes()
                != instance
                    .current_terms_digest
                    .ok_or(Error::UnsupportedCurrentLowering)?
        {
            return Err(Error::MismatchedArtifact);
        }
        let native_terms = NativeTermsV1 {
            market: instance.market_id.bytes(),
            terms_digest: terms.terms.bytes(),
            basis_degree: template.basis_degree,
            outcome_count: template.outcome_count,
            payout_denominator: template.payout_denominator(),
        };
        let mut policy = LiquidityPolicyV1 {
            policy_id: [1; 32],
            terms: native_terms,
            payoff_region_digest: self.payoff_region_digest,
            quote_schedule_digest,
            max_inventory: self.max_inventory,
            collateral_cap: self.collateral_cap,
            batch_start: self.batch_start,
            batch_end: self.batch_end,
            fee_policy_id: self.fee_policy_id,
            withdrawal_policy_id: self.withdrawal_policy_id,
            compiler_version: self.compiler_version,
        };
        policy.policy_id = liquidity_policy_id(&policy);
        policy.validate()?;
        Ok(policy)
    }
}

fn liquidity_policy_id(policy: &LiquidityPolicyV1) -> Id {
    let mut out = Vec::with_capacity(360);
    out.extend_from_slice(b"DCLIQP1\0");
    out.extend_from_slice(&policy.terms.market);
    out.extend_from_slice(&policy.terms.terms_digest);
    push_u8(&mut out, policy.terms.basis_degree);
    push_u8(&mut out, policy.terms.outcome_count);
    push_u64(&mut out, policy.terms.payout_denominator);
    out.extend_from_slice(&policy.payoff_region_digest);
    out.extend_from_slice(&policy.quote_schedule_digest);
    for amount in policy.max_inventory {
        push_u64(&mut out, amount);
    }
    push_u64(&mut out, policy.collateral_cap);
    push_u64(&mut out, policy.batch_start);
    push_u64(&mut out, policy.batch_end);
    out.extend_from_slice(&policy.fee_policy_id);
    out.extend_from_slice(&policy.withdrawal_policy_id);
    push_u32(&mut out, policy.compiler_version);
    content_id(LIQUIDITY_POLICY_DOMAIN, &out)
}

/// Immutable bounded recurring instantiation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPlanV1 {
    /// Template instantiated at every ordinal.
    pub template_id: Id,
    /// Collateral Realm.
    pub realm: Id,
    /// Realm Profile.
    pub profile: Id,
    /// Current Realm-namespaced price-grid identity.
    pub price_grid: Id,
    /// Frozen market/venue fee policy identity.
    pub fee_policy_id: Id,
    /// Exact per-Instance rent/work quote artifact.
    pub work_envelope_id: Id,
    /// Exact funded liquidity blueprint.
    pub liquidity_blueprint_id: Id,
    /// Start bucket of ordinal zero.
    pub first_start_bucket: u64,
    /// Difference between consecutive start buckets.
    pub stride_buckets: u64,
    /// Finite number of possible Instances.
    pub instance_count: u32,
    /// Buckets before start during which anyone may instantiate the next item.
    pub creation_lead_buckets: u64,
    /// Market-local liability cap for every Instance.
    pub market_collateral_cap: u64,
}

impl SeriesPlanV1 {
    /// Validate every content binding and the entire finite time/nonce range.
    pub fn validate(
        &self,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
    ) -> Result<()> {
        template.validate()?;
        work.validate()?;
        liquidity.validate(template)?;
        if self.template_id != template.id()?
            || self.work_envelope_id != work.id()?
            || self.liquidity_blueprint_id != liquidity.id(template)?
        {
            return Err(Error::MismatchedArtifact);
        }
        check_id(&self.realm)?;
        check_id(&self.profile)?;
        check_id(&self.price_grid)?;
        check_id(&self.fee_policy_id)?;
        if self.stride_buckets == 0
            || self.instance_count == 0
            || self.instance_count > MAX_SERIES_INSTANCES
            || self.creation_lead_buckets == 0
            || self.first_start_bucket < self.creation_lead_buckets
            || self.market_collateral_cap == 0
            || liquidity.collateral_cap > self.market_collateral_cap
        {
            return Err(Error::InvalidParameter);
        }
        let last = u64::from(self.instance_count - 1);
        self.stride_buckets
            .checked_mul(last)
            .and_then(|delta| self.first_start_bucket.checked_add(delta))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Validate this plan against the current immutable Realm/Profile/Grid
    /// accounts. This is an offline structural join, not account ownership or
    /// PDA authentication.
    pub fn validate_current_accounts(
        &self,
        realm: &RealmAccount,
        profile: &ProfileAccount,
        grid: &PriceGridAccount,
    ) -> Result<()> {
        realm.validate()?;
        profile.validate()?;
        grid.validate()?;
        if realm.realm.bytes() != self.realm
            || realm.profile.bytes() != self.profile
            || profile.realm != realm.realm
            || profile.profile != realm.profile
            || grid.realm != realm.realm
            || grid.grid.bytes() != self.price_grid
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Canonical body bytes.
    pub fn canonical_bytes(
        &self,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
    ) -> Result<Vec<u8>> {
        self.validate(template, work, liquidity)?;
        let mut out = Vec::with_capacity(300);
        out.extend_from_slice(b"DCSERIV1");
        out.extend_from_slice(&self.template_id);
        out.extend_from_slice(&self.realm);
        out.extend_from_slice(&self.profile);
        out.extend_from_slice(&self.price_grid);
        out.extend_from_slice(&self.fee_policy_id);
        out.extend_from_slice(&self.work_envelope_id);
        out.extend_from_slice(&self.liquidity_blueprint_id);
        push_u64(&mut out, self.first_start_bucket);
        push_u64(&mut out, self.stride_buckets);
        push_u32(&mut out, self.instance_count);
        push_u64(&mut out, self.creation_lead_buckets);
        push_u64(&mut out, self.market_collateral_cap);
        Ok(out)
    }

    /// Canonical Series plan identity.
    pub fn id(
        &self,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
    ) -> Result<Id> {
        Ok(content_id(
            SERIES_DOMAIN,
            &self.canonical_bytes(template, work, liquidity)?,
        ))
    }

    fn start_bucket(&self, ordinal: u32) -> Result<u64> {
        if ordinal >= self.instance_count {
            return Err(Error::SeriesExhausted);
        }
        self.first_start_bucket
            .checked_add(
                self.stride_buckets
                    .checked_mul(u64::from(ordinal))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Mutable prepaid compartments and cursor for one immutable SeriesPlan.
///
/// No fee or projected revenue field exists. Every future Instance is funded
/// at activation from exact creation, work, and liquidity compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingV1 {
    /// Immutable Series identity this state serves.
    pub series_id: Id,
    /// First ordinal not yet instantiated or explicitly lapsed.
    pub next_ordinal: u32,
    /// Remaining account/rent principal.
    pub creation_lamports: u64,
    /// Remaining keeper/work budget.
    pub liveness_lamports: u64,
    /// Remaining collateral reserved for passive-liquidity tranches.
    pub liquidity_collateral: u64,
}

impl SeriesFundingV1 {
    fn validate_against(
        &self,
        plan: &SeriesPlanV1,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
    ) -> Result<()> {
        if self.series_id != plan.id(template, work, liquidity)?
            || self.next_ordinal > plan.instance_count
        {
            return Err(Error::MismatchedArtifact);
        }
        let remaining = u64::from(plan.instance_count - self.next_ordinal);
        let required_creation = work
            .creation_lamports
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liveness = work
            .liveness_lamports
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liquidity = liquidity
            .collateral_cap
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.creation_lamports < required_creation
            || self.liveness_lamports < required_liveness
            || self.liquidity_collateral < required_liquidity
        {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(())
    }

    /// Activate only when every bounded future obligation is already funded.
    /// Exact equality deliberately prevents an unowned surplus from being
    /// mislabeled as payer principal in this model.
    pub fn activate(
        plan: &SeriesPlanV1,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
        creation_lamports: u64,
        liveness_lamports: u64,
        liquidity_collateral: u64,
    ) -> Result<Self> {
        plan.validate(template, work, liquidity)?;
        let count = u64::from(plan.instance_count);
        let required_creation = work
            .creation_lamports
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liveness = work
            .liveness_lamports
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liquidity = liquidity
            .collateral_cap
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        if creation_lamports != required_creation
            || liveness_lamports != required_liveness
            || liquidity_collateral != required_liquidity
        {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(Self {
            series_id: plan.id(template, work, liquidity)?,
            next_ordinal: 0,
            creation_lamports,
            liveness_lamports,
            liquidity_collateral,
        })
    }

    /// Instantiate exactly the next ordinal during its frozen creation window.
    /// The returned state is a staged copy; refusals never mutate `self`.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_next(
        self,
        plan: &SeriesPlanV1,
        template: &TemplateV1,
        source: &SourceSpecViewV1,
        summary: &SummaryProgramV1,
        hatchery: &HatcheryProgramV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
        requested_ordinal: u32,
        current_bucket: u64,
    ) -> Result<(Self, CompiledInstanceV1)> {
        self.validate_against(plan, template, work, liquidity)?;
        if self.next_ordinal >= plan.instance_count {
            return Err(Error::SeriesExhausted);
        }
        if requested_ordinal != self.next_ordinal {
            return Err(Error::WrongOrdinal);
        }
        let start = plan.start_bucket(requested_ordinal)?;
        let eligible = start
            .checked_sub(plan.creation_lead_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        if current_bucket < eligible || current_bucket >= start {
            return Err(Error::NotEligible);
        }
        if self.creation_lamports < work.creation_lamports
            || self.liveness_lamports < work.liveness_lamports
            || self.liquidity_collateral < liquidity.collateral_cap
        {
            return Err(Error::InsufficientPrepayment);
        }
        let instance = compile_instance(
            plan,
            template,
            source,
            summary,
            hatchery,
            work,
            liquidity,
            requested_ordinal,
        )?;
        let mut next = self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.creation_lamports -= work.creation_lamports;
        next.liveness_lamports -= work.liveness_lamports;
        next.liquidity_collateral -= liquidity.collateral_cap;
        Ok((next, instance))
    }

    /// Permissionlessly lapse an ordinal whose creation deadline passed.
    ///
    /// No compartment is debited. Its now-unneeded allocation remains visible
    /// as terminally refundable Series principal rather than being silently
    /// reassigned, paid to the caller, or counted as fee revenue.
    pub fn lapse_next(
        self,
        plan: &SeriesPlanV1,
        template: &TemplateV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
        current_bucket: u64,
    ) -> Result<Self> {
        self.validate_against(plan, template, work, liquidity)?;
        if self.next_ordinal >= plan.instance_count {
            return Err(Error::SeriesExhausted);
        }
        if current_bucket < plan.start_bucket(self.next_ordinal)? {
            return Err(Error::NotEligible);
        }
        let mut next = self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(next)
    }

    /// Advance over a canonically identical Instance already created by
    /// another Series or caller, without spending this Series' allocations.
    ///
    /// Instance identity excludes Series and ordinal on purpose: identical
    /// economic descriptors converge. The unspent allocation remains visible
    /// as refundable Series principal.
    #[allow(clippy::too_many_arguments)]
    pub fn advance_existing(
        self,
        plan: &SeriesPlanV1,
        template: &TemplateV1,
        source: &SourceSpecViewV1,
        summary: &SummaryProgramV1,
        hatchery: &HatcheryProgramV1,
        work: &WorkEnvelopeV1,
        liquidity: &LiquidityBlueprintV1,
        existing: &CompiledInstanceV1,
        current_bucket: u64,
    ) -> Result<Self> {
        self.validate_against(plan, template, work, liquidity)?;
        if self.next_ordinal >= plan.instance_count {
            return Err(Error::SeriesExhausted);
        }
        let start = plan.start_bucket(self.next_ordinal)?;
        let eligible = start
            .checked_sub(plan.creation_lead_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        if current_bucket < eligible || current_bucket >= start {
            return Err(Error::NotEligible);
        }
        let expected = compile_instance(
            plan,
            template,
            source,
            summary,
            hatchery,
            work,
            liquidity,
            self.next_ordinal,
        )?;
        if existing.instance_id != expected.instance_id
            || existing.template_id != expected.template_id
            || existing.hatchery_window_id != expected.hatchery_window_id
            || existing.statistic_result_id != expected.statistic_result_id
            || existing.market_id != expected.market_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let mut next = self;
        next.next_ordinal = next
            .next_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(next)
    }
}

/// Deterministic, liability-free projection of one scheduled Instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledInstanceV1 {
    /// Canonical semantic Instance identity, independent of creator/Series.
    pub instance_id: Id,
    /// Owning immutable Series plan.
    pub series_id: Id,
    /// Template inherited through the Series.
    pub template_id: Id,
    /// Scheduled ordinal.
    pub ordinal: u32,
    /// Current adapter's deterministic 64-bit compatibility nonce, derived
    /// from `instance_id` rather than chosen by a caller.
    pub market_nonce: u64,
    /// Current adapter's deterministic Market identity. A future account
    /// generation should bind the full `instance_id`; this legacy projection
    /// retains a disclosed 64-bit truncation boundary.
    pub market_id: Hash32,
    /// Exact compact raw Hatchery window identity. It deliberately excludes
    /// statistic and SummaryProgram so several derived feature families can
    /// reuse the same authenticated raw pages/window seal.
    pub hatchery_window_id: Id,
    /// Exact statistic-result identity derived from the raw Hatchery window.
    pub statistic_result_id: Id,
    /// Current WindowDomain identity used by SourceArchive V1/V2.
    pub current_window_id: Id,
    /// Exact first observation bucket.
    pub start_bucket: u64,
    /// Exact exclusive observation end.
    pub end_bucket_exclusive: u64,
    /// Exact evidence maturity bucket.
    pub maturity_bucket_exclusive: u64,
    /// Exact creation/rent allocation debited from Series principal.
    pub creation_lamports: u64,
    /// Exact liveness allocation, independent of fees.
    pub liveness_lamports: u64,
    /// Exact passive-liquidity collateral allocation.
    pub liquidity_collateral: u64,
    /// Current Terms digest once a supported compatibility lowering is made.
    pub current_terms_digest: Option<Id>,
}

/// Minimal mutable Instance cursor preventing caller-selected epoch identity.
///
/// A future onchain Instance tail owns this cursor and must remain durable
/// across child cleanup. Closing/reopening a root without a retained replay
/// anchor would invalidate this model and is therefore outside this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceEpochCursorV1 {
    instance_id: Id,
    next_epoch_index: u64,
}

impl InstanceEpochCursorV1 {
    /// Start one canonical Instance at epoch index zero.
    pub fn new(instance_id: Id) -> Result<Self> {
        check_id(&instance_id)?;
        Ok(Self {
            instance_id,
            next_epoch_index: 0,
        })
    }

    /// Immutable Instance identity this cursor belongs to.
    pub const fn instance_id(self) -> Id {
        self.instance_id
    }

    /// The only epoch index the next transition may create.
    pub const fn next_epoch_index(self) -> u64 {
        self.next_epoch_index
    }

    /// Create exactly the next canonical epoch identity and advance the cursor
    /// on the returned staged copy.
    pub fn create_next(self, requested_index: u64) -> Result<(Self, Id)> {
        if requested_index != self.next_epoch_index {
            return Err(Error::WrongEpoch);
        }
        let mut body = Vec::with_capacity(48);
        body.extend_from_slice(b"DCEPOCH1");
        body.extend_from_slice(&self.instance_id);
        push_u64(&mut body, requested_index);
        let epoch_id = content_id(INSTANCE_EPOCH_DOMAIN, &body);
        let mut next = self;
        next.next_epoch_index = next
            .next_epoch_index
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok((next, epoch_id))
    }
}

/// Derive one future outcome identity from the full canonical InstanceId.
///
/// The current compatibility projection instead derives outcomes from the
/// legacy MarketId. A future compact Instance/Market generation should use
/// this full-width parent so no outcome identity inherits the 64-bit nonce
/// truncation boundary.
pub fn canonical_instance_outcome_id(
    instance_id: Id,
    outcome_count: u8,
    outcome_index: u8,
) -> Result<Id> {
    check_id(&instance_id)?;
    if outcome_count < 2
        || usize::from(outcome_count) > MAX_OUTCOMES
        || outcome_index >= outcome_count
    {
        return Err(Error::InvalidParameter);
    }
    let mut body = Vec::with_capacity(44);
    body.extend_from_slice(b"DCOUTCV1");
    body.extend_from_slice(&instance_id);
    push_u8(&mut body, outcome_index);
    Ok(content_id(INSTANCE_OUTCOME_DOMAIN, &body))
}

/// Compile an ordinal without changing Series funding. Callers normally use
/// [`SeriesFundingV1::instantiate_next`], which stages this output and the
/// compartment debits atomically in the pure model.
#[allow(clippy::too_many_arguments)]
pub fn compile_instance(
    plan: &SeriesPlanV1,
    template: &TemplateV1,
    source: &SourceSpecViewV1,
    summary: &SummaryProgramV1,
    hatchery: &HatcheryProgramV1,
    work: &WorkEnvelopeV1,
    liquidity: &LiquidityBlueprintV1,
    ordinal: u32,
) -> Result<CompiledInstanceV1> {
    plan.validate(template, work, liquidity)?;
    template.validate_bindings(source, summary, hatchery)?;
    let series_id = plan.id(template, work, liquidity)?;
    let start = plan.start_bucket(ordinal)?;
    let end = start
        .checked_add(template.window_span_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let maturity = end
        .checked_add(template.repair_grace_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let instance_id = {
        let mut body = Vec::with_capacity(280);
        body.extend_from_slice(b"DCINSTV1");
        body.extend_from_slice(&template.id()?);
        body.extend_from_slice(&plan.realm);
        body.extend_from_slice(&plan.profile);
        body.extend_from_slice(&plan.price_grid);
        body.extend_from_slice(&plan.fee_policy_id);
        body.extend_from_slice(&plan.work_envelope_id);
        body.extend_from_slice(&plan.liquidity_blueprint_id);
        push_u64(&mut body, start);
        push_u64(&mut body, plan.market_collateral_cap);
        content_id(INSTANCE_DOMAIN, &body)
    };
    let mut nonce_bytes = [0_u8; 8];
    nonce_bytes.copy_from_slice(&instance_id[..8]);
    let nonce = u64::from_le_bytes(nonce_bytes);
    let market_id = canonical_market_id(
        Hash32::from_bytes(plan.realm),
        Hash32::from_bytes(plan.profile),
        nonce,
    );
    let hatchery_window_id = hatchery_window_id(template, start, end, maturity)?;
    let statistic_result_id = statistic_result_id(template, hatchery_window_id)?;
    let current_window_id =
        current_window_id(template, source, summary, hatchery, start, end, maturity)?;
    Ok(CompiledInstanceV1 {
        instance_id,
        series_id,
        template_id: template.id()?,
        ordinal,
        market_nonce: nonce,
        market_id,
        hatchery_window_id,
        statistic_result_id,
        current_window_id,
        start_bucket: start,
        end_bucket_exclusive: end,
        maturity_bucket_exclusive: maturity,
        creation_lamports: work.creation_lamports,
        liveness_lamports: work.liveness_lamports,
        liquidity_collateral: liquidity.collateral_cap,
        current_terms_digest: None,
    })
}

fn hatchery_window_id(template: &TemplateV1, start: u64, end: u64, maturity: u64) -> Result<Id> {
    template.validate()?;
    let mut body = Vec::with_capacity(140);
    body.extend_from_slice(b"DCHWINV1");
    body.extend_from_slice(&template.source_spec_id);
    body.extend_from_slice(&template.hatchery_program_id);
    push_u64(&mut body, start);
    push_u64(&mut body, end);
    push_u64(&mut body, maturity);
    push_u64(&mut body, template.repair_generation);
    push_u16(&mut body, template.coverage_policy_id);
    push_u64(&mut body, template.coverage_policy_parameter);
    Ok(content_id(HATCHERY_WINDOW_DOMAIN, &body))
}

fn statistic_result_id(template: &TemplateV1, hatchery_window_id: Id) -> Result<Id> {
    template.validate()?;
    check_id(&hatchery_window_id)?;
    let mut body = Vec::with_capacity(80);
    body.extend_from_slice(b"DCSTATV1");
    body.extend_from_slice(&hatchery_window_id);
    body.extend_from_slice(&template.summary_program_id);
    push_u16(&mut body, template.statistic as u16);
    Ok(content_id(STATISTIC_RESULT_DOMAIN, &body))
}

fn window_domain(
    template: &TemplateV1,
    source: &SourceSpecViewV1,
    summary: &SummaryProgramV1,
    hatchery: &HatcheryProgramV1,
    start: u64,
    end: u64,
    maturity: u64,
) -> Result<WindowDomain> {
    template.validate_bindings(source, summary, hatchery)?;
    let feed = FeedIdentity::new(
        source.source_adapter_id,
        source.source_spec_id,
        source.source_version,
        summary.evaluator_version,
    )
    .map_err(|_| Error::InvalidParameter)?;
    let grid = Grid::new(
        source.grid_family_id,
        source.grid_version,
        source.bucket_seconds,
    )
    .map_err(|_| Error::InvalidParameter)?;
    let coverage = CoveragePolicy::from_registry(
        template.coverage_policy_id,
        template.coverage_policy_parameter,
    )
    .map_err(|_| Error::InvalidParameter)?;
    WindowDomain::new(
        feed,
        grid,
        start,
        end,
        maturity,
        template.repair_generation,
        coverage,
    )
    .map_err(|_| Error::InvalidParameter)
}

fn current_window_id(
    template: &TemplateV1,
    source: &SourceSpecViewV1,
    summary: &SummaryProgramV1,
    hatchery: &HatcheryProgramV1,
    start: u64,
    end: u64,
    maturity: u64,
) -> Result<Id> {
    let domain = window_domain(template, source, summary, hatchery, start, end, maturity)?;
    let mut bytes = [0_u8; WINDOW_DOMAIN_BYTES];
    domain.encode_canonical(&mut bytes);
    let mut hasher = Sha256::new();
    hasher.update(WINDOW_DOMAIN_TAG);
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

/// Lower a terminal Instance to the exact current TermsAccount v3 shape.
///
/// The account is validated by its owning codec and therefore can be fed into
/// current market-construction tooling. This does not create or submit an
/// account, and does not make Series/Instance state live on SBF.
#[allow(clippy::too_many_arguments)]
pub fn lower_current_terms(
    instance: &mut CompiledInstanceV1,
    plan: &SeriesPlanV1,
    template: &TemplateV1,
    source: &SourceSpecViewV1,
    summary: &SummaryProgramV1,
    hatchery: &HatcheryProgramV1,
    work: &WorkEnvelopeV1,
    liquidity: &LiquidityBlueprintV1,
) -> Result<TermsAccount> {
    let expected = compile_instance(
        plan,
        template,
        source,
        summary,
        hatchery,
        work,
        liquidity,
        instance.ordinal,
    )?;
    if expected.instance_id != instance.instance_id
        || expected.market_id != instance.market_id
        || expected.hatchery_window_id != instance.hatchery_window_id
        || expected.statistic_result_id != instance.statistic_result_id
    {
        return Err(Error::MismatchedArtifact);
    }
    let statistic_id = match template.statistic {
        StatisticProgramV1::TerminalInterval => 1,
        StatisticProgramV1::MaximumDrawdownInterval => {
            return Err(Error::UnsupportedCurrentLowering)
        }
    };
    let maturity_horizon_buckets = template
        .window_span_buckets
        .checked_add(template.repair_grace_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut terms = TermsAccount {
        terms: Hash32::ZERO,
        realm: Hash32::from_bytes(plan.realm),
        profile: Hash32::from_bytes(plan.profile),
        feed: Hash32::from_bytes(source.source_spec_id),
        price_grid: Hash32::from_bytes(plan.price_grid),
        outcome_count: template.outcome_count,
        payout_count: template.payout_count,
        payouts: template.payouts,
        grid_family_id: source.grid_family_id,
        grid_version: source.grid_version,
        bucket_seconds: source.bucket_seconds,
        expected_start_bucket: instance.start_bucket,
        expected_end_bucket_exclusive: instance.end_bucket_exclusive,
        maturity_horizon_buckets,
        coverage_policy_id: u32::from(template.coverage_policy_id),
        repair_policy_id: template.repair_policy_id,
        failure_policy_id: template.failure_policy_id,
        statistic_id,
        ambiguity_policy_id: template.ambiguity_policy_id,
        edge_policy_id: template.edge_policy_id,
        basis_degree: template.basis_degree,
        knot_count: template.knot_count,
        uniform_log2_spacing: template.uniform_log2_spacing,
        failure_payout_index: template.failure_payout_index,
        coverage_policy_parameter: template.coverage_policy_parameter,
        repair_generation: template.repair_generation,
        source_version: source.source_version,
        evaluator_version: summary.evaluator_version,
        source_adapter_id: Hash32::from_bytes(source.source_adapter_id),
        payout_map: template.payout_map,
        knots: template.knots,
        collateral_cap: plan.market_collateral_cap,
        stored_bump: 0,
        flags: 0,
    };
    terms.terms = terms.recomputed_terms_digest()?;
    terms.validate()?;
    instance.current_terms_digest = Some(terms.terms.bytes());
    Ok(terms)
}

/// Construct the current MarketAccount projection for a lowered Instance.
/// PDA bumps and creation slot are adapter facts supplied explicitly.
pub fn current_market_projection(
    instance: &CompiledInstanceV1,
    plan: &SeriesPlanV1,
    template: &TemplateV1,
    stored_bump: u8,
    hoard_bump: u8,
    created_slot: u64,
) -> Result<MarketAccount> {
    let terms = instance
        .current_terms_digest
        .ok_or(Error::UnsupportedCurrentLowering)?;
    if instance.series_id == [0; 32] || instance.template_id != template.id()? {
        return Err(Error::MismatchedArtifact);
    }
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    for (index, outcome) in outcomes
        .iter_mut()
        .enumerate()
        .take(usize::from(template.outcome_count))
    {
        *outcome = canonical_outcome_id(instance.market_id, index as u8);
    }
    let market = MarketAccount {
        market: instance.market_id,
        realm: Hash32::from_bytes(plan.realm),
        profile: Hash32::from_bytes(plan.profile),
        terms: Hash32::from_bytes(terms),
        outcome_count: template.outcome_count,
        lifecycle: 0,
        stored_bump,
        hoard_bump,
        outcomes,
        feed: Hash32::from_bytes(template.source_spec_id),
        collateral_cap: plan.market_collateral_cap,
        created_slot,
        reserved: Hash32::ZERO,
    };
    market.validate()?;
    Ok(market)
}

/// Convenience constructor for a categorical one-hot payout set.
///
/// It refuses more than eight outcomes because the current kernel's finite
/// payout-set account carries only eight vectors. Wider categorical partitions
/// need a later payout representation rather than truncated liabilities.
pub fn categorical_one_hot_payouts(
    outcome_count: u8,
    denominator: u64,
) -> Result<([PayoutVectorBytes; MAX_PAYOUTS], u8, [u8; MAX_OUTCOMES])> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_PAYOUTS || denominator == 0 {
        return Err(Error::InvalidParameter);
    }
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    for index in 0..usize::from(outcome_count) {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[index] = denominator;
        payouts[index] = PayoutVectorBytes {
            denominator,
            weights,
        };
        map[index] = index as u8;
    }
    Ok((payouts, outcome_count, map))
}
