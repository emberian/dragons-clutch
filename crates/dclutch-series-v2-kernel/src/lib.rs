#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free recurring-Series V2 record admission.
//!
//! This kernel is the one semantic owner of immutable Template, occurrence,
//! and Ticket bytes.  It hostile-decodes the Lean-owned fixed layouts, checks
//! exact scheduling and occurrence proofs, binds prepaid funding, and projects
//! the complete future-Market identity.  It performs no account access, PDA
//! derivation, CPI, token movement, or mutation; those are adapter boundaries.

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{Identity as CoreIdentity, MarketCoreStateSeedsV1, MarketIdentity};
use sha2::{Digest, Sha256};

/// Lean-generated Series V2 widths, offsets, domains, and hostile examples.
#[allow(dead_code, missing_docs)]
#[doc(hidden)]
pub mod generated;

pub use generated::{
    SERIES_MAXIMUM_MERKLE_HEIGHT_V2, SERIES_OCCURRENCE_BYTES_V2, SERIES_TEMPLATE_BYTES_V2,
    SERIES_TICKET_BYTES_V2,
};

const HEADER_VERSION_OFFSET: usize = 8;
const HEADER_PROFILE_OFFSET: usize = 10;
const HASH_SEPARATOR: [u8; 1] = [0];

/// Refusal from exact Series V2 content admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesV2Error {
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
pub struct AccountKeyV2([u8; 32]);

impl AccountKeyV2 {
    /// Validate and construct one nonzero account key.
    pub fn new(bytes: [u8; 32]) -> Result<Self, SeriesV2Error> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(SeriesV2Error::Identity);
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
pub struct FoundingFundsV2 {
    hoard_principal: u64,
    market_rent: u64,
    capability_native: u64,
    founding_work: u64,
}

impl FoundingFundsV2 {
    /// Return the checked native-lamport total.
    ///
    /// Hoard principal is denominated in Realm collateral and is deliberately
    /// excluded. It is never added to lamports.
    pub fn checked_native_total(self) -> Result<u64, SeriesV2Error> {
        self.market_rent
            .checked_add(self.capability_native)
            .and_then(|total| total.checked_add(self.founding_work))
            .ok_or(SeriesV2Error::Funding)
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

/// Hostile-decoded immutable recurring Template V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateV2 {
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
    refund_owner: AccountKeyV2,
    occurrence_count: u32,
    first_slot: u64,
    period_slots: u64,
    retry_window: u64,
    close_rent: u64,
}

impl TemplateV2 {
    /// Decode one exact Lean-owned 400-byte Template record.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV2Error> {
        exact_header(
            bytes,
            generated::SERIES_TEMPLATE_BYTES_V2,
            &generated::SERIES_TEMPLATE_MAGIC_V2,
        )?;
        let value = Self {
            occurrence_count: read_u32(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V2,
            )?,
            first_slot: read_u64(bytes, generated::SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V2)?,
            period_slots: read_u64(bytes, generated::SERIES_TEMPLATE_PERIOD_SLOTS_OFFSET_V2)?,
            retry_window: read_u64(bytes, generated::SERIES_TEMPLATE_RETRY_WINDOW_OFFSET_V2)?,
            close_rent: read_u64(bytes, generated::SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V2)?,
            realm: read_content_id(bytes, generated::SERIES_TEMPLATE_REALM_OFFSET_V2)?,
            release_set: read_content_id(bytes, generated::SERIES_TEMPLATE_RELEASE_SET_OFFSET_V2)?,
            product_generator: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PRODUCT_GENERATOR_OFFSET_V2,
            )?,
            occurrence_generator: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_GENERATOR_OFFSET_V2,
            )?,
            capability_template: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_CAPABILITY_TEMPLATE_OFFSET_V2,
            )?,
            product_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PRODUCT_DERIVATION_OFFSET_V2,
            )?,
            occurrence_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_OCCURRENCE_DERIVATION_OFFSET_V2,
            )?,
            capability_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_CAPABILITY_DERIVATION_OFFSET_V2,
            )?,
            funding_derivation: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_FUNDING_DERIVATION_OFFSET_V2,
            )?,
            projection_root: read_content_id(
                bytes,
                generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V2,
            )?,
            refund_owner: read_account_key(
                bytes,
                generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V2,
            )?,
        };
        if value.occurrence_count == 0 || value.period_slots == 0 {
            return Err(SeriesV2Error::Schedule);
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
    pub const fn refund_owner(self) -> AccountKeyV2 {
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
    pub fn scheduled_slot(self, occurrence: u32) -> Result<u64, SeriesV2Error> {
        if occurrence >= self.occurrence_count {
            return Err(SeriesV2Error::Schedule);
        }
        self.first_slot
            .checked_add(
                u64::from(occurrence)
                    .checked_mul(self.period_slots)
                    .ok_or(SeriesV2Error::Schedule)?,
            )
            .ok_or(SeriesV2Error::Schedule)
    }

    /// Derive the inclusive last retry slot with checked arithmetic.
    pub fn retry_through(self, occurrence: u32) -> Result<u64, SeriesV2Error> {
        self.scheduled_slot(occurrence)?
            .checked_add(self.retry_window)
            .ok_or(SeriesV2Error::Schedule)
    }
}

/// Hostile-decoded exact realized occurrence V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceV2 {
    occurrence: u32,
    scheduled_slot: u64,
    product: ContentId,
    result_domain: ContentId,
    resolution_policy: ContentId,
    liability_basis: ContentId,
    rational_representation: ContentId,
    capability_manifest: ContentId,
    funding_list: ContentId,
    market: AccountKeyV2,
    funds: FoundingFundsV2,
}

