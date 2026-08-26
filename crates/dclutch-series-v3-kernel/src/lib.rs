#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free recurring-Series V3 record admission.
//!
//! This kernel is the one semantic owner of immutable Template, occurrence,
//! and Ticket bytes.  It hostile-decodes the Lean-owned fixed layouts, checks
//! exact scheduling and occurrence proofs, binds prepaid funding, and projects
//! the complete future-Market identity.  It performs no account access, PDA
//! derivation, CPI, token movement, or mutation; those are adapter boundaries.

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, SeriesCoreActionV1,
    SeriesCoreRequestV1, SeriesPermitExpiryRequestV1,
};
use sha2::{Digest, Sha256};

/// Complete stateless Ticket-to-Found Consume composition.
pub mod composition;
/// Stateless pre-founding SeriesEscrow effect sequence.
pub mod escrow;
/// Lean-generated Series V3 widths, offsets, domains, and hostile examples.
#[allow(dead_code, missing_docs)]
#[doc(hidden)]
pub mod generated;
/// Stateless joint root/Ticket replay-plan evaluator.
pub mod plan;
/// Total fixed-layout Series/Ticket replay evaluator.
pub mod replay;
/// Exact Series action request and occurrence-proof wire.
pub mod request;
/// Stateless complete semantic plan for Shadow-AOT and differential execution.
pub mod shadow;

pub use generated::{
    SERIES_MAXIMUM_MERKLE_HEIGHT_V3, SERIES_OCCURRENCE_BYTES_V3,
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_OCCURRENCE_SCHEMA_RELEASE_PREIMAGE_V3,
    SERIES_TEMPLATE_BYTES_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TEMPLATE_SCHEMA_RELEASE_PREIMAGE_V3, SERIES_TICKET_BYTES_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, SERIES_TICKET_SCHEMA_RELEASE_PREIMAGE_V3,
};

const HEADER_VERSION_OFFSET: usize = 8;
const HEADER_PROFILE_OFFSET: usize = 10;
const HASH_SEPARATOR: [u8; 1] = [0];

/// Refusal from exact Series V3 content admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesV3Error {
    /// An input had the wrong fixed width.
    Length,
    /// Magic, version, profile, or reserved bytes were noncanonical.
    Header,
    /// A content or account identity was the reserved all-zero value.
    Identity,
    /// A schedule coordinate overflowed or did not match its Template.
    Schedule,
    /// Exact founding funding was invalid or did not match.
    Funding,
    /// A content digest, occurrence projection, or Ticket commitment differed.
    Commitment,
    /// Future-Market coordinates or the derived address differed.
    Market,
    /// An adapter requested an action outside the occurrence/Core seam.
    Action,
}

/// Opaque nonzero 32-byte account or program identity.
///
/// The kernel deliberately does not import Solana's `Pubkey`; an adapter may
/// convert these exact bytes only after successful hostile decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct AccountKeyV3([u8; 32]);

impl AccountKeyV3 {
    /// Validate and construct one nonzero account key.
    pub fn new(bytes: [u8; 32]) -> Result<Self, SeriesV3Error> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(SeriesV3Error::Identity);
        }
        Ok(Self(bytes))
    }

    /// Return exact key bytes.
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Borrow exact key bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Four disjoint occurrence-owned founding compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingFundsV3 {
    hoard_principal: u64,
    market_rent: u64,
    capability_native: u64,
    founding_work: u64,
}

impl FoundingFundsV3 {
    /// Return the checked native-lamport total.
    ///
    /// Hoard principal is denominated in Realm collateral and is deliberately
    /// excluded. It is never added to lamports.
    pub fn checked_native_total(self) -> Result<u64, SeriesV3Error> {
        self.market_rent
            .checked_add(self.capability_native)
            .and_then(|total| total.checked_add(self.founding_work))
            .ok_or(SeriesV3Error::Funding)
    }

    /// Return collateral principal; it is never rent, fees, or work funding.
    pub const fn hoard_principal(self) -> u64 {
        self.hoard_principal
    }

    /// Return exact prepaid Market account rent.
    pub const fn market_rent(self) -> u64 {
        self.market_rent
    }

    /// Return exact prepaid native capability-account funding.
    pub const fn capability_native(self) -> u64 {
        self.capability_native
    }

    /// Return exact prepaid founding work capital.
    pub const fn founding_work(self) -> u64 {
        self.founding_work
    }
}

/// Hostile-decoded immutable recurring Template V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateV3 {
    realm: ContentId,
    release_set: ContentId,
    product_generator: ContentId,
    occurrence_generator: ContentId,
    capability_template: ContentId,
    product_derivation: ContentId,
    occurrence_derivation: ContentId,
    capability_derivation: ContentId,
    funding_derivation: ContentId,
    projection_root: ContentId,
    refund_owner: AccountKeyV3,
    occurrence_count: u32,
    first_slot: u64,
    period_slots: u64,
    retry_window: u64,
    close_rent: u64,
}

