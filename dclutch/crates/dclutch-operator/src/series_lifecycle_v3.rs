//! One chain-derived answer to “what can this recurring Series do next?”
//!
//! The lower-level Series operator deliberately exposes five exact builders.
//! That is the right authority boundary for callers that already know the act,
//! but it is the wrong interface for a stranger-facing operator: asking the
//! caller to select `Consume` or `Expire` makes the client a second author of
//! the schedule, and asking it whether a terminal Ticket may retire makes it a
//! second author of replay state. This module derives those choices from one
//! same-slot snapshot and delegates every request byte and economic preflight
//! to the existing canonical builders.
//!
//! Missing chain evidence is an acquisition result, not a malformed request.
//! The caller can therefore say exactly which account or finalized record is
//! still needed without pretending that an unavailable act was refused by the
//! program. No RPC, signing, submission, or offchain workflow state lives here.

use dclutch_trading::series::{
    AccountKeyV3, TemplateV3, admit_occurrence, admit_ticket,
    replay::{SeriesPhaseV3, SeriesStateV3, TicketStateV3},
    terminal::SeriesLifecycleRentSinkV3,
};
use dclutch_trading_sbf::series::{
    instruction::SeriesActionV3,
    operator::{
        SeriesCloseSnapshotV3, SeriesOccurrenceSnapshotV3, SeriesOperatorErrorV3,
        SeriesRetireSnapshotV3, UnsignedSeriesActionV3, build_close_v3, build_consume_v3,
        build_expire_v3, build_prepare_v3, build_retire_v3,
    },
};

/// Finalized immutable records and mutable replay for the current occurrence.
#[derive(Clone, Copy)]
pub struct SeriesCurrentOccurrenceV3<'a> {
    /// Exact finalized realized occurrence bytes.
    pub occurrence_bytes: &'a [u8],
    /// Exact finalized immutable Ticket bytes.
    pub ticket_bytes: &'a [u8],
    /// Ordered occurrence-projection Merkle siblings.
    pub siblings: &'a [[u8; 32]],
    /// Current Ticket replay state, absent before Prepare.
    pub ticket_state: Option<TicketStateV3>,
}

/// One terminal Ticket candidate that may be retired independently.
#[derive(Clone, Copy)]
pub struct SeriesTerminalTicketV3<'a> {
    /// Exact finalized immutable Ticket bytes.
    pub ticket_bytes: &'a [u8],
    /// Current terminal Ticket replay state.
    pub ticket_state: TicketStateV3,
    /// Complete observed Ticket account lamports.
    pub observed_lamports: u64,
    /// Same-snapshot Rent minimum for the Ticket account.
    pub exact_rent: u64,
}

/// One same-slot snapshot sufficient to derive the next Series acts.
#[derive(Clone, Copy)]
pub struct SeriesLifecycleSnapshotV3<'a> {
    /// Exact finalized Template bytes selected by the immutable root.
    pub template_bytes: &'a [u8],
    /// Current Trading-owned Series root tail.
    pub series: SeriesStateV3,
    /// Current chain Clock slot.
    pub now_slot: u64,
    /// Current occurrence evidence, when it has been acquired.
    pub current: Option<SeriesCurrentOccurrenceV3<'a>>,
    /// Any one terminal Ticket selected for permissionless retirement.
    pub terminal_ticket: Option<SeriesTerminalTicketV3<'a>>,
    /// Complete observed composite-root lamports.
    pub observed_root_lamports: u64,
    /// Same-snapshot Rent minimum for the composite root.
    pub exact_root_rent: u64,
    /// Authenticated Market+generation lifecycle Rent destination, if acquired.
    pub rent_sink: Option<SeriesLifecycleRentSinkV3>,
}

/// Chain evidence the caller must acquire before an act can be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesAcquisitionV3 {
    /// Fetch the realized occurrence, immutable Ticket, and Merkle proof for this index.
    CurrentOccurrence {
        /// Exact occurrence index named by the root.
        occurrence: u32,
    },
    /// Fetch the current prepared Ticket replay account.
    CurrentTicketReplay,
    /// Discover any one terminal Ticket replay still counted by the root.
    TerminalTicket {
        /// Exact number of live Ticket replay accounts still counted by the root.
        outstanding: u32,
    },
    /// Authenticate the Market-generation lifecycle RentCredit destination.
    LifecycleRentCredit,
}