impl OccurrenceV2 {
    /// Decode one exact Lean-owned 320-byte occurrence record.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV2Error> {
        exact_header(
            bytes,
            generated::SERIES_OCCURRENCE_BYTES_V2,
            &generated::SERIES_OCCURRENCE_MAGIC_V2,
        )?;
        require_zero(
            bytes,
            generated::SERIES_OCCURRENCE_RESERVED_OFFSET_V2,
            generated::SERIES_OCCURRENCE_RESERVED_BYTES_V2,
        )?;
        let funds = FoundingFundsV2 {
            hoard_principal: read_u64(
                bytes,
                generated::SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V2,
            )?,
            market_rent: read_u64(bytes, generated::SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V2)?,
            capability_native: read_u64(
                bytes,
                generated::SERIES_OCCURRENCE_CAPABILITY_NATIVE_OFFSET_V2,
            )?,
            founding_work: read_u64(bytes, generated::SERIES_OCCURRENCE_FOUNDING_WORK_OFFSET_V2)?,
        };
        if funds.hoard_principal == 0 {
            return Err(SeriesV2Error::Funding);
        }
        funds.checked_native_total()?;
        Ok(Self {
            occurrence: read_u32(bytes, generated::SERIES_OCCURRENCE_INDEX_OFFSET_V2)?,
            scheduled_slot: read_u64(bytes, generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V2)?,
            product: read_content_id(bytes, generated::SERIES_OCCURRENCE_PRODUCT_OFFSET_V2)?,
            result_domain: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_RESULT_DOMAIN_OFFSET_V2,
            )?,
            resolution_policy: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V2,
            )?,
            liability_basis: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V2,
            )?,
            rational_representation: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V2,
            )?,
            capability_manifest: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_CAPABILITY_MANIFEST_OFFSET_V2,
            )?,
            funding_list: read_content_id(
                bytes,
                generated::SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V2,
            )?,
            market: read_account_key(bytes, generated::SERIES_OCCURRENCE_MARKET_OFFSET_V2)?,
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

    /// Return this occurrence's realized Product identity.
    pub const fn product(self) -> ContentId {
        self.product
    }

    /// Return this occurrence's realized exhaustive result-domain identity.
    pub const fn result_domain(self) -> ContentId {
        self.result_domain
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
    pub const fn market(self) -> AccountKeyV2 {
        self.market
    }

    /// Return four exact disjoint founding compartments.
    pub const fn funds(self) -> FoundingFundsV2 {
        self.funds
    }
}

/// Hostile-decoded immutable occurrence Ticket commitment V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketV2 {
    template: ContentId,
    occurrence_id: ContentId,
    market: AccountKeyV2,
    funding_list: ContentId,
    founder: AccountKeyV2,
    refund_owner: AccountKeyV2,
    occurrence: u32,
    funds: FoundingFundsV2,
}

