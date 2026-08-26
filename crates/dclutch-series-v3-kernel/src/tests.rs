use super::*;

fn key(byte: u8) -> AccountKeyV3 {
    AccountKeyV3::new([byte; 32]).expect("nonzero test key")
}

fn core_id(byte: u8) -> CoreIdentity {
    CoreIdentity::new([byte; 32]).expect("nonzero Core identity")
}

fn permit_expiry_request(
    admitted: AdmittedOccurrenceV3,
    ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    ticket_context: CoreIdentity,
) -> SeriesPermitExpiryRequestV1 {
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    let ticket_record = ticket.ticket();
    let intent = dclutch_market_core_codec::FoundingIntentV5::new(
        255,
        core_content_identity(template.release_set()).expect("release"),
        core_account_identity(occurrence.market()).expect("Market"),
        core_content_identity(product.product_record()).expect("Product record"),
        core_id(100),
        core_account_identity(ticket_record.founder()).expect("founder"),
        ticket_context,
        core_id(101),
        core_id(102),
        core_id(103),
        core_id(104),
        core_id(105),
        core_id(106),
        core_id(107),
        core_id(108),
        core_id(109),
        u64::from(occurrence.occurrence()) + 1,
        occurrence.funds().hoard_principal(),
        1,
        template
            .retry_through(occurrence.occurrence())
            .expect("retry deadline"),
        4,
        1,
    )
    .expect("canonical founding intent");
    let permit =
        dclutch_market_core_codec::SeriesFoundingPermitV1::new(intent, core_id(110), core_id(111))
            .expect("canonical permit");
    SeriesPermitExpiryRequestV1::new(permit)
}

fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) {
    target
        .get_mut(offset..offset + N)
        .expect("fixture field")
        .copy_from_slice(value);
}

fn rewrite_header_as_obsolete_v2(bytes: &mut [u8]) {
    *bytes.get_mut(7).expect("versioned magic byte") = b'2';
    bytes
        .get_mut(8..10)
        .expect("schema bytes")
        .copy_from_slice(&2_u16.to_le_bytes());
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
    template: [u8; SERIES_TEMPLATE_BYTES_V3],
    occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3],
    ticket: [u8; SERIES_TICKET_BYTES_V3],
    siblings: [[u8; 32]; 2],
    funding: [AccountKeyV3; 2],
    registry_program: AccountKeyV3,
    product: AuthenticatedProductProjectionV2,
}