impl TemplateV3 {
    /// Decode one exact Lean-owned 400-byte Template record.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV3Error> {
        exact_header(
            bytes,
            generated::SERIES_TEMPLATE_BYTES_V3,
            &generated::SERIES_TEMPLATE_MAGIC_V3,
        )?;
        let value = Self {
            occurrence_count: read_u32(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
            )?,
            first_slot: read_u64(bytes, generated::SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3)?,
            period_slots: read_u64(bytes, generated::SERIES_TEMPLATE_PERIOD_SLOTS_OFFSET_V3)?,
            retry_window: read_u64(bytes, generated::SERIES_TEMPLATE_RETRY_WINDOW_OFFSET_V3)?,
            close_rent: read_u64(bytes, generated::SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3)?,
            realm: read_content_id(bytes, generated::SERIES_TEMPLATE_REALM_OFFSET_V3)?,
            release_set: read_content_id(bytes, generated::SERIES_TEMPLATE_RELEASE_SET_OFFSET_V3)?,
            product_generator: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PRODUCT_GENERATOR_OFFSET_V3,
            )?,
            occurrence_generator: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_GENERATOR_OFFSET_V3,
            )?,
            capability_template: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_CAPABILITY_TEMPLATE_OFFSET_V3,
            )?,
            product_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PRODUCT_DERIVATION_OFFSET_V3,
            )?,
            occurrence_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_DERIVATION_OFFSET_V3,
            )?,
            capability_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_CAPABILITY_DERIVATION_OFFSET_V3,
            )?,
            funding_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_FUNDING_DERIVATION_OFFSET_V3,
            )?,
            projection_root: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            )?,
            refund_owner: read_account_key(
                bytes,
                generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
            )?,
        };
        if value.occurrence_count == 0 || value.period_slots == 0 {
            return Err(SeriesV3Error::Schedule);
        }
        value.retry_through(value.occurrence_count - 1)?;
        Ok(value)
    }

    /// Return the immutable Realm content identity.
    pub const fn realm(self) -> ContentId {
        self.realm
    }

    /// Return the immutable ReleaseSet content identity.
    pub const fn release_set(self) -> ContentId {
        self.release_set
    }

    /// Return the reusable Product-generator identity.
    pub const fn product_generator(self) -> ContentId {
        self.product_generator
    }

    /// Return the reusable occurrence-generator identity.
    pub const fn occurrence_generator(self) -> ContentId {
        self.occurrence_generator
    }

    /// Return the reusable capability-template identity.
    pub const fn capability_template(self) -> ContentId {
        self.capability_template
    }

    /// Return the Product derivation-policy identity.
    pub const fn product_derivation(self) -> ContentId {
        self.product_derivation
    }

    /// Return the occurrence derivation-policy identity.
    pub const fn occurrence_derivation(self) -> ContentId {
        self.occurrence_derivation
    }

    /// Return the capability derivation-policy identity.
    pub const fn capability_derivation(self) -> ContentId {
        self.capability_derivation
    }

    /// Return the funding derivation-policy identity.
    pub const fn funding_derivation(self) -> ContentId {
        self.funding_derivation
    }

    /// Return the exact committed occurrence-projection root.
    pub const fn projection_root(self) -> ContentId {
        self.projection_root
    }

    /// Return the immutable terminal Series rent-refund owner.
    pub const fn refund_owner(self) -> AccountKeyV3 {
        self.refund_owner
    }

    /// Return the finite number of scheduled occurrences.
    pub const fn occurrence_count(self) -> u32 {
        self.occurrence_count
    }

    /// Return the first scheduled slot.
    pub const fn first_slot(self) -> u64 {
        self.first_slot
    }

    /// Return the positive occurrence period in slots.
    pub const fn period_slots(self) -> u64 {
        self.period_slots
    }

    /// Return the inclusive retry-window width in slots.
    pub const fn retry_window(self) -> u64 {
        self.retry_window
    }

    /// Return separately prepaid terminal close rent.
    pub const fn close_rent(self) -> u64 {
        self.close_rent
    }

    /// Derive one exact scheduled slot with checked arithmetic.
    pub fn scheduled_slot(self, occurrence: u32) -> Result<u64, SeriesV3Error> {
        if occurrence >= self.occurrence_count {
            return Err(SeriesV3Error::Schedule);
        }
        self.first_slot
            .checked_add(
                u64::from(occurrence)
                    .checked_mul(self.period_slots)
                    .ok_or(SeriesV3Error::Schedule)?,
            )
            .ok_or(SeriesV3Error::Schedule)
    }

    /// Derive the inclusive last retry slot with checked arithmetic.
    pub fn retry_through(self, occurrence: u32) -> Result<u64, SeriesV3Error> {
        self.scheduled_slot(occurrence)?
            .checked_add(self.retry_window)
            .ok_or(SeriesV3Error::Schedule)
    }
}

