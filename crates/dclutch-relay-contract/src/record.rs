//! The persisted `RelayedObservationRecordV1`: a direct Market child that
//! collects one ordered set of attested account observations, is sealed by a
//! quorum of the release's key set, and is then consumed once by a resolution.
//!
//! The record is seeded by `observed_slot`, so **at most one record exists per
//! set per slot**.  That is what bounds equivocation structurally: a relayer
//! that signs two different observations of the same set at the same slot cannot
//! overwrite the first, and can only publish a second signed message that
//! contradicts an on-chain one — a permanent, publicly checkable proof.
//!
//! Filling is 1-of-n authenticated and sealing is m-of-n, deliberately.  A
//! single honest member can complete a record; the quorum only certifies it.  A
//! malicious member who fills a record with false bytes cannot get it sealed,
//! and honest members simply build a record at a different slot — so a bad fill
//! is a wasted rent deposit and a permanent signed lie, never a denial of
//! service.
//!
//! Every mutator here follows the house discipline: all reads are hoisted, every
//! fallible check finishes before the first byte changes, and the phase byte is
//! written **last**, so a partially written record can never advertise a more
//! advanced phase than its contents.

use crate::{
    ADDRESS_BYTES, Error, MAX_RELAYED_ACCOUNTS_V1, RELAYED_RECORD_ACCOUNT_SET_ID_OFFSET,
    RELAYED_RECORD_CREATED_UNIX_SECONDS_OFFSET, RELAYED_RECORD_FILLED_COUNT_OFFSET,
    RELAYED_RECORD_GENERATION_OFFSET, RELAYED_RECORD_HEADER_BYTES, RELAYED_RECORD_MAGIC,
    RELAYED_RECORD_MARKET_OFFSET, RELAYED_RECORD_OBSERVED_CLUSTER_ID_OFFSET,
    RELAYED_RECORD_OBSERVED_SLOT_OFFSET, RELAYED_RECORD_PHASE_OFFSET,
    RELAYED_RECORD_PROVIDER_RELEASE_ID_OFFSET, RELAYED_RECORD_RELAYER_KEY_SET_ID_OFFSET,
    RELAYED_RECORD_RENT_CREDIT_BENEFICIARY_OFFSET, RELAYED_RECORD_RESERVED_OFFSET,
    RELAYED_RECORD_RESERVED_TAIL_OFFSET, RELAYED_RECORD_SEAL_COUNT_OFFSET,
    RELAYED_RECORD_SEAL_THRESHOLD_OFFSET, RELAYED_RECORD_SEALED_BY_BITMAP_OFFSET,
    RELAYED_RECORD_SEALED_UNIX_SECONDS_OFFSET, RELAYED_RECORD_SET_COUNT_OFFSET,
    RELAYED_RECORD_SET_DIGEST_OFFSET, RELAYED_RECORD_SLOT_BYTES,
    RELAYED_RECORD_SOURCE_MATERIAL_ID_OFFSET, Result, array, i64_at, one, put, require_nonzero,
    require_zero, slice, u16_at, u64_at, variable_header,
    wire::{AccountObservationV1, AttestationMessageV1, ObservationSetSealV1},
};

/// Persisted lifecycle phase of one observation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RelayedRecordPhaseV1 {
    /// Accepting appends in strictly increasing set order.
    Collecting = 1,
    /// The quorum has certified the complete set; ready to be consumed once.
    Sealed = 2,
    /// A resolution has consumed the sealed set.
    Consumed = 3,
    /// Closed into the pre-existing RentCredit beneficiary.
    Retired = 4,
}

impl RelayedRecordPhaseV1 {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Collecting),
            2 => Ok(Self::Sealed),
            3 => Ok(Self::Consumed),
            4 => Ok(Self::Retired),
            _ => Err(Error::NonCanonicalRecord),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Exact PDA seed material for one observation record.
///
/// Seeding by `observed_slot` is not a convenience: it is the equivocation
/// bound.  A second contradictory observation of the same set at the same slot
/// has nowhere to live.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedRecordPdaSeedsV1 {
    market: [u8; ADDRESS_BYTES],
    generation_le: [u8; 8],
    account_set_id: [u8; 32],
    observed_slot_le: [u8; 8],
}

impl RelayedRecordPdaSeedsV1 {
    /// Construct the seed tuple for one record.
    pub fn new(
        market: [u8; ADDRESS_BYTES],
        generation: u64,
        account_set_id: [u8; 32],
        observed_slot: u64,
    ) -> Result<Self> {
        require_nonzero(&market)?;
        require_nonzero(&account_set_id)?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            account_set_id,
            observed_slot_le: observed_slot.to_le_bytes(),
        })
    }

    /// The PDA seed domain.
    pub const fn domain(self) -> &'static [u8] {
        crate::RELAYED_RECORD_PDA_DOMAIN_V1
    }
    /// The owning Market.
    pub const fn market(self) -> [u8; ADDRESS_BYTES] {
        self.market
    }
    /// The Market generation, little-endian.
    pub const fn generation_le(self) -> [u8; 8] {
        self.generation_le
    }
    /// The founding-time pinned ordered account set.
    pub const fn account_set_id(self) -> [u8; 32] {
        self.account_set_id
    }
    /// The finalized foreign slot, little-endian.
    pub const fn observed_slot_le(self) -> [u8; 8] {
        self.observed_slot_le
    }
}

