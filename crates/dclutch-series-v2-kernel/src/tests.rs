use super::*;

fn key(byte: u8) -> AccountKeyV2 {
    AccountKeyV2::new([byte; 32]).expect("nonzero test key")
}

fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) {
    target
        .get_mut(offset..offset + N)
        .expect("fixture field")
        .copy_from_slice(value);
}

fn projection_root(
    occurrence_id: ContentId,
    mut occurrence: u32,
    siblings: &[[u8; 32]],
) -> [u8; 32] {
    let mut node = occurrence_id.to_bytes();
    for sibling in siblings {
        node = if occurrence & 1 == 0 {
            projection_node_hash(&node, sibling)
        } else {
            projection_node_hash(sibling, &node)
        };
        occurrence >>= 1;
    }
    node
}

#[derive(Clone)]
struct Fixture {
    template: [u8; SERIES_TEMPLATE_BYTES_V2],
    occurrence: [u8; SERIES_OCCURRENCE_BYTES_V2],
    ticket: [u8; SERIES_TICKET_BYTES_V2],
    siblings: [[u8; 32]; 2],
    funding: [AccountKeyV2; 2],
    registry_program: AccountKeyV2,
}

impl Fixture {
    fn new() -> Self {
        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V2;
        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V2;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V2;
        let siblings = [[90; 32], [91; 32]];
        let funding = [key(70), key(71)];
        let funding_id = funding_list_id(&funding).expect("funding list");
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V2,
            &funding_id.to_bytes(),
        );
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let root = projection_root(occurrence_id, 1, &siblings);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V2,
            &root,
        );
        let template_id = template_content_id(&template).expect("Template ID");
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V2,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V2,
            &occurrence_id.to_bytes(),
        );
        let market: [u8; 32] = occurrence[generated::SERIES_OCCURRENCE_MARKET_OFFSET_V2
            ..generated::SERIES_OCCURRENCE_MARKET_OFFSET_V2 + 32]
            .try_into()
            .expect("Market field");
        put(
            &mut ticket,
            generated::SERIES_TICKET_MARKET_OFFSET_V2,
            &market,
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V2,
            &funding_id.to_bytes(),
        );
        Self {
            template,
            occurrence,
            ticket,
            siblings,
            funding,
            registry_program: key(59),
        }
    }

    fn admit(&self) -> AdmittedOccurrenceV2 {
        admit_occurrence(&self.template, &self.occurrence, &self.siblings)
            .expect("admitted occurrence")
    }
}

#[test]
fn hostile_records_and_exact_ticket_join_are_one_kernel_truth() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = admit_ticket(&fixture.ticket).expect("admitted Ticket");
    admitted
        .require_ticket(ticket.ticket())
        .expect("exact Ticket join");
    require_funding_list(admitted.occurrence(), &fixture.funding).expect("exact funding list");

    let mut substituted = fixture.ticket;
    *substituted
        .get_mut(generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V2)
        .expect("occurrence ID byte") ^= 1;
    let substituted = admit_ticket(&substituted).expect("substituted Ticket still canonical");
    assert_eq!(
        admitted.require_ticket(substituted.ticket()),
        Err(SeriesV2Error::Commitment)
    );
}

#[test]
fn proof_length_value_order_and_content_substitution_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        admit_occurrence(
            &fixture.template,
            &fixture.occurrence,
            &fixture.siblings[..1]
        ),
        Err(SeriesV2Error::Commitment)
    );

    let mut changed = fixture.siblings;
    changed[0][0] ^= 1;
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &changed),
        Err(SeriesV2Error::Commitment)
    );

    let swapped = [fixture.siblings[1], fixture.siblings[0]];
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &swapped),
        Err(SeriesV2Error::Commitment)
    );

    let mut occurrence = fixture.occurrence;
    *occurrence
        .get_mut(generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V2)
        .expect("LiabilityBasis byte") ^= 1;
    assert_eq!(
        admit_occurrence(&fixture.template, &occurrence, &fixture.siblings),
        Err(SeriesV2Error::Commitment)
    );
}

