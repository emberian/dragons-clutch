#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Provider-neutral, fixed-layout source and resolution contracts.
//!
//! This crate commits *what a Product occurrence means* from a source, while
//! leaving message transport, account discovery, RPC, transaction assembly,
//! and provider SDKs to a separately authenticated adapter.  A provider
//! release is an opaque, versioned adapter identity; it never makes provider
//! transport Product truth.  Product truth is the occurrence-to-source,
//! window, statistic, and result-mapping linkage in [`ResolutionPolicyV1`].
//!
//! Every statistic uses signed integer atoms and exact rational intermediates.
//! The named rounding boundary is the statistic-to-result mapping boundary;
//! no float and no implicit division are admitted.  Fixed bounds are selected
//! by [`SourceCapacityProfileV1`].  A provisional profile must name a lifting
//! plan; a later profile can lift limits without changing old policy bytes.

use core::convert::TryInto;
pub use dclutch_product_contract::result_domain::FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1;
use dclutch_product_contract::{
    product::InstanceV1,
    result_domain::{FINITE_RESULT_DOMAIN_BYTES, FiniteResultDomainV1},
};

/// Exact width of an opaque nonzero content identity.
pub const CONTENT_ID_BYTES: usize = 32;
/// Exact width of a provider-release preimage.
pub const PROVIDER_RELEASE_BYTES: usize = 176;
/// Exact width of the first Pyth provider adapter configuration.
pub const PYTH_ADAPTER_CONFIG_BYTES: usize = 64;
/// Exact width of a source-capacity profile preimage.
pub const SOURCE_CAPACITY_PROFILE_BYTES: usize = 112;
/// Exact width of a source-specification preimage.
pub const SOURCE_SPEC_BYTES: usize = 192;
/// Exact width of a window-specification preimage.
pub const WINDOW_SPEC_BYTES: usize = 112;
/// Exact width of a statistic-specification preimage.
pub const STATISTIC_SPEC_BYTES: usize = 176;
/// Exact width of a resolution-policy preimage.
pub const RESOLUTION_POLICY_BYTES: usize = 240;
/// Maximum attempts in the V1 recovery artifact profile.
pub const MAX_RECOVERY_ATTEMPTS: usize = 4;
/// Exact width of one fixed recovery-attempt slot.
pub const RECOVERY_ATTEMPT_BYTES: usize = 112;
/// Exact width of a recovery-policy preimage.
pub const RECOVERY_POLICY_BYTES: usize = 528;
/// Exact width of one normalized provider-evidence record.
pub const NORMALIZED_EVIDENCE_BYTES: usize = 208;
/// Exact width of the single canonical immutable Source material preimage.
pub const SOURCE_MATERIAL_BYTES: usize = 4_176;
/// Exact width of one persisted source-resolution state.
pub const SOURCE_RESOLUTION_STATE_BYTES: usize = 224;
/// Provisional maximum observations retained by one V1 shared child.
///
/// This is an on-chain account-profile bound. A later schema release can lift
/// it without reinterpreting any V1 child.
pub const MAX_SHARED_OBSERVATIONS: usize = 16;
/// Exact width of one persisted shared-observation child, including all
/// normalized observations authenticated by the selected provider adapter.
pub const SHARED_OBSERVATION_STATE_BYTES: usize =
    288 + MAX_SHARED_OBSERVATIONS * NORMALIZED_EVIDENCE_BYTES;
/// Exact width of one explicit generation-reopen link.
pub const REOPEN_LINK_BYTES: usize = 128;
/// Exact shared source-instruction header width.
pub const SOURCE_INSTRUCTION_HEADER_BYTES: usize = 16;
/// Exact CreateResolution instruction width.
pub const CREATE_RESOLUTION_INSTRUCTION_BYTES: usize = 288;
/// Exact fixed Source prefix before a provider-release-owned evidence payload.
pub const ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES: usize = 32;
/// Exact FailNext, Exhaust, or Retire instruction width.
pub const GENERATION_INSTRUCTION_BYTES: usize = 24;
/// Exact CommitFailure instruction width.
pub const COMMIT_FAILURE_INSTRUCTION_BYTES: usize = 32;
/// Exact child-count-guarded Retire instruction width.
pub const RETIRE_INSTRUCTION_BYTES: usize = 32;
/// Exact CreateSharedObservation instruction width.
pub const CREATE_SHARED_OBSERVATION_INSTRUCTION_BYTES: usize = 208;
/// Exact fixed Source prefix before a provider-release-owned shared payload.
pub const ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES: usize = 64;
/// Chain-derived maximum byte width of one Solana PDA seed component.
pub const SVM_MAX_PDA_SEED_BYTES: usize = 32;

/// Canonical provider-release magic.
pub const PROVIDER_RELEASE_MAGIC: [u8; 8] = *b"DCLTPRV1";
/// Canonical Pyth adapter-configuration magic.
pub const PYTH_ADAPTER_CONFIG_MAGIC: [u8; 8] = *b"DCLTPAC1";
/// Canonical capacity-profile magic.
pub const SOURCE_CAPACITY_PROFILE_MAGIC: [u8; 8] = *b"DCLTSCP1";
/// Canonical source-specification magic.
pub const SOURCE_SPEC_MAGIC: [u8; 8] = *b"DCLTSRC1";
/// Canonical window-specification magic.
pub const WINDOW_SPEC_MAGIC: [u8; 8] = *b"DCLTWIN1";
/// Canonical statistic-specification magic.
pub const STATISTIC_SPEC_MAGIC: [u8; 8] = *b"DCLTSTA1";
/// Canonical resolution-policy magic.
pub const RESOLUTION_POLICY_MAGIC: [u8; 8] = *b"DCLTRSP1";
/// Canonical recovery-policy magic.
pub const RECOVERY_POLICY_MAGIC: [u8; 8] = *b"DCLTRCV1";
/// Canonical normalized-evidence magic.
pub const NORMALIZED_EVIDENCE_MAGIC: [u8; 8] = *b"DCLTNEV1";
/// Canonical single Source-material magic.
pub const SOURCE_MATERIAL_MAGIC: [u8; 8] = *b"DCLTSMV1";
/// Canonical persisted source-resolution-state magic.
pub const SOURCE_RESOLUTION_STATE_MAGIC: [u8; 8] = *b"DCLTSRS1";
/// Canonical persisted shared-observation-state magic.
pub const SHARED_OBSERVATION_STATE_MAGIC: [u8; 8] = *b"DCLTSOS1";
/// Canonical generation-reopen-link magic.
pub const REOPEN_LINK_MAGIC: [u8; 8] = *b"DCLTRPN1";
/// Canonical source instruction magic.
pub const SOURCE_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTSIX1";
/// Implemented schema release for all V1 records.
pub const SCHEMA_VERSION: u16 = 1;

/// PDA domain for one source-resolution state per Market generation.
pub const SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1: &[u8] = b"dclutch/source-state/v1";
/// PDA domain for one shared observation per Market/source/window generation.
pub const SHARED_OBSERVATION_PDA_DOMAIN_V1: &[u8] = b"dclutch/shared-obs/v1";

/// Closed preimage of the persisted source-state schema release.
pub const SOURCE_STATE_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/source-resolution-state-schema/v1";
/// SHA-256 content identity of [`SOURCE_STATE_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const SOURCE_STATE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xc6, 0x50, 0x91, 0x93, 0x79, 0xac, 0x0b, 0x43, 0xc5, 0x14, 0x87, 0x39, 0xd5, 0x79, 0x63, 0xa4,
    0xc8, 0x55, 0x6c, 0x32, 0xdd, 0xb6, 0xaf, 0xb2, 0xae, 0x74, 0xff, 0x33, 0x70, 0x9c, 0x3a, 0xc3,
];
/// Closed preimage of the source-state PDA derivation release.
pub const SOURCE_STATE_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/source-resolution-state-derivation/v1";
/// SHA-256 content identity of [`SOURCE_STATE_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const SOURCE_STATE_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0xa0, 0x70, 0xea, 0xe4, 0xfe, 0xef, 0x3e, 0x31, 0xbb, 0x4e, 0x74, 0xca, 0x57, 0xea, 0x65, 0xf6,
    0xbe, 0xa2, 0x7f, 0x73, 0x02, 0x6b, 0xfd, 0x31, 0x5b, 0x02, 0xfe, 0xe3, 0xb1, 0x4a, 0x51, 0x26,
];
/// Closed preimage of the shared-observation schema release.
pub const SHARED_OBSERVATION_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/shared-observation-schema/v1";
/// SHA-256 content identity of [`SHARED_OBSERVATION_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const SHARED_OBSERVATION_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x12, 0x47, 0x65, 0xee, 0x87, 0xac, 0xfe, 0x30, 0x91, 0x65, 0x1f, 0x30, 0xb1, 0xa9, 0x20, 0xbf,
    0xfa, 0xa3, 0x4a, 0xdf, 0x8a, 0x9d, 0x0a, 0x59, 0xe0, 0xf4, 0x55, 0xe2, 0x0d, 0x8e, 0x20, 0x53,
];
/// Closed preimage of the shared-observation PDA derivation release.
pub const SHARED_OBSERVATION_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/shared-observation-derivation/v1";
/// SHA-256 content identity of [`SHARED_OBSERVATION_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const SHARED_OBSERVATION_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0xd1, 0xaf, 0x03, 0xc6, 0xd7, 0xe3, 0x1f, 0x4c, 0x7a, 0xd3, 0x0a, 0x30, 0x79, 0x86, 0x2f, 0x80,
    0x29, 0xc1, 0x7e, 0xcf, 0x9c, 0xc2, 0x65, 0xb1, 0x82, 0xe1, 0xc0, 0xa7, 0x67, 0x62, 0xcb, 0x21,
];
/// Closed preimage of the narrow reopen-link schema.
pub const REOPEN_LINK_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/source-reopen-link-schema/v1";
/// SHA-256 content identity of [`REOPEN_LINK_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const REOPEN_LINK_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x27, 0xcd, 0xb7, 0x6c, 0xcf, 0x87, 0x32, 0x26, 0xeb, 0xa2, 0x27, 0xcc, 0xa4, 0xce, 0x5a, 0xc0,
    0x8d, 0xcc, 0x85, 0x69, 0xf7, 0x78, 0x7d, 0x88, 0x9c, 0x4b, 0x19, 0xc0, 0x04, 0x35, 0x0b, 0xac,
];
/// Closed preimage of the one persisted Source-material schema.
pub const SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/source-material-schema/v1";
/// SHA-256 content identity of [`SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xa3, 0xa2, 0x02, 0x1f, 0x3a, 0x57, 0xcb, 0xd5, 0x59, 0xe8, 0x2f, 0xd4, 0x03, 0xfc, 0xf3, 0x15,
    0x1d, 0x65, 0x7d, 0xad, 0x6d, 0xc6, 0x11, 0x1f, 0xf3, 0xc7, 0xe9, 0x83, 0x63, 0x1d, 0xef, 0x93,
];
/// Closed preimage of the Source-material raw/staging record derivation.
pub const SOURCE_MATERIAL_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/source-material-record-derivation/v1";
/// SHA-256 identity of [`SOURCE_MATERIAL_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const SOURCE_MATERIAL_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0xb7, 0xb4, 0x17, 0xbd, 0xf0, 0x4d, 0x2d, 0x4b, 0xd5, 0x4c, 0x7f, 0x7b, 0x69, 0xd9, 0xdf, 0x4d,
    0xd3, 0xdf, 0x49, 0xb2, 0x78, 0x75, 0xa5, 0x40, 0x33, 0x4f, 0xf2, 0x5b, 0x57, 0x99, 0x17, 0x3b,
];
/// Closed first provider-extension release: Pyth Receiver V2 post/reclaim.
pub const PYTH_PROVIDER_EXTENSION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/source-pyth-receiver-provider-extension/v1";
/// SHA-256 identity of [`PYTH_PROVIDER_EXTENSION_RELEASE_PREIMAGE_V1`].
pub const PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1: [u8; 32] = [
    0xc7, 0xec, 0xdf, 0xf1, 0x34, 0xc8, 0x92, 0x16, 0x20, 0xdd, 0x8a, 0xeb, 0xae, 0x69, 0x70, 0x6a,
    0x2b, 0x3b, 0x15, 0xd3, 0x21, 0x81, 0xb0, 0xdc, 0xb5, 0xb3, 0x15, 0x72, 0x27, 0x41, 0xf7, 0xc9,
];
/// Closed preimage release for the canonical shared-evidence-set digest.
pub const SHARED_EVIDENCE_SET_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/source-shared-evidence-set/v1";
/// SHA-256 identity of [`SHARED_EVIDENCE_SET_RELEASE_PREIMAGE_V1`].
pub const SHARED_EVIDENCE_SET_RELEASE_ID_V1: [u8; 32] = [
    0x29, 0xb6, 0xf6, 0x6f, 0xa5, 0xa2, 0x43, 0x47, 0xdc, 0xcd, 0x8e, 0x9c, 0xd2, 0x4f, 0x04, 0x95,
    0x75, 0x47, 0x7c, 0x5a, 0x3a, 0x70, 0x99, 0xc3, 0xce, 0xea, 0x8f, 0x06, 0x48, 0x05, 0xa7, 0xe8,
];
/// Exact fixed header before canonical normalized observations in a shared
/// evidence-set digest preimage.
pub const SHARED_EVIDENCE_SET_HEADER_BYTES_V1: usize = 176;

const _: () = assert!(SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1.len() <= SVM_MAX_PDA_SEED_BYTES);
const _: () = assert!(SHARED_OBSERVATION_PDA_DOMAIN_V1.len() <= SVM_MAX_PDA_SEED_BYTES);

/// Refusal returned by a hostile decoder, constructor, or exact evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input was not its one exact canonical width.
    InvalidLength,
    /// Record magic did not identify the requested contract.
    InvalidMagic,
    /// Record schema is not implemented by this crate.
    UnsupportedSchema,
    /// A reserved byte or inactive fixed-layout slot was nonzero.
    NonCanonicalReservedBytes,
    /// An opaque required identity used the all-zero sentinel.
    ZeroContentId,
    /// A byte did not name a defined capacity-envelope kind.
    UnknownCapacityEnvelope,
    /// A capacity that must be positive was zero.
    ZeroCapacity,
    /// Capacity fields did not obey their exact canonical relation.
    NonCanonicalCapacity,
    /// A byte did not name a defined source-access profile.
    UnknownSourceAccess,
    /// A source profile carried fields forbidden for its access profile.
    NonCanonicalSourceProfile,
    /// A byte did not name a defined window kind.
    UnknownWindowKind,
    /// A window's timestamps or timing limits were inconsistent.
    InvalidWindow,
    /// A byte did not name a defined statistic family.
    UnknownStatistic,
    /// A byte did not name a defined rounding boundary.
    UnknownRounding,
    /// A statistic used a noncanonical family-specific field.
    NonCanonicalStatistic,
    /// A statistic's sample count exceeded the selected capacity profile.
    StatisticExceedsCapacity,
    /// Exact signed-integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A sample count or sample timestamp failed the committed schedule.
    InvalidObservationSchedule,
    /// A threshold statistic did not receive a valid exact threshold.
    InvalidThreshold,
    /// A recovery policy exceeded its capacity profile's fixed attempt bound.
    RecoveryExceedsCapacity,
    /// Recovery attempts were not in strictly increasing deadline order.
    NonCanonicalRecoveryOrder,
    /// Linked preimages did not bind the same occurrence or dependency ID.
    LinkageMismatch,
    /// A recovery status transition was not admitted.
    InvalidRecoveryTransition,
    /// Terminal failure was requested before every committed attempt exhausted.
    RecoveryNotExhausted,
    /// A required Market key or rent-beneficiary authority was zero.
    ZeroIdentifier,
    /// A persisted phase or its phase-specific fields were not canonical.
    NonCanonicalState,
    /// A state, evidence, mapping, child, or reopen link did not bind its authority.
    StateBindingMismatch,
    /// Current time did not admit the requested acceptance or exhaustion edge.
    DeadlineNotReached,
    /// Current time was after the active attempt's committed deadline.
    DeadlineElapsed,
    /// Provider publication time was too old or too far in the future.
    InvalidPublicationTime,
    /// A finite result map was empty, unordered, or exceeded its fixed profile.
    InvalidResultMap,
    /// A mapped Product result selector was outside its committed outcome width.
    InvalidResultSelector,
    /// The selected mapping release is not the closed built-in V1 release.
    UnsupportedMappingRelease,
    /// A required positive replay or terminal sequence was zero.
    ZeroSequence,
    /// A source operation required the other access profile.
    WrongSourceAccessProfile,
    /// A supplied shared observation was not accepted or was already retired.
    InvalidSharedObservation,
    /// A reopen link did not name exactly the next Market generation.
    InvalidReopenLink,
    /// An instruction action byte was not defined by the V1 wire grammar.
    UnknownInstructionAction,
    /// An account frame had the wrong count, privilege, or an unsafe alias.
    InvalidAccountFrame,
    /// Normalized evidence bytes exceeded the selected fixed capacity profile.
    EvidenceExceedsCapacity,
    /// Shared-observation creation exceeded the selected fixed child bound.
    SharedChildrenExceedCapacity,
    /// Source material selected a provider extension absent from this release.
    UnsupportedProviderExtension,
    /// One embedded Source-material component or inactive recovery slot was noncanonical.
    NonCanonicalSourceMaterial,
    /// A provider-owned instruction extension was empty, unexpected, or malformed.
    InvalidProviderPayload,
    /// A Market child-count replay guard did not match the authenticated Market.
    MarketChildCountMismatch,
    /// A Pyth feed, exponent, or confidence bound did not match committed configuration.
    InvalidPythObservation,
}

/// Result alias for source-contract operations.
pub type Result<T> = core::result::Result<T, Error>;

/// A validated opaque content identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; CONTENT_ID_BYTES]);

impl ContentId {
    /// Construct a nonzero opaque identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroContentId);
        }
        Ok(Self(bytes))
    }

    /// Decode one exact-width nonzero identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact opaque bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact opaque bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES] {
        &self.0
    }
}

/// Immutable adapter-release identity for one source provider.
///
/// The release identifies a parser/normalizer boundary.  It does not encode a
/// provider message, account, endpoint, signer, or transport authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderReleaseV1 {
    provider_family_id: ContentId,
    adapter_release_id: ContentId,
    provider_deployment_release_id: ContentId,
    decoding_rules_id: ContentId,
    transport_profile_id: ContentId,
}

impl ProviderReleaseV1 {
    /// Construct one provider release from five immutable identities.
    pub const fn new(
        provider_family_id: ContentId,
        adapter_release_id: ContentId,
        provider_deployment_release_id: ContentId,
        decoding_rules_id: ContentId,
        transport_profile_id: ContentId,
    ) -> Self {
        Self {
            provider_family_id,
            adapter_release_id,
            provider_deployment_release_id,
            decoding_rules_id,
            transport_profile_id,
        }
    }

    /// Decode one hostile canonical provider-release preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_MAGIC)?;
        zero(bytes, 10, 6)?;
        Ok(Self::new(
            content(bytes, 16)?,
            content(bytes, 48)?,
            content(bytes, 80)?,
            content(bytes, 112)?,
            content(bytes, 144)?,
        ))
    }

    /// Encode exact canonical provider-release bytes.
    pub fn to_bytes(self) -> [u8; PROVIDER_RELEASE_BYTES] {
        let mut out = base::<PROVIDER_RELEASE_BYTES>(PROVIDER_RELEASE_MAGIC);
        put(&mut out, 16, self.provider_family_id.as_bytes());
        put(&mut out, 48, self.adapter_release_id.as_bytes());
        put(&mut out, 80, self.provider_deployment_release_id.as_bytes());
        put(&mut out, 112, self.decoding_rules_id.as_bytes());
        put(&mut out, 144, self.transport_profile_id.as_bytes());
        out
    }

    /// Return the parser/normalizer release identity.
    pub const fn adapter_release_id(self) -> ContentId {
        self.adapter_release_id
    }

    /// Return the provider-family identity.
    pub const fn provider_family_id(self) -> ContentId {
        self.provider_family_id
    }

    /// Return the exact pinned provider deployment-release content identity.
    pub const fn provider_deployment_release_id(self) -> ContentId {
        self.provider_deployment_release_id
    }

    /// Return the immutable decoding-rules identity.
    pub const fn decoding_rules_id(self) -> ContentId {
        self.decoding_rules_id
    }

    /// Return the immutable transport-profile identity.
    pub const fn transport_profile_id(self) -> ContentId {
        self.transport_profile_id
    }
}

/// Immutable first-release Pyth feed and integer-normalization configuration.
///
/// The provider feed is interpreted directly as signed price atoms only when
/// its exponent exactly matches `expected_exponent`. Confidence is accepted
/// only within the committed inclusive basis-point bound. Source domain and
/// unit semantics remain owned by [`SourceSpecV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythAdapterConfigV1 {
    provider_feed_id: [u8; 32],
    expected_exponent: i32,
    max_confidence_bps: u16,
}

impl PythAdapterConfigV1 {
    /// Construct one exact Pyth adapter configuration.
    pub fn new(
        provider_feed_id: [u8; 32],
        expected_exponent: i32,
        max_confidence_bps: u16,
    ) -> Result<Self> {
        nonzero_identifier(&provider_feed_id)?;
        if max_confidence_bps == 0 || max_confidence_bps > 10_000 {
            return Err(Error::InvalidPythObservation);
        }
        Ok(Self {
            provider_feed_id,
            expected_exponent,
            max_confidence_bps,
        })
    }

    /// Decode one exact hostile Pyth configuration.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, PYTH_ADAPTER_CONFIG_BYTES, PYTH_ADAPTER_CONFIG_MAGIC)?;
        zero(bytes, 48, 16)?;
        Self::new(
            read_array(bytes, 16)?,
            i32::from_le_bytes(read_array(bytes, 12)?),
            u16::from_le_bytes(read_array(bytes, 10)?),
        )
    }

    /// Encode the exact canonical Pyth configuration bytes.
    pub fn to_bytes(self) -> [u8; PYTH_ADAPTER_CONFIG_BYTES] {
        let mut out = base::<PYTH_ADAPTER_CONFIG_BYTES>(PYTH_ADAPTER_CONFIG_MAGIC);
        put(&mut out, 10, &self.max_confidence_bps.to_le_bytes());
        put(&mut out, 12, &self.expected_exponent.to_le_bytes());
        put(&mut out, 16, &self.provider_feed_id);
        out
    }

    /// Return the exact provider feed identifier.
    pub const fn provider_feed_id(self) -> [u8; 32] {
        self.provider_feed_id
    }

    /// Return the required raw Pyth base-ten exponent.
    pub const fn expected_exponent(self) -> i32 {
        self.expected_exponent
    }

    /// Return the inclusive maximum confidence ratio in basis points.
    pub const fn max_confidence_bps(self) -> u16 {
        self.max_confidence_bps
    }

    fn validate_update(
        self,
        provider_feed_id: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
    ) -> Result<i128> {
        let absolute_price = price.unsigned_abs();
        let confidence_scaled = u128::from(confidence)
            .checked_mul(10_000)
            .ok_or(Error::ArithmeticOverflow)?;
        let admitted_confidence = u128::from(absolute_price)
            .checked_mul(u128::from(self.max_confidence_bps))
            .ok_or(Error::ArithmeticOverflow)?;
        if provider_feed_id != self.provider_feed_id
            || exponent != self.expected_exponent
            || confidence_scaled > admitted_confidence
        {
            return Err(Error::InvalidPythObservation);
        }
        Ok(i128::from(price))
    }
}

/// Evidence class for a fixed capacity envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CapacityEnvelope {
    /// Bounds are supported by the named measurement manifest.
    Measured = 1,
    /// Bounds are temporary and the named identity is a required lifting plan.
    Provisional = 2,
}

impl CapacityEnvelope {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Measured),
            2 => Ok(Self::Provisional),
            _ => Err(Error::UnknownCapacityEnvelope),
        }
    }
    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Bounded artifact profile for source observations and recovery attempts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCapacityProfileV1 {
    envelope: CapacityEnvelope,
    max_samples: u16,
    max_recovery_attempts: u8,
    verifier_release_id: ContentId,
    envelope_basis_id: ContentId,
    max_observation_bytes: u32,
    max_shared_children: u32,
}

impl SourceCapacityProfileV1 {
    /// Construct a profile and enforce all V1 mathematical/profile constraints.
    pub fn new(
        envelope: CapacityEnvelope,
        max_samples: u16,
        max_recovery_attempts: u8,
        verifier_release_id: ContentId,
        envelope_basis_id: ContentId,
        max_observation_bytes: u32,
        max_shared_children: u32,
    ) -> Result<Self> {
        if max_samples == 0 || max_observation_bytes == 0 {
            return Err(Error::ZeroCapacity);
        }
        if usize::from(max_recovery_attempts) > MAX_RECOVERY_ATTEMPTS {
            return Err(Error::RecoveryExceedsCapacity);
        }
        Ok(Self {
            envelope,
            max_samples,
            max_recovery_attempts,
            verifier_release_id,
            envelope_basis_id,
            max_observation_bytes,
            max_shared_children,
        })
    }

    /// Decode one exact canonical capacity profile.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            SOURCE_CAPACITY_PROFILE_BYTES,
            SOURCE_CAPACITY_PROFILE_MAGIC,
        )?;
        let envelope = CapacityEnvelope::decode(one(bytes, 10)?)?;
        let attempts = one(bytes, 11)?;
        zero(bytes, 14, 2)?;
        zero(bytes, 88, 24)?;
        Self::new(
            envelope,
            u16::from_le_bytes(read_array(bytes, 12)?),
            attempts,
            content(bytes, 16)?,
            content(bytes, 48)?,
            u32::from_le_bytes(read_array(bytes, 80)?),
            u32::from_le_bytes(read_array(bytes, 84)?),
        )
    }

    /// Encode exact canonical capacity-profile bytes.
    pub fn to_bytes(self) -> [u8; SOURCE_CAPACITY_PROFILE_BYTES] {
        let mut out = base::<SOURCE_CAPACITY_PROFILE_BYTES>(SOURCE_CAPACITY_PROFILE_MAGIC);
        put(
            &mut out,
            10,
            &[self.envelope.byte(), self.max_recovery_attempts],
        );
        put(&mut out, 12, &self.max_samples.to_le_bytes());
        put(&mut out, 16, self.verifier_release_id.as_bytes());
        put(&mut out, 48, self.envelope_basis_id.as_bytes());
        put(&mut out, 80, &self.max_observation_bytes.to_le_bytes());
        put(&mut out, 84, &self.max_shared_children.to_le_bytes());
        out
    }

    /// Return the maximum exact sample count.
    pub const fn max_samples(self) -> u16 {
        self.max_samples
    }
    /// Return the maximum ordered recovery attempts.
    pub const fn max_recovery_attempts(self) -> u8 {
        self.max_recovery_attempts
    }

    /// Return the maximum canonical normalized-evidence byte budget.
    pub const fn max_observation_bytes(self) -> u32 {
        self.max_observation_bytes
    }

    /// Return the maximum selected shared-observation children.
    pub const fn max_shared_children(self) -> u32 {
        self.max_shared_children
    }
}

/// How a source supplies observations without making storage universal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceAccessProfile {
    /// Cheap Pyth-style terminal observation admitted in one transaction.
    PythTerminalOneTransaction = 1,
    /// A reusable bounded observation child, shared by compatible policies.
    SharedObservationChild = 2,
}

impl SourceAccessProfile {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::PythTerminalOneTransaction),
            2 => Ok(Self::SharedObservationChild),
            _ => Err(Error::UnknownSourceAccess),
        }
    }
    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Provider-neutral semantics and unit contract for observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecV1 {
    domain_id: ContentId,
    unit_id: ContentId,
    provider_release_id: ContentId,
    access_profile: SourceAccessProfile,
    adapter_config_id: ContentId,
    capacity_profile_id: ContentId,
}

impl SourceSpecV1 {
    /// Construct a source semantic contract.  `adapter_config_id` is adapter
    /// configuration only; Product truth remains in `ResolutionPolicyV1`.
    pub const fn new(
        domain_id: ContentId,
        unit_id: ContentId,
        provider_release_id: ContentId,
        access_profile: SourceAccessProfile,
        adapter_config_id: ContentId,
        capacity_profile_id: ContentId,
    ) -> Self {
        Self {
            domain_id,
            unit_id,
            provider_release_id,
            access_profile,
            adapter_config_id,
            capacity_profile_id,
        }
    }

    /// Decode one exact hostile source preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, SOURCE_SPEC_BYTES, SOURCE_SPEC_MAGIC)?;
        zero(bytes, 11, 5)?;
        zero(bytes, 176, 16)?;
        Ok(Self::new(
            content(bytes, 16)?,
            content(bytes, 48)?,
            content(bytes, 80)?,
            SourceAccessProfile::decode(one(bytes, 10)?)?,
            content(bytes, 112)?,
            content(bytes, 144)?,
        ))
    }

    /// Encode exact canonical source bytes.
    pub fn to_bytes(self) -> [u8; SOURCE_SPEC_BYTES] {
        let mut out = base::<SOURCE_SPEC_BYTES>(SOURCE_SPEC_MAGIC);
        put(&mut out, 10, &[self.access_profile.byte()]);
        put(&mut out, 16, self.domain_id.as_bytes());
        put(&mut out, 48, self.unit_id.as_bytes());
        put(&mut out, 80, self.provider_release_id.as_bytes());
        put(&mut out, 112, self.adapter_config_id.as_bytes());
        put(&mut out, 144, self.capacity_profile_id.as_bytes());
        out
    }

    /// Validate the selected release and capacity identities supplied by the composing layer.
    pub fn validate_dependencies(
        self,
        provider_release_id: ContentId,
        capacity_profile_id: ContentId,
    ) -> Result<()> {
        if self.provider_release_id != provider_release_id
            || self.capacity_profile_id != capacity_profile_id
        {
            return Err(Error::LinkageMismatch);
        }
        Ok(())
    }

    /// Return the observation unit identity.
    pub const fn unit_id(self) -> ContentId {
        self.unit_id
    }
    /// Return the selected access profile.
    pub const fn access_profile(self) -> SourceAccessProfile {
        self.access_profile
    }

    /// Return the semantic observation-domain identity.
    pub const fn domain_id(self) -> ContentId {
        self.domain_id
    }

    /// Return the selected provider-release identity.
    pub const fn provider_release_id(self) -> ContentId {
        self.provider_release_id
    }

    /// Return adapter-only immutable configuration identity.
    pub const fn adapter_config_id(self) -> ContentId {
        self.adapter_config_id
    }

    /// Return the selected source-capacity profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }
}

