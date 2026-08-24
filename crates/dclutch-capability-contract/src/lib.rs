#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free canonical contracts for immutable Market capability manifests and
//! their separately mutable, segregated funding state.
//!
//! A composing hash policy hashes [`CapabilityManifestV1::as_bytes`]. The
//! resulting nonzero [`ContentId`] is the sole Market capability authority.
//! This crate deliberately contains no hashing implementation, Solana types,
//! account access, CPI, token policy, or global capability enumeration.

use core::convert::TryInto;

/// SDK-free readiness account frames and state-transition planning.
pub mod readiness_frame;
/// Exact readiness instruction wires without SVM dependencies.
pub mod readiness_instruction;

pub use dclutch_core_contract::ContentId;

/// Exact manifest header width.
pub const MANIFEST_HEADER_BYTES: usize = 16;
/// Exact profile-1 capability-entry width.
pub const CAPABILITY_ENTRY_BYTES: usize = 288;
/// Exact immutable funding-quote width.
pub const FUNDING_QUOTE_BYTES: usize = 64;
/// Exact mutable funding-state width.
pub const FUNDING_STATE_BYTES: usize = 192;
/// Exact transient Market-opening readiness width.
pub const MARKET_OPENING_READINESS_BYTES: usize = 128;
/// Chain-derived maximum byte width of one Solana PDA seed component.
pub const SVM_MAX_PDA_SEED_BYTES: usize = 32;
/// Maximum profile-1 manifest byte width.
pub const MAX_MANIFEST_BYTES: usize =
    MANIFEST_HEADER_BYTES + MAX_CAPABILITIES * CAPABILITY_ENTRY_BYTES;

/// Provisional artifact-profile bound on capability entries.
///
/// This is neither a mathematical nor a product limit. The lifting plan is a
/// new manifest artifact profile with a wider layout, preserving profile-1
/// content identities and founding new Markets against the wider preimage.
pub const MAX_CAPABILITIES: usize = 16;
/// Provisional artifact-profile bound on dependencies per entry.
///
/// Dependencies are sorted entry indices rather than globally assigned bits.
/// A later artifact profile may widen this array without introducing a closed
/// capability-kind registry.
pub const MAX_DEPENDENCIES_PER_CAPABILITY: usize = 16;

/// Canonical manifest magic.
pub const MANIFEST_MAGIC: [u8; 8] = *b"DCLTCAP1";
/// Implemented manifest schema version.
pub const MANIFEST_SCHEMA_VERSION: u16 = 1;
/// Implemented provisional artifact profile.
pub const ARTIFACT_PROFILE_V1: u16 = 1;
/// Canonical funding-state magic.
pub const FUNDING_STATE_MAGIC: [u8; 8] = *b"DCLTCFS1";
/// Implemented funding-state schema version.
pub const FUNDING_STATE_SCHEMA_VERSION: u16 = 1;
/// Canonical transient Market-opening readiness magic.
pub const MARKET_OPENING_READINESS_MAGIC: [u8; 8] = *b"DCLTMOR1";
/// Implemented Market-opening readiness schema.
pub const MARKET_OPENING_READINESS_SCHEMA_VERSION: u16 = 1;
/// Adapter PDA seed domain for one transient Market-opening readiness child.
///
/// This crate derives no Solana address. The adapter derives it from this
/// domain plus exact Market key and generation, then authenticates the record.
pub const MARKET_OPENING_READINESS_PDA_DOMAIN: &[u8] = b"dclutch/open-readiness/v1";

/// The exact canonical empty-manifest preimage.
pub const EMPTY_MANIFEST_BYTES: [u8; MANIFEST_HEADER_BYTES] = [
    b'D', b'C', b'L', b'T', b'C', b'A', b'P', b'1', 1, 0, 1, 0, 0, 0, 0, 0,
];

const MANIFEST_SCHEMA_OFFSET: usize = 8;
const MANIFEST_PROFILE_OFFSET: usize = 10;
const MANIFEST_COUNT_OFFSET: usize = 12;
const MANIFEST_RESERVED_OFFSET: usize = 14;
const MANIFEST_RESERVED_BYTES: usize = 2;

const KIND_ID_OFFSET: usize = 0;
const RELEASE_ID_OFFSET: usize = 32;
const CONFIG_ID_OFFSET: usize = 64;
const CAPACITY_PROFILE_ID_OFFSET: usize = 96;
const CHILD_SCHEMA_ID_OFFSET: usize = 128;
const CHILD_DERIVATION_ID_OFFSET: usize = 160;
const ACTIVATION_POLICY_OFFSET: usize = 192;
const DEPENDENCY_COUNT_OFFSET: usize = 193;
const ENTRY_RESERVED_OFFSET: usize = 194;
const ENTRY_RESERVED_BYTES: usize = 6;
const ACTIVATION_DEADLINE_OFFSET: usize = 200;
const DEPENDENCIES_OFFSET: usize = 208;
const QUOTE_OFFSET: usize = 224;

const FUNDING_RENT_OFFSET: usize = 0;
const FUNDING_CREATION_OFFSET: usize = 8;
const FUNDING_WORK_OFFSET: usize = 16;
const FUNDING_PROVIDER_OFFSET: usize = 24;
const FUNDING_BOUNTY_OFFSET: usize = 32;
const FUNDING_LIQUIDITY_OFFSET: usize = 40;
const FUNDING_SERVICE_OFFSET: usize = 48;
const FUNDING_TOTAL_OFFSET: usize = 56;

const STATE_SCHEMA_OFFSET: usize = 8;
const STATE_STATUS_OFFSET: usize = 10;
const STATE_HEADER_RESERVED_OFFSET: usize = 11;
const STATE_HEADER_RESERVED_BYTES: usize = 5;
const STATE_MANIFEST_ID_OFFSET: usize = 16;
const STATE_ENTRY_INDEX_OFFSET: usize = 48;
const STATE_BODY_RESERVED_OFFSET: usize = 50;
const STATE_BODY_RESERVED_BYTES: usize = 6;
const STATE_ACTIVATION_SLOT_OFFSET: usize = 56;
const STATE_REMAINING_OFFSET: usize = 64;
const STATE_RELEASED_OFFSET: usize = 128;

const READINESS_SCHEMA_OFFSET: usize = 8;
const READINESS_RESERVED_OFFSET: usize = 10;
const READINESS_RESERVED_BYTES: usize = 6;
const READINESS_MARKET_OFFSET: usize = 16;
const READINESS_GENERATION_OFFSET: usize = 48;
const READINESS_MANIFEST_OFFSET: usize = 56;
const READINESS_ENTRY_COUNT_OFFSET: usize = 88;
const READINESS_NEXT_ENTRY_OFFSET: usize = 90;
const READINESS_BODY_RESERVED_OFFSET: usize = 92;
const READINESS_BODY_RESERVED_BYTES: usize = 4;
const READINESS_RENT_REFUND_OFFSET: usize = 96;

/// Explicit refusal returned by manifest and funding contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have its one exact canonical width.
    InvalidLength,
    /// Magic bytes did not identify the requested record.
    InvalidMagic,
    /// The record names an unsupported schema version.
    UnsupportedSchema,
    /// The manifest names an unsupported artifact profile.
    UnsupportedArtifactProfile,
    /// Reserved bytes or unused dependency slots were not zero.
    NonCanonicalReservedBytes,
    /// A content-addressed identity was the reserved all-zero value.
    ZeroContentId,
    /// A required Market key or rent-refund identity was all zero.
    ZeroIdentifier,
    /// The provisional artifact profile's entry bound was exceeded.
    TooManyCapabilities,
    /// The provisional artifact profile's dependency bound was exceeded.
    TooManyDependencies,
    /// Entries were not strictly ordered by unique capability-kind identity.
    NonCanonicalEntryOrder,
    /// Dependency indices were not strictly increasing.
    NonCanonicalDependencyOrder,
    /// A dependency index did not identify an entry in this manifest.
    InvalidDependency,
    /// The bounded dependency graph contains a cycle.
    CyclicDependencies,
    /// An activation-policy byte was unknown.
    UnknownActivationPolicy,
    /// A founding-required entry carried a nonzero lazy deadline.
    UnexpectedActivationDeadline,
    /// A lazy entry omitted its nonzero activation deadline.
    MissingActivationDeadline,
    /// A lazy entry did not prepay any rent or creation principal.
    MissingLazyCreationFunding,
    /// Summing exact principal compartments overflowed `u64`.
    ArithmeticOverflow,
    /// An encoded total did not equal its exact compartment sum.
    FundingTotalMismatch,
    /// A funding-state status byte was unknown.
    UnknownFundingStatus,
    /// Funding state did not bind to the supplied manifest entry.
    FundingBindingMismatch,
    /// Present observed principal did not exactly equal remaining principal.
    PresentPrincipalMismatch,
    /// Remaining plus released principal did not equal the immutable quote.
    FundingConservationMismatch,
    /// The requested compartment had insufficient segregated principal.
    InsufficientCompartmentPrincipal,
    /// A zero-principal release was requested.
    ZeroPrincipalRelease,
    /// Rent or creation principal was requested outside atomic activation.
    ActivationCompartmentRequired,
    /// The requested activation or release required a different status.
    InvalidFundingStatus,
    /// Lazy activation was attempted after the immutable slot deadline.
    ActivationDeadlineElapsed,
    /// A founding-required capability remained inactive at Market opening.
    FoundingCapabilityInactive,
    /// No founding-required entry matched the requested immutable config.
    RequiredFoundingConfigMissing,
    /// More than one founding-required entry matched the requested config.
    RequiredFoundingConfigAmbiguous,
    /// Resolution-Fund rent did not equal the adapter's exact rent calculation.
    ResolutionFundRentMismatch,
    /// A one-shot resolution Fund omitted its positive success bounty.
    MissingResolutionFundBounty,
    /// A one-shot resolution Fund named principal it does not physically hold.
    ExtraneousResolutionFundPrincipal,
    /// A readiness record did not bind the authenticated Market, generation, manifest, or entry count.
    ReadinessBindingMismatch,
    /// An advance did not name the one canonical next manifest entry.
    ReadinessIndexMismatch,
    /// Market opening attempted before every manifest entry became ready.
    ReadinessIncomplete,
}

