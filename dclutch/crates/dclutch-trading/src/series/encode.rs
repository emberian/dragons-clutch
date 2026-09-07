//! Canonical fixed-layout Series record construction from explicit founder facts.
//!
//! The generated layout and the existing hostile decoder remain the authorities.
//! No example record, account observation, or implicit economic default enters
//! these encoders. A Ticket inherits its funding and refund owner from the
//! admitted occurrence rather than accepting a second copy of those facts.

use super::{
    AccountKeyV3, AdmittedOccurrenceV3, ContentId, FoundingFundsV3, OccurrenceV3, SeriesV3Error,
    TemplateV3, TicketV3, generated::*,
};

/// Explicit immutable inputs to one recurring Template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateRecordInputV3 {
    /// Realm content identity.
    pub realm: ContentId,
    /// Selected release-set identity.
    pub release_set: ContentId,
    /// Product generator identity.
    pub product_generator: ContentId,
    /// Occurrence generator identity.
    pub occurrence_generator: ContentId,
    /// Capability template identity.
    pub capability_template: ContentId,
    /// Product derivation identity.
    pub product_derivation: ContentId,
    /// Occurrence derivation identity.
    pub occurrence_derivation: ContentId,
    /// Capability derivation identity.
    pub capability_derivation: ContentId,
    /// Funding derivation identity.
    pub funding_derivation: ContentId,
    /// Ordered occurrence projection Merkle root.
    pub projection_root: ContentId,
    /// Immutable refund owner for the entire Series.
    pub refund_owner: AccountKeyV3,
    /// Exact nonzero count of committed occurrences.
    pub occurrence_count: u32,
    /// First scheduled slot.
    pub first_slot: u64,
    /// Nonzero distance between scheduled occurrences.
    pub period_slots: u64,
    /// Inclusive retry window after each scheduled slot.
    pub retry_window: u64,
    /// Prepaid terminal close principal, separate from collateral.
    pub close_rent: u64,
}

impl From<TemplateV3> for TemplateRecordInputV3 {
    fn from(value: TemplateV3) -> Self {
        Self {
            realm: value.realm,
            release_set: value.release_set,
            product_generator: value.product_generator,
            occurrence_generator: value.occurrence_generator,
            capability_template: value.capability_template,
            product_derivation: value.product_derivation,
            occurrence_derivation: value.occurrence_derivation,
            capability_derivation: value.capability_derivation,
            funding_derivation: value.funding_derivation,
            projection_root: value.projection_root,
            refund_owner: value.refund_owner,
            occurrence_count: value.occurrence_count,
            first_slot: value.first_slot,
            period_slots: value.period_slots,
            retry_window: value.retry_window,
            close_rent: value.close_rent,
        }
    }
}

/// Explicit immutable facts for one realized occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceRecordInputV3 {
    /// Ordered occurrence index.
    pub occurrence: u32,
    /// Slot derived from the selected Template schedule.
    pub scheduled_slot: u64,
    /// Finalized Product record identity.
    pub product_record: ContentId,
    /// Resolution policy identity.
    pub resolution_policy: ContentId,
    /// Liability basis identity.
    pub liability_basis: ContentId,
    /// Rational representation identity.
    pub rational_representation: ContentId,
    /// Future Market capability manifest identity.
    pub capability_manifest: ContentId,
    /// Ordered canonical FundingState-list commitment.
    pub funding_list: ContentId,
    /// Derived future Market address.
    pub market: AccountKeyV3,
    /// Exact disjoint founding compartments.
    pub funds: FoundingFundsV3,
}

impl From<OccurrenceV3> for OccurrenceRecordInputV3 {
    fn from(value: OccurrenceV3) -> Self {
        Self {
            occurrence: value.occurrence,
            scheduled_slot: value.scheduled_slot,
            product_record: value.product_record,
            resolution_policy: value.resolution_policy,
            liability_basis: value.liability_basis,
            rational_representation: value.rational_representation,
            capability_manifest: value.capability_manifest,
            funding_list: value.funding_list,
            market: value.market,
            funds: value.funds,
        }
    }
}

impl FoundingFundsV3 {
    /// Construct exact founding funding, refusing absent collateral or native overflow.
    /// Collateral principal is never included in the native-lamport sum.
    pub fn new(
        hoard_principal: u64,
        market_rent: u64,
        capability_native: u64,
        founding_work: u64,
    ) -> Result<Self, SeriesV3Error> {
        if hoard_principal == 0 {
            return Err(SeriesV3Error::Funding);
        }
        let value = Self {
            hoard_principal,
            market_rent,
            capability_native,
            founding_work,
        };
        value.checked_native_total()?;
        Ok(value)
    }
}