/// Time interpretation for a resolution window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WindowKind {
    /// One terminal target instant; the start and end must be identical.
    Terminal = 1,
    /// A closed interval sampled on a separately committed finite schedule.
    ScheduledInterval = 2,
}

impl WindowKind {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Terminal),
            2 => Ok(Self::ScheduledInterval),
            _ => Err(Error::UnknownWindowKind),
        }
    }
    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Exact source window and allowed publication skew.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowSpecV1 {
    source_spec_id: ContentId,
    kind: WindowKind,
    start_unix_seconds: i64,
    end_unix_seconds: i64,
    max_age_seconds: u32,
    max_future_skew_seconds: u32,
    schedule_id: ContentId,
}

impl WindowSpecV1 {
    /// Construct a terminal or scheduled interval window.
    pub fn new(
        source_spec_id: ContentId,
        kind: WindowKind,
        start_unix_seconds: i64,
        end_unix_seconds: i64,
        max_age_seconds: u32,
        max_future_skew_seconds: u32,
        schedule_id: ContentId,
    ) -> Result<Self> {
        if max_age_seconds == 0 {
            return Err(Error::InvalidWindow);
        }
        match kind {
            WindowKind::Terminal if start_unix_seconds != end_unix_seconds => {
                return Err(Error::InvalidWindow);
            }
            WindowKind::ScheduledInterval if start_unix_seconds >= end_unix_seconds => {
                return Err(Error::InvalidWindow);
            }
            _ => {}
        }
        Ok(Self {
            source_spec_id,
            kind,
            start_unix_seconds,
            end_unix_seconds,
            max_age_seconds,
            max_future_skew_seconds,
            schedule_id,
        })
    }

    /// Decode one exact hostile window preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, WINDOW_SPEC_BYTES, WINDOW_SPEC_MAGIC)?;
        zero(bytes, 11, 5)?;
        zero(bytes, 104, 8)?;
        Self::new(
            content(bytes, 16)?,
            WindowKind::decode(one(bytes, 10)?)?,
            i64::from_le_bytes(read_array(bytes, 48)?),
            i64::from_le_bytes(read_array(bytes, 56)?),
            u32::from_le_bytes(read_array(bytes, 64)?),
            u32::from_le_bytes(read_array(bytes, 68)?),
            content(bytes, 72)?,
        )
    }

    /// Encode exact canonical window bytes.
    pub fn to_bytes(self) -> [u8; WINDOW_SPEC_BYTES] {
        let mut out = base::<WINDOW_SPEC_BYTES>(WINDOW_SPEC_MAGIC);
        put(&mut out, 10, &[self.kind.byte()]);
        put(&mut out, 16, self.source_spec_id.as_bytes());
        put(&mut out, 48, &self.start_unix_seconds.to_le_bytes());
        put(&mut out, 56, &self.end_unix_seconds.to_le_bytes());
        put(&mut out, 64, &self.max_age_seconds.to_le_bytes());
        put(&mut out, 68, &self.max_future_skew_seconds.to_le_bytes());
        put(&mut out, 72, self.schedule_id.as_bytes());
        out
    }

    /// Check that this window belongs to the supplied source identity.
    pub fn validate_source(self, source_spec_id: ContentId) -> Result<()> {
        if self.source_spec_id != source_spec_id {
            Err(Error::LinkageMismatch)
        } else {
            Ok(())
        }
    }
    /// Return the closed lower time bound.
    pub const fn start_unix_seconds(self) -> i64 {
        self.start_unix_seconds
    }
    /// Return the closed upper time bound.
    pub const fn end_unix_seconds(self) -> i64 {
        self.end_unix_seconds
    }

    /// Return the source-specification identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the exact window kind.
    pub const fn kind(self) -> WindowKind {
        self.kind
    }

    /// Return the maximum admitted provider-publication age.
    pub const fn max_age_seconds(self) -> u32 {
        self.max_age_seconds
    }

    /// Return the maximum admitted future publication skew.
    pub const fn max_future_skew_seconds(self) -> u32 {
        self.max_future_skew_seconds
    }

    /// Return the committed finite-schedule identity.
    pub const fn schedule_id(self) -> ContentId {
        self.schedule_id
    }
}

/// Exact statistic family over finite source observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StatisticKind {
    /// One terminal sample in a terminal window.
    TerminalSample = 1,
    /// Exact sum/count average over a committed equal-time schedule.
    ExactScheduledAverage = 2,
    /// Exact minimum of bounded observations.
    Minimum = 3,
    /// Exact maximum of bounded observations.
    Maximum = 4,
    /// Boolean result of each atom being at least a signed integer threshold.
    AtLeastThreshold = 5,
    /// Boolean result of each atom being at most a signed integer threshold.
    AtMostThreshold = 6,
    /// Exact median of an odd, equally scheduled interval with at least three samples.
    OddScheduledMedian = 7,
}

impl StatisticKind {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::TerminalSample),
            2 => Ok(Self::ExactScheduledAverage),
            3 => Ok(Self::Minimum),
            4 => Ok(Self::Maximum),
            5 => Ok(Self::AtLeastThreshold),
            6 => Ok(Self::AtMostThreshold),
            7 => Ok(Self::OddScheduledMedian),
            _ => Err(Error::UnknownStatistic),
        }
    }
    const fn byte(self) -> u8 {
        self as u8
    }
    const fn is_threshold(self) -> bool {
        matches!(self, Self::AtLeastThreshold | Self::AtMostThreshold)
    }
}

/// The sole explicit rounding boundary before result mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RoundingBoundary {
    /// Pass an exact numerator and positive denominator to the mapping release.
    ExactRational = 1,
    /// Round exact rational values toward negative infinity at this boundary.
    Floor = 2,
    /// Round exact rational values toward positive infinity at this boundary.
    Ceiling = 3,
}

impl RoundingBoundary {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::ExactRational),
            2 => Ok(Self::Floor),
            3 => Ok(Self::Ceiling),
            _ => Err(Error::UnknownRounding),
        }
    }
    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Exact bounded statistic configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticSpecV1 {
    source_unit_id: ContentId,
    result_unit_id: ContentId,
    kind: StatisticKind,
    rounding: RoundingBoundary,
    required_samples: u16,
    threshold_atoms: i128,
    capacity_profile_id: ContentId,
    evaluator_release_id: ContentId,
}

impl StatisticSpecV1 {
    /// Construct a statistic after checking against the authenticated profile.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_unit_id: ContentId,
        result_unit_id: ContentId,
        kind: StatisticKind,
        rounding: RoundingBoundary,
        required_samples: u16,
        threshold_atoms: i128,
        capacity_profile_id: ContentId,
        evaluator_release_id: ContentId,
        profile: SourceCapacityProfileV1,
    ) -> Result<Self> {
        if required_samples == 0 || required_samples > profile.max_samples {
            return Err(Error::StatisticExceedsCapacity);
        }
        if kind == StatisticKind::TerminalSample && required_samples != 1 {
            return Err(Error::NonCanonicalStatistic);
        }
        if kind == StatisticKind::ExactScheduledAverage && required_samples < 2 {
            return Err(Error::NonCanonicalStatistic);
        }
        if kind == StatisticKind::OddScheduledMedian
            && (required_samples < 3
                || required_samples.is_multiple_of(2)
                || rounding != RoundingBoundary::ExactRational)
        {
            return Err(Error::NonCanonicalStatistic);
        }
        if !kind.is_threshold() && threshold_atoms != 0 {
            return Err(Error::NonCanonicalStatistic);
        }
        Ok(Self {
            source_unit_id,
            result_unit_id,
            kind,
            rounding,
            required_samples,
            threshold_atoms,
            capacity_profile_id,
            evaluator_release_id,
        })
    }

    /// Decode structurally canonical bytes. Call `validate_capacity` before use.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, STATISTIC_SPEC_BYTES, STATISTIC_SPEC_MAGIC)?;
        zero(bytes, 12, 4)?;
        zero(bytes, 82, 14)?;
        let value = Self {
            source_unit_id: content(bytes, 16)?,
            result_unit_id: content(bytes, 48)?,
            kind: StatisticKind::decode(one(bytes, 10)?)?,
            rounding: RoundingBoundary::decode(one(bytes, 11)?)?,
            required_samples: u16::from_le_bytes(read_array(bytes, 80)?),
            threshold_atoms: i128::from_le_bytes(read_array(bytes, 96)?),
            capacity_profile_id: content(bytes, 112)?,
            evaluator_release_id: content(bytes, 144)?,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode exact canonical statistic bytes.
    pub fn to_bytes(self) -> [u8; STATISTIC_SPEC_BYTES] {
        let mut out = base::<STATISTIC_SPEC_BYTES>(STATISTIC_SPEC_MAGIC);
        put(&mut out, 10, &[self.kind.byte(), self.rounding.byte()]);
        put(&mut out, 16, self.source_unit_id.as_bytes());
        put(&mut out, 48, self.result_unit_id.as_bytes());
        put(&mut out, 80, &self.required_samples.to_le_bytes());
        put(&mut out, 96, &self.threshold_atoms.to_le_bytes());
        put(&mut out, 112, self.capacity_profile_id.as_bytes());
        put(&mut out, 144, self.evaluator_release_id.as_bytes());
        out
    }

    /// Recheck capacity-profile linkage and its sample bound.
    pub fn validate_capacity(
        self,
        capacity_profile_id: ContentId,
        profile: SourceCapacityProfileV1,
    ) -> Result<()> {
        self.validate_shape()?;
        if self.capacity_profile_id != capacity_profile_id
            || self.required_samples > profile.max_samples
        {
            return Err(Error::LinkageMismatch);
        }
        Ok(())
    }

    /// Return the required source-unit identity.
    pub const fn source_unit_id(self) -> ContentId {
        self.source_unit_id
    }

    /// Return the result-unit identity consumed by the mapping release.
    pub const fn result_unit_id(self) -> ContentId {
        self.result_unit_id
    }

    /// Return the selected statistic family.
    pub const fn kind(self) -> StatisticKind {
        self.kind
    }

    /// Return the one named statistic-to-result rounding boundary.
    pub const fn rounding(self) -> RoundingBoundary {
        self.rounding
    }

    /// Return the exact required observation count.
    pub const fn required_samples(self) -> u16 {
        self.required_samples
    }

    /// Return the exact signed threshold atoms, or zero for non-threshold families.
    pub const fn threshold_atoms(self) -> i128 {
        self.threshold_atoms
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Return the exact evaluator-release identity.
    pub const fn evaluator_release_id(self) -> ContentId {
        self.evaluator_release_id
    }

    fn validate_shape(self) -> Result<()> {
        if self.required_samples == 0 {
            return Err(Error::StatisticExceedsCapacity);
        }
        if self.kind == StatisticKind::TerminalSample && self.required_samples != 1 {
            return Err(Error::NonCanonicalStatistic);
        }
        if self.kind == StatisticKind::ExactScheduledAverage && self.required_samples < 2 {
            return Err(Error::NonCanonicalStatistic);
        }
        if self.kind == StatisticKind::OddScheduledMedian
            && (self.required_samples < 3
                || self.required_samples.is_multiple_of(2)
                || self.rounding != RoundingBoundary::ExactRational)
        {
            return Err(Error::NonCanonicalStatistic);
        }
        if !self.kind.is_threshold() && self.threshold_atoms != 0 {
            return Err(Error::NonCanonicalStatistic);
        }
        Ok(())
    }
}

/// One exact source observation after adapter normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Signed integer source atoms; units come exclusively from `SourceSpecV1`.
    pub atoms: i128,
    /// Unix timestamp selected by the adapter's immutable decoding rules.
    pub unix_seconds: i64,
}

/// Exact evaluator output before or at the named rounding boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticValue {
    /// A positive-denominator exact rational result.
    ExactRational {
        /// Signed exact numerator.
        numerator: i128,
        /// Positive exact denominator.
        denominator: u16,
    },
    /// One signed atom after the named floor or ceiling boundary.
    RoundedAtoms(i128),
}

/// Evaluate a bounded statistic with no float arithmetic or hidden rounding.
pub fn evaluate(
    spec: StatisticSpecV1,
    window: WindowSpecV1,
    observations: &[Observation],
) -> Result<StatisticValue> {
    spec.validate_shape()?;
    if observations.len() != usize::from(spec.required_samples) {
        return Err(Error::InvalidObservationSchedule);
    }
    validate_observation_order(observations)?;
    if spec.kind == StatisticKind::OddScheduledMedian {
        validate_odd_median_schedule(window, observations)?;
    }
    let mut sum = 0i128;
    let mut min = 0i128;
    let mut max = 0i128;
    for (index, item) in observations.iter().enumerate() {
        if item.unix_seconds < window.start_unix_seconds
            || item.unix_seconds > window.end_unix_seconds
        {
            return Err(Error::InvalidObservationSchedule);
        }
        if index == 0 {
            min = item.atoms;
            max = item.atoms;
        } else {
            if item.atoms < min {
                min = item.atoms;
            }
            if item.atoms > max {
                max = item.atoms;
            }
        }
        sum = sum
            .checked_add(item.atoms)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    let raw = match spec.kind {
        StatisticKind::TerminalSample => {
            observations
                .first()
                .ok_or(Error::InvalidObservationSchedule)?
                .atoms
        }
        StatisticKind::ExactScheduledAverage => {
            return finalize(sum, spec.required_samples, spec.rounding);
        }
        StatisticKind::Minimum => min,
        StatisticKind::Maximum => max,
        StatisticKind::AtLeastThreshold => {
            if min >= spec.threshold_atoms {
                1
            } else {
                0
            }
        }
        StatisticKind::AtMostThreshold => {
            if max <= spec.threshold_atoms {
                1
            } else {
                0
            }
        }
        StatisticKind::OddScheduledMedian => exact_median(observations)?,
    };
    finalize(raw, 1, spec.rounding)
}

fn validate_observation_order(observations: &[Observation]) -> Result<()> {
    let mut previous: Option<i64> = None;
    for observation in observations {
        if let Some(timestamp) = previous
            && observation.unix_seconds <= timestamp
        {
            return Err(Error::InvalidObservationSchedule);
        }
        previous = Some(observation.unix_seconds);
    }
    Ok(())
}

fn validate_odd_median_schedule(window: WindowSpecV1, observations: &[Observation]) -> Result<()> {
    if window.kind != WindowKind::ScheduledInterval || observations.len() < 3 {
        return Err(Error::NonCanonicalStatistic);
    }
    let first = observations
        .first()
        .ok_or(Error::InvalidObservationSchedule)?
        .unix_seconds;
    let last = observations
        .last()
        .ok_or(Error::InvalidObservationSchedule)?
        .unix_seconds;
    if first != window.start_unix_seconds || last != window.end_unix_seconds {
        return Err(Error::InvalidObservationSchedule);
    }
    let intervals = i64::try_from(observations.len().saturating_sub(1))
        .map_err(|_| Error::InvalidObservationSchedule)?;
    let span = window
        .end_unix_seconds
        .checked_sub(window.start_unix_seconds)
        .ok_or(Error::ArithmeticOverflow)?;
    if intervals == 0 || span.rem_euclid(intervals) != 0 {
        return Err(Error::InvalidObservationSchedule);
    }
    let cadence = span.div_euclid(intervals);
    if cadence <= 0 {
        return Err(Error::InvalidObservationSchedule);
    }
    for (index, observation) in observations.iter().enumerate() {
        let position = i64::try_from(index).map_err(|_| Error::InvalidObservationSchedule)?;
        let expected = cadence
            .checked_mul(position)
            .and_then(|offset| window.start_unix_seconds.checked_add(offset))
            .ok_or(Error::ArithmeticOverflow)?;
        if observation.unix_seconds != expected {
            return Err(Error::InvalidObservationSchedule);
        }
    }
    Ok(())
}

fn exact_median(observations: &[Observation]) -> Result<i128> {
    let rank = observations.len() / 2;
    for candidate in observations {
        let mut below = 0usize;
        let mut equal = 0usize;
        for item in observations {
            if item.atoms < candidate.atoms {
                below = below.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            } else if item.atoms == candidate.atoms {
                equal = equal.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
        }
        let after_equal = below.checked_add(equal).ok_or(Error::ArithmeticOverflow)?;
        if below <= rank && rank < after_equal {
            return Ok(candidate.atoms);
        }
    }
    Err(Error::InvalidObservationSchedule)
}

fn finalize(
    numerator: i128,
    denominator: u16,
    rounding: RoundingBoundary,
) -> Result<StatisticValue> {
    match rounding {
        RoundingBoundary::ExactRational => Ok(StatisticValue::ExactRational {
            numerator,
            denominator,
        }),
        RoundingBoundary::Floor => Ok(StatisticValue::RoundedAtoms(
            numerator.div_euclid(i128::from(denominator)),
        )),
        RoundingBoundary::Ceiling => {
            let q = numerator.div_euclid(i128::from(denominator));
            let r = numerator.rem_euclid(i128::from(denominator));
            if r == 0 {
                Ok(StatisticValue::RoundedAtoms(q))
            } else {
                Ok(StatisticValue::RoundedAtoms(
                    q.checked_add(1).ok_or(Error::ArithmeticOverflow)?,
                ))
            }
        }
    }
}

/// Immutable policy binding one canonical Product instance to source resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionPolicyV1 {
    capacity_profile_id: ContentId,
    product_instance_id: ContentId,
    source_spec_id: ContentId,
    window_spec_id: ContentId,
    statistic_spec_id: ContentId,
    result_domain_id: ContentId,
    recovery_policy_id: Option<ContentId>,
}

impl ResolutionPolicyV1 {
    /// Construct the single Product-to-source truth linkage.
    pub const fn new(
        capacity_profile_id: ContentId,
        product_instance_id: ContentId,
        source_spec_id: ContentId,
        window_spec_id: ContentId,
        statistic_spec_id: ContentId,
        result_domain_id: ContentId,
        recovery_policy_id: Option<ContentId>,
    ) -> Self {
        Self {
            capacity_profile_id,
            product_instance_id,
            source_spec_id,
            window_spec_id,
            statistic_spec_id,
            result_domain_id,
            recovery_policy_id,
        }
    }
    /// Decode one exact hostile policy preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, RESOLUTION_POLICY_BYTES, RESOLUTION_POLICY_MAGIC)?;
        zero(bytes, 10, 6)?;
        let recovery = read_optional_content(bytes, 208)?;
        Ok(Self::new(
            content(bytes, 16)?,
            content(bytes, 48)?,
            content(bytes, 80)?,
            content(bytes, 112)?,
            content(bytes, 144)?,
            content(bytes, 176)?,
            recovery,
        ))
    }
    /// Encode exact canonical policy bytes.
    pub fn to_bytes(self) -> [u8; RESOLUTION_POLICY_BYTES] {
        let mut out = base::<RESOLUTION_POLICY_BYTES>(RESOLUTION_POLICY_MAGIC);
        put(&mut out, 16, self.capacity_profile_id.as_bytes());
        put(&mut out, 48, self.product_instance_id.as_bytes());
        put(&mut out, 80, self.source_spec_id.as_bytes());
        put(&mut out, 112, self.window_spec_id.as_bytes());
        put(&mut out, 144, self.statistic_spec_id.as_bytes());
        put(&mut out, 176, self.result_domain_id.as_bytes());
        if let Some(id) = self.recovery_policy_id {
            put(&mut out, 208, id.as_bytes());
        }
        out
    }
    /// Validate policy linkage to all supplied authenticated dependencies.
    pub fn validate_links(
        self,
        product_instance_id: ContentId,
        capacity_profile_id: ContentId,
        source_id: ContentId,
        window_id: ContentId,
        statistic_id: ContentId,
        result_domain_id: ContentId,
    ) -> Result<()> {
        if self.product_instance_id != product_instance_id
            || self.capacity_profile_id != capacity_profile_id
            || self.source_spec_id != source_id
            || self.window_spec_id != window_id
            || self.statistic_spec_id != statistic_id
            || self.result_domain_id != result_domain_id
        {
            Err(Error::LinkageMismatch)
        } else {
            Ok(())
        }
    }

    /// Validate Product/source semantics after exact preimage IDs have been
    /// authenticated by the composing hash boundary.
    ///
    /// `result_domain_id` is SHA-256 over
    /// [`FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1`], one zero separator, and the
    /// exact 352-byte Product domain. It is distinct from the domain's embedded
    /// semantic-release identity and from any finalized-record envelope digest.
    /// The pure contract checks the typed linkage; the record adapter must
    /// compute and authenticate this domain-separated content identity.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_material(
        self,
        product_instance_id: ContentId,
        product_instance: InstanceV1,
        source_id: ContentId,
        source: SourceSpecV1,
        window_id: ContentId,
        window: WindowSpecV1,
        statistic_id: ContentId,
        statistic: StatisticSpecV1,
        result_domain: FiniteResultDomainV1,
    ) -> Result<()> {
        self.validate_embedded_material(
            product_instance_id,
            source_id,
            source,
            window_id,
            window,
            statistic_id,
            statistic,
            result_domain,
        )?;
        if product_instance.result_domain_id().to_bytes() != self.result_domain_id.to_bytes()
            || product_instance.partition_cell_count() != u32::from(result_domain.outcome_count())
        {
            return Err(Error::LinkageMismatch);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_embedded_material(
        self,
        product_instance_id: ContentId,
        source_id: ContentId,
        source: SourceSpecV1,
        window_id: ContentId,
        window: WindowSpecV1,
        statistic_id: ContentId,
        statistic: StatisticSpecV1,
        result_domain: FiniteResultDomainV1,
    ) -> Result<()> {
        self.validate_links(
            product_instance_id,
            self.capacity_profile_id,
            source_id,
            window_id,
            statistic_id,
            self.result_domain_id,
        )?;
        window.validate_source(source_id)?;
        if source.capacity_profile_id != self.capacity_profile_id
            || statistic.capacity_profile_id != self.capacity_profile_id
            || statistic.source_unit_id != source.unit_id
            || result_domain.coordinate_domain_id().to_bytes() != source.domain_id().to_bytes()
            || result_domain.result_unit_id().to_bytes() != statistic.result_unit_id().to_bytes()
        {
            return Err(Error::LinkageMismatch);
        }
        if source.access_profile == SourceAccessProfile::PythTerminalOneTransaction
            && (window.kind != WindowKind::Terminal
                || statistic.kind != StatisticKind::TerminalSample)
        {
            return Err(Error::NonCanonicalSourceProfile);
        }
        Ok(())
    }
    /// Return the optional finite recovery-policy identity.
    pub const fn recovery_policy_id(self) -> Option<ContentId> {
        self.recovery_policy_id
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Return the canonical Product-instance content identity.
    pub const fn product_instance_id(self) -> ContentId {
        self.product_instance_id
    }

    /// Return the primary source identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the committed window identity.
    pub const fn window_spec_id(self) -> ContentId {
        self.window_spec_id
    }

    /// Return the committed statistic identity.
    pub const fn statistic_spec_id(self) -> ContentId {
        self.statistic_spec_id
    }

    /// Return the exact Product result-domain content identity.
    pub const fn result_domain_id(self) -> ContentId {
        self.result_domain_id
    }
}

/// Immutable one-attempt recovery record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAttemptV1 {
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    deadline_unix_seconds: i64,
    funding_allocation_id: ContentId,
}

impl RecoveryAttemptV1 {
    /// Construct a prepaid attempt bound to one immutable funding allocation.
    pub const fn new(
        source_spec_id: ContentId,
        provider_release_id: ContentId,
        deadline_unix_seconds: i64,
        funding_allocation_id: ContentId,
    ) -> Self {
        Self {
            source_spec_id,
            provider_release_id,
            deadline_unix_seconds,
            funding_allocation_id,
        }
    }
    fn decode_slot(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RECOVERY_ATTEMPT_BYTES {
            return Err(Error::InvalidLength);
        }
        zero(bytes, 72, 8)?;
        Ok(Self::new(
            content(bytes, 0)?,
            content(bytes, 32)?,
            i64::from_le_bytes(read_array(bytes, 64)?),
            content(bytes, 80)?,
        ))
    }
    fn to_slot_bytes(self) -> [u8; RECOVERY_ATTEMPT_BYTES] {
        let mut out = [0u8; RECOVERY_ATTEMPT_BYTES];
        put(&mut out, 0, self.source_spec_id.as_bytes());
        put(&mut out, 32, self.provider_release_id.as_bytes());
        put(&mut out, 64, &self.deadline_unix_seconds.to_le_bytes());
        put(&mut out, 80, self.funding_allocation_id.as_bytes());
        out
    }

    /// Return the exact recovery source identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the exact provider-release identity.
    pub const fn provider_release_id(self) -> ContentId {
        self.provider_release_id
    }

    /// Return the inclusive recovery-attempt deadline.
    pub const fn deadline_unix_seconds(self) -> i64 {
        self.deadline_unix_seconds
    }

    /// Return the immutable capability funding-allocation identity.
    pub const fn funding_allocation_id(self) -> ContentId {
        self.funding_allocation_id
    }
}

/// Finite ordered recovery plan; failure is legal only after every attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryPolicyV1 {
    capacity_profile_id: ContentId,
    product_instance_id: ContentId,
    attempts: [Option<RecoveryAttemptV1>; MAX_RECOVERY_ATTEMPTS],
    attempt_count: u8,
}

impl RecoveryPolicyV1 {
    /// Construct an ordered finite recovery plan under its authenticated profile.
    pub fn new(
        capacity_profile_id: ContentId,
        product_instance_id: ContentId,
        attempts: [Option<RecoveryAttemptV1>; MAX_RECOVERY_ATTEMPTS],
        attempt_count: u8,
        profile: SourceCapacityProfileV1,
    ) -> Result<Self> {
        if attempt_count == 0
            || attempt_count > profile.max_recovery_attempts
            || usize::from(attempt_count) > MAX_RECOVERY_ATTEMPTS
        {
            return Err(Error::RecoveryExceedsCapacity);
        }
        let value = Self {
            capacity_profile_id,
            product_instance_id,
            attempts,
            attempt_count,
        };
        value.validate_shape()?;
        Ok(value)
    }
    /// Decode structurally canonical recovery bytes. Call `validate_capacity` before use.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, RECOVERY_POLICY_BYTES, RECOVERY_POLICY_MAGIC)?;
        let count = one(bytes, 10)?;
        zero(bytes, 11, 5)?;
        let mut attempts = [None; MAX_RECOVERY_ATTEMPTS];
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let offset = 80usize
                .checked_add(
                    index
                        .checked_mul(RECOVERY_ATTEMPT_BYTES)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            let slot = bytes
                .get(
                    offset
                        ..offset
                            .checked_add(RECOVERY_ATTEMPT_BYTES)
                            .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::InvalidLength)?;
            if index < usize::from(count) {
                let item = RecoveryAttemptV1::decode_slot(slot)?;
                if let Some(place) = attempts.get_mut(index) {
                    *place = Some(item);
                }
            } else {
                zero(slot, 0, RECOVERY_ATTEMPT_BYTES)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let value = Self {
            capacity_profile_id: content(bytes, 16)?,
            product_instance_id: content(bytes, 48)?,
            attempts,
            attempt_count: count,
        };
        value.validate_shape()?;
        Ok(value)
    }
    /// Encode exact canonical recovery-policy bytes.
    pub fn to_bytes(self) -> [u8; RECOVERY_POLICY_BYTES] {
        let mut out = base::<RECOVERY_POLICY_BYTES>(RECOVERY_POLICY_MAGIC);
        put(&mut out, 10, &[self.attempt_count]);
        put(&mut out, 16, self.capacity_profile_id.as_bytes());
        put(&mut out, 48, self.product_instance_id.as_bytes());
        for (index, attempt) in self.attempts.iter().enumerate() {
            if let Some(value) = attempt {
                let offset = 80usize.saturating_add(index.saturating_mul(RECOVERY_ATTEMPT_BYTES));
                put(&mut out, offset, &value.to_slot_bytes());
            }
        }
        out
    }
    /// Recheck capacity and canonical Product-instance linkage.
    pub fn validate_capacity(
        self,
        capacity_profile_id: ContentId,
        product_instance_id: ContentId,
        profile: SourceCapacityProfileV1,
    ) -> Result<()> {
        if self.capacity_profile_id != capacity_profile_id
            || self.product_instance_id != product_instance_id
        {
            return Err(Error::LinkageMismatch);
        }
        if self.attempt_count > profile.max_recovery_attempts {
            return Err(Error::RecoveryExceedsCapacity);
        }
        self.validate_shape()
    }
    fn validate_shape(self) -> Result<()> {
        if self.attempt_count == 0 || usize::from(self.attempt_count) > MAX_RECOVERY_ATTEMPTS {
            return Err(Error::RecoveryExceedsCapacity);
        }
        let mut prior: Option<i64> = None;
        for (index, slot) in self.attempts.iter().enumerate() {
            if index < usize::from(self.attempt_count) {
                let current = slot.ok_or(Error::NonCanonicalReservedBytes)?;
                if let Some(previous) = prior
                    && current.deadline_unix_seconds <= previous
                {
                    return Err(Error::NonCanonicalRecoveryOrder);
                }
                prior = Some(current.deadline_unix_seconds);
            } else if slot.is_some() {
                return Err(Error::NonCanonicalReservedBytes);
            }
        }
        Ok(())
    }
    /// Return the number of ordered committed attempts.
    pub const fn attempt_count(self) -> u8 {
        self.attempt_count
    }

    /// Return the selected capacity-profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Return the canonical Product-instance content identity.
    pub const fn product_instance_id(self) -> ContentId {
        self.product_instance_id
    }

    /// Return one exact committed attempt, refusing every inactive or out-of-range slot.
    pub fn attempt(self, index: u8) -> Result<RecoveryAttemptV1> {
        if index >= self.attempt_count {
            return Err(Error::InvalidRecoveryTransition);
        }
        self.attempts
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Error::NonCanonicalReservedBytes)
    }
}

/// Exact normalized evidence emitted by one authenticated provider adapter.
///
/// This is not itself proof of provider authenticity. The adapter selected by
/// `provider_release_id` authenticates provider bytes and their content ID,
/// then supplies these provider-neutral integer facts to this contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedProviderEvidenceV1 {
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    provider_evidence_id: ContentId,
    adapter_release_id: ContentId,
    schedule_id: ContentId,
    schedule_index: u16,
    observation_unix_seconds: i64,
    publication_unix_seconds: i64,
    atoms: i128,
}