/// Hostile-decoded exact realized occurrence V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceV3 {
    occurrence: u32,
    scheduled_slot: u64,
    product_record: ContentId,
    resolution_policy: ContentId,
    liability_basis: ContentId,
    rational_representation: ContentId,
    capability_manifest: ContentId,
    funding_list: ContentId,
    market: AccountKeyV3,
    funds: FoundingFundsV3,
}

impl OccurrenceV3 {
    /// Decode one exact Lean-owned 288-byte occurrence record.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV3Error> {
        exact_header(
            bytes,
            generated::SERIES_OCCURRENCE_BYTES_V3,
            &generated::SERIES_OCCURRENCE_MAGIC_V3,
        )?;
        require_zero(
            bytes,
            generated::SERIES_OCCURRENCE_RESERVED_OFFSET_V3,
            generated::SERIES_OCCURRENCE_RESERVED_BYTES_V3,
        )?;
        let funds = FoundingFundsV3 {
            hoard_principal: read_u64(
                bytes,
                generated::SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3,
            )?,
            market_rent: read_u64(bytes, generated::SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V3)?,
            capability_native: read_u64(
                bytes,
                generated::SERIES_OCCURRENCE_CAPABILITY_NATIVE_OFFSET_V3,
            )?,
            founding_work: read_u64(bytes, generated::SERIES_OCCURRENCE_FOUNDING_WORK_OFFSET_V3)?,
        };
        if funds.hoard_principal == 0 {
            return Err(SeriesV3Error::Funding);
        }
        funds.checked_native_total()?;
        Ok(Self {
            occurrence: read_u32(bytes, generated::SERIES_OCCURRENCE_INDEX_OFFSET_V3)?,
            scheduled_slot: read_u64(bytes, generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3)?,
            product_record: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
            )?,
            resolution_policy: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V3,
            )?,
            liability_basis: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3,
            )?,
            rational_representation: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V3,
            )?,
            capability_manifest: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_CAPABILITY_MANIFEST_OFFSET_V3,
            )?,
            funding_list: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3,
            )?,
            market: read_account_key(bytes, generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3)?,
            funds,
        })
    }

    /// Return the scheduled occurrence index.
    pub const fn occurrence(self) -> u32 {
        self.occurrence
    }

    /// Return the exact scheduled slot stored in the content record.
    pub const fn scheduled_slot(self) -> u64 {
        self.scheduled_slot
    }

    /// Return the exact finalized Product-record foreign key.
    ///
    /// Stable Product and result-domain identities are derived from an
    /// independently authenticated Product Runtime V2 graph. They are never
    /// duplicated in the Series occurrence record.
    pub const fn product_record(self) -> ContentId {
        self.product_record
    }

    /// Return this occurrence's resolution-policy identity.
    pub const fn resolution_policy(self) -> ContentId {
        self.resolution_policy
    }

    /// Return this occurrence's LiabilityBasisV2 identity.
    pub const fn liability_basis(self) -> ContentId {
        self.liability_basis
    }

    /// Return this occurrence's RationalRepresentationV2 identity.
    pub const fn rational_representation(self) -> ContentId {
        self.rational_representation
    }

    /// Return this occurrence's exact capability-manifest identity.
    pub const fn capability_manifest(self) -> ContentId {
        self.capability_manifest
    }

    /// Return the ordered FundingState-list identity.
    pub const fn funding_list(self) -> ContentId {
        self.funding_list
    }

    /// Return the canonical future Core Market address committed by this record.
    pub const fn market(self) -> AccountKeyV3 {
        self.market
    }

    /// Return four exact disjoint founding compartments.
    pub const fn funds(self) -> FoundingFundsV3 {
        self.funds
    }
}

/// Hostile-decoded immutable occurrence Ticket commitment V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketV3 {
    template: ContentId,
    occurrence_id: ContentId,
    market: AccountKeyV3,
    funding_list: ContentId,
    founder: AccountKeyV3,
    refund_owner: AccountKeyV3,
    occurrence: u32,
    funds: FoundingFundsV3,
}