/// Exact persisted width of a record holding `set_count` positions.
///
/// The record is runtime-width, following the runtime-width Source resolution
/// state rather than the fixed-width shared-observation child.  An exact-length
/// decoder is strictly more hostile than a padded fixed one, and a four-account
/// set stops paying rent for four unused slots.
pub fn relayed_observation_record_bytes_v1(set_count: u16) -> Result<usize> {
    let count = usize::from(set_count);
    if count == 0 || count > MAX_RELAYED_ACCOUNTS_V1 {
        return Err(Error::InvalidSetGeometry);
    }
    RELAYED_RECORD_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(RELAYED_RECORD_SLOT_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn slot_offset(index: u16) -> Result<usize> {
    let index = usize::from(index);
    if index >= MAX_RELAYED_ACCOUNTS_V1 {
        return Err(Error::InvalidSetGeometry);
    }
    RELAYED_RECORD_HEADER_BYTES
        .checked_add(
            index
                .checked_mul(RELAYED_RECORD_SLOT_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Borrowed, fully validated view of one persisted record.
///
/// Construction runs the complete canonical validator, so every accessor below
/// reads a field that has already been checked for its phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedObservationRecordViewV1<'a> {
    bytes: &'a [u8],
}

impl<'a> RelayedObservationRecordViewV1<'a> {
    /// Hostile-decode one persisted record of its exact runtime width.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        validate_relayed_observation_record_bytes_v1(bytes)?;
        Ok(Self { bytes })
    }

    /// The validated backing bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// The persisted lifecycle phase.
    pub fn phase(self) -> Result<RelayedRecordPhaseV1> {
        RelayedRecordPhaseV1::decode(one(self.bytes, RELAYED_RECORD_PHASE_OFFSET)?)
    }
    /// The owning Market.
    pub fn market(self) -> Result<[u8; ADDRESS_BYTES]> {
        array(self.bytes, RELAYED_RECORD_MARKET_OFFSET)
    }
    /// The Market generation this record was created under.
    pub fn generation(self) -> Result<u64> {
        u64_at(self.bytes, RELAYED_RECORD_GENERATION_OFFSET)
    }
    /// The immutable Source material this record serves.
    pub fn source_material_id(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_SOURCE_MATERIAL_ID_OFFSET)
    }
    /// The founding-time pinned ordered account set.
    pub fn account_set_id(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_ACCOUNT_SET_ID_OFFSET)
    }
    /// The provider release that authorizes this record's signers.
    pub fn provider_release_id(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_PROVIDER_RELEASE_ID_OFFSET)
    }
    /// The immutable relayer key set.
    pub fn relayer_key_set_id(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_RELAYER_KEY_SET_ID_OFFSET)
    }
    /// The genesis hash of the cluster observed.
    pub fn observed_cluster_id(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_OBSERVED_CLUSTER_ID_OFFSET)
    }
    /// The finalized foreign slot every accepted body was read at.
    pub fn observed_slot(self) -> Result<u64> {
        u64_at(self.bytes, RELAYED_RECORD_OBSERVED_SLOT_OFFSET)
    }
    /// The running fold over accepted bodies; final once sealed.
    pub fn set_digest(self) -> Result<[u8; 32]> {
        array(self.bytes, RELAYED_RECORD_SET_DIGEST_OFFSET)
    }
    /// The pre-existing RentCredit that receives this record's rent on closure.
    pub fn rent_credit_beneficiary(self) -> Result<[u8; ADDRESS_BYTES]> {
        array(self.bytes, RELAYED_RECORD_RENT_CREDIT_BENEFICIARY_OFFSET)
    }
    /// Devnet `Clock` at creation.
    pub fn created_unix_seconds(self) -> Result<i64> {
        i64_at(self.bytes, RELAYED_RECORD_CREATED_UNIX_SECONDS_OFFSET)
    }
    /// Devnet `Clock` at sealing; zero until sealed.
    pub fn sealed_unix_seconds(self) -> Result<i64> {
        i64_at(self.bytes, RELAYED_RECORD_SEALED_UNIX_SECONDS_OFFSET)
    }
    /// The cardinality of the ordered set.
    pub fn set_count(self) -> Result<u16> {
        u16_at(self.bytes, RELAYED_RECORD_SET_COUNT_OFFSET)
    }
    /// How many positions have been filled.
    pub fn filled_count(self) -> Result<u16> {
        u16_at(self.bytes, RELAYED_RECORD_FILLED_COUNT_OFFSET)
    }
    /// The release's quorum threshold, echoed at creation.
    pub fn seal_threshold(self) -> Result<u8> {
        one(self.bytes, RELAYED_RECORD_SEAL_THRESHOLD_OFFSET)
    }
    /// How many distinct key-set members have sealed.
    pub fn seal_count(self) -> Result<u8> {
        one(self.bytes, RELAYED_RECORD_SEAL_COUNT_OFFSET)
    }
    /// Which key-set members have sealed, by position.
    pub fn sealed_by_bitmap(self) -> Result<u8> {
        one(self.bytes, RELAYED_RECORD_SEALED_BY_BITMAP_OFFSET)
    }

    /// The PDA seed tuple, re-derived from the persisted record itself.
    pub fn pda_seeds(self) -> Result<RelayedRecordPdaSeedsV1> {
        RelayedRecordPdaSeedsV1::new(
            self.market()?,
            self.generation()?,
            self.account_set_id()?,
            self.observed_slot()?,
        )
    }

    /// One accepted observation body, borrowed from the record.
    pub fn observation(self, index: u16) -> Result<AccountObservationV1<'a>> {
        if index >= self.filled_count()? {
            return Err(Error::InvalidSetGeometry);
        }
        let offset = slot_offset(index)?;
        let region = slice(self.bytes, offset, RELAYED_RECORD_SLOT_BYTES)?;
        AccountObservationV1::decode_prefix(region).map(|(body, _)| body)
    }

    /// Validate this record before a resolution consumes it.
    ///
    /// The record's program ownership and PDA derivation are the authority after
    /// sealing; signatures are verified at append and seal time and never again.
    #[allow(clippy::too_many_arguments)]
    pub fn require_consumable(
        self,
        market: [u8; ADDRESS_BYTES],
        generation: u64,
        source_material_id: [u8; 32],
        account_set_id: [u8; 32],
        provider_release_id: [u8; 32],
        relayer_key_set_id: [u8; 32],
        pinned_cluster_id: [u8; 32],
    ) -> Result<()> {
        if self.phase()? != RelayedRecordPhaseV1::Sealed {
            return Err(Error::InvalidRecordTransition);
        }
        if self.observed_cluster_id()? != pinned_cluster_id {
            return Err(Error::ObservedClusterMismatch);
        }
        if self.account_set_id()? != account_set_id {
            return Err(Error::AccountSetMismatch);
        }
        if self.market()? != market
            || self.generation()? != generation
            || self.source_material_id()? != source_material_id
            || self.provider_release_id()? != provider_release_id
            || self.relayer_key_set_id()? != relayer_key_set_id
        {
            return Err(Error::RecordBindingMismatch);
        }
        if self.filled_count()? != self.set_count()? {
            return Err(Error::NonCanonicalRecord);
        }
        if self.seal_count()? < self.seal_threshold()? {
            return Err(Error::SealThresholdNotReached);
        }
        Ok(())
    }
}