impl NormalizedProviderEvidenceV1 {
    /// Construct one exact normalized evidence record.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        source_spec_id: ContentId,
        provider_release_id: ContentId,
        provider_evidence_id: ContentId,
        adapter_release_id: ContentId,
        schedule_id: ContentId,
        schedule_index: u16,
        observation_unix_seconds: i64,
        publication_unix_seconds: i64,
        atoms: i128,
    ) -> Self {
        Self {
            source_spec_id,
            provider_release_id,
            provider_evidence_id,
            adapter_release_id,
            schedule_id,
            schedule_index,
            observation_unix_seconds,
            publication_unix_seconds,
            atoms,
        }
    }

    /// Decode one exact hostile normalized record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, NORMALIZED_EVIDENCE_BYTES, NORMALIZED_EVIDENCE_MAGIC)?;
        zero(bytes, 12, 4)?;
        Ok(Self::new(
            content(bytes, 16)?,
            content(bytes, 48)?,
            content(bytes, 80)?,
            content(bytes, 112)?,
            content(bytes, 144)?,
            u16::from_le_bytes(read_array(bytes, 10)?),
            i64::from_le_bytes(read_array(bytes, 176)?),
            i64::from_le_bytes(read_array(bytes, 184)?),
            i128::from_le_bytes(read_array(bytes, 192)?),
        ))
    }

    /// Encode exact canonical normalized evidence bytes.
    pub fn to_bytes(self) -> [u8; NORMALIZED_EVIDENCE_BYTES] {
        let mut out = base::<NORMALIZED_EVIDENCE_BYTES>(NORMALIZED_EVIDENCE_MAGIC);
        put(&mut out, 10, &self.schedule_index.to_le_bytes());
        put(&mut out, 16, self.source_spec_id.as_bytes());
        put(&mut out, 48, self.provider_release_id.as_bytes());
        put(&mut out, 80, self.provider_evidence_id.as_bytes());
        put(&mut out, 112, self.adapter_release_id.as_bytes());
        put(&mut out, 144, self.schedule_id.as_bytes());
        put(&mut out, 176, &self.observation_unix_seconds.to_le_bytes());
        put(&mut out, 184, &self.publication_unix_seconds.to_le_bytes());
        put(&mut out, 192, &self.atoms.to_le_bytes());
        out
    }

    /// Validate immutable linkage, closed window, schedule position, and Clock-relative age.
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        self,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        window: WindowSpecV1,
        expected_schedule_index: u16,
        current_unix_seconds: i64,
    ) -> Result<Observation> {
        if self.source_spec_id != source_spec_id
            || self.provider_release_id != provider_release_id
            || source.provider_release_id != provider_release_id
            || self.adapter_release_id != provider_release.adapter_release_id
            || self.schedule_id != window.schedule_id
            || self.schedule_index != expected_schedule_index
        {
            return Err(Error::LinkageMismatch);
        }
        if self.observation_unix_seconds < window.start_unix_seconds
            || self.observation_unix_seconds > window.end_unix_seconds
        {
            return Err(Error::InvalidObservationSchedule);
        }
        let oldest = current_unix_seconds
            .checked_sub(i64::from(window.max_age_seconds))
            .ok_or(Error::ArithmeticOverflow)?;
        let newest = current_unix_seconds
            .checked_add(i64::from(window.max_future_skew_seconds))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.publication_unix_seconds < oldest || self.publication_unix_seconds > newest {
            return Err(Error::InvalidPublicationTime);
        }
        Ok(Observation {
            atoms: self.atoms,
            unix_seconds: self.observation_unix_seconds,
        })
    }

    /// Return the source-specification identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the provider-release identity.
    pub const fn provider_release_id(self) -> ContentId {
        self.provider_release_id
    }

    /// Return the content identity of the provider evidence authenticated by the adapter.
    pub const fn provider_evidence_id(self) -> ContentId {
        self.provider_evidence_id
    }

    /// Return the adapter release that performed provider authentication.
    pub const fn adapter_release_id(self) -> ContentId {
        self.adapter_release_id
    }

    /// Return the committed schedule identity.
    pub const fn schedule_id(self) -> ContentId {
        self.schedule_id
    }

    /// Return the zero-based schedule position.
    pub const fn schedule_index(self) -> u16 {
        self.schedule_index
    }

    /// Return the normalized observation timestamp.
    pub const fn observation_unix_seconds(self) -> i64 {
        self.observation_unix_seconds
    }

    /// Return the provider publication timestamp selected by decoding rules.
    pub const fn publication_unix_seconds(self) -> i64 {
        self.publication_unix_seconds
    }

    /// Return exact signed source atoms.
    pub const fn atoms(self) -> i128 {
        self.atoms
    }
}

/// Exact pure obligation handed to the named Pyth SVM provider adapter.
///
/// Construction proves only immutable Source-material linkage. The SVM
/// adapter must additionally authenticate the pinned Pyth release, program and
/// ProgramData, deployment slots, config digest, router, message ownership,
/// fully verified posted update, exact feed/config semantics, and reclaim
/// postconditions before calling [`Self::normalize_authenticated_update`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythProviderAdapterObligationV1 {
    source_spec_id: ContentId,
    source: SourceSpecV1,
    provider_release_id: ContentId,
    provider_release: ProviderReleaseV1,
    adapter_config: PythAdapterConfigV1,
}

impl PythProviderAdapterObligationV1 {
    /// Select the closed Pyth extension from one embedded material source.
    pub fn from_material(material: SourceMaterialV1, source_spec_id: ContentId) -> Result<Self> {
        let (source, provider_release_id, provider_release) = material.source(source_spec_id)?;
        let adapter_config = material.adapter_config(source_spec_id)?;
        if provider_release.adapter_release_id().to_bytes() != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        {
            return Err(Error::UnsupportedProviderExtension);
        }
        Ok(Self {
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config,
        })
    }

    /// Select the closed Pyth extension from a borrowed runtime material view.
    pub fn from_material_view(
        material: SourceMaterialViewV1<'_>,
        source_spec_id: ContentId,
    ) -> Result<Self> {
        let (source, provider_release_id, provider_release) = material.source(source_spec_id)?;
        let adapter_config = material.adapter_config(source_spec_id)?;
        if provider_release.adapter_release_id().to_bytes() != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        {
            return Err(Error::UnsupportedProviderExtension);
        }
        Ok(Self {
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config,
        })
    }

    /// Normalize facts only after the SVM adapter discharged the documented
    /// Pyth account, release, message, update, and reclaim obligations.
    #[allow(clippy::too_many_arguments)]
    pub fn normalize_authenticated_update(
        self,
        provider_evidence_id: ContentId,
        schedule_id: ContentId,
        schedule_index: u16,
        provider_feed_id: [u8; 32],
        price: i64,
        confidence: u64,
        exponent: i32,
        publication_unix_seconds: i64,
    ) -> Result<NormalizedProviderEvidenceV1> {
        let atoms =
            self.adapter_config
                .validate_update(provider_feed_id, price, confidence, exponent)?;
        Ok(NormalizedProviderEvidenceV1::new(
            self.source_spec_id,
            self.provider_release_id,
            provider_evidence_id,
            self.provider_release.adapter_release_id(),
            schedule_id,
            schedule_index,
            publication_unix_seconds,
            publication_unix_seconds,
            atoms,
        ))
    }

    /// Return the exact adapter configuration selected by the Source.
    pub const fn adapter_config_id(self) -> ContentId {
        self.source.adapter_config_id()
    }

    /// Return the embedded configuration whose content digest is selected by
    /// [`Self::adapter_config_id`].
    pub const fn adapter_config(self) -> PythAdapterConfigV1 {
        self.adapter_config
    }

    /// Return the exact embedded provider release.
    pub const fn provider_release(self) -> ProviderReleaseV1 {
        self.provider_release
    }
}

fn statistic_rational(value: StatisticValue) -> (i128, u64) {
    match value {
        StatisticValue::ExactRational {
            numerator,
            denominator,
        } => (numerator, u64::from(denominator)),
        StatisticValue::RoundedAtoms(atoms) => (atoms, 1),
    }
}

/// Narrow immutable linkage for a source state created for a successor generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReopenLinkV1 {
    market: [u8; 32],
    predecessor_state_id: ContentId,
    previous_generation: u64,
    next_generation: u64,
    predecessor_terminal_evidence_id: ContentId,
}

impl ReopenLinkV1 {
    /// Construct a link only for the exact next generation of the same Market key.
    pub fn new(
        market: [u8; 32],
        predecessor_state_id: ContentId,
        previous_generation: u64,
        next_generation: u64,
        predecessor_terminal_evidence_id: ContentId,
    ) -> Result<Self> {
        nonzero_identifier(&market)?;
        if previous_generation.checked_add(1) != Some(next_generation) {
            return Err(Error::InvalidReopenLink);
        }
        Ok(Self {
            market,
            predecessor_state_id,
            previous_generation,
            next_generation,
            predecessor_terminal_evidence_id,
        })
    }

    /// Decode one exact hostile reopen link.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, REOPEN_LINK_BYTES, REOPEN_LINK_MAGIC)?;
        zero(bytes, 10, 6)?;
        Self::new(
            read_array(bytes, 16)?,
            content(bytes, 48)?,
            u64::from_le_bytes(read_array(bytes, 80)?),
            u64::from_le_bytes(read_array(bytes, 88)?),
            content(bytes, 96)?,
        )
    }

    /// Encode exact canonical reopen-link bytes.
    pub fn to_bytes(self) -> [u8; REOPEN_LINK_BYTES] {
        let mut out = base::<REOPEN_LINK_BYTES>(REOPEN_LINK_MAGIC);
        put(&mut out, 16, &self.market);
        put(&mut out, 48, self.predecessor_state_id.as_bytes());
        put(&mut out, 80, &self.previous_generation.to_le_bytes());
        put(&mut out, 88, &self.next_generation.to_le_bytes());
        put(
            &mut out,
            96,
            self.predecessor_terminal_evidence_id.as_bytes(),
        );
        out
    }

    /// Validate the successor Market binding supplied by the composing adapter.
    pub fn validate_successor(self, market: [u8; 32], generation: u64) -> Result<()> {
        if self.market != market || self.next_generation != generation {
            return Err(Error::InvalidReopenLink);
        }
        Ok(())
    }

    /// Return the Market account key bytes.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the authenticated predecessor-state content identity.
    pub const fn predecessor_state_id(self) -> ContentId {
        self.predecessor_state_id
    }

    /// Return the predecessor generation.
    pub const fn previous_generation(self) -> u64 {
        self.previous_generation
    }

    /// Return the exactly-next generation.
    pub const fn next_generation(self) -> u64 {
        self.next_generation
    }

    /// Return the predecessor terminal-evidence identity.
    pub const fn predecessor_terminal_evidence_id(self) -> ContentId {
        self.predecessor_terminal_evidence_id
    }
}

/// Provider-neutral terminal route emitted for Market settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceResolutionRouteV1 {
    /// The primary source resolved the occurrence.
    Primary = 1,
    /// The active ordered recovery attempt resolved the occurrence.
    Recovery = 2,
    /// Exhaustion selected the Product-owned failure result.
    Failure = 3,
}

impl SourceResolutionRouteV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Primary),
            2 => Ok(Self::Recovery),
            3 => Ok(Self::Failure),
            _ => Err(Error::NonCanonicalState),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Adapter-consumable provider-neutral settlement decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionDecisionV1 {
    route: SourceResolutionRouteV1,
    selector: u8,
    outcome_count: u8,
    resolution_evidence_id: ContentId,
    terminal_sequence: u64,
}

impl SourceResolutionDecisionV1 {
    fn new(
        route: SourceResolutionRouteV1,
        selector: u8,
        outcome_count: u8,
        resolution_evidence_id: ContentId,
        terminal_sequence: u64,
    ) -> Result<Self> {
        if terminal_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        if outcome_count < 2 || selector >= outcome_count {
            return Err(Error::InvalidResultSelector);
        }
        Ok(Self {
            route,
            selector,
            outcome_count,
            resolution_evidence_id,
            terminal_sequence,
        })
    }

    /// Return the provider-neutral primary/recovery/failure route.
    pub const fn route(self) -> SourceResolutionRouteV1 {
        self.route
    }

    /// Return the zero-based Product result cell.
    pub const fn selector(self) -> u8 {
        self.selector
    }

    /// Return the exact Product result width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Return the content identity of accepted evidence.
    pub const fn resolution_evidence_id(self) -> ContentId {
        self.resolution_evidence_id
    }

    /// Return the positive terminal replay sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

/// One embedded recovery source and its exact provider release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryMaterialSlotV1 {
    source_spec_id: ContentId,
    source: SourceSpecV1,
    provider_release_id: ContentId,
    provider_release: ProviderReleaseV1,
    adapter_config: PythAdapterConfigV1,
}

impl RecoveryMaterialSlotV1 {
    /// Construct one by-value recovery material slot.
    pub fn new(
        source_spec_id: ContentId,
        source: SourceSpecV1,
        provider_release_id: ContentId,
        provider_release: ProviderReleaseV1,
        adapter_config: PythAdapterConfigV1,
    ) -> Result<Self> {
        if source.provider_release_id() != provider_release_id
            || provider_release.adapter_release_id().to_bytes()
                != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        {
            return Err(Error::UnsupportedProviderExtension);
        }
        Ok(Self {
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config,
        })
    }

    /// Return the embedded source identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the embedded source specification.
    pub const fn source(self) -> SourceSpecV1 {
        self.source
    }

    /// Return the embedded provider-release identity.
    pub const fn provider_release_id(self) -> ContentId {
        self.provider_release_id
    }

    /// Return the embedded provider release.
    pub const fn provider_release(self) -> ProviderReleaseV1 {
        self.provider_release
    }

    /// Return the embedded Pyth adapter configuration.
    pub const fn adapter_config(self) -> PythAdapterConfigV1 {
        self.adapter_config
    }
}

/// The single canonical immutable Source authority preimage.
///
/// A Market's `resolution_policy_id` names the content digest of these exact
/// bytes. All policy, canonical Product instance/domain, capacity, source,
/// provider, window, statistic, and ordered-recovery semantics are embedded by
/// value; no operation accepts those components as separately authenticated
/// records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMaterialV1 {
    policy: ResolutionPolicyV1,
    capacity_profile_id: ContentId,
    capacity_profile: SourceCapacityProfileV1,
    primary_source_id: ContentId,
    primary_source: SourceSpecV1,
    primary_provider_release_id: ContentId,
    primary_provider_release: ProviderReleaseV1,
    primary_adapter_config: PythAdapterConfigV1,
    window_id: ContentId,
    window: WindowSpecV1,
    statistic_id: ContentId,
    statistic: StatisticSpecV1,
    product_instance_id: ContentId,
    result_domain: FiniteResultDomainV1,
    recovery: Option<(ContentId, RecoveryPolicyV1)>,
    recovery_slots: [Option<RecoveryMaterialSlotV1>; MAX_RECOVERY_ATTEMPTS],
}

