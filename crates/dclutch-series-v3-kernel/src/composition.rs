//! Stateless recurring-Series effect composition.
//!
//! This module joins all Series-owned semantic facts for Consume before any
//! physical effect. It returns the Core request, exact Custody expectation,
//! replay candidates, and classified native funding. Generic Trading remains
//! the sole caller/writer, while Core, Claims, and Custody remain the sole
//! owners of their physical state and receipts.

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::SeriesCoreRequestV1;

use crate::{
    AccountKeyV3, AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2,
    SeriesV3Error,
    escrow::{TerminalSeriesEscrowPlanV3, consume_series_escrow_v3},
    plan::{
        SeriesReplayActionV3, SeriesReplayPlanErrorV3, SeriesReplayWitnessV3, evaluate_replay_v3,
    },
    pre_founding_series_escrow,
    replay::{SeriesStateV3, TicketStateV3},
    series_core_consume_request,
};

/// Refusal from complete stateless Consume composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsumeCompositionErrorV3 {
    /// Immutable content, Product, Market, or funding commitment refused.
    Content(SeriesV3Error),
    /// Mutable root/Ticket replay bytes or optimistic revisions refused.
    Replay(SeriesReplayPlanErrorV3),
    /// The current Clock slot was outside the exact scheduled retry window.
    Schedule,
}

impl From<SeriesV3Error> for SeriesConsumeCompositionErrorV3 {
    fn from(value: SeriesV3Error) -> Self {
        Self::Content(value)
    }
}

impl From<SeriesReplayPlanErrorV3> for SeriesConsumeCompositionErrorV3 {
    fn from(value: SeriesReplayPlanErrorV3) -> Self {
        Self::Replay(value)
    }
}

/// Complete Series-owned semantic result for one atomic Ticket consumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeCompositionV3 {
    core_request: SeriesCoreRequestV1,
    escrow: TerminalSeriesEscrowPlanV3,
    replay: SeriesReplayWitnessV3,
    funding_list: ContentId,
    native_from_ticket: u64,
}

impl SeriesConsumeCompositionV3 {
    /// Exact canonical 336-byte request which Core must execute atomically.
    pub const fn core_request(self) -> SeriesCoreRequestV1 {
        self.core_request
    }

    /// Exact SeriesEscrow-to-Hoard and terminal cleanup expectations.
    pub const fn escrow(self) -> TerminalSeriesEscrowPlanV3 {
        self.escrow
    }

    /// Joint root/Ticket candidate bytes committed only after every effect.
    pub const fn replay(self) -> SeriesReplayWitnessV3 {
        self.replay
    }

    /// Exact ordered FundingState-list commitment selected by the occurrence.
    pub const fn funding_list(self) -> ContentId {
        self.funding_list
    }

    /// Exact native lamports drained from Ticket custody on success.
    ///
    /// Hoard principal is Realm collateral and is never included.
    pub const fn native_from_ticket(self) -> u64 {
        self.native_from_ticket
    }
}

/// Compose one admitted prepared Ticket into its exact atomic Found request.
#[allow(clippy::too_many_arguments)]
pub fn compose_series_consume_v3(
    admitted: AdmittedOccurrenceV3,
    admitted_ticket: AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
    registry_program: AccountKeyV3,
    ticket_state_account: AccountKeyV3,
    series_bytes: &[u8],
    ticket_state_bytes: &[u8],
    now_slot: u64,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
) -> Result<SeriesConsumeCompositionV3, SeriesConsumeCompositionErrorV3> {
    admitted.require_ticket(admitted_ticket.ticket())?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    let series = SeriesStateV3::decode(series_bytes, template.occurrence_count())
        .map_err(SeriesReplayPlanErrorV3::State)?;
    let ticket_state =
        TicketStateV3::decode(ticket_state_bytes).map_err(SeriesReplayPlanErrorV3::State)?;
    if series.next_occurrence() != occurrence.occurrence()
        || ticket_state.ticket_record_id() != admitted_ticket.content_id()
    {
        return Err(SeriesConsumeCompositionErrorV3::Replay(
            SeriesReplayPlanErrorV3::TicketSubstitution,
        ));
    }
    let retry_through = template.retry_through(occurrence.occurrence())?;
    if now_slot < occurrence.scheduled_slot() || now_slot > retry_through {
        return Err(SeriesConsumeCompositionErrorV3::Schedule);
    }

    let replay = evaluate_replay_v3(
        SeriesReplayActionV3::Consume {
            ticket_record: admitted_ticket.content_id(),
            expected_ticket_revision,
        },
        template.occurrence_count(),
        expected_series_revision,
        series_bytes,
        Some(ticket_state_bytes),
    )?;
    let escrow = pre_founding_series_escrow(admitted, admitted_ticket, product, registry_program)?;
    let core_request = series_core_consume_request(
        admitted,
        admitted_ticket,
        product,
        ticket_state_account,
        expected_series_revision,
        expected_ticket_revision,
    )?;
    Ok(SeriesConsumeCompositionV3 {
        core_request,
        escrow: consume_series_escrow_v3(escrow),
        replay,
        funding_list: occurrence.funding_list(),
        native_from_ticket: occurrence.funds().checked_native_total()?,
    })
}