/// Economic consequence of one ready unsigned Series act.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsequenceV3 {
    /// Create and exactly prepay the current occurrence's unique Ticket.
    PrepareTicket,
    /// Consume the Ticket into its exact founded/open Market and terminalize replay.
    FoundOccurrenceMarket,
    /// Refund every expired compartment and terminalize the Ticket without a Market.
    ExpireAndRefund,
    /// Delete one terminal Ticket and credit its complete native balance to Rent V2.
    RetireTicket,
    /// Delete the exhausted Series root and credit every classified lamport to Rent V2.
    CloseRoot,
}

/// One exact unsigned act selected from authenticated state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedSeriesActV3 {
    action: SeriesActionV3,
    consequence: SeriesConsequenceV3,
    request: UnsignedSeriesActionV3,
}

impl PlannedSeriesActV3 {
    /// Exact action selected from the root, Ticket, and Clock snapshot.
    pub const fn action(self) -> SeriesActionV3 {
        self.action
    }

    /// Stable user-facing consequence category; request bytes remain authoritative.
    pub const fn consequence(self) -> SeriesConsequenceV3 {
        self.consequence
    }

    /// Exact unsigned family request constructed by the canonical Series builder.
    pub const fn request(self) -> UnsignedSeriesActionV3 {
        self.request
    }
}

/// Status of the act that advances the current occurrence or terminal root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesNextActV3 {
    /// Every required fact is present and the unsigned request is ready.
    Ready(PlannedSeriesActV3),
    /// A prepared Ticket exists, but its occurrence has not reached its scheduled slot.
    WaitUntil {
        /// First slot at which Consume may succeed.
        scheduled_slot: u64,
    },
    /// More chain evidence is required before selecting or constructing the act.
    Acquire(SeriesAcquisitionV3),
}

/// Same-snapshot Series lifecycle report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleReportV3 {
    phase: SeriesPhaseV3,
    next_occurrence: u32,
    outstanding_tickets: u32,
    next: SeriesNextActV3,
    housekeeping: Option<SeriesNextActV3>,
}

impl SeriesLifecycleReportV3 {
    /// Authenticated root lifecycle phase.
    pub const fn phase(self) -> SeriesPhaseV3 {
        self.phase
    }

    /// Root-owned next occurrence index.
    pub const fn next_occurrence(self) -> u32 {
        self.next_occurrence
    }

    /// Root-owned count of live Ticket replay accounts.
    pub const fn outstanding_tickets(self) -> u32 {
        self.outstanding_tickets
    }

    /// Act that advances the current occurrence or closes the terminal root.
    pub const fn next(self) -> SeriesNextActV3 {
        self.next
    }

    /// Optional retirement of a prior terminal Ticket while the Series stays active.
    pub const fn housekeeping(self) -> Option<SeriesNextActV3> {
        self.housekeeping
    }
}

/// Derive the current Series acts from one same-slot snapshot.
///
/// The function accepts no action selector. Schedule and replay state select
/// Prepare/Consume/Expire/Retire/Close, and every ready request is rebuilt by
/// the lower-level chain-derived operator before it is returned.
pub fn inspect_series_lifecycle_v3(
    snapshot: SeriesLifecycleSnapshotV3<'_>,
) -> Result<SeriesLifecycleReportV3, SeriesOperatorErrorV3> {
    let template =
        TemplateV3::decode(snapshot.template_bytes).map_err(|_| SeriesOperatorErrorV3::Content)?;
    snapshot
        .series
        .encode(template.occurrence_count())
        .map_err(|_| SeriesOperatorErrorV3::Replay)?;

    let phase = snapshot.series.phase();
    let next_occurrence = snapshot.series.next_occurrence();
    let outstanding_tickets = snapshot.series.outstanding_ticket_accounts();
    let current_ticket_count = u32::from(snapshot.series.current_ticket_prepared());
    let terminal_ticket_count = outstanding_tickets
        .checked_sub(current_ticket_count)
        .ok_or(SeriesOperatorErrorV3::Replay)?;
    let housekeeping = if phase == SeriesPhaseV3::Active && terminal_ticket_count > 0 {
        Some(plan_housekeeping_v3(snapshot, terminal_ticket_count)?)
    } else {
        None
    };
    let next = match phase {
        SeriesPhaseV3::Active => plan_active_v3(snapshot, template)?,
        SeriesPhaseV3::Terminal if outstanding_tickets > 0 => {
            plan_housekeeping_v3(snapshot, outstanding_tickets)?
        }
        SeriesPhaseV3::Terminal => plan_close_v3(snapshot)?,
    };
    Ok(SeriesLifecycleReportV3 {
        phase,
        next_occurrence,
        outstanding_tickets,
        next,
        housekeeping,
    })
}