impl TicketV3 {
    /// Decode one exact Lean-owned 256-byte Ticket commitment.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV3Error> {
        exact_header(
            bytes,
            generated::SERIES_TICKET_BYTES_V3,
            &generated::SERIES_TICKET_MAGIC_V3,
        )?;
        require_zero(
            bytes,
            generated::SERIES_TICKET_RESERVED_OFFSET_V3,
            generated::SERIES_TICKET_RESERVED_BYTES_V3,
        )?;
        let funds = FoundingFundsV3 {
            hoard_principal: read_u64(bytes, generated::SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3)?,
            market_rent: read_u64(bytes, generated::SERIES_TICKET_MARKET_RENT_OFFSET_V3)?,
            capability_native: read_u64(
                bytes,
                generated::SERIES_TICKET_CAPABILITY_NATIVE_OFFSET_V3,
            )?,
            founding_work: read_u64(bytes, generated::SERIES_TICKET_FOUNDING_WORK_OFFSET_V3)?,
        };
        if funds.hoard_principal == 0 {
            return Err(SeriesV3Error::Funding);
        }
        funds.checked_native_total()?;
        Ok(Self {
            occurrence: read_u32(bytes, generated::SERIES_TICKET_INDEX_OFFSET_V3)?,
            template: read_content_id(bytes, generated::SERIES_TICKET_TEMPLATE_OFFSET_V3)?,
            occurrence_id: read_content_id(
                bytes,
                generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            )?,
            market: read_account_key(bytes, generated::SERIES_TICKET_MARKET_OFFSET_V3)?,
            funding_list: read_content_id(bytes, generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V3)?,
            founder: read_account_key(bytes, generated::SERIES_TICKET_FOUNDER_OFFSET_V3)?,
            refund_owner: read_account_key(bytes, generated::SERIES_TICKET_REFUND_OWNER_OFFSET_V3)?,
            funds,
        })
    }

    /// Return the exact Template identity.
    pub const fn template(self) -> ContentId {
        self.template
    }

    /// Return the exact content identity of the realized occurrence.
    pub const fn occurrence_id(self) -> ContentId {
        self.occurrence_id
    }

    /// Return the exact committed future Core Market address.
    pub const fn market(self) -> AccountKeyV3 {
        self.market
    }

    /// Return the exact ordered FundingState-list identity.
    pub const fn funding_list(self) -> ContentId {
        self.funding_list
    }

    /// Return the immutable founding beneficiary.
    pub const fn founder(self) -> AccountKeyV3 {
        self.founder
    }

    /// Return the immutable expiry/rent refund owner.
    pub const fn refund_owner(self) -> AccountKeyV3 {
        self.refund_owner
    }

    /// Return the scheduled occurrence index.
    pub const fn occurrence(self) -> u32 {
        self.occurrence
    }

    /// Return exact ticket-owned founding compartments.
    pub const fn funds(self) -> FoundingFundsV3 {
        self.funds
    }
}

/// One hostile-decoded Ticket paired with its exact content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedTicketV3 {
    ticket: TicketV3,
    content_id: ContentId,
}

impl AdmittedTicketV3 {
    /// Return the exact immutable Ticket record.
    pub const fn ticket(self) -> TicketV3 {
        self.ticket
    }

    /// Return its domain-separated content identity.
    pub const fn content_id(self) -> ContentId {
        self.content_id
    }
}

/// Fully admitted scheduled occurrence and its exact content identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedOccurrenceV3 {
    template: TemplateV3,
    template_id: ContentId,
    occurrence: OccurrenceV3,
    occurrence_id: ContentId,
}

impl AdmittedOccurrenceV3 {
    /// Return the admitted immutable Template.
    pub const fn template(self) -> TemplateV3 {
        self.template
    }

    /// Return the Template's exact content identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }

    /// Return the admitted exact realized occurrence.
    pub const fn occurrence(self) -> OccurrenceV3 {
        self.occurrence
    }

    /// Return the occurrence's exact content identity.
    pub const fn occurrence_id(self) -> ContentId {
        self.occurrence_id
    }

    /// Require a Ticket to bind this exact realized occurrence and funding.
    pub fn require_ticket(self, ticket: TicketV3) -> Result<(), SeriesV3Error> {
        if ticket.template != self.template_id
            || ticket.occurrence_id != self.occurrence_id
            || ticket.market != self.occurrence.market
            || ticket.funding_list != self.occurrence.funding_list
            || ticket.occurrence != self.occurrence.occurrence
            || ticket.funds != self.occurrence.funds
        {
            return Err(SeriesV3Error::Commitment);
        }
        Ok(())
    }
}

/// SDK-free projection of the exact future Market identity and PDA seeds.
///
/// The adapter derives the actual address under the current Registry-selected
/// Core program and calls [`FutureMarketProjectionV3::require_address`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FutureMarketProjectionV3 {
    committed_address: AccountKeyV3,
    identity: MarketIdentity,
    seeds: MarketCoreStateSeedsV2,
}