/// Validate one persisted record without constructing a by-value copy.
pub fn validate_relayed_observation_record_bytes_v1(bytes: &[u8]) -> Result<()> {
    variable_header(bytes, RELAYED_RECORD_MAGIC)?;
    require_zero(bytes, RELAYED_RECORD_RESERVED_OFFSET, 2)?;
    require_zero(bytes, RELAYED_RECORD_RESERVED_TAIL_OFFSET, 4)?;

    let set_count = u16_at(bytes, RELAYED_RECORD_SET_COUNT_OFFSET)?;
    if bytes.len() != relayed_observation_record_bytes_v1(set_count)? {
        return Err(Error::InvalidLength);
    }

    let market: [u8; ADDRESS_BYTES] = array(bytes, RELAYED_RECORD_MARKET_OFFSET)?;
    let beneficiary: [u8; ADDRESS_BYTES] =
        array(bytes, RELAYED_RECORD_RENT_CREDIT_BENEFICIARY_OFFSET)?;
    require_nonzero(&market)?;
    require_nonzero(&beneficiary)?;
    for offset in [
        RELAYED_RECORD_SOURCE_MATERIAL_ID_OFFSET,
        RELAYED_RECORD_ACCOUNT_SET_ID_OFFSET,
        RELAYED_RECORD_PROVIDER_RELEASE_ID_OFFSET,
        RELAYED_RECORD_RELAYER_KEY_SET_ID_OFFSET,
        RELAYED_RECORD_OBSERVED_CLUSTER_ID_OFFSET,
        RELAYED_RECORD_SET_DIGEST_OFFSET,
    ] {
        let identity: [u8; 32] = array(bytes, offset)?;
        require_nonzero(&identity)?;
    }

    let phase = RelayedRecordPhaseV1::decode(one(bytes, RELAYED_RECORD_PHASE_OFFSET)?)?;
    let filled = u16_at(bytes, RELAYED_RECORD_FILLED_COUNT_OFFSET)?;
    let threshold = one(bytes, RELAYED_RECORD_SEAL_THRESHOLD_OFFSET)?;
    let seals = one(bytes, RELAYED_RECORD_SEAL_COUNT_OFFSET)?;
    let bitmap = one(bytes, RELAYED_RECORD_SEALED_BY_BITMAP_OFFSET)?;
    let created = i64_at(bytes, RELAYED_RECORD_CREATED_UNIX_SECONDS_OFFSET)?;
    let sealed = i64_at(bytes, RELAYED_RECORD_SEALED_UNIX_SECONDS_OFFSET)?;

    if created <= 0
        || filled > set_count
        || threshold == 0
        || threshold > crate::MAX_RELAYER_KEYS_V1_U8
    {
        return Err(Error::NonCanonicalRecord);
    }
    if u32::from(seals) != bitmap.count_ones() {
        return Err(Error::NonCanonicalRecord);
    }
    match phase {
        RelayedRecordPhaseV1::Collecting => {
            // Partial seals are legitimate here: the quorum accumulates while
            // the record is still Collecting and only the threshold flips the
            // phase.  What is *not* legitimate is a seal count at or above the
            // threshold that never advanced, a seal time before there is a
            // seal, or a seal over a set that is not complete.
            if seals >= threshold || sealed != 0 {
                return Err(Error::NonCanonicalRecord);
            }
            if seals > 0 && filled != set_count {
                return Err(Error::NonCanonicalRecord);
            }
        }
        RelayedRecordPhaseV1::Sealed | RelayedRecordPhaseV1::Consumed => {
            if filled != set_count || seals < threshold || sealed < created {
                return Err(Error::NonCanonicalRecord);
            }
        }
        RelayedRecordPhaseV1::Retired => {
            // Retirement is legal from any live phase, so the only rule left is
            // that a record which claims a seal time must have been sealed.
            if sealed != 0 && sealed < created {
                return Err(Error::NonCanonicalRecord);
            }
        }
    }

    for index in 0..set_count {
        let offset = slot_offset(index)?;
        let region = slice(bytes, offset, RELAYED_RECORD_SLOT_BYTES)?;
        if index < filled {
            let (body, consumed) = AccountObservationV1::decode_prefix(region)?;
            let _ = body;
            require_zero(
                region,
                consumed,
                RELAYED_RECORD_SLOT_BYTES
                    .checked_sub(consumed)
                    .ok_or(Error::InvalidLength)?,
            )?;
        } else if region.iter().any(|byte| *byte != 0) {
            // Trailing bytes in an unfilled slot would be observation material
            // nothing ever authenticated.
            return Err(Error::NonCanonicalRecord);
        }
    }
    Ok(())
}

