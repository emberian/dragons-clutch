//! One-shot Core-owned capability for recurring-Series Claims founding.
//!
//! The permit PDA is cycle-free: its seeds contain only release, Market, and
//! Ticket context. The body binds the exact Claims V5 intent and request
//! digests. Claims reads but never mutates the permit; Core closes it only
//! after independently authenticating Claims poststate and the final Series
//! opening transition.

use crate::{
    Error, FOUNDING_INTENT_BYTES_V5, Identity, PHYSICAL_ABI_VERSION_V1,
    SERIES_FOUNDING_PERMIT_BYTES_V1, SERIES_FOUNDING_PERMIT_MAGIC_V1,
    generated_physical::{
        SERIES_FOUNDING_INTENT_BASIS_SCALE_OFFSET, SERIES_FOUNDING_INTENT_BUMP_OFFSET,
        SERIES_FOUNDING_INTENT_CLAIMS_PROGRAM_OFFSET, SERIES_FOUNDING_INTENT_EXPIRY_SLOT_OFFSET,
        SERIES_FOUNDING_INTENT_FOUNDER_OFFSET, SERIES_FOUNDING_INTENT_FUNDING_SOURCE_OFFSET,
        SERIES_FOUNDING_INTENT_GENERATION_OFFSET, SERIES_FOUNDING_INTENT_HOARD_OFFSET,
        SERIES_FOUNDING_INTENT_MAGIC_OFFSET, SERIES_FOUNDING_INTENT_MARKET_OFFSET,
        SERIES_FOUNDING_INTENT_NORMAL_REPLAY_REVISION_OFFSET,
        SERIES_FOUNDING_INTENT_PARENT_ROOT_OFFSET, SERIES_FOUNDING_INTENT_PRODUCT_RECORD_OFFSET,
        SERIES_FOUNDING_INTENT_PROJECTED_RECEIPT_DIGEST_OFFSET,
        SERIES_FOUNDING_INTENT_PROJECTED_REPLAY_OFFSET,
        SERIES_FOUNDING_INTENT_PROJECTED_REQUEST_DIGEST_OFFSET,
        SERIES_FOUNDING_INTENT_PROJECTED_RESULTING_REVISION_OFFSET,
        SERIES_FOUNDING_INTENT_QUANTITY_OFFSET, SERIES_FOUNDING_INTENT_RELEASE_SET_OFFSET,
        SERIES_FOUNDING_INTENT_RENT_CREDIT_OFFSET, SERIES_FOUNDING_INTENT_RESERVED_OFFSET,
        SERIES_FOUNDING_INTENT_SOURCE_OFFSET, SERIES_FOUNDING_INTENT_TICKET_CONTEXT_OFFSET,
        SERIES_FOUNDING_INTENT_TRADING_PROGRAM_OFFSET, SERIES_FOUNDING_INTENT_VERSION_OFFSET,
        SERIES_PERMIT_BASIS_SCALE_OFFSET, SERIES_PERMIT_BUMP_OFFSET,
        SERIES_PERMIT_CLAIMS_INTENT_DIGEST_OFFSET, SERIES_PERMIT_CLAIMS_PROGRAM_OFFSET,
        SERIES_PERMIT_CLAIMS_REQUEST_DIGEST_OFFSET, SERIES_PERMIT_EXPIRY_SLOT_OFFSET,
        SERIES_PERMIT_FOUNDER_OFFSET, SERIES_PERMIT_FUNDING_SOURCE_OFFSET,
        SERIES_PERMIT_GENERATION_OFFSET, SERIES_PERMIT_HOARD_OFFSET, SERIES_PERMIT_MAGIC_OFFSET,
        SERIES_PERMIT_MARKET_OFFSET, SERIES_PERMIT_NORMAL_REPLAY_REVISION_OFFSET,
        SERIES_PERMIT_PARENT_ROOT_OFFSET, SERIES_PERMIT_PRODUCT_RECORD_OFFSET,
        SERIES_PERMIT_PROJECTED_RECEIPT_DIGEST_OFFSET, SERIES_PERMIT_PROJECTED_REPLAY_OFFSET,
        SERIES_PERMIT_PROJECTED_REQUEST_DIGEST_OFFSET,
        SERIES_PERMIT_PROJECTED_RESULTING_REVISION_OFFSET, SERIES_PERMIT_QUANTITY_OFFSET,
        SERIES_PERMIT_RELEASE_SET_OFFSET, SERIES_PERMIT_RENT_CREDIT_OFFSET,
        SERIES_PERMIT_RESERVED_OFFSET, SERIES_PERMIT_SOURCE_OFFSET,
        SERIES_PERMIT_TICKET_CONTEXT_OFFSET, SERIES_PERMIT_TRADING_PROGRAM_OFFSET,
        SERIES_PERMIT_VERSION_OFFSET,
    },
};