/// Product Runtime V2 facts admitted by its canonical graph reader.
///
/// This value is an adapter input, not a second Product authority. The Series
/// kernel only joins the authenticated record identity to the occurrence and
/// uses the graph-derived stable identity to project Core Market coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductProjectionV2 {
    product_record: ContentId,
    stable_product_id: ContentId,
    result_domain: ContentId,
}

impl AuthenticatedProductProjectionV2 {
    /// Construct the exact output of the canonical Product Runtime V2 reader.
    pub const fn new(
        product_record: ContentId,
        stable_product_id: ContentId,
        result_domain: ContentId,
    ) -> Self {
        Self {
            product_record,
            stable_product_id,
            result_domain,
        }
    }

    /// Return the authenticated finalized Product-record digest.
    pub const fn product_record(self) -> ContentId {
        self.product_record
    }

    /// Return the stable semantic Product identity derived from the graph.
    pub const fn stable_product_id(self) -> ContentId {
        self.stable_product_id
    }

    /// Return the exhaustive result-domain identity derived from the graph.
    pub const fn result_domain(self) -> ContentId {
        self.result_domain
    }
}

/// Exact semantic inputs for pre-founding Custody `SeriesEscrow` creation.
///
/// This projection contains no token-account, Rent, replay, Registry, or CPI
/// observation. The Custody adapter must authenticate those physical facts,
/// but it may not reinterpret the immutable identities or collateral amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefoundingSeriesEscrowV3 {
    template_id: ContentId,
    occurrence_id: ContentId,
    ticket_id: ContentId,
    release_set: ContentId,
    realm: ContentId,
    market: AccountKeyV3,
    founder: AccountKeyV3,
    refund_owner: AccountKeyV3,
    occurrence: u32,
    hoard_principal: u64,
    future_market: FutureMarketProjectionV3,
}

impl PrefoundingSeriesEscrowV3 {
    /// Finalized immutable Template content identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }

    /// Finalized immutable occurrence content identity.
    pub const fn occurrence_id(self) -> ContentId {
        self.occurrence_id
    }

    /// Finalized immutable Ticket content identity and Custody replay context.
    pub const fn ticket_id(self) -> ContentId {
        self.ticket_id
    }

    /// ReleaseSet selected by the recurring Template.
    pub const fn release_set(self) -> ContentId {
        self.release_set
    }

    /// Realm selecting the sole collateral asset.
    pub const fn realm(self) -> ContentId {
        self.realm
    }

    /// Exact future Market coordinate committed by occurrence and Ticket.
    pub const fn market(self) -> AccountKeyV3 {
        self.market
    }

    /// External collateral owner funding the pre-founding escrow.
    pub const fn founder(self) -> AccountKeyV3 {
        self.founder
    }

    /// Immutable expiry and Rent-refund beneficiary.
    pub const fn refund_owner(self) -> AccountKeyV3 {
        self.refund_owner
    }

    /// Scheduled occurrence coordinate and Custody order nonce.
    pub const fn occurrence(self) -> u32 {
        self.occurrence
    }

    /// Future Market generation, canonically `occurrence + 1`.
    pub fn generation(self) -> u64 {
        u64::from(self.occurrence) + 1
    }

    /// Exact Realm-collateral amount locked in `SeriesEscrow`.
    pub const fn hoard_principal(self) -> u64 {
        self.hoard_principal
    }

    /// Full future-Market identity and seed projection.
    pub const fn future_market(self) -> FutureMarketProjectionV3 {
        self.future_market
    }
}

impl FutureMarketProjectionV3 {
    /// Return the future Market address committed by Occurrence V3.
    pub const fn committed_address(self) -> AccountKeyV3 {
        self.committed_address
    }

    /// Return every immutable Core Market coordinate.
    pub const fn identity(self) -> MarketIdentity {
        self.identity
    }

    /// Return the sole ordered Core PDA seed projection.
    pub const fn seeds(self) -> MarketCoreStateSeedsV2 {
        self.seeds
    }

    /// Require an adapter-derived address to equal the committed address.
    pub fn require_address(self, derived: AccountKeyV3) -> Result<(), SeriesV3Error> {
        if derived != self.committed_address {
            return Err(SeriesV3Error::Market);
        }
        Ok(())
    }
}