/// Create one observation record directly into exact account bytes.
///
/// `seed_digest` is `SHA-256` of [`crate::release::encode_set_digest_seed_preimage_v1`],
/// computed by the caller: this crate hashes nothing.  Every fallible check
/// completes before the caller-owned output changes.
///
/// The record is **not** a Market child and no child-count delta is returned.
/// It is transport held under the Resolution role, created and reclaimed by
/// whoever pays for it, exactly like the provider-update lifecycle account
/// beside it; the successor `CoreState`'s counter is Core-owned and Resolution
/// has no write authority over it.  What bounds creation is the record's own
/// address: it is seeded by the observed slot, so one set at one slot has one
/// place to live, and the worker who creates it funds it.
#[allow(clippy::too_many_arguments)]
pub fn create_relayed_observation_record_into_v1(
    output: &mut [u8],
    binding: RelayedRecordBindingV1,
    rent_credit_beneficiary: [u8; ADDRESS_BYTES],
    set_count: u16,
    seal_threshold: u8,
    seed_digest: [u8; 32],
    created_unix_seconds: i64,
) -> Result<()> {
    if output.len() != relayed_observation_record_bytes_v1(set_count)? {
        return Err(Error::InvalidLength);
    }
    binding.validate()?;
    require_nonzero(&rent_credit_beneficiary)?;
    require_nonzero(&seed_digest)?;
    if seal_threshold == 0 || seal_threshold > crate::MAX_RELAYER_KEYS_V1_U8 {
        return Err(Error::NonCanonicalKeySet);
    }
    if created_unix_seconds <= 0 {
        return Err(Error::NonCanonicalRecord);
    }

    output.fill(0);
    put(output, 0, &RELAYED_RECORD_MAGIC)?;
    put(output, 8, &crate::RELAYED_SCHEMA_VERSION.to_le_bytes())?;
    put(output, RELAYED_RECORD_MARKET_OFFSET, &binding.market)?;
    put(
        output,
        RELAYED_RECORD_GENERATION_OFFSET,
        &binding.generation.to_le_bytes(),
    )?;
    put(
        output,
        RELAYED_RECORD_SOURCE_MATERIAL_ID_OFFSET,
        &binding.source_material_id,
    )?;
    put(
        output,
        RELAYED_RECORD_ACCOUNT_SET_ID_OFFSET,
        &binding.account_set_id,
    )?;
    put(
        output,
        RELAYED_RECORD_PROVIDER_RELEASE_ID_OFFSET,
        &binding.provider_release_id,
    )?;
    put(
        output,
        RELAYED_RECORD_RELAYER_KEY_SET_ID_OFFSET,
        &binding.relayer_key_set_id,
    )?;
    put(
        output,
        RELAYED_RECORD_OBSERVED_CLUSTER_ID_OFFSET,
        &binding.observed_cluster_id,
    )?;
    put(
        output,
        RELAYED_RECORD_OBSERVED_SLOT_OFFSET,
        &binding.observed_slot.to_le_bytes(),
    )?;
    put(output, RELAYED_RECORD_SET_DIGEST_OFFSET, &seed_digest)?;
    put(
        output,
        RELAYED_RECORD_RENT_CREDIT_BENEFICIARY_OFFSET,
        &rent_credit_beneficiary,
    )?;
    put(
        output,
        RELAYED_RECORD_CREATED_UNIX_SECONDS_OFFSET,
        &created_unix_seconds.to_le_bytes(),
    )?;
    put(
        output,
        RELAYED_RECORD_SET_COUNT_OFFSET,
        &set_count.to_le_bytes(),
    )?;
    put(
        output,
        RELAYED_RECORD_SEAL_THRESHOLD_OFFSET,
        &[seal_threshold],
    )?;
    put(
        output,
        RELAYED_RECORD_PHASE_OFFSET,
        &[RelayedRecordPhaseV1::Collecting.byte()],
    )
}

/// The immutable identity every route re-checks against the persisted record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedRecordBindingV1 {
    /// The owning Market.
    pub market: [u8; ADDRESS_BYTES],
    /// The Market generation.
    pub generation: u64,
    /// The immutable Source material.
    pub source_material_id: [u8; 32],
    /// The founding-time pinned ordered account set.
    pub account_set_id: [u8; 32],
    /// The provider release naming the key set and decoding rules.
    pub provider_release_id: [u8; 32],
    /// The immutable relayer key set.
    pub relayer_key_set_id: [u8; 32],
    /// The release-pinned genesis hash of the cluster being observed.
    pub observed_cluster_id: [u8; 32],
    /// The finalized foreign slot this record is seeded by.
    pub observed_slot: u64,
}

impl RelayedRecordBindingV1 {
    fn validate(self) -> Result<()> {
        require_nonzero(&self.market)?;
        for identity in [
            self.source_material_id,
            self.account_set_id,
            self.provider_release_id,
            self.relayer_key_set_id,
            self.observed_cluster_id,
        ] {
            require_nonzero(&identity)?;
        }
        Ok(())
    }

    fn require_matches(self, view: RelayedObservationRecordViewV1<'_>) -> Result<()> {
        if view.observed_cluster_id()? != self.observed_cluster_id {
            return Err(Error::ObservedClusterMismatch);
        }
        if view.account_set_id()? != self.account_set_id {
            return Err(Error::AccountSetMismatch);
        }
        if view.observed_slot()? != self.observed_slot {
            return Err(Error::ObservedSlotMismatch);
        }
        if view.market()? != self.market
            || view.generation()? != self.generation
            || view.source_material_id()? != self.source_material_id
            || view.provider_release_id()? != self.provider_release_id
            || view.relayer_key_set_id()? != self.relayer_key_set_id
        {
            return Err(Error::RecordBindingMismatch);
        }
        Ok(())
    }
}