const IDENTITY_BYTES: usize = 32;

/// Canonical Claims V5 founding intent derived from a Series permit.
///
/// This is the exact 544-byte permit projection with only
/// `claims_intent_digest` and `claims_request_digest` omitted. It retains the
/// permit header so every producer hashes precisely the same projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingIntentV5 {
    bump: u8,
    release_set: Identity,
    market: Identity,
    product_record: Identity,
    source: Identity,
    founder: Identity,
    ticket_context: Identity,
    parent_root: Identity,
    projected_replay: Identity,
    funding_source: Identity,
    hoard: Identity,
    projected_request_digest: Identity,
    projected_receipt_digest: Identity,
    trading_program: Identity,
    claims_program: Identity,
    rent_credit: Identity,
    generation: u64,
    quantity: u64,
    basis_scale: u64,
    expiry_slot: u64,
    projected_resulting_revision: u64,
    normal_replay_revision: u64,
}

impl FoundingIntentV5 {
    /// Construct one exact cycle-free Claims founding intent.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bump: u8,
        release_set: Identity,
        market: Identity,
        product_record: Identity,
        source: Identity,
        founder: Identity,
        ticket_context: Identity,
        parent_root: Identity,
        projected_replay: Identity,
        funding_source: Identity,
        hoard: Identity,
        projected_request_digest: Identity,
        projected_receipt_digest: Identity,
        trading_program: Identity,
        claims_program: Identity,
        rent_credit: Identity,
        generation: u64,
        quantity: u64,
        basis_scale: u64,
        expiry_slot: u64,
        projected_resulting_revision: u64,
        normal_replay_revision: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            bump,
            release_set,
            market,
            product_record,
            source,
            founder,
            ticket_context,
            parent_root,
            projected_replay,
            funding_source,
            hoard,
            projected_request_digest,
            projected_receipt_digest,
            trading_program,
            claims_program,
            rent_credit,
            generation,
            quantity,
            basis_scale,
            expiry_slot,
            projected_resulting_revision,
            normal_replay_revision,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact canonical intent projection.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            FOUNDING_INTENT_BYTES_V5,
            SERIES_FOUNDING_INTENT_MAGIC_OFFSET,
            SERIES_FOUNDING_INTENT_VERSION_OFFSET,
            SERIES_FOUNDING_INTENT_RESERVED_OFFSET,
        )?;
        let value = Self {
            bump: read_u8(input, SERIES_FOUNDING_INTENT_BUMP_OFFSET)?,
            release_set: read_identity(input, SERIES_FOUNDING_INTENT_RELEASE_SET_OFFSET)?,
            market: read_identity(input, SERIES_FOUNDING_INTENT_MARKET_OFFSET)?,
            product_record: read_identity(input, SERIES_FOUNDING_INTENT_PRODUCT_RECORD_OFFSET)?,
            source: read_identity(input, SERIES_FOUNDING_INTENT_SOURCE_OFFSET)?,
            founder: read_identity(input, SERIES_FOUNDING_INTENT_FOUNDER_OFFSET)?,
            ticket_context: read_identity(input, SERIES_FOUNDING_INTENT_TICKET_CONTEXT_OFFSET)?,
            parent_root: read_identity(input, SERIES_FOUNDING_INTENT_PARENT_ROOT_OFFSET)?,
            projected_replay: read_identity(input, SERIES_FOUNDING_INTENT_PROJECTED_REPLAY_OFFSET)?,
            funding_source: read_identity(input, SERIES_FOUNDING_INTENT_FUNDING_SOURCE_OFFSET)?,
            hoard: read_identity(input, SERIES_FOUNDING_INTENT_HOARD_OFFSET)?,
            projected_request_digest: read_identity(
                input,
                SERIES_FOUNDING_INTENT_PROJECTED_REQUEST_DIGEST_OFFSET,
            )?,
            projected_receipt_digest: read_identity(
                input,
                SERIES_FOUNDING_INTENT_PROJECTED_RECEIPT_DIGEST_OFFSET,
            )?,
            trading_program: read_identity(input, SERIES_FOUNDING_INTENT_TRADING_PROGRAM_OFFSET)?,
            claims_program: read_identity(input, SERIES_FOUNDING_INTENT_CLAIMS_PROGRAM_OFFSET)?,
            rent_credit: read_identity(input, SERIES_FOUNDING_INTENT_RENT_CREDIT_OFFSET)?,
            generation: read_u64(input, SERIES_FOUNDING_INTENT_GENERATION_OFFSET)?,
            quantity: read_u64(input, SERIES_FOUNDING_INTENT_QUANTITY_OFFSET)?,
            basis_scale: read_u64(input, SERIES_FOUNDING_INTENT_BASIS_SCALE_OFFSET)?,
            expiry_slot: read_u64(input, SERIES_FOUNDING_INTENT_EXPIRY_SLOT_OFFSET)?,
            projected_resulting_revision: read_u64(
                input,
                SERIES_FOUNDING_INTENT_PROJECTED_RESULTING_REVISION_OFFSET,
            )?,
            normal_replay_revision: read_u64(
                input,
                SERIES_FOUNDING_INTENT_NORMAL_REPLAY_REVISION_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact canonical intent projection.
    pub fn encode(self) -> Result<[u8; FOUNDING_INTENT_BYTES_V5], Error> {
        self.validate()?;
        let mut output = [0; FOUNDING_INTENT_BYTES_V5];
        put(
            &mut output,
            SERIES_FOUNDING_INTENT_MAGIC_OFFSET,
            &SERIES_FOUNDING_PERMIT_MAGIC_V1,
        )?;
        put_u16(
            &mut output,
            SERIES_FOUNDING_INTENT_VERSION_OFFSET,
            PHYSICAL_ABI_VERSION_V1,
        )?;
        put_u8(&mut output, SERIES_FOUNDING_INTENT_BUMP_OFFSET, self.bump)?;
        put_intent_identities(&mut output, self)?;
        put_intent_scalars(&mut output, self)?;
        Ok(output)
    }

    fn validate(self) -> Result<(), Error> {
        validate_coordinates(
            self.bump,
            self.market,
            self.funding_source,
            self.hoard,
            self.projected_replay,
            self.trading_program,
            self.claims_program,
            self.generation,
            self.quantity,
            self.basis_scale,
            self.expiry_slot,
            self.projected_resulting_revision,
            self.normal_replay_revision,
        )
    }

    /// Exact PDA bump selected by Core.
    pub const fn bump(self) -> u8 {
        self.bump
    }
    /// Current immutable release set.
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Future/newly-founded Market.
    pub const fn market(self) -> Identity {
        self.market
    }
    /// Runtime Product record root.
    pub const fn product_record(self) -> Identity {
        self.product_record
    }
    /// Source material selected by the Market.
    pub const fn source(self) -> Identity {
        self.source
    }
    /// Immutable founding beneficiary.
    pub const fn founder(self) -> Identity {
        self.founder
    }
    /// Exact finalized Ticket content identity.
    pub const fn ticket_context(self) -> Identity {
        self.ticket_context
    }
    /// Trading composite-root identity.
    pub const fn parent_root(self) -> Identity {
        self.parent_root
    }
    /// Projected replay rewritten into the normal Custody replay.
    pub const fn projected_replay(self) -> Identity {
        self.projected_replay
    }
    /// Exact pre-founding funding source Vault.
    pub const fn funding_source(self) -> Identity {
        self.funding_source
    }
    /// Exact newly credited Hoard Vault.
    pub const fn hoard(self) -> Identity {
        self.hoard
    }
    /// Exact deterministic Custody Realize request digest.
    pub const fn projected_request_digest(self) -> Identity {
        self.projected_request_digest
    }
    /// SHA-256 digest of the exact deterministic 320-byte Realize receipt.
    pub const fn projected_receipt_digest(self) -> Identity {
        self.projected_receipt_digest
    }
    /// Current Registry-selected Trading program.
    pub const fn trading_program(self) -> Identity {
        self.trading_program
    }
    /// Current Registry-selected Claims program.
    pub const fn claims_program(self) -> Identity {
        self.claims_program
    }
    /// Immutable permit-close and expiry-refund destination.
    pub const fn rent_credit(self) -> Identity {
        self.rent_credit
    }
    /// Nonzero occurrence-derived Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact positive complete-set quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
    /// Exact positive semantic basis scale.
    pub const fn basis_scale(self) -> u64 {
        self.basis_scale
    }
    /// Last slot at which final opening is accepted.
    pub const fn expiry_slot(self) -> u64 {
        self.expiry_slot
    }
    /// Terminal projected-Custody revision before replay rewrite.
    pub const fn projected_resulting_revision(self) -> u64 {
        self.projected_resulting_revision
    }
    /// Rewritten normal Custody replay revision, canonically one.
    pub const fn normal_replay_revision(self) -> u64 {
        self.normal_replay_revision
    }
}

/// Immutable Core-owned one-shot Claims founding capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFoundingPermitV1 {
    intent: FoundingIntentV5,
    claims_intent_digest: Identity,
    claims_request_digest: Identity,
}