impl TicketV2 {
    /// Decode one exact Lean-owned 256-byte Ticket commitment.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesV2Error> {
        exact_header(
            bytes,
            generated::SERIES_TICKET_BYTES_V2,
            &generated::SERIES_TICKET_MAGIC_V2,
        )?;
        require_zero(
            bytes,
            generated::SERIES_TICKET_RESERVED_OFFSET_V2,
            generated::SERIES_TICKET_RESERVED_BYTES_V2,
        )?;
        let funds = FoundingFundsV2 {
            hoard_principal: read_u64(bytes, generated::SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V2)?,
            market_rent: read_u64(bytes, generated::SERIES_TICKET_MARKET_RENT_OFFSET_V2)?,
            capability_native: read_u64(
                bytes,
                generated::SERIES_TICKET_CAPABILITY_NATIVE_OFFSET_V2,
            )?,
            founding_work: read_u64(bytes, generated::SERIES_TICKET_FOUNDING_WORK_OFFSET_V2)?,
        };
        if funds.hoard_principal == 0 {
            return Err(SeriesV2Error::Funding);
        }
        funds.checked_native_total()?;
        Ok(Self {
            occurrence: read_u32(bytes, generated::SERIES_TICKET_INDEX_OFFSET_V2)?,
            template: read_content_id(bytes, generated::SERIES_TICKET_TEMPLATE_OFFSET_V2)?,
            occurrence_id: read_content_id(
                bytes,
                generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V2,
            )?,
            market: read_account_key(bytes, generated::SERIES_TICKET_MARKET_OFFSET_V2)?,
            funding_list: read_content_id(bytes, generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V2)?,
            founder: read_account_key(bytes, generated::SERIES_TICKET_FOUNDER_OFFSET_V2)?,
            refund_owner: read_account_key(bytes, generated::SERIES_TICKET_REFUND_OWNER_OFFSET_V2)?,
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
    pub const fn market(self) -> AccountKeyV2 {
        self.market
    }

    /// Return the exact ordered FundingState-list identity.
    pub const fn funding_list(self) -> ContentId {
        self.funding_list
    }

    /// Return the immutable founding beneficiary.
    pub const fn founder(self) -> AccountKeyV2 {
        self.founder
    }

    /// Return the immutable expiry/rent refund owner.
    pub const fn refund_owner(self) -> AccountKeyV2 {
        self.refund_owner
    }

    /// Return the scheduled occurrence index.
    pub const fn occurrence(self) -> u32 {
        self.occurrence
    }

    /// Return exact ticket-owned founding compartments.
    pub const fn funds(self) -> FoundingFundsV2 {
        self.funds
    }
}

/// One hostile-decoded Ticket paired with its exact content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedTicketV2 {
    ticket: TicketV2,
    content_id: ContentId,
}

impl AdmittedTicketV2 {
    /// Return the exact immutable Ticket record.
    pub const fn ticket(self) -> TicketV2 {
        self.ticket
    }

    /// Return its domain-separated content identity.
    pub const fn content_id(self) -> ContentId {
        self.content_id
    }
}

/// Fully admitted scheduled occurrence and its exact content identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedOccurrenceV2 {
    template: TemplateV2,
    template_id: ContentId,
    occurrence: OccurrenceV2,
    occurrence_id: ContentId,
}

impl AdmittedOccurrenceV2 {
    /// Return the admitted immutable Template.
    pub const fn template(self) -> TemplateV2 {
        self.template
    }

    /// Return the Template's exact content identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }

    /// Return the admitted exact realized occurrence.
    pub const fn occurrence(self) -> OccurrenceV2 {
        self.occurrence
    }

    /// Return the occurrence's exact content identity.
    pub const fn occurrence_id(self) -> ContentId {
        self.occurrence_id
    }

    /// Require a Ticket to bind this exact realized occurrence and funding.
    pub fn require_ticket(self, ticket: TicketV2) -> Result<(), SeriesV2Error> {
        if ticket.template != self.template_id
            || ticket.occurrence_id != self.occurrence_id
            || ticket.market != self.occurrence.market
            || ticket.funding_list != self.occurrence.funding_list
            || ticket.occurrence != self.occurrence.occurrence
            || ticket.funds != self.occurrence.funds
        {
            return Err(SeriesV2Error::Commitment);
        }
        Ok(())
    }
}

/// SDK-free projection of the exact future Market identity and PDA seeds.
///
/// The adapter derives the actual address under the current Registry-selected
/// Core program and calls [`FutureMarketProjectionV2::require_address`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FutureMarketProjectionV2 {
    committed_address: AccountKeyV2,
    identity: MarketIdentity,
    seeds: MarketCoreStateSeedsV1,
}

/// Exact semantic inputs for pre-founding Custody `SeriesEscrow` creation.
///
/// This projection contains no token-account, Rent, replay, Registry, or CPI
/// observation. The Custody adapter must authenticate those physical facts,
/// but it may not reinterpret the immutable identities or collateral amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefoundingSeriesEscrowV2 {
    template_id: ContentId,
    occurrence_id: ContentId,
    ticket_id: ContentId,
    release_set: ContentId,
    realm: ContentId,
    market: AccountKeyV2,
    founder: AccountKeyV2,
    refund_owner: AccountKeyV2,
    occurrence: u32,
    hoard_principal: u64,
    future_market: FutureMarketProjectionV2,
}

