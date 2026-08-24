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

/// Exact width of an opaque nonzero content identity.
pub const CONTENT_ID_BYTES: usize = 32;
/// Exact width of a provider-release preimage.
pub const PROVIDER_RELEASE_BYTES: usize = 128;
/// Exact width of a source-capacity profile preimage.
pub const SOURCE_CAPACITY_PROFILE_BYTES: usize = 112;
/// Exact width of a source-specification preimage.
pub const SOURCE_SPEC_BYTES: usize = 192;
/// Exact width of a window-specification preimage.
pub const WINDOW_SPEC_BYTES: usize = 112;
/// Exact width of a statistic-specification preimage.
pub const STATISTIC_SPEC_BYTES: usize = 176;
/// Exact width of a result-mapping preimage.
pub const RESULT_MAPPING_BYTES: usize = 144;
/// Exact width of a resolution-policy preimage.
pub const RESOLUTION_POLICY_BYTES: usize = 224;
/// Maximum attempts in the V1 recovery artifact profile.
pub const MAX_RECOVERY_ATTEMPTS: usize = 4;
/// Exact width of one immutable source-funding quote reference.
pub const FUNDING_QUOTE_REF_BYTES: usize = 96;
/// Exact width of one fixed recovery-attempt slot.
pub const RECOVERY_ATTEMPT_BYTES: usize = 176;
/// Exact width of a recovery-policy preimage.
pub const RECOVERY_POLICY_BYTES: usize = 800;

/// Canonical provider-release magic.
pub const PROVIDER_RELEASE_MAGIC: [u8; 8] = *b"DCLTPRV1";
/// Canonical capacity-profile magic.
pub const SOURCE_CAPACITY_PROFILE_MAGIC: [u8; 8] = *b"DCLTSCP1";
/// Canonical source-specification magic.
pub const SOURCE_SPEC_MAGIC: [u8; 8] = *b"DCLTSRC1";
/// Canonical window-specification magic.
pub const WINDOW_SPEC_MAGIC: [u8; 8] = *b"DCLTWIN1";
/// Canonical statistic-specification magic.
pub const STATISTIC_SPEC_MAGIC: [u8; 8] = *b"DCLTSTA1";
/// Canonical result-mapping magic.
pub const RESULT_MAPPING_MAGIC: [u8; 8] = *b"DCLTRMP1";
/// Canonical resolution-policy magic.
pub const RESOLUTION_POLICY_MAGIC: [u8; 8] = *b"DCLTRSP1";
/// Canonical recovery-policy magic.
pub const RECOVERY_POLICY_MAGIC: [u8; 8] = *b"DCLTRCV1";
/// Implemented schema release for all V1 records.
pub const SCHEMA_VERSION: u16 = 1;

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
    /// A byte did not name a defined funding quote representation.
    UnknownFundingQuote,
    /// Exact funding compartments did not sum to their encoded total.
    FundingTotalMismatch,
    /// A recovery policy exceeded its capacity profile's fixed attempt bound.
    RecoveryExceedsCapacity,
    /// Recovery attempts were not in strictly increasing deadline order.
    NonCanonicalRecoveryOrder,
    /// A recovery attempt omitted a prepaid funding reference.
    MissingPrepaidFunding,
    /// Linked preimages did not bind the same occurrence or dependency ID.
    LinkageMismatch,
    /// A recovery status transition was not admitted.
    InvalidRecoveryTransition,
    /// Terminal failure was requested before every committed attempt exhausted.
    RecoveryNotExhausted,
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
    decoding_rules_id: ContentId,
    transport_profile_id: ContentId,
}

