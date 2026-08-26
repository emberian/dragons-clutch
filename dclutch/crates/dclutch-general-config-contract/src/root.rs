//! Minimal mutable General root-state tail and activation planning.
//!
//! One Trading-owned account contains the immutable common capability-root
//! header followed by this 128-byte tail. The tail retains one configuration
//! fact: the content identity of the exact [`crate::GeneralConfigV2`] preimage.
//! Every hot caller must supply that preimage, hash it at the physical
//! boundary, and call
//! [`GeneralRootV2::require_hot_context`] before using any configuration field.

use dclutch_capability_contract::{
    ActivationPolicy, CapabilityManifestV1, FundingCustodyObservationV1, FundingStateV1,
    FundingStatus,
};
use dclutch_core_contract::{ContentId, MarketRoot, Phase};

use crate::GeneralConfigV2;

/// Exact canonical byte width of the mutable [`GeneralRootV2`] tail.
pub const GENERAL_ROOT_BYTES_V2: usize = crate::generated::GENERAL_ROOT_BYTES_V2;
/// Domain preimage for the General capability-kind identity.
pub const GENERAL_CAPABILITY_KIND_PREIMAGE_V1: &[u8] = b"dclutch/general/capability-kind/v1";
/// SHA-256 of [`GENERAL_CAPABILITY_KIND_PREIMAGE_V1`].
pub const GENERAL_CAPABILITY_KIND_ID_V1: [u8; 32] = [
    0xcb, 0x8b, 0xc8, 0x7d, 0xbb, 0xf9, 0xee, 0x58, 0xfc, 0xe8, 0x60, 0x01, 0xa1, 0xbb, 0x5d, 0x1f,
    0x2a, 0xf5, 0x7c, 0xec, 0x1d, 0xee, 0xff, 0x86, 0x66, 0x10, 0xca, 0x7f, 0xc4, 0x27, 0x4c, 0xb5,
];
/// Schema label for the descriptor-selected mutable General root-state tail.
pub const GENERAL_ROOT_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/general-root-v2";
/// SHA-256 of [`GENERAL_ROOT_SCHEMA_PREIMAGE_V2`].
pub const GENERAL_ROOT_SCHEMA_ID_V2: [u8; 32] = [
    0xb9, 0x45, 0x37, 0xe8, 0xb1, 0xb6, 0xbd, 0x12, 0x52, 0x90, 0x5a, 0xf7, 0xef, 0x7d, 0xa6, 0x04,
    0x70, 0x81, 0xed, 0x19, 0xd3, 0xe2, 0xae, 0x8c, 0xef, 0x52, 0xe3, 0xe7, 0x60, 0xba, 0x74, 0x9d,
];

/// Persistent General-root lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralLifecycleV2 {
    /// Batches may be opened and settled.
    Active = 1,
    /// No new batch may open; existing batches may finish.
    Retiring = 2,
    /// No batch remains; the root is retained only as replay evidence.
    Retired = 3,
}

impl GeneralLifecycleV2 {
    fn decode(value: u8) -> RootResult<Self> {
        match value {
            1 => Ok(Self::Active),
            2 => Ok(Self::Retiring),
            3 => Ok(Self::Retired),
            _ => Err(RootError::UnknownLifecycle),
        }
    }
}

/// Explicit refusal from a hostile root or activation input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootError {
    /// A root byte slice had another width.
    InvalidLength,
    /// Magic or schema version selected another layout.
    UnsupportedSchema,
    /// Reserved bytes were not canonically zero.
    NonCanonicalReservedBytes,
    /// A required Market/config identity or exact Rent amount was zero.
    ZeroIdentity,
    /// The lifecycle byte was unknown.
    UnknownLifecycle,
    /// A requested lifecycle operation was not admitted.
    InvalidLifecycle,
    /// A Market, generation, config, revision, or sequence guard differed.
    CoordinateMismatch,
    /// A checked replay coordinate overflowed or underflowed.
    Arithmetic,
    /// Retirement was attempted with live batches.
    OutstandingBatches,
    /// Config bytes or immutable config coordinates refused.
    Config(crate::Error),
    /// The canonical Core Market contract refused.
    Core(dclutch_core_contract::Error),
    /// The canonical capability manifest or funding contract refused.
    Capability(dclutch_capability_contract::Error),
    /// No exact General capability entry was present.
    CapabilityMissing,
    /// More than one exact General capability entry was present.
    CapabilityAmbiguous,
    /// An exact entry selected another descriptor, config, capacity, or root-state schema.
    CapabilityMismatch,
    /// Market phase did not admit the entry's immutable activation policy.
    ActivationPhaseMismatch,
    /// Capability activation did not release exactly root rent and zero creation funding.
    ActivationFundingMismatch,
    /// An existing root differed from the exact idempotent activation result.
    ActivationReplayMismatch,
}

impl From<crate::Error> for RootError {
    fn from(value: crate::Error) -> Self {
        Self::Config(value)
    }
}

impl From<dclutch_core_contract::Error> for RootError {
    fn from(value: dclutch_core_contract::Error) -> Self {
        Self::Core(value)
    }
}