impl Fixture {
    fn new() -> Self {
        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        let siblings = [[90; 32], [91; 32]];
        let funding = [key(70), key(71)];
        let funding_id = funding_list_id(&funding).expect("funding list");
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3,
            &funding_id.to_bytes(),
        );
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let root = projection_root(occurrence_id, 1, &siblings);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("Template ID");
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        let market: [u8; 32] = occurrence[generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3
            ..generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3 + 32]
            .try_into()
            .expect("Market field");
        put(
            &mut ticket,
            generated::SERIES_TICKET_MARKET_OFFSET_V3,
            &market,
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_FUNDING_LIST_OFFSET_V3,
            &funding_id.to_bytes(),
        );
        let product_record = ContentId::new(
            occurrence[generated::SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3
                ..generated::SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3 + 32]
                .try_into()
                .expect("Product record field"),
        )
        .expect("Product record");
        Self {
            template,
            occurrence,
            ticket,
            siblings,
            funding,
            registry_program: key(59),
            product: AuthenticatedProductProjectionV2::new(
                product_record,
                ContentId::new([61; 32]).expect("stable Product"),
                ContentId::new([62; 32]).expect("result domain"),
            ),
        }
    }

    fn admit(&self) -> AdmittedOccurrenceV3 {
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
        .get_mut(generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3)
        .expect("occurrence ID byte") ^= 1;
    let substituted = admit_ticket(&substituted).expect("substituted Ticket still canonical");
    assert_eq!(
        admitted.require_ticket(substituted.ticket()),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn proof_length_value_order_and_content_substitution_refuse() {
    let fixture = Fixture::new();
    let mut proof_bytes = [0_u8; 64];
    proof_bytes[..32].copy_from_slice(&fixture.siblings[0]);
    proof_bytes[32..].copy_from_slice(&fixture.siblings[1]);
    assert_eq!(
        admit_occurrence_bytes(&fixture.template, &fixture.occurrence, &proof_bytes),
        Ok(fixture.admit())
    );
    assert_eq!(
        admit_occurrence_bytes(&fixture.template, &fixture.occurrence, &proof_bytes[..63]),
        Err(SeriesV3Error::Commitment)
    );
    assert_eq!(
        admit_occurrence(
            &fixture.template,
            &fixture.occurrence,
            &fixture.siblings[..1]
        ),
        Err(SeriesV3Error::Commitment)
    );

    let mut changed = fixture.siblings;
    changed[0][0] ^= 1;
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &changed),
        Err(SeriesV3Error::Commitment)
    );

    let swapped = [fixture.siblings[1], fixture.siblings[0]];
    assert_eq!(
        admit_occurrence(&fixture.template, &fixture.occurrence, &swapped),
        Err(SeriesV3Error::Commitment)
    );

    let mut occurrence = fixture.occurrence;
    *occurrence
        .get_mut(generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3)
        .expect("LiabilityBasis byte") ^= 1;
    assert_eq!(
        admit_occurrence(&fixture.template, &occurrence, &fixture.siblings),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn future_market_projection_binds_every_core_coordinate() {
    let fixture = Fixture::new();
    let projection =
        future_market_projection(fixture.admit(), fixture.product, fixture.registry_program)
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
        Err(SeriesV3Error::Market)
    );

    assert_eq!(
        projection.identity().product_record.to_bytes(),
        fixture.product.product_record().to_bytes()
    );
    assert_eq!(
        projection.identity().product_id.to_bytes(),
        fixture.product.stable_product_id().to_bytes()
    );
    let changed_registry = future_market_projection(fixture.admit(), fixture.product, key(58))
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
    let escrow =
        pre_founding_series_escrow(admitted, ticket, fixture.product, fixture.registry_program)
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
fn consume_core_request_is_one_sdk_free_occurrence_projection() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = admit_ticket(&fixture.ticket).expect("admitted Ticket");
    let request = series_core_consume_request(admitted, ticket, fixture.product, key(72), 8, 11)
        .expect("canonical Consume request");
    assert_eq!(request.action(), SeriesCoreActionV1::Consume);
    assert_eq!(
        request.template().to_bytes(),
        admitted.template_id().to_bytes()
    );
    assert_eq!(
        request.ticket().expect("Ticket state").to_bytes(),
        key(72).to_bytes()
    );
    assert_eq!(
        request.product().expect("Product record").to_bytes(),
        fixture.product.product_record().to_bytes()
    );
    assert_eq!(request.market_generation(), Some(2));
    assert_eq!(request.expected_series_revision(), 8);
    assert_eq!(request.expected_ticket_revision(), 11);
    assert_eq!(
        request.market_rent(),
        admitted.occurrence().funds().market_rent()
    );
    assert_eq!(
        request.capability_rent(),
        admitted.occurrence().funds().capability_native()
    );
    assert_eq!(
        request.work(),
        admitted.occurrence().funds().founding_work()
    );
    assert_eq!(
        request.hoard_principal(),
        admitted.occurrence().funds().hoard_principal()
    );

    let substituted = AuthenticatedProductProjectionV2::new(
        ContentId::new([99; 32]).expect("substituted Product record"),
        fixture.product.stable_product_id(),
        fixture.product.result_domain(),
    );
    assert_eq!(
        series_core_consume_request(admitted, ticket, substituted, key(72), 8, 11),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn permit_expiry_candidate_joins_series_owned_semantics() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let ticket = admit_ticket(&fixture.ticket).expect("admitted Ticket");
    let request = permit_expiry_request(
        admitted,
        ticket,
        fixture.product,
        core_content_identity(ticket.content_id()).expect("Ticket context"),
    );
    validate_series_permit_expiry_request_v3(admitted, ticket, fixture.product, request)
        .expect("exact Series permit expiry");

    let substituted = permit_expiry_request(admitted, ticket, fixture.product, core_id(112));
    assert_eq!(
        validate_series_permit_expiry_request_v3(admitted, ticket, fixture.product, substituted,),
        Err(SeriesV3Error::Commitment)
    );
}

#[test]
fn atomic_consume_composes_core_escrow_funding_and_commit_last_replay() {
    let fixture = Fixture::new();
    let admitted = fixture.admit();
    let admitted_ticket = admit_ticket(&fixture.ticket).expect("admitted Ticket");
    let occurrence_count = admitted.template().occurrence_count();
    let series = replay::SeriesStateV3::new(admitted.template().close_rent())
        .prepare_ticket(0)
        .expect("occurrence zero prepared")
        .settle_current(1, occurrence_count)
        .expect("occurrence zero settled")
        .retire_ticket(2)
        .expect("occurrence zero Ticket retired")
        .prepare_ticket(3)
        .expect("occurrence one prepared");
    let series_bytes = series.encode(occurrence_count).expect("Series bytes");
    let ticket_state = replay::TicketStateV3::prepared(admitted_ticket.content_id()).encode();
    let now_slot = admitted.occurrence().scheduled_slot();

    let composition = composition::compose_series_consume_v3(
        admitted,
        admitted_ticket,
        fixture.product,
        fixture.registry_program,
        key(72),
        &series_bytes,
        &ticket_state,
        now_slot,
        4,
        0,
    )
    .expect("one atomic semantic composition");
    assert_eq!(
        composition.core_request().action(),
        SeriesCoreActionV1::Consume
    );
    assert_eq!(
        composition.funding_list(),
        admitted.occurrence().funding_list()
    );
    let funds = admitted.occurrence().funds();
    assert_eq!(
        composition.native_from_ticket(),
        funds.market_rent() + funds.capability_native() + funds.founding_work()
    );
    assert_ne!(composition.native_from_ticket(), funds.hoard_principal());
    assert!(matches!(
        composition.replay().series(),
        plan::ReplayCandidateV3::Replace(_)
    ));
    assert!(matches!(
        composition.replay().ticket(),
        plan::ReplayCandidateV3::Replace(_)
    ));
    let escrow = composition.escrow();
    assert_eq!(escrow.source_replay_revision(), 3);
    assert_eq!(escrow.amount(), funds.hoard_principal());
    assert_eq!(escrow.escrow().ticket_id(), admitted_ticket.content_id());

    let after_retry = admitted
        .template()
        .retry_through(admitted.occurrence().occurrence())
        .expect("retry deadline")
        .checked_add(1)
        .expect("hostile slot");
    assert_eq!(
        composition::compose_series_consume_v3(
            admitted,
            admitted_ticket,
            fixture.product,
            fixture.registry_program,
            key(72),
            &series_bytes,
            &ticket_state,
            after_retry,
            4,
            0,
        ),
        Err(composition::SeriesConsumeCompositionErrorV3::Schedule)
    );

    let substituted_product = AuthenticatedProductProjectionV2::new(
        ContentId::new([99; 32]).expect("substituted Product record"),
        fixture.product.stable_product_id(),
        fixture.product.result_domain(),
    );
    assert_eq!(
        composition::compose_series_consume_v3(
            admitted,
            admitted_ticket,
            substituted_product,
            fixture.registry_program,
            key(72),
            &series_bytes,
            &ticket_state,
            now_slot,
            4,
            0,
        ),
        Err(composition::SeriesConsumeCompositionErrorV3::Content(
            SeriesV3Error::Commitment
        ))
    );
    assert_eq!(
        composition::compose_series_consume_v3(
            admitted,
            admitted_ticket,
            fixture.product,
            fixture.registry_program,
            key(72),
            &series_bytes,
            &ticket_state,
            now_slot,
            4,
            1,
        ),
        Err(composition::SeriesConsumeCompositionErrorV3::Replay(
            plan::SeriesReplayPlanErrorV3::State(replay::SeriesStateError::Replay)
        ))
    );
}

#[test]
fn product_record_join_refuses_substituted_runtime_projection() {
    let fixture = Fixture::new();
    let substituted = AuthenticatedProductProjectionV2::new(
        ContentId::new([63; 32]).expect("different Product record"),
        fixture.product.stable_product_id(),
        fixture.product.result_domain(),
    );
    assert_eq!(
        future_market_projection(fixture.admit(), substituted, fixture.registry_program),
        Err(SeriesV3Error::Commitment)
    );
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
    assert_eq!(funding_list_id(&[]), Err(SeriesV3Error::Funding));
    assert_eq!(
        funding_list_id(&[key(1), key(1)]),
        Err(SeriesV3Error::Funding)
    );
}

#[test]
fn widths_reserved_zero_identity_and_schedule_overflow_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        TemplateV3::decode(&fixture.template[..SERIES_TEMPLATE_BYTES_V3 - 1]),
        Err(SeriesV3Error::Length)
    );
    let mut occurrence = fixture.occurrence;
    occurrence[generated::SERIES_OCCURRENCE_RESERVED_OFFSET_V3] = 1;
    assert_eq!(
        OccurrenceV3::decode(&occurrence),
        Err(SeriesV3Error::Header)
    );
    let mut ticket = fixture.ticket;
    ticket[generated::SERIES_TICKET_FOUNDER_OFFSET_V3
        ..generated::SERIES_TICKET_FOUNDER_OFFSET_V3 + 32]
        .fill(0);
    assert_eq!(TicketV3::decode(&ticket), Err(SeriesV3Error::Identity));

    let mut overflow = fixture.template;
    put(
        &mut overflow,
        generated::SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3,
        &u64::MAX.to_le_bytes(),
    );
    assert_eq!(TemplateV3::decode(&overflow), Err(SeriesV3Error::Schedule));

    let mut obsolete_template = fixture.template;
    rewrite_header_as_obsolete_v2(&mut obsolete_template);
    assert_eq!(
        TemplateV3::decode(&obsolete_template),
        Err(SeriesV3Error::Header)
    );
    let mut obsolete_occurrence = fixture.occurrence;
    rewrite_header_as_obsolete_v2(&mut obsolete_occurrence);
    assert_eq!(
        OccurrenceV3::decode(&obsolete_occurrence),
        Err(SeriesV3Error::Header)
    );
    let mut obsolete_ticket = fixture.ticket;
    rewrite_header_as_obsolete_v2(&mut obsolete_ticket);
    assert_eq!(
        TicketV3::decode(&obsolete_ticket),
        Err(SeriesV3Error::Header)
    );
}