impl ProviderReleaseV1 {
    /// Construct one provider release from four immutable identities.
    pub const fn new(
        provider_family_id: ContentId,
        adapter_release_id: ContentId,
        decoding_rules_id: ContentId,
        transport_profile_id: ContentId,
    ) -> Self {
        Self {
            provider_family_id,
            adapter_release_id,
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
        ))
    }

    /// Encode exact canonical provider-release bytes.
    pub fn to_bytes(self) -> [u8; PROVIDER_RELEASE_BYTES] {
        let mut out = base::<PROVIDER_RELEASE_BYTES>(PROVIDER_RELEASE_MAGIC);
        put(&mut out, 16, self.provider_family_id.as_bytes());
        put(&mut out, 48, self.adapter_release_id.as_bytes());
        put(&mut out, 80, self.decoding_rules_id.as_bytes());
        put(&mut out, 112, self.transport_profile_id.as_bytes());
        out
    }

    /// Return the parser/normalizer release identity.
    pub const fn adapter_release_id(self) -> ContentId {
        self.adapter_release_id
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
        zero(bytes, 160, 16)?;
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
    };
    finalize(raw, 1, spec.rounding)
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

/// Product-owned mapping from exact source result to one finite claim outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultMappingV1 {
    occurrence_id: ContentId,
    result_domain_id: ContentId,
    mapping_release_id: ContentId,
    mapping_artifact_id: ContentId,
}

impl ResultMappingV1 {
    /// Construct an immutable Product mapping without provider transport fields.
    pub const fn new(
        occurrence_id: ContentId,
        result_domain_id: ContentId,
        mapping_release_id: ContentId,
        mapping_artifact_id: ContentId,
    ) -> Self {
        Self {
            occurrence_id,
            result_domain_id,
            mapping_release_id,
            mapping_artifact_id,
        }
    }
    /// Decode one exact hostile result mapping.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, RESULT_MAPPING_BYTES, RESULT_MAPPING_MAGIC)?;
        zero(bytes, 10, 6)?;
        Ok(Self::new(
            content(bytes, 16)?,
            content(bytes, 48)?,
            content(bytes, 80)?,
            content(bytes, 112)?,
        ))
    }
    /// Encode exact canonical result-mapping bytes.
    pub fn to_bytes(self) -> [u8; RESULT_MAPPING_BYTES] {
        let mut out = base::<RESULT_MAPPING_BYTES>(RESULT_MAPPING_MAGIC);
        put(&mut out, 16, self.occurrence_id.as_bytes());
        put(&mut out, 48, self.result_domain_id.as_bytes());
        put(&mut out, 80, self.mapping_release_id.as_bytes());
        put(&mut out, 112, self.mapping_artifact_id.as_bytes());
        out
    }
    /// Check Product occurrence linkage.
    pub fn validate_occurrence(self, occurrence_id: ContentId) -> Result<()> {
        if self.occurrence_id == occurrence_id {
            Ok(())
        } else {
            Err(Error::LinkageMismatch)
        }
    }
}

/// Immutable policy binding one Product occurrence to source resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionPolicyV1 {
    capacity_profile_id: ContentId,
    occurrence_id: ContentId,
    source_spec_id: ContentId,
    window_spec_id: ContentId,
    statistic_spec_id: ContentId,
    result_mapping_id: ContentId,
    recovery_policy_id: Option<ContentId>,
}