impl SourceMaterialV1 {
    /// Construct and cross-validate the one immutable Source material graph.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        policy: ResolutionPolicyV1,
        capacity_profile_id: ContentId,
        capacity_profile: SourceCapacityProfileV1,
        primary_source_id: ContentId,
        primary_source: SourceSpecV1,
        primary_provider_release_id: ContentId,
        primary_provider_release: ProviderReleaseV1,
        primary_adapter_config: PythAdapterConfigV1,
        window_id: ContentId,
        window: WindowSpecV1,
        statistic_id: ContentId,
        statistic: StatisticSpecV1,
        product_instance_id: ContentId,
        product_instance: InstanceV1,
        result_domain: FiniteResultDomainV1,
        recovery: Option<(ContentId, RecoveryPolicyV1)>,
        recovery_slots: [Option<RecoveryMaterialSlotV1>; MAX_RECOVERY_ATTEMPTS],
    ) -> Result<Self> {
        policy.validate_material(
            product_instance_id,
            product_instance,
            primary_source_id,
            primary_source,
            window_id,
            window,
            statistic_id,
            statistic,
            result_domain,
        )?;
        statistic.validate_capacity(capacity_profile_id, capacity_profile)?;
        if policy.capacity_profile_id != capacity_profile_id
            || primary_source.provider_release_id() != primary_provider_release_id
            || primary_provider_release.adapter_release_id().to_bytes()
                != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
            || result_domain.coordinate_domain_id().to_bytes()
                != primary_source.domain_id().to_bytes()
            || result_domain.result_unit_id().to_bytes() != statistic.result_unit_id().to_bytes()
        {
            return Err(Error::LinkageMismatch);
        }
        result_domain
            .validate()
            .map_err(|_| Error::InvalidResultMap)?;
        match (policy.recovery_policy_id(), recovery) {
            (None, None) => {
                if recovery_slots.iter().any(Option::is_some) {
                    return Err(Error::NonCanonicalSourceMaterial);
                }
            }
            (Some(expected_id), Some((actual_id, recovery_policy))) => {
                if expected_id != actual_id {
                    return Err(Error::LinkageMismatch);
                }
                recovery_policy.validate_capacity(
                    capacity_profile_id,
                    product_instance_id,
                    capacity_profile,
                )?;
                let active = usize::from(recovery_policy.attempt_count());
                let mut index = 0usize;
                while index < MAX_RECOVERY_ATTEMPTS {
                    let slot = recovery_slots.get(index).copied().flatten();
                    match (index < active, slot) {
                        (true, Some(slot)) => {
                            let attempt = recovery_policy.attempt(
                                u8::try_from(index).map_err(|_| Error::ArithmeticOverflow)?,
                            )?;
                            if slot.source_spec_id != attempt.source_spec_id()
                                || slot.provider_release_id != attempt.provider_release_id()
                            {
                                return Err(Error::LinkageMismatch);
                            }
                            slot.source.validate_dependencies(
                                slot.provider_release_id,
                                capacity_profile_id,
                            )?;
                            validate_recovery_source(primary_source, slot.source)?;
                        }
                        (false, None) => {}
                        _ => return Err(Error::NonCanonicalSourceMaterial),
                    }
                    index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
            }
            _ => return Err(Error::LinkageMismatch),
        }
        let value = Self {
            policy,
            capacity_profile_id,
            capacity_profile,
            primary_source_id,
            primary_source,
            primary_provider_release_id,
            primary_provider_release,
            primary_adapter_config,
            window_id,
            window,
            statistic_id,
            statistic,
            product_instance_id,
            result_domain,
            recovery,
            recovery_slots,
        };
        if value.primary_source.access_profile() == SourceAccessProfile::SharedObservationChild
            && usize::from(value.statistic.required_samples()) > MAX_SHARED_OBSERVATIONS
        {
            return Err(Error::StatisticExceedsCapacity);
        }
        Ok(value)
    }

    /// Decode one exact hostile Source-material preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        validate_source_material_bytes_v1(bytes)?;
        let policy = ResolutionPolicyV1::decode(slice(bytes, 16, RESOLUTION_POLICY_BYTES)?)?;
        let capacity_profile_id = content(bytes, 256)?;
        let capacity_profile =
            SourceCapacityProfileV1::decode(slice(bytes, 288, SOURCE_CAPACITY_PROFILE_BYTES)?)?;
        let primary_source_id = content(bytes, 400)?;
        let primary_source = SourceSpecV1::decode(slice(bytes, 432, SOURCE_SPEC_BYTES)?)?;
        let window_id = content(bytes, 624)?;
        let window = WindowSpecV1::decode(slice(bytes, 656, WINDOW_SPEC_BYTES)?)?;
        let statistic_id = content(bytes, 768)?;
        let statistic = StatisticSpecV1::decode(slice(bytes, 800, STATISTIC_SPEC_BYTES)?)?;
        let product_instance_id = content(bytes, 976)?;
        let result_domain =
            FiniteResultDomainV1::decode(slice(bytes, 1008, FINITE_RESULT_DOMAIN_BYTES)?)
                .map_err(|_| Error::InvalidResultMap)?;
        let primary_provider_release_id = content(bytes, 1360)?;
        let primary_provider_release =
            ProviderReleaseV1::decode(slice(bytes, 1392, PROVIDER_RELEASE_BYTES)?)?;
        let primary_adapter_config =
            PythAdapterConfigV1::decode(slice(bytes, 1568, PYTH_ADAPTER_CONFIG_BYTES)?)?;
        let recovery_id = read_optional_content(bytes, 1632)?;
        let recovery = match recovery_id {
            Some(id) => Some((
                id,
                RecoveryPolicyV1::decode(slice(bytes, 1664, RECOVERY_POLICY_BYTES)?)?,
            )),
            None => {
                zero(bytes, 1664, RECOVERY_POLICY_BYTES)?;
                None
            }
        };
        let mut recovery_slots = [None; MAX_RECOVERY_ATTEMPTS];
        let active = recovery.map_or(0, |(_, value)| usize::from(value.attempt_count()));
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let source_offset = 2192usize
                .checked_add(index.checked_mul(224).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let provider_offset = 3088usize
                .checked_add(index.checked_mul(208).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let config_offset = 3920usize
                .checked_add(
                    index
                        .checked_mul(PYTH_ADAPTER_CONFIG_BYTES)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if index < active {
                let slot = recovery_slots
                    .get_mut(index)
                    .ok_or(Error::ArithmeticOverflow)?;
                *slot = Some(RecoveryMaterialSlotV1::new(
                    content(bytes, source_offset)?,
                    SourceSpecV1::decode(slice(bytes, source_offset + 32, SOURCE_SPEC_BYTES)?)?,
                    content(bytes, provider_offset)?,
                    ProviderReleaseV1::decode(slice(
                        bytes,
                        provider_offset + 32,
                        PROVIDER_RELEASE_BYTES,
                    )?)?,
                    PythAdapterConfigV1::decode(slice(
                        bytes,
                        config_offset,
                        PYTH_ADAPTER_CONFIG_BYTES,
                    )?)?,
                )?);
            } else {
                zero(bytes, source_offset, 224)?;
                zero(bytes, provider_offset, 208)?;
                zero(bytes, config_offset, PYTH_ADAPTER_CONFIG_BYTES)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(Self {
            policy,
            capacity_profile_id,
            capacity_profile,
            primary_source_id,
            primary_source,
            primary_provider_release_id,
            primary_provider_release,
            primary_adapter_config,
            window_id,
            window,
            statistic_id,
            statistic,
            product_instance_id,
            result_domain,
            recovery,
            recovery_slots,
        })
    }

    /// Encode the one exact canonical Source-material preimage.
    pub fn to_bytes(self) -> [u8; SOURCE_MATERIAL_BYTES] {
        let mut out = base::<SOURCE_MATERIAL_BYTES>(SOURCE_MATERIAL_MAGIC);
        put(&mut out, 16, &self.policy.to_bytes());
        put(&mut out, 256, self.capacity_profile_id.as_bytes());
        put(&mut out, 288, &self.capacity_profile.to_bytes());
        put(&mut out, 400, self.primary_source_id.as_bytes());
        put(&mut out, 432, &self.primary_source.to_bytes());
        put(&mut out, 624, self.window_id.as_bytes());
        put(&mut out, 656, &self.window.to_bytes());
        put(&mut out, 768, self.statistic_id.as_bytes());
        put(&mut out, 800, &self.statistic.to_bytes());
        put(&mut out, 976, self.product_instance_id.as_bytes());
        put(&mut out, 1008, &self.result_domain.to_bytes());
        put(&mut out, 1360, self.primary_provider_release_id.as_bytes());
        put(&mut out, 1392, &self.primary_provider_release.to_bytes());
        put(&mut out, 1568, &self.primary_adapter_config.to_bytes());
        if let Some((id, recovery)) = self.recovery {
            put(&mut out, 1632, id.as_bytes());
            put(&mut out, 1664, &recovery.to_bytes());
        }
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            if let Some(slot) = self.recovery_slots.get(index).copied().flatten() {
                let source_offset = 2192 + index * 224;
                let provider_offset = 3088 + index * 208;
                let config_offset = 3920 + index * PYTH_ADAPTER_CONFIG_BYTES;
                put(&mut out, source_offset, slot.source_spec_id.as_bytes());
                put(&mut out, source_offset + 32, &slot.source.to_bytes());
                put(
                    &mut out,
                    provider_offset,
                    slot.provider_release_id.as_bytes(),
                );
                put(
                    &mut out,
                    provider_offset + 32,
                    &slot.provider_release.to_bytes(),
                );
                put(&mut out, config_offset, &slot.adapter_config.to_bytes());
            }
            index += 1;
        }
        out
    }

    /// Return the immutable policy.
    pub const fn policy(self) -> ResolutionPolicyV1 {
        self.policy
    }

    /// Return the primary source identity.
    pub const fn primary_source_id(self) -> ContentId {
        self.primary_source_id
    }

    /// Return the primary source.
    pub const fn primary_source(self) -> SourceSpecV1 {
        self.primary_source
    }

    /// Return the primary provider release and its embedded identity.
    pub const fn primary_provider_release(self) -> (ContentId, ProviderReleaseV1) {
        (
            self.primary_provider_release_id,
            self.primary_provider_release,
        )
    }

    /// Return the embedded primary Pyth adapter configuration.
    pub const fn primary_adapter_config(self) -> PythAdapterConfigV1 {
        self.primary_adapter_config
    }

    /// Return the source-capacity identity and profile.
    pub const fn capacity_profile(self) -> (ContentId, SourceCapacityProfileV1) {
        (self.capacity_profile_id, self.capacity_profile)
    }

    /// Return the window identity and specification.
    pub const fn window_spec(self) -> (ContentId, WindowSpecV1) {
        (self.window_id, self.window)
    }

    /// Return the immutable window.
    pub const fn window(self) -> WindowSpecV1 {
        self.window
    }

    /// Return the immutable statistic.
    pub const fn statistic(self) -> StatisticSpecV1 {
        self.statistic
    }

    /// Return the canonical external Product-instance content identity.
    pub const fn product_instance_id(self) -> ContentId {
        self.product_instance_id
    }

    /// Return the sole Product-owned finite result-domain authority.
    pub const fn result_domain(self) -> FiniteResultDomainV1 {
        self.result_domain
    }

    /// Return the optional ordered recovery policy by value.
    pub const fn recovery_policy(self) -> Option<(ContentId, RecoveryPolicyV1)> {
        self.recovery
    }

    /// Return one exact active recovery material slot.
    pub fn recovery_slot(self, index: u8) -> Result<RecoveryMaterialSlotV1> {
        let (_, recovery) = self.recovery.ok_or(Error::LinkageMismatch)?;
        recovery.attempt(index)?;
        self.recovery_slots
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Error::NonCanonicalSourceMaterial)
    }

    /// Select one embedded source and provider release by exact source ID.
    pub fn source(
        self,
        source_spec_id: ContentId,
    ) -> Result<(SourceSpecV1, ContentId, ProviderReleaseV1)> {
        if source_spec_id == self.primary_source_id {
            return Ok((
                self.primary_source,
                self.primary_provider_release_id,
                self.primary_provider_release,
            ));
        }
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            if let Some(slot) = self.recovery_slots.get(index).copied().flatten()
                && slot.source_spec_id == source_spec_id
            {
                return Ok((slot.source, slot.provider_release_id, slot.provider_release));
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Err(Error::LinkageMismatch)
    }

    /// Select the embedded Pyth adapter configuration for one exact source.
    pub fn adapter_config(self, source_spec_id: ContentId) -> Result<PythAdapterConfigV1> {
        if source_spec_id == self.primary_source_id {
            return Ok(self.primary_adapter_config);
        }
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            if let Some(slot) = self.recovery_slots.get(index).copied().flatten()
                && slot.source_spec_id == source_spec_id
            {
                return Ok(slot.adapter_config);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Err(Error::LinkageMismatch)
    }
}

/// Borrowed, fully validated view of one canonical Source-material preimage.
///
/// The view keeps the 4,176-byte authority in caller-owned account memory and
/// decodes only one bounded component at a time. It is the executable runtime
/// representation; [`SourceMaterialV1`] remains the convenient by-value
/// construction representation for offline tooling and host tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMaterialViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> SourceMaterialViewV1<'a> {
    /// Validate and borrow one exact hostile Source-material preimage.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        validate_source_material_bytes_v1(bytes)?;
        Ok(Self { bytes })
    }

    /// Return the exact canonical bytes retained by this view.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Return the immutable policy.
    pub fn policy(self) -> Result<ResolutionPolicyV1> {
        ResolutionPolicyV1::decode(slice(self.bytes, 16, RESOLUTION_POLICY_BYTES)?)
    }

    /// Return the primary source identity and specification.
    pub fn primary_source(self) -> Result<(ContentId, SourceSpecV1)> {
        Ok((
            content(self.bytes, 400)?,
            SourceSpecV1::decode(slice(self.bytes, 432, SOURCE_SPEC_BYTES)?)?,
        ))
    }

    /// Return the primary provider-release identity and release.
    pub fn primary_provider_release(self) -> Result<(ContentId, ProviderReleaseV1)> {
        Ok((
            content(self.bytes, 1360)?,
            ProviderReleaseV1::decode(slice(self.bytes, 1392, PROVIDER_RELEASE_BYTES)?)?,
        ))
    }

    /// Return the embedded primary Pyth adapter configuration.
    pub fn primary_adapter_config(self) -> Result<PythAdapterConfigV1> {
        PythAdapterConfigV1::decode(slice(self.bytes, 1568, PYTH_ADAPTER_CONFIG_BYTES)?)
    }

    /// Return the source-capacity identity and profile.
    pub fn capacity_profile(self) -> Result<(ContentId, SourceCapacityProfileV1)> {
        Ok((
            content(self.bytes, 256)?,
            SourceCapacityProfileV1::decode(slice(
                self.bytes,
                288,
                SOURCE_CAPACITY_PROFILE_BYTES,
            )?)?,
        ))
    }

    /// Return the window identity and specification.
    pub fn window_spec(self) -> Result<(ContentId, WindowSpecV1)> {
        Ok((
            content(self.bytes, 624)?,
            WindowSpecV1::decode(slice(self.bytes, 656, WINDOW_SPEC_BYTES)?)?,
        ))
    }

    /// Return the immutable window specification.
    pub fn window(self) -> Result<WindowSpecV1> {
        self.window_spec().map(|(_, window)| window)
    }

    /// Return the immutable statistic specification.
    pub fn statistic(self) -> Result<StatisticSpecV1> {
        StatisticSpecV1::decode(slice(self.bytes, 800, STATISTIC_SPEC_BYTES)?)
    }

    /// Return the canonical external Product-instance content identity.
    pub fn product_instance_id(self) -> Result<ContentId> {
        content(self.bytes, 976)
    }

    /// Return the sole Product-owned finite result-domain authority.
    pub fn result_domain(self) -> Result<FiniteResultDomainV1> {
        FiniteResultDomainV1::decode(slice(self.bytes, 1008, FINITE_RESULT_DOMAIN_BYTES)?)
            .map_err(|_| Error::InvalidResultMap)
    }

    /// Return the optional ordered recovery policy by value.
    pub fn recovery_policy(self) -> Result<Option<(ContentId, RecoveryPolicyV1)>> {
        let Some(id) = read_optional_content(self.bytes, 1632)? else {
            return Ok(None);
        };
        Ok(Some((
            id,
            RecoveryPolicyV1::decode(slice(self.bytes, 1664, RECOVERY_POLICY_BYTES)?)?,
        )))
    }

    /// Return one exact active recovery material slot.
    pub fn recovery_slot(self, index: u8) -> Result<RecoveryMaterialSlotV1> {
        let (_, recovery) = self.recovery_policy()?.ok_or(Error::LinkageMismatch)?;
        recovery.attempt(index)?;
        decode_recovery_material_slot_v1(self.bytes, usize::from(index))
    }

    /// Select one embedded source and provider release by exact source ID.
    pub fn source(
        self,
        source_spec_id: ContentId,
    ) -> Result<(SourceSpecV1, ContentId, ProviderReleaseV1)> {
        let (primary_id, primary_source) = self.primary_source()?;
        if source_spec_id == primary_id {
            let (provider_id, provider) = self.primary_provider_release()?;
            return Ok((primary_source, provider_id, provider));
        }
        let active = self
            .recovery_policy()?
            .map_or(0usize, |(_, policy)| usize::from(policy.attempt_count()));
        let mut index = 0usize;
        while index < active {
            let slot = decode_recovery_material_slot_v1(self.bytes, index)?;
            if slot.source_spec_id() == source_spec_id {
                return Ok((
                    slot.source(),
                    slot.provider_release_id(),
                    slot.provider_release(),
                ));
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Err(Error::LinkageMismatch)
    }

    /// Select the embedded Pyth adapter configuration for one exact source.
    pub fn adapter_config(self, source_spec_id: ContentId) -> Result<PythAdapterConfigV1> {
        let (primary_id, _) = self.primary_source()?;
        if source_spec_id == primary_id {
            return self.primary_adapter_config();
        }
        let active = self
            .recovery_policy()?
            .map_or(0usize, |(_, policy)| usize::from(policy.attempt_count()));
        let mut index = 0usize;
        while index < active {
            let slot = decode_recovery_material_slot_v1(self.bytes, index)?;
            if slot.source_spec_id() == source_spec_id {
                return Ok(slot.adapter_config());
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Err(Error::LinkageMismatch)
    }
}

/// Validate one Source-material preimage without constructing its 4,176-byte
/// by-value representation.
#[inline(never)]
pub fn validate_source_material_bytes_v1(bytes: &[u8]) -> Result<()> {
    header(bytes, SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_MAGIC)?;
    zero(bytes, 10, 6)?;
    validate_source_material_core_v1(bytes)?;
    validate_source_material_recovery_v1(bytes)
}

#[inline(never)]
fn validate_source_material_core_v1(bytes: &[u8]) -> Result<()> {
    let policy = ResolutionPolicyV1::decode(slice(bytes, 16, RESOLUTION_POLICY_BYTES)?)?;
    let capacity_profile_id = content(bytes, 256)?;
    let capacity_profile =
        SourceCapacityProfileV1::decode(slice(bytes, 288, SOURCE_CAPACITY_PROFILE_BYTES)?)?;
    let primary_source_id = content(bytes, 400)?;
    let primary_source = SourceSpecV1::decode(slice(bytes, 432, SOURCE_SPEC_BYTES)?)?;
    let window_id = content(bytes, 624)?;
    let window = WindowSpecV1::decode(slice(bytes, 656, WINDOW_SPEC_BYTES)?)?;
    let statistic_id = content(bytes, 768)?;
    let statistic = StatisticSpecV1::decode(slice(bytes, 800, STATISTIC_SPEC_BYTES)?)?;
    let product_instance_id = content(bytes, 976)?;
    let result_domain =
        FiniteResultDomainV1::decode(slice(bytes, 1008, FINITE_RESULT_DOMAIN_BYTES)?)
            .map_err(|_| Error::InvalidResultMap)?;
    policy.validate_embedded_material(
        product_instance_id,
        primary_source_id,
        primary_source,
        window_id,
        window,
        statistic_id,
        statistic,
        result_domain,
    )?;
    statistic.validate_capacity(capacity_profile_id, capacity_profile)?;
    if policy.capacity_profile_id != capacity_profile_id {
        return Err(Error::LinkageMismatch);
    }
    validate_source_material_primary_provider_v1(bytes, primary_source)?;
    if primary_source.access_profile() == SourceAccessProfile::SharedObservationChild
        && usize::from(statistic.required_samples()) > MAX_SHARED_OBSERVATIONS
    {
        return Err(Error::StatisticExceedsCapacity);
    }
    Ok(())
}

#[inline(never)]
fn validate_source_material_primary_provider_v1(
    bytes: &[u8],
    primary_source: SourceSpecV1,
) -> Result<()> {
    let provider_release_id = content(bytes, 1360)?;
    let provider_release = ProviderReleaseV1::decode(slice(bytes, 1392, PROVIDER_RELEASE_BYTES)?)?;
    PythAdapterConfigV1::decode(slice(bytes, 1568, PYTH_ADAPTER_CONFIG_BYTES)?)?;
    if primary_source.provider_release_id() != provider_release_id
        || provider_release.adapter_release_id().to_bytes() != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
    {
        return Err(Error::LinkageMismatch);
    }
    Ok(())
}

#[inline(never)]
fn validate_source_material_recovery_v1(bytes: &[u8]) -> Result<()> {
    let policy = ResolutionPolicyV1::decode(slice(bytes, 16, RESOLUTION_POLICY_BYTES)?)?;
    let capacity_profile_id = content(bytes, 256)?;
    let capacity_profile =
        SourceCapacityProfileV1::decode(slice(bytes, 288, SOURCE_CAPACITY_PROFILE_BYTES)?)?;
    let primary_source = SourceSpecV1::decode(slice(bytes, 432, SOURCE_SPEC_BYTES)?)?;
    let recovery_id = read_optional_content(bytes, 1632)?;
    let recovery = match recovery_id {
        Some(id) => Some((
            id,
            RecoveryPolicyV1::decode(slice(bytes, 1664, RECOVERY_POLICY_BYTES)?)?,
        )),
        None => {
            zero(bytes, 1664, RECOVERY_POLICY_BYTES)?;
            None
        }
    };
    let active = match (policy.recovery_policy_id(), recovery) {
        (None, None) => 0usize,
        (Some(expected), Some((actual, recovery_policy))) => {
            if expected != actual {
                return Err(Error::LinkageMismatch);
            }
            recovery_policy.validate_capacity(
                capacity_profile_id,
                policy.product_instance_id(),
                capacity_profile,
            )?;
            usize::from(recovery_policy.attempt_count())
        }
        _ => return Err(Error::LinkageMismatch),
    };
    let mut index = 0usize;
    while index < MAX_RECOVERY_ATTEMPTS {
        if index < active {
            validate_source_material_recovery_slot_v1(
                bytes,
                index,
                recovery.ok_or(Error::LinkageMismatch)?.1,
                primary_source,
                capacity_profile_id,
            )?;
        } else {
            zero_recovery_material_slot_v1(bytes, index)?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

#[inline(never)]
fn validate_source_material_recovery_slot_v1(
    bytes: &[u8],
    index: usize,
    recovery: RecoveryPolicyV1,
    primary_source: SourceSpecV1,
    capacity_profile_id: ContentId,
) -> Result<()> {
    let slot = decode_recovery_material_slot_v1(bytes, index)?;
    let attempt = recovery.attempt(u8::try_from(index).map_err(|_| Error::ArithmeticOverflow)?)?;
    if slot.source_spec_id() != attempt.source_spec_id()
        || slot.provider_release_id() != attempt.provider_release_id()
    {
        return Err(Error::LinkageMismatch);
    }
    slot.source()
        .validate_dependencies(slot.provider_release_id(), capacity_profile_id)?;
    validate_recovery_source(primary_source, slot.source())
}

fn recovery_material_offsets_v1(index: usize) -> Result<(usize, usize, usize)> {
    if index >= MAX_RECOVERY_ATTEMPTS {
        return Err(Error::ArithmeticOverflow);
    }
    let source = 2192usize
        .checked_add(index.checked_mul(224).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?;
    let provider = 3088usize
        .checked_add(index.checked_mul(208).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?;
    let config = 3920usize
        .checked_add(
            index
                .checked_mul(PYTH_ADAPTER_CONFIG_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)?;
    Ok((source, provider, config))
}

#[inline(never)]
fn decode_recovery_material_slot_v1(bytes: &[u8], index: usize) -> Result<RecoveryMaterialSlotV1> {
    let (source, provider, config) = recovery_material_offsets_v1(index)?;
    RecoveryMaterialSlotV1::new(
        content(bytes, source)?,
        SourceSpecV1::decode(slice(bytes, source + 32, SOURCE_SPEC_BYTES)?)?,
        content(bytes, provider)?,
        ProviderReleaseV1::decode(slice(bytes, provider + 32, PROVIDER_RELEASE_BYTES)?)?,
        PythAdapterConfigV1::decode(slice(bytes, config, PYTH_ADAPTER_CONFIG_BYTES)?)?,
    )
}

#[inline(never)]
fn zero_recovery_material_slot_v1(bytes: &[u8], index: usize) -> Result<()> {
    let (source, provider, config) = recovery_material_offsets_v1(index)?;
    zero(bytes, source, 224)?;
    zero(bytes, provider, 208)?;
    zero(bytes, config, PYTH_ADAPTER_CONFIG_BYTES)
}

/// Return the exact canonical preimage width for a shared evidence set.
pub fn shared_evidence_set_preimage_len_v1(observation_count: u16) -> Result<usize> {
    if observation_count == 0 || usize::from(observation_count) > MAX_SHARED_OBSERVATIONS {
        return Err(Error::InvalidSharedObservation);
    }
    SHARED_EVIDENCE_SET_HEADER_BYTES_V1
        .checked_add(
            usize::from(observation_count)
                .checked_mul(NORMALIZED_EVIDENCE_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Encode the one canonical digest preimage for an accepted shared evidence
/// set. The caller hashes exactly the returned bytes; no caller-selected
/// aggregate identity is semantic authority.
pub fn encode_shared_evidence_set_preimage_v1(
    material_id: ContentId,
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    window_spec_id: ContentId,
    observations: &[NormalizedProviderEvidenceV1],
    output: &mut [u8],
) -> Result<()> {
    let count = u16::try_from(observations.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let expected = shared_evidence_set_preimage_len_v1(count)?;
    if output.len() != expected {
        return Err(Error::InvalidLength);
    }
    output.fill(0);
    put(output, 0, SHARED_EVIDENCE_SET_RELEASE_PREIMAGE_V1);
    put(output, 40, &SCHEMA_VERSION.to_le_bytes());
    put(output, 42, &count.to_le_bytes());
    put(output, 48, material_id.as_bytes());
    put(output, 80, source_spec_id.as_bytes());
    put(output, 112, provider_release_id.as_bytes());
    put(output, 144, window_spec_id.as_bytes());
    let mut index = 0usize;
    while index < observations.len() {
        let observation = *observations.get(index).ok_or(Error::ArithmeticOverflow)?;
        if observation.source_spec_id() != source_spec_id
            || observation.provider_release_id() != provider_release_id
        {
            return Err(Error::StateBindingMismatch);
        }
        let offset = SHARED_EVIDENCE_SET_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(NORMALIZED_EVIDENCE_BYTES)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        put(output, offset, &observation.to_bytes());
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

/// Canonical persisted source-resolution lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceResolutionPhaseV1 {
    /// Primary source may still be accepted.
    Primary = 0,
    /// Exactly one ordered recovery attempt may be accepted.
    Recovery = 1,
    /// A primary or recovery result has been committed.
    Resolved = 2,
    /// Every admitted attempt is exhausted; no result is selected yet.
    Exhausted = 3,
    /// Product-owned failure semantics have been committed.
    FailureCommitted = 4,
    /// Terminal state was retired after settlement.
    Retired = 5,
}

impl SourceResolutionPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Primary),
            1 => Ok(Self::Recovery),
            2 => Ok(Self::Resolved),
            3 => Ok(Self::Exhausted),
            4 => Ok(Self::FailureCommitted),
            5 => Ok(Self::Retired),
            _ => Err(Error::NonCanonicalState),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// PDA seed material for one source-resolution state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    bump: u8,
}

impl SourceResolutionPdaSeedsV1 {
    /// Return the exact, unhashed PDA domain seed.
    pub const fn domain(self) -> &'static [u8] {
        SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1
    }

    /// Return the exact Market-key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return little-endian generation seed bytes.
    pub const fn generation_le(self) -> [u8; 8] {
        self.generation_le
    }

    /// Return the canonical PDA bump byte.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// Persisted, Market-bound source and ordered-recovery execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionStateV1 {
    phase: SourceResolutionPhaseV1,
    active_attempt: u8,
    terminal_route: Option<SourceResolutionRouteV1>,
    result_selector: u8,
    pda_bump: u8,
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    rent_beneficiary: [u8; 32],
    reopen_link_id: Option<ContentId>,
    resolution_evidence_id: Option<ContentId>,
    terminal_sequence: u64,
    resolved_at_unix_seconds: i64,
    retired_at_unix_seconds: i64,
}

/// Joined result of creating one resolution state and registering one direct child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionCreationPlanV1 {
    state: SourceResolutionStateV1,
    market_delta: MarketChildDeltaV1,
}

impl SourceResolutionCreationPlanV1 {
    /// Return the exact new state to persist.
    pub const fn state(self) -> SourceResolutionStateV1 {
        self.state
    }

    /// Return the exactly-one Market registration to apply atomically.
    pub const fn market_delta(self) -> MarketChildDeltaV1 {
        self.market_delta
    }
}

impl SourceResolutionStateV1 {
    /// Begin a fresh primary-source state for one authenticated Market generation.
    pub fn fresh(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        rent_beneficiary: [u8; 32],
        pda_bump: u8,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<SourceResolutionCreationPlanV1> {
        Self::begin(
            market,
            generation,
            material_id,
            rent_beneficiary,
            pda_bump,
            None,
            expected_market_child_count,
            authenticated_market_child_count,
        )
    }

    /// Begin a successor-generation state bound to an authenticated reopen link.
    #[allow(clippy::too_many_arguments)]
    pub fn reopened(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        rent_beneficiary: [u8; 32],
        pda_bump: u8,
        reopen_link_id: ContentId,
        reopen_link: ReopenLinkV1,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<SourceResolutionCreationPlanV1> {
        reopen_link.validate_successor(market, generation)?;
        Self::begin(
            market,
            generation,
            material_id,
            rent_beneficiary,
            pda_bump,
            Some(reopen_link_id),
            expected_market_child_count,
            authenticated_market_child_count,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn begin(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        rent_beneficiary: [u8; 32],
        pda_bump: u8,
        reopen_link_id: Option<ContentId>,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<SourceResolutionCreationPlanV1> {
        nonzero_identifier(&market)?;
        nonzero_identifier(&rent_beneficiary)?;
        let state = Self {
            phase: SourceResolutionPhaseV1::Primary,
            active_attempt: 0,
            terminal_route: None,
            result_selector: 0,
            pda_bump,
            market,
            generation,
            material_id,
            rent_beneficiary,
            reopen_link_id,
            resolution_evidence_id: None,
            terminal_sequence: 0,
            resolved_at_unix_seconds: 0,
            retired_at_unix_seconds: 0,
        };
        Ok(SourceResolutionCreationPlanV1 {
            state,
            market_delta: MarketChildDeltaV1::register(
                expected_market_child_count,
                authenticated_market_child_count,
            )?,
        })
    }

    /// Decode and structurally validate one exact persisted state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            SOURCE_RESOLUTION_STATE_BYTES,
            SOURCE_RESOLUTION_STATE_MAGIC,
        )?;
        zero(bytes, 15, 1)?;
        zero(bytes, 208, 16)?;
        let route_byte = one(bytes, 12)?;
        let terminal_route = if route_byte == 0 {
            None
        } else {
            Some(SourceResolutionRouteV1::decode(route_byte)?)
        };
        let value = Self {
            phase: SourceResolutionPhaseV1::decode(one(bytes, 10)?)?,
            active_attempt: one(bytes, 11)?,
            terminal_route,
            result_selector: one(bytes, 13)?,
            pda_bump: one(bytes, 14)?,
            market: read_array(bytes, 16)?,
            generation: u64::from_le_bytes(read_array(bytes, 48)?),
            material_id: content(bytes, 56)?,
            rent_beneficiary: read_array(bytes, 88)?,
            reopen_link_id: read_optional_content(bytes, 120)?,
            resolution_evidence_id: read_optional_content(bytes, 152)?,
            terminal_sequence: u64::from_le_bytes(read_array(bytes, 184)?),
            resolved_at_unix_seconds: i64::from_le_bytes(read_array(bytes, 192)?),
            retired_at_unix_seconds: i64::from_le_bytes(read_array(bytes, 200)?),
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode exact canonical persisted state bytes.
    pub fn to_bytes(self) -> [u8; SOURCE_RESOLUTION_STATE_BYTES] {
        let mut out = base::<SOURCE_RESOLUTION_STATE_BYTES>(SOURCE_RESOLUTION_STATE_MAGIC);
        put(&mut out, 10, &[self.phase.byte(), self.active_attempt]);
        if let Some(route) = self.terminal_route {
            put(&mut out, 12, &[route.byte()]);
        }
        put(&mut out, 13, &[self.result_selector, self.pda_bump]);
        put(&mut out, 16, &self.market);
        put(&mut out, 48, &self.generation.to_le_bytes());
        put(&mut out, 56, self.material_id.as_bytes());
        put(&mut out, 88, &self.rent_beneficiary);
        if let Some(id) = self.reopen_link_id {
            put(&mut out, 120, id.as_bytes());
        }
        if let Some(id) = self.resolution_evidence_id {
            put(&mut out, 152, id.as_bytes());
        }
        put(&mut out, 184, &self.terminal_sequence.to_le_bytes());
        put(&mut out, 192, &self.resolved_at_unix_seconds.to_le_bytes());
        put(&mut out, 200, &self.retired_at_unix_seconds.to_le_bytes());
        out
    }

    /// Move from the expired primary/current recovery leg to exactly the next attempt.
    ///
    /// `authenticated_funding_allocation_id` is the narrow adapter seam. The
    /// adapter obtains it only after authenticating the actual capability
    /// `FundingStateV1` and its present principal; this contract never stores amounts.
    pub fn fail_next(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV1,
        authenticated_funding_allocation_id: ContentId,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        let (_, recovery) = material.recovery.ok_or(Error::LinkageMismatch)?;
        let next_index = match self.phase {
            SourceResolutionPhaseV1::Primary => {
                require_after(current_unix_seconds, primary_deadline(material.window)?)?;
                0
            }
            SourceResolutionPhaseV1::Recovery => {
                let current = recovery.attempt(self.active_attempt)?;
                require_after(current_unix_seconds, current.deadline_unix_seconds)?;
                let next = self
                    .active_attempt
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next >= recovery.attempt_count {
                    return Err(Error::RecoveryNotExhausted);
                }
                next
            }
            _ => return Err(Error::InvalidRecoveryTransition),
        };
        let attempt = recovery.attempt(next_index)?;
        if attempt.funding_allocation_id != authenticated_funding_allocation_id {
            return Err(Error::LinkageMismatch);
        }
        self.phase = SourceResolutionPhaseV1::Recovery;
        self.active_attempt = next_index;
        Ok(())
    }

    /// Commit explicit exhaustion only after the no-recovery primary or final attempt expires.
    pub fn exhaust(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV1,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        match (self.phase, material.recovery) {
            (SourceResolutionPhaseV1::Primary, None) => {
                require_after(current_unix_seconds, primary_deadline(material.window)?)?;
            }
            (SourceResolutionPhaseV1::Recovery, Some((_, policy))) => {
                if self.active_attempt.checked_add(1) != Some(policy.attempt_count) {
                    return Err(Error::RecoveryNotExhausted);
                }
                let attempt = policy.attempt(self.active_attempt)?;
                require_after(current_unix_seconds, attempt.deadline_unix_seconds)?;
            }
            _ => return Err(Error::RecoveryNotExhausted),
        }
        self.phase = SourceResolutionPhaseV1::Exhausted;
        self.active_attempt = 0;
        Ok(())
    }

    /// Apply accepted provider evidence and commit its mapped primary or recovery result.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_provider_output(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV1,
        resolution_evidence_id: ContentId,
        evidence: &[NormalizedProviderEvidenceV1],
        shared_observation: Option<SharedObservationStateV1>,
        authenticated_funding_allocation_id: Option<ContentId>,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV1> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        let (route, source_spec_id, source, provider_release_id, provider_release) =
            match self.phase {
                SourceResolutionPhaseV1::Primary => {
                    if authenticated_funding_allocation_id.is_some() {
                        return Err(Error::LinkageMismatch);
                    }
                    require_not_after(current_unix_seconds, primary_deadline(material.window)?)?;
                    (
                        SourceResolutionRouteV1::Primary,
                        material.primary_source_id,
                        material.primary_source,
                        material.primary_provider_release_id,
                        material.primary_provider_release,
                    )
                }
                SourceResolutionPhaseV1::Recovery => {
                    let (_, recovery_policy) = material.recovery.ok_or(Error::LinkageMismatch)?;
                    let attempt = recovery_policy.attempt(self.active_attempt)?;
                    let slot = material.recovery_slot(self.active_attempt)?;
                    let Some(funding_id) = authenticated_funding_allocation_id else {
                        return Err(Error::LinkageMismatch);
                    };
                    if attempt.funding_allocation_id != funding_id {
                        return Err(Error::LinkageMismatch);
                    }
                    require_not_after(current_unix_seconds, attempt.deadline_unix_seconds)?;
                    (
                        SourceResolutionRouteV1::Recovery,
                        slot.source_spec_id,
                        slot.source,
                        slot.provider_release_id,
                        slot.provider_release,
                    )
                }
                _ => return Err(Error::InvalidRecoveryTransition),
            };
        match (source.access_profile, shared_observation) {
            (SourceAccessProfile::PythTerminalOneTransaction, None) => {
                if evidence.len() != 1
                    || evidence.first().map(|item| item.provider_evidence_id())
                        != Some(resolution_evidence_id)
                {
                    return Err(Error::StateBindingMismatch);
                }
            }
            (SourceAccessProfile::SharedObservationChild, Some(child)) => {
                child.validate_for_resolution(
                    self.market,
                    self.generation,
                    material_id,
                    source_spec_id,
                    material.window_id,
                    resolution_evidence_id,
                    evidence,
                )?;
            }
            _ => return Err(Error::WrongSourceAccessProfile),
        }
        source.validate_dependencies(provider_release_id, material.capacity_profile_id)?;
        validate_evidence_capacity(material.capacity_profile, evidence)?;
        let statistic_value = evaluate_normalized_evidence(
            material.statistic,
            material.window,
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            evidence,
            current_unix_seconds,
        )?;
        let (numerator, denominator) = statistic_rational(statistic_value);
        let selector = material
            .result_domain
            .map(numerator, denominator)
            .map_err(|_| Error::InvalidResultMap)?;
        let decision = SourceResolutionDecisionV1::new(
            route,
            selector,
            material.result_domain.outcome_count(),
            resolution_evidence_id,
            terminal_sequence,
        )?;
        if current_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        self.phase = SourceResolutionPhaseV1::Resolved;
        self.active_attempt = 0;
        self.terminal_route = Some(route);
        self.result_selector = selector;
        self.resolution_evidence_id = Some(resolution_evidence_id);
        self.terminal_sequence = terminal_sequence;
        self.resolved_at_unix_seconds = current_unix_seconds;
        Ok(decision)
    }

    /// Commit the Product-owned failure result after explicit exhaustion.
    pub fn commit_failure(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV1,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV1> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        if self.phase != SourceResolutionPhaseV1::Exhausted {
            return Err(Error::RecoveryNotExhausted);
        }
        if current_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        let decision = SourceResolutionDecisionV1::new(
            SourceResolutionRouteV1::Failure,
            material.result_domain.failure_selector(),
            material.result_domain.outcome_count(),
            material_id,
            terminal_sequence,
        )?;
        self.phase = SourceResolutionPhaseV1::FailureCommitted;
        self.terminal_route = Some(SourceResolutionRouteV1::Failure);
        self.result_selector = decision.selector;
        self.resolution_evidence_id = Some(material_id);
        self.terminal_sequence = terminal_sequence;
        self.resolved_at_unix_seconds = current_unix_seconds;
        Ok(decision)
    }

    /// Enter the next recovery attempt using bounded borrowed material.
    pub fn fail_next_view(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialViewV1<'_>,
        authenticated_funding_allocation_id: ContentId,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        let (_, recovery) = material.recovery_policy()?.ok_or(Error::LinkageMismatch)?;
        let next_index = match self.phase {
            SourceResolutionPhaseV1::Primary => {
                require_after(current_unix_seconds, primary_deadline(material.window()?)?)?;
                0
            }
            SourceResolutionPhaseV1::Recovery => {
                let current = recovery.attempt(self.active_attempt)?;
                require_after(current_unix_seconds, current.deadline_unix_seconds())?;
                let next = self
                    .active_attempt
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next >= recovery.attempt_count() {
                    return Err(Error::RecoveryNotExhausted);
                }
                next
            }
            _ => return Err(Error::InvalidRecoveryTransition),
        };
        let attempt = recovery.attempt(next_index)?;
        if attempt.funding_allocation_id() != authenticated_funding_allocation_id {
            return Err(Error::LinkageMismatch);
        }
        self.phase = SourceResolutionPhaseV1::Recovery;
        self.active_attempt = next_index;
        Ok(())
    }

    /// Commit exhaustion using bounded borrowed material.
    pub fn exhaust_view(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialViewV1<'_>,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        match (self.phase, material.recovery_policy()?) {
            (SourceResolutionPhaseV1::Primary, None) => {
                require_after(current_unix_seconds, primary_deadline(material.window()?)?)?;
            }
            (SourceResolutionPhaseV1::Recovery, Some((_, policy))) => {
                if self.active_attempt.checked_add(1) != Some(policy.attempt_count()) {
                    return Err(Error::RecoveryNotExhausted);
                }
                let attempt = policy.attempt(self.active_attempt)?;
                require_after(current_unix_seconds, attempt.deadline_unix_seconds())?;
            }
            _ => return Err(Error::RecoveryNotExhausted),
        }
        self.phase = SourceResolutionPhaseV1::Exhausted;
        self.active_attempt = 0;
        Ok(())
    }

    /// Apply provider evidence using bounded borrowed material and child views.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_provider_output_view(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialViewV1<'_>,
        resolution_evidence_id: ContentId,
        evidence: &[NormalizedProviderEvidenceV1],
        shared_observation: Option<SharedObservationStateViewV1<'_>>,
        authenticated_funding_allocation_id: Option<ContentId>,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV1> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        let window = material.window()?;
        let (route, source_spec_id, source, provider_release_id, provider_release) =
            match self.phase {
                SourceResolutionPhaseV1::Primary => {
                    if authenticated_funding_allocation_id.is_some() {
                        return Err(Error::LinkageMismatch);
                    }
                    require_not_after(current_unix_seconds, primary_deadline(window)?)?;
                    let (source_spec_id, source) = material.primary_source()?;
                    let (provider_release_id, provider_release) =
                        material.primary_provider_release()?;
                    (
                        SourceResolutionRouteV1::Primary,
                        source_spec_id,
                        source,
                        provider_release_id,
                        provider_release,
                    )
                }
                SourceResolutionPhaseV1::Recovery => {
                    let (_, recovery_policy) =
                        material.recovery_policy()?.ok_or(Error::LinkageMismatch)?;
                    let attempt = recovery_policy.attempt(self.active_attempt)?;
                    let slot = material.recovery_slot(self.active_attempt)?;
                    let Some(funding_id) = authenticated_funding_allocation_id else {
                        return Err(Error::LinkageMismatch);
                    };
                    if attempt.funding_allocation_id() != funding_id {
                        return Err(Error::LinkageMismatch);
                    }
                    require_not_after(current_unix_seconds, attempt.deadline_unix_seconds())?;
                    (
                        SourceResolutionRouteV1::Recovery,
                        slot.source_spec_id(),
                        slot.source(),
                        slot.provider_release_id(),
                        slot.provider_release(),
                    )
                }
                _ => return Err(Error::InvalidRecoveryTransition),
            };
        match (source.access_profile(), shared_observation) {
            (SourceAccessProfile::PythTerminalOneTransaction, None) => {
                if evidence.len() != 1
                    || evidence.first().map(|item| item.provider_evidence_id())
                        != Some(resolution_evidence_id)
                {
                    return Err(Error::StateBindingMismatch);
                }
            }
            (SourceAccessProfile::SharedObservationChild, Some(child)) => {
                child.validate_for_resolution(
                    self.market,
                    self.generation,
                    material_id,
                    source_spec_id,
                    material.window_spec()?.0,
                    resolution_evidence_id,
                    evidence,
                )?;
            }
            _ => return Err(Error::WrongSourceAccessProfile),
        }
        let (capacity_profile_id, capacity_profile) = material.capacity_profile()?;
        source.validate_dependencies(provider_release_id, capacity_profile_id)?;
        validate_evidence_capacity(capacity_profile, evidence)?;
        let statistic_value = evaluate_normalized_evidence(
            material.statistic()?,
            window,
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            evidence,
            current_unix_seconds,
        )?;
        let (numerator, denominator) = statistic_rational(statistic_value);
        let result_domain = material.result_domain()?;
        let selector = result_domain
            .map(numerator, denominator)
            .map_err(|_| Error::InvalidResultMap)?;
        let decision = SourceResolutionDecisionV1::new(
            route,
            selector,
            result_domain.outcome_count(),
            resolution_evidence_id,
            terminal_sequence,
        )?;
        if current_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        self.phase = SourceResolutionPhaseV1::Resolved;
        self.active_attempt = 0;
        self.terminal_route = Some(route);
        self.result_selector = selector;
        self.resolution_evidence_id = Some(resolution_evidence_id);
        self.terminal_sequence = terminal_sequence;
        self.resolved_at_unix_seconds = current_unix_seconds;
        Ok(decision)
    }

    /// Commit Product-owned failure using bounded borrowed material.
    pub fn commit_failure_view(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialViewV1<'_>,
        expected_generation: u64,
        current_unix_seconds: i64,
        terminal_sequence: u64,
    ) -> Result<SourceResolutionDecisionV1> {
        self.validate_material_binding(material_id)?;
        self.require_generation(expected_generation)?;
        if self.phase != SourceResolutionPhaseV1::Exhausted {
            return Err(Error::RecoveryNotExhausted);
        }
        if current_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        let domain = material.result_domain()?;
        let decision = SourceResolutionDecisionV1::new(
            SourceResolutionRouteV1::Failure,
            domain.failure_selector(),
            domain.outcome_count(),
            material_id,
            terminal_sequence,
        )?;
        self.phase = SourceResolutionPhaseV1::FailureCommitted;
        self.terminal_route = Some(SourceResolutionRouteV1::Failure);
        self.result_selector = decision.selector();
        self.resolution_evidence_id = Some(material_id);
        self.terminal_sequence = terminal_sequence;
        self.resolved_at_unix_seconds = current_unix_seconds;
        Ok(decision)
    }

    /// Retire a terminal state while retaining replay evidence until adapter closure.
    pub fn retire(
        &mut self,
        generation: u64,
        current_unix_seconds: i64,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<MarketChildDeltaV1> {
        if generation != self.generation
            || !matches!(
                self.phase,
                SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
            )
            || current_unix_seconds < self.resolved_at_unix_seconds
        {
            return Err(Error::InvalidRecoveryTransition);
        }
        let delta = MarketChildDeltaV1::retire(
            expected_market_child_count,
            authenticated_market_child_count,
        )?;
        let mut candidate = *self;
        candidate.phase = SourceResolutionPhaseV1::Retired;
        candidate.retired_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(delta)
    }

    /// Reconstruct the terminal provider-neutral decision retained in this state.
    pub fn decision(self, outcome_count: u8) -> Result<SourceResolutionDecisionV1> {
        if !matches!(
            self.phase,
            SourceResolutionPhaseV1::Resolved
                | SourceResolutionPhaseV1::FailureCommitted
                | SourceResolutionPhaseV1::Retired
        ) {
            return Err(Error::InvalidRecoveryTransition);
        }
        SourceResolutionDecisionV1::new(
            self.terminal_route.ok_or(Error::NonCanonicalState)?,
            self.result_selector,
            outcome_count,
            self.resolution_evidence_id
                .ok_or(Error::NonCanonicalState)?,
            self.terminal_sequence,
        )
    }

    /// Return exact PDA derivation seeds.
    pub const fn pda_seeds(self) -> SourceResolutionPdaSeedsV1 {
        SourceResolutionPdaSeedsV1 {
            market: self.market,
            generation_le: self.generation.to_le_bytes(),
            bump: self.pda_bump,
        }
    }

    /// Return the current persisted phase.
    pub const fn phase(self) -> SourceResolutionPhaseV1 {
        self.phase
    }

    /// Return the current recovery attempt only in the recovery phase.
    pub const fn active_recovery_attempt(self) -> Option<u8> {
        if matches!(self.phase, SourceResolutionPhaseV1::Recovery) {
            Some(self.active_attempt)
        } else {
            None
        }
    }

    /// Return the bound Market key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the bound immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the immutable resolution-policy identity.
    pub const fn material_id(self) -> ContentId {
        self.material_id
    }

    /// Return the pre-existing RentCredit beneficiary authority.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the optional authenticated reopen-link identity.
    pub const fn reopen_link_id(self) -> Option<ContentId> {
        self.reopen_link_id
    }

    fn validate_material_binding(self, material_id: ContentId) -> Result<()> {
        if self.material_id != material_id {
            return Err(Error::StateBindingMismatch);
        }
        Ok(())
    }

    fn require_generation(self, expected_generation: u64) -> Result<()> {
        if self.generation == expected_generation {
            Ok(())
        } else {
            Err(Error::StateBindingMismatch)
        }
    }

    fn validate_shape(self) -> Result<()> {
        nonzero_identifier(&self.market)?;
        nonzero_identifier(&self.rent_beneficiary)?;
        match self.phase {
            SourceResolutionPhaseV1::Primary | SourceResolutionPhaseV1::Exhausted => {
                if self.active_attempt != 0
                    || self.terminal_route.is_some()
                    || self.result_selector != 0
                    || self.resolution_evidence_id.is_some()
                    || self.terminal_sequence != 0
                    || self.resolved_at_unix_seconds != 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Recovery => {
                if usize::from(self.active_attempt) >= MAX_RECOVERY_ATTEMPTS
                    || self.terminal_route.is_some()
                    || self.result_selector != 0
                    || self.resolution_evidence_id.is_some()
                    || self.terminal_sequence != 0
                    || self.resolved_at_unix_seconds != 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Resolved => {
                if !matches!(
                    self.terminal_route,
                    Some(SourceResolutionRouteV1::Primary | SourceResolutionRouteV1::Recovery)
                ) || self.resolution_evidence_id.is_none()
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds != 0
                    || self.active_attempt != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::FailureCommitted => {
                if self.terminal_route != Some(SourceResolutionRouteV1::Failure)
                    || self.resolution_evidence_id != Some(self.material_id)
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds != 0
                    || self.active_attempt != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SourceResolutionPhaseV1::Retired => {
                if self.terminal_route.is_none()
                    || self.resolution_evidence_id.is_none()
                    || self.terminal_sequence == 0
                    || self.resolved_at_unix_seconds <= 0
                    || self.retired_at_unix_seconds < self.resolved_at_unix_seconds
                    || self.active_attempt != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
        }
        Ok(())
    }
}

/// Direction of one exact direct-Market-child count transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketChildDeltaKindV1 {
    /// A newly created resolution state or shared observation registers once.
    Register,
    /// A retired and closing state unregisters once.
    Retire,
}

/// Pure replay-guarded plan for exactly one Market child-count delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketChildDeltaV1 {
    kind: MarketChildDeltaKindV1,
    before: u64,
    after: u64,
}

impl MarketChildDeltaV1 {
    /// Plan one registration against the exact authenticated prior count.
    pub fn register(expected_prior_count: u64, authenticated_prior_count: u64) -> Result<Self> {
        if expected_prior_count != authenticated_prior_count {
            return Err(Error::MarketChildCountMismatch);
        }
        Ok(Self {
            kind: MarketChildDeltaKindV1::Register,
            before: expected_prior_count,
            after: expected_prior_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
        })
    }

    /// Plan one retirement against the exact authenticated prior count.
    pub fn retire(expected_prior_count: u64, authenticated_prior_count: u64) -> Result<Self> {
        if expected_prior_count != authenticated_prior_count {
            return Err(Error::MarketChildCountMismatch);
        }
        Ok(Self {
            kind: MarketChildDeltaKindV1::Retire,
            before: expected_prior_count,
            after: expected_prior_count
                .checked_sub(1)
                .ok_or(Error::MarketChildCountMismatch)?,
        })
    }

    /// Return whether this plan registers or retires.
    pub const fn kind(self) -> MarketChildDeltaKindV1 {
        self.kind
    }

    /// Return the exact authenticated count before mutation.
    pub const fn before(self) -> u64 {
        self.before
    }

    /// Return the exact required count after one mutation.
    pub const fn after(self) -> u64 {
        self.after
    }
}

fn primary_deadline(window: WindowSpecV1) -> Result<i64> {
    window
        .end_unix_seconds
        .checked_add(i64::from(window.max_age_seconds))
        .ok_or(Error::ArithmeticOverflow)
}

fn require_after(current: i64, deadline: i64) -> Result<()> {
    if current > deadline {
        Ok(())
    } else {
        Err(Error::DeadlineNotReached)
    }
}

fn require_not_after(current: i64, deadline: i64) -> Result<()> {
    if current <= deadline {
        Ok(())
    } else {
        Err(Error::DeadlineElapsed)
    }
}

fn validate_recovery_source(primary: SourceSpecV1, recovery: SourceSpecV1) -> Result<()> {
    if primary.domain_id != recovery.domain_id
        || primary.unit_id != recovery.unit_id
        || primary.capacity_profile_id != recovery.capacity_profile_id
    {
        return Err(Error::LinkageMismatch);
    }
    Ok(())
}

fn validate_evidence_capacity(
    profile: SourceCapacityProfileV1,
    evidence: &[NormalizedProviderEvidenceV1],
) -> Result<()> {
    if evidence.len() > usize::from(profile.max_samples) {
        return Err(Error::EvidenceExceedsCapacity);
    }
    let bytes = evidence
        .len()
        .checked_mul(NORMALIZED_EVIDENCE_BYTES)
        .ok_or(Error::ArithmeticOverflow)?;
    let bytes = u32::try_from(bytes).map_err(|_| Error::EvidenceExceedsCapacity)?;
    if bytes > profile.max_observation_bytes {
        return Err(Error::EvidenceExceedsCapacity);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn evaluate_normalized_evidence(
    statistic: StatisticSpecV1,
    window: WindowSpecV1,
    source_spec_id: ContentId,
    source: SourceSpecV1,
    provider_release_id: ContentId,
    provider_release: ProviderReleaseV1,
    evidence: &[NormalizedProviderEvidenceV1],
    current_unix_seconds: i64,
) -> Result<StatisticValue> {
    statistic.validate_shape()?;
    if evidence.len() != usize::from(statistic.required_samples) {
        return Err(Error::InvalidObservationSchedule);
    }
    let mut sum = 0i128;
    let mut min = 0i128;
    let mut max = 0i128;
    let mut previous_timestamp: Option<i64> = None;
    for (index, item) in evidence.iter().enumerate() {
        let schedule_index = u16::try_from(index).map_err(|_| Error::StatisticExceedsCapacity)?;
        let observation = item.validate(
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            window,
            schedule_index,
            current_unix_seconds,
        )?;
        if let Some(previous) = previous_timestamp
            && observation.unix_seconds <= previous
        {
            return Err(Error::InvalidObservationSchedule);
        }
        previous_timestamp = Some(observation.unix_seconds);
        if index == 0 {
            min = observation.atoms;
            max = observation.atoms;
        } else {
            min = core::cmp::min(min, observation.atoms);
            max = core::cmp::max(max, observation.atoms);
        }
        sum = sum
            .checked_add(observation.atoms)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    let raw = match statistic.kind {
        StatisticKind::TerminalSample => {
            evidence
                .first()
                .ok_or(Error::InvalidObservationSchedule)?
                .atoms
        }
        StatisticKind::ExactScheduledAverage => {
            return finalize(sum, statistic.required_samples, statistic.rounding);
        }
        StatisticKind::Minimum => min,
        StatisticKind::Maximum => max,
        StatisticKind::AtLeastThreshold => i128::from(u8::from(min >= statistic.threshold_atoms)),
        StatisticKind::AtMostThreshold => i128::from(u8::from(max <= statistic.threshold_atoms)),
        StatisticKind::OddScheduledMedian => {
            validate_normalized_median_schedule(window, evidence)?;
            exact_normalized_median(evidence)?
        }
    };
    finalize(raw, 1, statistic.rounding)
}

fn validate_normalized_median_schedule(
    window: WindowSpecV1,
    evidence: &[NormalizedProviderEvidenceV1],
) -> Result<()> {
    if window.kind != WindowKind::ScheduledInterval || evidence.len() < 3 {
        return Err(Error::NonCanonicalStatistic);
    }
    let first = evidence
        .first()
        .ok_or(Error::InvalidObservationSchedule)?
        .observation_unix_seconds;
    let last = evidence
        .last()
        .ok_or(Error::InvalidObservationSchedule)?
        .observation_unix_seconds;
    if first != window.start_unix_seconds || last != window.end_unix_seconds {
        return Err(Error::InvalidObservationSchedule);
    }
    let intervals = i64::try_from(evidence.len().saturating_sub(1))
        .map_err(|_| Error::InvalidObservationSchedule)?;
    let span = window
        .end_unix_seconds
        .checked_sub(window.start_unix_seconds)
        .ok_or(Error::ArithmeticOverflow)?;
    if intervals == 0 || span.rem_euclid(intervals) != 0 {
        return Err(Error::InvalidObservationSchedule);
    }
    let cadence = span.div_euclid(intervals);
    if cadence <= 0 {
        return Err(Error::InvalidObservationSchedule);
    }
    for (index, item) in evidence.iter().enumerate() {
        let position = i64::try_from(index).map_err(|_| Error::InvalidObservationSchedule)?;
        let expected = cadence
            .checked_mul(position)
            .and_then(|offset| window.start_unix_seconds.checked_add(offset))
            .ok_or(Error::ArithmeticOverflow)?;
        if item.observation_unix_seconds != expected {
            return Err(Error::InvalidObservationSchedule);
        }
    }
    Ok(())
}

fn exact_normalized_median(evidence: &[NormalizedProviderEvidenceV1]) -> Result<i128> {
    let rank = evidence.len() / 2;
    for candidate in evidence {
        let mut below = 0usize;
        let mut equal = 0usize;
        for item in evidence {
            if item.atoms < candidate.atoms {
                below = below.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            } else if item.atoms == candidate.atoms {
                equal = equal.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
        }
        if below <= rank && rank < below.checked_add(equal).ok_or(Error::ArithmeticOverflow)? {
            return Ok(candidate.atoms);
        }
    }
    Err(Error::InvalidObservationSchedule)
}

/// Persisted phase of an explicitly selected shared-observation child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SharedObservationPhaseV1 {
    /// Created and awaiting the first provider-authenticated observation.
    Open = 0,
    /// A strict prefix of the committed schedule has been authenticated.
    Collecting = 1,
    /// The exact complete observation set is accepted and replay-stable.
    Accepted = 2,
    /// The child was retired and may be closed into its beneficiary RentCredit.
    Retired = 3,
}

impl SharedObservationPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Collecting),
            2 => Ok(Self::Accepted),
            3 => Ok(Self::Retired),
            _ => Err(Error::NonCanonicalState),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Exact PDA seeds for one shared-observation child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedObservationPdaSeedsV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    source_spec_id: ContentId,
    window_spec_id: ContentId,
    bump: u8,
}

impl SharedObservationPdaSeedsV1 {
    /// Return the exact, unhashed PDA domain.
    pub const fn domain(self) -> &'static [u8] {
        SHARED_OBSERVATION_PDA_DOMAIN_V1
    }

    /// Return the Market key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return little-endian generation seed bytes.
    pub const fn generation_le(self) -> [u8; 8] {
        self.generation_le
    }

    /// Return the source-specification identity seed.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the window-specification identity seed.
    pub const fn window_spec_id(self) -> ContentId {
        self.window_spec_id
    }

    /// Return the PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// Exact reusable observation child admitted only by `SharedObservationChild`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedObservationStateV1 {
    phase: SharedObservationPhaseV1,
    pda_bump: u8,
    observation_count: u16,
    expected_observation_count: u16,
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    window_spec_id: ContentId,
    rent_beneficiary: [u8; 32],
    evidence_id: Option<ContentId>,
    accepted_sequence: u64,
    created_at_unix_seconds: i64,
    retired_at_unix_seconds: i64,
    observations: [Option<NormalizedProviderEvidenceV1>; MAX_SHARED_OBSERVATIONS],
}

/// Joined result of creating one shared child and registering it on the Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedObservationCreationPlanV1 {
    state: SharedObservationStateV1,
    market_delta: MarketChildDeltaV1,
}

impl SharedObservationCreationPlanV1 {
    /// Return the exact new shared child to persist.
    pub const fn state(self) -> SharedObservationStateV1 {
        self.state
    }

    /// Return the exactly-one Market registration to apply atomically.
    pub const fn market_delta(self) -> MarketChildDeltaV1 {
        self.market_delta
    }
}

impl SharedObservationStateV1 {
    /// Create a child only for the explicitly selected shared access profile.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        material: SourceMaterialV1,
        source_spec_id: ContentId,
        observed_shared_children: u32,
        window_spec_id: ContentId,
        rent_beneficiary: [u8; 32],
        pda_bump: u8,
        current_unix_seconds: i64,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<SharedObservationCreationPlanV1> {
        nonzero_identifier(&market)?;
        nonzero_identifier(&rent_beneficiary)?;
        let (source, provider_release_id, _) = material.source(source_spec_id)?;
        if source.access_profile != SourceAccessProfile::SharedObservationChild {
            return Err(Error::WrongSourceAccessProfile);
        }
        if source.capacity_profile_id != material.capacity_profile_id
            || window_spec_id != material.window_id
        {
            return Err(Error::LinkageMismatch);
        }
        if observed_shared_children >= material.capacity_profile.max_shared_children {
            return Err(Error::SharedChildrenExceedCapacity);
        }
        material.window.validate_source(source_spec_id)?;
        let expected_observation_count = material.statistic.required_samples;
        if expected_observation_count == 0
            || usize::from(expected_observation_count) > MAX_SHARED_OBSERVATIONS
        {
            return Err(Error::StatisticExceedsCapacity);
        }
        if current_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        let state = Self {
            phase: SharedObservationPhaseV1::Open,
            pda_bump,
            observation_count: 0,
            expected_observation_count,
            market,
            generation,
            material_id,
            source_spec_id,
            provider_release_id,
            window_spec_id,
            rent_beneficiary,
            evidence_id: None,
            accepted_sequence: 0,
            created_at_unix_seconds: current_unix_seconds,
            retired_at_unix_seconds: 0,
            observations: [None; MAX_SHARED_OBSERVATIONS],
        };
        Ok(SharedObservationCreationPlanV1 {
            state,
            market_delta: MarketChildDeltaV1::register(
                expected_market_child_count,
                authenticated_market_child_count,
            )?,
        })
    }

    /// Decode one exact hostile child state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            SHARED_OBSERVATION_STATE_BYTES,
            SHARED_OBSERVATION_STATE_MAGIC,
        )?;
        zero(bytes, 272, 16)?;
        let observation_count = u16::from_le_bytes(read_array(bytes, 12)?);
        let expected_observation_count = u16::from_le_bytes(read_array(bytes, 14)?);
        let mut observations = [None; MAX_SHARED_OBSERVATIONS];
        let mut index = 0usize;
        while index < MAX_SHARED_OBSERVATIONS {
            let offset = 288usize
                .checked_add(
                    index
                        .checked_mul(NORMALIZED_EVIDENCE_BYTES)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if index < usize::from(observation_count) {
                let observation = observations
                    .get_mut(index)
                    .ok_or(Error::ArithmeticOverflow)?;
                *observation = Some(NormalizedProviderEvidenceV1::decode(slice(
                    bytes,
                    offset,
                    NORMALIZED_EVIDENCE_BYTES,
                )?)?);
            } else {
                zero(bytes, offset, NORMALIZED_EVIDENCE_BYTES)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let value = Self {
            phase: SharedObservationPhaseV1::decode(one(bytes, 10)?)?,
            pda_bump: one(bytes, 11)?,
            observation_count,
            expected_observation_count,
            market: read_array(bytes, 16)?,
            generation: u64::from_le_bytes(read_array(bytes, 48)?),
            material_id: content(bytes, 56)?,
            source_spec_id: content(bytes, 88)?,
            provider_release_id: content(bytes, 120)?,
            window_spec_id: content(bytes, 152)?,
            rent_beneficiary: read_array(bytes, 184)?,
            evidence_id: read_optional_content(bytes, 216)?,
            accepted_sequence: u64::from_le_bytes(read_array(bytes, 248)?),
            created_at_unix_seconds: i64::from_le_bytes(read_array(bytes, 256)?),
            retired_at_unix_seconds: i64::from_le_bytes(read_array(bytes, 264)?),
            observations,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode exact canonical shared-child bytes.
    pub fn to_bytes(self) -> [u8; SHARED_OBSERVATION_STATE_BYTES] {
        let mut out = base::<SHARED_OBSERVATION_STATE_BYTES>(SHARED_OBSERVATION_STATE_MAGIC);
        put(&mut out, 10, &[self.phase.byte(), self.pda_bump]);
        put(&mut out, 12, &self.observation_count.to_le_bytes());
        put(&mut out, 14, &self.expected_observation_count.to_le_bytes());
        put(&mut out, 16, &self.market);
        put(&mut out, 48, &self.generation.to_le_bytes());
        put(&mut out, 56, self.material_id.as_bytes());
        put(&mut out, 88, self.source_spec_id.as_bytes());
        put(&mut out, 120, self.provider_release_id.as_bytes());
        put(&mut out, 152, self.window_spec_id.as_bytes());
        put(&mut out, 184, &self.rent_beneficiary);
        if let Some(id) = self.evidence_id {
            put(&mut out, 216, id.as_bytes());
        }
        put(&mut out, 248, &self.accepted_sequence.to_le_bytes());
        put(&mut out, 256, &self.created_at_unix_seconds.to_le_bytes());
        put(&mut out, 264, &self.retired_at_unix_seconds.to_le_bytes());
        let mut index = 0usize;
        while index < usize::from(self.observation_count) {
            if let Some(observation) = self.observations.get(index).copied().flatten() {
                put(
                    &mut out,
                    288 + index * NORMALIZED_EVIDENCE_BYTES,
                    &observation.to_bytes(),
                );
            }
            index += 1;
        }
        out
    }

    /// Append exactly the next normalized output of the selected provider adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_provider_output(
        &mut self,
        material_id: ContentId,
        material: SourceMaterialV1,
        completed_evidence_id: Option<ContentId>,
        evidence: NormalizedProviderEvidenceV1,
        accepted_sequence: u64,
        expected_generation: u64,
        current_unix_seconds: i64,
    ) -> Result<()> {
        let mut candidate = *self;
        if !matches!(
            candidate.phase,
            SharedObservationPhaseV1::Open | SharedObservationPhaseV1::Collecting
        ) {
            return Err(Error::InvalidSharedObservation);
        }
        let (source, provider_release_id, provider_release) =
            material.source(candidate.source_spec_id)?;
        if candidate.material_id != material_id
            || candidate.provider_release_id != provider_release_id
            || candidate.window_spec_id != material.window_id
            || candidate.generation != expected_generation
            || source.access_profile != SourceAccessProfile::SharedObservationChild
            || accepted_sequence == 0
            || accepted_sequence <= candidate.accepted_sequence
        {
            return Err(Error::StateBindingMismatch);
        }
        source.validate_dependencies(provider_release_id, material.capacity_profile_id)?;
        evidence.validate(
            candidate.source_spec_id,
            source,
            provider_release_id,
            provider_release,
            material.window,
            candidate.observation_count,
            current_unix_seconds,
        )?;
        let index = usize::from(candidate.observation_count);
        let observation = candidate
            .observations
            .get_mut(index)
            .ok_or(Error::StatisticExceedsCapacity)?;
        if observation.is_some() {
            return Err(Error::StatisticExceedsCapacity);
        }
        *observation = Some(evidence);
        candidate.observation_count = candidate
            .observation_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        candidate.accepted_sequence = accepted_sequence;
        if candidate.observation_count == candidate.expected_observation_count {
            candidate.evidence_id = Some(completed_evidence_id.ok_or(Error::StateBindingMismatch)?);
            candidate.phase = SharedObservationPhaseV1::Accepted;
        } else {
            if completed_evidence_id.is_some() {
                return Err(Error::StateBindingMismatch);
            }
            candidate.phase = SharedObservationPhaseV1::Collecting;
        }
        candidate.validate_shape()?;
        *self = candidate;
        Ok(())
    }

    /// Validate one accepted child before reusing its immutable evidence set.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_for_resolution(
        self,
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        source_spec_id: ContentId,
        window_spec_id: ContentId,
        evidence_id: ContentId,
        evidence: &[NormalizedProviderEvidenceV1],
    ) -> Result<()> {
        if self.phase != SharedObservationPhaseV1::Accepted
            || self.market != market
            || self.generation != generation
            || self.material_id != material_id
            || self.source_spec_id != source_spec_id
            || self.window_spec_id != window_spec_id
            || self.evidence_id != Some(evidence_id)
            || evidence.len() != usize::from(self.observation_count)
        {
            return Err(Error::InvalidSharedObservation);
        }
        for (index, supplied) in evidence.iter().enumerate() {
            if self.observations.get(index).copied().flatten() != Some(*supplied) {
                return Err(Error::InvalidSharedObservation);
            }
        }
        Ok(())
    }

    /// Retire this selected child; no universal archive is created.
    pub fn retire(
        &mut self,
        generation: u64,
        current_unix_seconds: i64,
        expected_market_child_count: u64,
        authenticated_market_child_count: u64,
    ) -> Result<MarketChildDeltaV1> {
        if self.generation != generation
            || self.phase == SharedObservationPhaseV1::Retired
            || current_unix_seconds < self.created_at_unix_seconds
        {
            return Err(Error::InvalidSharedObservation);
        }
        let delta = MarketChildDeltaV1::retire(
            expected_market_child_count,
            authenticated_market_child_count,
        )?;
        let mut candidate = *self;
        candidate.phase = SharedObservationPhaseV1::Retired;
        candidate.retired_at_unix_seconds = current_unix_seconds;
        candidate.validate_shape()?;
        *self = candidate;
        Ok(delta)
    }

    /// Return exact PDA seed material.
    pub const fn pda_seeds(self) -> SharedObservationPdaSeedsV1 {
        SharedObservationPdaSeedsV1 {
            market: self.market,
            generation_le: self.generation.to_le_bytes(),
            source_spec_id: self.source_spec_id,
            window_spec_id: self.window_spec_id,
            bump: self.pda_bump,
        }
    }

    /// Return the persisted child phase.
    pub const fn phase(self) -> SharedObservationPhaseV1 {
        self.phase
    }

    /// Return the immutable rent beneficiary authority.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the immutable Source-material identity selected at creation.
    pub const fn material_id(self) -> ContentId {
        self.material_id
    }

    /// Return the accepted evidence identity, when present.
    pub const fn evidence_id(self) -> Option<ContentId> {
        self.evidence_id
    }

    /// Return the exact number of provider-authenticated observations retained.
    pub const fn observation_count(self) -> u16 {
        self.observation_count
    }

    /// Return one exact retained observation by schedule index.
    pub fn observation(self, index: u16) -> Result<NormalizedProviderEvidenceV1> {
        if index >= self.observation_count {
            return Err(Error::InvalidObservationSchedule);
        }
        self.observations
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(Error::NonCanonicalState)
    }

    fn validate_shape(self) -> Result<()> {
        nonzero_identifier(&self.market)?;
        nonzero_identifier(&self.rent_beneficiary)?;
        if self.created_at_unix_seconds <= 0 {
            return Err(Error::NonCanonicalState);
        }
        if self.expected_observation_count == 0
            || usize::from(self.expected_observation_count) > MAX_SHARED_OBSERVATIONS
            || self.observation_count > self.expected_observation_count
        {
            return Err(Error::NonCanonicalState);
        }
        match self.phase {
            SharedObservationPhaseV1::Open => {
                if self.observation_count != 0
                    || self.evidence_id.is_some()
                    || self.accepted_sequence != 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SharedObservationPhaseV1::Collecting => {
                if self.observation_count == 0
                    || self.observation_count >= self.expected_observation_count
                    || self.evidence_id.is_some()
                    || self.accepted_sequence == 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SharedObservationPhaseV1::Accepted => {
                if self.observation_count != self.expected_observation_count
                    || self.evidence_id.is_none()
                    || self.accepted_sequence == 0
                    || self.retired_at_unix_seconds != 0
                {
                    return Err(Error::NonCanonicalState);
                }
            }
            SharedObservationPhaseV1::Retired => {
                let accepted_fields_match = (self.observation_count
                    == self.expected_observation_count
                    && self.evidence_id.is_some()
                    && self.accepted_sequence != 0)
                    || (self.observation_count < self.expected_observation_count
                        && self.evidence_id.is_none()
                        && ((self.observation_count == 0 && self.accepted_sequence == 0)
                            || (self.observation_count > 0 && self.accepted_sequence > 0)));
                if !accepted_fields_match
                    || self.retired_at_unix_seconds < self.created_at_unix_seconds
                {
                    return Err(Error::NonCanonicalState);
                }
            }
        }
        let mut index = 0usize;
        while index < MAX_SHARED_OBSERVATIONS {
            let is_present = self.observations.get(index).copied().flatten().is_some();
            if (index < usize::from(self.observation_count)) != is_present {
                return Err(Error::NonCanonicalState);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

/// Borrowed, fully validated view of one canonical shared-observation child.
///
/// The view retains the 3,616-byte state in caller-owned account memory and
/// decodes only the bounded field or observation requested by an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedObservationStateViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> SharedObservationStateViewV1<'a> {
    /// Validate and borrow one exact hostile shared-observation state.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        validate_shared_observation_state_bytes_v1(bytes)?;
        Ok(Self { bytes })
    }

    /// Return the exact canonical bytes retained by this view.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Return exact PDA seed material.
    pub fn pda_seeds(self) -> Result<SharedObservationPdaSeedsV1> {
        Ok(SharedObservationPdaSeedsV1 {
            market: read_array(self.bytes, 16)?,
            generation_le: read_array(self.bytes, 48)?,
            source_spec_id: content(self.bytes, 88)?,
            window_spec_id: content(self.bytes, 152)?,
            bump: one(self.bytes, 11)?,
        })
    }

    /// Return the persisted child phase.
    pub fn phase(self) -> Result<SharedObservationPhaseV1> {
        SharedObservationPhaseV1::decode(one(self.bytes, 10)?)
    }

    /// Return the immutable rent-beneficiary authority.
    pub fn rent_beneficiary(self) -> Result<[u8; 32]> {
        read_array(self.bytes, 184)
    }

    /// Return the selected Source-material identity.
    pub fn material_id(self) -> Result<ContentId> {
        content(self.bytes, 56)
    }

    /// Return the selected Source specification identity.
    pub fn source_spec_id(self) -> Result<ContentId> {
        content(self.bytes, 88)
    }

    /// Return the selected provider-release identity.
    pub fn provider_release_id(self) -> Result<ContentId> {
        content(self.bytes, 120)
    }

    /// Return the selected window identity.
    pub fn window_spec_id(self) -> Result<ContentId> {
        content(self.bytes, 152)
    }

    /// Return the accepted evidence identity, when present.
    pub fn evidence_id(self) -> Result<Option<ContentId>> {
        read_optional_content(self.bytes, 216)
    }

    /// Return the exact number of retained observations.
    pub fn observation_count(self) -> Result<u16> {
        Ok(u16::from_le_bytes(read_array(self.bytes, 12)?))
    }

    /// Return the exact expected observation count.
    pub fn expected_observation_count(self) -> Result<u16> {
        Ok(u16::from_le_bytes(read_array(self.bytes, 14)?))
    }

    /// Return the most recent accepted replay sequence.
    pub fn accepted_sequence(self) -> Result<u64> {
        Ok(u64::from_le_bytes(read_array(self.bytes, 248)?))
    }

    /// Return the generation selected at creation.
    pub fn generation(self) -> Result<u64> {
        Ok(u64::from_le_bytes(read_array(self.bytes, 48)?))
    }

    /// Return the creation timestamp.
    pub fn created_at_unix_seconds(self) -> Result<i64> {
        Ok(i64::from_le_bytes(read_array(self.bytes, 256)?))
    }

    /// Return one exact retained observation by schedule index.
    pub fn observation(self, index: u16) -> Result<NormalizedProviderEvidenceV1> {
        if index >= self.observation_count()? {
            return Err(Error::InvalidObservationSchedule);
        }
        decode_shared_observation_slot_v1(self.bytes, usize::from(index))
    }

    /// Validate one accepted child before reusing its immutable evidence set.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_for_resolution(
        self,
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        source_spec_id: ContentId,
        window_spec_id: ContentId,
        evidence_id: ContentId,
        evidence: &[NormalizedProviderEvidenceV1],
    ) -> Result<()> {
        if self.phase()? != SharedObservationPhaseV1::Accepted
            || read_array::<32>(self.bytes, 16)? != market
            || self.generation()? != generation
            || self.material_id()? != material_id
            || self.source_spec_id()? != source_spec_id
            || self.window_spec_id()? != window_spec_id
            || self.evidence_id()? != Some(evidence_id)
            || evidence.len() != usize::from(self.observation_count()?)
        {
            return Err(Error::InvalidSharedObservation);
        }
        for (index, supplied) in evidence.iter().enumerate() {
            if decode_shared_observation_slot_v1(self.bytes, index)? != *supplied {
                return Err(Error::InvalidSharedObservation);
            }
        }
        Ok(())
    }
}

/// Validate one shared-observation state without constructing its 3,616-byte
/// by-value representation.
#[inline(never)]
pub fn validate_shared_observation_state_bytes_v1(bytes: &[u8]) -> Result<()> {
    header(
        bytes,
        SHARED_OBSERVATION_STATE_BYTES,
        SHARED_OBSERVATION_STATE_MAGIC,
    )?;
    zero(bytes, 272, 16)?;
    validate_shared_observation_header_v1(bytes)?;
    validate_shared_observation_slots_v1(bytes)
}

#[inline(never)]
fn validate_shared_observation_header_v1(bytes: &[u8]) -> Result<()> {
    let phase = SharedObservationPhaseV1::decode(one(bytes, 10)?)?;
    let observation_count = u16::from_le_bytes(read_array(bytes, 12)?);
    let expected_observation_count = u16::from_le_bytes(read_array(bytes, 14)?);
    let market = read_array(bytes, 16)?;
    let rent_beneficiary = read_array(bytes, 184)?;
    nonzero_identifier(&market)?;
    nonzero_identifier(&rent_beneficiary)?;
    content(bytes, 56)?;
    content(bytes, 88)?;
    content(bytes, 120)?;
    content(bytes, 152)?;
    let evidence_id = read_optional_content(bytes, 216)?;
    let accepted_sequence = u64::from_le_bytes(read_array(bytes, 248)?);
    let created_at_unix_seconds = i64::from_le_bytes(read_array(bytes, 256)?);
    let retired_at_unix_seconds = i64::from_le_bytes(read_array(bytes, 264)?);
    if created_at_unix_seconds <= 0
        || expected_observation_count == 0
        || usize::from(expected_observation_count) > MAX_SHARED_OBSERVATIONS
        || observation_count > expected_observation_count
    {
        return Err(Error::NonCanonicalState);
    }
    match phase {
        SharedObservationPhaseV1::Open => {
            if observation_count != 0
                || evidence_id.is_some()
                || accepted_sequence != 0
                || retired_at_unix_seconds != 0
            {
                return Err(Error::NonCanonicalState);
            }
        }
        SharedObservationPhaseV1::Collecting => {
            if observation_count == 0
                || observation_count >= expected_observation_count
                || evidence_id.is_some()
                || accepted_sequence == 0
                || retired_at_unix_seconds != 0
            {
                return Err(Error::NonCanonicalState);
            }
        }
        SharedObservationPhaseV1::Accepted => {
            if observation_count != expected_observation_count
                || evidence_id.is_none()
                || accepted_sequence == 0
                || retired_at_unix_seconds != 0
            {
                return Err(Error::NonCanonicalState);
            }
        }
        SharedObservationPhaseV1::Retired => {
            let terminal_shape = (observation_count == expected_observation_count
                && evidence_id.is_some()
                && accepted_sequence != 0)
                || (observation_count < expected_observation_count
                    && evidence_id.is_none()
                    && ((observation_count == 0 && accepted_sequence == 0)
                        || (observation_count > 0 && accepted_sequence > 0)));
            if !terminal_shape || retired_at_unix_seconds < created_at_unix_seconds {
                return Err(Error::NonCanonicalState);
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn validate_shared_observation_slots_v1(bytes: &[u8]) -> Result<()> {
    let observation_count = usize::from(u16::from_le_bytes(read_array(bytes, 12)?));
    let mut index = 0usize;
    while index < MAX_SHARED_OBSERVATIONS {
        if index < observation_count {
            decode_shared_observation_slot_v1(bytes, index)?;
        } else {
            let offset = shared_observation_slot_offset_v1(index)?;
            zero(bytes, offset, NORMALIZED_EVIDENCE_BYTES)?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn shared_observation_slot_offset_v1(index: usize) -> Result<usize> {
    if index >= MAX_SHARED_OBSERVATIONS {
        return Err(Error::StatisticExceedsCapacity);
    }
    288usize
        .checked_add(
            index
                .checked_mul(NORMALIZED_EVIDENCE_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

#[inline(never)]
fn decode_shared_observation_slot_v1(
    bytes: &[u8],
    index: usize,
) -> Result<NormalizedProviderEvidenceV1> {
    NormalizedProviderEvidenceV1::decode(slice(
        bytes,
        shared_observation_slot_offset_v1(index)?,
        NORMALIZED_EVIDENCE_BYTES,
    )?)
}

/// Create one shared-observation child directly into exact account bytes.
///
/// Every fallible authorization and child-count check completes before the
/// caller-owned output is changed.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn create_shared_observation_state_into_v1(
    output: &mut [u8],
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    material: SourceMaterialViewV1<'_>,
    source_spec_id: ContentId,
    observed_shared_children: u32,
    window_spec_id: ContentId,
    rent_beneficiary: [u8; 32],
    pda_bump: u8,
    current_unix_seconds: i64,
    expected_market_child_count: u64,
    authenticated_market_child_count: u64,
) -> Result<MarketChildDeltaV1> {
    if output.len() != SHARED_OBSERVATION_STATE_BYTES {
        return Err(Error::InvalidLength);
    }
    nonzero_identifier(&market)?;
    nonzero_identifier(&rent_beneficiary)?;
    let (source, provider_release_id, _) = material.source(source_spec_id)?;
    let (capacity_profile_id, capacity_profile) = material.capacity_profile()?;
    let (material_window_id, window) = material.window_spec()?;
    if source.access_profile() != SourceAccessProfile::SharedObservationChild {
        return Err(Error::WrongSourceAccessProfile);
    }
    if source.capacity_profile_id() != capacity_profile_id || window_spec_id != material_window_id {
        return Err(Error::LinkageMismatch);
    }
    if observed_shared_children >= capacity_profile.max_shared_children() {
        return Err(Error::SharedChildrenExceedCapacity);
    }
    window.validate_source(source_spec_id)?;
    let expected_observation_count = material.statistic()?.required_samples();
    if expected_observation_count == 0
        || usize::from(expected_observation_count) > MAX_SHARED_OBSERVATIONS
    {
        return Err(Error::StatisticExceedsCapacity);
    }
    if current_unix_seconds <= 0 {
        return Err(Error::NonCanonicalState);
    }
    let delta = MarketChildDeltaV1::register(
        expected_market_child_count,
        authenticated_market_child_count,
    )?;
    output.fill(0);
    put(output, 0, &SHARED_OBSERVATION_STATE_MAGIC);
    put(output, 8, &SCHEMA_VERSION.to_le_bytes());
    put(
        output,
        10,
        &[SharedObservationPhaseV1::Open.byte(), pda_bump],
    );
    put(output, 14, &expected_observation_count.to_le_bytes());
    put(output, 16, &market);
    put(output, 48, &generation.to_le_bytes());
    put(output, 56, material_id.as_bytes());
    put(output, 88, source_spec_id.as_bytes());
    put(output, 120, provider_release_id.as_bytes());
    put(output, 152, window_spec_id.as_bytes());
    put(output, 184, &rent_beneficiary);
    put(output, 256, &current_unix_seconds.to_le_bytes());
    Ok(delta)
}

/// Append one authenticated provider output directly to shared-child bytes.
///
/// Refusal is atomic: all parsing, linkage, replay, schedule, and completion
/// checks finish before the first byte changes.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn accept_shared_provider_output_in_place_v1(
    bytes: &mut [u8],
    material_id: ContentId,
    material: SourceMaterialViewV1<'_>,
    completed_evidence_id: Option<ContentId>,
    evidence: NormalizedProviderEvidenceV1,
    accepted_sequence: u64,
    expected_generation: u64,
    current_unix_seconds: i64,
) -> Result<()> {
    let view = SharedObservationStateViewV1::decode(bytes)?;
    if !matches!(
        view.phase()?,
        SharedObservationPhaseV1::Open | SharedObservationPhaseV1::Collecting
    ) {
        return Err(Error::InvalidSharedObservation);
    }
    let source_spec_id = view.source_spec_id()?;
    let (source, provider_release_id, provider_release) = material.source(source_spec_id)?;
    let (capacity_profile_id, _) = material.capacity_profile()?;
    let (window_spec_id, window) = material.window_spec()?;
    let observation_count = view.observation_count()?;
    let expected_observation_count = view.expected_observation_count()?;
    if view.material_id()? != material_id
        || view.provider_release_id()? != provider_release_id
        || view.window_spec_id()? != window_spec_id
        || view.generation()? != expected_generation
        || source.access_profile() != SourceAccessProfile::SharedObservationChild
        || accepted_sequence == 0
        || accepted_sequence <= view.accepted_sequence()?
    {
        return Err(Error::StateBindingMismatch);
    }
    source.validate_dependencies(provider_release_id, capacity_profile_id)?;
    evidence.validate(
        source_spec_id,
        source,
        provider_release_id,
        provider_release,
        window,
        observation_count,
        current_unix_seconds,
    )?;
    let next_count = observation_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_phase = if next_count == expected_observation_count {
        if completed_evidence_id.is_none() {
            return Err(Error::StateBindingMismatch);
        }
        SharedObservationPhaseV1::Accepted
    } else {
        if next_count > expected_observation_count || completed_evidence_id.is_some() {
            return Err(Error::StateBindingMismatch);
        }
        SharedObservationPhaseV1::Collecting
    };
    let slot_offset = shared_observation_slot_offset_v1(usize::from(observation_count))?;
    put(bytes, slot_offset, &evidence.to_bytes());
    put(bytes, 12, &next_count.to_le_bytes());
    put(bytes, 248, &accepted_sequence.to_le_bytes());
    if let Some(evidence_id) = completed_evidence_id {
        put(bytes, 216, evidence_id.as_bytes());
    }
    put(bytes, 10, &[next_phase.byte()]);
    Ok(())
}

/// Retire one shared-observation child directly in its exact account bytes.
///
/// Refusal leaves the bytes unchanged, including on a Market child-count
/// mismatch.
#[inline(never)]
pub fn retire_shared_observation_in_place_v1(
    bytes: &mut [u8],
    generation: u64,
    current_unix_seconds: i64,
    expected_market_child_count: u64,
    authenticated_market_child_count: u64,
) -> Result<MarketChildDeltaV1> {
    let view = SharedObservationStateViewV1::decode(bytes)?;
    if view.generation()? != generation
        || view.phase()? == SharedObservationPhaseV1::Retired
        || current_unix_seconds < view.created_at_unix_seconds()?
    {
        return Err(Error::InvalidSharedObservation);
    }
    let delta = MarketChildDeltaV1::retire(
        expected_market_child_count,
        authenticated_market_child_count,
    )?;
    put(bytes, 10, &[SharedObservationPhaseV1::Retired.byte()]);
    put(bytes, 264, &current_unix_seconds.to_le_bytes());
    Ok(delta)
}

/// Closed V1 source instruction action set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceActionV1 {
    /// Create a Market-generation source-resolution state.
    CreateResolution = 1,
    /// Accept primary or recovery evidence and apply Product mapping.
    AcceptEvidence = 2,
    /// Expire the current leg and enter exactly the next recovery attempt.
    FailNext = 3,
    /// Commit explicit exhaustion after the last admitted leg.
    Exhaust = 4,
    /// Commit Product-owned failure semantics after exhaustion.
    CommitFailure = 5,
    /// Retire a terminal source-resolution state.
    RetireResolution = 6,
    /// Create an explicitly selected shared-observation child.
    CreateSharedObservation = 7,
    /// Accept one evidence set into a shared-observation child.
    AcceptSharedObservation = 8,
    /// Retire a shared-observation child without creating an archive.
    RetireSharedObservation = 9,
}

impl SourceActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::CreateResolution),
            2 => Ok(Self::AcceptEvidence),
            3 => Ok(Self::FailNext),
            4 => Ok(Self::Exhaust),
            5 => Ok(Self::CommitFailure),
            6 => Ok(Self::RetireResolution),
            7 => Ok(Self::CreateSharedObservation),
            8 => Ok(Self::AcceptSharedObservation),
            9 => Ok(Self::RetireSharedObservation),
            _ => Err(Error::UnknownInstructionAction),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Fixed CreateResolution wire fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateResolutionInstructionV1 {
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    rent_beneficiary: [u8; 32],
    expected_market_child_count: u64,
    pda_bump: u8,
    reopen_link: Option<ReopenLinkV1>,
}

impl CreateResolutionInstructionV1 {
    /// Construct a fresh or explicitly reopen-linked request.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        rent_beneficiary: [u8; 32],
        expected_market_child_count: u64,
        pda_bump: u8,
        reopen_link: Option<ReopenLinkV1>,
    ) -> Result<Self> {
        nonzero_identifier(&market)?;
        nonzero_identifier(&rent_beneficiary)?;
        if let Some(link) = reopen_link {
            link.validate_successor(market, generation)?;
        }
        Ok(Self {
            market,
            generation,
            material_id,
            rent_beneficiary,
            expected_market_child_count,
            pda_bump,
            reopen_link,
        })
    }

    /// Encode one exact fixed instruction.
    pub fn to_bytes(self) -> [u8; CREATE_RESOLUTION_INSTRUCTION_BYTES] {
        let mut out = instruction_base::<CREATE_RESOLUTION_INSTRUCTION_BYTES>(
            SourceActionV1::CreateResolution,
        );
        put(&mut out, 16, &self.market);
        put(&mut out, 48, &self.generation.to_le_bytes());
        put(&mut out, 56, self.material_id.as_bytes());
        put(&mut out, 88, &self.rent_beneficiary);
        put(
            &mut out,
            120,
            &self.expected_market_child_count.to_le_bytes(),
        );
        put(&mut out, 128, &[self.pda_bump]);
        if let Some(link) = self.reopen_link {
            put(&mut out, 129, &[1]);
            put(&mut out, 144, &link.to_bytes());
        }
        out
    }

    /// Return the requested Market key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the one Source-material content identity.
    pub const fn material_id(self) -> ContentId {
        self.material_id
    }

    /// Return the RentCredit beneficiary authority.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the exact Market child-count replay guard.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }

    /// Return the requested PDA bump.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }

    /// Return the optional by-value successor-generation linkage.
    pub const fn reopen_link(self) -> Option<ReopenLinkV1> {
        self.reopen_link
    }
}

/// Fixed Source prefix for AcceptEvidence; provider bytes follow this prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptEvidenceInstructionV1 {
    generation: u64,
    terminal_sequence: u64,
}

impl AcceptEvidenceInstructionV1 {
    /// Construct an evidence-identity and replay-sequence request.
    pub fn new(generation: u64, terminal_sequence: u64) -> Result<Self> {
        if terminal_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        Ok(Self {
            generation,
            terminal_sequence,
        })
    }

    /// Encode one exact request with no caller-selected result.
    pub fn to_prefix_bytes(self) -> [u8; ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES] {
        let mut out = instruction_base::<ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES>(
            SourceActionV1::AcceptEvidence,
        );
        put(&mut out, 16, &self.generation.to_le_bytes());
        put(&mut out, 24, &self.terminal_sequence.to_le_bytes());
        out
    }

    /// Return the expected generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the positive terminal sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

/// Fixed generation-guarded wire shared by simple lifecycle actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationInstructionV1 {
    action: SourceActionV1,
    generation: u64,
}

impl GenerationInstructionV1 {
    /// Construct only FailNext or Exhaust.
    pub fn new(action: SourceActionV1, generation: u64) -> Result<Self> {
        if !matches!(action, SourceActionV1::FailNext | SourceActionV1::Exhaust) {
            return Err(Error::UnknownInstructionAction);
        }
        Ok(Self { action, generation })
    }

    /// Encode one exact lifecycle request.
    pub fn to_bytes(self) -> [u8; GENERATION_INSTRUCTION_BYTES] {
        let mut out = instruction_base::<GENERATION_INSTRUCTION_BYTES>(self.action);
        put(&mut out, 16, &self.generation.to_le_bytes());
        out
    }

    /// Return the exact lifecycle action.
    pub const fn action(self) -> SourceActionV1 {
        self.action
    }

    /// Return the expected Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Fixed generation and Market-child-count replay guard for retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetireInstructionV1 {
    action: SourceActionV1,
    generation: u64,
    expected_market_child_count: u64,
}

impl RetireInstructionV1 {
    /// Construct one resolution-state or shared-child retirement request.
    pub fn new(
        action: SourceActionV1,
        generation: u64,
        expected_market_child_count: u64,
    ) -> Result<Self> {
        if !matches!(
            action,
            SourceActionV1::RetireResolution | SourceActionV1::RetireSharedObservation
        ) {
            return Err(Error::UnknownInstructionAction);
        }
        Ok(Self {
            action,
            generation,
            expected_market_child_count,
        })
    }

    /// Encode one exact retirement request.
    pub fn to_bytes(self) -> [u8; RETIRE_INSTRUCTION_BYTES] {
        let mut out = instruction_base::<RETIRE_INSTRUCTION_BYTES>(self.action);
        put(&mut out, 16, &self.generation.to_le_bytes());
        put(
            &mut out,
            24,
            &self.expected_market_child_count.to_le_bytes(),
        );
        out
    }

    /// Return the retirement action.
    pub const fn action(self) -> SourceActionV1 {
        self.action
    }

    /// Return the expected generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact authenticated Market child count before retirement.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }
}

/// Fixed failure-commit wire containing no failure selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitFailureInstructionV1 {
    generation: u64,
    terminal_sequence: u64,
}

impl CommitFailureInstructionV1 {
    /// Construct an exhausted-state failure commit.
    pub fn new(generation: u64, terminal_sequence: u64) -> Result<Self> {
        if terminal_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        Ok(Self {
            generation,
            terminal_sequence,
        })
    }

    /// Encode one exact failure-commit request.
    pub fn to_bytes(self) -> [u8; COMMIT_FAILURE_INSTRUCTION_BYTES] {
        let mut out =
            instruction_base::<COMMIT_FAILURE_INSTRUCTION_BYTES>(SourceActionV1::CommitFailure);
        put(&mut out, 16, &self.generation.to_le_bytes());
        put(&mut out, 24, &self.terminal_sequence.to_le_bytes());
        out
    }

    /// Return the expected generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the positive terminal sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

/// Fixed CreateSharedObservation wire fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateSharedObservationInstructionV1 {
    market: [u8; 32],
    generation: u64,
    material_id: ContentId,
    source_spec_id: ContentId,
    window_spec_id: ContentId,
    rent_beneficiary: [u8; 32],
    expected_market_child_count: u64,
    pda_bump: u8,
}

impl CreateSharedObservationInstructionV1 {
    /// Construct one shared-child request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market: [u8; 32],
        generation: u64,
        material_id: ContentId,
        source_spec_id: ContentId,
        window_spec_id: ContentId,
        rent_beneficiary: [u8; 32],
        expected_market_child_count: u64,
        pda_bump: u8,
    ) -> Result<Self> {
        nonzero_identifier(&market)?;
        nonzero_identifier(&rent_beneficiary)?;
        Ok(Self {
            market,
            generation,
            material_id,
            source_spec_id,
            window_spec_id,
            rent_beneficiary,
            expected_market_child_count,
            pda_bump,
        })
    }

    /// Encode one exact child-create request.
    pub fn to_bytes(self) -> [u8; CREATE_SHARED_OBSERVATION_INSTRUCTION_BYTES] {
        let mut out = instruction_base::<CREATE_SHARED_OBSERVATION_INSTRUCTION_BYTES>(
            SourceActionV1::CreateSharedObservation,
        );
        put(&mut out, 16, &self.market);
        put(&mut out, 48, &self.generation.to_le_bytes());
        put(&mut out, 56, self.material_id.as_bytes());
        put(&mut out, 88, self.source_spec_id.as_bytes());
        put(&mut out, 120, self.window_spec_id.as_bytes());
        put(&mut out, 152, &self.rent_beneficiary);
        put(
            &mut out,
            184,
            &self.expected_market_child_count.to_le_bytes(),
        );
        put(&mut out, 192, &[self.pda_bump]);
        out
    }

    /// Return the Market key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the single Source-material content identity.
    pub const fn material_id(self) -> ContentId {
        self.material_id
    }

    /// Return the source-specification identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the window-specification identity.
    pub const fn window_spec_id(self) -> ContentId {
        self.window_spec_id
    }

    /// Return the RentCredit beneficiary authority.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Return the exact Market child-count replay guard.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }

    /// Return the requested PDA bump.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }
}

/// Fixed AcceptSharedObservation wire fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptSharedObservationInstructionV1 {
    generation: u64,
    accepted_sequence: u64,
    completed_evidence_id: Option<ContentId>,
}

impl AcceptSharedObservationInstructionV1 {
    /// Construct one replay-guarded child accept request.
    pub fn new(
        generation: u64,
        accepted_sequence: u64,
        completed_evidence_id: Option<ContentId>,
    ) -> Result<Self> {
        if accepted_sequence == 0 {
            return Err(Error::ZeroSequence);
        }
        Ok(Self {
            generation,
            accepted_sequence,
            completed_evidence_id,
        })
    }

    /// Encode one exact child-accept request.
    pub fn to_prefix_bytes(self) -> [u8; ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES] {
        let mut out = instruction_base::<ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES>(
            SourceActionV1::AcceptSharedObservation,
        );
        put(&mut out, 16, &self.generation.to_le_bytes());
        put(&mut out, 24, &self.accepted_sequence.to_le_bytes());
        if let Some(id) = self.completed_evidence_id {
            put(&mut out, 32, id.as_bytes());
        }
        out
    }

    /// Return the expected generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the positive child replay sequence.
    pub const fn accepted_sequence(self) -> u64 {
        self.accepted_sequence
    }

    /// Return the evidence-set digest expected only on the completing append.
    pub const fn completed_evidence_id(self) -> Option<ContentId> {
        self.completed_evidence_id
    }
}

/// Hostile-decoded closed source instruction set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceInstructionV1<'a> {
    /// Create a source-resolution state.
    CreateResolution(CreateResolutionInstructionV1),
    /// Accept source evidence.
    AcceptEvidence(AcceptEvidenceInstructionV1, &'a [u8]),
    /// Enter exactly the next recovery attempt.
    FailNext(GenerationInstructionV1),
    /// Commit exhaustion.
    Exhaust(GenerationInstructionV1),
    /// Commit failure semantics.
    CommitFailure(CommitFailureInstructionV1),
    /// Retire a source-resolution state.
    RetireResolution(RetireInstructionV1),
    /// Create a shared observation.
    CreateSharedObservation(CreateSharedObservationInstructionV1),
    /// Accept a shared observation.
    AcceptSharedObservation(AcceptSharedObservationInstructionV1, &'a [u8]),
    /// Retire a shared observation.
    RetireSharedObservation(RetireInstructionV1),
}

impl<'a> SourceInstructionV1<'a> {
    /// Decode one hostile fixed Source prefix plus the selected provider payload.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < SOURCE_INSTRUCTION_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != SOURCE_INSTRUCTION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        zero(bytes, 11, 5)?;
        let action = SourceActionV1::decode(one(bytes, 10)?)?;
        match action {
            SourceActionV1::CreateResolution => {
                require_instruction_length(bytes, CREATE_RESOLUTION_INSTRUCTION_BYTES)?;
                zero(bytes, 130, 14)?;
                zero(bytes, 272, 16)?;
                let reopen_link = match one(bytes, 129)? {
                    0 => {
                        zero(bytes, 144, REOPEN_LINK_BYTES)?;
                        None
                    }
                    1 => Some(ReopenLinkV1::decode(slice(bytes, 144, REOPEN_LINK_BYTES)?)?),
                    _ => return Err(Error::InvalidReopenLink),
                };
                Ok(Self::CreateResolution(CreateResolutionInstructionV1::new(
                    read_array(bytes, 16)?,
                    u64::from_le_bytes(read_array(bytes, 48)?),
                    content(bytes, 56)?,
                    read_array(bytes, 88)?,
                    u64::from_le_bytes(read_array(bytes, 120)?),
                    one(bytes, 128)?,
                    reopen_link,
                )?))
            }
            SourceActionV1::AcceptEvidence => {
                if bytes.len() < ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES {
                    return Err(Error::InvalidLength);
                }
                Ok(Self::AcceptEvidence(
                    AcceptEvidenceInstructionV1::new(
                        u64::from_le_bytes(read_array(bytes, 16)?),
                        u64::from_le_bytes(read_array(bytes, 24)?),
                    )?,
                    bytes
                        .get(ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES..)
                        .ok_or(Error::InvalidLength)?,
                ))
            }
            SourceActionV1::CommitFailure => {
                require_instruction_length(bytes, COMMIT_FAILURE_INSTRUCTION_BYTES)?;
                Ok(Self::CommitFailure(CommitFailureInstructionV1::new(
                    u64::from_le_bytes(read_array(bytes, 16)?),
                    u64::from_le_bytes(read_array(bytes, 24)?),
                )?))
            }
            SourceActionV1::CreateSharedObservation => {
                require_instruction_length(bytes, CREATE_SHARED_OBSERVATION_INSTRUCTION_BYTES)?;
                zero(bytes, 193, 15)?;
                Ok(Self::CreateSharedObservation(
                    CreateSharedObservationInstructionV1::new(
                        read_array(bytes, 16)?,
                        u64::from_le_bytes(read_array(bytes, 48)?),
                        content(bytes, 56)?,
                        content(bytes, 88)?,
                        content(bytes, 120)?,
                        read_array(bytes, 152)?,
                        u64::from_le_bytes(read_array(bytes, 184)?),
                        one(bytes, 192)?,
                    )?,
                ))
            }
            SourceActionV1::AcceptSharedObservation => {
                if bytes.len() <= ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES {
                    return Err(Error::InvalidProviderPayload);
                }
                Ok(Self::AcceptSharedObservation(
                    AcceptSharedObservationInstructionV1::new(
                        u64::from_le_bytes(read_array(bytes, 16)?),
                        u64::from_le_bytes(read_array(bytes, 24)?),
                        read_optional_content(bytes, 32)?,
                    )?,
                    bytes
                        .get(ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES..)
                        .ok_or(Error::InvalidLength)?,
                ))
            }
            SourceActionV1::FailNext | SourceActionV1::Exhaust => {
                require_instruction_length(bytes, GENERATION_INSTRUCTION_BYTES)?;
                let value = GenerationInstructionV1::new(
                    action,
                    u64::from_le_bytes(read_array(bytes, 16)?),
                )?;
                match action {
                    SourceActionV1::FailNext => Ok(Self::FailNext(value)),
                    SourceActionV1::Exhaust => Ok(Self::Exhaust(value)),
                    _ => Err(Error::UnknownInstructionAction),
                }
            }
            SourceActionV1::RetireResolution | SourceActionV1::RetireSharedObservation => {
                require_instruction_length(bytes, RETIRE_INSTRUCTION_BYTES)?;
                let value = RetireInstructionV1::new(
                    action,
                    u64::from_le_bytes(read_array(bytes, 16)?),
                    u64::from_le_bytes(read_array(bytes, 24)?),
                )?;
                match action {
                    SourceActionV1::RetireResolution => Ok(Self::RetireResolution(value)),
                    SourceActionV1::RetireSharedObservation => {
                        Ok(Self::RetireSharedObservation(value))
                    }
                    _ => Err(Error::UnknownInstructionAction),
                }
            }
        }
    }
}

fn instruction_base<const N: usize>(action: SourceActionV1) -> [u8; N] {
    let mut out = [0u8; N];
    put(&mut out, 0, &SOURCE_INSTRUCTION_MAGIC);
    put(&mut out, 8, &SCHEMA_VERSION.to_le_bytes());
    put(&mut out, 10, &[action.byte()]);
    out
}

fn require_instruction_length(bytes: &[u8], expected: usize) -> Result<()> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

/// Semantic account class for exact source-operation frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAccountClassV1 {
    /// Transaction payer or provider resolver authority.
    SignerAuthority,
    /// Program-owned mutable source-resolution state.
    ResolutionState,
    /// Program-owned mutable shared-observation child.
    SharedObservation,
    /// Provider-neutral Market state.
    Market,
    /// The one finalized immutable Source-material raw record.
    SourceMaterialRecord,
    /// Vacant canonical staging PDA proving material finalization.
    RecordStagingVacancy,
    /// Immutable capability manifest selected by the Market.
    CapabilityManifest,
    /// Mutable capability funding state authenticated against present principal.
    FundingState,
    /// Pre-existing permanent beneficiary RentCredit.
    RentCredit,
    /// Executable System or provider program.
    ExecutableProgram,
    /// Clock or Rent sysvar.
    Sysvar,
    /// Mutable provider-owned temporary account.
    ProviderMutable,
    /// Readonly provider program-data or configuration account.
    ProviderReadonly,
    /// Readonly provider message account.
    ProviderMessage,
    /// Writable provider treasury.
    ProviderTreasury,
}

/// Semantic role name in one ordered source-operation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAccountNameV1 {
    /// Funding payer.
    Payer,
    /// Source-resolution state PDA.
    ResolutionState,
    /// Readonly retired predecessor state on a reopen.
    PredecessorResolutionState,
    /// Shared-observation PDA.
    SharedObservation,
    /// Provider-neutral Market.
    Market,
    /// Single raw Source-material record.
    SourceMaterial,
    /// Canonical vacant staging PDA paired with Source material.
    SourceMaterialStagingVacancy,
    /// Capability manifest required only by recovery funding authentication.
    CapabilityManifest,
    /// Mutable capability funding state.
    FundingState,
    /// Permanent beneficiary RentCredit.
    RentCredit,
    /// Executable System Program.
    SystemProgram,
    /// Rent sysvar.
    RentSysvar,
    /// Clock sysvar.
    ClockSysvar,
    /// Pyth resolver authority.
    ProviderResolver,
    /// Temporary Pyth update account.
    ProviderUpdate,
    /// Pyth Receiver program.
    ReceiverProgram,
    /// Pyth Receiver ProgramData.
    ReceiverProgramData,
    /// Pyth Receiver configuration.
    ReceiverConfig,
    /// Encoded VAA consumed by Receiver.
    EncodedVaa,
    /// Pyth router program.
    RouterProgram,
    /// Pyth router ProgramData.
    RouterProgramData,
    /// Pyth Receiver treasury.
    ReceiverTreasury,
}

/// One ordered SDK-free account-role requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountRoleV1 {
    name: SourceAccountNameV1,
    class: SourceAccountClassV1,
    signer: bool,
    writable: bool,
    executable: bool,
}

impl SourceAccountRoleV1 {
    /// Return the ordered semantic role name.
    pub const fn name(self) -> SourceAccountNameV1 {
        self.name
    }

    /// Return the adapter-authenticated account class.
    pub const fn class(self) -> SourceAccountClassV1 {
        self.class
    }

    /// Return the exact signer requirement.
    pub const fn is_signer(self) -> bool {
        self.signer
    }

    /// Return the exact writable requirement.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// Return the exact executable requirement.
    pub const fn is_executable(self) -> bool {
        self.executable
    }
}

const fn source_role(
    name: SourceAccountNameV1,
    class: SourceAccountClassV1,
    signer: bool,
    writable: bool,
    executable: bool,
) -> SourceAccountRoleV1 {
    SourceAccountRoleV1 {
        name,
        class,
        signer,
        writable,
        executable,
    }
}
const PAYER: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::Payer,
    SourceAccountClassV1::SignerAuthority,
    true,
    true,
    false,
);
const STATE: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ResolutionState,
    SourceAccountClassV1::ResolutionState,
    false,
    true,
    false,
);
const PREDECESSOR: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::PredecessorResolutionState,
    SourceAccountClassV1::ResolutionState,
    false,
    false,
    false,
);
const SHARED: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::SharedObservation,
    SourceAccountClassV1::SharedObservation,
    false,
    true,
    false,
);
const SHARED_READ: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::SharedObservation,
    SourceAccountClassV1::SharedObservation,
    false,
    false,
    false,
);
const MARKET_READ: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::Market,
    SourceAccountClassV1::Market,
    false,
    false,
    false,
);
const MARKET_WRITE: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::Market,
    SourceAccountClassV1::Market,
    false,
    true,
    false,
);
const MATERIAL: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::SourceMaterial,
    SourceAccountClassV1::SourceMaterialRecord,
    false,
    false,
    false,
);
const MATERIAL_STAGE: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::SourceMaterialStagingVacancy,
    SourceAccountClassV1::RecordStagingVacancy,
    false,
    false,
    false,
);
const MANIFEST: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::CapabilityManifest,
    SourceAccountClassV1::CapabilityManifest,
    false,
    false,
    false,
);
const FUNDING: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::FundingState,
    SourceAccountClassV1::FundingState,
    false,
    true,
    false,
);
const CREDIT_READ: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::RentCredit,
    SourceAccountClassV1::RentCredit,
    false,
    false,
    false,
);
const CREDIT_WRITE: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::RentCredit,
    SourceAccountClassV1::RentCredit,
    false,
    true,
    false,
);
const SYSTEM: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::SystemProgram,
    SourceAccountClassV1::ExecutableProgram,
    false,
    false,
    true,
);
const RENT: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::RentSysvar,
    SourceAccountClassV1::Sysvar,
    false,
    false,
    false,
);
const CLOCK: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ClockSysvar,
    SourceAccountClassV1::Sysvar,
    false,
    false,
    false,
);
const RESOLVER: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ProviderResolver,
    SourceAccountClassV1::SignerAuthority,
    true,
    true,
    false,
);
const UPDATE: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ProviderUpdate,
    SourceAccountClassV1::ProviderMutable,
    true,
    true,
    false,
);
const RECEIVER: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ReceiverProgram,
    SourceAccountClassV1::ExecutableProgram,
    false,
    false,
    true,
);
const RECEIVER_DATA: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ReceiverProgramData,
    SourceAccountClassV1::ProviderReadonly,
    false,
    false,
    false,
);
const CONFIG: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ReceiverConfig,
    SourceAccountClassV1::ProviderReadonly,
    false,
    false,
    false,
);
const VAA: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::EncodedVaa,
    SourceAccountClassV1::ProviderMessage,
    false,
    false,
    false,
);
const ROUTER: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::RouterProgram,
    SourceAccountClassV1::ExecutableProgram,
    false,
    false,
    true,
);
const ROUTER_DATA: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::RouterProgramData,
    SourceAccountClassV1::ProviderReadonly,
    false,
    false,
    false,
);
const TREASURY: SourceAccountRoleV1 = source_role(
    SourceAccountNameV1::ReceiverTreasury,
    SourceAccountClassV1::ProviderTreasury,
    false,
    true,
    false,
);