/// Decode and authenticate one exact scheduled occurrence projection.
///
/// `siblings` has the unique `ceil(log2(occurrence_count))` length. The leaf
/// is the domain-separated occurrence ID. Internal nodes bind the Lean-owned
/// node domain, one zero separator, and ordered children.
pub fn admit_occurrence(
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    siblings: &[[u8; 32]],
) -> Result<AdmittedOccurrenceV3, SeriesV3Error> {
    let template = TemplateV3::decode(template_bytes)?;
    let occurrence = OccurrenceV3::decode(occurrence_bytes)?;
    let template_id = template_content_id(template_bytes)?;
    let occurrence_id = occurrence_content_id(occurrence_bytes)?;
    if occurrence.occurrence >= template.occurrence_count
        || occurrence.scheduled_slot != template.scheduled_slot(occurrence.occurrence)?
        || siblings.len() != proof_height(template.occurrence_count)
        || siblings.len() > generated::SERIES_MAXIMUM_MERKLE_HEIGHT_V3
    {
        return Err(SeriesV3Error::Commitment);
    }
    let mut node = occurrence_id.to_bytes();
    let mut index = occurrence.occurrence;
    for sibling in siblings {
        node = if index & 1 == 0 {
            projection_node_hash(&node, sibling)
        } else {
            projection_node_hash(sibling, &node)
        };
        index >>= 1;
    }
    if node != template.projection_root.to_bytes() {
        return Err(SeriesV3Error::Commitment);
    }
    Ok(AdmittedOccurrenceV3 {
        template,
        template_id,
        occurrence,
        occurrence_id,
    })
}

/// Decode and authenticate an occurrence from exact borrowed proof bytes.
///
/// This is the SBF-oriented equivalent of [`admit_occurrence`]. It refuses a
/// partial sibling and iterates the canonical byte string one 32-byte node at
/// a time, avoiding a maximum-width proof copy on the adapter stack.
pub fn admit_occurrence_bytes(
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    proof_bytes: &[u8],
) -> Result<AdmittedOccurrenceV3, SeriesV3Error> {
    if !proof_bytes.len().is_multiple_of(32) {
        return Err(SeriesV3Error::Commitment);
    }
    let template = TemplateV3::decode(template_bytes)?;
    let occurrence = OccurrenceV3::decode(occurrence_bytes)?;
    let template_id = template_content_id(template_bytes)?;
    let occurrence_id = occurrence_content_id(occurrence_bytes)?;
    let proof_count = proof_bytes.len() / 32;
    if occurrence.occurrence >= template.occurrence_count
        || occurrence.scheduled_slot != template.scheduled_slot(occurrence.occurrence)?
        || proof_count != proof_height(template.occurrence_count)
        || proof_count > generated::SERIES_MAXIMUM_MERKLE_HEIGHT_V3
    {
        return Err(SeriesV3Error::Commitment);
    }
    let mut node = occurrence_id.to_bytes();
    let mut index = occurrence.occurrence;
    for sibling_bytes in proof_bytes.chunks_exact(32) {
        let sibling: [u8; 32] = sibling_bytes
            .try_into()
            .map_err(|_| SeriesV3Error::Commitment)?;
        node = if index & 1 == 0 {
            projection_node_hash(&node, &sibling)
        } else {
            projection_node_hash(&sibling, &node)
        };
        index >>= 1;
    }
    if node != template.projection_root.to_bytes() {
        return Err(SeriesV3Error::Commitment);
    }
    Ok(AdmittedOccurrenceV3 {
        template,
        template_id,
        occurrence,
        occurrence_id,
    })
}

/// Project every immutable future-Market coordinate without deriving a PDA.
pub fn future_market_projection(
    admitted: AdmittedOccurrenceV3,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
) -> Result<FutureMarketProjectionV3, SeriesV3Error> {
    let occurrence = admitted.occurrence;
    if occurrence.product_record != product.product_record {
        return Err(SeriesV3Error::Commitment);
    }
    let identity = MarketIdentity {
        market_id: core_account_identity(occurrence.market)?,
        realm_id: core_content_identity(admitted.template.realm)?,
        product_record: core_content_identity(product.product_record)?,
        product_id: core_content_identity(product.stable_product_id)?,
        resolution_policy: core_content_identity(occurrence.resolution_policy)?,
        capability_manifest: core_content_identity(occurrence.capability_manifest)?,
        selected_release_set: core_content_identity(admitted.template.release_set)?,
        registry_program: core_account_identity(registry_program)?,
        generation: u64::from(occurrence.occurrence) + 1,
    };
    Ok(FutureMarketProjectionV3 {
        committed_address: occurrence.market,
        identity,
        seeds: MarketCoreStateSeedsV2::new(identity),
    })
}

/// Join exact Template, occurrence, and Ticket records for pre-founding escrow.
///
/// The returned Ticket identity is the one replay context shared by Prepare
/// and the mutually exclusive Consume/Expire terminal edge. A Custody adapter
/// must use the exact hoard principal and may not substitute a private vault.
pub fn pre_founding_series_escrow(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
) -> Result<PrefoundingSeriesEscrowV3, SeriesV3Error> {
    let ticket = admitted_ticket.ticket;
    admitted.require_ticket(ticket)?;
    let template = admitted.template;
    let occurrence = admitted.occurrence;
    Ok(PrefoundingSeriesEscrowV3 {
        template_id: admitted.template_id,
        occurrence_id: admitted.occurrence_id,
        ticket_id: admitted_ticket.content_id,
        release_set: template.release_set,
        realm: template.realm,
        market: occurrence.market,
        founder: ticket.founder,
        refund_owner: ticket.refund_owner,
        occurrence: occurrence.occurrence,
        hoard_principal: occurrence.funds.hoard_principal,
        future_market: future_market_projection(admitted, product, registry_program)?,
    })
}