impl ResolutionPolicyV1 {
    /// Construct the single Product-to-source truth linkage.
    pub const fn new(
        capacity_profile_id: ContentId,
        occurrence_id: ContentId,
        source_spec_id: ContentId,
        window_spec_id: ContentId,
        statistic_spec_id: ContentId,
        result_mapping_id: ContentId,
        recovery_policy_id: Option<ContentId>,
    ) -> Self {
        Self {
            capacity_profile_id,
            occurrence_id,
            source_spec_id,
            window_spec_id,
            statistic_spec_id,
            result_mapping_id,
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
        put(&mut out, 48, self.occurrence_id.as_bytes());
        put(&mut out, 80, self.source_spec_id.as_bytes());
        put(&mut out, 112, self.window_spec_id.as_bytes());
        put(&mut out, 144, self.statistic_spec_id.as_bytes());
        put(&mut out, 176, self.result_mapping_id.as_bytes());
        if let Some(id) = self.recovery_policy_id {
            put(&mut out, 208, id.as_bytes());
        }
        out
    }
    /// Validate policy linkage to all supplied authenticated dependencies.
    pub fn validate_links(
        self,
        occurrence_id: ContentId,
        capacity_profile_id: ContentId,
        source_id: ContentId,
        window_id: ContentId,
        statistic_id: ContentId,
        result_id: ContentId,
    ) -> Result<()> {
        if self.occurrence_id != occurrence_id
            || self.capacity_profile_id != capacity_profile_id
            || self.source_spec_id != source_id
            || self.window_spec_id != window_id
            || self.statistic_spec_id != statistic_id
            || self.result_mapping_id != result_id
        {
            Err(Error::LinkageMismatch)
        } else {
            Ok(())
        }
    }

    /// Validate source/window/statistic/result semantics after their IDs have
    /// been authenticated by the composing hash boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_material(
        self,
        source_id: ContentId,
        source: SourceSpecV1,
        window_id: ContentId,
        window: WindowSpecV1,
        statistic_id: ContentId,
        statistic: StatisticSpecV1,
        result_id: ContentId,
        result: ResultMappingV1,
    ) -> Result<()> {
        self.validate_links(
            self.occurrence_id,
            self.capacity_profile_id,
            source_id,
            window_id,
            statistic_id,
            result_id,
        )?;
        window.validate_source(source_id)?;
        result.validate_occurrence(self.occurrence_id)?;
        if source.capacity_profile_id != self.capacity_profile_id
            || statistic.capacity_profile_id != self.capacity_profile_id
            || statistic.source_unit_id != source.unit_id
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
}

/// A reference to either a content-addressed capability funding quote or exact compatible compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingQuoteRefV1 {
    /// Opaque identity of a separately authenticated dclutch capability quote.
    ContentAddressed(ContentId),
    /// Exact compatible present-principal compartments, never Hoard or future fees.
    ExactCompartments {
        /// Rent principal.
        rent_principal: u64,
        /// Creation principal.
        creation_principal: u64,
        /// Work principal.
        work_principal: u64,
        /// Provider principal.
        provider_principal: u64,
        /// Bounty principal.
        bounty_principal: u64,
        /// Liquidity principal.
        liquidity_principal: u64,
        /// Service principal.
        service_principal: u64,
        /// Canonical compartment sum.
        total_principal: u64,
    },
}

impl FundingQuoteRefV1 {
    /// Construct and check exact compatible present-principal compartments.
    #[allow(clippy::too_many_arguments)]
    pub fn compartments(
        rent_principal: u64,
        creation_principal: u64,
        work_principal: u64,
        provider_principal: u64,
        bounty_principal: u64,
        liquidity_principal: u64,
        service_principal: u64,
    ) -> Result<Self> {
        let total_principal = sum_u64([
            rent_principal,
            creation_principal,
            work_principal,
            provider_principal,
            bounty_principal,
            liquidity_principal,
            service_principal,
        ])?;
        if total_principal == 0 {
            return Err(Error::MissingPrepaidFunding);
        }
        Ok(Self::ExactCompartments {
            rent_principal,
            creation_principal,
            work_principal,
            provider_principal,
            bounty_principal,
            liquidity_principal,
            service_principal,
            total_principal,
        })
    }
    /// Decode one exact canonical quote reference.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_QUOTE_REF_BYTES {
            return Err(Error::InvalidLength);
        }
        match one(bytes, 0)? {
            1 => {
                zero(bytes, 1, 31)?;
                zero(bytes, 64, 32)?;
                Ok(Self::ContentAddressed(content(bytes, 32)?))
            }
            2 => {
                zero(bytes, 1, 31)?;
                let result = Self::compartments(
                    read_u64(bytes, 32)?,
                    read_u64(bytes, 40)?,
                    read_u64(bytes, 48)?,
                    read_u64(bytes, 56)?,
                    read_u64(bytes, 64)?,
                    read_u64(bytes, 72)?,
                    read_u64(bytes, 80)?,
                )?;
                if let Self::ExactCompartments {
                    total_principal, ..
                } = result
                    && total_principal != read_u64(bytes, 88)?
                {
                    return Err(Error::FundingTotalMismatch);
                }
                Ok(result)
            }
            _ => Err(Error::UnknownFundingQuote),
        }
    }
    /// Encode exact canonical funding quote reference bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_QUOTE_REF_BYTES] {
        let mut out = [0u8; FUNDING_QUOTE_REF_BYTES];
        match self {
            Self::ContentAddressed(id) => {
                put(&mut out, 0, &[1]);
                put(&mut out, 32, id.as_bytes());
            }
            Self::ExactCompartments {
                rent_principal,
                creation_principal,
                work_principal,
                provider_principal,
                bounty_principal,
                liquidity_principal,
                service_principal,
                total_principal,
            } => {
                put(&mut out, 0, &[2]);
                put(&mut out, 32, &rent_principal.to_le_bytes());
                put(&mut out, 40, &creation_principal.to_le_bytes());
                put(&mut out, 48, &work_principal.to_le_bytes());
                put(&mut out, 56, &provider_principal.to_le_bytes());
                put(&mut out, 64, &bounty_principal.to_le_bytes());
                put(&mut out, 72, &liquidity_principal.to_le_bytes());
                put(&mut out, 80, &service_principal.to_le_bytes());
                put(&mut out, 88, &total_principal.to_le_bytes());
            }
        }
        out
    }
}