impl From<dclutch_capability_contract::Error> for RootError {
    fn from(value: dclutch_capability_contract::Error) -> Self {
        Self::Capability(value)
    }
}

/// Result alias for General-root operations.
pub type RootResult<T> = core::result::Result<T, RootError>;

/// Minimal mutable General state appended to the immutable Trading root header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRootV2 {
    lifecycle: GeneralLifecycleV2,
    market: [u8; 32],
    config_id: [u8; 32],
    generation: u64,
    revision: u64,
    next_batch_sequence: u64,
    open_batches: u64,
}

impl GeneralRootV2 {
    /// Construct the one initial active root.
    pub fn active(market: [u8; 32], config_id: [u8; 32], generation: u64) -> RootResult<Self> {
        let value = Self {
            lifecycle: GeneralLifecycleV2::Active,
            market,
            config_id,
            generation,
            revision: 1,
            next_batch_sequence: 0,
            open_batches: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact root.
    pub fn decode(bytes: &[u8]) -> RootResult<Self> {
        if bytes.len() != GENERAL_ROOT_BYTES_V2 {
            return Err(RootError::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != crate::generated::GENERAL_ROOT_MAGIC_V2
            || read_u16(bytes, crate::generated::ROOT_VERSION_OFFSET)?
                != crate::generated::ABI_VERSION_V2
        {
            return Err(RootError::UnsupportedSchema);
        }
        require_zero(bytes, crate::generated::ROOT_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(bytes, crate::generated::ROOT_RESERVED_TAIL_OFFSET, 16)?;
        let value = Self {
            lifecycle: GeneralLifecycleV2::decode(read_u8(
                bytes,
                crate::generated::ROOT_LIFECYCLE_OFFSET,
            )?)?,
            market: read_array(bytes, crate::generated::ROOT_MARKET_OFFSET)?,
            config_id: read_array(bytes, crate::generated::ROOT_CONFIG_ID_OFFSET)?,
            generation: read_u64(bytes, crate::generated::ROOT_GENERATION_OFFSET)?,
            revision: read_u64(bytes, crate::generated::ROOT_REVISION_OFFSET)?,
            next_batch_sequence: read_u64(
                bytes,
                crate::generated::ROOT_NEXT_BATCH_SEQUENCE_OFFSET,
            )?,
            open_batches: read_u64(bytes, crate::generated::ROOT_OPEN_BATCHES_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact Lean-owned root layout.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_ROOT_BYTES_V2] {
        let mut output = [0_u8; GENERAL_ROOT_BYTES_V2];
        put(&mut output, 0, &crate::generated::GENERAL_ROOT_MAGIC_V2);
        put(
            &mut output,
            crate::generated::ROOT_VERSION_OFFSET,
            &crate::generated::ABI_VERSION_V2.to_le_bytes(),
        );
        put(
            &mut output,
            crate::generated::ROOT_LIFECYCLE_OFFSET,
            &[self.lifecycle as u8],
        );
        put(
            &mut output,
            crate::generated::ROOT_MARKET_OFFSET,
            &self.market,
        );
        put(
            &mut output,
            crate::generated::ROOT_CONFIG_ID_OFFSET,
            &self.config_id,
        );
        for (offset, value) in [
            (crate::generated::ROOT_GENERATION_OFFSET, self.generation),
            (crate::generated::ROOT_REVISION_OFFSET, self.revision),
            (
                crate::generated::ROOT_NEXT_BATCH_SEQUENCE_OFFSET,
                self.next_batch_sequence,
            ),
            (
                crate::generated::ROOT_OPEN_BATCHES_OFFSET,
                self.open_batches,
            ),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Validate all canonical cross-field invariants.
    pub fn validate(self) -> RootResult<()> {
        if is_zero(&self.market) || is_zero(&self.config_id) {
            return Err(RootError::ZeroIdentity);
        }
        if self.revision == 0 {
            return Err(RootError::CoordinateMismatch);
        }
        if self.lifecycle == GeneralLifecycleV2::Retired && self.open_batches != 0 {
            return Err(RootError::OutstandingBatches);
        }
        Ok(())
    }

    /// Require the exact root/config binding before consuming config fields.
    ///
    /// `authenticated_config_id` must be SHA-256 of `config.to_bytes()` as
    /// computed by the physical adapter from the supplied immutable account.
    pub fn require_hot_context(
        self,
        market_key: [u8; 32],
        market_generation: u64,
        market_claim_basis_id: [u8; 32],
        authenticated_config_id: [u8; 32],
        config: GeneralConfigV2,
        expected_revision: u64,
    ) -> RootResult<()> {
        if self.lifecycle != GeneralLifecycleV2::Active
            || self.market != market_key
            || self.config_id != authenticated_config_id
            || self.generation != market_generation
            || self.revision != expected_revision
        {
            return Err(RootError::CoordinateMismatch);
        }
        config
            .require_market_coordinates(market_generation, market_claim_basis_id)
            .map_err(RootError::from)
    }

    /// Open one new batch under exact revision and sequence replay guards.
    pub fn open_batch(
        &mut self,
        expected_revision: u64,
        expected_sequence: u64,
    ) -> RootResult<u64> {
        if self.lifecycle != GeneralLifecycleV2::Active {
            return Err(RootError::InvalidLifecycle);
        }
        if self.revision != expected_revision || self.next_batch_sequence != expected_sequence {
            return Err(RootError::CoordinateMismatch);
        }
        let next_revision = self.revision.checked_add(1).ok_or(RootError::Arithmetic)?;
        let next_sequence = self
            .next_batch_sequence
            .checked_add(1)
            .ok_or(RootError::Arithmetic)?;
        let next_open = self
            .open_batches
            .checked_add(1)
            .ok_or(RootError::Arithmetic)?;
        self.revision = next_revision;
        self.next_batch_sequence = next_sequence;
        self.open_batches = next_open;
        Ok(expected_sequence)
    }

    /// Close one batch under an exact revision replay guard.
    pub fn close_batch(&mut self, expected_revision: u64) -> RootResult<()> {
        if self.lifecycle == GeneralLifecycleV2::Retired {
            return Err(RootError::InvalidLifecycle);
        }
        if self.revision != expected_revision {
            return Err(RootError::CoordinateMismatch);
        }
        let next_revision = self.revision.checked_add(1).ok_or(RootError::Arithmetic)?;
        let next_open = self
            .open_batches
            .checked_sub(1)
            .ok_or(RootError::OutstandingBatches)?;
        self.revision = next_revision;
        self.open_batches = next_open;
        Ok(())
    }

    /// Stop opening batches while preserving settlement of existing batches.
    pub fn begin_retiring(&mut self, expected_revision: u64) -> RootResult<()> {
        if self.lifecycle != GeneralLifecycleV2::Active {
            return Err(RootError::InvalidLifecycle);
        }
        if self.revision != expected_revision {
            return Err(RootError::CoordinateMismatch);
        }
        let next_revision = self.revision.checked_add(1).ok_or(RootError::Arithmetic)?;
        self.lifecycle = GeneralLifecycleV2::Retiring;
        self.revision = next_revision;
        Ok(())
    }

    /// Retire after every batch is closed.
    pub fn retire(&mut self, expected_revision: u64) -> RootResult<()> {
        if self.lifecycle != GeneralLifecycleV2::Retiring {
            return Err(RootError::InvalidLifecycle);
        }
        if self.revision != expected_revision {
            return Err(RootError::CoordinateMismatch);
        }
        if self.open_batches != 0 {
            return Err(RootError::OutstandingBatches);
        }
        let next_revision = self.revision.checked_add(1).ok_or(RootError::Arithmetic)?;
        self.lifecycle = GeneralLifecycleV2::Retired;
        self.revision = next_revision;
        Ok(())
    }

    /// Current lifecycle.
    #[must_use]
    pub const fn lifecycle(self) -> GeneralLifecycleV2 {
        self.lifecycle
    }
    /// Canonical Core Market account key.
    #[must_use]
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Sole persisted configuration fact.
    #[must_use]
    pub const fn config_id(self) -> [u8; 32] {
        self.config_id
    }
    /// Immutable Market generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact root revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Next unique batch sequence.
    #[must_use]
    pub const fn next_batch_sequence(self) -> u64 {
        self.next_batch_sequence
    }
    /// Number of live nested batches.
    #[must_use]
    pub const fn open_batches(self) -> u64 {
        self.open_batches
    }
}

/// Exact dust-safe funding classification for a precreated root PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DustSafeRootCreationV2 {
    exact_rent_lamports: u64,
    precreation_lamports: u64,
    funding_top_up_lamports: u64,
    displaced_prepaid_lamports: u64,
    unsolicited_surplus_lamports: u64,
}

impl DustSafeRootCreationV2 {
    /// Classify funding without stranding or reinterpreting a lamport.
    #[must_use]
    pub fn new(exact_rent_lamports: u64, precreation_lamports: u64) -> Self {
        Self {
            exact_rent_lamports,
            precreation_lamports,
            funding_top_up_lamports: exact_rent_lamports.saturating_sub(precreation_lamports),
            displaced_prepaid_lamports: core::cmp::min(precreation_lamports, exact_rent_lamports),
            unsolicited_surplus_lamports: precreation_lamports.saturating_sub(exact_rent_lamports),
        }
    }

    /// Exact Rent minimum retained by the new root.
    #[must_use]
    pub const fn exact_rent_lamports(self) -> u64 {
        self.exact_rent_lamports
    }
    /// Lamports present before the program assigned the PDA.
    #[must_use]
    pub const fn precreation_lamports(self) -> u64 {
        self.precreation_lamports
    }
    /// Amount transferred from segregated Rent funding into the root.
    #[must_use]
    pub const fn funding_top_up_lamports(self) -> u64 {
        self.funding_top_up_lamports
    }
    /// Precreation lamports replacing prepaid Rent and refunded from funding.
    #[must_use]
    pub const fn displaced_prepaid_lamports(self) -> u64 {
        self.displaced_prepaid_lamports
    }
    /// Unsolicited excess routed to the immutable Market RentCredit.
    #[must_use]
    pub const fn unsolicited_surplus_lamports(self) -> u64 {
        self.unsolicited_surplus_lamports
    }
}

/// Whether an activation creates a root or proves an exact prior result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralActivationDispositionV2 {
    /// Register and create one new root.
    Create,
    /// Preserve the exact prior root and every balance/state coordinate.
    Idempotent,
}

/// Broad pure composition of Core preconditions and General-owned activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationV2 {
    disposition: GeneralActivationDispositionV2,
    root: GeneralRootV2,
    market_after: MarketRoot,
    funding_after: FundingStateV1,
    creation: DustSafeRootCreationV2,
    manifest_entry_index: u16,
}

impl GeneralActivationV2 {
    /// Whether physical creation is required.
    #[must_use]
    pub const fn disposition(self) -> GeneralActivationDispositionV2 {
        self.disposition
    }
    /// Exact root to store or exact pre-existing root to preserve.
    #[must_use]
    pub const fn root(self) -> GeneralRootV2 {
        self.root
    }
    /// Exact Core Market poststate; only Core may store it.
    #[must_use]
    pub const fn market_after(self) -> MarketRoot {
        self.market_after
    }
    /// Exact funding poststate; only the capability controller may store it.
    #[must_use]
    pub const fn funding_after(self) -> FundingStateV1 {
        self.funding_after
    }
    /// Dust-safe physical funding classification; all zeros on replay.
    #[must_use]
    pub const fn creation(self) -> DustSafeRootCreationV2 {
        self.creation
    }
    /// Exact selected manifest entry.
    #[must_use]
    pub const fn manifest_entry_index(self) -> u16 {
        self.manifest_entry_index
    }
}

/// General-owned result behind a Core-authenticated capability envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOwnedActivationV2 {
    disposition: GeneralActivationDispositionV2,
    root_state: GeneralRootV2,
    funding_after: FundingStateV1,
    creation: DustSafeRootCreationV2,
}

impl GeneralOwnedActivationV2 {
    /// Whether the adapter creates accounts or proves the exact prior result.
    #[must_use]
    pub const fn disposition(self) -> GeneralActivationDispositionV2 {
        self.disposition
    }
    /// Exact mutable General tail to persist or preserve.
    #[must_use]
    pub const fn root_state(self) -> GeneralRootV2 {
        self.root_state
    }
    /// Exact shared capability-funding poststate owned by General.
    #[must_use]
    pub const fn funding_after(self) -> FundingStateV1 {
        self.funding_after
    }
    /// Exact dust-safe physical split; all zeros on replay.
    #[must_use]
    pub const fn creation(self) -> DustSafeRootCreationV2 {
        self.creation
    }
}

/// Run General-owned activation after Core and the common Trading boundary
/// authenticate the finalized manifest, descriptor, immutable composite-root
/// header, and exact selected-entry request.
///
/// This function does not inspect a finalized-record wrapper or mutate Core
/// state. It consumes the canonical manifest itself because that is the sole
/// owner of the selected 528-byte funding quote. General alone advances and
/// persists `funding` and the mutable tail in the same composite account; Core
/// verifies the acknowledgement before incrementing its outstanding-capability
/// count. `exact_root_rent_lamports` and `precreation_lamports` always refer to
/// the complete common-header plus General-tail account.
#[allow(clippy::too_many_arguments)]
pub fn activate_general_owned_v2(
    market_key: [u8; 32],
    generation: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    config_id: ContentId,
    config: GeneralConfigV2,
    funding: FundingStateV1,
    custody: FundingCustodyObservationV1,
    current_slot: u64,
    exact_root_rent_lamports: u64,
    precreation_lamports: u64,
    existing_root_state: Option<GeneralRootV2>,
) -> RootResult<GeneralOwnedActivationV2> {
    if is_zero(&market_key) || exact_root_rent_lamports == 0 {
        return Err(RootError::ZeroIdentity);
    }
    if config.generation() != generation || funding.entry_index() != entry_index {
        return Err(RootError::CapabilityMismatch);
    }
    let entry = manifest.entry(entry_index)?;
    if entry.kind_id().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || entry.release_id().to_bytes() != config.capability_program_id()
        || entry.config_id() != config_id
        || entry.capacity_profile_id().to_bytes() != config.capacity_profile_id()
        || entry.child_schema_id().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
    {
        return Err(RootError::CapabilityMismatch);
    }
    let expected_state = GeneralRootV2::active(market_key, config_id.to_bytes(), generation)?;
    if let Some(present) = existing_root_state {
        if present != expected_state || precreation_lamports != exact_root_rent_lamports {
            return Err(RootError::ActivationReplayMismatch);
        }
        funding.validate_against(manifest_id, manifest, custody)?;
        if funding.status() != FundingStatus::Active || funding.activation_slot() != current_slot {
            return Err(RootError::ActivationReplayMismatch);
        }
        return Ok(GeneralOwnedActivationV2 {
            disposition: GeneralActivationDispositionV2::Idempotent,
            root_state: present,
            funding_after: funding,
            creation: DustSafeRootCreationV2::new(0, 0),
        });
    }
    let mut funding_after = funding;
    let debit = funding_after.activate(manifest_id, manifest, custody, current_slot)?;
    if debit.rent_lamports() != exact_root_rent_lamports || debit.creation_lamports() != 0 {
        return Err(RootError::ActivationFundingMismatch);
    }
    Ok(GeneralOwnedActivationV2 {
        disposition: GeneralActivationDispositionV2::Create,
        root_state: expected_state,
        funding_after,
        creation: DustSafeRootCreationV2::new(exact_root_rent_lamports, precreation_lamports),
    })
}

/// Plan canonical Market→manifest→config General activation.
///
/// The caller authenticates the Market PDA and Registry ownership, hashes the
/// exact manifest and config accounts into the supplied content IDs, and
/// reauthenticates this program as the Registry-selected Trading release.  A
/// physical implementation asks Core to store `market_after`; the capability
/// controller alone stores `funding_after` and must do so atomically with its
/// child effects. This broader pure composition remains an executable model of
/// Core's precondition, not a duplicate physical authority path.
#[allow(clippy::too_many_arguments)]
pub fn plan_general_activation_v2(
    market_key: [u8; 32],
    market: MarketRoot,
    expected_market_child_count: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    config_id: ContentId,
    config: GeneralConfigV2,
    funding: FundingStateV1,
    custody: FundingCustodyObservationV1,
    current_slot: u64,
    exact_root_rent_lamports: u64,
    precreation_lamports: u64,
    existing_root: Option<GeneralRootV2>,
) -> RootResult<GeneralActivationV2> {
    if is_zero(&market_key) || exact_root_rent_lamports == 0 {
        return Err(RootError::ZeroIdentity);
    }
    market.validate()?;
    let identity = market.identity();
    if identity.capability_manifest_id() != manifest_id {
        return Err(RootError::CapabilityMismatch);
    }
    config
        .require_market_coordinates(identity.generation(), identity.claim_basis_id().to_bytes())?;
    let (entry_index, entry) = select_general_entry(manifest, config_id)?;
    if entry.release_id().to_bytes() != config.capability_program_id()
        || entry.config_id() != config_id
        || entry.capacity_profile_id().to_bytes() != config.capacity_profile_id()
        || entry.child_schema_id().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
    {
        return Err(RootError::CapabilityMismatch);
    }
    match (entry.activation_policy(), market.phase()) {
        (ActivationPolicy::RequiredAtFounding, Phase::Founding)
        | (ActivationPolicy::PrepaidLazy, Phase::Founding | Phase::Open) => {}
        _ => return Err(RootError::ActivationPhaseMismatch),
    }
    let expected_root =
        GeneralRootV2::active(market_key, config_id.to_bytes(), identity.generation())?;
    if let Some(present) = existing_root {
        if present != expected_root {
            return Err(RootError::ActivationReplayMismatch);
        }
        funding.validate_against(manifest_id, manifest, custody)?;
        if funding.status() != FundingStatus::Active || funding.entry_index() != entry_index {
            return Err(RootError::ActivationReplayMismatch);
        }
        return Ok(GeneralActivationV2 {
            disposition: GeneralActivationDispositionV2::Idempotent,
            root: present,
            market_after: market,
            funding_after: funding,
            creation: DustSafeRootCreationV2::new(0, 0),
            manifest_entry_index: entry_index,
        });
    }
    if funding.entry_index() != entry_index {
        return Err(RootError::CapabilityMismatch);
    }
    let mut funding_after = funding;
    let debit = funding_after.activate(manifest_id, manifest, custody, current_slot)?;
    if debit.rent_lamports() != exact_root_rent_lamports || debit.creation_lamports() != 0 {
        return Err(RootError::ActivationFundingMismatch);
    }
    let mut market_after = market;
    market_after.register_child(identity.generation(), expected_market_child_count)?;
    Ok(GeneralActivationV2 {
        disposition: GeneralActivationDispositionV2::Create,
        root: expected_root,
        market_after,
        funding_after,
        creation: DustSafeRootCreationV2::new(exact_root_rent_lamports, precreation_lamports),
        manifest_entry_index: entry_index,
    })
}

fn select_general_entry(
    manifest: CapabilityManifestV1<'_>,
    config_id: ContentId,
) -> RootResult<(u16, dclutch_capability_contract::CapabilityEntryV1)> {
    let kind = ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1)?;
    let mut selected = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest.entry(index)?;
        if entry.kind_id() == kind && entry.config_id() == config_id {
            if selected.is_some() {
                return Err(RootError::CapabilityAmbiguous);
            }
            selected = Some((index, entry));
        }
        index = index.checked_add(1).ok_or(RootError::Arithmetic)?;
    }
    selected.ok_or(RootError::CapabilityMissing)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_u8(input: &[u8], offset: usize) -> RootResult<u8> {
    input.get(offset).copied().ok_or(RootError::InvalidLength)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> RootResult<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(RootError::InvalidLength)?)
        .ok_or(RootError::InvalidLength)?
        .try_into()
        .map_err(|_| RootError::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> RootResult<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> RootResult<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> RootResult<()> {
    if input
        .get(offset..offset.checked_add(width).ok_or(RootError::InvalidLength)?)
        .ok_or(RootError::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(RootError::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(target) = output.get_mut(offset..end) else {
        return;
    };
    target.copy_from_slice(value);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::MarketIdentity;
    use sha2::{Digest, Sha256};

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn root() -> GeneralRootV2 {
        GeneralRootV2::active(id(0x11), id(0x22), 7)
            .unwrap_or_else(|error| panic!("root fixture: {error:?}"))
    }

    fn content(low: u8) -> ContentId {
        ContentId::new(id(low)).unwrap_or_else(|error| panic!("content fixture: {error:?}"))
    }

    fn config_for_program(capability_program_id: [u8; 32]) -> GeneralConfigV2 {
        GeneralConfigV2::new(crate::GeneralConfigV2Input {
            capacity_profile_id: id(0x41),
            claim_basis_id: id(0x42),
            capability_program_id,
            generation: 7,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: 64,
            max_pages_per_candidate: 2,
            continuation_reward_lamports: 5,
            selection_policy_id: id(0x33),
            outcome_count: 2,
            quote_surplus_beneficiary: id(0x43),
        })
        .unwrap_or_else(|error| panic!("config fixture: {error:?}"))
    }

    fn config() -> GeneralConfigV2 {
        config_for_program(id(0x23))
    }

    fn quote(rent: u64, creation: u64) -> FundingQuoteV1 {
        let native = |amount| {
            if amount == 0 {
                CompartmentFundingV1::not_applicable()
            } else {
                CompartmentFundingV1::native_lamports(amount)
                    .unwrap_or_else(|error| panic!("amount fixture: {error:?}"))
            }
        };
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                native(rent),
                native(creation),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .unwrap_or_else(|error| panic!("amounts fixture: {error:?}")),
            None,
        )
        .unwrap_or_else(|error| panic!("quote fixture: {error:?}"))
    }

    fn general_entry(
        config_id: ContentId,
        capacity_id: ContentId,
        quote: FundingQuoteV1,
    ) -> CapabilityEntryV1 {
        CapabilityEntryV1::new(
            ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1)
                .unwrap_or_else(|error| panic!("kind fixture: {error:?}")),
            ContentId::new(config().capability_program_id())
                .unwrap_or_else(|error| panic!("release fixture: {error:?}")),
            config_id,
            capacity_id,
            ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2)
                .unwrap_or_else(|error| panic!("schema fixture: {error:?}")),
            content(0x82),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .unwrap_or_else(|error| panic!("entry fixture: {error:?}"))
    }

    #[test]
    fn lean_root_fixture_round_trips_exactly() {
        let root = root();
        assert_eq!(root.to_bytes(), crate::generated::GENERAL_ROOT_EXAMPLE_V2);
        assert_eq!(GeneralRootV2::decode(&root.to_bytes()), Ok(root));
        assert_eq!(
            Sha256::digest(GENERAL_CAPABILITY_KIND_PREIMAGE_V1).as_slice(),
            GENERAL_CAPABILITY_KIND_ID_V1
        );
        assert_eq!(
            Sha256::digest(GENERAL_ROOT_SCHEMA_PREIMAGE_V2).as_slice(),
            GENERAL_ROOT_SCHEMA_ID_V2
        );
    }

    #[test]
    fn hostile_root_width_headers_reserved_and_identities_refuse() {
        let canonical = root().to_bytes();
        for width in 0..GENERAL_ROOT_BYTES_V2 {
            assert_eq!(
                GeneralRootV2::decode(canonical.get(..width).unwrap_or(&[])),
                Err(RootError::InvalidLength)
            );
        }
        for (offset, expected) in [
            (0, RootError::UnsupportedSchema),
            (
                crate::generated::ROOT_VERSION_OFFSET,
                RootError::UnsupportedSchema,
            ),
            (
                crate::generated::ROOT_RESERVED_HEADER_OFFSET,
                RootError::NonCanonicalReservedBytes,
            ),
            (
                crate::generated::ROOT_RESERVED_TAIL_OFFSET,
                RootError::NonCanonicalReservedBytes,
            ),
        ] {
            let mut hostile = canonical;
            hostile[offset] ^= 1;
            assert_eq!(GeneralRootV2::decode(&hostile), Err(expected));
        }
        for offset in [
            crate::generated::ROOT_MARKET_OFFSET,
            crate::generated::ROOT_CONFIG_ID_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile[offset..offset + 32].fill(0);
            assert_eq!(
                GeneralRootV2::decode(&hostile),
                Err(RootError::ZeroIdentity)
            );
        }
    }

    #[test]
    fn replay_guards_and_late_refusals_leave_root_unchanged() {
        let mut root = root();
        let before = root;
        assert_eq!(root.open_batch(2, 0), Err(RootError::CoordinateMismatch));
        assert_eq!(root, before);
        assert_eq!(root.open_batch(1, 0), Ok(0));
        let opened = root;
        assert_eq!(root.retire(2), Err(RootError::InvalidLifecycle));
        assert_eq!(root, opened);
        assert_eq!(root.begin_retiring(2), Ok(()));
        let retiring = root;
        assert_eq!(root.retire(3), Err(RootError::OutstandingBatches));
        assert_eq!(root, retiring);
        assert_eq!(root.close_batch(3), Ok(()));
        assert_eq!(root.retire(4), Ok(()));
        assert_eq!(root.lifecycle(), GeneralLifecycleV2::Retired);
        assert_eq!(root.open_batches(), 0);
    }

    #[test]
    fn hot_context_requires_exact_content_hash_and_market_coordinates() {
        let config = config();
        let config_digest: [u8; 32] = Sha256::digest(config.to_bytes()).into();
        let root = GeneralRootV2::active(id(0x11), config_digest, 7)
            .unwrap_or_else(|error| panic!("root fixture: {error:?}"));
        assert_eq!(
            root.require_hot_context(id(0x11), 7, id(0x42), config_digest, config, 1),
            Ok(())
        );
        let mut substituted_digest = config_digest;
        substituted_digest[0] ^= 1;
        for result in [
            root.require_hot_context(id(0x11), 7, id(0x42), substituted_digest, config, 1),
            root.require_hot_context(id(0x12), 7, id(0x42), config_digest, config, 1),
            root.require_hot_context(id(0x11), 8, id(0x42), config_digest, config, 1),
            root.require_hot_context(id(0x11), 7, id(0x43), config_digest, config, 1),
            root.require_hot_context(id(0x11), 7, id(0x42), config_digest, config, 2),
        ] {
            assert!(matches!(
                result,
                Err(RootError::CoordinateMismatch | RootError::Config(_))
            ));
        }
    }

    #[test]
    fn dust_safe_plan_conserves_prepaid_rent_and_surplus() {
        let below = DustSafeRootCreationV2::new(100, 40);
        assert_eq!(below.funding_top_up_lamports(), 60);
        assert_eq!(below.displaced_prepaid_lamports(), 40);
        assert_eq!(below.unsolicited_surplus_lamports(), 0);
        assert_eq!(
            below.funding_top_up_lamports() + below.displaced_prepaid_lamports(),
            below.exact_rent_lamports()
        );
        let above = DustSafeRootCreationV2::new(100, 140);
        assert_eq!(above.funding_top_up_lamports(), 0);
        assert_eq!(above.displaced_prepaid_lamports(), 100);
        assert_eq!(above.unsolicited_surplus_lamports(), 40);
        assert_eq!(
            above.exact_rent_lamports() + above.unsolicited_surplus_lamports(),
            above.precreation_lamports()
        );
    }

    #[test]
    fn core_signed_activation_advances_only_general_owned_root_and_funding() {
        let config = config();
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let entry = general_entry(config_id, content(0x41), quote(100, 0));
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage)
            .unwrap_or_else(|error| panic!("manifest fixture: {error:?}"));
        let pending_custody = FundingCustodyObservationV1::native_only(120, 20)
            .unwrap_or_else(|error| panic!("custody fixture: {error:?}"));
        let funding = FundingStateV1::new(manifest_id, manifest, 0, pending_custody)
            .unwrap_or_else(|error| panic!("funding fixture: {error:?}"));
        let created = activate_general_owned_v2(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            funding,
            pending_custody,
            9,
            100,
            40,
            None,
        )
        .unwrap_or_else(|error| panic!("General-owned activation: {error:?}"));
        assert_eq!(
            created.disposition(),
            GeneralActivationDispositionV2::Create
        );
        assert_eq!(created.funding_after().status(), FundingStatus::Active);
        assert_eq!(created.funding_after().activation_slot(), 9);
        assert_eq!(created.creation().funding_top_up_lamports(), 60);
        assert_eq!(created.creation().displaced_prepaid_lamports(), 40);

        let active_custody = FundingCustodyObservationV1::native_only(20, 20)
            .unwrap_or_else(|error| panic!("active custody fixture: {error:?}"));
        let replay = activate_general_owned_v2(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            created.funding_after(),
            active_custody,
            9,
            100,
            100,
            Some(created.root_state()),
        )
        .unwrap_or_else(|error| panic!("General-owned replay: {error:?}"));
        assert_eq!(
            replay.disposition(),
            GeneralActivationDispositionV2::Idempotent
        );
        assert_eq!(replay.root_state(), created.root_state());
        assert_eq!(replay.funding_after(), created.funding_after());
        assert_eq!(replay.creation(), DustSafeRootCreationV2::new(0, 0));

        let stale = activate_general_owned_v2(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            created.funding_after(),
            active_custody,
            10,
            100,
            100,
            Some(created.root_state()),
        );
        assert_eq!(stale, Err(RootError::ActivationReplayMismatch));
        let dusty_replay = activate_general_owned_v2(
            id(0x11),
            7,
            manifest_id,
            manifest,
            0,
            config_id,
            config,
            created.funding_after(),
            active_custody,
            9,
            100,
            101,
            Some(created.root_state()),
        );
        assert_eq!(dusty_replay, Err(RootError::ActivationReplayMismatch));
    }

    #[test]
    fn substituted_capability_program_identity_refuses_without_funding_mutation() {
        let config = config();
        let substituted_config = config_for_program(id(0x24));
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let entry = general_entry(config_id, content(0x41), quote(100, 0));
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage)
            .unwrap_or_else(|error| panic!("manifest fixture: {error:?}"));
        let custody = FundingCustodyObservationV1::native_only(120, 20)
            .unwrap_or_else(|error| panic!("custody fixture: {error:?}"));
        let funding = FundingStateV1::new(manifest_id, manifest, 0, custody)
            .unwrap_or_else(|error| panic!("funding fixture: {error:?}"));
        let funding_before = funding;

        assert_ne!(
            config.capability_program_id(),
            substituted_config.capability_program_id()
        );
        assert_eq!(
            activate_general_owned_v2(
                id(0x11),
                7,
                manifest_id,
                manifest,
                0,
                config_id,
                substituted_config,
                funding,
                custody,
                9,
                100,
                40,
                None,
            ),
            Err(RootError::CapabilityMismatch)
        );
        assert_eq!(funding, funding_before);
    }

    #[test]
    fn canonical_activation_plans_core_owned_poststates_then_replays_idempotently() {
        let config = config();
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let identity = MarketIdentity::new(
            content(0x61),
            content(0x62),
            content(0x42),
            content(0x64),
            manifest_id,
            7,
        );
        let market = MarketRoot::founding(identity, id(0x71))
            .unwrap_or_else(|error| panic!("market fixture: {error:?}"));
        let entry = general_entry(config_id, content(0x41), quote(100, 0));
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage)
            .unwrap_or_else(|error| panic!("manifest fixture: {error:?}"));
        let pending_custody = FundingCustodyObservationV1::native_only(120, 20)
            .unwrap_or_else(|error| panic!("custody fixture: {error:?}"));
        let funding = FundingStateV1::new(manifest_id, manifest, 0, pending_custody)
            .unwrap_or_else(|error| panic!("funding fixture: {error:?}"));
        let created = plan_general_activation_v2(
            id(0x11),
            market,
            0,
            manifest_id,
            manifest,
            config_id,
            config,
            funding,
            pending_custody,
            9,
            100,
            40,
            None,
        )
        .unwrap_or_else(|error| panic!("activation fixture: {error:?}"));
        assert_eq!(
            created.disposition(),
            GeneralActivationDispositionV2::Create
        );
        let root = created.root();
        let market_after = created.market_after();
        let funding_after = created.funding_after();
        let creation = created.creation();
        assert_eq!(created.manifest_entry_index(), 0);
        assert_eq!(root.config_id(), config_id.to_bytes());
        assert_eq!(market_after.outstanding_children(), 1);
        assert_eq!(funding_after.status(), FundingStatus::Active);
        assert_eq!(creation.funding_top_up_lamports(), 60);
        assert_eq!(creation.displaced_prepaid_lamports(), 40);

        let active_custody = FundingCustodyObservationV1::native_only(20, 20)
            .unwrap_or_else(|error| panic!("active custody fixture: {error:?}"));
        assert_eq!(
            plan_general_activation_v2(
                id(0x11),
                market_after,
                1,
                manifest_id,
                manifest,
                config_id,
                config,
                funding_after,
                active_custody,
                10,
                100,
                100,
                Some(root),
            ),
            Ok(GeneralActivationV2 {
                disposition: GeneralActivationDispositionV2::Idempotent,
                root,
                market_after,
                funding_after,
                creation: DustSafeRootCreationV2::new(0, 0),
                manifest_entry_index: 0,
            })
        );
    }

    #[test]
    fn late_activation_refusal_does_not_mutate_any_input() {
        let config = config();
        let config_id = content(0x51);
        let manifest_id = content(0x52);
        let identity = MarketIdentity::new(
            content(0x61),
            content(0x62),
            content(0x42),
            content(0x64),
            manifest_id,
            7,
        );
        let market = MarketRoot::founding(identity, id(0x71))
            .unwrap_or_else(|error| panic!("market fixture: {error:?}"));
        let entry = general_entry(config_id, content(0x41), quote(100, 1));
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage)
            .unwrap_or_else(|error| panic!("manifest fixture: {error:?}"));
        let custody = FundingCustodyObservationV1::native_only(121, 20)
            .unwrap_or_else(|error| panic!("custody fixture: {error:?}"));
        let funding = FundingStateV1::new(manifest_id, manifest, 0, custody)
            .unwrap_or_else(|error| panic!("funding fixture: {error:?}"));
        let market_before = market;
        let funding_before = funding;
        assert_eq!(
            plan_general_activation_v2(
                id(0x11),
                market,
                0,
                manifest_id,
                manifest,
                config_id,
                config,
                funding,
                custody,
                9,
                100,
                0,
                None,
            ),
            Err(RootError::ActivationFundingMismatch)
        );
        assert_eq!(market, market_before);
        assert_eq!(funding, funding_before);
    }
}
