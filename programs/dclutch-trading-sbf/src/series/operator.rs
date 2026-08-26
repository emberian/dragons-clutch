//! Chain-derived, unsigned recurring-Series V3 request construction.
//!
//! The builder re-decodes the finalized immutable records and mutable replay
//! bytes supplied by a client snapshot. User input selects only an action and
//! current Clock slot; content identities and optimistic revisions are copied
//! from authenticated state. This module signs and submits nothing.

use dclutch_core_contract::ContentId;
use dclutch_series_v3_kernel::plan::{SeriesReplayActionV3, evaluate_replay_v3};

use super::{
    AdmittedOccurrenceV3, AdmittedTicketV3, SeriesV3Error, TemplateV3, admit_occurrence,
    admit_ticket,
    instruction::{
        SERIES_ACTION_HEADER_BYTES_V3, SERIES_ACTION_MAXIMUM_BYTES_V3,
        SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3, SeriesActionRequestV3, SeriesActionV3,
        encode_series_action_header_v3,
    },
    state::{SeriesStateV3, TicketStateV3},
    template_content_id,
};

/// Refusal from unsigned Series V3 construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesOperatorErrorV3 {
    /// Immutable Template, occurrence, Ticket, or proof bytes refused.
    Content,
    /// Mutable root/Ticket replay bytes or optimistic coordinates refused.
    Replay,
    /// The selected action was outside its exact schedule window.
    Schedule,
    /// The requested packet exceeded the mathematical u32 Merkle bound.
    Proof,
}

impl From<SeriesV3Error> for SeriesOperatorErrorV3 {
    fn from(_: SeriesV3Error) -> Self {
        Self::Content
    }
}

/// Exact unsigned family request passed to the canonical Trading hot outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedSeriesActionV3 {
    bytes: [u8; SERIES_ACTION_MAXIMUM_BYTES_V3],
    len: u16,
}

impl UnsignedSeriesActionV3 {
    /// Borrow the exact initialized request bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// Hostile-decode the constructed request for independent inspection.
    pub fn decode(&self) -> Result<SeriesActionRequestV3<'_>, SeriesOperatorErrorV3> {
        SeriesActionRequestV3::decode(self.as_bytes()).map_err(|_| SeriesOperatorErrorV3::Content)
    }
}

/// Same-snapshot finalized records and mutable state for an occurrence action.
#[derive(Clone, Copy)]
pub struct SeriesOccurrenceSnapshotV3<'a> {
    /// Exact finalized Template bytes.
    pub template_bytes: &'a [u8],
    /// Exact finalized realized occurrence bytes.
    pub occurrence_bytes: &'a [u8],
    /// Exact finalized immutable Ticket bytes.
    pub ticket_bytes: &'a [u8],
    /// Ordered occurrence-projection Merkle siblings.
    pub siblings: &'a [[u8; 32]],
    /// Current Trading-owned mutable Series tail.
    pub series: SeriesStateV3,
    /// Current Ticket replay state; absent only before Prepare.
    pub ticket_state: Option<TicketStateV3>,
    /// Current chain Clock slot.
    pub now_slot: u64,
}

impl SeriesOccurrenceSnapshotV3<'_> {
    fn admit(self) -> Result<(AdmittedOccurrenceV3, AdmittedTicketV3), SeriesOperatorErrorV3> {
        let occurrence =
            admit_occurrence(self.template_bytes, self.occurrence_bytes, self.siblings)?;
        let ticket = admit_ticket(self.ticket_bytes)?;
        occurrence.require_ticket(ticket.ticket())?;
        if self.series.next_occurrence() != occurrence.occurrence().occurrence() {
            return Err(SeriesOperatorErrorV3::Replay);
        }
        Ok((occurrence, ticket))
    }
}

/// Same-snapshot finalized Template/Ticket and replay state for Ticket retire.
#[derive(Clone, Copy)]
pub struct SeriesRetireSnapshotV3<'a> {
    /// Exact finalized Template bytes.
    pub template_bytes: &'a [u8],
    /// Exact finalized immutable Ticket bytes.
    pub ticket_bytes: &'a [u8],
    /// Current Trading-owned mutable Series tail.
    pub series: SeriesStateV3,
    /// Current terminal Ticket replay state.
    pub ticket_state: TicketStateV3,
}