impl SeriesFoundingPermitV1 {
    /// Construct the canonical body after Core has authenticated all inputs.
    pub fn new(
        intent: FoundingIntentV5,
        claims_intent_digest: Identity,
        claims_request_digest: Identity,
    ) -> Result<Self, Error> {
        intent.validate()?;
        if claims_intent_digest == claims_request_digest {
            return Err(Error::InvalidAlias);
        }
        Ok(Self {
            intent,
            claims_intent_digest,
            claims_request_digest,
        })
    }

    /// Hostile-decode one exact Core-owned permit.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            SERIES_FOUNDING_PERMIT_BYTES_V1,
            SERIES_PERMIT_MAGIC_OFFSET,
            SERIES_PERMIT_VERSION_OFFSET,
            SERIES_PERMIT_RESERVED_OFFSET,
        )?;
        let intent = FoundingIntentV5 {
            bump: read_u8(input, SERIES_PERMIT_BUMP_OFFSET)?,
            release_set: read_identity(input, SERIES_PERMIT_RELEASE_SET_OFFSET)?,
            market: read_identity(input, SERIES_PERMIT_MARKET_OFFSET)?,
            product_record: read_identity(input, SERIES_PERMIT_PRODUCT_RECORD_OFFSET)?,
            source: read_identity(input, SERIES_PERMIT_SOURCE_OFFSET)?,
            founder: read_identity(input, SERIES_PERMIT_FOUNDER_OFFSET)?,
            ticket_context: read_identity(input, SERIES_PERMIT_TICKET_CONTEXT_OFFSET)?,
            parent_root: read_identity(input, SERIES_PERMIT_PARENT_ROOT_OFFSET)?,
            projected_replay: read_identity(input, SERIES_PERMIT_PROJECTED_REPLAY_OFFSET)?,
            funding_source: read_identity(input, SERIES_PERMIT_FUNDING_SOURCE_OFFSET)?,
            hoard: read_identity(input, SERIES_PERMIT_HOARD_OFFSET)?,
            projected_request_digest: read_identity(
                input,
                SERIES_PERMIT_PROJECTED_REQUEST_DIGEST_OFFSET,
            )?,
            projected_receipt_digest: read_identity(
                input,
                SERIES_PERMIT_PROJECTED_RECEIPT_DIGEST_OFFSET,
            )?,
            trading_program: read_identity(input, SERIES_PERMIT_TRADING_PROGRAM_OFFSET)?,
            claims_program: read_identity(input, SERIES_PERMIT_CLAIMS_PROGRAM_OFFSET)?,
            rent_credit: read_identity(input, SERIES_PERMIT_RENT_CREDIT_OFFSET)?,
            generation: read_u64(input, SERIES_PERMIT_GENERATION_OFFSET)?,
            quantity: read_u64(input, SERIES_PERMIT_QUANTITY_OFFSET)?,
            basis_scale: read_u64(input, SERIES_PERMIT_BASIS_SCALE_OFFSET)?,
            expiry_slot: read_u64(input, SERIES_PERMIT_EXPIRY_SLOT_OFFSET)?,
            projected_resulting_revision: read_u64(
                input,
                SERIES_PERMIT_PROJECTED_RESULTING_REVISION_OFFSET,
            )?,
            normal_replay_revision: read_u64(input, SERIES_PERMIT_NORMAL_REPLAY_REVISION_OFFSET)?,
        };
        Self::new(
            intent,
            read_identity(input, SERIES_PERMIT_CLAIMS_INTENT_DIGEST_OFFSET)?,
            read_identity(input, SERIES_PERMIT_CLAIMS_REQUEST_DIGEST_OFFSET)?,
        )
    }

    /// Encode the exact immutable permit body.
    pub fn encode(self) -> Result<[u8; SERIES_FOUNDING_PERMIT_BYTES_V1], Error> {
        self.intent.validate()?;
        let mut output = [0; SERIES_FOUNDING_PERMIT_BYTES_V1];
        put(
            &mut output,
            SERIES_PERMIT_MAGIC_OFFSET,
            &SERIES_FOUNDING_PERMIT_MAGIC_V1,
        )?;
        put_u16(
            &mut output,
            SERIES_PERMIT_VERSION_OFFSET,
            PHYSICAL_ABI_VERSION_V1,
        )?;
        put_u8(&mut output, SERIES_PERMIT_BUMP_OFFSET, self.intent.bump)?;
        put_permit_identities(&mut output, self)?;
        put_permit_scalars(&mut output, self.intent)?;
        Ok(output)
    }

    /// Verify the exact canonical intent projection and Claims V5 request
    /// digest observed by an adapter.
    pub fn verify_for_intent_and_request(
        self,
        observed_intent: FoundingIntentV5,
        observed_intent_digest: Identity,
        observed_request_digest: Identity,
    ) -> Result<(), Error> {
        if self.intent != observed_intent
            || self.claims_intent_digest != observed_intent_digest
            || self.claims_request_digest != observed_request_digest
        {
            return Err(Error::InvalidCoordinates);
        }
        Ok(())
    }

    /// Project the exact Claims V5 founding intent.
    pub const fn intent(self) -> FoundingIntentV5 {
        self.intent
    }
    /// SHA-256 digest of [`FoundingIntentV5::encode`].
    pub const fn claims_intent_digest(self) -> Identity {
        self.claims_intent_digest
    }
    /// SHA-256 digest of the exact Claims Founding V5 request bytes.
    pub const fn claims_request_digest(self) -> Identity {
        self.claims_request_digest
    }
    /// Project the sole cycle-free Core permit PDA seeds.
    pub const fn seeds(self) -> SeriesFoundingPermitSeedsV1 {
        SeriesFoundingPermitSeedsV1::new(
            self.intent.release_set,
            self.intent.market,
            self.intent.ticket_context,
        )
    }
}