/// Exact Pyth Receiver provider-extension accounts. The selected adapter must
/// authenticate Program/ProgramData linkage, deployment slots, config digest,
/// router/config/treasury PDAs, encoded VAA ownership and payload, post a fully
/// verified update, normalize exact time/value fields, and reclaim the update.
pub const PYTH_PROVIDER_EXTENSION_FRAME_V1: [SourceAccountRoleV1; 10] = [
    RESOLVER,
    UPDATE,
    RECEIVER,
    RECEIVER_DATA,
    CONFIG,
    VAA,
    ROUTER,
    ROUTER_DATA,
    TREASURY,
    SYSTEM,
];

/// Select the only provider extension implemented by V1.
pub fn provider_extension_roles_v1(
    adapter_release_id: ContentId,
) -> Result<&'static [SourceAccountRoleV1]> {
    if adapter_release_id.to_bytes() == PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1 {
        Ok(&PYTH_PROVIDER_EXTENSION_FRAME_V1)
    } else {
        Err(Error::UnsupportedProviderExtension)
    }
}

/// Exact fixed Source prefix for inline primary acceptance.
pub const ACCEPT_PRIMARY_INLINE_SOURCE_PREFIX_V1: [SourceAccountRoleV1; 6] =
    [STATE, MARKET_WRITE, MATERIAL, MATERIAL_STAGE, RENT, CLOCK];