/// Same-snapshot finalized Template and mutable root state for terminal close.
#[derive(Clone, Copy)]
pub struct SeriesCloseSnapshotV3<'a> {
    /// Exact finalized Template bytes.
    pub template_bytes: &'a [u8],
    /// Current Trading-owned mutable Series tail.
    pub series: SeriesStateV3,
}

/// Construct a dust-tolerant pre-founding Ticket preparation request.
pub fn build_prepare_v3(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    let (occurrence, ticket) = snapshot.admit()?;
    if snapshot.ticket_state.is_some()
        || snapshot.now_slot
            > occurrence
                .template()
                .retry_through(occurrence.occurrence().occurrence())?
    {
        return Err(SeriesOperatorErrorV3::Schedule);
    }
    preflight_replay(
        occurrence.template(),
        snapshot.series,
        None,
        SeriesReplayActionV3::Prepare {
            ticket_record: ticket.content_id(),
        },
    )?;
    build_occurrence(
        SeriesActionV3::Prepare,
        occurrence,
        ticket,
        snapshot.series.revision(),
        0,
        snapshot.siblings,
    )
}

/// Construct the atomic prepared-Ticket to Found-Market Consume request.
pub fn build_consume_v3(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    let (occurrence, ticket) = snapshot.admit()?;
    let ticket_state = require_ticket_state(ticket, snapshot.ticket_state)?;
    let scheduled = occurrence.occurrence().scheduled_slot();
    let retry = occurrence
        .template()
        .retry_through(occurrence.occurrence().occurrence())?;
    if snapshot.now_slot < scheduled || snapshot.now_slot > retry {
        return Err(SeriesOperatorErrorV3::Schedule);
    }
    preflight_replay(
        occurrence.template(),
        snapshot.series,
        Some(ticket_state),
        SeriesReplayActionV3::Consume {
            ticket_record: ticket.content_id(),
            expected_ticket_revision: ticket_state.revision(),
        },
    )?;
    build_occurrence(
        SeriesActionV3::Consume,
        occurrence,
        ticket,
        snapshot.series.revision(),
        ticket_state.revision(),
        snapshot.siblings,
    )
}

/// Construct an exact Ticket refund request after the retry deadline.
pub fn build_expire_v3(
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    let (occurrence, ticket) = snapshot.admit()?;
    let ticket_state = require_ticket_state(ticket, snapshot.ticket_state)?;
    if snapshot.now_slot
        <= occurrence
            .template()
            .retry_through(occurrence.occurrence().occurrence())?
    {
        return Err(SeriesOperatorErrorV3::Schedule);
    }
    preflight_replay(
        occurrence.template(),
        snapshot.series,
        Some(ticket_state),
        SeriesReplayActionV3::Expire {
            ticket_record: ticket.content_id(),
            expected_ticket_revision: ticket_state.revision(),
        },
    )?;
    build_occurrence(
        SeriesActionV3::Expire,
        occurrence,
        ticket,
        snapshot.series.revision(),
        ticket_state.revision(),
        snapshot.siblings,
    )
}

/// Construct deletion of one already terminal Ticket replay account.
pub fn build_retire_v3(
    snapshot: SeriesRetireSnapshotV3<'_>,
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    let template_id = template_content_id(snapshot.template_bytes)?;
    let ticket = admit_ticket(snapshot.ticket_bytes)?;
    if ticket.ticket().template() != template_id
        || ticket.content_id() != snapshot.ticket_state.ticket_record_id()
        || !snapshot.ticket_state.phase().terminal()
    {
        return Err(SeriesOperatorErrorV3::Replay);
    }
    let template = TemplateV3::decode(snapshot.template_bytes)?;
    preflight_replay(
        template,
        snapshot.series,
        Some(snapshot.ticket_state),
        SeriesReplayActionV3::Retire {
            ticket_record: ticket.content_id(),
            expected_ticket_revision: snapshot.ticket_state.revision(),
        },
    )?;
    build(
        SeriesActionV3::Retire,
        template_id,
        None,
        Some(ticket.content_id()),
        snapshot.series.revision(),
        snapshot.ticket_state.revision(),
        &[],
    )
}