/// Result alias for this contract crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Whether physical capability activation is required during founding or may
/// be performed lazily from prepaid principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ActivationPolicy {
    /// The capability must be active before the Market can open.
    RequiredAtFounding = 0,
    /// Physical creation is precommitted and prepaid but may occur by deadline.
    PrepaidLazy = 1,
}

impl ActivationPolicy {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::RequiredAtFounding),
            1 => Ok(Self::PrepaidLazy),
            _ => Err(Error::UnknownActivationPolicy),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::RequiredAtFounding => 0,
            Self::PrepaidLazy => 1,
        }
    }
}

/// Exact principal compartments used by an immutable quote or mutable ledger.
///
/// These are present capitalization categories. Hoard collateral and expected
/// future fee revenue are intentionally not representable.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FundingAmountsV1 {
    rent_principal: u64,
    creation_principal: u64,
    work_principal: u64,
    provider_principal: u64,
    bounty_principal: u64,
    liquidity_principal: u64,
    service_principal: u64,
    total_principal: u64,
}

impl FundingAmountsV1 {
    /// Construct compartments and their checked canonical total.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rent_principal: u64,
        creation_principal: u64,
        work_principal: u64,
        provider_principal: u64,
        bounty_principal: u64,
        liquidity_principal: u64,
        service_principal: u64,
    ) -> Result<Self> {
        let values = [
            rent_principal,
            creation_principal,
            work_principal,
            provider_principal,
            bounty_principal,
            liquidity_principal,
            service_principal,
        ];
        let total_principal = checked_sum(&values)?;
        Ok(Self {
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

    /// Decode one exact canonical compartment record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_QUOTE_BYTES {
            return Err(Error::InvalidLength);
        }
        let result = Self::new(
            read_u64(bytes, FUNDING_RENT_OFFSET)?,
            read_u64(bytes, FUNDING_CREATION_OFFSET)?,
            read_u64(bytes, FUNDING_WORK_OFFSET)?,
            read_u64(bytes, FUNDING_PROVIDER_OFFSET)?,
            read_u64(bytes, FUNDING_BOUNTY_OFFSET)?,
            read_u64(bytes, FUNDING_LIQUIDITY_OFFSET)?,
            read_u64(bytes, FUNDING_SERVICE_OFFSET)?,
        )?;
        if result.total_principal != read_u64(bytes, FUNDING_TOTAL_OFFSET)? {
            return Err(Error::FundingTotalMismatch);
        }
        Ok(result)
    }

    /// Return the exact canonical bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_QUOTE_BYTES] {
        let mut output = [0u8; FUNDING_QUOTE_BYTES];
        put_u64(&mut output, FUNDING_RENT_OFFSET, self.rent_principal);
        put_u64(
            &mut output,
            FUNDING_CREATION_OFFSET,
            self.creation_principal,
        );
        put_u64(&mut output, FUNDING_WORK_OFFSET, self.work_principal);
        put_u64(
            &mut output,
            FUNDING_PROVIDER_OFFSET,
            self.provider_principal,
        );
        put_u64(&mut output, FUNDING_BOUNTY_OFFSET, self.bounty_principal);
        put_u64(
            &mut output,
            FUNDING_LIQUIDITY_OFFSET,
            self.liquidity_principal,
        );
        put_u64(&mut output, FUNDING_SERVICE_OFFSET, self.service_principal);
        put_u64(&mut output, FUNDING_TOTAL_OFFSET, self.total_principal);
        output
    }

    /// Return rent principal.
    pub const fn rent_principal(self) -> u64 {
        self.rent_principal
    }
    /// Return physical-creation principal.
    pub const fn creation_principal(self) -> u64 {
        self.creation_principal
    }
    /// Return ongoing work principal.
    pub const fn work_principal(self) -> u64 {
        self.work_principal
    }
    /// Return provider principal.
    pub const fn provider_principal(self) -> u64 {
        self.provider_principal
    }
    /// Return resolution or maintenance bounty principal.
    pub const fn bounty_principal(self) -> u64 {
        self.bounty_principal
    }
    /// Return segregated liquidity principal.
    pub const fn liquidity_principal(self) -> u64 {
        self.liquidity_principal
    }
    /// Return service principal.
    pub const fn service_principal(self) -> u64 {
        self.service_principal
    }
    /// Return the checked sum of all compartments.
    pub const fn total_principal(self) -> u64 {
        self.total_principal
    }

    fn value(self, compartment: FundingCompartment) -> u64 {
        match compartment {
            FundingCompartment::Rent => self.rent_principal,
            FundingCompartment::Creation => self.creation_principal,
            FundingCompartment::Work => self.work_principal,
            FundingCompartment::Provider => self.provider_principal,
            FundingCompartment::Bounty => self.bounty_principal,
            FundingCompartment::Liquidity => self.liquidity_principal,
            FundingCompartment::Service => self.service_principal,
        }
    }

    fn with_value(self, compartment: FundingCompartment, value: u64) -> Result<Self> {
        Self::new(
            if compartment == FundingCompartment::Rent {
                value
            } else {
                self.rent_principal
            },
            if compartment == FundingCompartment::Creation {
                value
            } else {
                self.creation_principal
            },
            if compartment == FundingCompartment::Work {
                value
            } else {
                self.work_principal
            },
            if compartment == FundingCompartment::Provider {
                value
            } else {
                self.provider_principal
            },
            if compartment == FundingCompartment::Bounty {
                value
            } else {
                self.bounty_principal
            },
            if compartment == FundingCompartment::Liquidity {
                value
            } else {
                self.liquidity_principal
            },
            if compartment == FundingCompartment::Service {
                value
            } else {
                self.service_principal
            },
        )
    }
}

/// Immutable funding quote committed by one capability entry.
pub type FundingQuoteV1 = FundingAmountsV1;

/// One canonical capability entry selected by a Market manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityEntryV1 {
    kind_id: ContentId,
    release_id: ContentId,
    config_id: ContentId,
    capacity_profile_id: ContentId,
    child_schema_id: ContentId,
    child_derivation_id: ContentId,
    activation_policy: ActivationPolicy,
    activation_deadline_slot: u64,
    dependency_count: u8,
    dependencies: [u8; MAX_DEPENDENCIES_PER_CAPABILITY],
    funding_quote: FundingQuoteV1,
}

