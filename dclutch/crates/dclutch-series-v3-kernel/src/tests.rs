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
fn funding_list_is_unbounded_by_the_physical_width_profile_but_bounded_by_the_chain() {
    let mut wide = [key(1); 17];
    for (index, entry) in wide.iter_mut().enumerate() {
        let byte = u8::try_from(index + 1).expect("small index");
        *entry = key(byte);
    }
    // Seventeen: the point of this case. Every physical profile that reaches
    // this kernel caps its funding span at sixteen, and the kernel does not
    // inherit that cap.
    assert!(funding_list_id(&wide).is_ok());
    assert_ne!(
        funding_list_id(&wide).expect("wide exact list"),
        funding_list_id(&wide[..16]).expect("shorter exact list")
    );
    // What it IS bounded by is the number of accounts a transaction can lock,
    // because the only thing that ever checks a funding list presents the whole
    // list in one instruction.
    let mut widest = [key(1); SERIES_MAX_FUNDING_STATES_V3];
    for (index, entry) in widest.iter_mut().enumerate() {
        *entry = distinct_key(index);
    }
    assert!(funding_list_id(&widest).is_ok());
    let mut over = [key(1); SERIES_MAX_FUNDING_STATES_V3 + 1];
    for (index, entry) in over.iter_mut().enumerate() {
        *entry = distinct_key(index);
    }
    assert_eq!(funding_list_id(&over), Err(SeriesV3Error::Funding));
    assert_eq!(funding_list_id(&[]), Err(SeriesV3Error::Funding));
    assert_eq!(
        funding_list_id(&[key(1), key(1)]),
        Err(SeriesV3Error::Funding)
    );
}

fn distinct_key(index: usize) -> AccountKeyV3 {
    let mut bytes = [0_u8; 32];
    bytes[0] = 1;
    bytes[1] = u8::try_from(index / 256).expect("high byte");
    bytes[2] = u8::try_from(index % 256).expect("low byte");
    AccountKeyV3::new(bytes).expect("nonzero key")
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

// ---------------------------------------------------------------------------
// Recurrence: more than one occurrence on one shared clock.
//
// Everything else in this kernel's test corpus proves Series at
// `occurrence_count = 1`. `plan::tests::full_joint_lifecycle_is_atomic_and_terminal`
// passes 1; `terminal::tests` passes 1 and its root-closure test explicitly
// REWRITES the example Template's occurrence count from 3 down to 1;
// `shadow::tests` and the two operator suites hand-fast-forward the root state
// to "occurrence one prepared" as fixture setup and then evaluate a single
// action. `activation::tests::terminal_state` does loop the occurrences, but
// only over `SeriesStateV3` — never through the joint evaluator, never with a
// Ticket, and never distinguishing a consumed occurrence from an expired one.
//
// C-07 asks for the opposite of all of that: "redistribute funding once,
// settle occurrences, close terminal state and replay safely across MORE THAN
// ONE occurrence". These tests are that clause.
// ---------------------------------------------------------------------------

use crate::{
    activation::series_activation_root_tail_v3,
    plan::{
        ReplayCandidateV3, SeriesReplayActionV3, SeriesReplayPlanErrorV3, SeriesReplayWitnessV3,
        evaluate_replay_v3,
    },
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesPhaseV3, SeriesStateError,
        SeriesStateV3, TicketPhaseV3, TicketStateV3,
    },
    terminal::{
        SeriesLifecycleRentSinkV3, SeriesTerminalErrorV3, plan_series_root_closure_v3,
        plan_ticket_retirement_v3,
    },
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
};

/// One Template carrying a real recurrence: `occurrences` scheduled slots on
/// the example Template's own period, refunding to `wallet`.
fn recurrence_template(wallet: AccountKeyV3, occurrences: u32) -> TemplateV3 {
    let mut bytes = generated::SERIES_EXAMPLE_TEMPLATE_V3;
    put(
        &mut bytes,
        generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
        &occurrences.to_le_bytes(),
    );
    put(
        &mut bytes,
        generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
        &wallet.to_bytes(),
    );
    TemplateV3::decode(&bytes).expect("recurrence Template")
}