/// Construct terminal Series-root deletion after every Ticket is retired.
pub fn build_close_v3(
    snapshot: SeriesCloseSnapshotV3<'_>,
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    let template_id = template_content_id(snapshot.template_bytes)?;
    let template = TemplateV3::decode(snapshot.template_bytes)?;
    preflight_replay(template, snapshot.series, None, SeriesReplayActionV3::Close)?;
    build(
        SeriesActionV3::Close,
        template_id,
        None,
        None,
        snapshot.series.revision(),
        0,
        &[],
    )
}

fn require_ticket_state(
    ticket: AdmittedTicketV3,
    state: Option<TicketStateV3>,
) -> Result<TicketStateV3, SeriesOperatorErrorV3> {
    let state = state.ok_or(SeriesOperatorErrorV3::Replay)?;
    if state.ticket_record_id() != ticket.content_id() || state.phase().terminal() {
        return Err(SeriesOperatorErrorV3::Replay);
    }
    Ok(state)
}

fn preflight_replay(
    template: TemplateV3,
    series: SeriesStateV3,
    ticket: Option<TicketStateV3>,
    action: SeriesReplayActionV3,
) -> Result<(), SeriesOperatorErrorV3> {
    let occurrence_count = template.occurrence_count();
    let series_bytes = series
        .encode(occurrence_count)
        .map_err(|_| SeriesOperatorErrorV3::Replay)?;
    let ticket_bytes = ticket.map(TicketStateV3::encode);
    evaluate_replay_v3(
        action,
        occurrence_count,
        series.revision(),
        &series_bytes,
        ticket_bytes.as_ref().map(<[u8; 64]>::as_slice),
    )
    .map_err(|_| SeriesOperatorErrorV3::Replay)?;
    Ok(())
}

fn build_occurrence(
    action: SeriesActionV3,
    occurrence: AdmittedOccurrenceV3,
    ticket: AdmittedTicketV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    siblings: &[[u8; 32]],
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    build(
        action,
        occurrence.template_id(),
        Some(occurrence.occurrence_id()),
        Some(ticket.content_id()),
        expected_series_revision,
        expected_ticket_revision,
        siblings,
    )
}