/// Exact fixed Source prefix for inline recovery acceptance.
pub const ACCEPT_RECOVERY_INLINE_SOURCE_PREFIX_V1: [SourceAccountRoleV1; 8] = [
    STATE,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    MANIFEST,
    FUNDING,
    CLOCK,
];
/// Exact fixed Source prefix for shared-child provider acceptance.
pub const ACCEPT_SHARED_SOURCE_PREFIX_V1: [SourceAccountRoleV1; 5] =
    [SHARED, MATERIAL, MATERIAL_STAGE, RENT, CLOCK];

/// Exact fresh resolution-state creation frame and one Market registration.
pub const CREATE_RESOLUTION_FRESH_FRAME_V1: [SourceAccountRoleV1; 8] = [
    PAYER,
    STATE,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CREDIT_READ,
    SYSTEM,
];
/// Exact reopen-linked creation frame with a readonly predecessor state.
pub const CREATE_RESOLUTION_REOPEN_FRAME_V1: [SourceAccountRoleV1; 9] = [
    PAYER,
    STATE,
    PREDECESSOR,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CREDIT_READ,
    SYSTEM,
];
/// Exact inline primary frame: Source prefix followed by the Pyth extension.
pub const ACCEPT_PRIMARY_INLINE_FRAME_V1: [SourceAccountRoleV1; 16] = [
    STATE,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CLOCK,
    RESOLVER,
    UPDATE,
    RECEIVER,
    RECEIVER_DATA,
    CONFIG,
    VAA,
    ROUTER,
    ROUTER_DATA,
    TREASURY,
    SYSTEM,
];
/// Exact primary frame consuming an already authenticated shared child.
pub const ACCEPT_PRIMARY_SHARED_FRAME_V1: [SourceAccountRoleV1; 7] = [
    STATE,
    SHARED_READ,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CLOCK,
];
/// Exact recovery frame with capability funding and Pyth extension.
pub const ACCEPT_RECOVERY_INLINE_FRAME_V1: [SourceAccountRoleV1; 18] = [
    STATE,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    MANIFEST,
    FUNDING,
    CLOCK,
    RESOLVER,
    UPDATE,
    RECEIVER,
    RECEIVER_DATA,
    CONFIG,
    VAA,
    ROUTER,
    ROUTER_DATA,
    TREASURY,
    SYSTEM,
];
/// Exact recovery frame consuming an already authenticated shared child.
pub const ACCEPT_RECOVERY_SHARED_FRAME_V1: [SourceAccountRoleV1; 9] = [
    STATE,
    SHARED_READ,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    MANIFEST,
    FUNDING,
    CLOCK,
];
/// Exact ordered recovery-entry frame with actual present capability funding.
pub const FAIL_NEXT_FRAME_V1: [SourceAccountRoleV1; 8] = [
    STATE,
    MARKET_READ,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    MANIFEST,
    FUNDING,
    CLOCK,
];
/// Exact no-recovery primary exhaustion frame.
pub const EXHAUST_PRIMARY_FRAME_V1: [SourceAccountRoleV1; 6] =
    [STATE, MARKET_READ, MATERIAL, MATERIAL_STAGE, RENT, CLOCK];