fn plan_active_v3(
    snapshot: SeriesLifecycleSnapshotV3<'_>,
    template: TemplateV3,
) -> Result<SeriesNextActV3, SeriesOperatorErrorV3> {
    let Some(current) = snapshot.current else {
        return Ok(SeriesNextActV3::Acquire(
            SeriesAcquisitionV3::CurrentOccurrence {
                occurrence: snapshot.series.next_occurrence(),
            },
        ));
    };
    let occurrence = admit_occurrence(
        snapshot.template_bytes,
        current.occurrence_bytes,
        current.siblings,
    )
    .map_err(|_| SeriesOperatorErrorV3::Content)?;
    let ticket = admit_ticket(current.ticket_bytes).map_err(|_| SeriesOperatorErrorV3::Content)?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| SeriesOperatorErrorV3::Content)?;
    if occurrence.occurrence().occurrence() != snapshot.series.next_occurrence() {
        return Err(SeriesOperatorErrorV3::Replay);
    }
    let occurrence_snapshot = SeriesOccurrenceSnapshotV3 {
        template_bytes: snapshot.template_bytes,
        occurrence_bytes: current.occurrence_bytes,
        ticket_bytes: current.ticket_bytes,
        siblings: current.siblings,
        series: snapshot.series,
        ticket_state: current.ticket_state,
        now_slot: snapshot.now_slot,
    };
    if !snapshot.series.current_ticket_prepared() {
        if current.ticket_state.is_some() {
            return Err(SeriesOperatorErrorV3::Replay);
        }
        return ready(
            SeriesActionV3::Prepare,
            SeriesConsequenceV3::PrepareTicket,
            build_prepare_v3(occurrence_snapshot)?,
        );
    }
    let ticket_state = current.ticket_state.ok_or(SeriesOperatorErrorV3::Replay)?;
    if ticket_state.ticket_record_id() != ticket.content_id() || ticket_state.phase().terminal() {
        return Err(SeriesOperatorErrorV3::Replay);
    }
    let scheduled = occurrence.occurrence().scheduled_slot();
    let retry = template
        .retry_through(occurrence.occurrence().occurrence())
        .map_err(|_| SeriesOperatorErrorV3::Content)?;
    if snapshot.now_slot < scheduled {
        // Validate the complete replay/content join at the first legal slot so
        // `WaitUntil` never masks a substituted Ticket or stale revision.
        build_consume_v3(SeriesOccurrenceSnapshotV3 {
            now_slot: scheduled,
            ..occurrence_snapshot
        })?;
        return Ok(SeriesNextActV3::WaitUntil {
            scheduled_slot: scheduled,
        });
    }
    if snapshot.now_slot <= retry {
        ready(
            SeriesActionV3::Consume,
            SeriesConsequenceV3::FoundOccurrenceMarket,
            build_consume_v3(occurrence_snapshot)?,
        )
    } else {
        ready(
            SeriesActionV3::Expire,
            SeriesConsequenceV3::ExpireAndRefund,
            build_expire_v3(occurrence_snapshot)?,
        )
    }
}