/// Project one admitted Ticket consumption into the canonical Core request.
///
/// This function is SDK-free and stateless. It binds the Product record root,
/// every immutable occurrence coordinate, all four disjoint founding
/// compartments, and both optimistic replay revisions. Core must still
/// authenticate Product Runtime V2, Found inputs, the current Trading caller,
/// and every child effect before returning its acknowledgement.
#[allow(clippy::too_many_arguments)]
pub fn series_core_consume_request(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    ticket_state_account: AccountKeyV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
) -> Result<SeriesCoreRequestV1, SeriesV3Error> {
    let ticket = admitted_ticket.ticket;
    admitted.require_ticket(ticket)?;
    let template = admitted.template;
    let occurrence = admitted.occurrence;
    if occurrence.product_record != product.product_record {
        return Err(SeriesV3Error::Commitment);
    }
    let funds = occurrence.funds;
    SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Consume,
        core_content_identity(template.release_set)?,
        core_content_identity(admitted.template_id)?,
        core_account_identity(ticket_state_account)?,
        core_account_identity(occurrence.market)?,
        core_content_identity(template.realm)?,
        core_content_identity(product.product_record)?,
        core_account_identity(ticket.refund_owner)?,
        core_account_identity(ticket.founder)?,
        occurrence.occurrence,
        expected_series_revision,
        expected_ticket_revision,
        funds.market_rent,
        funds.capability_native,
        funds.founding_work,
        funds.hoard_principal,
    )
    .map_err(|_| SeriesV3Error::Commitment)
}

/// Validate one exact Core permit-expiry request against Series-owned facts.
///
/// The 640-byte physical request contains a candidate permit because the
/// canonical PDA is still System-owned and has no data before Consume. This
/// SDK-free join proves the immutable release, Market, Product record,
/// founder, Ticket context, generation, retry deadline, and total Hoard
/// principal. Adapter-owned program, vault, replay, and RentCredit identities
/// remain independently authenticated physical observations.
pub fn validate_series_permit_expiry_request_v3(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    request: SeriesPermitExpiryRequestV1,
) -> Result<(), SeriesV3Error> {
    let ticket = admitted_ticket.ticket;
    admitted.require_ticket(ticket)?;
    let template = admitted.template;
    let occurrence = admitted.occurrence;
    if occurrence.product_record != product.product_record {
        return Err(SeriesV3Error::Commitment);
    }
    let intent = request.permit().intent();
    let expected_expiry = template.retry_through(occurrence.occurrence)?;
    let expected_generation = u64::from(occurrence.occurrence)
        .checked_add(1)
        .ok_or(SeriesV3Error::Schedule)?;
    let principal = intent
        .quantity()
        .checked_mul(intent.basis_scale())
        .ok_or(SeriesV3Error::Funding)?;
    if intent.release_set() != core_content_identity(template.release_set)?
        || intent.market() != core_account_identity(occurrence.market)?
        || intent.product_record() != core_content_identity(product.product_record)?
        || intent.founder() != core_account_identity(ticket.founder)?
        || intent.ticket_context() != core_content_identity(admitted_ticket.content_id)?
        || intent.generation() != expected_generation
        || intent.expiry_slot() != expected_expiry
        || principal != occurrence.funds.hoard_principal
    {
        return Err(SeriesV3Error::Commitment);
    }
    Ok(())
}

/// Hash an exact ordered, nonempty, alias-free FundingState key list.
///
/// The preimage is `u16_le(count) || key[0] || ... || key[count-1]` under the
/// Lean-owned funding-list domain. This SDK-free kernel imposes no old
/// width-specific account-profile bound; the exact list must only fit `u16`.
pub fn funding_list_id(funding_states: &[AccountKeyV3]) -> Result<ContentId, SeriesV3Error> {
    if funding_states.is_empty() {
        return Err(SeriesV3Error::Funding);
    }
    let count = u16::try_from(funding_states.len()).map_err(|_| SeriesV3Error::Funding)?;
    let mut hasher = content_hasher(&generated::SERIES_FUNDING_LIST_DOMAIN_V3);
    hasher.update(count.to_le_bytes());
    for (index, key) in funding_states.iter().enumerate() {
        if funding_states
            .get(..index)
            .ok_or(SeriesV3Error::Funding)?
            .iter()
            .any(|prior| prior == key)
        {
            return Err(SeriesV3Error::Funding);
        }
        hasher.update(key.as_bytes());
    }
    content_id_from_hasher(hasher)
}