/// Cycle-free PDA seed projection for one Series founding permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFoundingPermitSeedsV1 {
    release_set: [u8; IDENTITY_BYTES],
    market: [u8; IDENTITY_BYTES],
    ticket_context: [u8; IDENTITY_BYTES],
}

impl SeriesFoundingPermitSeedsV1 {
    /// Construct the exact seed coordinates.
    #[must_use]
    pub const fn new(release_set: Identity, market: Identity, ticket_context: Identity) -> Self {
        Self {
            release_set: release_set.to_bytes(),
            market: market.to_bytes(),
            ticket_context: ticket_context.to_bytes(),
        }
    }

    /// Return the sole seed order, excluding the bump.
    #[must_use]
    pub fn as_slices(&self) -> [&[u8]; 4] {
        [
            crate::SERIES_FOUNDING_PERMIT_PDA_DOMAIN_V1.as_slice(),
            &self.release_set,
            &self.market,
            &self.ticket_context,
        ]
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_coordinates(
    _bump: u8,
    market: Identity,
    funding_source: Identity,
    hoard: Identity,
    projected_replay: Identity,
    trading_program: Identity,
    claims_program: Identity,
    generation: u64,
    quantity: u64,
    basis_scale: u64,
    expiry_slot: u64,
    projected_resulting_revision: u64,
    normal_replay_revision: u64,
) -> Result<(), Error> {
    if market == funding_source
        || market == hoard
        || market == projected_replay
        || funding_source == hoard
        || projected_replay == hoard
        || trading_program == claims_program
        || generation == 0
        || quantity == 0
        || basis_scale == 0
        || quantity.checked_mul(basis_scale).is_none()
        || expiry_slot == 0
        || projected_resulting_revision == 0
        || normal_replay_revision != 1
    {
        return Err(Error::InvalidCoordinates);
    }
    Ok(())
}

fn exact_header(
    input: &[u8],
    expected_len: usize,
    magic_offset: usize,
    version_offset: usize,
    reserved_offset: usize,
) -> Result<(), Error> {
    if input.len() != expected_len {
        return Err(Error::InvalidLength);
    }
    if input.get(magic_offset..magic_offset.saturating_add(8))
        != Some(SERIES_FOUNDING_PERMIT_MAGIC_V1.as_slice())
    {
        return Err(Error::InvalidMagic);
    }
    if read_u16(input, version_offset)? != PHYSICAL_ABI_VERSION_V1 {
        return Err(Error::UnsupportedVersion);
    }
    if input
        .get(reserved_offset..reserved_offset.saturating_add(5))
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonzeroReserved);
    }
    Ok(())
}