impl CapabilityEntryV1 {
    /// Construct and validate one entry.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind_id: ContentId,
        release_id: ContentId,
        config_id: ContentId,
        capacity_profile_id: ContentId,
        child_schema_id: ContentId,
        child_derivation_id: ContentId,
        activation_policy: ActivationPolicy,
        activation_deadline_slot: u64,
        dependency_count: u8,
        dependencies: [u8; MAX_DEPENDENCIES_PER_CAPABILITY],
        funding_quote: FundingQuoteV1,
    ) -> Result<Self> {
        let count = usize::from(dependency_count);
        if count > MAX_DEPENDENCIES_PER_CAPABILITY {
            return Err(Error::TooManyDependencies);
        }
        require_zero(
            &dependencies,
            count,
            MAX_DEPENDENCIES_PER_CAPABILITY - count,
        )?;
        validate_dependency_order(&dependencies, count)?;
        validate_activation(activation_policy, activation_deadline_slot, funding_quote)?;
        Ok(Self {
            kind_id,
            release_id,
            config_id,
            capacity_profile_id,
            child_schema_id,
            child_derivation_id,
            activation_policy,
            activation_deadline_slot,
            dependency_count,
            dependencies,
            funding_quote,
        })
    }

    /// Decode one exact canonical profile-1 entry.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_ENTRY_BYTES {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, ENTRY_RESERVED_OFFSET, ENTRY_RESERVED_BYTES)?;
        let mut dependencies = [0u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        copy_exact(
            &mut dependencies,
            0,
            subslice(bytes, DEPENDENCIES_OFFSET, MAX_DEPENDENCIES_PER_CAPABILITY)?,
        )?;
        Self::new(
            read_content_id(bytes, KIND_ID_OFFSET)?,
            read_content_id(bytes, RELEASE_ID_OFFSET)?,
            read_content_id(bytes, CONFIG_ID_OFFSET)?,
            read_content_id(bytes, CAPACITY_PROFILE_ID_OFFSET)?,
            read_content_id(bytes, CHILD_SCHEMA_ID_OFFSET)?,
            read_content_id(bytes, CHILD_DERIVATION_ID_OFFSET)?,
            ActivationPolicy::decode(read_byte(bytes, ACTIVATION_POLICY_OFFSET)?)?,
            read_u64(bytes, ACTIVATION_DEADLINE_OFFSET)?,
            read_byte(bytes, DEPENDENCY_COUNT_OFFSET)?,
            dependencies,
            FundingQuoteV1::decode(subslice(bytes, QUOTE_OFFSET, FUNDING_QUOTE_BYTES)?)?,
        )
    }

    /// Return the exact canonical profile-1 bytes.
    pub fn to_bytes(self) -> [u8; CAPABILITY_ENTRY_BYTES] {
        let mut output = [0u8; CAPABILITY_ENTRY_BYTES];
        copy_content_id(&mut output, KIND_ID_OFFSET, self.kind_id);
        copy_content_id(&mut output, RELEASE_ID_OFFSET, self.release_id);
        copy_content_id(&mut output, CONFIG_ID_OFFSET, self.config_id);
        copy_content_id(
            &mut output,
            CAPACITY_PROFILE_ID_OFFSET,
            self.capacity_profile_id,
        );
        copy_content_id(&mut output, CHILD_SCHEMA_ID_OFFSET, self.child_schema_id);
        copy_content_id(
            &mut output,
            CHILD_DERIVATION_ID_OFFSET,
            self.child_derivation_id,
        );
        put_byte(
            &mut output,
            ACTIVATION_POLICY_OFFSET,
            self.activation_policy.byte(),
        );
        put_byte(&mut output, DEPENDENCY_COUNT_OFFSET, self.dependency_count);
        copy_infallible(&mut output, DEPENDENCIES_OFFSET, &self.dependencies);
        put_u64(
            &mut output,
            ACTIVATION_DEADLINE_OFFSET,
            self.activation_deadline_slot,
        );
        copy_infallible(&mut output, QUOTE_OFFSET, &self.funding_quote.to_bytes());
        output
    }

    /// Return the nonzero capability-kind content identity used for ordering.
    pub const fn kind_id(self) -> ContentId {
        self.kind_id
    }
    /// Return the selected implementation-release content identity.
    pub const fn release_id(self) -> ContentId {
        self.release_id
    }
    /// Return immutable configuration content identity.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }
    /// Return the capacity-profile content identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }
    /// Return the child-layout schema content identity.
    pub const fn child_schema_id(self) -> ContentId {
        self.child_schema_id
    }
    /// Return the child PDA/derivation-policy content identity.
    pub const fn child_derivation_id(self) -> ContentId {
        self.child_derivation_id
    }
    /// Return the immutable activation policy.
    pub const fn activation_policy(self) -> ActivationPolicy {
        self.activation_policy
    }
    /// Return the lazy activation deadline slot, or zero for founding-required.
    pub const fn activation_deadline_slot(self) -> u64 {
        self.activation_deadline_slot
    }
    /// Return the number of active dependency indices.
    pub const fn dependency_count(self) -> u8 {
        self.dependency_count
    }
    /// Return one dependency index, refusing an inactive array position.
    pub fn dependency(self, position: usize) -> Result<u8> {
        if position >= usize::from(self.dependency_count) {
            return Err(Error::InvalidDependency);
        }
        self.dependencies
            .get(position)
            .copied()
            .ok_or(Error::InvalidDependency)
    }
    /// Return the immutable present-principal quote.
    pub const fn funding_quote(self) -> FundingQuoteV1 {
        self.funding_quote
    }
}

/// Unique founding-required manifest entry selected by immutable config.
///
/// The index and decoded entry are returned together so a composing adapter
/// cannot select funding from one entry while deriving a child from another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequiredFoundingEntryV1 {
    index: u16,
    entry: CapabilityEntryV1,
}

impl RequiredFoundingEntryV1 {
    /// Return the exact canonical manifest index.
    pub const fn index(self) -> u16 {
        self.index
    }

    /// Return the uniquely selected immutable entry.
    pub const fn entry(self) -> CapabilityEntryV1 {
        self.entry
    }

    /// Validate the current one-shot resolution-Fund funding profile.
    ///
    /// `exact_fund_rent` is calculated from the authenticated Fund account
    /// width and Rent sysvar by the adapter. The immutable entry must quote
    /// exactly that rent, provider reimbursement, and a positive bounty. It
    /// may not quote creation, work, liquidity, or service principal because
    /// the specialized Fund account does not hold those compartments.
    pub fn validate_one_shot_resolution_fund_quote(
        self,
        exact_fund_rent: u64,
    ) -> Result<FundingQuoteV1> {
        let quote = self.entry.funding_quote;
        if quote.rent_principal != exact_fund_rent {
            return Err(Error::ResolutionFundRentMismatch);
        }
        if quote.bounty_principal == 0 {
            return Err(Error::MissingResolutionFundBounty);
        }
        if quote.creation_principal != 0
            || quote.work_principal != 0
            || quote.liquidity_principal != 0
            || quote.service_principal != 0
        {
            return Err(Error::ExtraneousResolutionFundPrincipal);
        }
        Ok(quote)
    }
}

/// Borrowed, validated canonical manifest preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityManifestV1<'a> {
    bytes: &'a [u8],
    entry_count: u16,
}

impl<'a> CapabilityManifestV1<'a> {
    /// Decode and fully validate an exact canonical profile-1 manifest.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < MANIFEST_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != MANIFEST_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, MANIFEST_SCHEMA_OFFSET)? != MANIFEST_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, MANIFEST_PROFILE_OFFSET)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, MANIFEST_RESERVED_OFFSET, MANIFEST_RESERVED_BYTES)?;
        let entry_count = read_u16(bytes, MANIFEST_COUNT_OFFSET)?;
        let count = usize::from(entry_count);
        if count > MAX_CAPABILITIES {
            return Err(Error::TooManyCapabilities);
        }
        let expected = manifest_bytes_for_count(count)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let manifest = Self { bytes, entry_count };
        validate_manifest(&manifest)?;
        Ok(manifest)
    }

    /// Encode entries into caller-owned storage and return its validated view.
    pub fn encode_into(entries: &[CapabilityEntryV1], output: &'a mut [u8]) -> Result<Self> {
        if entries.len() > MAX_CAPABILITIES {
            return Err(Error::TooManyCapabilities);
        }
        let expected = manifest_bytes_for_count(entries.len())?;
        if output.len() != expected {
            return Err(Error::InvalidLength);
        }
        validate_entry_slice(entries)?;
        output.fill(0);
        copy_exact(output, 0, &MANIFEST_MAGIC)?;
        put_u16(output, MANIFEST_SCHEMA_OFFSET, MANIFEST_SCHEMA_VERSION);
        put_u16(output, MANIFEST_PROFILE_OFFSET, ARTIFACT_PROFILE_V1);
        let entry_count = u16::try_from(entries.len()).map_err(|_| Error::TooManyCapabilities)?;
        put_u16(output, MANIFEST_COUNT_OFFSET, entry_count);
        for (index, entry) in entries.iter().enumerate() {
            let offset = entry_offset(index)?;
            copy_exact(output, offset, &entry.to_bytes())?;
        }
        Self::decode(output)
    }

    /// Decode the exact canonical empty manifest.
    pub fn empty() -> Result<CapabilityManifestV1<'static>> {
        CapabilityManifestV1::decode(&EMPTY_MANIFEST_BYTES)
    }

    /// Borrow the exact content-hash preimage.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Return the number of capability entries.
    pub const fn entry_count(self) -> u16 {
        self.entry_count
    }

    /// Decode one selected entry.
    pub fn entry(self, index: u16) -> Result<CapabilityEntryV1> {
        if index >= self.entry_count {
            return Err(Error::InvalidDependency);
        }
        let offset = entry_offset(usize::from(index))?;
        CapabilityEntryV1::decode(subslice(self.bytes, offset, CAPABILITY_ENTRY_BYTES)?)
    }

    /// Select the unique founding-required entry for one immutable config.
    ///
    /// The current resolution adapter passes the authenticated Market
    /// identity's `resolution_policy_id`. Lazy entries and founding-required
    /// entries for other configs are ignored. Missing and ambiguous matches
    /// are explicit refusals; manifest order never becomes an implicit
    /// funding-authority tie breaker.
    pub fn required_founding_entry_for_config(
        self,
        config_id: ContentId,
    ) -> Result<RequiredFoundingEntryV1> {
        let mut selected: Option<RequiredFoundingEntryV1> = None;
        let mut index = 0u16;
        while index < self.entry_count {
            let entry = self.entry(index)?;
            if entry.activation_policy == ActivationPolicy::RequiredAtFounding
                && entry.config_id == config_id
            {
                if selected.is_some() {
                    return Err(Error::RequiredFoundingConfigAmbiguous);
                }
                selected = Some(RequiredFoundingEntryV1 { index, entry });
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        selected.ok_or(Error::RequiredFoundingConfigMissing)
    }
}

/// Mutable lifecycle status for one capability's prepaid funding state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingStatus {
    /// Exact quote principal is present but physical activation has not run.
    Pending = 0,
    /// Physical activation completed atomically with rent/creation release.
    Active = 1,
}