fn plan_housekeeping_v3(
    snapshot: SeriesLifecycleSnapshotV3<'_>,
    terminal_ticket_count: u32,
) -> Result<SeriesNextActV3, SeriesOperatorErrorV3> {
    let Some(ticket) = snapshot.terminal_ticket else {
        return Ok(SeriesNextActV3::Acquire(
            SeriesAcquisitionV3::TerminalTicket {
                outstanding: terminal_ticket_count,
            },
        ));
    };
    let Some(rent_sink) = snapshot.rent_sink else {
        return Ok(SeriesNextActV3::Acquire(
            SeriesAcquisitionV3::LifecycleRentCredit,
        ));
    };
    ready(
        SeriesActionV3::Retire,
        SeriesConsequenceV3::RetireTicket,
        build_retire_v3(SeriesRetireSnapshotV3 {
            template_bytes: snapshot.template_bytes,
            ticket_bytes: ticket.ticket_bytes,
            series: snapshot.series,
            ticket_state: ticket.ticket_state,
            observed_ticket_lamports: ticket.observed_lamports,
            exact_ticket_rent: ticket.exact_rent,
            rent_sink,
        })?,
    )
}

fn plan_close_v3(
    snapshot: SeriesLifecycleSnapshotV3<'_>,
) -> Result<SeriesNextActV3, SeriesOperatorErrorV3> {
    let Some(rent_sink) = snapshot.rent_sink else {
        return Ok(SeriesNextActV3::Acquire(
            SeriesAcquisitionV3::LifecycleRentCredit,
        ));
    };
    ready(
        SeriesActionV3::Close,
        SeriesConsequenceV3::CloseRoot,
        build_close_v3(SeriesCloseSnapshotV3 {
            template_bytes: snapshot.template_bytes,
            series: snapshot.series,
            observed_root_lamports: snapshot.observed_root_lamports,
            exact_root_rent: snapshot.exact_root_rent,
            rent_sink,
        })?,
    )
}

fn ready(
    action: SeriesActionV3,
    consequence: SeriesConsequenceV3,
    request: UnsignedSeriesActionV3,
) -> Result<SeriesNextActV3, SeriesOperatorErrorV3> {
    if request.decode()?.action() != action {
        return Err(SeriesOperatorErrorV3::Content);
    }
    Ok(SeriesNextActV3::Ready(PlannedSeriesActV3 {
        action,
        consequence,
        request,
    }))
}