#[test]
fn future_market_projection_binds_every_core_coordinate() {
    let fixture = Fixture::new();
    let projection = future_market_projection(fixture.admit(), fixture.registry_program)
        .expect("future Market projection");
    assert_eq!(
        projection.identity().market_id.to_bytes(),
        projection.committed_address().to_bytes()
    );
    assert_eq!(projection.identity().generation, 2);
    assert_eq!(projection.seeds().as_slices().len(), 9);
    projection
        .require_address(projection.committed_address())
        .expect("exact derived address");
    assert_eq!(
        projection.require_address(key(99)),
        Err(SeriesV2Error::Market)
    );

    let changed_registry = future_market_projection(fixture.admit(), key(58))
        .expect("changed Registry remains a projection");
    assert_ne!(
        projection.identity().registry_program,
        changed_registry.identity().registry_program
    );
}

#[test]
fn pre_founding_escrow_uses_ticket_replay_and_exact_collateral() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = admit_ticket(&fixture.ticket).expect("admitted Ticket");
    let escrow = pre_founding_series_escrow(admitted, ticket, fixture.registry_program)
        .expect("pre-founding SeriesEscrow");
    assert_eq!(escrow.template_id(), admitted.template_id());
    assert_eq!(escrow.occurrence_id(), admitted.occurrence_id());
    assert_eq!(escrow.ticket_id(), ticket.content_id());
    assert_eq!(escrow.market(), admitted.occurrence().market());
    assert_eq!(escrow.founder(), ticket.ticket().founder());
    assert_eq!(escrow.refund_owner(), ticket.ticket().refund_owner());
    assert_eq!(escrow.occurrence(), admitted.occurrence().occurrence());
    assert_eq!(escrow.generation(), 2);
    assert_eq!(
        escrow.hoard_principal(),
        admitted.occurrence().funds().hoard_principal()
    );
    assert_eq!(escrow.future_market().committed_address(), escrow.market());
}

#[test]
fn funding_list_is_unbounded_by_old_width_profile_but_exact() {
    let mut wide = [key(1); 17];
    for (index, entry) in wide.iter_mut().enumerate() {
        let byte = u8::try_from(index + 1).expect("small index");
        *entry = key(byte);
    }
    assert!(funding_list_id(&wide).is_ok());
    assert_ne!(
        funding_list_id(&wide).expect("wide exact list"),
        funding_list_id(&wide[..16]).expect("shorter exact list")
    );
    assert_eq!(funding_list_id(&[]), Err(SeriesV2Error::Funding));
    assert_eq!(
        funding_list_id(&[key(1), key(1)]),
        Err(SeriesV2Error::Funding)
    );
}

#[test]
fn widths_reserved_zero_identity_and_schedule_overflow_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        TemplateV2::decode(&fixture.template[..SERIES_TEMPLATE_BYTES_V2 - 1]),
        Err(SeriesV2Error::Length)
    );
    let mut occurrence = fixture.occurrence;
    occurrence[generated::SERIES_OCCURRENCE_RESERVED_OFFSET_V2] = 1;
    assert_eq!(
        OccurrenceV2::decode(&occurrence),
        Err(SeriesV2Error::Header)
    );
    let mut ticket = fixture.ticket;
    ticket[generated::SERIES_TICKET_FOUNDER_OFFSET_V2
        ..generated::SERIES_TICKET_FOUNDER_OFFSET_V2 + 32]
        .fill(0);
    assert_eq!(TicketV2::decode(&ticket), Err(SeriesV2Error::Identity));

    let mut overflow = fixture.template;
    put(
        &mut overflow,
        generated::SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V2,
        &u64::MAX.to_le_bytes(),
    );
    assert_eq!(TemplateV2::decode(&overflow), Err(SeriesV2Error::Schedule));
}