impl FundingStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Active),
            _ => Err(Error::UnknownFundingStatus),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::Active => 1,
        }
    }
}

/// A segregated funding compartment that may be released after activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingCompartment {
    /// Child rent principal, released only by activation.
    Rent,
    /// Child physical-creation principal, released only by activation.
    Creation,
    /// Ongoing work principal.
    Work,
    /// Provider principal.
    Provider,
    /// Bounty principal.
    Bounty,
    /// Liquidity principal.
    Liquidity,
    /// Service principal.
    Service,
}

/// Exact activation transfer plan returned for atomic adapter execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationDebitV1 {
    rent_principal: u64,
    creation_principal: u64,
}

impl ActivationDebitV1 {
    /// Return rent principal that must move atomically with activation.
    pub const fn rent_principal(self) -> u64 {
        self.rent_principal
    }
    /// Return creation principal that must move atomically with activation.
    pub const fn creation_principal(self) -> u64 {
        self.creation_principal
    }
}

/// Separately mutable, manifest-bound capability funding ledger.
///
/// The composing adapter supplies the observed holding-account principal to
/// [`Self::new`] and [`Self::validate_against`]. It must commit state only in
/// the same successful transaction that executes a returned debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingStateV1 {
    manifest_content_id: ContentId,
    entry_index: u16,
    status: FundingStatus,
    activation_slot: u64,
    remaining: FundingAmountsV1,
    released: FundingAmountsV1,
}

impl FundingStateV1 {
    /// Construct an exactly prepaid pending state for one manifest entry.
    pub fn new(
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        entry_index: u16,
        observed_present_principal: u64,
    ) -> Result<Self> {
        let entry = manifest.entry(entry_index)?;
        if observed_present_principal != entry.funding_quote.total_principal {
            return Err(Error::PresentPrincipalMismatch);
        }
        Ok(Self {
            manifest_content_id,
            entry_index,
            status: FundingStatus::Pending,
            activation_slot: 0,
            remaining: entry.funding_quote,
            released: FundingAmountsV1::default(),
        })
    }

    /// Decode one exact canonical funding-state record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_STATE_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != FUNDING_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, STATE_SCHEMA_OFFSET)? != FUNDING_STATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            STATE_HEADER_RESERVED_OFFSET,
            STATE_HEADER_RESERVED_BYTES,
        )?;
        require_zero(bytes, STATE_BODY_RESERVED_OFFSET, STATE_BODY_RESERVED_BYTES)?;
        let result = Self {
            manifest_content_id: read_content_id(bytes, STATE_MANIFEST_ID_OFFSET)?,
            entry_index: read_u16(bytes, STATE_ENTRY_INDEX_OFFSET)?,
            status: FundingStatus::decode(read_byte(bytes, STATE_STATUS_OFFSET)?)?,
            activation_slot: read_u64(bytes, STATE_ACTIVATION_SLOT_OFFSET)?,
            remaining: FundingAmountsV1::decode(subslice(
                bytes,
                STATE_REMAINING_OFFSET,
                FUNDING_QUOTE_BYTES,
            )?)?,
            released: FundingAmountsV1::decode(subslice(
                bytes,
                STATE_RELEASED_OFFSET,
                FUNDING_QUOTE_BYTES,
            )?)?,
        };
        match result.status {
            FundingStatus::Pending
                if result.activation_slot != 0 || result.released.total_principal != 0 =>
            {
                Err(Error::InvalidFundingStatus)
            }
            FundingStatus::Active => Ok(result),
            FundingStatus::Pending => Ok(result),
        }
    }

    /// Return exact canonical bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_STATE_BYTES] {
        let mut output = [0u8; FUNDING_STATE_BYTES];
        copy_infallible(&mut output, 0, &FUNDING_STATE_MAGIC);
        put_u16(
            &mut output,
            STATE_SCHEMA_OFFSET,
            FUNDING_STATE_SCHEMA_VERSION,
        );
        put_byte(&mut output, STATE_STATUS_OFFSET, self.status.byte());
        copy_content_id(
            &mut output,
            STATE_MANIFEST_ID_OFFSET,
            self.manifest_content_id,
        );
        put_u16(&mut output, STATE_ENTRY_INDEX_OFFSET, self.entry_index);
        put_u64(
            &mut output,
            STATE_ACTIVATION_SLOT_OFFSET,
            self.activation_slot,
        );
        copy_infallible(
            &mut output,
            STATE_REMAINING_OFFSET,
            &self.remaining.to_bytes(),
        );
        copy_infallible(
            &mut output,
            STATE_RELEASED_OFFSET,
            &self.released.to_bytes(),
        );
        output
    }

    /// Validate binding, per-compartment quote conservation, pending/active
    /// invariants, and exact presently held principal.
    pub fn validate_against(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        observed_present_principal: u64,
    ) -> Result<()> {
        if self.manifest_content_id != manifest_content_id {
            return Err(Error::FundingBindingMismatch);
        }
        let entry = manifest.entry(self.entry_index)?;
        validate_conservation(self.remaining, self.released, entry.funding_quote)?;
        if observed_present_principal != self.remaining.total_principal {
            return Err(Error::PresentPrincipalMismatch);
        }
        match self.status {
            FundingStatus::Pending => {
                if self.activation_slot != 0 || self.released.total_principal != 0 {
                    return Err(Error::InvalidFundingStatus);
                }
            }
            FundingStatus::Active => {
                if self.remaining.rent_principal != 0
                    || self.remaining.creation_principal != 0
                    || self.released.rent_principal != entry.funding_quote.rent_principal
                    || self.released.creation_principal != entry.funding_quote.creation_principal
                {
                    return Err(Error::FundingConservationMismatch);
                }
            }
        }
        Ok(())
    }

    /// Determine whether this funding state permits Market opening at a slot.
    pub fn validate_market_open(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        observed_present_principal: u64,
        current_slot: u64,
    ) -> Result<()> {
        self.validate_against(manifest_content_id, manifest, observed_present_principal)?;
        let entry = manifest.entry(self.entry_index)?;
        match (entry.activation_policy, self.status) {
            (ActivationPolicy::RequiredAtFounding, FundingStatus::Pending) => {
                Err(Error::FoundingCapabilityInactive)
            }
            (ActivationPolicy::PrepaidLazy, FundingStatus::Pending)
                if current_slot > entry.activation_deadline_slot =>
            {
                Err(Error::ActivationDeadlineElapsed)
            }
            _ => Ok(()),
        }
    }

    /// Activate a capability and segregatedly release its exact rent and
    /// creation quote. The adapter executes the returned transfer plan and
    /// persists this mutation atomically only after physical creation succeeds.
    pub fn activate(
        &mut self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        observed_present_principal: u64,
        current_slot: u64,
    ) -> Result<ActivationDebitV1> {
        self.validate_against(manifest_content_id, manifest, observed_present_principal)?;
        if self.status != FundingStatus::Pending {
            return Err(Error::InvalidFundingStatus);
        }
        let entry = manifest.entry(self.entry_index)?;
        if entry.activation_policy == ActivationPolicy::PrepaidLazy
            && current_slot > entry.activation_deadline_slot
        {
            return Err(Error::ActivationDeadlineElapsed);
        }
        let debit = ActivationDebitV1 {
            rent_principal: entry.funding_quote.rent_principal,
            creation_principal: entry.funding_quote.creation_principal,
        };
        let mut next_remaining = self.remaining;
        let mut next_released = self.released;
        move_compartment(
            &mut next_remaining,
            &mut next_released,
            FundingCompartment::Rent,
            debit.rent_principal,
        )?;
        move_compartment(
            &mut next_remaining,
            &mut next_released,
            FundingCompartment::Creation,
            debit.creation_principal,
        )?;
        self.remaining = next_remaining;
        self.released = next_released;
        self.status = FundingStatus::Active;
        self.activation_slot = current_slot;
        Ok(debit)
    }

    /// Release exact principal from one non-activation compartment.
    pub fn release(
        &mut self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        observed_present_principal: u64,
        compartment: FundingCompartment,
        principal: u64,
    ) -> Result<()> {
        self.validate_against(manifest_content_id, manifest, observed_present_principal)?;
        if self.status != FundingStatus::Active {
            return Err(Error::InvalidFundingStatus);
        }
        if principal == 0 {
            return Err(Error::ZeroPrincipalRelease);
        }
        if matches!(
            compartment,
            FundingCompartment::Rent | FundingCompartment::Creation
        ) {
            return Err(Error::ActivationCompartmentRequired);
        }
        move_compartment(
            &mut self.remaining,
            &mut self.released,
            compartment,
            principal,
        )
    }

    /// Return the bound manifest content identity.
    pub const fn manifest_content_id(self) -> ContentId {
        self.manifest_content_id
    }
    /// Return the bound manifest entry index.
    pub const fn entry_index(self) -> u16 {
        self.entry_index
    }
    /// Return activation status.
    pub const fn status(self) -> FundingStatus {
        self.status
    }
    /// Return the activation slot, or zero while pending.
    pub const fn activation_slot(self) -> u64 {
        self.activation_slot
    }
    /// Return exact presently held principal by compartment.
    pub const fn remaining(self) -> FundingAmountsV1 {
        self.remaining
    }
    /// Return exact previously released principal by compartment.
    pub const fn released(self) -> FundingAmountsV1 {
        self.released
    }
}