/// Append one authenticated observation into the next position of a record.
///
/// `folded_digest` is `SHA-256(current_set_digest || body_bytes)`, computed by
/// the caller from the record's own current digest and the exact accepted body.
/// Appends fill strictly increasing positions, so a repeat is a replay refusal
/// rather than an overwrite.
pub fn append_relayed_observation_in_place_v1(
    bytes: &mut [u8],
    binding: RelayedRecordBindingV1,
    message: AttestationMessageV1<'_>,
    folded_digest: [u8; 32],
) -> Result<()> {
    let (offset, body_width, filled, set_count) = {
        let view = RelayedObservationRecordViewV1::decode(bytes)?;
        if view.phase()? != RelayedRecordPhaseV1::Collecting {
            return Err(Error::InvalidRecordTransition);
        }
        binding.require_matches(view)?;

        if message.observed_cluster_id() != binding.observed_cluster_id {
            return Err(Error::ObservedClusterMismatch);
        }
        if message.account_set_id() != binding.account_set_id {
            return Err(Error::AccountSetMismatch);
        }
        if message.observed_slot() != binding.observed_slot {
            return Err(Error::ObservedSlotMismatch);
        }
        let set_count = view.set_count()?;
        if message.set_count() != set_count {
            return Err(Error::InvalidSetGeometry);
        }
        let filled = view.filled_count()?;
        if message.set_index() != filled {
            return Err(Error::InvalidAppendOrder);
        }
        require_nonzero(&folded_digest)?;
        (
            slot_offset(filled)?,
            message.body().encoded_len(),
            filled,
            set_count,
        )
    };
    if body_width > RELAYED_RECORD_SLOT_BYTES {
        return Err(Error::InvalidInlineWidth);
    }
    let next_filled = filled.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    if next_filled > set_count {
        return Err(Error::InvalidSetGeometry);
    }

    let region = bytes
        .get_mut(offset..)
        .ok_or(Error::InvalidLength)?
        .get_mut(..RELAYED_RECORD_SLOT_BYTES)
        .ok_or(Error::InvalidLength)?;
    region.fill(0);
    message.body().encode_into(region)?;
    put(bytes, RELAYED_RECORD_SET_DIGEST_OFFSET, &folded_digest)?;
    put(
        bytes,
        RELAYED_RECORD_FILLED_COUNT_OFFSET,
        &next_filled.to_le_bytes(),
    )?;
    Ok(())
}

/// Record one key-set member's seal over a completed set.
///
/// The record only reaches [`RelayedRecordPhaseV1::Sealed`] when the release's
/// threshold is met, and a member that seals twice is refused rather than
/// counted twice.
pub fn seal_relayed_observation_in_place_v1(
    bytes: &mut [u8],
    binding: RelayedRecordBindingV1,
    seal: ObservationSetSealV1,
    member_index: u8,
    sealed_unix_seconds: i64,
) -> Result<()> {
    let (next_seals, next_bitmap, threshold, reached) = {
        let view = RelayedObservationRecordViewV1::decode(bytes)?;
        if view.phase()? != RelayedRecordPhaseV1::Collecting {
            return Err(Error::InvalidRecordTransition);
        }
        binding.require_matches(view)?;
        if view.filled_count()? != view.set_count()? {
            return Err(Error::InvalidRecordTransition);
        }
        if seal.observed_cluster_id() != binding.observed_cluster_id {
            return Err(Error::ObservedClusterMismatch);
        }
        if seal.account_set_id() != binding.account_set_id {
            return Err(Error::AccountSetMismatch);
        }
        if seal.observed_slot() != binding.observed_slot {
            return Err(Error::ObservedSlotMismatch);
        }
        if seal.set_count() != view.set_count()? {
            return Err(Error::InvalidSetGeometry);
        }
        if seal.set_digest() != view.set_digest()? {
            return Err(Error::SetDigestMismatch);
        }
        if usize::from(member_index) >= crate::MAX_RELAYER_KEYS_V1 {
            return Err(Error::NonCanonicalKeySet);
        }
        let bit = 1u8
            .checked_shl(u32::from(member_index))
            .ok_or(Error::ArithmeticOverflow)?;
        let bitmap = view.sealed_by_bitmap()?;
        if bitmap & bit != 0 {
            return Err(Error::DuplicateSeal);
        }
        let threshold = view.seal_threshold()?;
        let next_seals = view
            .seal_count()?
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if sealed_unix_seconds < view.created_unix_seconds()? {
            return Err(Error::NonCanonicalRecord);
        }
        (next_seals, bitmap | bit, threshold, next_seals >= threshold)
    };
    let _ = threshold;

    put(bytes, RELAYED_RECORD_SEAL_COUNT_OFFSET, &[next_seals])?;
    put(
        bytes,
        RELAYED_RECORD_SEALED_BY_BITMAP_OFFSET,
        &[next_bitmap],
    )?;
    if reached {
        put(
            bytes,
            RELAYED_RECORD_SEALED_UNIX_SECONDS_OFFSET,
            &sealed_unix_seconds.to_le_bytes(),
        )?;
        // The phase byte is written last, so a record can never advertise a
        // seal it does not yet carry the evidence for.
        put(
            bytes,
            RELAYED_RECORD_PHASE_OFFSET,
            &[RelayedRecordPhaseV1::Sealed.byte()],
        )?;
    }
    Ok(())
}

/// Mark one sealed record consumed by a resolution.
///
/// A sealed record is consumable exactly once, which is what stops one signed
/// observation from being replayed into two acceptances.
pub fn consume_relayed_observation_in_place_v1(bytes: &mut [u8]) -> Result<()> {
    {
        let view = RelayedObservationRecordViewV1::decode(bytes)?;
        if view.phase()? != RelayedRecordPhaseV1::Sealed {
            return Err(Error::InvalidRecordTransition);
        }
    }
    put(
        bytes,
        RELAYED_RECORD_PHASE_OFFSET,
        &[RelayedRecordPhaseV1::Consumed.byte()],
    )
}