fn put_intent_identities(output: &mut [u8], value: FoundingIntentV5) -> Result<(), Error> {
    for (offset, identity) in [
        (SERIES_FOUNDING_INTENT_RELEASE_SET_OFFSET, value.release_set),
        (SERIES_FOUNDING_INTENT_MARKET_OFFSET, value.market),
        (
            SERIES_FOUNDING_INTENT_PRODUCT_RECORD_OFFSET,
            value.product_record,
        ),
        (SERIES_FOUNDING_INTENT_SOURCE_OFFSET, value.source),
        (SERIES_FOUNDING_INTENT_FOUNDER_OFFSET, value.founder),
        (
            SERIES_FOUNDING_INTENT_TICKET_CONTEXT_OFFSET,
            value.ticket_context,
        ),
        (SERIES_FOUNDING_INTENT_PARENT_ROOT_OFFSET, value.parent_root),
        (
            SERIES_FOUNDING_INTENT_PROJECTED_REPLAY_OFFSET,
            value.projected_replay,
        ),
        (
            SERIES_FOUNDING_INTENT_FUNDING_SOURCE_OFFSET,
            value.funding_source,
        ),
        (SERIES_FOUNDING_INTENT_HOARD_OFFSET, value.hoard),
        (
            SERIES_FOUNDING_INTENT_PROJECTED_REQUEST_DIGEST_OFFSET,
            value.projected_request_digest,
        ),
        (
            SERIES_FOUNDING_INTENT_PROJECTED_RECEIPT_DIGEST_OFFSET,
            value.projected_receipt_digest,
        ),
        (
            SERIES_FOUNDING_INTENT_TRADING_PROGRAM_OFFSET,
            value.trading_program,
        ),
        (
            SERIES_FOUNDING_INTENT_CLAIMS_PROGRAM_OFFSET,
            value.claims_program,
        ),
        (SERIES_FOUNDING_INTENT_RENT_CREDIT_OFFSET, value.rent_credit),
    ] {
        put_identity(output, offset, identity)?;
    }
    Ok(())
}

