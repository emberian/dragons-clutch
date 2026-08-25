use crate::{
    AccountKind, Action, Error, InvocationV1, Limits, Phase, ReleaseReceiptV1, RequestV1,
    SeriesStateV1, TemplateV1, TicketPhase, TicketV1, generated_series as generated, interpret,
};

fn fixture() -> InvocationV1 {
    InvocationV1 {
        template: TemplateV1::decode(&generated::TEMPLATE_EXAMPLE).expect("Lean template"),
        series: SeriesStateV1::decode(&generated::SERIES_EXAMPLE).expect("Lean Series"),
        ticket: TicketV1::decode(&generated::TICKET_EXAMPLE).expect("Lean Ticket"),
        release_receipt: ReleaseReceiptV1::decode(&generated::RECEIPT_EXAMPLE)
            .expect("Lean receipt"),
        request: RequestV1::decode(&generated::REQUEST_EXAMPLE).expect("Lean request"),
        limits: Limits {
            slot_limit: 1000,
            lamport_limit: 10_000,
            revision_limit: 10_000,
        },
    }
}

#[test]
fn lean_vectors_round_trip_exactly() {
    let value = fixture();
    assert_eq!(value.template.to_bytes(), Ok(generated::TEMPLATE_EXAMPLE));
    assert_eq!(value.series.to_bytes(), Ok(generated::SERIES_EXAMPLE));
    assert_eq!(value.ticket.to_bytes(), Ok(generated::TICKET_EXAMPLE));
    assert_eq!(
        value.release_receipt.to_bytes(),
        Ok(generated::RECEIPT_EXAMPLE)
    );
    assert_eq!(value.request.to_bytes(), Ok(generated::REQUEST_EXAMPLE));
}

#[test]
fn consume_emits_exact_compartments_and_complete_set_seed() {
    let input = fixture();
    let candidate = interpret(input).expect("due funded ticket");
    assert_eq!(candidate.series.phase, Phase::Active);
    assert_eq!(candidate.series.next_occurrence, 1);
    assert_eq!(candidate.series.revision, 1);
    assert_eq!(candidate.ticket.phase, TicketPhase::Consumed);
    assert_eq!(candidate.ticket.revision, 1);
    assert!(candidate.ticket.funds.is_zero());
    assert_eq!(candidate.transfer_count, 3);
    let principal = candidate.transfers[0].expect("principal");
    assert_eq!(principal.source.kind, AccountKind::TicketEscrow);
    assert_eq!(principal.destination.kind, AccountKind::MarketHoard);
    assert_eq!(principal.amount, 10);
    let rent = candidate.transfers[1].expect("rent");
    assert_eq!(rent.destination.kind, AccountKind::MarketAccount);
    assert_eq!(rent.amount, 20);
    let work = candidate.transfers[2].expect("work");
    assert_eq!(work.destination.kind, AccountKind::Beneficiary);
    assert_eq!(work.amount, 5);
    assert_eq!(candidate.transfers[3], None);
    let founding = candidate.market.expect("founding");
    assert_eq!(founding.market_id, input.ticket.committed_market_id);
    assert_eq!(founding.scheduled_slot, 100);
    assert_eq!(founding.complete_set_seed.outcome_count, 3);
    assert_eq!(founding.complete_set_seed.quantity, 10);
    assert_eq!(founding.complete_set_seed.founder, input.ticket.founder);
}

#[test]
fn replay_after_success_refuses_without_a_parallel_path() {
    let input = fixture();
    let candidate = interpret(input).expect("first consume");
    let replay = InvocationV1 {
        series: candidate.series,
        ticket: candidate.ticket,
        request: RequestV1 {
            expected_series_revision: candidate.series.revision,
            expected_ticket_revision: candidate.ticket.revision,
            ..input.request
        },
        ..input
    };
    assert_eq!(interpret(replay), Err(Error::InvalidState));
}

#[test]
fn early_late_and_stale_revisions_refuse() {
    let input = fixture();
    assert_eq!(
        interpret(InvocationV1 {
            request: RequestV1 {
                now_slot: 99,
                ..input.request
            },
            ..input
        }),
        Err(Error::ScheduleRefusal)
    );
    assert_eq!(
        interpret(InvocationV1 {
            request: RequestV1 {
                now_slot: 106,
                ..input.request
            },
            ..input
        }),
        Err(Error::ScheduleRefusal)
    );
    assert_eq!(
        interpret(InvocationV1 {
            request: RequestV1 {
                expected_ticket_revision: 1,
                ..input.request
            },
            ..input
        }),
        Err(Error::RevisionMismatch)
    );
}

#[test]
fn exact_funding_refuses_both_under_and_overpayment() {
    let input = fixture();
    let mut under = input.ticket;
    under.funds.hoard_principal -= 1;
    assert_eq!(
        interpret(InvocationV1 {
            ticket: under,
            ..input
        }),
        Err(Error::InvalidState)
    );
    let mut over = input.ticket;
    over.funds.market_rent += 1;
    assert_eq!(
        interpret(InvocationV1 {
            ticket: over,
            ..input
        }),
        Err(Error::InvalidState)
    );
}