#[allow(clippy::too_many_arguments)]
fn build(
    action: SeriesActionV3,
    template: ContentId,
    occurrence: Option<ContentId>,
    ticket: Option<ContentId>,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    siblings: &[[u8; 32]],
) -> Result<UnsignedSeriesActionV3, SeriesOperatorErrorV3> {
    if siblings.len() > SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3 {
        return Err(SeriesOperatorErrorV3::Proof);
    }
    let proof_count = u8::try_from(siblings.len()).map_err(|_| SeriesOperatorErrorV3::Proof)?;
    let header = encode_series_action_header_v3(
        action,
        template,
        occurrence,
        ticket,
        expected_series_revision,
        expected_ticket_revision,
        proof_count,
    )
    .map_err(|_| SeriesOperatorErrorV3::Content)?;
    let mut bytes = [0_u8; SERIES_ACTION_MAXIMUM_BYTES_V3];
    bytes
        .get_mut(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesOperatorErrorV3::Proof)?
        .copy_from_slice(&header);
    for (index, sibling) in siblings.iter().enumerate() {
        let start = SERIES_ACTION_HEADER_BYTES_V3
            .checked_add(index.checked_mul(32).ok_or(SeriesOperatorErrorV3::Proof)?)
            .ok_or(SeriesOperatorErrorV3::Proof)?;
        let end = start.checked_add(32).ok_or(SeriesOperatorErrorV3::Proof)?;
        bytes
            .get_mut(start..end)
            .ok_or(SeriesOperatorErrorV3::Proof)?
            .copy_from_slice(sibling);
    }
    let len = SERIES_ACTION_HEADER_BYTES_V3
        .checked_add(
            siblings
                .len()
                .checked_mul(32)
                .ok_or(SeriesOperatorErrorV3::Proof)?,
        )
        .ok_or(SeriesOperatorErrorV3::Proof)?;
    let len = u16::try_from(len).map_err(|_| SeriesOperatorErrorV3::Proof)?;
    let request = UnsignedSeriesActionV3 { bytes, len };
    request.decode()?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use solana_program::hash::hashv;

    use super::*;
    use crate::series::{
        SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, generated,
        occurrence_content_id,
        state::{TicketPhaseV3, TicketStateV3},
    };

    const HASH_SEPARATOR: [u8; 1] = [0];

    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
        bytes
            .get_mut(offset..offset + N)
            .expect("fixture field")
            .copy_from_slice(value);
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

    #[derive(Clone)]
    struct Fixture {
        template: [u8; SERIES_TEMPLATE_BYTES_V3],
        occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3],
        ticket: [u8; SERIES_TICKET_BYTES_V3],
        siblings: [[u8; 32]; 2],
        series_at_one: SeriesStateV3,
        ticket_state: TicketStateV3,
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
            let ticket_id = admit_ticket(&ticket).expect("Ticket ID").content_id();
            let series_at_one = SeriesStateV3::new(7)
                .prepare_ticket(0)
                .expect("prepare occurrence zero")
                .settle_current(1, 3)
                .expect("settle occurrence zero")
                .retire_ticket(2)
                .expect("retire occurrence zero");
            Self {
                template,
                occurrence,
                ticket,
                siblings,
                series_at_one,
                ticket_state: TicketStateV3::prepared(ticket_id),
            }
        }

        fn occurrence_snapshot(
            &self,
            now_slot: u64,
            ticket_state: Option<TicketStateV3>,
        ) -> SeriesOccurrenceSnapshotV3<'_> {
            self.occurrence_snapshot_with_series(now_slot, ticket_state, self.series_at_one)
        }

        fn occurrence_snapshot_with_series(
            &self,
            now_slot: u64,
            ticket_state: Option<TicketStateV3>,
            series: SeriesStateV3,
        ) -> SeriesOccurrenceSnapshotV3<'_> {
            SeriesOccurrenceSnapshotV3 {
                template_bytes: &self.template,
                occurrence_bytes: &self.occurrence,
                ticket_bytes: &self.ticket,
                siblings: &self.siblings,
                series,
                ticket_state,
                now_slot,
            }
        }
    }

    #[test]
    fn occurrence_builders_copy_chain_ids_revisions_and_exact_proof() {
        let fixture = Fixture::new();
        let prepare =
            build_prepare_v3(fixture.occurrence_snapshot(100, None)).expect("Prepare request");
        let prepare = prepare.decode().expect("Prepare decode");
        assert_eq!(prepare.action(), SeriesActionV3::Prepare);
        assert_eq!(prepare.expected_series_revision(), 3);
        assert_eq!(prepare.expected_ticket_revision(), 0);
        assert_eq!(prepare.proof_count(), 2);

        let prepared_series = fixture
            .series_at_one
            .prepare_ticket(fixture.series_at_one.revision())
            .expect("prepared root");
        let consume = build_consume_v3(fixture.occurrence_snapshot_with_series(
            110,
            Some(fixture.ticket_state),
            prepared_series,
        ))
        .expect("Consume request");
        let consume = consume.decode().expect("Consume decode");
        assert_eq!(consume.action(), SeriesActionV3::Consume);
        assert_eq!(consume.expected_series_revision(), 4);
        assert_eq!(consume.expected_ticket_revision(), 0);
        assert_eq!(consume.occurrence(), prepare.occurrence());
        assert_eq!(consume.ticket(), prepare.ticket());

        let expire = build_expire_v3(fixture.occurrence_snapshot_with_series(
            116,
            Some(fixture.ticket_state),
            prepared_series,
        ))
        .expect("Expire request");
        assert_eq!(
            expire.decode().expect("Expire decode").action(),
            SeriesActionV3::Expire
        );
    }

    #[test]
    fn stale_replay_wrong_window_and_substituted_proof_refuse() {
        let fixture = Fixture::new();
        assert_eq!(
            build_prepare_v3(fixture.occurrence_snapshot(100, Some(fixture.ticket_state))),
            Err(SeriesOperatorErrorV3::Schedule)
        );
        assert_eq!(
            build_consume_v3(
                fixture.occurrence_snapshot_with_series(
                    109,
                    Some(fixture.ticket_state),
                    fixture
                        .series_at_one
                        .prepare_ticket(fixture.series_at_one.revision())
                        .expect("prepared root"),
                )
            ),
            Err(SeriesOperatorErrorV3::Schedule)
        );
        assert_eq!(
            build_expire_v3(
                fixture.occurrence_snapshot_with_series(
                    115,
                    Some(fixture.ticket_state),
                    fixture
                        .series_at_one
                        .prepare_ticket(fixture.series_at_one.revision())
                        .expect("prepared root"),
                )
            ),
            Err(SeriesOperatorErrorV3::Schedule)
        );

        let mut substituted = fixture.clone();
        substituted.siblings[0][0] ^= 1;
        assert_eq!(
            build_consume_v3(
                substituted.occurrence_snapshot_with_series(
                    110,
                    Some(substituted.ticket_state),
                    substituted
                        .series_at_one
                        .prepare_ticket(substituted.series_at_one.revision())
                        .expect("prepared root"),
                )
            ),
            Err(SeriesOperatorErrorV3::Content)
        );
    }

    #[test]
    fn retire_and_close_require_chain_terminal_state() {
        let fixture = Fixture::new();
        assert_eq!(
            build_retire_v3(SeriesRetireSnapshotV3 {
                template_bytes: &fixture.template,
                ticket_bytes: &fixture.ticket,
                series: fixture.series_at_one,
                ticket_state: fixture.ticket_state,
            }),
            Err(SeriesOperatorErrorV3::Replay)
        );
        let terminal_ticket = fixture
            .ticket_state
            .settle(0, TicketPhaseV3::Consumed)
            .expect("terminal Ticket");
        assert_eq!(
            build_retire_v3(SeriesRetireSnapshotV3 {
                template_bytes: &fixture.template,
                ticket_bytes: &fixture.ticket,
                series: fixture.series_at_one,
                ticket_state: terminal_ticket,
            }),
            Err(SeriesOperatorErrorV3::Replay)
        );
        let retire_series = fixture
            .series_at_one
            .prepare_ticket(fixture.series_at_one.revision())
            .expect("prepare current")
            .settle_current(fixture.series_at_one.revision() + 1, 3)
            .expect("settle current");
        let retire = build_retire_v3(SeriesRetireSnapshotV3 {
            template_bytes: &fixture.template,
            ticket_bytes: &fixture.ticket,
            series: retire_series,
            ticket_state: terminal_ticket,
        })
        .expect("Retire request");
        assert_eq!(
            retire.decode().expect("Retire decode").action(),
            SeriesActionV3::Retire
        );

        assert_eq!(
            build_close_v3(SeriesCloseSnapshotV3 {
                template_bytes: &fixture.template,
                series: fixture.series_at_one,
            }),
            Err(SeriesOperatorErrorV3::Replay)
        );
        let mut closed = SeriesStateV3::new(7);
        for revision in [0_u64, 3, 6] {
            closed = closed
                .prepare_ticket(revision)
                .expect("prepare")
                .settle_current(revision + 1, 3)
                .expect("settle")
                .retire_ticket(revision + 2)
                .expect("retire");
        }
        let close = build_close_v3(SeriesCloseSnapshotV3 {
            template_bytes: &fixture.template,
            series: closed,
        })
        .expect("Close request");
        assert_eq!(
            close.decode().expect("Close decode").action(),
            SeriesActionV3::Close
        );
    }
}