/// Retire one record into its pre-existing RentCredit beneficiary.
///
/// Retirement is legal from `Collecting`, `Sealed` or `Consumed`.  A record
/// abandoned mid-fill is exactly the wasted rent deposit a bad filler pays for.
pub fn retire_relayed_observation_in_place_v1(
    bytes: &mut [u8],
    generation: u64,
    current_unix_seconds: i64,
) -> Result<()> {
    {
        let view = RelayedObservationRecordViewV1::decode(bytes)?;
        if view.generation()? != generation {
            return Err(Error::RecordBindingMismatch);
        }
        if view.phase()? == RelayedRecordPhaseV1::Retired {
            return Err(Error::InvalidRecordTransition);
        }
        if current_unix_seconds < view.created_unix_seconds()? {
            return Err(Error::NonCanonicalRecord);
        }
    }
    put(
        bytes,
        RELAYED_RECORD_PHASE_OFFSET,
        &[RelayedRecordPhaseV1::Retired.byte()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RELAYED_RECORD_SET_COUNT_OFFSET as SET_COUNT, SOLANA_DEVNET_GENESIS_HASH_V1,
        SOLANA_MAINNET_GENESIS_HASH_V1,
    };

    const MARKET: [u8; 32] = [0x10; 32];
    const GENERATION: u64 = 7;
    const MATERIAL: [u8; 32] = [0x11; 32];
    const ACCOUNT_SET: [u8; 32] = [0x12; 32];
    const RELEASE: [u8; 32] = [0x13; 32];
    const KEY_SET: [u8; 32] = [0x14; 32];
    const BENEFICIARY: [u8; 32] = [0x15; 32];
    const SLOT: u64 = 423_941_138;
    const SEED_DIGEST: [u8; 32] = [0x20; 32];
    const CREATED: i64 = 1_000_000;
    const SET_SIZE: u16 = 2;

    fn binding() -> RelayedRecordBindingV1 {
        RelayedRecordBindingV1 {
            market: MARKET,
            generation: GENERATION,
            source_material_id: MATERIAL,
            account_set_id: ACCOUNT_SET,
            provider_release_id: RELEASE,
            relayer_key_set_id: KEY_SET,
            observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
            observed_slot: SLOT,
        }
    }

    struct Record {
        bytes: [u8; 1432],
    }

    impl Record {
        fn create(threshold: u8) -> Self {
            let width = relayed_observation_record_bytes_v1(SET_SIZE).expect("width");
            assert_eq!(width, 1432);
            let mut bytes = [0u8; 1432];
            create_relayed_observation_record_into_v1(
                &mut bytes,
                binding(),
                BENEFICIARY,
                SET_SIZE,
                threshold,
                SEED_DIGEST,
                CREATED,
            )
            .expect("create");
            Self { bytes }
        }

        fn view(&self) -> RelayedObservationRecordViewV1<'_> {
            RelayedObservationRecordViewV1::decode(&self.bytes).expect("decodes")
        }
    }

    fn body(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn attestation<'a>(
        index: u16,
        inline: &'a [u8],
        cluster: [u8; 32],
        slot: u64,
        account_set: [u8; 32],
    ) -> AttestationMessageV1<'a> {
        let observation = AccountObservationV1::new(
            [0x30 + u8::try_from(index).expect("small"); 32],
            [0x40; 32],
            5,
            u32::try_from(inline.len()).expect("fits"),
            inline,
            false,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("body");
        AttestationMessageV1::new(
            cluster,
            [0x50; 32],
            [0x51; 32],
            account_set,
            slot,
            index,
            SET_SIZE,
            observation,
        )
        .expect("message")
    }

    fn good(index: u16, inline: &[u8]) -> AttestationMessageV1<'_> {
        attestation(
            index,
            inline,
            SOLANA_MAINNET_GENESIS_HASH_V1,
            SLOT,
            ACCOUNT_SET,
        )
    }

    fn seal(digest: [u8; 32]) -> ObservationSetSealV1 {
        ObservationSetSealV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            [0x50; 32],
            ACCOUNT_SET,
            SLOT,
            SET_SIZE,
            digest,
        )
        .expect("seal")
    }

    fn fill(record: &mut Record) {
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(0, &[1, 2, 3]),
            body(0xa1),
        )
        .expect("first append");
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(1, &[4, 5]),
            body(0xa2),
        )
        .expect("second append");
    }

    #[test]
    fn a_created_record_is_collecting_and_carries_its_seed_digest() {
        let record = Record::create(1);
        let view = record.view();
        assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Collecting));
        assert_eq!(view.set_count(), Ok(SET_SIZE));
        assert_eq!(view.filled_count(), Ok(0));
        assert_eq!(view.seal_count(), Ok(0));
        assert_eq!(view.set_digest(), Ok(SEED_DIGEST));
        assert_eq!(view.sealed_unix_seconds(), Ok(0));
        assert_eq!(
            view.observed_cluster_id(),
            Ok(SOLANA_MAINNET_GENESIS_HASH_V1)
        );
    }

    #[test]
    fn the_record_is_runtime_width_in_its_set_count() {
        assert_eq!(relayed_observation_record_bytes_v1(1), Ok(872));
        assert_eq!(relayed_observation_record_bytes_v1(4), Ok(2_552));
        assert_eq!(
            relayed_observation_record_bytes_v1(8),
            Ok(crate::RELAYED_RECORD_MAX_BYTES)
        );
        assert_eq!(
            relayed_observation_record_bytes_v1(0),
            Err(Error::InvalidSetGeometry)
        );
        assert_eq!(
            relayed_observation_record_bytes_v1(9),
            Err(Error::InvalidSetGeometry)
        );
    }

    #[test]
    fn the_full_lifecycle_runs_create_append_seal_consume() {
        let mut record = Record::create(1);
        fill(&mut record);
        assert_eq!(record.view().filled_count(), Ok(SET_SIZE));
        assert_eq!(record.view().set_digest(), Ok(body(0xa2)));
        assert_eq!(record.view().phase(), Ok(RelayedRecordPhaseV1::Collecting));

        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            0,
            CREATED + 5,
        )
        .expect("seal");
        let view = record.view();
        assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Sealed));
        assert_eq!(view.seal_count(), Ok(1));
        assert_eq!(view.sealed_by_bitmap(), Ok(0b0000_0001));
        assert_eq!(view.sealed_unix_seconds(), Ok(CREATED + 5));
        assert_eq!(
            view.require_consumable(
                MARKET,
                GENERATION,
                MATERIAL,
                ACCOUNT_SET,
                RELEASE,
                KEY_SET,
                SOLANA_MAINNET_GENESIS_HASH_V1
            ),
            Ok(())
        );

        consume_relayed_observation_in_place_v1(&mut record.bytes).expect("consume");
        assert_eq!(record.view().phase(), Ok(RelayedRecordPhaseV1::Consumed));
        // A sealed record is consumable exactly once: one signed observation
        // can never be replayed into two acceptances.
        assert_eq!(
            consume_relayed_observation_in_place_v1(&mut record.bytes),
            Err(Error::InvalidRecordTransition)
        );
    }

    #[test]
    fn m_minus_one_seals_leave_the_record_unconsumable() {
        let mut record = Record::create(3);
        fill(&mut record);
        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            0,
            CREATED,
        )
        .expect("first seal");
        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            2,
            CREATED,
        )
        .expect("second seal");
        let view = record.view();
        assert_eq!(view.seal_count(), Ok(2));
        assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Collecting));
        assert_eq!(
            view.require_consumable(
                MARKET,
                GENERATION,
                MATERIAL,
                ACCOUNT_SET,
                RELEASE,
                KEY_SET,
                SOLANA_MAINNET_GENESIS_HASH_V1
            ),
            Err(Error::InvalidRecordTransition)
        );
        assert_eq!(
            consume_relayed_observation_in_place_v1(&mut record.bytes),
            Err(Error::InvalidRecordTransition)
        );

        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            4,
            CREATED,
        )
        .expect("third seal");
        assert_eq!(record.view().phase(), Ok(RelayedRecordPhaseV1::Sealed));
        assert_eq!(record.view().sealed_by_bitmap(), Ok(0b0001_0101));
    }

    #[test]
    fn one_member_cannot_reach_a_quorum_alone() {
        let mut record = Record::create(2);
        fill(&mut record);
        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            1,
            CREATED,
        )
        .expect("seal");
        let before = record.bytes;
        assert_eq!(
            seal_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                seal(body(0xa2)),
                1,
                CREATED
            ),
            Err(Error::DuplicateSeal)
        );
        assert_eq!(record.bytes, before, "a refused seal changed bytes");
        assert_eq!(record.view().phase(), Ok(RelayedRecordPhaseV1::Collecting));
    }

    #[test]
    fn a_replayed_attestation_refuses_on_the_append_order_and_preserves_the_record() {
        let mut record = Record::create(1);
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(0, &[1, 2, 3]),
            body(0xa1),
        )
        .expect("first append");
        let before = record.bytes;
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                good(0, &[1, 2, 3]),
                body(0xa9)
            ),
            Err(Error::InvalidAppendOrder)
        );
        assert_eq!(record.bytes, before, "a refused replay changed bytes");
        // The next position in order still lands, so the refusal above was a
        // replay refusal and not a wedged record.
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                good(1, &[4, 5]),
                body(0xa2)
            ),
            Ok(())
        );
    }

    #[test]
    fn an_attestation_of_the_wrong_cluster_refuses_on_the_cluster_identity() {
        let mut record = Record::create(1);
        let hostile = attestation(
            0,
            &[1, 2, 3],
            SOLANA_DEVNET_GENESIS_HASH_V1,
            SLOT,
            ACCOUNT_SET,
        );
        let before = record.bytes;
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                hostile,
                body(0xa1)
            ),
            Err(Error::ObservedClusterMismatch)
        );
        assert_eq!(record.bytes, before);
    }

    #[test]
    fn an_attestation_of_another_slot_or_another_set_refuses_on_its_own_field() {
        let mut record = Record::create(1);
        let stale = attestation(
            0,
            &[1, 2, 3],
            SOLANA_MAINNET_GENESIS_HASH_V1,
            SLOT - 1,
            ACCOUNT_SET,
        );
        assert_eq!(
            append_relayed_observation_in_place_v1(&mut record.bytes, binding(), stale, body(0xa1)),
            Err(Error::ObservedSlotMismatch)
        );
        let elsewhere = attestation(
            0,
            &[1, 2, 3],
            SOLANA_MAINNET_GENESIS_HASH_V1,
            SLOT,
            [0x99; 32],
        );
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                elsewhere,
                body(0xa1)
            ),
            Err(Error::AccountSetMismatch)
        );
    }

    #[test]
    fn no_append_may_follow_a_seal() {
        let mut record = Record::create(1);
        fill(&mut record);
        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            0,
            CREATED,
        )
        .expect("seal");
        let before = record.bytes;
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                good(0, &[1, 2, 3]),
                body(0xff)
            ),
            Err(Error::InvalidRecordTransition)
        );
        assert_eq!(record.bytes, before);
    }

    #[test]
    fn an_incomplete_set_cannot_be_sealed() {
        let mut record = Record::create(1);
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(0, &[1, 2, 3]),
            body(0xa1),
        )
        .expect("append");
        assert_eq!(
            seal_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                seal(body(0xa1)),
                0,
                CREATED
            ),
            Err(Error::InvalidRecordTransition)
        );
    }

    #[test]
    fn a_seal_over_a_different_fold_refuses() {
        let mut record = Record::create(1);
        fill(&mut record);
        let before = record.bytes;
        assert_eq!(
            seal_relayed_observation_in_place_v1(
                &mut record.bytes,
                binding(),
                seal(body(0xbb)),
                0,
                CREATED
            ),
            Err(Error::SetDigestMismatch)
        );
        assert_eq!(record.bytes, before);
    }

    #[test]
    fn a_substituted_binding_refuses_before_anything_is_written() {
        let mut record = Record::create(1);
        let mut hostile = binding();
        hostile.provider_release_id = [0x99; 32];
        let before = record.bytes;
        assert_eq!(
            append_relayed_observation_in_place_v1(
                &mut record.bytes,
                hostile,
                good(0, &[1, 2, 3]),
                body(0xa1)
            ),
            Err(Error::RecordBindingMismatch)
        );
        assert_eq!(record.bytes, before);
    }

    #[test]
    fn a_record_of_the_wrong_length_refuses() {
        let record = Record::create(1);
        for width in [0usize, 311, 312, 1431] {
            let candidate = record.bytes.get(..width).expect("prefix");
            assert!(
                RelayedObservationRecordViewV1::decode(candidate).is_err(),
                "a {width}-byte record was accepted"
            );
        }
    }

    #[test]
    fn observation_material_in_an_unfilled_slot_refuses() {
        let mut record = Record::create(1);
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(0, &[1, 2, 3]),
            body(0xa1),
        )
        .expect("append");
        let ghost = slot_offset(1).expect("offset");
        put(&mut record.bytes, ghost, &[0x77]).expect("write");
        assert_eq!(
            RelayedObservationRecordViewV1::decode(&record.bytes),
            Err(Error::NonCanonicalRecord)
        );
    }

    #[test]
    fn trailing_bytes_after_a_filled_slots_inline_region_refuse() {
        let mut record = Record::create(1);
        append_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            good(0, &[1, 2, 3]),
            body(0xa1),
        )
        .expect("append");
        let padding = slot_offset(0).expect("offset") + crate::RELAYED_OBSERVATION_HEAD_BYTES + 3;
        put(&mut record.bytes, padding, &[0x77]).expect("write");
        assert_eq!(
            RelayedObservationRecordViewV1::decode(&record.bytes),
            Err(Error::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn a_declared_set_count_the_bytes_cannot_cover_refuses() {
        let mut record = Record::create(1);
        put(&mut record.bytes, SET_COUNT, &3u16.to_le_bytes()).expect("write");
        assert_eq!(
            RelayedObservationRecordViewV1::decode(&record.bytes),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn accepted_observations_read_back_exactly() {
        let mut record = Record::create(1);
        fill(&mut record);
        let view = record.view();
        let first = view.observation(0).expect("first");
        assert_eq!(first.inline(), &[1, 2, 3]);
        assert_eq!(first.key(), [0x30; 32]);
        let second = view.observation(1).expect("second");
        assert_eq!(second.inline(), &[4, 5]);
        assert_eq!(view.observation(2), Err(Error::InvalidSetGeometry));
    }

    #[test]
    fn the_pda_seed_tuple_is_a_function_of_the_slot_so_a_second_record_has_nowhere_to_live() {
        let record = Record::create(1);
        let seeds = record.view().pda_seeds().expect("seeds");
        assert_eq!(seeds.market(), MARKET);
        assert_eq!(seeds.generation_le(), GENERATION.to_le_bytes());
        assert_eq!(seeds.account_set_id(), ACCOUNT_SET);
        assert_eq!(seeds.observed_slot_le(), SLOT.to_le_bytes());
        // Two contradictory observations of the same set at the same slot derive
        // the identical PDA, so the second cannot be created beside the first.
        let same =
            RelayedRecordPdaSeedsV1::new(MARKET, GENERATION, ACCOUNT_SET, SLOT).expect("seeds");
        assert_eq!(seeds, same);
        let later =
            RelayedRecordPdaSeedsV1::new(MARKET, GENERATION, ACCOUNT_SET, SLOT + 1).expect("seeds");
        assert_ne!(seeds, later);
    }

    #[test]
    fn retirement_is_legal_from_every_live_phase_and_is_terminal() {
        let mut collecting = Record::create(1);
        assert!(
            retire_relayed_observation_in_place_v1(&mut collecting.bytes, GENERATION, CREATED)
                .is_ok()
        );
        assert_eq!(collecting.view().phase(), Ok(RelayedRecordPhaseV1::Retired));
        assert_eq!(
            retire_relayed_observation_in_place_v1(&mut collecting.bytes, GENERATION, CREATED),
            Err(Error::InvalidRecordTransition)
        );

        let mut consumed = Record::create(1);
        fill(&mut consumed);
        seal_relayed_observation_in_place_v1(
            &mut consumed.bytes,
            binding(),
            seal(body(0xa2)),
            0,
            CREATED,
        )
        .expect("seal");
        consume_relayed_observation_in_place_v1(&mut consumed.bytes).expect("consume");
        assert!(
            retire_relayed_observation_in_place_v1(&mut consumed.bytes, GENERATION, CREATED)
                .is_ok()
        );
    }

    #[test]
    fn retiring_under_the_wrong_generation_refuses_and_changes_nothing() {
        let mut record = Record::create(1);
        let before = record.bytes;
        assert_eq!(
            retire_relayed_observation_in_place_v1(&mut record.bytes, GENERATION + 1, CREATED),
            Err(Error::RecordBindingMismatch)
        );
        assert_eq!(record.bytes, before, "a refused retirement changed bytes");
    }

    #[test]
    fn a_creation_that_refuses_writes_nothing() {
        // The buffer starts non-zero so that "wrote nothing" is distinguishable
        // from "wrote zeroes": `create` fills before it writes, and a refusal
        // must happen strictly before that fill.
        let mut bytes = [0xa5u8; 1432];
        assert_eq!(
            create_relayed_observation_record_into_v1(
                &mut bytes,
                binding(),
                BENEFICIARY,
                SET_SIZE,
                0,
                SEED_DIGEST,
                CREATED,
            ),
            Err(Error::NonCanonicalKeySet)
        );
        assert!(bytes.iter().all(|byte| *byte == 0xa5));
    }

    #[test]
    fn a_consumable_record_still_refuses_a_substituted_pinned_cluster() {
        let mut record = Record::create(1);
        fill(&mut record);
        seal_relayed_observation_in_place_v1(
            &mut record.bytes,
            binding(),
            seal(body(0xa2)),
            0,
            CREATED,
        )
        .expect("seal");
        assert_eq!(
            record.view().require_consumable(
                MARKET,
                GENERATION,
                MATERIAL,
                ACCOUNT_SET,
                RELEASE,
                KEY_SET,
                SOLANA_DEVNET_GENESIS_HASH_V1
            ),
            Err(Error::ObservedClusterMismatch)
        );
    }
}