/// Convert a nonzero account identity for callers that already hostile-decoded a key.
///
/// This tiny helper keeps client adapters from reaching for unchecked casts
/// when turning an authenticated Solana key into the SDK-free Series type.
pub fn series_account_key_v3(bytes: [u8; 32]) -> Result<AccountKeyV3, SeriesOperatorErrorV3> {
    AccountKeyV3::new(bytes).map_err(|_| SeriesOperatorErrorV3::Content)
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_market::rent::{
        RefundAuthority,
        lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
    };
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, generated,
        occurrence_content_id, template_content_id,
    };
    use solana_program::hash::hashv;

    use super::*;

    const HASH_SEPARATOR: [u8; 1] = [0];

    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
        bytes[offset..offset + N].copy_from_slice(value);
    }

    fn projection_root(
        occurrence_id: ContentId,
        mut index: u32,
        siblings: &[[u8; 32]],
    ) -> [u8; 32] {
        let mut node = occurrence_id.to_bytes();
        for sibling in siblings {
            node = if index & 1 == 0 {
                hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &HASH_SEPARATOR,
                    &node,
                    sibling,
                ])
                .to_bytes()
            } else {
                hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &HASH_SEPARATOR,
                    sibling,
                    &node,
                ])
                .to_bytes()
            };
            index >>= 1;
        }
        node
    }

    struct Fixture {
        template: [u8; SERIES_TEMPLATE_BYTES_V3],
        occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3],
        ticket: [u8; SERIES_TICKET_BYTES_V3],
        siblings: [[u8; 32]; 2],
        ticket_id: ContentId,
        scheduled: u64,
        retry: u64,
    }

    impl Fixture {
        fn new() -> Self {
            let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
            let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
            let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
            let siblings = [[90; 32], [91; 32]];
            let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
            put(
                &mut template,
                generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
                &projection_root(occurrence_id, 1, &siblings),
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
            let ticket_id = admit_ticket(&ticket).expect("Ticket").content_id();
            let occurrence =
                admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
            let scheduled = occurrence.occurrence().scheduled_slot();
            let retry = occurrence
                .template()
                .retry_through(occurrence.occurrence().occurrence())
                .expect("retry");
            Self {
                template,
                occurrence: generated::SERIES_EXAMPLE_OCCURRENCE_V3,
                ticket,
                siblings,
                ticket_id,
                scheduled,
                retry,
            }
        }

        fn current(&self, ticket_state: Option<TicketStateV3>) -> SeriesCurrentOccurrenceV3<'_> {
            SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &self.occurrence,
                ticket_bytes: &self.ticket,
                siblings: &self.siblings,
                ticket_state,
            }
        }

        fn snapshot<'a>(
            &'a self,
            series: SeriesStateV3,
            now_slot: u64,
            current: Option<SeriesCurrentOccurrenceV3<'a>>,
        ) -> SeriesLifecycleSnapshotV3<'a> {
            SeriesLifecycleSnapshotV3 {
                template_bytes: &self.template,
                series,
                now_slot,
                current,
                terminal_ticket: None,
                observed_root_lamports: 20,
                exact_root_rent: 10,
                rent_sink: None,
            }
        }
    }

    fn rent_sink(refund_wallet: AccountKeyV3) -> SeriesLifecycleRentSinkV3 {
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(refund_wallet.to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new([71; 32]).expect("Market"),
            LifecycleAccountIdV2::new([72; 32]).expect("release"),
            3,
            4,
        )
        .expect("Rent V2");
        SeriesLifecycleRentSinkV3::admit(
            AccountKeyV3::new([73; 32]).expect("credit"),
            &credit.to_bytes(),
            AccountKeyV3::new([71; 32]).expect("Market"),
            ContentId::new([72; 32]).expect("release"),
            3,
            refund_wallet,
        )
        .expect("sink")
    }

    fn ready_action(status: SeriesNextActV3) -> SeriesActionV3 {
        match status {
            SeriesNextActV3::Ready(plan) => plan.action(),
            other => panic!("expected ready act, got {other:?}"),
        }
    }

    #[test]
    fn active_series_derives_prepare_wait_consume_and_expire_without_an_action_input() {
        let fixture = Fixture::new();
        let initial = SeriesStateV3::new(7);
        let missing = inspect_series_lifecycle_v3(fixture.snapshot(initial, 0, None))
            .expect("missing evidence report");
        assert_eq!(
            missing.next(),
            SeriesNextActV3::Acquire(SeriesAcquisitionV3::CurrentOccurrence { occurrence: 0 })
        );

        // The generated occurrence is index one, so bring the root to the same
        // authenticated index before asking for its act.
        let at_one = initial
            .prepare_ticket(0)
            .expect("prepare zero")
            .settle_current(1, 3)
            .expect("settle zero")
            .retire_ticket(2)
            .expect("retire zero");
        let prepare = inspect_series_lifecycle_v3(fixture.snapshot(
            at_one,
            fixture.scheduled - 1,
            Some(fixture.current(None)),
        ))
        .expect("prepare report");
        assert_eq!(ready_action(prepare.next()), SeriesActionV3::Prepare);

        let prepared = at_one.prepare_ticket(at_one.revision()).expect("prepared");
        let ticket = TicketStateV3::prepared(fixture.ticket_id);
        let waiting = inspect_series_lifecycle_v3(fixture.snapshot(
            prepared,
            fixture.scheduled - 1,
            Some(fixture.current(Some(ticket))),
        ))
        .expect("wait report");
        assert_eq!(
            waiting.next(),
            SeriesNextActV3::WaitUntil {
                scheduled_slot: fixture.scheduled
            }
        );
        assert_eq!(waiting.housekeeping(), None);
        let consume = inspect_series_lifecycle_v3(fixture.snapshot(
            prepared,
            fixture.scheduled,
            Some(fixture.current(Some(ticket))),
        ))
        .expect("consume report");
        assert_eq!(ready_action(consume.next()), SeriesActionV3::Consume);
        let expire = inspect_series_lifecycle_v3(fixture.snapshot(
            prepared,
            fixture.retry + 1,
            Some(fixture.current(Some(ticket))),
        ))
        .expect("expire report");
        assert_eq!(ready_action(expire.next()), SeriesActionV3::Expire);
    }

    #[test]
    fn a_prior_terminal_ticket_is_housekeeping_not_the_current_occurrence() {
        let fixture = Fixture::new();
        let with_prior_terminal = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare zero")
            .settle_current(1, 3)
            .expect("settle zero");
        let report = inspect_series_lifecycle_v3(fixture.snapshot(
            with_prior_terminal,
            fixture.scheduled - 1,
            Some(fixture.current(None)),
        ))
        .expect("current occurrence plus prior terminal report");
        assert_eq!(ready_action(report.next()), SeriesActionV3::Prepare);
        assert_eq!(
            report.housekeeping(),
            Some(SeriesNextActV3::Acquire(
                SeriesAcquisitionV3::TerminalTicket { outstanding: 1 }
            ))
        );
    }

    #[test]
    fn terminal_series_names_missing_ticket_then_builds_retire_and_close() {
        let fixture = Fixture::new();
        let template = TemplateV3::decode(&fixture.template).expect("Template");
        let mut terminal = SeriesStateV3::new(template.close_rent());
        for revision in [0_u64, 3, 6] {
            terminal = terminal
                .prepare_ticket(revision)
                .expect("prepare")
                .settle_current(revision + 1, template.occurrence_count())
                .expect("settle")
                .retire_ticket(revision + 2)
                .expect("retire");
        }
        // Restore one outstanding terminal Ticket while preserving the
        // terminal occurrence cursor.
        let with_ticket = terminal.prepare_ticket(terminal.revision()).err();
        assert!(
            with_ticket.is_some(),
            "terminal root cannot prepare another occurrence"
        );
        let before_last_retire = SeriesStateV3::new(template.close_rent())
            .prepare_ticket(0)
            .expect("prepare 0")
            .settle_current(1, 3)
            .expect("settle 0")
            .retire_ticket(2)
            .expect("retire 0")
            .prepare_ticket(3)
            .expect("prepare 1")
            .settle_current(4, 3)
            .expect("settle 1")
            .retire_ticket(5)
            .expect("retire 1")
            .prepare_ticket(6)
            .expect("prepare 2")
            .settle_current(7, 3)
            .expect("settle 2");
        let missing =
            inspect_series_lifecycle_v3(fixture.snapshot(before_last_retire, fixture.retry, None))
                .expect("missing Ticket report");
        assert_eq!(
            missing.next(),
            SeriesNextActV3::Acquire(SeriesAcquisitionV3::TerminalTicket { outstanding: 1 })
        );

        let ticket_state = TicketStateV3::prepared(fixture.ticket_id)
            .settle(0, dclutch_trading::series::replay::TicketPhaseV3::Consumed)
            .expect("terminal Ticket");
        let refund = admit_ticket(&fixture.ticket)
            .expect("Ticket")
            .ticket()
            .refund_owner();
        let mut retire_snapshot = fixture.snapshot(before_last_retire, fixture.retry, None);
        retire_snapshot.terminal_ticket = Some(SeriesTerminalTicketV3 {
            ticket_bytes: &fixture.ticket,
            ticket_state,
            observed_lamports: 11,
            exact_rent: 10,
        });
        retire_snapshot.rent_sink = Some(rent_sink(refund));
        let retire = inspect_series_lifecycle_v3(retire_snapshot).expect("retire report");
        assert_eq!(ready_action(retire.next()), SeriesActionV3::Retire);

        let mut close_snapshot = fixture.snapshot(terminal, fixture.retry, None);
        close_snapshot.rent_sink = Some(rent_sink(
            TemplateV3::decode(&fixture.template)
                .expect("Template")
                .refund_owner(),
        ));
        let close = inspect_series_lifecycle_v3(close_snapshot).expect("close report");
        assert_eq!(ready_action(close.next()), SeriesActionV3::Close);
    }

    #[test]
    fn waiting_does_not_hide_a_substituted_ticket_or_stale_revision() {
        let fixture = Fixture::new();
        let at_one = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare zero")
            .settle_current(1, 3)
            .expect("settle zero")
            .retire_ticket(2)
            .expect("retire zero");
        let prepared = at_one.prepare_ticket(at_one.revision()).expect("prepared");
        let wrong = TicketStateV3::prepared(ContentId::new([99; 32]).expect("wrong Ticket"));
        assert_eq!(
            inspect_series_lifecycle_v3(fixture.snapshot(
                prepared,
                fixture.scheduled - 1,
                Some(fixture.current(Some(wrong))),
            )),
            Err(SeriesOperatorErrorV3::Replay)
        );
    }
}