/// Transient direct Market child proving canonical opening-capability readiness.
///
/// This record owns only progression and a sponsor's rent-refund identity.
/// Each [`FundingStateV1`] remains the sole owner of its quote, remaining
/// principal, released principal, and activation facts. There is deliberately
/// no duplicated Ready status: readiness is derived exactly when
/// `next_entry_index == entry_count`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketOpeningReadinessV1 {
    market: [u8; 32],
    generation: u64,
    manifest_content_id: ContentId,
    entry_count: u16,
    next_entry_index: u16,
    sponsor_rent_refund: [u8; 32],
}

impl MarketOpeningReadinessV1 {
    /// Begin one transient readiness child for an authenticated canonical manifest.
    ///
    /// The composing adapter must authenticate that `manifest_content_id` is
    /// the hash of `manifest.as_bytes()`, derive this direct child from
    /// [`MARKET_OPENING_READINESS_PDA_DOMAIN`], and increment the Market child
    /// count atomically with persistence.
    pub fn begin(
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        sponsor_rent_refund: [u8; 32],
    ) -> Result<Self> {
        require_nonzero_identifier(&market)?;
        require_nonzero_identifier(&sponsor_rent_refund)?;
        Ok(Self {
            market,
            generation,
            manifest_content_id,
            entry_count: manifest.entry_count(),
            next_entry_index: 0,
            sponsor_rent_refund,
        })
    }

    /// Decode one exact canonical readiness record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != MARKET_OPENING_READINESS_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != MARKET_OPENING_READINESS_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, READINESS_SCHEMA_OFFSET)? != MARKET_OPENING_READINESS_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, READINESS_RESERVED_OFFSET, READINESS_RESERVED_BYTES)?;
        require_zero(
            bytes,
            READINESS_BODY_RESERVED_OFFSET,
            READINESS_BODY_RESERVED_BYTES,
        )?;
        let result = Self {
            market: read_array(bytes, READINESS_MARKET_OFFSET)?,
            generation: read_u64(bytes, READINESS_GENERATION_OFFSET)?,
            manifest_content_id: read_content_id(bytes, READINESS_MANIFEST_OFFSET)?,
            entry_count: read_u16(bytes, READINESS_ENTRY_COUNT_OFFSET)?,
            next_entry_index: read_u16(bytes, READINESS_NEXT_ENTRY_OFFSET)?,
            sponsor_rent_refund: read_array(bytes, READINESS_RENT_REFUND_OFFSET)?,
        };
        require_nonzero_identifier(&result.market)?;
        require_nonzero_identifier(&result.sponsor_rent_refund)?;
        if usize::from(result.entry_count) > MAX_CAPABILITIES
            || result.next_entry_index > result.entry_count
        {
            return Err(Error::ReadinessBindingMismatch);
        }
        Ok(result)
    }

    /// Return exact canonical readiness bytes.
    pub fn to_bytes(self) -> [u8; MARKET_OPENING_READINESS_BYTES] {
        let mut output = [0u8; MARKET_OPENING_READINESS_BYTES];
        copy_infallible(&mut output, 0, &MARKET_OPENING_READINESS_MAGIC);
        put_u16(
            &mut output,
            READINESS_SCHEMA_OFFSET,
            MARKET_OPENING_READINESS_SCHEMA_VERSION,
        );
        copy_infallible(&mut output, READINESS_MARKET_OFFSET, &self.market);
        put_u64(&mut output, READINESS_GENERATION_OFFSET, self.generation);
        copy_content_id(
            &mut output,
            READINESS_MANIFEST_OFFSET,
            self.manifest_content_id,
        );
        put_u16(&mut output, READINESS_ENTRY_COUNT_OFFSET, self.entry_count);
        put_u16(
            &mut output,
            READINESS_NEXT_ENTRY_OFFSET,
            self.next_entry_index,
        );
        copy_infallible(
            &mut output,
            READINESS_RENT_REFUND_OFFSET,
            &self.sponsor_rent_refund,
        );
        output
    }

    /// Advance exactly one canonical manifest entry after validating its actual funding state.
    ///
    /// All refusals occur before `next_entry_index` changes. The adapter must
    /// seal capability operations after each accepted advance: while the
    /// Market remains Founding and before this transient child is consumed at
    /// Open, no SBF capability operation may release funding principal.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        expected_entry_index: u16,
        funding: FundingStateV1,
        observed_present_principal: u64,
        current_slot: u64,
    ) -> Result<()> {
        self.validate_binding(market, generation, manifest_content_id, manifest)?;
        if expected_entry_index != self.next_entry_index {
            return Err(Error::ReadinessIndexMismatch);
        }
        if self.next_entry_index >= self.entry_count {
            return Err(Error::ReadinessIndexMismatch);
        }
        if funding.entry_index() != expected_entry_index {
            return Err(Error::FundingBindingMismatch);
        }
        funding.validate_market_open(
            manifest_content_id,
            manifest,
            observed_present_principal,
            current_slot,
        )?;
        self.next_entry_index = self
            .next_entry_index
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Require this record is derived Ready and matches the exact Market opening.
    ///
    /// An SBF Open transition must atomically consume/close this child,
    /// create custody, refund this record's rent to `sponsor_rent_refund`, and
    /// keep the Market direct-child count coherent. This kernel deliberately
    /// stores neither custody nor any principal amount.
    pub fn require_ready_for_open(
        self,
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
    ) -> Result<()> {
        self.validate_binding(market, generation, manifest_content_id, manifest)?;
        if self.next_entry_index != self.entry_count {
            return Err(Error::ReadinessIncomplete);
        }
        Ok(())
    }

    /// Return whether all canonical manifest entries have advanced.
    pub const fn is_ready(self) -> bool {
        self.next_entry_index == self.entry_count
    }

    /// Return the canonical next entry index.
    pub const fn next_entry_index(self) -> u16 {
        self.next_entry_index
    }

    /// Return the immutable entry count bound at begin.
    pub const fn entry_count(self) -> u16 {
        self.entry_count
    }

    /// Return the authenticated sponsor rent-refund identity.
    pub const fn sponsor_rent_refund(&self) -> &[u8; 32] {
        &self.sponsor_rent_refund
    }

    fn validate_binding(
        self,
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
    ) -> Result<()> {
        if self.market != market
            || self.generation != generation
            || self.manifest_content_id != manifest_content_id
            || self.entry_count != manifest.entry_count()
            || self.next_entry_index > self.entry_count
        {
            return Err(Error::ReadinessBindingMismatch);
        }
        Ok(())
    }
}