fn put_intent_scalars(output: &mut [u8], value: FoundingIntentV5) -> Result<(), Error> {
    for (offset, scalar) in [
        (SERIES_FOUNDING_INTENT_GENERATION_OFFSET, value.generation),
        (SERIES_FOUNDING_INTENT_QUANTITY_OFFSET, value.quantity),
        (SERIES_FOUNDING_INTENT_BASIS_SCALE_OFFSET, value.basis_scale),
        (SERIES_FOUNDING_INTENT_EXPIRY_SLOT_OFFSET, value.expiry_slot),
        (
            SERIES_FOUNDING_INTENT_PROJECTED_RESULTING_REVISION_OFFSET,
            value.projected_resulting_revision,
        ),
        (
            SERIES_FOUNDING_INTENT_NORMAL_REPLAY_REVISION_OFFSET,
            value.normal_replay_revision,
        ),
    ] {
        put_u64(output, offset, scalar)?;
    }
    Ok(())
}

fn put_permit_identities(output: &mut [u8], value: SeriesFoundingPermitV1) -> Result<(), Error> {
    let intent = value.intent;
    for (offset, identity) in [
        (SERIES_PERMIT_RELEASE_SET_OFFSET, intent.release_set),
        (SERIES_PERMIT_MARKET_OFFSET, intent.market),
        (SERIES_PERMIT_PRODUCT_RECORD_OFFSET, intent.product_record),
        (SERIES_PERMIT_SOURCE_OFFSET, intent.source),
        (SERIES_PERMIT_FOUNDER_OFFSET, intent.founder),
        (SERIES_PERMIT_TICKET_CONTEXT_OFFSET, intent.ticket_context),
        (SERIES_PERMIT_PARENT_ROOT_OFFSET, intent.parent_root),
        (
            SERIES_PERMIT_PROJECTED_REPLAY_OFFSET,
            intent.projected_replay,
        ),
        (SERIES_PERMIT_FUNDING_SOURCE_OFFSET, intent.funding_source),
        (SERIES_PERMIT_HOARD_OFFSET, intent.hoard),
        (
            SERIES_PERMIT_PROJECTED_REQUEST_DIGEST_OFFSET,
            intent.projected_request_digest,
        ),
        (
            SERIES_PERMIT_PROJECTED_RECEIPT_DIGEST_OFFSET,
            intent.projected_receipt_digest,
        ),
        (
            SERIES_PERMIT_CLAIMS_INTENT_DIGEST_OFFSET,
            value.claims_intent_digest,
        ),
        (
            SERIES_PERMIT_CLAIMS_REQUEST_DIGEST_OFFSET,
            value.claims_request_digest,
        ),
        (SERIES_PERMIT_TRADING_PROGRAM_OFFSET, intent.trading_program),
        (SERIES_PERMIT_CLAIMS_PROGRAM_OFFSET, intent.claims_program),
        (SERIES_PERMIT_RENT_CREDIT_OFFSET, intent.rent_credit),
    ] {
        put_identity(output, offset, identity)?;
    }
    Ok(())
}