/// One admitted Ticket per occurrence. The occurrence index is part of the
/// Ticket record, so distinct occurrences get distinct content identities —
/// which is what makes the cross-occurrence substitution control meaningful.
fn recurrence_ticket(wallet: AccountKeyV3, occurrence: u32) -> AdmittedTicketV3 {
    let mut bytes = generated::SERIES_EXAMPLE_TICKET_V3;
    put(
        &mut bytes,
        generated::SERIES_TICKET_INDEX_OFFSET_V3,
        &occurrence.to_le_bytes(),
    );
    put(
        &mut bytes,
        generated::SERIES_TICKET_REFUND_OWNER_OFFSET_V3,
        &wallet.to_bytes(),
    );
    admit_ticket(&bytes).expect("occurrence Ticket")
}

fn recurrence_sink(wallet: AccountKeyV3) -> SeriesLifecycleRentSinkV3 {
    let credit = LifecycleRentCreditV2::new(
        RefundAuthority::new(wallet.to_bytes()).expect("wallet"),
        LifecycleAccountIdV2::new([31; 32]).expect("Market"),
        LifecycleAccountIdV2::new([32; 32]).expect("release"),
        7,
        9,
    )
    .expect("credit");
    SeriesLifecycleRentSinkV3::admit(
        key(30),
        &credit.to_bytes(),
        key(31),
        ContentId::new([32; 32]).expect("release set"),
        7,
        wallet,
    )
    .expect("sink")
}

fn root_of(witness: SeriesReplayWitnessV3) -> [u8; SERIES_STATE_BYTES_V3] {
    match witness.series() {
        ReplayCandidateV3::Replace(bytes) => Some(bytes),
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => None,
    }
    .expect("the action replaces the Series root")
}

fn ticket_of(witness: SeriesReplayWitnessV3) -> [u8; SERIES_TICKET_STATE_BYTES_V3] {
    match witness.ticket() {
        ReplayCandidateV3::Replace(bytes) => Some(bytes),
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => None,
    }
    .expect("the action replaces the Ticket state")
}

/// The clock two occurrences share is the Template's own, and occurrence
/// zero's retry window closes strictly before occurrence one is due.
///
/// This is the property that makes "expire one, consume another" a schedule
/// rather than a pair of unrelated events: if the windows overlapped, a
/// prepared Ticket could still be retried after its successor was already
/// scheduled, and `settle_current` — which admits exactly one settlement per
/// occurrence and does not name a slot — would be the only thing standing
/// between the two.
#[test]
fn the_shared_clock_separates_every_occurrence_from_its_successor() {
    let template = recurrence_template(key(71), 3);
    assert_eq!(template.occurrence_count(), 3);
    for occurrence in 0..template.occurrence_count() {
        let scheduled = template.scheduled_slot(occurrence).expect("scheduled slot");
        assert_eq!(
            scheduled,
            template.first_slot() + u64::from(occurrence) * template.period_slots()
        );
        let retry_through = template.retry_through(occurrence).expect("retry through");
        assert_eq!(retry_through, scheduled + template.retry_window());
        if let Ok(next) = template.scheduled_slot(occurrence + 1) {
            assert!(
                retry_through < next,
                "occurrence {occurrence} may still be retried at {retry_through}, \
                 at or after its successor is due at {next}"
            );
        }
    }
    // Past the last occurrence there is no slot at all; the schedule is finite.
    assert_eq!(
        template.scheduled_slot(template.occurrence_count()),
        Err(SeriesV3Error::Schedule)
    );
}

