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

/// Typed funding quotes, custody observations, PDA projections, and transitions.
pub mod funding;
/// Lean-emitted byte coordinates for the `DCLTCAP1` manifest, the `DCLTFQ01`
/// typed funding quote, the `DCLTCFS1` funding state, and the `DCLTMOR1`
/// opening-readiness record.
///
/// This module is the crate's single authority for every width, offset, magic
/// and seed domain in the family; the constants below are projections of it.
/// Some coordinates this crate does not itself read are still emitted, because
/// the same Lean schema also drives the browser's shared manifest decoder.
#[allow(missing_docs, dead_code)]
pub(crate) mod generated_abi;
/// SDK-free readiness account frames and state-transition planning.
pub mod readiness_frame;
/// Exact readiness instruction wires without SVM dependencies.
pub mod readiness_instruction;
/// Reusable capability templates and exact occurrence-manifest projection.
pub mod template;

pub use funding::*;
pub use template::*;

pub use dclutch_core_contract::ContentId;

/// Exact manifest header width.
pub const MANIFEST_HEADER_BYTES: usize = generated_abi::CAPABILITY_MANIFEST_HEADER_BYTES_V1;
/// Exact profile-1 capability-entry width.
pub const CAPABILITY_ENTRY_BYTES: usize = generated_abi::CAPABILITY_ENTRY_BYTES_V1;
/// Exact transient Market-opening readiness width.
pub const MARKET_OPENING_READINESS_BYTES: usize = generated_abi::MARKET_OPENING_READINESS_BYTES_V1;
/// Chain-derived maximum byte width of one Solana PDA seed component.
pub const SVM_MAX_PDA_SEED_BYTES: usize = 32;
/// Maximum profile-1 manifest byte width.
pub const MAX_MANIFEST_BYTES: usize = generated_abi::CAPABILITY_MANIFEST_MAX_BYTES_V1;

/// Provisional artifact-profile bound on capability entries.
///
/// This is neither a mathematical nor a product limit. The lifting plan is a
/// new manifest artifact profile with a wider layout, preserving profile-1
/// content identities and founding new Markets against the wider preimage.
pub const MAX_CAPABILITIES: usize = generated_abi::MAX_CAPABILITIES_V1;
/// Provisional artifact-profile bound on dependencies per entry.
///
/// Dependencies are sorted entry indices rather than globally assigned bits.
/// A later artifact profile may widen this array without introducing a closed
/// capability-kind registry.
pub const MAX_DEPENDENCIES_PER_CAPABILITY: usize =
    generated_abi::MAX_DEPENDENCIES_PER_CAPABILITY_V1;

/// Canonical manifest magic.
pub const MANIFEST_MAGIC: [u8; 8] = generated_abi::CAPABILITY_MANIFEST_MAGIC_V1;
/// Implemented manifest schema version.
pub const MANIFEST_SCHEMA_VERSION: u16 = generated_abi::CAPABILITY_MANIFEST_SCHEMA_VERSION_V1;
/// Opaque schema/validator-release identity for canonical profile-1 manifests.
///
/// The SVM adapter and offchain operators share this semantic owner when they
/// derive finalized raw-record addresses. It is not a deployed-program or ELF
/// identity.
pub const CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-manifest-profile-1-v1";
/// SHA-256 identity of [`CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x6b, 0xce, 0xf7, 0xb2, 0x83, 0x67, 0xcb, 0x8d, 0x08, 0x97, 0x10, 0xba, 0x58, 0xe6, 0x84, 0x31,
    0x2f, 0x43, 0x4c, 0x4b, 0xc4, 0x20, 0xee, 0xfd, 0x0f, 0x7a, 0x15, 0x0a, 0x90, 0x82, 0x88, 0xdf,
];
/// Implemented provisional artifact profile.
pub const ARTIFACT_PROFILE_V1: u16 = generated_abi::CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1;
/// Canonical transient Market-opening readiness magic.
pub const MARKET_OPENING_READINESS_MAGIC: [u8; 8] =
    generated_abi::MARKET_OPENING_READINESS_MAGIC_V1;
/// Implemented Market-opening readiness schema.
pub const MARKET_OPENING_READINESS_SCHEMA_VERSION: u16 =
    generated_abi::MARKET_OPENING_READINESS_SCHEMA_VERSION_V1;