/// Immutable one-attempt recovery record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryAttemptV1 {
    source_spec_id: ContentId,
    provider_release_id: ContentId,
    deadline_unix_seconds: i64,
    funding_quote: FundingQuoteRefV1,
}

impl RecoveryAttemptV1 {
    /// Construct a prepaid attempt.  The quote is immutable and segregated.
    pub const fn new(
        source_spec_id: ContentId,
        provider_release_id: ContentId,
        deadline_unix_seconds: i64,
        funding_quote: FundingQuoteRefV1,
    ) -> Self {
        Self {
            source_spec_id,
            provider_release_id,
            deadline_unix_seconds,
            funding_quote,
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
            FundingQuoteRefV1::decode(bytes.get(80..176).ok_or(Error::InvalidLength)?)?,
        ))
    }
    fn to_slot_bytes(self) -> [u8; RECOVERY_ATTEMPT_BYTES] {
        let mut out = [0u8; RECOVERY_ATTEMPT_BYTES];
        put(&mut out, 0, self.source_spec_id.as_bytes());
        put(&mut out, 32, self.provider_release_id.as_bytes());
        put(&mut out, 64, &self.deadline_unix_seconds.to_le_bytes());
        put(&mut out, 80, &self.funding_quote.to_bytes());
        out
    }
}

/// Finite ordered recovery plan; failure is legal only after every attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryPolicyV1 {
    capacity_profile_id: ContentId,
    occurrence_id: ContentId,
    attempts: [Option<RecoveryAttemptV1>; MAX_RECOVERY_ATTEMPTS],
    attempt_count: u8,
}

