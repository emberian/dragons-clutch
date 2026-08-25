//! Recurring-Series V2 content records and deterministic occurrence admission.
//!
//! A reusable Template commits generators and derivation policies, but does
//! not hard-code one Product category, outcome width, denominator, liability
//! basis, representation, or capability family. Each realized occurrence is
//! content addressed and admitted only by its scheduled-index Merkle proof.
//! The operator supplies proof bytes; it cannot substitute any realized ID.

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Identity as CoreIdentity, MarketCoreStateSeedsV1, MarketIdentity, SeriesCoreActionV1,
    SeriesCoreRequestV1,
};
use solana_program::{hash::hashv, pubkey::Pubkey};

#[allow(dead_code, missing_docs)]
mod generated;

pub use generated::{
    SERIES_MAXIMUM_MERKLE_HEIGHT_V2, SERIES_OCCURRENCE_BYTES_V2, SERIES_TEMPLATE_BYTES_V2,
    SERIES_TICKET_BYTES_V2,
};

const HEADER_VERSION_OFFSET: usize = 8;
const HEADER_PROFILE_OFFSET: usize = 10;
const MAXIMUM_FUNDING_STATES: usize = 16;
const FUNDING_LIST_PREFIX_BYTES: usize = 2;
const FUNDING_LIST_BUFFER_BYTES: usize = FUNDING_LIST_PREFIX_BYTES + 32 * MAXIMUM_FUNDING_STATES;
const HASH_SEPARATOR: [u8; 1] = [0];

