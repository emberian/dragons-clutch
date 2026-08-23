#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Allocation-free SourcePlane V3 and recurring-product compiler core.
//!
//! This crate is deliberately below every Solana account adapter. It defines
//! fixed canonical bytes, content identities, source-only feed state, reusable
//! raw pages, immutable window evidence, statistic keys and result digests,
//! conservative drawdown, and deterministic recurring Instance lowering. It
//! authenticates no account owner, PDA, clock, oracle, signature, or program
//! release; an adapter must establish those facts before calling this core.
//!
//! The crate contains no allocator, Solana SDK, token type, floating point, or
//! caller-selected market nonce. All public decoding is exact-length and
//! fail-closed. Future source-plane versions are not treated as compatible
//! merely because their version number is larger.

mod codec;
mod compiler;
mod drawdown;
mod source;

pub use compiler::{
    compile_instance, CompiledInstanceV3, InstanceDescriptorV3, LiquidityEnvelopeV3,
    PartitionViewV3, PayoutTableV3, PayoutVectorV3, ProductTemplateV3, SeriesFundingV3,
    SeriesPlanV3, WorkEnvelopeV3, EXTENDED_WINDOW_02, FAILURE_UNIFORM_REFUND_01,
    INSTANCE_DESCRIPTOR_BYTES, LIQUIDITY_ENVELOPE_BYTES, MAX_OUTCOMES, MAX_PAYOUTS,
    MAX_SERIES_INSTANCES, PAYOUT_TABLE_BYTES, PRODUCT_TEMPLATE_BYTES, SERIES_FUNDING_BYTES,
    SERIES_PLAN_BYTES, WORK_ENVELOPE_BYTES,
};
pub use drawdown::{
    DrawdownIntervalV3, DrawdownSummaryV3, DRAWDOWN_PPM_SCALE, DRAWDOWN_SUMMARY_BYTES,
};
pub use source::{
    OpenRawPageV3, RawPageV3, RawRecordKindV3, RawRecordV3, SourceHeadV3, SourcePlaneProgramV3,
    StatisticKeyV3, StatisticKindV3, StatisticResultStatusV3, StatisticResultV3, SummaryProgramV3,
    WindowClosureReceiptV3, WindowSealV3, WindowSpecV3, WindowWorkV3, CAP_REALM_NEUTRAL_FEED,
    CAP_REUSABLE_RAW_PAGES, CAP_SOURCE_ONLY_HEAD, CAP_STATISTIC_RESULTS, FEATURE_DRAWDOWN_INTERVAL,
    FEATURE_TERMINAL_INTERVAL, MAX_RAW_PAGE_RECORDS, MAX_SOURCE_VALUE, OPEN_RAW_PAGE_BYTES,
    RAW_PAGE_BYTES, RAW_RECORD_BYTES, SOURCE_HEAD_BYTES, SOURCE_PLANE_PROGRAM_BYTES,
    STATISTIC_KEY_BYTES, STATISTIC_RESULT_BYTES, SUMMARY_PROGRAM_BYTES,
    WINDOW_CLOSURE_RECEIPT_BYTES, WINDOW_SEAL_BYTES, WINDOW_SPEC_BYTES, WINDOW_WORK_BYTES,
};

use sha2::{Digest, Sha256};

/// A canonical 32-byte content or externally authenticated object identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; 32]);

impl ContentId {
    /// Reserved all-zero padding identity. It is never a valid live reference.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct an identity from exact bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Whether this is the reserved all-zero padding identity.
    pub fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub(crate) fn validate(self) -> Result<()> {
        if self.is_zero() {
            Err(Error::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// A deterministic refusal from a canonical codec or pure transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was not the exact fixed width of the named codec.
    Truncated,
    /// Input or output contained bytes after the exact fixed layout.
    TrailingBytes,
    /// A fixed account/artifact discriminator did not match.
    BadMagic,
    /// A schema, codec, or semantic version was not the exact registered one.
    BadVersion,
    /// Reserved bytes were not all zero.
    NonCanonicalReserved,
    /// A required content identity was the reserved all-zero value.
    ZeroIdentity,
    /// A count, range, amount, enum, or other scalar was outside its domain.
    InvalidParameter,
    /// Inactive fixed-width entries were not exact zero padding.
    NonCanonicalPadding,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A referenced object did not match its recomputed content identity.
    MismatchedArtifact,
    /// A raw page did not extend the exact source-only head or page chain.
    DiscontinuousPage,
    /// A window did not contain every canonical bucket or reach maturity.
    IncompleteWindow,
    /// A caller attempted to append evidence after the canonical maturity page.
    WindowAlreadyMature,
    /// The requested statistic is not in the evaluator's closed feature set.
    UnsupportedStatistic,
    /// A registered policy is not implemented by this exact core version.
    UnsupportedPolicy,
    /// `FAIL_UNIFORM_REFUND_01` selected a non-uniform payout vector.
    FailurePayoutNotUniform,
    /// The requested Series ordinal was not the durable exact-next ordinal.
    WrongOrdinal,
    /// A permissionless create/lapse transition was outside its exact interval.
    NotEligible,
    /// Every finite Series ordinal has already advanced.
    SeriesExhausted,
    /// Segregated prepaid compartments cannot cover every remaining obligation.
    InsufficientPrepayment,
}

/// Result alias for the allocation-free core.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact fixed-layout encoding and hostile decoding contract.
pub trait FixedCodec: Sized {
    /// Exact number of canonical bytes; shorter and longer inputs both refuse.
    const ENCODED_LEN: usize;

    /// Validate and encode into an exact-length caller-owned buffer.
    fn encode_into(&self, output: &mut [u8]) -> Result<()>;

    /// Decode an exact-length canonical value and validate all padding.
    fn decode(input: &[u8]) -> Result<Self>;
}

pub(crate) fn content_id(domain: &[u8], body: &[u8]) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    ContentId::from_bytes(hasher.finalize().into())
}