/// Require actual ordered FundingState keys to match an occurrence commitment.
pub fn require_funding_list(
    occurrence: OccurrenceV3,
    funding_states: &[AccountKeyV3],
) -> Result<(), SeriesV3Error> {
    if funding_list_id(funding_states)? != occurrence.funding_list {
        return Err(SeriesV3Error::Funding);
    }
    Ok(())
}

/// Compute the exact domain-separated Template V3 content identity.
pub fn template_content_id(template_bytes: &[u8]) -> Result<ContentId, SeriesV3Error> {
    TemplateV3::decode(template_bytes)?;
    content_id(
        &generated::SERIES_TEMPLATE_CONTENT_DOMAIN_V3,
        template_bytes,
    )
}

/// Compute the exact domain-separated occurrence V3 content identity.
pub fn occurrence_content_id(occurrence_bytes: &[u8]) -> Result<ContentId, SeriesV3Error> {
    OccurrenceV3::decode(occurrence_bytes)?;
    content_id(
        &generated::SERIES_OCCURRENCE_CONTENT_DOMAIN_V3,
        occurrence_bytes,
    )
}

/// Compute the exact domain-separated Ticket V3 content identity.
pub fn ticket_content_id(ticket_bytes: &[u8]) -> Result<ContentId, SeriesV3Error> {
    TicketV3::decode(ticket_bytes)?;
    content_id(&generated::SERIES_TICKET_CONTENT_DOMAIN_V3, ticket_bytes)
}

/// Hostile-decode one Ticket and bind the exact bytes to their identity.
pub fn admit_ticket(ticket_bytes: &[u8]) -> Result<AdmittedTicketV3, SeriesV3Error> {
    Ok(AdmittedTicketV3 {
        ticket: TicketV3::decode(ticket_bytes)?,
        content_id: ticket_content_id(ticket_bytes)?,
    })
}

fn proof_height(count: u32) -> usize {
    if count <= 1 {
        0
    } else {
        usize::try_from(u32::BITS - (count - 1).leading_zeros()).unwrap_or(0)
    }
}

fn projection_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(generated::SERIES_PROJECTION_NODE_DOMAIN_V3);
    hasher.update(HASH_SEPARATOR);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn content_id(domain: &[u8], bytes: &[u8]) -> Result<ContentId, SeriesV3Error> {
    let mut hasher = content_hasher(domain);
    hasher.update(bytes);
    content_id_from_hasher(hasher)
}

fn content_hasher(domain: &[u8]) -> Sha256 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(HASH_SEPARATOR);
    hasher
}

fn content_id_from_hasher(hasher: Sha256) -> Result<ContentId, SeriesV3Error> {
    ContentId::new(hasher.finalize().into()).map_err(|_| SeriesV3Error::Identity)
}

fn core_content_identity(value: ContentId) -> Result<CoreIdentity, SeriesV3Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV3Error::Identity)
}

fn core_account_identity(value: AccountKeyV3) -> Result<CoreIdentity, SeriesV3Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV3Error::Identity)
}

fn exact_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), SeriesV3Error> {
    if bytes.len() != width {
        return Err(SeriesV3Error::Length);
    }
    if bytes.get(..magic.len()) != Some(magic.as_slice())
        || read_u16(bytes, HEADER_VERSION_OFFSET)? != generated::SERIES_TEMPLATE_SCHEMA_V3
        || read_u16(bytes, HEADER_PROFILE_OFFSET)? != generated::SERIES_TEMPLATE_PROFILE_V3
    {
        return Err(SeriesV3Error::Header);
    }
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), SeriesV3Error> {
    if !bytes
        .get(offset..offset.checked_add(width).ok_or(SeriesV3Error::Length)?)
        .ok_or(SeriesV3Error::Length)?
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(SeriesV3Error::Header);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SeriesV3Error> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SeriesV3Error> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SeriesV3Error> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_content_id(bytes: &[u8], offset: usize) -> Result<ContentId, SeriesV3Error> {
    ContentId::new(read_array(bytes, offset)?).map_err(|_| SeriesV3Error::Identity)
}

fn read_account_key(bytes: &[u8], offset: usize) -> Result<AccountKeyV3, SeriesV3Error> {
    AccountKeyV3::new(read_array(bytes, offset)?)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], SeriesV3Error> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(SeriesV3Error::Length)?)
        .ok_or(SeriesV3Error::Length)?
        .try_into()
        .map_err(|_| SeriesV3Error::Length)
}

#[cfg(test)]
mod tests;