/// Adapter PDA seed domain for one transient Market-opening readiness child.
///
/// This crate derives no Solana address. The adapter derives it from this
/// domain plus exact Market key and generation, then authenticates the record.
pub const MARKET_OPENING_READINESS_PDA_DOMAIN: &[u8] =
    generated_abi::MARKET_OPENING_READINESS_PDA_DOMAIN_V1;

/// The exact canonical empty-manifest preimage.
pub const EMPTY_MANIFEST_BYTES: [u8; MANIFEST_HEADER_BYTES] = [
    b'D', b'C', b'L', b'T', b'C', b'A', b'P', b'1', 1, 0, 1, 0, 0, 0, 0, 0,
];

const MANIFEST_SCHEMA_OFFSET: usize = generated_abi::CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1;
const MANIFEST_PROFILE_OFFSET: usize = generated_abi::CAPABILITY_MANIFEST_PROFILE_OFFSET_V1;
const MANIFEST_COUNT_OFFSET: usize = generated_abi::CAPABILITY_MANIFEST_COUNT_OFFSET_V1;
const MANIFEST_RESERVED_OFFSET: usize = generated_abi::CAPABILITY_MANIFEST_RESERVED_OFFSET_V1;
const MANIFEST_RESERVED_BYTES: usize = generated_abi::CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1;

const KIND_ID_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_KIND_ID_OFFSET_V1;
const RELEASE_ID_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1;
const CONFIG_ID_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1;
const CAPACITY_PROFILE_ID_OFFSET: usize =
    generated_abi::CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1;
const CHILD_SCHEMA_ID_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1;
const CHILD_DERIVATION_ID_OFFSET: usize =
    generated_abi::CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1;
const ACTIVATION_POLICY_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1;
const DEPENDENCY_COUNT_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1;
const ENTRY_RESERVED_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_RESERVED_OFFSET_V1;
const ENTRY_RESERVED_BYTES: usize = generated_abi::CAPABILITY_ENTRY_RESERVED_BYTES_V1;
const ACTIVATION_DEADLINE_OFFSET: usize =
    generated_abi::CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1;
const DEPENDENCIES_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1;
const QUOTE_OFFSET: usize = generated_abi::CAPABILITY_ENTRY_QUOTE_OFFSET_V1;