/// Exact final-recovery exhaustion frame; recovery is embedded in material.
pub const EXHAUST_RECOVERY_FRAME_V1: [SourceAccountRoleV1; 6] = EXHAUST_PRIMARY_FRAME_V1;
/// Exact Product-owned failure-commit frame.
pub const COMMIT_FAILURE_FRAME_V1: [SourceAccountRoleV1; 5] =
    [STATE, MARKET_WRITE, MATERIAL, MATERIAL_STAGE, RENT];
/// Exact state retirement, Market decrement, and RentCredit closure frame.
pub const RETIRE_RESOLUTION_FRAME_V1: [SourceAccountRoleV1; 4] =
    [STATE, MARKET_WRITE, CREDIT_WRITE, CLOCK];
/// Exact direct shared-child creation and Market registration frame.
pub const CREATE_SHARED_OBSERVATION_FRAME_V1: [SourceAccountRoleV1; 9] = [
    PAYER,
    SHARED,
    MARKET_WRITE,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CREDIT_READ,
    SYSTEM,
    CLOCK,
];
/// Exact shared-child append frame followed by the Pyth provider extension.
pub const ACCEPT_SHARED_OBSERVATION_FRAME_V1: [SourceAccountRoleV1; 15] = [
    SHARED,
    MATERIAL,
    MATERIAL_STAGE,
    RENT,
    CLOCK,
    RESOLVER,
    UPDATE,
    RECEIVER,
    RECEIVER_DATA,
    CONFIG,
    VAA,
    ROUTER,
    ROUTER_DATA,
    TREASURY,
    SYSTEM,
];
/// Exact shared-child retirement, Market decrement, and RentCredit closure.
pub const RETIRE_SHARED_OBSERVATION_FRAME_V1: [SourceAccountRoleV1; 4] =
    [SHARED, MARKET_WRITE, CREDIT_WRITE, CLOCK];

/// Closed exact account-frame selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFrameKindV1 {
    /// Fresh source-state creation.
    CreateResolutionFresh,
    /// Reopen-linked source-state creation.
    CreateResolutionReopen,
    /// Inline primary evidence acceptance.
    AcceptPrimaryInline,
    /// Shared primary evidence acceptance.
    AcceptPrimaryShared,
    /// Inline recovery evidence acceptance.
    AcceptRecoveryInline,
    /// Shared recovery evidence acceptance.
    AcceptRecoveryShared,
    /// Enter the next recovery leg.
    FailNext,
    /// Exhaust a primary policy without recovery.
    ExhaustPrimary,
    /// Exhaust the final recovery leg.
    ExhaustRecovery,
    /// Commit Product-owned failure.
    CommitFailure,
    /// Retire source-resolution state.
    RetireResolution,
    /// Create shared observation.
    CreateSharedObservation,
    /// Accept shared observation.
    AcceptSharedObservation,
    /// Retire shared observation.
    RetireSharedObservation,
}

/// SDK-free observed account key and privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAccountPrivilegeV1 {
    /// Exact account key bytes used for alias validation.
    pub key: [u8; 32],
    /// Runtime signer privilege.
    pub is_signer: bool,
    /// Runtime writable privilege.
    pub is_writable: bool,
    /// Runtime executable privilege.
    pub is_executable: bool,
}

/// Return the exact ordered roles for one source operation.
pub const fn source_frame_roles_v1(kind: SourceFrameKindV1) -> &'static [SourceAccountRoleV1] {
    match kind {
        SourceFrameKindV1::CreateResolutionFresh => &CREATE_RESOLUTION_FRESH_FRAME_V1,
        SourceFrameKindV1::CreateResolutionReopen => &CREATE_RESOLUTION_REOPEN_FRAME_V1,
        SourceFrameKindV1::AcceptPrimaryInline => &ACCEPT_PRIMARY_INLINE_FRAME_V1,
        SourceFrameKindV1::AcceptPrimaryShared => &ACCEPT_PRIMARY_SHARED_FRAME_V1,
        SourceFrameKindV1::AcceptRecoveryInline => &ACCEPT_RECOVERY_INLINE_FRAME_V1,
        SourceFrameKindV1::AcceptRecoveryShared => &ACCEPT_RECOVERY_SHARED_FRAME_V1,
        SourceFrameKindV1::FailNext => &FAIL_NEXT_FRAME_V1,
        SourceFrameKindV1::ExhaustPrimary => &EXHAUST_PRIMARY_FRAME_V1,
        SourceFrameKindV1::ExhaustRecovery => &EXHAUST_RECOVERY_FRAME_V1,
        SourceFrameKindV1::CommitFailure => &COMMIT_FAILURE_FRAME_V1,
        SourceFrameKindV1::RetireResolution => &RETIRE_RESOLUTION_FRAME_V1,
        SourceFrameKindV1::CreateSharedObservation => &CREATE_SHARED_OBSERVATION_FRAME_V1,
        SourceFrameKindV1::AcceptSharedObservation => &ACCEPT_SHARED_OBSERVATION_FRAME_V1,
        SourceFrameKindV1::RetireSharedObservation => &RETIRE_SHARED_OBSERVATION_FRAME_V1,
    }
}

/// Validate exact count, privileges, and complete no-alias policy for one frame.
pub fn validate_source_frame_v1(
    kind: SourceFrameKindV1,
    accounts: &[SourceAccountPrivilegeV1],
) -> Result<()> {
    let roles = source_frame_roles_v1(kind);
    if accounts.len() != roles.len() {
        return Err(Error::InvalidAccountFrame);
    }
    for (actual, required) in accounts.iter().zip(roles) {
        if actual.is_signer != required.signer
            || actual.is_writable != required.writable
            || actual.is_executable != required.executable
        {
            return Err(Error::InvalidAccountFrame);
        }
    }
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key {
                return Err(Error::InvalidAccountFrame);
            }
        }
    }
    Ok(())
}

fn header(bytes: &[u8], expected: usize, magic: [u8; 8]) -> Result<()> {
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != magic {
        return Err(Error::InvalidMagic);
    }
    if u16::from_le_bytes(read_array(bytes, 8)?) != SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}