#[test]
fn current_registry_receipt_is_mandatory_and_exact() {
    let input = fixture();
    let mut substituted = input.release_receipt;
    substituted.release_set_id[0] ^= 1;
    assert_eq!(
        interpret(InvocationV1 {
            release_receipt: substituted,
            ..input
        }),
        Err(Error::ReleaseAdmission)
    );
    let mut parallel_core = input.release_receipt;
    parallel_core.observed_program[0] ^= 1;
    assert_eq!(
        interpret(InvocationV1 {
            release_receipt: parallel_core,
            ..input
        }),
        Err(Error::ReleaseAdmission)
    );
    let mut flags = generated::RECEIPT_EXAMPLE;
    flags[generated::HEADER_RESERVED_OFFSET] = 1;
    assert_eq!(ReleaseReceiptV1::decode(&flags), Err(Error::UnknownTag));
}

#[test]
fn expiry_refunds_each_nonzero_compartment_and_advances() {
    let input = fixture();
    let candidate = interpret(InvocationV1 {
        request: RequestV1 {
            action: Action::Expire,
            now_slot: 106,
            work_recipient: [0; 32],
            ..input.request
        },
        ..input
    })
    .expect("past retry window");
    assert_eq!(candidate.series.next_occurrence, 1);
    assert_eq!(candidate.ticket.phase, TicketPhase::Expired);
    assert!(candidate.ticket.funds.is_zero());
    assert_eq!(candidate.transfer_count, 3);
    assert_eq!(candidate.transfers[0].expect("principal").amount, 10);
    assert_eq!(candidate.transfers[1].expect("rent").amount, 20);
    assert_eq!(candidate.transfers[2].expect("work").amount, 5);
    for transfer in candidate.transfers.into_iter().flatten() {
        assert_eq!(transfer.source.kind, AccountKind::TicketEscrow);
        assert_eq!(transfer.destination.kind, AccountKind::Beneficiary);
        assert_eq!(transfer.destination.identity, input.ticket.refund_owner);
    }
}

#[test]
fn last_occurrence_terminalizes_then_close_returns_only_close_rent() {
    let input = fixture();
    let last = InvocationV1 {
        series: SeriesStateV1 {
            next_occurrence: 2,
            revision: 2,
            ..input.series
        },
        ticket: TicketV1 {
            occurrence: 2,
            ticket_id: id(42),
            committed_market_id: id(43),
            ..input.ticket
        },
        request: RequestV1 {
            now_slot: 120,
            expected_series_revision: 2,
            ..input.request
        },
        ..input
    };
    let terminal = interpret(last).expect("last consume");
    assert_eq!(terminal.series.phase, Phase::Terminal);
    assert_eq!(terminal.series.next_occurrence, 3);
    let closed = interpret(InvocationV1 {
        series: terminal.series,
        ticket: terminal.ticket,
        request: RequestV1 {
            action: Action::Close,
            now_slot: 120,
            expected_series_revision: terminal.series.revision,
            expected_ticket_revision: terminal.ticket.revision,
            work_recipient: [0; 32],
        },
        ..last
    })
    .expect("terminal close");
    assert_eq!(closed.series.phase, Phase::Closed);
    assert_eq!(closed.series.close_rent_lamports, 0);
    assert_eq!(closed.ticket, terminal.ticket);
    assert_eq!(closed.transfer_count, 1);
    let rent = closed.transfers[0].expect("close rent");
    assert_eq!(rent.source.kind, AccountKind::SeriesEscrow);
    assert_eq!(rent.destination.kind, AccountKind::Beneficiary);
    assert_eq!(rent.amount, input.template.series_close_rent_lamports);
    assert_eq!(closed.market, None);
}

#[test]
fn hostile_wire_shapes_tags_and_reserved_bytes_refuse() {
    assert_eq!(
        TemplateV1::decode(&generated::TEMPLATE_EXAMPLE[..239]),
        Err(Error::InvalidLength)
    );
    let mut extended = [0_u8; 241];
    extended[..240].copy_from_slice(&generated::TEMPLATE_EXAMPLE);
    assert_eq!(TemplateV1::decode(&extended), Err(Error::InvalidLength));
    let mut magic = generated::TEMPLATE_EXAMPLE;
    magic[0] ^= 1;
    assert_eq!(TemplateV1::decode(&magic), Err(Error::InvalidMagic));
    let mut version = generated::TEMPLATE_EXAMPLE;
    version[generated::HEADER_VERSION_OFFSET] = 2;
    assert_eq!(TemplateV1::decode(&version), Err(Error::UnsupportedVersion));
    let mut reserved = generated::SERIES_EXAMPLE;
    reserved[generated::SERIES_RESERVED_BODY_OFFSET] = 1;
    assert_eq!(
        SeriesStateV1::decode(&reserved),
        Err(Error::NonCanonicalReserved)
    );
    let mut phase = generated::TICKET_EXAMPLE;
    phase[generated::HEADER_TAG_OFFSET] = 9;
    assert_eq!(TicketV1::decode(&phase), Err(Error::UnknownTag));
    let mut action = generated::REQUEST_EXAMPLE;
    action[generated::HEADER_TAG_OFFSET] = 9;
    assert_eq!(RequestV1::decode(&action), Err(Error::UnknownTag));
}

#[test]
fn overflow_and_named_profile_bounds_refuse() {
    let input = fixture();
    let overflow = InvocationV1 {
        template: TemplateV1 {
            first_occurrence_slot: u64::MAX - 2,
            period_slots: 10,
            ..input.template
        },
        ..input
    };
    assert_eq!(interpret(overflow), Err(Error::ArithmeticOverflow));
    assert_eq!(
        interpret(InvocationV1 {
            limits: Limits {
                slot_limit: 120,
                ..input.limits
            },
            ..input
        }),
        Err(Error::ProfileBound)
    );
}

fn id(value: u8) -> [u8; 32] {
    let mut identity = [0; 32];
    identity[0] = value;
    identity
}