impl PrefoundingSeriesEscrowV2 {
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
    pub const fn market(self) -> AccountKeyV2 {
        self.market
    }

    /// External collateral owner funding the pre-founding escrow.
    pub const fn founder(self) -> AccountKeyV2 {
        self.founder
    }

    /// Immutable expiry and Rent-refund beneficiary.
    pub const fn refund_owner(self) -> AccountKeyV2 {
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
    pub const fn future_market(self) -> FutureMarketProjectionV2 {
        self.future_market
    }
}

impl FutureMarketProjectionV2 {
    /// Return the future Market address committed by Occurrence V2.
    pub const fn committed_address(self) -> AccountKeyV2 {
        self.committed_address
    }

    /// Return every immutable Core Market coordinate.
    pub const fn identity(self) -> MarketIdentity {
        self.identity
    }

    /// Return the sole ordered Core PDA seed projection.
    pub const fn seeds(self) -> MarketCoreStateSeedsV1 {
        self.seeds
    }

    /// Require an adapter-derived address to equal the committed address.
    pub fn require_address(self, derived: AccountKeyV2) -> Result<(), SeriesV2Error> {
        if derived != self.committed_address {
            return Err(SeriesV2Error::Market);
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
) -> Result<AdmittedOccurrenceV2, SeriesV2Error> {
    let template = TemplateV2::decode(template_bytes)?;
    let occurrence = OccurrenceV2::decode(occurrence_bytes)?;
    let template_id = template_content_id(template_bytes)?;
    let occurrence_id = occurrence_content_id(occurrence_bytes)?;
    if occurrence.occurrence >= template.occurrence_count
        || occurrence.scheduled_slot != template.scheduled_slot(occurrence.occurrence)?
        || siblings.len() != proof_height(template.occurrence_count)
        || siblings.len() > generated::SERIES_MAXIMUM_MERKLE_HEIGHT_V2
    {
        return Err(SeriesV2Error::Commitment);
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
        return Err(SeriesV2Error::Commitment);
    }
    Ok(AdmittedOccurrenceV2 {
        template,
        template_id,
        occurrence,
        occurrence_id,
    })
}

/// Project every immutable future-Market coordinate without deriving a PDA.
pub fn future_market_projection(
    admitted: AdmittedOccurrenceV2,
    registry_program: AccountKeyV2,
) -> Result<FutureMarketProjectionV2, SeriesV2Error> {
    let occurrence = admitted.occurrence;
    let identity = MarketIdentity {
        market_id: core_account_identity(occurrence.market)?,
        realm_id: core_content_identity(admitted.template.realm)?,
        product_id: core_content_identity(occurrence.product)?,
        result_domain: core_content_identity(occurrence.result_domain)?,
        resolution_policy: core_content_identity(occurrence.resolution_policy)?,
        capability_manifest: core_content_identity(occurrence.capability_manifest)?,
        selected_release_set: core_content_identity(admitted.template.release_set)?,
        registry_program: core_account_identity(registry_program)?,
        generation: u64::from(occurrence.occurrence) + 1,
    };
    Ok(FutureMarketProjectionV2 {
        committed_address: occurrence.market,
        identity,
        seeds: MarketCoreStateSeedsV1::new(identity),
    })
}

/// Join exact Template, occurrence, and Ticket records for pre-founding escrow.
///
/// The returned Ticket identity is the one replay context shared by Prepare
/// and the mutually exclusive Consume/Expire terminal edge. A Custody adapter
/// must use the exact hoard principal and may not substitute a private vault.
pub fn pre_founding_series_escrow(
    admitted: AdmittedOccurrenceV2,
    admitted_ticket: AdmittedTicketV2,
    registry_program: AccountKeyV2,
) -> Result<PrefoundingSeriesEscrowV2, SeriesV2Error> {
    let ticket = admitted_ticket.ticket;
    admitted.require_ticket(ticket)?;
    let template = admitted.template;
    let occurrence = admitted.occurrence;
    Ok(PrefoundingSeriesEscrowV2 {
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
        future_market: future_market_projection(admitted, registry_program)?,
    })
}

/// Hash an exact ordered, nonempty, alias-free FundingState key list.
///
/// The preimage is `u16_le(count) || key[0] || ... || key[count-1]` under the
/// Lean-owned funding-list domain. This SDK-free kernel imposes no old
/// width-specific account-profile bound; the exact list must only fit `u16`.
pub fn funding_list_id(funding_states: &[AccountKeyV2]) -> Result<ContentId, SeriesV2Error> {
    if funding_states.is_empty() {
        return Err(SeriesV2Error::Funding);
    }
    let count = u16::try_from(funding_states.len()).map_err(|_| SeriesV2Error::Funding)?;
    let mut hasher = content_hasher(&generated::SERIES_FUNDING_LIST_DOMAIN_V2);
    hasher.update(count.to_le_bytes());
    for (index, key) in funding_states.iter().enumerate() {
        if funding_states
            .get(..index)
            .ok_or(SeriesV2Error::Funding)?
            .iter()
            .any(|prior| prior == key)
        {
            return Err(SeriesV2Error::Funding);
        }
        hasher.update(key.as_bytes());
    }
    content_id_from_hasher(hasher)
}

/// Require actual ordered FundingState keys to match an occurrence commitment.
pub fn require_funding_list(
    occurrence: OccurrenceV2,
    funding_states: &[AccountKeyV2],
) -> Result<(), SeriesV2Error> {
    if funding_list_id(funding_states)? != occurrence.funding_list {
        return Err(SeriesV2Error::Funding);
    }
    Ok(())
}

/// Compute the exact domain-separated Template V2 content identity.
pub fn template_content_id(template_bytes: &[u8]) -> Result<ContentId, SeriesV2Error> {
    TemplateV2::decode(template_bytes)?;
    content_id(
        &generated::SERIES_TEMPLATE_CONTENT_DOMAIN_V2,
        template_bytes,
    )
}

/// Compute the exact domain-separated occurrence V2 content identity.
pub fn occurrence_content_id(occurrence_bytes: &[u8]) -> Result<ContentId, SeriesV2Error> {
    OccurrenceV2::decode(occurrence_bytes)?;
    content_id(
        &generated::SERIES_OCCURRENCE_CONTENT_DOMAIN_V2,
        occurrence_bytes,
    )
}

/// Compute the exact domain-separated Ticket V2 content identity.
pub fn ticket_content_id(ticket_bytes: &[u8]) -> Result<ContentId, SeriesV2Error> {
    TicketV2::decode(ticket_bytes)?;
    content_id(&generated::SERIES_TICKET_CONTENT_DOMAIN_V2, ticket_bytes)
}

/// Hostile-decode one Ticket and bind the exact bytes to their identity.
pub fn admit_ticket(ticket_bytes: &[u8]) -> Result<AdmittedTicketV2, SeriesV2Error> {
    Ok(AdmittedTicketV2 {
        ticket: TicketV2::decode(ticket_bytes)?,
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
    hasher.update(generated::SERIES_PROJECTION_NODE_DOMAIN_V2);
    hasher.update(HASH_SEPARATOR);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn content_id(domain: &[u8], bytes: &[u8]) -> Result<ContentId, SeriesV2Error> {
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

fn content_id_from_hasher(hasher: Sha256) -> Result<ContentId, SeriesV2Error> {
    ContentId::new(hasher.finalize().into()).map_err(|_| SeriesV2Error::Identity)
}

fn core_content_identity(value: ContentId) -> Result<CoreIdentity, SeriesV2Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV2Error::Identity)
}

fn core_account_identity(value: AccountKeyV2) -> Result<CoreIdentity, SeriesV2Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV2Error::Identity)
}

fn exact_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), SeriesV2Error> {
    if bytes.len() != width {
        return Err(SeriesV2Error::Length);
    }
    if bytes.get(..magic.len()) != Some(magic.as_slice())
        || read_u16(bytes, HEADER_VERSION_OFFSET)? != generated::SERIES_TEMPLATE_SCHEMA_V2
        || read_u16(bytes, HEADER_PROFILE_OFFSET)? != generated::SERIES_TEMPLATE_PROFILE_V2
    {
        return Err(SeriesV2Error::Header);
    }
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), SeriesV2Error> {
    if !bytes
        .get(offset..offset.checked_add(width).ok_or(SeriesV2Error::Length)?)
        .ok_or(SeriesV2Error::Length)?
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(SeriesV2Error::Header);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SeriesV2Error> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SeriesV2Error> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SeriesV2Error> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_content_id(bytes: &[u8], offset: usize) -> Result<ContentId, SeriesV2Error> {
    ContentId::new(read_array(bytes, offset)?).map_err(|_| SeriesV2Error::Identity)
}

fn read_account_key(bytes: &[u8], offset: usize) -> Result<AccountKeyV2, SeriesV2Error> {
    AccountKeyV2::new(read_array(bytes, offset)?)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], SeriesV2Error> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(SeriesV2Error::Length)?)
        .ok_or(SeriesV2Error::Length)?
        .try_into()
        .map_err(|_| SeriesV2Error::Length)
}

#[cfg(test)]
mod tests;