/// Refusal from exact Series V2 content admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesV2Error {
    /// An input had the wrong fixed width.
    Length,
    /// Magic, version, profile, or reserved bytes were noncanonical.
    Header,
    /// A content or actor identity was the reserved all-zero value.
    Identity,
    /// A schedule coordinate overflowed or did not match its Template.
    Schedule,
    /// Exact founding funding was invalid or did not match.
    Funding,
    /// A content digest, occurrence projection, or Ticket commitment differed.
    Commitment,
    /// The Market did not equal the occurrence's canonical Core PDA.
    Market,
    /// A requested Core projection was not a Series occurrence action.
    Action,
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
    /// Return the checked sum without reclassifying any compartment.
    pub fn checked_total(self) -> Result<u64, SeriesV2Error> {
        self.hoard_principal
            .checked_add(self.market_rent)
            .and_then(|total| total.checked_add(self.capability_native))
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
    refund_owner: Pubkey,
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
            refund_owner: read_pubkey(bytes, generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V2)?,
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
    pub const fn refund_owner(self) -> Pubkey {
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

/// Hostile-decoded, exact realized occurrence V2.
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
    market: Pubkey,
    funds: FoundingFundsV2,
}

impl OccurrenceV2 {
    /// Decode one exact Lean-owned 352-byte occurrence record.
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
        funds.checked_total()?;
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
            market: read_pubkey(bytes, generated::SERIES_OCCURRENCE_MARKET_OFFSET_V2)?,
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

    /// Return the canonical Core Market PDA committed by this occurrence.
    pub const fn market(self) -> Pubkey {
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
    market: Pubkey,
    funding_list: ContentId,
    founder: Pubkey,
    refund_owner: Pubkey,
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
        funds.checked_total()?;
        Ok(Self {
            occurrence: read_u32(bytes, generated::SERIES_TICKET_INDEX_OFFSET_V2)?,
            template: read_content_id(bytes, generated::SERIES_TICKET_TEMPLATE_OFFSET_V2)?,
            occurrence_id: read_content_id(
                bytes,
                generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V2,
            )?,
            market: read_pubkey(bytes, generated::SERIES_TICKET_MARKET_OFFSET_V2)?,
            funding_list: read_content_id(bytes, generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V2)?,
            founder: read_pubkey(bytes, generated::SERIES_TICKET_FOUNDER_OFFSET_V2)?,
            refund_owner: read_pubkey(bytes, generated::SERIES_TICKET_REFUND_OWNER_OFFSET_V2)?,
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

    /// Return the exact committed Core Market PDA.
    pub const fn market(self) -> Pubkey {
        self.market
    }

    /// Return the exact ordered FundingState-list identity.
    pub const fn funding_list(self) -> ContentId {
        self.funding_list
    }

    /// Return the immutable founding beneficiary.
    pub const fn founder(self) -> Pubkey {
        self.founder
    }

    /// Return the immutable expiry/rent refund owner.
    pub const fn refund_owner(self) -> Pubkey {
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

    /// Project an exact occurrence action onto the canonical Series-to-Core ABI.
    #[allow(clippy::too_many_arguments)]
    pub fn core_request(
        self,
        action: SeriesCoreActionV1,
        ticket: TicketV2,
        ticket_account: Pubkey,
        expected_series_revision: u64,
        expected_ticket_revision: u64,
    ) -> Result<SeriesCoreRequestV1, SeriesV2Error> {
        if action == SeriesCoreActionV1::Close || ticket_account == Pubkey::default() {
            return Err(SeriesV2Error::Action);
        }
        self.require_ticket(ticket)?;
        let funds = self.occurrence.funds;
        SeriesCoreRequestV1::occurrence(
            action,
            core_identity(self.template.release_set)?,
            core_identity(self.template_id)?,
            core_pubkey_identity(ticket_account)?,
            core_pubkey_identity(self.occurrence.market)?,
            core_identity(self.template.realm)?,
            core_identity(self.occurrence.product)?,
            core_pubkey_identity(ticket.refund_owner)?,
            core_pubkey_identity(ticket.founder)?,
            self.occurrence.occurrence,
            expected_series_revision,
            expected_ticket_revision,
            funds.market_rent,
            funds.capability_native,
            funds.founding_work,
            funds.hoard_principal,
        )
        .map_err(|_| SeriesV2Error::Commitment)
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

/// Decode and authenticate one exact scheduled occurrence projection.
///
/// `siblings` must have the unique `ceil(log2(occurrence_count))` length.
/// The leaf is the occurrence content ID. Each internal node is SHA-256 over
/// the Lean-owned node domain, one zero separator byte, then ordered children.
pub fn admit_occurrence(
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    siblings: &[[u8; 32]],
) -> Result<AdmittedOccurrenceV2, SeriesV2Error> {
    let template = TemplateV2::decode(template_bytes)?;
    let occurrence = OccurrenceV2::decode(occurrence_bytes)?;
    let template_id = content_id(
        &generated::SERIES_TEMPLATE_CONTENT_DOMAIN_V2,
        template_bytes,
    )?;
    let occurrence_id = content_id(
        &generated::SERIES_OCCURRENCE_CONTENT_DOMAIN_V2,
        occurrence_bytes,
    )?;
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
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V2,
                &HASH_SEPARATOR,
                &node,
                sibling,
            ])
            .to_bytes()
        } else {
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V2,
                &HASH_SEPARATOR,
                sibling,
                &node,
            ])
            .to_bytes()
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

/// Require the occurrence's Market to be its full immutable Core PDA.
pub fn require_market_pda(
    admitted: AdmittedOccurrenceV2,
    core_program: &Pubkey,
    registry_program: &Pubkey,
) -> Result<(), SeriesV2Error> {
    if *core_program == Pubkey::default() || *registry_program == Pubkey::default() {
        return Err(SeriesV2Error::Market);
    }
    let occurrence = admitted.occurrence;
    let identity = MarketIdentity {
        market_id: core_pubkey_identity(occurrence.market)?,
        realm_id: core_identity(admitted.template.realm)?,
        product_id: core_identity(occurrence.product)?,
        result_domain: core_identity(occurrence.result_domain)?,
        resolution_policy: core_identity(occurrence.resolution_policy)?,
        capability_manifest: core_identity(occurrence.capability_manifest)?,
        selected_release_set: core_identity(admitted.template.release_set)?,
        registry_program: core_pubkey_identity(*registry_program)?,
        generation: u64::from(occurrence.occurrence) + 1,
    };
    let seeds = MarketCoreStateSeedsV1::new(identity);
    let expected = Pubkey::find_program_address(&seeds.as_slices(), core_program).0;
    if expected != occurrence.market {
        return Err(SeriesV2Error::Market);
    }
    Ok(())
}

/// Hash the exact ordered nonempty FundingState-key list.
///
/// The preimage is `u16_le(count) || key[0] || ... || key[count-1]` under the
/// Lean-owned funding-list domain. Reordering, aliasing, zero keys, or adding a
/// state changes or refuses the identity.
pub fn funding_list_id(funding_states: &[Pubkey]) -> Result<ContentId, SeriesV2Error> {
    if funding_states.is_empty() || funding_states.len() > MAXIMUM_FUNDING_STATES {
        return Err(SeriesV2Error::Funding);
    }
    let mut preimage = [0_u8; FUNDING_LIST_BUFFER_BYTES];
    let count = u16::try_from(funding_states.len()).map_err(|_| SeriesV2Error::Funding)?;
    preimage
        .get_mut(..FUNDING_LIST_PREFIX_BYTES)
        .ok_or(SeriesV2Error::Funding)?
        .copy_from_slice(&count.to_le_bytes());
    for (index, key) in funding_states.iter().enumerate() {
        if *key == Pubkey::default()
            || funding_states
                .get(..index)
                .ok_or(SeriesV2Error::Funding)?
                .iter()
                .any(|prior| prior == key)
        {
            return Err(SeriesV2Error::Funding);
        }
        let start = FUNDING_LIST_PREFIX_BYTES + index * 32;
        preimage
            .get_mut(start..start.checked_add(32).ok_or(SeriesV2Error::Funding)?)
            .ok_or(SeriesV2Error::Funding)?
            .copy_from_slice(key.as_ref());
    }
    let used = FUNDING_LIST_PREFIX_BYTES + funding_states.len() * 32;
    content_id(
        &generated::SERIES_FUNDING_LIST_DOMAIN_V2,
        preimage.get(..used).ok_or(SeriesV2Error::Funding)?,
    )
}

/// Require actual ordered FundingState accounts to match the occurrence ID.
pub fn require_funding_list(
    occurrence: OccurrenceV2,
    funding_states: &[Pubkey],
) -> Result<(), SeriesV2Error> {
    if funding_list_id(funding_states)? != occurrence.funding_list {
        return Err(SeriesV2Error::Funding);
    }
    Ok(())
}

fn proof_height(count: u32) -> usize {
    if count <= 1 {
        0
    } else {
        (u32::BITS - (count - 1).leading_zeros()) as usize
    }
}

fn content_id(domain: &[u8], bytes: &[u8]) -> Result<ContentId, SeriesV2Error> {
    ContentId::new(hashv(&[domain, &HASH_SEPARATOR, bytes]).to_bytes())
        .map_err(|_| SeriesV2Error::Identity)
}

fn core_identity(value: ContentId) -> Result<CoreIdentity, SeriesV2Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV2Error::Identity)
}

fn core_pubkey_identity(value: Pubkey) -> Result<CoreIdentity, SeriesV2Error> {
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

fn read_pubkey(bytes: &[u8], offset: usize) -> Result<Pubkey, SeriesV2Error> {
    let value = Pubkey::new_from_array(read_array(bytes, offset)?);
    if value == Pubkey::default() {
        return Err(SeriesV2Error::Identity);
    }
    Ok(value)
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
