#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Registry-independent recurring product and Series identity core.
//!
//! This crate freezes exact artifact bytes, typed SHA-256 identities, immutable
//! joins, recurrence arithmetic, and a per-component funding projection. It is
//! deliberately below every account and SBF adapter: it allocates no account
//! tags or instruction intents and imports no Solana, token, oracle, CPI, or
//! account-memory type.
//!
//! The selected recovery semantics are evidence-only. No type in this crate
//! contains a data-failure payout index or vector. Legacy V3 numeric-fallback
//! artifacts are explicitly refused rather than relabeled.

mod artifacts;
mod codec;
mod compile;
mod funding;
mod registry;

pub use artifacts::{
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV1, MarketInstancePreimageV1,
    NativeClaimBasisV1, ProductTemplateV4, RecoveryAttemptV1, SeriesAttachmentPlanV1,
    SeriesFundingTermsV1, SeriesPlanV4, BASIS_BYTES, EVIDENCE_ONLY_RECOVERY_POLICY_BYTES,
    MARKET_GENESIS_PROFILE_BYTES, MARKET_GENESIS_PROFILE_DOMAIN, MARKET_INSTANCE_DOMAIN,
    MARKET_INSTANCE_PREIMAGE_BYTES, MAX_BASIS_DEGREE, MAX_OUTCOMES, MAX_PAYOUTS,
    MAX_RECOVERY_ATTEMPTS, NATIVE_CLAIM_BASIS_DOMAIN, PAYOUT_MAP_UNUSED, PRODUCT_TEMPLATE_BYTES,
    PRODUCT_TEMPLATE_DOMAIN, RECOVERY_POLICY_DOMAIN, SERIES_ATTACHMENT_PLAN_BYTES,
    SERIES_ATTACHMENT_PLAN_DOMAIN, SERIES_FUNDING_TERMS_BYTES, SERIES_FUNDING_TERMS_DOMAIN,
    SERIES_PLAN_BYTES, SERIES_PLAN_DOMAIN, UNIFORM_SPACING_NONE,
};
pub use compile::{
    compile_ordinal, AbsoluteRecoveryAttemptV1, CompiledOrdinalV1, CompiledScheduleV1,
};
pub use funding::{
    project_component_debits, AdapterAuthenticatedComponentStatusV1,
    AdapterAuthenticatedFulfillmentStatusV1, ComponentDebitV1, DebitProjectionV1,
    FundingBalancesV1, RecoveryAttemptFundingV1, SeriesFundingQuoteV1, SERIES_FUNDING_QUOTE_BYTES,
    SERIES_FUNDING_QUOTE_DOMAIN,
};
pub use registry::{
    CapabilitySemanticOwnersV1, RealmCollateralProjectionV1, RegistryCapabilityProjectionV1,
};

use sha2::{Digest, Sha256};

/// A canonical 32-byte external artifact or release identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; 32]);

impl ContentId {
    /// Reserved all-zero padding identity. It is not a valid live reference.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct an identity from exact bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Whether this is the reserved all-zero identity.
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

macro_rules! typed_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(ContentId);

        impl $name {
            /// Construct a typed identity from exact digest bytes.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(ContentId::from_bytes(bytes))
            }

            /// Return the exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0.bytes()
            }

            /// Return this identity through the generic content-ID boundary.
            pub const fn content_id(self) -> ContentId {
                self.0
            }

            /// Refuse the all-zero identity reserved for inactive padding.
            pub fn validate(self) -> Result<()> {
                self.0.validate()
            }
        }
    };
}

typed_id!(
    NativeClaimBasisId,
    "Typed identity of one `NativeClaimBasisV1`."
);
typed_id!(
    EvidenceOnlyRecoveryPolicyId,
    "Typed identity of one `EvidenceOnlyRecoveryPolicyV1`."
);
typed_id!(
    ProductTemplateId,
    "Typed identity of one `ProductTemplateV4`."
);
typed_id!(
    MarketGenesisProfileId,
    "Typed identity of one `MarketGenesisProfileV1`."
);
typed_id!(
    MarketInstanceId,
    "Typed identity of one economic `MarketInstancePreimageV1`."
);
typed_id!(
    SeriesAttachmentPlanId,
    "Typed identity of one `SeriesAttachmentPlanV1`."
);
typed_id!(SeriesPlanId, "Typed identity of one `SeriesPlanV4`.");
typed_id!(
    SeriesFundingQuoteId,
    "Typed identity of one `SeriesFundingQuoteV1`."
);
typed_id!(
    SeriesFundingTermsId,
    "Typed identity of one `SeriesFundingTermsV1`."
);

/// A deterministic refusal from a fixed codec or pure projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was shorter than the exact named layout.
    Truncated,
    /// Input or output was longer than the exact named layout.
    TrailingBytes,
    /// A fixed artifact discriminator did not match.
    BadMagic,
    /// A schema was not the one exact version implemented here.
    BadVersion,
    /// Reserved bytes were not all zero.
    NonCanonicalReserved,
    /// A required identity was all zero.
    ZeroIdentity,
    /// A count, scalar, enum, or amount was outside its admitted domain.
    InvalidParameter,
    /// An inactive fixed-width entry was not canonical padding.
    NonCanonicalPadding,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A referenced artifact did not match its recomputed typed identity.
    MismatchedArtifact,
    /// A finite recovery schedule was unordered, overlapping, or empty.
    InvalidSchedule,
    /// A current numeric-fallback Product/Payout V3 body was presented.
    LegacyNumericFallback,
    /// The requested Series ordinal is outside the immutable finite schedule.
    WrongOrdinal,
    /// An exact-existing versus absent component projection was inconsistent.
    InvalidComponentStatus,
    /// Available segregated funding could not cover the projected components.
    InsufficientPrepayment,
    /// The selected capability profile does not admit the requested semantics.
    UnsupportedCapability,
}

/// Result alias for this allocation-free core.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact fixed-layout encoding and hostile decoding contract.
pub trait FixedCodec: Sized {
    /// Exact canonical body length; shorter and longer inputs both refuse.
    const ENCODED_LEN: usize;

    /// Validate and encode into an exact-length caller-owned buffer.
    fn encode_into(&self, output: &mut [u8]) -> Result<()>;

    /// Decode one exact-length canonical value and validate all padding.
    fn decode(input: &[u8]) -> Result<Self>;
}

pub(crate) fn content_id(domain: &[u8], body: &[u8]) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    ContentId::from_bytes(hasher.finalize().into())
}