fn base<const N: usize>(magic: [u8; 8]) -> [u8; N] {
    let mut out = [0u8; N];
    put(&mut out, 0, &magic);
    put(&mut out, 8, &SCHEMA_VERSION.to_le_bytes());
    out
}
fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset.checked_add(length).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}
fn one(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}
fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array(bytes, offset)?)
}
fn read_optional_content(bytes: &[u8], offset: usize) -> Result<Option<ContentId>> {
    let raw = read_array(bytes, offset)?;
    if raw.iter().all(|byte| *byte == 0) {
        Ok(None)
    } else {
        Ok(Some(ContentId::new(raw)?))
    }
}
fn nonzero_identifier(bytes: &[u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}
fn zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}
fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    if let Some(dest) = output.get_mut(offset..offset.saturating_add(input.len())) {
        dest.copy_from_slice(input);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn id(fill: u8) -> ContentId {
        ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test identity")
    }
    fn product_id(fill: u8) -> dclutch_product_contract::ContentId {
        dclutch_product_contract::ContentId::new([fill; CONTENT_ID_BYTES])
            .expect("nonzero Product identity")
    }
    fn product_instance(result_domain_id: u8) -> InstanceV1 {
        use dclutch_product_contract::{capacity::CapacityProfileId, product::InstanceV1Input};
        InstanceV1::new(InstanceV1Input {
            terms_id: product_id(31),
            occurrence_id: product_id(32),
            claim_basis_id: product_id(33),
            result_domain_id: product_id(result_domain_id),
            capacity_profile_id: CapacityProfileId::new(product_id(34)),
            partition_cell_count: 3,
        })
        .expect("canonical Product instance")
    }
    fn profile() -> SourceCapacityProfileV1 {
        SourceCapacityProfileV1::new(CapacityEnvelope::Provisional, 8, 2, id(1), id(2), 512, 4)
            .expect("valid profile")
    }
    fn statistic(rounding: RoundingBoundary) -> StatisticSpecV1 {
        StatisticSpecV1 {
            source_unit_id: id(3),
            result_unit_id: id(3),
            kind: StatisticKind::ExactScheduledAverage,
            rounding,
            required_samples: 2,
            threshold_atoms: 0,
            capacity_profile_id: id(9),
            evaluator_release_id: id(4),
        }
    }
    fn window() -> WindowSpecV1 {
        WindowSpecV1::new(id(5), WindowKind::ScheduledInterval, 10, 20, 5, 1, id(6))
            .expect("valid window")
    }

    #[test]
    fn exact_average_keeps_or_names_rounding_boundary() {
        let samples = [
            Observation {
                atoms: -2,
                unix_seconds: 10,
            },
            Observation {
                atoms: 1,
                unix_seconds: 20,
            },
        ];
        assert_eq!(
            evaluate(
                statistic(RoundingBoundary::ExactRational),
                window(),
                &samples
            ),
            Ok(StatisticValue::ExactRational {
                numerator: -1,
                denominator: 2
            })
        );
        assert_eq!(
            evaluate(statistic(RoundingBoundary::Floor), window(), &samples),
            Ok(StatisticValue::RoundedAtoms(-1))
        );
        assert_eq!(
            evaluate(statistic(RoundingBoundary::Ceiling), window(), &samples),
            Ok(StatisticValue::RoundedAtoms(0))
        );
    }

    #[test]
    fn hostile_encodings_and_linkage_refuse() {
        let source = SourceSpecV1::new(
            id(1),
            id(2),
            id(3),
            SourceAccessProfile::PythTerminalOneTransaction,
            id(4),
            id(5),
        );
        let bytes = source.to_bytes();
        assert_eq!(SourceSpecV1::decode(&bytes), Ok(source));
        for length in 0..SOURCE_SPEC_BYTES {
            assert_eq!(
                SourceSpecV1::decode(bytes.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut changed = bytes;
        if let Some(slot) = changed.get_mut(11) {
            *slot = 1;
        };
        assert_eq!(
            SourceSpecV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        assert_eq!(
            source.validate_dependencies(id(3), id(6)),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn schedule_time_and_statistic_shapes_refuse() {
        assert_eq!(
            WindowSpecV1::new(id(1), WindowKind::Terminal, 4, 5, 1, 0, id(2)),
            Err(Error::InvalidWindow)
        );
        assert_eq!(
            WindowSpecV1::new(id(1), WindowKind::ScheduledInterval, 5, 5, 1, 0, id(2)),
            Err(Error::InvalidWindow)
        );
        let bad = StatisticSpecV1 {
            kind: StatisticKind::TerminalSample,
            required_samples: 2,
            ..statistic(RoundingBoundary::Floor)
        };
        assert_eq!(
            evaluate(bad, window(), &[]),
            Err(Error::NonCanonicalStatistic)
        );
        let outside = [
            Observation {
                atoms: 1,
                unix_seconds: 9,
            },
            Observation {
                atoms: 2,
                unix_seconds: 20,
            },
        ];
        assert_eq!(
            evaluate(statistic(RoundingBoundary::Floor), window(), &outside),
            Err(Error::InvalidObservationSchedule)
        );
    }

    #[test]
    fn recovery_is_ordered_prepaid_and_cannot_fail_early() {
        let funding_allocation_id = id(11);
        let attempts = [
            Some(RecoveryAttemptV1::new(
                id(7),
                id(8),
                30,
                funding_allocation_id,
            )),
            Some(RecoveryAttemptV1::new(
                id(7),
                id(8),
                40,
                funding_allocation_id,
            )),
            None,
            None,
        ];
        let recovery =
            RecoveryPolicyV1::new(id(9), id(10), attempts, 2, profile()).expect("valid recovery");
        assert_eq!(RecoveryPolicyV1::decode(&recovery.to_bytes()), Ok(recovery));
        assert_eq!(
            recovery.attempt(0).map(|item| item.deadline_unix_seconds()),
            Ok(30)
        );
        assert_eq!(
            recovery.attempt(1).map(|item| item.deadline_unix_seconds()),
            Ok(40)
        );
        assert_eq!(recovery.attempt(2), Err(Error::InvalidRecoveryTransition));
        let unordered = [
            Some(RecoveryAttemptV1::new(
                id(7),
                id(8),
                40,
                funding_allocation_id,
            )),
            Some(RecoveryAttemptV1::new(
                id(7),
                id(8),
                30,
                funding_allocation_id,
            )),
            None,
            None,
        ];
        assert_eq!(
            RecoveryPolicyV1::new(id(9), id(10), unordered, 2, profile()),
            Err(Error::NonCanonicalRecoveryOrder)
        );
    }

    #[test]
    fn recovery_funding_is_one_content_identity_and_policy_binds_every_link() {
        let attempt = RecoveryAttemptV1::new(id(7), id(8), 30, id(11));
        assert_eq!(
            RecoveryAttemptV1::decode_slot(&attempt.to_slot_bytes()),
            Ok(attempt)
        );
        let mut missing_funding = attempt.to_slot_bytes();
        missing_funding
            .get_mut(80..112)
            .expect("funding ID")
            .fill(0);
        assert_eq!(
            RecoveryAttemptV1::decode_slot(&missing_funding),
            Err(Error::ZeroContentId)
        );
        let policy = ResolutionPolicyV1::new(id(1), id(2), id(3), id(4), id(5), id(6), None);
        assert_eq!(
            policy.validate_links(id(2), id(1), id(3), id(4), id(5), id(6)),
            Ok(())
        );
        assert_eq!(
            policy.validate_links(id(2), id(1), id(3), id(4), id(5), id(7)),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn canonical_tails_and_pyth_terminal_profile_are_enforced() {
        let profile_bytes = profile().to_bytes();
        let mut changed_profile = profile_bytes;
        if let Some(slot) = changed_profile.get_mut(88) {
            *slot = 1;
        }
        assert_eq!(
            SourceCapacityProfileV1::decode(&changed_profile),
            Err(Error::NonCanonicalReservedBytes)
        );
        let stat = StatisticSpecV1 {
            source_unit_id: id(2),
            result_unit_id: id(2),
            kind: StatisticKind::TerminalSample,
            rounding: RoundingBoundary::ExactRational,
            required_samples: 1,
            threshold_atoms: 0,
            capacity_profile_id: id(1),
            evaluator_release_id: id(7),
        };
        let mut changed_stat = stat.to_bytes();
        if let Some(slot) = changed_stat.get_mut(82) {
            *slot = 1;
        }
        assert_eq!(
            StatisticSpecV1::decode(&changed_stat),
            Err(Error::NonCanonicalReservedBytes)
        );
        let source = SourceSpecV1::new(
            id(3),
            id(2),
            id(4),
            SourceAccessProfile::PythTerminalOneTransaction,
            id(5),
            id(1),
        );
        let scheduled =
            WindowSpecV1::new(id(11), WindowKind::ScheduledInterval, 10, 20, 5, 1, id(6))
                .expect("valid scheduled window");
        let policy = ResolutionPolicyV1::new(id(1), id(8), id(11), id(12), id(13), id(14), None);
        let domain = FiniteResultDomainV1::new(product_id(3), product_id(2), 1, &[0])
            .expect("linked Product domain");
        assert_eq!(
            policy.validate_material(
                id(8),
                product_instance(14),
                id(11),
                source,
                id(12),
                scheduled,
                id(13),
                stat,
                domain,
            ),
            Err(Error::NonCanonicalSourceProfile)
        );
    }

    fn finite_domain() -> FiniteResultDomainV1 {
        FiniteResultDomainV1::new(product_id(11), product_id(10), 1, &[0])
            .expect("finite Product domain")
    }

    fn runtime_material(
        recovery: Option<(ContentId, RecoveryPolicyV1)>,
        access: SourceAccessProfile,
    ) -> (SourceMaterialV1, SourceSpecV1, ProviderReleaseV1) {
        let capacity = profile();
        let source = SourceSpecV1::new(id(11), id(10), id(9), access, id(12), id(1));
        let provider = ProviderReleaseV1::new(
            id(16),
            ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("Pyth extension"),
            id(17),
            id(18),
            id(19),
        );
        let adapter_config =
            PythAdapterConfigV1::new([42; 32], -8, 100).expect("Pyth adapter configuration");
        let window = WindowSpecV1::new(id(3), WindowKind::Terminal, 100, 100, 10, 2, id(13))
            .expect("window");
        let statistic = StatisticSpecV1::new(
            id(10),
            id(10),
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            id(1),
            id(14),
            capacity,
        )
        .expect("statistic");
        let policy = ResolutionPolicyV1::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(15),
            recovery.map(|(content_id, _)| content_id),
        );
        let mut recovery_slots = [None; MAX_RECOVERY_ATTEMPTS];
        if let Some((_, recovery_policy)) = recovery {
            let mut index = 0u8;
            while index < recovery_policy.attempt_count() {
                *recovery_slots
                    .get_mut(usize::from(index))
                    .expect("bounded recovery slot") = Some(
                    RecoveryMaterialSlotV1::new(id(3), source, id(9), provider, adapter_config)
                        .expect("recovery material"),
                );
                index = index.checked_add(1).expect("bounded attempts");
            }
        }
        let material = SourceMaterialV1::new(
            policy,
            id(1),
            capacity,
            id(3),
            source,
            id(9),
            provider,
            adapter_config,
            id(4),
            window,
            id(5),
            statistic,
            id(2),
            product_instance(15),
            finite_domain(),
            recovery,
            recovery_slots,
        )
        .expect("linked material");
        (material, source, provider)
    }

    fn normalized(atoms: i128, publication: i64) -> NormalizedProviderEvidenceV1 {
        NormalizedProviderEvidenceV1::new(
            id(3),
            id(9),
            id(23),
            ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("Pyth extension"),
            id(13),
            0,
            100,
            publication,
            atoms,
        )
    }

    fn shared_median_material() -> SourceMaterialV1 {
        let capacity = SourceCapacityProfileV1::new(
            CapacityEnvelope::Provisional,
            8,
            2,
            id(1),
            id(2),
            1_024,
            4,
        )
        .expect("shared capacity");
        let source = SourceSpecV1::new(
            id(11),
            id(10),
            id(9),
            SourceAccessProfile::SharedObservationChild,
            id(12),
            id(1),
        );
        let provider = ProviderReleaseV1::new(
            id(16),
            ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("Pyth extension"),
            id(17),
            id(18),
            id(19),
        );
        let adapter_config =
            PythAdapterConfigV1::new([42; 32], -8, 100).expect("Pyth adapter configuration");
        let window = WindowSpecV1::new(
            id(3),
            WindowKind::ScheduledInterval,
            100,
            120,
            30,
            2,
            id(13),
        )
        .expect("shared window");
        let statistic = StatisticSpecV1::new(
            id(10),
            id(10),
            StatisticKind::OddScheduledMedian,
            RoundingBoundary::ExactRational,
            3,
            0,
            id(1),
            id(14),
            capacity,
        )
        .expect("shared median");
        SourceMaterialV1::new(
            ResolutionPolicyV1::new(id(1), id(2), id(3), id(4), id(5), id(15), None),
            id(1),
            capacity,
            id(3),
            source,
            id(9),
            provider,
            adapter_config,
            id(4),
            window,
            id(5),
            statistic,
            id(2),
            product_instance(15),
            finite_domain(),
            None,
            [None; MAX_RECOVERY_ATTEMPTS],
        )
        .expect("shared median material")
    }

    fn scheduled_normalized(
        schedule_index: u16,
        observation_unix_seconds: i64,
        atoms: i128,
    ) -> NormalizedProviderEvidenceV1 {
        NormalizedProviderEvidenceV1::new(
            id(3),
            id(9),
            id(u8::try_from(23u16 + schedule_index).expect("test evidence ID")),
            ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("Pyth extension"),
            id(13),
            schedule_index,
            observation_unix_seconds,
            observation_unix_seconds,
            atoms,
        )
    }

    #[test]
    fn product_domain_is_the_sole_exact_rational_mapping_authority() {
        let mapping = finite_domain();
        assert_eq!(
            FiniteResultDomainV1::decode(&mapping.to_bytes()),
            Ok(mapping)
        );
        assert_eq!(mapping.map(-1, 1), Ok(0));
        assert_eq!(mapping.map(0, 7), Ok(1));
        assert_eq!(mapping.map(i128::MAX, u64::MAX), Ok(1));
        let mut dirty = mapping.to_bytes();
        dirty[121] = 1;
        assert!(FiniteResultDomainV1::decode(&dirty).is_err());
        for length in 0..FINITE_RESULT_DOMAIN_BYTES {
            assert!(
                FiniteResultDomainV1::decode(mapping.to_bytes().get(..length).expect("prefix"))
                    .is_err()
            );
        }
    }

    #[test]
    fn borrowed_material_matches_by_value_and_refuses_product_aliases() {
        let (material, _, _) =
            runtime_material(None, SourceAccessProfile::PythTerminalOneTransaction);
        let bytes = material.to_bytes();
        let view = SourceMaterialViewV1::decode(&bytes).expect("borrowed material");
        assert_eq!(view.as_bytes(), bytes.as_slice());
        assert_eq!(view.policy(), Ok(material.policy()));
        assert_eq!(
            view.product_instance_id(),
            Ok(material.product_instance_id())
        );
        assert_eq!(
            view.primary_source(),
            Ok((id(3), material.primary_source()))
        );
        assert_eq!(
            view.primary_provider_release(),
            Ok(material.primary_provider_release())
        );
        assert_eq!(
            view.primary_adapter_config(),
            Ok(material.primary_adapter_config())
        );
        assert_eq!(view.capacity_profile(), Ok(material.capacity_profile()));
        assert_eq!(view.window_spec(), Ok(material.window_spec()));
        assert_eq!(view.statistic(), Ok(material.statistic()));
        assert_eq!(view.result_domain(), Ok(material.result_domain()));
        assert_eq!(view.recovery_policy(), Ok(None));
        assert_eq!(
            PythProviderAdapterObligationV1::from_material_view(view, id(3)),
            PythProviderAdapterObligationV1::from_material(material, id(3))
        );

        let mut dirty_domain = bytes;
        dirty_domain[1008 + 121] = 1;
        assert!(SourceMaterialViewV1::decode(&dirty_domain).is_err());

        let policy = material.policy();
        let (capacity_id, capacity) = material.capacity_profile();
        let (window_id, window) = material.window_spec();
        let (provider_id, provider) = material.primary_provider_release();
        assert_eq!(
            SourceMaterialV1::new(
                policy,
                capacity_id,
                capacity,
                id(3),
                material.primary_source(),
                provider_id,
                provider,
                material.primary_adapter_config(),
                window_id,
                window,
                id(5),
                material.statistic(),
                material.product_instance_id(),
                product_instance(16),
                material.result_domain(),
                None,
                [None; MAX_RECOVERY_ATTEMPTS],
            ),
            Err(Error::LinkageMismatch)
        );
    }

    #[test]
    fn borrowed_resolution_transitions_are_byte_equivalent() {
        let recovery = RecoveryPolicyV1::new(
            id(1),
            id(2),
            [
                Some(RecoveryAttemptV1::new(id(3), id(9), 120, id(20))),
                None,
                None,
                None,
            ],
            1,
            profile(),
        )
        .expect("recovery");
        let (material, _, _) = runtime_material(
            Some((id(8), recovery)),
            SourceAccessProfile::PythTerminalOneTransaction,
        );
        let material_bytes = material.to_bytes();
        let view = SourceMaterialViewV1::decode(&material_bytes).expect("borrowed material");
        let original = SourceResolutionStateV1::fresh([30; 32], 7, id(22), [31; 32], 1, 0, 0)
            .expect("state")
            .state();
        let mut by_value = original;
        let mut borrowed = original;
        assert_eq!(
            by_value.fail_next(id(22), material, id(20), 7, 111),
            borrowed.fail_next_view(id(22), view, id(20), 7, 111)
        );
        assert_eq!(borrowed, by_value);
        assert_eq!(
            by_value.exhaust(id(22), material, 7, 121),
            borrowed.exhaust_view(id(22), view, 7, 121)
        );
        assert_eq!(borrowed, by_value);
        assert_eq!(
            by_value.commit_failure(id(22), material, 7, 122, 9),
            borrowed.commit_failure_view(id(22), view, 7, 122, 9)
        );
        assert_eq!(borrowed.to_bytes(), by_value.to_bytes());
    }

    #[test]
    fn shared_in_place_transitions_are_equivalent_and_atomic() {
        let material = shared_median_material();
        let material_bytes = material.to_bytes();
        let material_view =
            SourceMaterialViewV1::decode(&material_bytes).expect("borrowed material");
        let reference_creation = SharedObservationStateV1::create(
            [30; 32],
            7,
            id(24),
            material,
            id(3),
            0,
            id(4),
            [31; 32],
            8,
            90,
            3,
            3,
        )
        .expect("reference creation");
        let mut bytes = [0xa5; SHARED_OBSERVATION_STATE_BYTES];
        let delta = create_shared_observation_state_into_v1(
            &mut bytes,
            [30; 32],
            7,
            id(24),
            material_view,
            id(3),
            0,
            id(4),
            [31; 32],
            8,
            90,
            3,
            3,
        )
        .expect("in-place creation");
        assert_eq!(delta, reference_creation.market_delta());
        assert_eq!(bytes, reference_creation.state().to_bytes());

        let mut refused_output = [0x5a; SHARED_OBSERVATION_STATE_BYTES];
        let before_refused_output = refused_output;
        assert_eq!(
            create_shared_observation_state_into_v1(
                &mut refused_output,
                [30; 32],
                7,
                id(24),
                material_view,
                id(3),
                0,
                id(4),
                [31; 32],
                8,
                90,
                3,
                2,
            ),
            Err(Error::MarketChildCountMismatch)
        );
        assert_eq!(refused_output, before_refused_output);

        let observations = [
            scheduled_normalized(0, 100, i128::MAX),
            scheduled_normalized(1, 110, -9),
            scheduled_normalized(2, 120, i128::MIN),
        ];
        let mut reference = reference_creation.state();
        let before_early_completion = bytes;
        assert_eq!(
            accept_shared_provider_output_in_place_v1(
                &mut bytes,
                id(24),
                material_view,
                Some(id(28)),
                observations[0],
                1,
                7,
                100,
            ),
            Err(Error::StateBindingMismatch)
        );
        assert_eq!(bytes, before_early_completion);

        for (index, observation) in observations.iter().copied().enumerate() {
            let sequence = u64::try_from(index + 1).expect("bounded sequence");
            let completed = (index == observations.len() - 1).then(|| id(28));
            reference
                .accept_provider_output(
                    id(24),
                    material,
                    completed,
                    observation,
                    sequence,
                    7,
                    observation.observation_unix_seconds(),
                )
                .expect("reference append");
            accept_shared_provider_output_in_place_v1(
                &mut bytes,
                id(24),
                material_view,
                completed,
                observation,
                sequence,
                7,
                observation.observation_unix_seconds(),
            )
            .expect("in-place append");
            assert_eq!(bytes, reference.to_bytes());
        }
        let accepted_view =
            SharedObservationStateViewV1::decode(&bytes).expect("accepted borrowed child");
        accepted_view
            .validate_for_resolution([30; 32], 7, id(24), id(3), id(4), id(28), &observations)
            .expect("exact retained set");
        assert_eq!(accepted_view.pda_seeds(), Ok(reference.pda_seeds()));
        assert_eq!(
            accepted_view.rent_beneficiary(),
            Ok(reference.rent_beneficiary())
        );

        let before_refused_retire = bytes;
        assert_eq!(
            retire_shared_observation_in_place_v1(&mut bytes, 7, 121, 4, 3),
            Err(Error::MarketChildCountMismatch)
        );
        assert_eq!(bytes, before_refused_retire);
        let reference_delta = reference.retire(7, 121, 4, 4).expect("reference retire");
        let raw_delta = retire_shared_observation_in_place_v1(&mut bytes, 7, 121, 4, 4)
            .expect("in-place retire");
        assert_eq!(raw_delta, reference_delta);
        assert_eq!(bytes, reference.to_bytes());
    }

    #[test]
    fn ordered_recovery_requires_deadlines_funding_and_explicit_exhaustion() {
        let recovery_id = id(8);
        let recovery = RecoveryPolicyV1::new(
            id(1),
            id(2),
            [
                Some(RecoveryAttemptV1::new(id(3), id(9), 120, id(20))),
                Some(RecoveryAttemptV1::new(id(3), id(9), 130, id(21))),
                None,
                None,
            ],
            2,
            profile(),
        )
        .expect("recovery");
        let (material, _, _) = runtime_material(
            Some((recovery_id, recovery)),
            SourceAccessProfile::PythTerminalOneTransaction,
        );
        let obligation = PythProviderAdapterObligationV1::from_material(material, id(3))
            .expect("Pyth obligation");
        assert_eq!(obligation.adapter_config_id(), id(12));
        assert_eq!(
            provider_extension_roles_v1(id(99)),
            Err(Error::UnsupportedProviderExtension)
        );
        let material_bytes = material.to_bytes();
        assert_eq!(SourceMaterialV1::decode(&material_bytes), Ok(material));
        for length in 0..SOURCE_MATERIAL_BYTES {
            assert_eq!(
                SourceMaterialV1::decode(material_bytes.get(..length).expect("material prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut dirty_material = material.to_bytes();
        dirty_material[2192 + 2 * 224] = 1;
        assert_eq!(
            SourceMaterialV1::decode(&dirty_material),
            Err(Error::NonCanonicalReservedBytes)
        );
        let creation = SourceResolutionStateV1::fresh([30; 32], 7, id(22), [31; 32], 254, 0, 0)
            .expect("state");
        assert_eq!(
            creation.market_delta().kind(),
            MarketChildDeltaKindV1::Register
        );
        assert_eq!(creation.market_delta().before(), 0);
        assert_eq!(creation.market_delta().after(), 1);
        assert_eq!(
            SourceResolutionStateV1::fresh([30; 32], 7, id(22), [31; 32], 1, 0, 1),
            Err(Error::MarketChildCountMismatch)
        );
        let mut state = creation.state();
        assert_eq!(
            SourceResolutionStateV1::decode(&state.to_bytes()),
            Ok(state)
        );
        assert_eq!(
            state.fail_next(id(22), material, id(20), 8, 111),
            Err(Error::StateBindingMismatch)
        );
        assert_eq!(
            state.commit_failure(id(22), material, 7, 111, 1),
            Err(Error::RecoveryNotExhausted)
        );
        assert_eq!(
            state.fail_next(id(22), material, id(20), 7, 110),
            Err(Error::DeadlineNotReached)
        );
        assert_eq!(
            state.fail_next(id(22), material, id(99), 7, 111),
            Err(Error::LinkageMismatch)
        );
        state
            .fail_next(id(22), material, id(20), 7, 111)
            .expect("attempt zero");
        assert_eq!(state.active_recovery_attempt(), Some(0));
        assert_eq!(
            state.exhaust(id(22), material, 7, 121),
            Err(Error::RecoveryNotExhausted)
        );
        state
            .fail_next(id(22), material, id(21), 7, 121)
            .expect("attempt one");
        assert_eq!(state.active_recovery_attempt(), Some(1));
        assert_eq!(
            state.exhaust(id(22), material, 7, 130),
            Err(Error::DeadlineNotReached)
        );
        state
            .exhaust(id(22), material, 7, 131)
            .expect("explicit exhaustion");
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Exhausted);
        let decision = state
            .commit_failure(id(22), material, 7, 132, 9)
            .expect("failure mapping");
        assert_eq!(decision.route(), SourceResolutionRouteV1::Failure);
        assert_eq!(decision.selector(), finite_domain().failure_selector());
        assert_eq!(decision.resolution_evidence_id(), id(22));
        let before_retire = state;
        assert_eq!(
            state.retire(7, 133, 1, 2),
            Err(Error::MarketChildCountMismatch)
        );
        assert_eq!(state, before_retire);
        let delta = state.retire(7, 133, 1, 1).expect("retire");
        assert_eq!(delta.kind(), MarketChildDeltaKindV1::Retire);
        assert_eq!(delta.after(), 0);
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Retired);
        assert_eq!(
            SourceResolutionStateV1::decode(&state.to_bytes()),
            Ok(state)
        );
    }

    #[test]
    fn accepted_recovery_is_derived_from_provider_evidence_not_caller_success() {
        let recovery_id = id(8);
        let recovery = RecoveryPolicyV1::new(
            id(1),
            id(2),
            [
                Some(RecoveryAttemptV1::new(id(3), id(9), 120, id(20))),
                None,
                None,
                None,
            ],
            1,
            profile(),
        )
        .expect("recovery");
        let (material, _, _) = runtime_material(
            Some((recovery_id, recovery)),
            SourceAccessProfile::PythTerminalOneTransaction,
        );
        let mut state = SourceResolutionStateV1::fresh([30; 32], 7, id(22), [31; 32], 1, 0, 0)
            .expect("state")
            .state();
        state
            .fail_next(id(22), material, id(20), 7, 111)
            .expect("recovery");
        let evidence = [normalized(5, 115)];
        assert_eq!(
            NormalizedProviderEvidenceV1::decode(&evidence[0].to_bytes()),
            Ok(evidence[0])
        );
        let decision = state
            .accept_provider_output(
                id(22),
                material,
                id(23),
                &evidence,
                None,
                Some(id(20)),
                7,
                115,
                4,
            )
            .expect("accepted");
        assert_eq!(decision.route(), SourceResolutionRouteV1::Recovery);
        assert_eq!(decision.selector(), 1);
        assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
        assert_eq!(
            state.accept_provider_output(
                id(22),
                material,
                id(23),
                &evidence,
                None,
                Some(id(20)),
                7,
                115,
                4,
            ),
            Err(Error::InvalidRecoveryTransition)
        );
    }

    #[test]
    fn pyth_normalization_uses_authenticated_update_and_shared_digest_is_canonical() {
        let (material, _, _) =
            runtime_material(None, SourceAccessProfile::PythTerminalOneTransaction);
        let obligation = PythProviderAdapterObligationV1::from_material(material, id(3))
            .expect("Pyth obligation");
        let accepted = obligation
            .normalize_authenticated_update(id(23), id(13), 0, [42; 32], -1_000_000, 5_000, -8, 100)
            .expect("configured update");
        assert_eq!(accepted.atoms(), -1_000_000);
        assert_eq!(accepted.observation_unix_seconds(), 100);
        assert_eq!(accepted.publication_unix_seconds(), 100);
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(23),
                id(13),
                0,
                [41; 32],
                -1_000_000,
                5_000,
                -8,
                100,
            ),
            Err(Error::InvalidPythObservation)
        );
        assert_eq!(
            obligation.normalize_authenticated_update(
                id(23),
                id(13),
                0,
                [42; 32],
                -1_000_000,
                5_000,
                -9,
                100,
            ),
            Err(Error::InvalidPythObservation)
        );
        assert_eq!(
            obligation.normalize_authenticated_update(id(23), id(13), 0, [42; 32], 1, 2, -8, 100,),
            Err(Error::InvalidPythObservation)
        );

        let observation = normalized(5, 100);
        let length = shared_evidence_set_preimage_len_v1(1).expect("one observation");
        let mut bytes = [0u8; SHARED_EVIDENCE_SET_HEADER_BYTES_V1 + NORMALIZED_EVIDENCE_BYTES];
        assert_eq!(bytes.len(), length);
        encode_shared_evidence_set_preimage_v1(
            id(22),
            id(3),
            id(9),
            id(13),
            &[observation],
            &mut bytes,
        )
        .expect("canonical set");
        assert_eq!(bytes.get(176..), Some(observation.to_bytes().as_slice()));
        assert_eq!(
            encode_shared_evidence_set_preimage_v1(
                id(22),
                id(4),
                id(9),
                id(13),
                &[observation],
                &mut bytes,
            ),
            Err(Error::StateBindingMismatch)
        );
        assert_eq!(
            shared_evidence_set_preimage_len_v1(0),
            Err(Error::InvalidSharedObservation)
        );
    }

    #[test]
    fn shared_observation_exists_only_for_selected_profile_and_is_replay_safe() {
        let (material, _, _) = runtime_material(None, SourceAccessProfile::SharedObservationChild);
        assert_eq!(
            SharedObservationStateV1::create(
                [30; 32],
                7,
                id(22),
                material,
                id(3),
                4,
                id(4),
                [31; 32],
                9,
                90,
                0,
                0,
            ),
            Err(Error::SharedChildrenExceedCapacity)
        );
        let mut child = SharedObservationStateV1::create(
            [30; 32],
            7,
            id(22),
            material,
            id(3),
            0,
            id(4),
            [31; 32],
            9,
            90,
            0,
            0,
        )
        .expect("child")
        .state();
        assert_eq!(
            SharedObservationStateV1::decode(&child.to_bytes()),
            Ok(child)
        );
        let evidence = [normalized(5, 105)];
        child
            .accept_provider_output(id(22), material, Some(id(23)), evidence[0], 1, 7, 105)
            .expect("accept child");
        assert_eq!(
            child.accept_provider_output(id(22), material, Some(id(23)), evidence[0], 2, 7, 105,),
            Err(Error::InvalidSharedObservation)
        );
        child
            .validate_for_resolution([30; 32], 7, id(22), id(3), id(4), id(23), &evidence)
            .expect("reusable accepted child");
        assert_eq!(
            child.validate_for_resolution([30; 32], 7, id(24), id(3), id(4), id(23), &evidence,),
            Err(Error::InvalidSharedObservation)
        );
        child.retire(7, 106, 1, 1).expect("retire child");
        assert_eq!(child.phase(), SharedObservationPhaseV1::Retired);

        let median_material = shared_median_material();
        let median_creation = SharedObservationStateV1::create(
            [30; 32],
            7,
            id(24),
            median_material,
            id(3),
            0,
            id(4),
            [31; 32],
            8,
            90,
            3,
            3,
        )
        .expect("progressive child");
        assert_eq!(median_creation.market_delta().before(), 3);
        assert_eq!(median_creation.market_delta().after(), 4);
        let mut median_child = median_creation.state();
        let observations = [
            scheduled_normalized(0, 100, i128::MAX),
            scheduled_normalized(1, 110, -9),
            scheduled_normalized(2, 120, i128::MIN),
        ];
        assert_eq!(
            median_child.accept_provider_output(
                id(24),
                median_material,
                Some(id(28)),
                observations[0],
                1,
                7,
                100,
            ),
            Err(Error::StateBindingMismatch)
        );
        assert_eq!(median_child.observation_count(), 0);
        median_child
            .accept_provider_output(id(24), median_material, None, observations[0], 1, 7, 100)
            .expect("first append");
        assert_eq!(median_child.phase(), SharedObservationPhaseV1::Collecting);
        assert_eq!(
            median_child.accept_provider_output(
                id(24),
                median_material,
                None,
                observations[0],
                2,
                7,
                110,
            ),
            Err(Error::LinkageMismatch)
        );
        median_child
            .accept_provider_output(id(24), median_material, None, observations[1], 2, 7, 110)
            .expect("second append");
        median_child
            .accept_provider_output(
                id(24),
                median_material,
                Some(id(28)),
                observations[2],
                3,
                7,
                120,
            )
            .expect("completing append");
        assert_eq!(median_child.phase(), SharedObservationPhaseV1::Accepted);
        assert_eq!(median_child.observation(1), Ok(observations[1]));
        median_child
            .validate_for_resolution([30; 32], 7, id(24), id(3), id(4), id(28), &observations)
            .expect("exact progressive observations");
        assert_eq!(
            SharedObservationStateV1::decode(&median_child.to_bytes()),
            Ok(median_child)
        );
        let median_delta = median_child.retire(7, 121, 4, 4).expect("median retire");
        assert_eq!(median_delta.before(), 4);
        assert_eq!(median_delta.after(), 3);

        let (inline_material, _, _) =
            runtime_material(None, SourceAccessProfile::PythTerminalOneTransaction);
        assert_eq!(
            SharedObservationStateV1::create(
                [30; 32],
                7,
                id(22),
                inline_material,
                id(3),
                0,
                id(4),
                [31; 32],
                1,
                90,
                0,
                0,
            ),
            Err(Error::WrongSourceAccessProfile)
        );
    }

    #[test]
    fn reopen_link_and_pda_seeds_bind_exactly_next_generation() {
        let link = ReopenLinkV1::new([30; 32], id(40), 7, 8, id(41)).expect("link");
        assert_eq!(ReopenLinkV1::decode(&link.to_bytes()), Ok(link));
        assert_eq!(
            ReopenLinkV1::new([30; 32], id(40), 7, 9, id(41)),
            Err(Error::InvalidReopenLink)
        );
        let state = SourceResolutionStateV1::reopened(
            [30; 32],
            8,
            id(22),
            [31; 32],
            250,
            id(42),
            link,
            0,
            0,
        )
        .expect("reopened")
        .state();
        assert_eq!(state.reopen_link_id(), Some(id(42)));
        assert_eq!(
            state.pda_seeds().domain(),
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1
        );
        assert_eq!(state.pda_seeds().generation_le(), 8u64.to_le_bytes());
        assert_eq!(state.pda_seeds().bump(), 250);
    }

    #[test]
    fn odd_scheduled_median_is_exact_allocation_free_and_hostile_to_bad_schedule() {
        let median = StatisticSpecV1::new(
            id(3),
            id(3),
            StatisticKind::OddScheduledMedian,
            RoundingBoundary::ExactRational,
            5,
            0,
            id(9),
            id(4),
            profile(),
        )
        .expect("median");
        let median_window =
            WindowSpecV1::new(id(5), WindowKind::ScheduledInterval, 0, 40, 5, 1, id(6))
                .expect("window");
        let samples = [
            Observation {
                atoms: i128::MAX,
                unix_seconds: 0,
            },
            Observation {
                atoms: i128::MIN,
                unix_seconds: 10,
            },
            Observation {
                atoms: 5,
                unix_seconds: 20,
            },
            Observation {
                atoms: 4,
                unix_seconds: 30,
            },
            Observation {
                atoms: 3,
                unix_seconds: 40,
            },
        ];
        assert_eq!(
            evaluate(median, median_window, &samples),
            Ok(StatisticValue::ExactRational {
                numerator: 4,
                denominator: 1
            })
        );
        for count in [1, 2, 4] {
            assert_eq!(
                StatisticSpecV1::new(
                    id(3),
                    id(3),
                    StatisticKind::OddScheduledMedian,
                    RoundingBoundary::ExactRational,
                    count,
                    0,
                    id(9),
                    id(4),
                    profile(),
                ),
                Err(Error::NonCanonicalStatistic)
            );
        }
        let terminal =
            WindowSpecV1::new(id(5), WindowKind::Terminal, 0, 0, 5, 1, id(6)).expect("terminal");
        assert_eq!(
            evaluate(median, terminal, &samples),
            Err(Error::NonCanonicalStatistic)
        );
        let mut wrong_cadence = samples;
        wrong_cadence[2].unix_seconds = 21;
        assert_eq!(
            evaluate(median, median_window, &wrong_cadence),
            Err(Error::InvalidObservationSchedule)
        );
        let mut wrong_order = samples;
        wrong_order[1].unix_seconds = 30;
        assert_eq!(
            evaluate(median, median_window, &wrong_order),
            Err(Error::InvalidObservationSchedule)
        );
        let mut duplicate = samples;
        duplicate[2].unix_seconds = 10;
        assert_eq!(
            evaluate(median, median_window, &duplicate),
            Err(Error::InvalidObservationSchedule)
        );
    }

    #[test]
    fn instruction_wires_and_frames_are_exact_and_hostile_decoded() {
        let create = CreateResolutionInstructionV1::new([30; 32], 7, id(22), [31; 32], 4, 9, None)
            .expect("create");
        assert_eq!(create.expected_market_child_count(), 4);
        let bytes = create.to_bytes();
        assert_eq!(
            SourceInstructionV1::decode(&bytes),
            Ok(SourceInstructionV1::CreateResolution(create))
        );
        let reopen_link = ReopenLinkV1::new([30; 32], id(40), 6, 7, id(41)).expect("reopen link");
        let reopen = CreateResolutionInstructionV1::new(
            [30; 32],
            7,
            id(22),
            [31; 32],
            4,
            9,
            Some(reopen_link),
        )
        .expect("reopen create");
        assert_eq!(
            SourceInstructionV1::decode(&reopen.to_bytes()),
            Ok(SourceInstructionV1::CreateResolution(reopen))
        );
        for length in 0..CREATE_RESOLUTION_INSTRUCTION_BYTES {
            assert_eq!(
                SourceInstructionV1::decode(bytes.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut dirty = bytes;
        dirty[287] = 1;
        assert_eq!(
            SourceInstructionV1::decode(&dirty),
            Err(Error::NonCanonicalReservedBytes)
        );
        let accept = AcceptEvidenceInstructionV1::new(7, 3).expect("accept");
        let mut accept_bytes = [0u8; ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES + 2];
        accept_bytes[..ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES]
            .copy_from_slice(&accept.to_prefix_bytes());
        accept_bytes[ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES..].copy_from_slice(&[7, 8]);
        assert_eq!(
            SourceInstructionV1::decode(&accept_bytes),
            Ok(SourceInstructionV1::AcceptEvidence(accept, &[7, 8]))
        );
        let commit = CommitFailureInstructionV1::new(7, 4).expect("commit");
        assert_eq!(
            SourceInstructionV1::decode(&commit.to_bytes()),
            Ok(SourceInstructionV1::CommitFailure(commit))
        );
        let retire =
            RetireInstructionV1::new(SourceActionV1::RetireResolution, 7, 4).expect("retire");
        assert_eq!(
            SourceInstructionV1::decode(&retire.to_bytes()),
            Ok(SourceInstructionV1::RetireResolution(retire))
        );
        for (action, expected) in [
            (
                SourceActionV1::FailNext,
                SourceInstructionV1::FailNext(
                    GenerationInstructionV1::new(SourceActionV1::FailNext, 7).expect("fail next"),
                ),
            ),
            (
                SourceActionV1::Exhaust,
                SourceInstructionV1::Exhaust(
                    GenerationInstructionV1::new(SourceActionV1::Exhaust, 7).expect("exhaust"),
                ),
            ),
        ] {
            let generation = GenerationInstructionV1::new(action, 7).expect("generation wire");
            assert_eq!(
                SourceInstructionV1::decode(&generation.to_bytes()),
                Ok(expected)
            );
        }
        let retire_shared = RetireInstructionV1::new(SourceActionV1::RetireSharedObservation, 7, 3)
            .expect("retire shared");
        assert_eq!(
            SourceInstructionV1::decode(&retire_shared.to_bytes()),
            Ok(SourceInstructionV1::RetireSharedObservation(retire_shared))
        );
        let create_shared = CreateSharedObservationInstructionV1::new(
            [30; 32],
            7,
            id(22),
            id(3),
            id(4),
            [31; 32],
            3,
            8,
        )
        .expect("create shared");
        assert_eq!(
            SourceInstructionV1::decode(&create_shared.to_bytes()),
            Ok(SourceInstructionV1::CreateSharedObservation(create_shared))
        );
        let accept_shared =
            AcceptSharedObservationInstructionV1::new(7, 2, Some(id(23))).expect("accept shared");
        let mut accept_shared_bytes = [0u8; ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES + 1];
        accept_shared_bytes[..ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES]
            .copy_from_slice(&accept_shared.to_prefix_bytes());
        accept_shared_bytes[ACCEPT_SHARED_OBSERVATION_INSTRUCTION_PREFIX_BYTES] = 9;
        assert_eq!(
            SourceInstructionV1::decode(&accept_shared_bytes),
            Ok(SourceInstructionV1::AcceptSharedObservation(
                accept_shared,
                &[9]
            ))
        );
        let mut unknown = retire.to_bytes();
        unknown[10] = 99;
        assert_eq!(
            SourceInstructionV1::decode(&unknown),
            Err(Error::UnknownInstructionAction)
        );

        let roles = source_frame_roles_v1(SourceFrameKindV1::AcceptRecoveryShared);
        let mut accounts = [SourceAccountPrivilegeV1 {
            key: [1; 32],
            is_signer: false,
            is_writable: false,
            is_executable: false,
        }; ACCEPT_RECOVERY_SHARED_FRAME_V1.len()];
        for (index, (account, role)) in accounts.iter_mut().zip(roles).enumerate() {
            account.key = [u8::try_from(index + 1).expect("bounded"); 32];
            account.is_signer = role.is_signer();
            account.is_writable = role.is_writable();
            account.is_executable = role.is_executable();
        }
        assert_eq!(
            validate_source_frame_v1(SourceFrameKindV1::AcceptRecoveryShared, &accounts),
            Ok(())
        );
        accounts[0].is_writable = false;
        assert_eq!(
            validate_source_frame_v1(SourceFrameKindV1::AcceptRecoveryShared, &accounts),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn closed_release_ids_are_sha256_of_their_exact_preimages() {
        let releases = [
            (
                SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1,
                SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
            ),
            (
                SOURCE_MATERIAL_DERIVATION_RELEASE_PREIMAGE_V1,
                SOURCE_MATERIAL_DERIVATION_RELEASE_ID_V1,
            ),
            (
                PYTH_PROVIDER_EXTENSION_RELEASE_PREIMAGE_V1,
                PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
            ),
            (
                SHARED_EVIDENCE_SET_RELEASE_PREIMAGE_V1,
                SHARED_EVIDENCE_SET_RELEASE_ID_V1,
            ),
            (
                SOURCE_STATE_SCHEMA_RELEASE_PREIMAGE_V1,
                SOURCE_STATE_SCHEMA_RELEASE_ID_V1,
            ),
            (
                SOURCE_STATE_DERIVATION_RELEASE_PREIMAGE_V1,
                SOURCE_STATE_DERIVATION_RELEASE_ID_V1,
            ),
            (
                SHARED_OBSERVATION_SCHEMA_RELEASE_PREIMAGE_V1,
                SHARED_OBSERVATION_SCHEMA_RELEASE_ID_V1,
            ),
            (
                SHARED_OBSERVATION_DERIVATION_RELEASE_PREIMAGE_V1,
                SHARED_OBSERVATION_DERIVATION_RELEASE_ID_V1,
            ),
            (
                REOPEN_LINK_SCHEMA_RELEASE_PREIMAGE_V1,
                REOPEN_LINK_SCHEMA_RELEASE_ID_V1,
            ),
        ];
        for (preimage, expected) in releases {
            assert_eq!(sha256_one_block(preimage), expected);
        }
    }

    #[allow(clippy::indexing_slicing, clippy::needless_range_loop)]
    fn sha256_one_block(input: &[u8]) -> [u8; 32] {
        assert!(input.len() < 56);
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut block = [0u8; 64];
        block[..input.len()].copy_from_slice(input);
        block[input.len()] = 0x80;
        let bit_length = u64::try_from(input.len()).expect("short input") * 8;
        block[56..64].copy_from_slice(&bit_length.to_be_bytes());
        let mut words = [0u32; 64];
        for index in 0..16 {
            let offset = index * 4;
            words[index] = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut state = [
            0x6a09e667u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        );
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
        let mut output = [0u8; 32];
        for (index, value) in state.iter().enumerate() {
            let offset = index * 4;
            output[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        output
    }
}