/// Encode a Template, then admit its exact bytes under the canonical decoder.
pub fn encode_template_v3(
    input: TemplateRecordInputV3,
) -> Result<[u8; SERIES_TEMPLATE_BYTES_V3], SeriesV3Error> {
    let mut bytes = header::<SERIES_TEMPLATE_BYTES_V3>(SERIES_TEMPLATE_MAGIC_V3)?;
    put(
        &mut bytes,
        SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
        &input.occurrence_count.to_le_bytes(),
    )?;
    for (offset, value) in [
        (SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3, input.first_slot),
        (SERIES_TEMPLATE_PERIOD_SLOTS_OFFSET_V3, input.period_slots),
        (SERIES_TEMPLATE_RETRY_WINDOW_OFFSET_V3, input.retry_window),
        (SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3, input.close_rent),
    ] {
        put(&mut bytes, offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (SERIES_TEMPLATE_REALM_OFFSET_V3, input.realm),
        (SERIES_TEMPLATE_RELEASE_SET_OFFSET_V3, input.release_set),
        (
            SERIES_TEMPLATE_PRODUCT_GENERATOR_OFFSET_V3,
            input.product_generator,
        ),
        (
            SERIES_TEMPLATE_OCCURRENCE_GENERATOR_OFFSET_V3,
            input.occurrence_generator,
        ),
        (
            SERIES_TEMPLATE_CAPABILITY_TEMPLATE_OFFSET_V3,
            input.capability_template,
        ),
        (
            SERIES_TEMPLATE_PRODUCT_DERIVATION_OFFSET_V3,
            input.product_derivation,
        ),
        (
            SERIES_TEMPLATE_OCCURRENCE_DERIVATION_OFFSET_V3,
            input.occurrence_derivation,
        ),
        (
            SERIES_TEMPLATE_CAPABILITY_DERIVATION_OFFSET_V3,
            input.capability_derivation,
        ),
        (
            SERIES_TEMPLATE_FUNDING_DERIVATION_OFFSET_V3,
            input.funding_derivation,
        ),
        (
            SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            input.projection_root,
        ),
    ] {
        put(&mut bytes, offset, &value.to_bytes())?;
    }
    put(
        &mut bytes,
        SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
        &input.refund_owner.to_bytes(),
    )?;
    TemplateV3::decode(&bytes)?;
    Ok(bytes)
}

/// Encode one occurrence. Template schedule and Merkle membership are admitted separately.
pub fn encode_occurrence_v3(
    input: OccurrenceRecordInputV3,
) -> Result<[u8; SERIES_OCCURRENCE_BYTES_V3], SeriesV3Error> {
    let mut bytes = header::<SERIES_OCCURRENCE_BYTES_V3>(SERIES_OCCURRENCE_MAGIC_V3)?;
    put(
        &mut bytes,
        SERIES_OCCURRENCE_INDEX_OFFSET_V3,
        &input.occurrence.to_le_bytes(),
    )?;
    put(
        &mut bytes,
        SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        &input.scheduled_slot.to_le_bytes(),
    )?;
    for (offset, value) in [
        (
            SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
            input.product_record,
        ),
        (
            SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V3,
            input.resolution_policy,
        ),
        (
            SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3,
            input.liability_basis,
        ),
        (
            SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V3,
            input.rational_representation,
        ),
        (
            SERIES_OCCURRENCE_CAPABILITY_MANIFEST_OFFSET_V3,
            input.capability_manifest,
        ),
        (SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3, input.funding_list),
    ] {
        put(&mut bytes, offset, &value.to_bytes())?;
    }
    put(
        &mut bytes,
        SERIES_OCCURRENCE_MARKET_OFFSET_V3,
        &input.market.to_bytes(),
    )?;
    write_funds(
        &mut bytes,
        SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3,
        input.funds,
    )?;
    OccurrenceV3::decode(&bytes)?;
    Ok(bytes)
}

/// Derive the unique Ticket funding/refund commitment for an admitted occurrence and founder.
pub fn encode_ticket_v3(
    admitted: AdmittedOccurrenceV3,
    founder: AccountKeyV3,
) -> Result<[u8; SERIES_TICKET_BYTES_V3], SeriesV3Error> {
    let mut bytes = header::<SERIES_TICKET_BYTES_V3>(SERIES_TICKET_MAGIC_V3)?;
    let occurrence = admitted.occurrence;
    put(
        &mut bytes,
        SERIES_TICKET_INDEX_OFFSET_V3,
        &occurrence.occurrence.to_le_bytes(),
    )?;
    for (offset, value) in [
        (
            SERIES_TICKET_TEMPLATE_OFFSET_V3,
            admitted.template_id.to_bytes(),
        ),
        (
            SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            admitted.occurrence_id.to_bytes(),
        ),
        (SERIES_TICKET_MARKET_OFFSET_V3, occurrence.market.to_bytes()),
        (
            SERIES_TICKET_FUNDING_LIST_OFFSET_V3,
            occurrence.funding_list.to_bytes(),
        ),
        (SERIES_TICKET_FOUNDER_OFFSET_V3, founder.to_bytes()),
        (
            SERIES_TICKET_REFUND_OWNER_OFFSET_V3,
            admitted.template.refund_owner.to_bytes(),
        ),
    ] {
        put(&mut bytes, offset, &value)?;
    }
    write_funds(
        &mut bytes,
        SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3,
        occurrence.funds,
    )?;
    admitted.require_ticket(TicketV3::decode(&bytes)?)?;
    Ok(bytes)
}

fn header<const N: usize>(magic: [u8; 8]) -> Result<[u8; N], SeriesV3Error> {
    let mut bytes = [0; N];
    put(&mut bytes, 0, &magic)?;
    put(
        &mut bytes,
        super::HEADER_VERSION_OFFSET,
        &SERIES_TEMPLATE_SCHEMA_V3.to_le_bytes(),
    )?;
    put(
        &mut bytes,
        super::HEADER_PROFILE_OFFSET,
        &SERIES_TEMPLATE_PROFILE_V3.to_le_bytes(),
    )?;
    Ok(bytes)
}

fn write_funds(
    bytes: &mut [u8],
    offset: usize,
    funds: FoundingFundsV3,
) -> Result<(), SeriesV3Error> {
    for (index, value) in [
        funds.hoard_principal,
        funds.market_rent,
        funds.capability_native,
        funds.founding_work,
    ]
    .into_iter()
    .enumerate()
    {
        let coordinate = index
            .checked_mul(8)
            .and_then(|stride| offset.checked_add(stride))
            .ok_or(SeriesV3Error::Length)?;
        put(bytes, coordinate, &value.to_le_bytes())?;
    }
    Ok(())
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), SeriesV3Error> {
    let end = offset
        .checked_add(value.len())
        .ok_or(SeriesV3Error::Length)?;
    bytes
        .get_mut(offset..end)
        .ok_or(SeriesV3Error::Length)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_records_match_generated_layout_and_reject_invalid_schedule_and_funds() {
        let template = TemplateV3::decode(&SERIES_EXAMPLE_TEMPLATE_V3).expect("generated Template");
        let mut input = TemplateRecordInputV3::from(template);
        assert_eq!(
            encode_template_v3(input).expect("exact Template"),
            SERIES_EXAMPLE_TEMPLATE_V3
        );
        let occurrence =
            OccurrenceV3::decode(&SERIES_EXAMPLE_OCCURRENCE_V3).expect("generated occurrence");
        assert_eq!(
            encode_occurrence_v3(occurrence.into()).expect("exact occurrence"),
            SERIES_EXAMPLE_OCCURRENCE_V3
        );
        input.period_slots = 0;
        assert_eq!(encode_template_v3(input), Err(SeriesV3Error::Schedule));
        input = template.into();
        input.first_slot = u64::MAX;
        assert_eq!(encode_template_v3(input), Err(SeriesV3Error::Schedule));
        assert_eq!(
            FoundingFundsV3::new(0, 1, 2, 3),
            Err(SeriesV3Error::Funding)
        );
        assert_eq!(
            FoundingFundsV3::new(1, u64::MAX, 1, 0),
            Err(SeriesV3Error::Funding)
        );
        assert_eq!(
            FoundingFundsV3::new(u64::MAX, 1, 2, 3)
                .expect("separate denominations")
                .checked_native_total(),
            Ok(6)
        );
    }

    #[test]
    fn encoded_ticket_inherits_the_admitted_funding_and_refund_owner() {
        let mut occurrence = OccurrenceRecordInputV3::from(
            OccurrenceV3::decode(&SERIES_EXAMPLE_OCCURRENCE_V3).expect("occurrence"),
        );
        occurrence.occurrence = 0;
        occurrence.scheduled_slot = 100;
        let occurrence_bytes = encode_occurrence_v3(occurrence).expect("occurrence bytes");
        let mut template = TemplateRecordInputV3::from(
            TemplateV3::decode(&SERIES_EXAMPLE_TEMPLATE_V3).expect("Template"),
        );
        template.occurrence_count = 1;
        template.projection_root =
            super::super::occurrence_content_id(&occurrence_bytes).expect("leaf");
        let template_bytes = encode_template_v3(template).expect("single leaf Template");
        let admitted =
            super::super::admit_occurrence(&template_bytes, &occurrence_bytes, &[]).expect("proof");
        let founder = AccountKeyV3::new([17; 32]).expect("founder");
        let mut bytes = encode_ticket_v3(admitted, founder).expect("Ticket");
        let ticket = TicketV3::decode(&bytes).expect("admitted Ticket");
        assert_eq!(ticket.founder(), founder);
        assert_eq!(ticket.refund_owner(), template.refund_owner);
        assert_eq!(ticket.funds(), occurrence.funds);
        admitted
            .require_ticket(ticket)
            .expect("complete immutable join");
        put(
            &mut bytes,
            SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3,
            &1_u64.to_le_bytes(),
        )
        .expect("hostile funding");
        assert_eq!(
            admitted.require_ticket(TicketV3::decode(&bytes).expect("well formed hostile")),
            Err(SeriesV3Error::Commitment)
        );
    }
}