fn validate_manifest(manifest: &CapabilityManifestV1<'_>) -> Result<()> {
    let count = usize::from(manifest.entry_count);
    let mut prior_kind: Option<[u8; 32]> = None;
    let mut index = 0usize;
    while index < count {
        let entry_index = u16::try_from(index).map_err(|_| Error::TooManyCapabilities)?;
        let entry = manifest.entry(entry_index)?;
        let kind = entry.kind_id.to_bytes();
        if prior_kind.is_some_and(|prior| prior >= kind) {
            return Err(Error::NonCanonicalEntryOrder);
        }
        prior_kind = Some(kind);
        let mut dependency_position = 0usize;
        while dependency_position < usize::from(entry.dependency_count) {
            let dependency = usize::from(entry.dependency(dependency_position)?);
            if dependency >= count || dependency == index {
                return Err(Error::InvalidDependency);
            }
            dependency_position = dependency_position
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    validate_acyclic(manifest)
}

fn validate_entry_slice(entries: &[CapabilityEntryV1]) -> Result<()> {
    let count = entries.len();
    let mut prior_kind: Option<[u8; 32]> = None;
    for (index, entry) in entries.iter().enumerate() {
        let kind = entry.kind_id.to_bytes();
        if prior_kind.is_some_and(|prior| prior >= kind) {
            return Err(Error::NonCanonicalEntryOrder);
        }
        prior_kind = Some(kind);
        let mut dependency_position = 0usize;
        while dependency_position < usize::from(entry.dependency_count) {
            let dependency = usize::from(entry.dependency(dependency_position)?);
            if dependency >= count || dependency == index {
                return Err(Error::InvalidDependency);
            }
            dependency_position = dependency_position
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }
    validate_entry_slice_acyclic(entries)
}

fn validate_entry_slice_acyclic(entries: &[CapabilityEntryV1]) -> Result<()> {
    let count = entries.len();
    let mut resolved = [false; MAX_CAPABILITIES];
    let mut resolved_count = 0usize;
    while resolved_count < count {
        let mut progressed = false;
        for (index, entry) in entries.iter().enumerate() {
            if !read_bool(&resolved, index)? {
                let mut ready = true;
                let mut dependency_position = 0usize;
                while dependency_position < usize::from(entry.dependency_count) {
                    let dependency = usize::from(entry.dependency(dependency_position)?);
                    if !read_bool(&resolved, dependency)? {
                        ready = false;
                    }
                    dependency_position = dependency_position
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                }
                if ready {
                    write_bool(&mut resolved, index, true)?;
                    resolved_count = resolved_count
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                    progressed = true;
                }
            }
        }
        if !progressed {
            return Err(Error::CyclicDependencies);
        }
    }
    Ok(())
}

fn validate_acyclic(manifest: &CapabilityManifestV1<'_>) -> Result<()> {
    let count = usize::from(manifest.entry_count);
    let mut resolved = [false; MAX_CAPABILITIES];
    let mut resolved_count = 0usize;
    while resolved_count < count {
        let mut progressed = false;
        let mut index = 0usize;
        while index < count {
            if !read_bool(&resolved, index)? {
                let entry_index = u16::try_from(index).map_err(|_| Error::TooManyCapabilities)?;
                let entry = manifest.entry(entry_index)?;
                let mut ready = true;
                let mut dependency_position = 0usize;
                while dependency_position < usize::from(entry.dependency_count) {
                    let dependency = usize::from(entry.dependency(dependency_position)?);
                    if !read_bool(&resolved, dependency)? {
                        ready = false;
                    }
                    dependency_position = dependency_position
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                }
                if ready {
                    write_bool(&mut resolved, index, true)?;
                    resolved_count = resolved_count
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                    progressed = true;
                }
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !progressed {
            return Err(Error::CyclicDependencies);
        }
    }
    Ok(())
}

fn validate_dependency_order(dependencies: &[u8], count: usize) -> Result<()> {
    let mut prior: Option<u8> = None;
    let mut index = 0usize;
    while index < count {
        let dependency = dependencies
            .get(index)
            .copied()
            .ok_or(Error::TooManyDependencies)?;
        if prior.is_some_and(|value| value >= dependency) {
            return Err(Error::NonCanonicalDependencyOrder);
        }
        prior = Some(dependency);
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn validate_activation(
    policy: ActivationPolicy,
    deadline: u64,
    quote: FundingQuoteV1,
) -> Result<()> {
    match policy {
        ActivationPolicy::RequiredAtFounding if deadline != 0 => {
            Err(Error::UnexpectedActivationDeadline)
        }
        ActivationPolicy::PrepaidLazy if deadline == 0 => Err(Error::MissingActivationDeadline),
        ActivationPolicy::PrepaidLazy
            if quote.rent_principal == 0 && quote.creation_principal == 0 =>
        {
            Err(Error::MissingLazyCreationFunding)
        }
        _ => Ok(()),
    }
}

fn validate_conservation(
    remaining: FundingAmountsV1,
    released: FundingAmountsV1,
    quote: FundingQuoteV1,
) -> Result<()> {
    let compartments = [
        FundingCompartment::Rent,
        FundingCompartment::Creation,
        FundingCompartment::Work,
        FundingCompartment::Provider,
        FundingCompartment::Bounty,
        FundingCompartment::Liquidity,
        FundingCompartment::Service,
    ];
    for compartment in compartments {
        if remaining
            .value(compartment)
            .checked_add(released.value(compartment))
            .ok_or(Error::ArithmeticOverflow)?
            != quote.value(compartment)
        {
            return Err(Error::FundingConservationMismatch);
        }
    }
    if remaining
        .total_principal
        .checked_add(released.total_principal)
        .ok_or(Error::ArithmeticOverflow)?
        != quote.total_principal
    {
        return Err(Error::FundingConservationMismatch);
    }
    Ok(())
}

fn move_compartment(
    remaining: &mut FundingAmountsV1,
    released: &mut FundingAmountsV1,
    compartment: FundingCompartment,
    principal: u64,
) -> Result<()> {
    let new_remaining = remaining
        .value(compartment)
        .checked_sub(principal)
        .ok_or(Error::InsufficientCompartmentPrincipal)?;
    let new_released = released
        .value(compartment)
        .checked_add(principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_remaining = remaining.with_value(compartment, new_remaining)?;
    let next_released = released.with_value(compartment, new_released)?;
    *remaining = next_remaining;
    *released = next_released;
    Ok(())
}

fn checked_sum(values: &[u64]) -> Result<u64> {
    let mut total = 0u64;
    for value in values {
        total = total.checked_add(*value).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(total)
}

fn require_nonzero_identifier(identifier: &[u8; 32]) -> Result<()> {
    if identifier.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(())
}

fn manifest_bytes_for_count(count: usize) -> Result<usize> {
    count
        .checked_mul(CAPABILITY_ENTRY_BYTES)
        .and_then(|entries| entries.checked_add(MANIFEST_HEADER_BYTES))
        .ok_or(Error::ArithmeticOverflow)
}

fn entry_offset(index: usize) -> Result<usize> {
    index
        .checked_mul(CAPABILITY_ENTRY_BYTES)
        .and_then(|offset| offset.checked_add(MANIFEST_HEADER_BYTES))
        .ok_or(Error::ArithmeticOverflow)
}

fn read_content_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array::<32>(bytes, offset)?).map_err(|_| Error::ZeroContentId)
}

fn copy_content_id(output: &mut [u8], offset: usize, id: ContentId) {
    copy_infallible(output, offset, id.as_bytes());
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(bytes, offset, width)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn copy_exact(output: &mut [u8], offset: usize, source: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(source.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(source);
    Ok(())
}

fn copy_infallible(output: &mut [u8], offset: usize, source: &[u8]) {
    if let Some(end) = offset.checked_add(source.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(source);
    }
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(byte) = output.get_mut(offset) {
        *byte = value;
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    let _ = copy_exact(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    let _ = copy_exact(output, offset, &value.to_le_bytes());
}

fn read_bool(values: &[bool], index: usize) -> Result<bool> {
    values.get(index).copied().ok_or(Error::InvalidDependency)
}

fn write_bool(values: &mut [bool], index: usize, value: bool) -> Result<()> {
    let destination = values.get_mut(index).ok_or(Error::InvalidDependency)?;
    *destination = value;
    Ok(())
}

#[cold]
#[cfg(test)]
fn unreachable_total() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        match ContentId::new([byte; 32]) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        }
    }

    fn quote() -> FundingQuoteV1 {
        match FundingQuoteV1::new(10, 20, 30, 40, 50, 60, 70) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        }
    }

    fn entry(kind: u8, policy: ActivationPolicy, dependency: Option<u8>) -> CapabilityEntryV1 {
        entry_with_config(kind, 22, policy, dependency, quote())
    }

    fn entry_with_config(
        kind: u8,
        config: u8,
        policy: ActivationPolicy,
        dependency: Option<u8>,
        funding_quote: FundingQuoteV1,
    ) -> CapabilityEntryV1 {
        let mut dependencies = [0u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        let dependency_count = if let Some(value) = dependency {
            if let Some(slot) = dependencies.get_mut(0) {
                *slot = value;
            }
            1
        } else {
            0
        };
        let deadline = if policy == ActivationPolicy::PrepaidLazy {
            500
        } else {
            0
        };
        match CapabilityEntryV1::new(
            id(kind),
            id(21),
            id(config),
            id(23),
            id(24),
            id(25),
            policy,
            deadline,
            dependency_count,
            dependencies,
            funding_quote,
        ) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        }
    }

    fn manifest<'a>(
        entries: &[CapabilityEntryV1],
        storage: &'a mut [u8],
    ) -> CapabilityManifestV1<'a> {
        match CapabilityManifestV1::encode_into(entries, storage) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        }
    }

    fn mutate(bytes: &mut [u8], offset: usize, value: u8) {
        if let Some(byte) = bytes.get_mut(offset) {
            *byte = value;
        }
    }

    fn entry_at(entries: &[CapabilityEntryV1], index: usize) -> CapabilityEntryV1 {
        match entries.get(index) {
            Some(value) => *value,
            None => unreachable_total(),
        }
    }

    #[test]
    fn canonical_empty_manifest_is_exact() {
        assert_eq!(MARKET_OPENING_READINESS_PDA_DOMAIN.len(), 25);
        assert!(MARKET_OPENING_READINESS_PDA_DOMAIN.len() <= SVM_MAX_PDA_SEED_BYTES);
        let empty = CapabilityManifestV1::empty();
        assert!(empty.is_ok());
        if let Ok(value) = empty {
            assert_eq!(value.entry_count(), 0);
            assert_eq!(value.as_bytes(), &EMPTY_MANIFEST_BYTES);
        }
        let mut trailing = [0u8; MANIFEST_HEADER_BYTES + 1];
        assert!(copy_exact(&mut trailing, 0, &EMPTY_MANIFEST_BYTES).is_ok());
        assert_eq!(
            CapabilityManifestV1::decode(&trailing),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn manifest_round_trip_preserves_open_kind_namespace() {
        let entries = [
            entry(1, ActivationPolicy::RequiredAtFounding, None),
            entry(9, ActivationPolicy::PrepaidLazy, Some(0)),
        ];
        let mut storage = [0xa5u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        let value = manifest(&entries, &mut storage);
        assert_eq!(value.entry_count(), 2);
        assert_eq!(value.entry(0), Ok(entry_at(&entries, 0)));
        assert_eq!(value.entry(1), Ok(entry_at(&entries, 1)));
        assert_eq!(value.as_bytes().len(), storage.len());
    }

    #[test]
    fn founding_config_selection_is_unique_and_never_order_fallback() {
        let entries = [
            entry_with_config(1, 30, ActivationPolicy::RequiredAtFounding, None, quote()),
            entry_with_config(2, 31, ActivationPolicy::PrepaidLazy, Some(0), quote()),
            entry_with_config(
                3,
                31,
                ActivationPolicy::RequiredAtFounding,
                Some(0),
                quote(),
            ),
        ];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let value = manifest(&entries, &mut storage);
        let selected = value.required_founding_entry_for_config(id(31));
        assert!(selected.is_ok());
        if let Ok(value) = selected {
            assert_eq!(value.index(), 2);
            assert_eq!(value.entry(), entry_at(&entries, 2));
        }
        assert_eq!(
            value.required_founding_entry_for_config(id(32)),
            Err(Error::RequiredFoundingConfigMissing)
        );

        let ambiguous = [
            entry_with_config(1, 40, ActivationPolicy::RequiredAtFounding, None, quote()),
            entry_with_config(2, 40, ActivationPolicy::RequiredAtFounding, None, quote()),
        ];
        let mut ambiguous_storage = [0u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&ambiguous, &mut ambiguous_storage);
        assert_eq!(
            manifest.required_founding_entry_for_config(id(40)),
            Err(Error::RequiredFoundingConfigAmbiguous)
        );
    }

    #[test]
    fn one_shot_resolution_fund_quote_has_only_held_compartments() {
        let fund_quote = match FundingQuoteV1::new(100, 0, 0, 40, 50, 0, 0) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        };
        let entries = [entry_with_config(
            1,
            44,
            ActivationPolicy::RequiredAtFounding,
            None,
            fund_quote,
        )];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let selected =
            match manifest(&entries, &mut storage).required_founding_entry_for_config(id(44)) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        assert_eq!(
            selected.validate_one_shot_resolution_fund_quote(100),
            Ok(fund_quote)
        );
        assert_eq!(
            selected.validate_one_shot_resolution_fund_quote(99),
            Err(Error::ResolutionFundRentMismatch)
        );

        let invalid_quotes = [
            (
                FundingQuoteV1::new(100, 0, 0, 40, 0, 0, 0),
                Error::MissingResolutionFundBounty,
            ),
            (
                FundingQuoteV1::new(100, 1, 0, 40, 50, 0, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                FundingQuoteV1::new(100, 0, 1, 40, 50, 0, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                FundingQuoteV1::new(100, 0, 0, 40, 50, 1, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                FundingQuoteV1::new(100, 0, 0, 40, 50, 0, 1),
                Error::ExtraneousResolutionFundPrincipal,
            ),
        ];
        for (candidate, expected) in invalid_quotes {
            let candidate = match candidate {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
            let entry =
                entry_with_config(1, 44, ActivationPolicy::RequiredAtFounding, None, candidate);
            let mut candidate_storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
            let selected = match manifest(&[entry], &mut candidate_storage)
                .required_founding_entry_for_config(id(44))
            {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
            assert_eq!(
                selected.validate_one_shot_resolution_fund_quote(100),
                Err(expected)
            );
        }
    }

    #[test]
    fn manifest_refuses_order_alias_and_cycles() {
        let descending = [
            entry(2, ActivationPolicy::RequiredAtFounding, None),
            entry(1, ActivationPolicy::RequiredAtFounding, None),
        ];
        let mut storage = [0xa5u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        assert_eq!(
            CapabilityManifestV1::encode_into(&descending, &mut storage),
            Err(Error::NonCanonicalEntryOrder)
        );
        assert!(storage.iter().all(|byte| *byte == 0xa5));
        let duplicate = [
            entry(1, ActivationPolicy::RequiredAtFounding, None),
            entry(1, ActivationPolicy::RequiredAtFounding, None),
        ];
        assert_eq!(
            CapabilityManifestV1::encode_into(&duplicate, &mut storage),
            Err(Error::NonCanonicalEntryOrder)
        );
        let cycle = [
            entry(1, ActivationPolicy::RequiredAtFounding, Some(1)),
            entry(2, ActivationPolicy::RequiredAtFounding, Some(0)),
        ];
        assert_eq!(
            CapabilityManifestV1::encode_into(&cycle, &mut storage),
            Err(Error::CyclicDependencies)
        );
    }

    #[test]
    fn dependencies_are_bounded_sorted_and_in_manifest() {
        let mut dependencies = [0u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        mutate(&mut dependencies, 0, 2);
        mutate(&mut dependencies, 1, 1);
        assert_eq!(
            CapabilityEntryV1::new(
                id(3),
                id(4),
                id(5),
                id(6),
                id(7),
                id(8),
                ActivationPolicy::RequiredAtFounding,
                0,
                2,
                dependencies,
                quote(),
            ),
            Err(Error::NonCanonicalDependencyOrder)
        );
        let entries = [entry(1, ActivationPolicy::RequiredAtFounding, Some(1))];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        assert_eq!(
            CapabilityManifestV1::encode_into(&entries, &mut storage),
            Err(Error::InvalidDependency)
        );
    }

    #[test]
    fn entry_refuses_noncanonical_padding_and_lazy_underfunding() {
        let encoded = entry(1, ActivationPolicy::RequiredAtFounding, None).to_bytes();
        let mut hostile = encoded;
        mutate(&mut hostile, DEPENDENCIES_OFFSET + 1, 7);
        assert_eq!(
            CapabilityEntryV1::decode(&hostile),
            Err(Error::NonCanonicalReservedBytes)
        );
        let zero_quote = match FundingQuoteV1::new(0, 0, 1, 0, 0, 0, 0) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        };
        assert_eq!(
            CapabilityEntryV1::new(
                id(1),
                id(2),
                id(3),
                id(4),
                id(5),
                id(6),
                ActivationPolicy::PrepaidLazy,
                5,
                0,
                [0u8; MAX_DEPENDENCIES_PER_CAPABILITY],
                zero_quote,
            ),
            Err(Error::MissingLazyCreationFunding)
        );
        assert_eq!(
            CapabilityEntryV1::new(
                id(1),
                id(2),
                id(3),
                id(4),
                id(5),
                id(6),
                ActivationPolicy::PrepaidLazy,
                0,
                0,
                [0u8; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote(),
            ),
            Err(Error::MissingActivationDeadline)
        );
    }

    #[test]
    fn hostile_headers_and_zero_ids_are_refused() {
        let mut bytes = EMPTY_MANIFEST_BYTES;
        mutate(&mut bytes, MANIFEST_RESERVED_OFFSET, 1);
        assert_eq!(
            CapabilityManifestV1::decode(&bytes),
            Err(Error::NonCanonicalReservedBytes)
        );
        let identity_offsets = [
            KIND_ID_OFFSET,
            RELEASE_ID_OFFSET,
            CONFIG_ID_OFFSET,
            CAPACITY_PROFILE_ID_OFFSET,
            CHILD_SCHEMA_ID_OFFSET,
            CHILD_DERIVATION_ID_OFFSET,
        ];
        for offset in identity_offsets {
            let mut entry_bytes = entry(1, ActivationPolicy::RequiredAtFounding, None).to_bytes();
            if let Some(identity) = entry_bytes.get_mut(offset..offset + 32) {
                identity.fill(0);
            }
            assert_eq!(
                CapabilityEntryV1::decode(&entry_bytes),
                Err(Error::ZeroContentId)
            );
        }
    }

    #[test]
    fn funding_totals_are_checked_and_canonical() {
        assert_eq!(
            FundingQuoteV1::new(u64::MAX, 1, 0, 0, 0, 0, 0),
            Err(Error::ArithmeticOverflow)
        );
        let mut encoded = quote().to_bytes();
        mutate(&mut encoded, FUNDING_TOTAL_OFFSET, 0);
        assert_eq!(
            FundingQuoteV1::decode(&encoded),
            Err(Error::FundingTotalMismatch)
        );
    }

    #[test]
    fn funding_is_prepaid_activated_and_conserved() {
        let entries = [entry(1, ActivationPolicy::PrepaidLazy, None)];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&entries, &mut storage);
        let manifest_id = id(99);
        assert_eq!(
            FundingStateV1::new(manifest_id, manifest, 0, quote().total_principal() - 1),
            Err(Error::PresentPrincipalMismatch)
        );
        let mut state =
            match FundingStateV1::new(manifest_id, manifest, 0, quote().total_principal()) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        assert!(
            state
                .validate_market_open(manifest_id, manifest, quote().total_principal(), 400,)
                .is_ok()
        );
        let debit = state.activate(manifest_id, manifest, quote().total_principal(), 500);
        assert_eq!(
            debit,
            Ok(ActivationDebitV1 {
                rent_principal: 10,
                creation_principal: 20,
            })
        );
        assert!(
            state
                .validate_against(manifest_id, manifest, quote().total_principal() - 30,)
                .is_ok()
        );
        assert!(
            state
                .release(
                    manifest_id,
                    manifest,
                    quote().total_principal() - 30,
                    FundingCompartment::Provider,
                    40,
                )
                .is_ok()
        );
        assert_eq!(
            state.release(
                manifest_id,
                manifest,
                quote().total_principal() - 70,
                FundingCompartment::Provider,
                1,
            ),
            Err(Error::InsufficientCompartmentPrincipal)
        );
        assert_eq!(
            state.release(
                manifest_id,
                manifest,
                quote().total_principal() - 70,
                FundingCompartment::Rent,
                1,
            ),
            Err(Error::ActivationCompartmentRequired)
        );
        assert!(
            state
                .validate_against(manifest_id, manifest, quote().total_principal() - 70,)
                .is_ok()
        );
        let round_trip = FundingStateV1::decode(&state.to_bytes());
        assert_eq!(round_trip, Ok(state));
    }

    #[test]
    fn founding_and_deadline_rules_are_exact() {
        let founding = entry(1, ActivationPolicy::RequiredAtFounding, None);
        let lazy = entry(2, ActivationPolicy::PrepaidLazy, None);
        let entries = [founding, lazy];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&entries, &mut storage);
        let mut founding_state =
            match FundingStateV1::new(id(99), manifest, 0, quote().total_principal()) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        assert_eq!(
            founding_state.validate_market_open(id(99), manifest, quote().total_principal(), 1,),
            Err(Error::FoundingCapabilityInactive)
        );
        assert!(
            founding_state
                .activate(id(99), manifest, quote().total_principal(), 1)
                .is_ok()
        );
        assert!(
            founding_state
                .validate_market_open(id(99), manifest, quote().total_principal() - 30, 1,)
                .is_ok()
        );
        let mut lazy_state =
            match FundingStateV1::new(id(99), manifest, 1, quote().total_principal()) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        let before = lazy_state;
        assert_eq!(
            lazy_state.activate(id(99), manifest, quote().total_principal(), 501),
            Err(Error::ActivationDeadlineElapsed)
        );
        assert_eq!(lazy_state, before);
        assert_eq!(
            lazy_state.validate_market_open(id(99), manifest, quote().total_principal(), 501,),
            Err(Error::ActivationDeadlineElapsed)
        );
    }

    #[test]
    fn funding_state_hostile_encoding_is_refused() {
        let entries = [entry(1, ActivationPolicy::RequiredAtFounding, None)];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&entries, &mut storage);
        let state = match FundingStateV1::new(id(99), manifest, 0, quote().total_principal()) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        };
        let mut bytes = state.to_bytes();
        mutate(&mut bytes, STATE_HEADER_RESERVED_OFFSET, 1);
        assert_eq!(
            FundingStateV1::decode(&bytes),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut wrong_total = state.to_bytes();
        mutate(
            &mut wrong_total,
            STATE_REMAINING_OFFSET + FUNDING_TOTAL_OFFSET,
            0,
        );
        assert_eq!(
            FundingStateV1::decode(&wrong_total),
            Err(Error::FundingTotalMismatch)
        );

        let mut conserved_lie = state.to_bytes();
        put_u64(
            &mut conserved_lie,
            STATE_REMAINING_OFFSET + FUNDING_WORK_OFFSET,
            quote().work_principal() - 1,
        );
        put_u64(
            &mut conserved_lie,
            STATE_REMAINING_OFFSET + FUNDING_TOTAL_OFFSET,
            quote().total_principal() - 1,
        );
        let decoded = match FundingStateV1::decode(&conserved_lie) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        };
        assert_eq!(
            decoded.validate_against(id(99), manifest, quote().total_principal() - 1),
            Err(Error::FundingConservationMismatch)
        );
    }

    #[test]
    fn readiness_advances_only_canonical_actual_funding_and_opens_only_when_derived_ready() {
        let entries = [
            entry(1, ActivationPolicy::RequiredAtFounding, None),
            entry(2, ActivationPolicy::PrepaidLazy, None),
        ];
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&entries, &mut storage);
        let manifest_id = id(99);
        let mut readiness =
            match MarketOpeningReadinessV1::begin([7; 32], 3, manifest_id, manifest, [8; 32]) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        let initial = readiness;
        let mut founding =
            match FundingStateV1::new(manifest_id, manifest, 0, quote().total_principal()) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        assert_eq!(
            readiness.advance(
                [7; 32],
                3,
                manifest_id,
                manifest,
                0,
                founding,
                quote().total_principal(),
                1,
            ),
            Err(Error::FoundingCapabilityInactive)
        );
        assert_eq!(readiness, initial);
        assert_eq!(
            readiness.advance(
                [7; 32],
                3,
                manifest_id,
                manifest,
                1,
                founding,
                quote().total_principal(),
                1,
            ),
            Err(Error::ReadinessIndexMismatch)
        );
        assert_eq!(readiness, initial);
        assert!(
            founding
                .activate(manifest_id, manifest, quote().total_principal(), 1)
                .is_ok()
        );
        assert_eq!(
            readiness.advance(
                [7; 32],
                3,
                manifest_id,
                manifest,
                0,
                founding,
                quote().total_principal() - 31,
                1,
            ),
            Err(Error::PresentPrincipalMismatch)
        );
        assert_eq!(readiness, initial);
        assert!(
            readiness
                .advance(
                    [7; 32],
                    3,
                    manifest_id,
                    manifest,
                    0,
                    founding,
                    quote().total_principal() - 30,
                    1,
                )
                .is_ok()
        );
        assert_eq!(readiness.next_entry_index(), 1);
        assert_eq!(
            readiness.require_ready_for_open([7; 32], 3, manifest_id, manifest),
            Err(Error::ReadinessIncomplete)
        );
        let lazy = match FundingStateV1::new(manifest_id, manifest, 1, quote().total_principal()) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        };
        assert_eq!(
            readiness.advance(
                [7; 32],
                3,
                manifest_id,
                manifest,
                0,
                lazy,
                quote().total_principal(),
                400,
            ),
            Err(Error::ReadinessIndexMismatch)
        );
        assert!(
            readiness
                .advance(
                    [7; 32],
                    3,
                    manifest_id,
                    manifest,
                    1,
                    lazy,
                    quote().total_principal(),
                    400,
                )
                .is_ok()
        );
        assert!(readiness.is_ready());
        assert!(
            readiness
                .require_ready_for_open([7; 32], 3, manifest_id, manifest)
                .is_ok()
        );
        assert_eq!(
            readiness.advance(
                [7; 32],
                3,
                manifest_id,
                manifest,
                2,
                lazy,
                quote().total_principal(),
                400,
            ),
            Err(Error::ReadinessIndexMismatch)
        );
        assert_eq!(
            readiness.require_ready_for_open([6; 32], 3, manifest_id, manifest),
            Err(Error::ReadinessBindingMismatch)
        );
        assert_eq!(
            readiness.require_ready_for_open([7; 32], 3, id(98), manifest),
            Err(Error::ReadinessBindingMismatch)
        );
        let one_entry = [entry(1, ActivationPolicy::RequiredAtFounding, None)];
        let mut shorter_storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let shorter_manifest =
            match CapabilityManifestV1::encode_into(&one_entry, &mut shorter_storage) {
                Ok(value) => value,
                Err(_) => unreachable_total(),
            };
        assert_eq!(
            readiness.require_ready_for_open([7; 32], 3, manifest_id, shorter_manifest),
            Err(Error::ReadinessBindingMismatch)
        );
        let mut malformed_manifest = [0u8; MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES];
        malformed_manifest.copy_from_slice(manifest.as_bytes());
        mutate(&mut malformed_manifest, MANIFEST_RESERVED_OFFSET, 1);
        assert_eq!(
            CapabilityManifestV1::decode(&malformed_manifest),
            Err(Error::NonCanonicalReservedBytes)
        );
        assert_eq!(
            MarketOpeningReadinessV1::decode(&readiness.to_bytes()),
            Ok(readiness)
        );
        let mut hostile = readiness.to_bytes();
        mutate(&mut hostile, READINESS_BODY_RESERVED_OFFSET, 1);
        assert_eq!(
            MarketOpeningReadinessV1::decode(&hostile),
            Err(Error::NonCanonicalReservedBytes)
        );
    }
}