fn put_permit_scalars(output: &mut [u8], value: FoundingIntentV5) -> Result<(), Error> {
    for (offset, scalar) in [
        (SERIES_PERMIT_GENERATION_OFFSET, value.generation),
        (SERIES_PERMIT_QUANTITY_OFFSET, value.quantity),
        (SERIES_PERMIT_BASIS_SCALE_OFFSET, value.basis_scale),
        (SERIES_PERMIT_EXPIRY_SLOT_OFFSET, value.expiry_slot),
        (
            SERIES_PERMIT_PROJECTED_RESULTING_REVISION_OFFSET,
            value.projected_resulting_revision,
        ),
        (
            SERIES_PERMIT_NORMAL_REPLAY_REVISION_OFFSET,
            value.normal_replay_revision,
        ),
    ] {
        put_u64(output, offset, scalar)?;
    }
    Ok(())
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_identity(input: &[u8], offset: usize) -> Result<Identity, Error> {
    Identity::new(read_array(input, offset)?)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    input
        .get(offset..offset.saturating_add(N))
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    output
        .get_mut(offset..offset.saturating_add(bytes.len()))
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn put_u8(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_identity(output: &mut [u8], offset: usize, value: Identity) -> Result<(), Error> {
    put(output, offset, &value.to_bytes())
}