/// TWO occurrences on one shared clock, through the real joint evaluator:
/// occurrence zero is CONSUMED, occurrence one EXPIRES, both Tickets retire,
/// and the terminal root closes returning its prepaid principal.
///
/// The starting root is not a literal. It is `series_activation_root_tail_v3`
/// — the exact tail Series' own activation writes — so the recurrence begins
/// where activation (`929095f5`) actually leaves the chain, and a change to
/// either owner shows up here instead of being absorbed by a fixture.
#[test]
fn two_occurrences_consume_then_expire_and_close_the_terminal_root() {
    let wallet = key(72);
    let template = recurrence_template(wallet, 2);
    let occurrences = template.occurrence_count();
    assert_eq!(occurrences, 2);

    let consumed_ticket = recurrence_ticket(wallet, 0);
    let expired_ticket = recurrence_ticket(wallet, 1);
    assert_ne!(
        consumed_ticket.content_id(),
        expired_ticket.content_id(),
        "two occurrences must not share one Ticket record identity"
    );

    // Activation's own tail, decoded under the Template's own occurrence count.
    let activated = series_activation_root_tail_v3(template).expect("activation tail");
    let root = SeriesStateV3::decode(&activated, occurrences).expect("decode activation tail");
    assert_eq!(root.phase(), SeriesPhaseV3::Active);
    assert_eq!(root.next_occurrence(), 0);
    assert_eq!(root.outstanding_ticket_accounts(), 0);
    assert_eq!(root.close_rent_remaining(), template.close_rent());

    // ---- occurrence zero: prepare, then CONSUME -------------------------
    let prepared_zero = evaluate_replay_v3(
        SeriesReplayActionV3::Prepare {
            ticket_record: consumed_ticket.content_id(),
        },
        occurrences,
        0,
        &activated,
        None,
    )
    .expect("prepare occurrence zero");
    let root_zero_prepared = root_of(prepared_zero);
    let ticket_zero_prepared = ticket_of(prepared_zero);

    let consumed = evaluate_replay_v3(
        SeriesReplayActionV3::Consume {
            ticket_record: consumed_ticket.content_id(),
            expected_ticket_revision: 0,
        },
        occurrences,
        1,
        &root_zero_prepared,
        Some(&ticket_zero_prepared),
    )
    .expect("consume occurrence zero");
    let root_after_zero = root_of(consumed);
    let ticket_zero_consumed = ticket_of(consumed);
    assert_eq!(
        TicketStateV3::decode(&ticket_zero_consumed)
            .expect("consumed Ticket")
            .phase(),
        TicketPhaseV3::Consumed
    );
    let root_state_after_zero =
        SeriesStateV3::decode(&root_after_zero, occurrences).expect("root after occurrence zero");
    assert_eq!(root_state_after_zero.next_occurrence(), 1);
    assert_eq!(
        root_state_after_zero.phase(),
        SeriesPhaseV3::Active,
        "one occurrence still remains, so the root is not terminal yet"
    );
    assert_eq!(root_state_after_zero.outstanding_ticket_accounts(), 1);

    // ---- occurrence one: prepare, then EXPIRE ---------------------------
    // The consumed Ticket of occurrence zero is deliberately still outstanding
    // here: retirement is a separate act, and the recurrence must advance
    // without it.
    let prepared_one = evaluate_replay_v3(
        SeriesReplayActionV3::Prepare {
            ticket_record: expired_ticket.content_id(),
        },
        occurrences,
        2,
        &root_after_zero,
        None,
    )
    .expect("prepare occurrence one");
    let root_one_prepared = root_of(prepared_one);
    let ticket_one_prepared = ticket_of(prepared_one);
    assert_eq!(
        SeriesStateV3::decode(&root_one_prepared, occurrences)
            .expect("root at occurrence one")
            .outstanding_ticket_accounts(),
        2,
        "both the consumed and the prepared Ticket are live accounts"
    );

    let expired = evaluate_replay_v3(
        SeriesReplayActionV3::Expire {
            ticket_record: expired_ticket.content_id(),
            expected_ticket_revision: 0,
        },
        occurrences,
        3,
        &root_one_prepared,
        Some(&ticket_one_prepared),
    )
    .expect("expire occurrence one");
    let root_after_one = root_of(expired);
    let ticket_one_expired = ticket_of(expired);
    assert_eq!(
        TicketStateV3::decode(&ticket_one_expired)
            .expect("expired Ticket")
            .phase(),
        TicketPhaseV3::Expired,
        "an expired occurrence is a DIFFERENT terminal phase from a consumed one"
    );
    let terminal_root =
        SeriesStateV3::decode(&root_after_one, occurrences).expect("root after occurrence one");
    assert_eq!(terminal_root.next_occurrence(), occurrences);
    assert_eq!(
        terminal_root.phase(),
        SeriesPhaseV3::Terminal,
        "every scheduled occurrence has settled"
    );

    // ---- the schedule is exhausted --------------------------------------
    assert_eq!(
        evaluate_replay_v3(
            SeriesReplayActionV3::Prepare {
                ticket_record: recurrence_ticket(wallet, 2).content_id(),
            },
            occurrences,
            4,
            &root_after_one,
            None,
        ),
        Err(SeriesReplayPlanErrorV3::State(SeriesStateError::Replay)),
        "a terminal Series admits no third occurrence"
    );

    // ---- close refuses while any Ticket account is still outstanding ----
    assert_eq!(
        evaluate_replay_v3(
            SeriesReplayActionV3::Close,
            occurrences,
            4,
            &root_after_one,
            None,
        ),
        Err(SeriesReplayPlanErrorV3::State(SeriesStateError::Replay)),
        "two Tickets are still live"
    );

    // ---- retire both Tickets through the real funding planner -----------
    // Occurrence zero's Ticket carries 3 lamports of unsolicited donation over
    // its exact rent; occurrence one's carries none. Both are credited to the
    // one lifecycle-scoped Rent sink, and neither can be credited twice.
    let sink = recurrence_sink(wallet);
    let retire_zero = plan_ticket_retirement_v3(
        occurrences,
        terminal_root,
        TicketStateV3::decode(&ticket_zero_consumed).expect("consumed Ticket"),
        consumed_ticket,
        4,
        1,
        26,
        23,
        sink,
    )
    .expect("retire the consumed Ticket");
    assert_eq!(retire_zero.ticket_rent(), 23);
    assert_eq!(retire_zero.donation(), 3);
    assert_eq!(retire_zero.total_credit(), Ok(26));
    assert_eq!(retire_zero.series_after().outstanding_ticket_accounts(), 1);

    // Once only, part one: the root's revision guard. The retirement replayed
    // at the revision it consumed refuses.
    assert_eq!(
        plan_ticket_retirement_v3(
            occurrences,
            retire_zero.series_after(),
            TicketStateV3::decode(&ticket_zero_consumed).expect("consumed Ticket"),
            consumed_ticket,
            4,
            1,
            26,
            23,
            sink,
        ),
        Err(SeriesTerminalErrorV3::Replay)
    );
    // Once only, part two: the Ticket's own revision guard.
    assert_eq!(
        plan_ticket_retirement_v3(
            occurrences,
            retire_zero.series_after(),
            TicketStateV3::decode(&ticket_zero_consumed).expect("consumed Ticket"),
            consumed_ticket,
            5,
            0,
            26,
            23,
            sink,
        ),
        Err(SeriesTerminalErrorV3::Replay)
    );
    // Once only, part three — and this is the arm that is NOT in the replay
    // state at all. `outstanding_ticket_accounts` is a COUNT, not a set: the
    // root never learns WHICH Ticket retired, so at the root revision
    // occurrence one's retirement legitimately holds, occurrence zero's
    // already-retired Ticket would decrement the count a second time if its
    // bytes could be presented again. What makes that unreachable is physical
    // rather than semantic — retirement DELETES the account, and a vacant or
    // zeroed account cannot decode. Both refusals are measured here rather
    // than assumed, because the stateless evaluator is handed these bytes by
    // its caller and cannot itself observe a deletion.
    assert_eq!(
        TicketStateV3::decode(&[]),
        Err(SeriesStateError::Encoding),
        "a deleted Ticket account has no bytes to re-present"
    );
    assert_eq!(
        TicketStateV3::decode(&[0_u8; SERIES_TICKET_STATE_BYTES_V3]),
        Err(SeriesStateError::Encoding),
        "a zeroed Ticket account of the right width still fails its magic"
    );

    let retire_one = plan_ticket_retirement_v3(
        occurrences,
        retire_zero.series_after(),
        TicketStateV3::decode(&ticket_one_expired).expect("expired Ticket"),
        expired_ticket,
        5,
        1,
        23,
        23,
        sink,
    )
    .expect("retire the expired Ticket");
    assert_eq!(retire_one.ticket_rent(), 23);
    assert_eq!(retire_one.donation(), 0);
    assert_eq!(retire_one.total_credit(), Ok(23));
    assert_eq!(retire_one.series_after().outstanding_ticket_accounts(), 0);

    // ---- terminal close returns the prepaid principal -------------------
    let closed_root = retire_one.series_after();
    assert_eq!(closed_root.phase(), SeriesPhaseV3::Terminal);
    assert_eq!(closed_root.close_rent_remaining(), template.close_rent());
    let close = plan_series_root_closure_v3(template, closed_root, 6, 40, 10, sink)
        .expect("close the terminal root");
    assert_eq!(close.root_rent(), 10);
    assert_eq!(
        close.close_rent(),
        template.close_rent(),
        "the Template's own prepaid close principal comes back, not a constant"
    );
    assert_eq!(
        close.donation(),
        30 - template.close_rent(),
        "everything not classified as rent or principal is donation"
    );
    assert_eq!(close.total_credit(), Ok(40));

    // Once only: closing again at the revision the close consumed refuses.
    assert_eq!(
        plan_series_root_closure_v3(template, closed_root, 5, 40, 10, sink),
        Err(SeriesTerminalErrorV3::Replay)
    );
}