impl RecoveryPolicyV1 {
    /// Construct an ordered finite recovery plan under its authenticated profile.
    pub fn new(
        capacity_profile_id: ContentId,
        occurrence_id: ContentId,
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
            occurrence_id,
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
        zero(bytes, 784, 16)?;
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
            occurrence_id: content(bytes, 48)?,
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
        put(&mut out, 48, self.occurrence_id.as_bytes());
        for (index, attempt) in self.attempts.iter().enumerate() {
            if let Some(value) = attempt {
                let offset = 80usize.saturating_add(index.saturating_mul(RECOVERY_ATTEMPT_BYTES));
                put(&mut out, offset, &value.to_slot_bytes());
            }
        }
        out
    }
    /// Recheck capacity and Product occurrence linkage.
    pub fn validate_capacity(
        self,
        capacity_profile_id: ContentId,
        occurrence_id: ContentId,
        profile: SourceCapacityProfileV1,
    ) -> Result<()> {
        if self.capacity_profile_id != capacity_profile_id || self.occurrence_id != occurrence_id {
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
}

/// Persistent recovery execution phase with no early terminal failure edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryState {
    /// No recovery attempt has been exhausted.
    Pending {
        /// Zero-based next attempt index.
        next_attempt: u8,
    },
    /// All attempts exhausted and terminal failure is now admitted.
    Exhausted,
    /// A success result was accepted.
    Resolved,
}

impl RecoveryState {
    /// Mark exactly the next attempt exhausted; only the final one reaches `Exhausted`.
    pub fn exhaust_next(self, policy: RecoveryPolicyV1) -> Result<Self> {
        match self {
            Self::Pending { next_attempt } if next_attempt < policy.attempt_count => {
                let after = next_attempt
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
                if after == policy.attempt_count {
                    Ok(Self::Exhausted)
                } else {
                    Ok(Self::Pending {
                        next_attempt: after,
                    })
                }
            }
            _ => Err(Error::InvalidRecoveryTransition),
        }
    }
    /// Admit terminal failure only after the finite policy exhausted.
    pub fn terminal_failure(self) -> Result<()> {
        if self == Self::Exhausted {
            Ok(())
        } else {
            Err(Error::RecoveryNotExhausted)
        }
    }
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
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}
fn sum_u64(values: [u64; 7]) -> Result<u64> {
    let mut total = 0u64;
    for value in values {
        total = total.checked_add(value).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(fill: u8) -> ContentId {
        ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test identity")
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
        let quote = FundingQuoteRefV1::compartments(1, 2, 3, 4, 5, 6, 7).expect("sum");
        let attempts = [
            Some(RecoveryAttemptV1::new(id(7), id(8), 30, quote)),
            Some(RecoveryAttemptV1::new(id(7), id(8), 40, quote)),
            None,
            None,
        ];
        let recovery =
            RecoveryPolicyV1::new(id(9), id(10), attempts, 2, profile()).expect("valid recovery");
        assert_eq!(RecoveryPolicyV1::decode(&recovery.to_bytes()), Ok(recovery));
        assert_eq!(
            RecoveryState::Pending { next_attempt: 0 }.terminal_failure(),
            Err(Error::RecoveryNotExhausted)
        );
        let first = RecoveryState::Pending { next_attempt: 0 }
            .exhaust_next(recovery)
            .expect("first");
        assert_eq!(first.terminal_failure(), Err(Error::RecoveryNotExhausted));
        let final_state = first.exhaust_next(recovery).expect("last");
        assert_eq!(final_state.terminal_failure(), Ok(()));
        let unordered = [
            Some(RecoveryAttemptV1::new(id(7), id(8), 40, quote)),
            Some(RecoveryAttemptV1::new(id(7), id(8), 30, quote)),
            None,
            None,
        ];
        assert_eq!(
            RecoveryPolicyV1::new(id(9), id(10), unordered, 2, profile()),
            Err(Error::NonCanonicalRecoveryOrder)
        );
    }

    #[test]
    fn quotes_are_canonical_and_policy_binds_every_link() {
        let quote = FundingQuoteRefV1::compartments(1, 2, 3, 4, 5, 6, 7).expect("quote");
        assert_eq!(FundingQuoteRefV1::decode(&quote.to_bytes()), Ok(quote));
        let mut altered = quote.to_bytes();
        if let Some(slot) = altered.get_mut(88) {
            *slot ^= 1;
        };
        assert_eq!(
            FundingQuoteRefV1::decode(&altered),
            Err(Error::FundingTotalMismatch)
        );
        assert_eq!(
            FundingQuoteRefV1::compartments(0, 0, 0, 0, 0, 0, 0),
            Err(Error::MissingPrepaidFunding)
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
        let result = ResultMappingV1::new(id(8), id(2), id(9), id(10));
        let policy = ResolutionPolicyV1::new(id(1), id(8), id(11), id(12), id(13), id(14), None);
        assert_eq!(
            policy.validate_material(
                id(11),
                source,
                id(12),
                scheduled,
                id(13),
                stat,
                id(14),
                result
            ),
            Err(Error::NonCanonicalSourceProfile)
        );
    }
}