const READINESS_SCHEMA_OFFSET: usize = generated_abi::READINESS_SCHEMA_OFFSET_V1;
const READINESS_RESERVED_OFFSET: usize = generated_abi::READINESS_RESERVED_OFFSET_V1;
const READINESS_RESERVED_BYTES: usize = generated_abi::READINESS_HEADER_RESERVED_BYTES_V1;
const READINESS_MARKET_OFFSET: usize = generated_abi::READINESS_MARKET_OFFSET_V1;
const READINESS_GENERATION_OFFSET: usize = generated_abi::READINESS_GENERATION_OFFSET_V1;
const READINESS_MANIFEST_OFFSET: usize = generated_abi::READINESS_MANIFEST_OFFSET_V1;
const READINESS_ENTRY_COUNT_OFFSET: usize = generated_abi::READINESS_ENTRY_COUNT_OFFSET_V1;
const READINESS_NEXT_ENTRY_OFFSET: usize = generated_abi::READINESS_NEXT_ENTRY_OFFSET_V1;
const READINESS_BODY_RESERVED_OFFSET: usize = generated_abi::READINESS_BODY_RESERVED_OFFSET_V1;
const READINESS_BODY_RESERVED_BYTES: usize = generated_abi::READINESS_BODY_RESERVED_BYTES_V1;
const READINESS_RENT_REFUND_OFFSET: usize = generated_abi::READINESS_RENT_REFUND_OFFSET_V1;

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
    /// Summing exact amounts within one physical asset class overflowed `u64`.
    ArithmeticOverflow,
    /// An asset-class tag was unknown.
    UnknownFundingAssetClass,
    /// Zero or N/A used a noncanonical asset-class representation.
    NonCanonicalFundingAssetClass,
    /// Rent or Creation was not N/A or native lamports.
    InvalidCompartmentAssetClass,
    /// An encoded per-asset total did not equal its exact compartment sum.
    FundingAssetTotalMismatch,
    /// Realm collateral was quoted without its immutable binding.
    MissingRealmCollateralBinding,
    /// A Realm binding existed although no Realm collateral was quoted.
    UnexpectedRealmCollateralBinding,
    /// Observed Realm, release, token program, or mint differed from the quote.
    RealmCollateralBindingMismatch,
    /// A funding-state status byte was unknown.
    UnknownFundingStatus,
    /// Funding state did not bind to the supplied manifest entry.
    FundingBindingMismatch,
    /// Present observed native lamports did not equal remaining lamports.
    PresentNativeLamportsMismatch,
    /// Present observed Realm tokens did not equal remaining Realm collateral.
    PresentRealmCollateralMismatch,
    /// A quote requiring Realm collateral had no authenticated token vault.
    MissingRealmCollateralVault,
    /// A native-only quote was paired with a Realm token vault.
    UnexpectedRealmCollateralVault,
    /// An observed token authority was not the canonical funding-authority PDA.
    FundingAuthorityMismatch,
    /// An observed token vault was not the canonical funding-vault PDA.
    FundingVaultMismatch,
    /// Physical custody did not contain its exact Rent or semantic principal.
    UnderfundedPhysicalCustody,
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
    /// A capability-template configuration projection selector was unknown.
    UnknownConfigProjection,
    /// A template encoded a static or occurrence-bound configuration noncanonically.
    NonCanonicalConfigProjection,
    /// No unique founding-required entry projected the occurrence resolution material.
    RequiredOccurrenceProjectionMissing,
    /// More than one founding-required entry projected the occurrence resolution material.
    RequiredOccurrenceProjectionAmbiguous,
    /// A supplied manifest was not the exact projection of its authenticated template.
    ProjectedManifestMismatch,
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
    /// Return the immutable typed funding quote.
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
        let amounts = quote.amounts();
        if amounts.rent().asset_class() != FundingAssetClassV1::NativeLamports
            || amounts.rent().amount() != exact_fund_rent
        {
            return Err(Error::ResolutionFundRentMismatch);
        }
        if amounts.bounty().asset_class() != FundingAssetClassV1::NativeLamports
            || amounts.bounty().amount() == 0
        {
            return Err(Error::MissingResolutionFundBounty);
        }
        if quote.realm_collateral().is_some()
            || amounts.creation().amount() != 0
            || amounts.work().amount() != 0
            || amounts.liquidity().amount() != 0
            || amounts.service().amount() != 0
            || !matches!(
                amounts.provider().asset_class(),
                FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
            )
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
        custody: FundingCustodyObservationV1,
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
        funding.validate_market_open(manifest_content_id, manifest, custody, current_slot)?;
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
            if quote.amounts().rent().amount() == 0 && quote.amounts().creation().amount() == 0 =>
        {
            Err(Error::MissingLazyCreationFunding)
        }
        _ => Ok(()),
    }
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
        match native_quote(10, 20, 30, 40, 50, 60, 70) {
            Ok(value) => value,
            Err(_) => unreachable_total(),
        }
    }

    fn native_amount(value: u64) -> Result<CompartmentFundingV1> {
        if value == 0 {
            Ok(CompartmentFundingV1::not_applicable())
        } else {
            CompartmentFundingV1::native_lamports(value)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn native_quote(
        rent: u64,
        creation: u64,
        work: u64,
        provider: u64,
        bounty: u64,
        liquidity: u64,
        service: u64,
    ) -> Result<FundingQuoteV1> {
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                native_amount(rent)?,
                native_amount(creation)?,
                native_amount(work)?,
                native_amount(provider)?,
                native_amount(bounty)?,
                native_amount(liquidity)?,
                native_amount(service)?,
            )?,
            None,
        )
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
        let fund_quote = match native_quote(100, 0, 0, 40, 50, 0, 0) {
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
                native_quote(100, 0, 0, 40, 0, 0, 0),
                Error::MissingResolutionFundBounty,
            ),
            (
                native_quote(100, 1, 0, 40, 50, 0, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                native_quote(100, 0, 1, 40, 50, 0, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                native_quote(100, 0, 0, 40, 50, 1, 0),
                Error::ExtraneousResolutionFundPrincipal,
            ),
            (
                native_quote(100, 0, 0, 40, 50, 0, 1),
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
        let zero_quote = match native_quote(0, 0, 1, 0, 0, 0, 0) {
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
}