/// A settlement belonging to one occurrence cannot be replayed into another.
///
/// This is the "replay safely across more than one occurrence" clause, and it
/// is the one a single-occurrence corpus structurally cannot state: with only
/// one occurrence there is no second root state to replay INTO.
#[test]
fn one_occurrences_settlement_cannot_be_replayed_into_another() {
    let wallet = key(73);
    let template = recurrence_template(wallet, 2);
    let occurrences = template.occurrence_count();
    let zero_ticket = recurrence_ticket(wallet, 0);
    let one_ticket = recurrence_ticket(wallet, 1);

    let activated = series_activation_root_tail_v3(template).expect("activation tail");
    let prepared_zero = evaluate_replay_v3(
        SeriesReplayActionV3::Prepare {
            ticket_record: zero_ticket.content_id(),
        },
        occurrences,
        0,
        &activated,
        None,
    )
    .expect("prepare occurrence zero");
    let root_zero_prepared = root_of(prepared_zero);
    let ticket_zero_prepared = ticket_of(prepared_zero);
    let consumed = evaluate_replay_v3(
        SeriesReplayActionV3::Consume {
            ticket_record: zero_ticket.content_id(),
            expected_ticket_revision: 0,
        },
        occurrences,
        1,
        &root_zero_prepared,
        Some(&ticket_zero_prepared),
    )
    .expect("consume occurrence zero");
    let root_after_zero = root_of(consumed);
    let ticket_zero_consumed = ticket_of(consumed);

    // Occurrence zero's exact settlement, byte for byte, replayed against the
    // root it already advanced: refused on the root's revision.
    assert_eq!(
        evaluate_replay_v3(
            SeriesReplayActionV3::Consume {
                ticket_record: zero_ticket.content_id(),
                expected_ticket_revision: 0,
            },
            occurrences,
            1,
            &root_after_zero,
            Some(&ticket_zero_prepared),
        ),
        Err(SeriesReplayPlanErrorV3::State(SeriesStateError::Replay))
    );

    // Even at the root revision occurrence one legitimately holds, occurrence
    // zero's already-terminal Ticket cannot settle a second time.
    let prepared_one = evaluate_replay_v3(
        SeriesReplayActionV3::Prepare {
            ticket_record: one_ticket.content_id(),
        },
        occurrences,
        2,
        &root_after_zero,
        None,
    )
    .expect("prepare occurrence one");
    let root_one_prepared = root_of(prepared_one);
    let ticket_one_prepared = ticket_of(prepared_one);
    assert_eq!(
        evaluate_replay_v3(
            SeriesReplayActionV3::Expire {
                ticket_record: zero_ticket.content_id(),
                expected_ticket_revision: 1,
            },
            occurrences,
            3,
            &root_one_prepared,
            Some(&ticket_zero_consumed),
        ),
        Err(SeriesReplayPlanErrorV3::State(SeriesStateError::Replay)),
        "an already consumed Ticket cannot also expire"
    );

    // And occurrence one's live settlement cannot be redirected onto
    // occurrence zero's Ticket record, which is what a substituted occurrence
    // would look like on the wire.
    assert_eq!(
        evaluate_replay_v3(
            SeriesReplayActionV3::Expire {
                ticket_record: zero_ticket.content_id(),
                expected_ticket_revision: 0,
            },
            occurrences,
            3,
            &root_one_prepared,
            Some(&ticket_one_prepared),
        ),
        Err(SeriesReplayPlanErrorV3::TicketSubstitution)
    );
}
