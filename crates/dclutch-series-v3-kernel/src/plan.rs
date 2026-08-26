//! Stateless joint replay planning for recurring Series V3.
//!
//! This module turns immutable input bytes into candidate output bytes. It
//! cannot access accounts, move value, invoke another program, or commit a
//! write. The generic Trading Account/Request/Transition/Effect interpreter
//! remains the sole physical authority and may use this evaluator only as an
//! accelerator or differential oracle.

use dclutch_core_contract::ContentId;

use crate::replay::{
    SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateError, SeriesStateV3,
    TicketPhaseV3, TicketStateV3,
};

/// One semantic replay action after immutable Series content admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesReplayActionV3 {
    /// Create the unique prepared Ticket for the current occurrence.
    Prepare {
        /// Exact immutable Ticket-record identity selected by the request.
        ticket_record: ContentId,
    },
    /// Atomically consume the prepared Ticket into its Found Market.
    Consume {
        /// Exact immutable Ticket-record identity selected by the request.
        ticket_record: ContentId,
        /// Expected mutable Ticket revision.
        expected_ticket_revision: u64,
    },
    /// Refund the prepared Ticket after its exact retry window.
    Expire {
        /// Exact immutable Ticket-record identity selected by the request.
        ticket_record: ContentId,
        /// Expected mutable Ticket revision.
        expected_ticket_revision: u64,
    },
    /// Delete one already-terminal Ticket replay account.
    Retire {
        /// Exact immutable Ticket-record identity selected by the request.
        ticket_record: ContentId,
        /// Expected mutable Ticket revision.
        expected_ticket_revision: u64,
    },
    /// Delete the terminal Series root after every Ticket was retired.
    Close,
}

/// Candidate disposition for one account owned by the generic Trading layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayCandidateV3<const N: usize> {
    /// The action does not reference this account coordinate.
    Unchanged,
    /// Replace the complete fixed-width state with these candidate bytes.
    Replace([u8; N]),
    /// Delete the account only after all preceding physical effects accept.
    Delete,
}

/// Joint root/Ticket result of one total replay evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesReplayWitnessV3 {
    action: SeriesReplayActionV3,
    series: ReplayCandidateV3<SERIES_STATE_BYTES_V3>,
    ticket: ReplayCandidateV3<SERIES_TICKET_STATE_BYTES_V3>,
}

impl SeriesReplayWitnessV3 {
    /// Evaluated semantic action.
    pub const fn action(self) -> SeriesReplayActionV3 {
        self.action
    }

    /// Candidate Series-root disposition.
    pub const fn series(self) -> ReplayCandidateV3<SERIES_STATE_BYTES_V3> {
        self.series
    }

    /// Candidate Ticket-state disposition.
    pub const fn ticket(self) -> ReplayCandidateV3<SERIES_TICKET_STATE_BYTES_V3> {
        self.ticket
    }
}

/// Refusal from joint replay evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesReplayPlanErrorV3 {
    /// The action supplied a missing or extraneous Ticket-state account.
    Frame,
    /// Hostile mutable state bytes or their replay relation refused.
    State(SeriesStateError),
    /// The mutable Ticket selected another immutable Ticket record.
    TicketSubstitution,
    /// Retire selected a Ticket which still permits an economic retry.
    LiveTicket,
}

impl From<SeriesStateError> for SeriesReplayPlanErrorV3 {
    fn from(value: SeriesStateError) -> Self {
        Self::State(value)
    }
}

/// Evaluate one joint root/Ticket replay transition without side effects.
///
/// `ticket_bytes` is absent only for `Prepare` and `Close`. Every candidate is
/// constructed after all root and Ticket checks succeed, so a caller cannot
/// observe or persist a partial joint transition.
pub fn evaluate_replay_v3(
    action: SeriesReplayActionV3,
    occurrence_count: u32,
    expected_series_revision: u64,
    series_bytes: &[u8],
    ticket_bytes: Option<&[u8]>,
) -> Result<SeriesReplayWitnessV3, SeriesReplayPlanErrorV3> {
    let series = SeriesStateV3::decode(series_bytes, occurrence_count)?;
    match action {
        SeriesReplayActionV3::Prepare { ticket_record } => {
            if ticket_bytes.is_some() {
                return Err(SeriesReplayPlanErrorV3::Frame);
            }
            let series_after = series.prepare_ticket(expected_series_revision)?;
            let ticket_after = TicketStateV3::prepared(ticket_record);
            Ok(SeriesReplayWitnessV3 {
                action,
                series: ReplayCandidateV3::Replace(series_after.encode(occurrence_count)?),
                ticket: ReplayCandidateV3::Replace(ticket_after.encode()),
            })
        }
        SeriesReplayActionV3::Consume {
            ticket_record,
            expected_ticket_revision,
        } => settle(
            action,
            ticket_record,
            expected_ticket_revision,
            TicketPhaseV3::Consumed,
            occurrence_count,
            expected_series_revision,
            series,
            ticket_bytes,
        ),
        SeriesReplayActionV3::Expire {
            ticket_record,
            expected_ticket_revision,
        } => settle(
            action,
            ticket_record,
            expected_ticket_revision,
            TicketPhaseV3::Expired,
            occurrence_count,
            expected_series_revision,
            series,
            ticket_bytes,
        ),
        SeriesReplayActionV3::Retire {
            ticket_record,
            expected_ticket_revision,
        } => {
            let ticket = required_ticket(ticket_bytes, ticket_record)?;
            if !ticket.phase().terminal() {
                return Err(SeriesReplayPlanErrorV3::LiveTicket);
            }
            if ticket.revision() != expected_ticket_revision {
                return Err(SeriesStateError::Replay.into());
            }
            let series_after = series.retire_ticket(expected_series_revision)?;
            Ok(SeriesReplayWitnessV3 {
                action,
                series: ReplayCandidateV3::Replace(series_after.encode(occurrence_count)?),
                ticket: ReplayCandidateV3::Delete,
            })
        }
        SeriesReplayActionV3::Close => {
            if ticket_bytes.is_some() {
                return Err(SeriesReplayPlanErrorV3::Frame);
            }
            series.admit_close(expected_series_revision)?;
            Ok(SeriesReplayWitnessV3 {
                action,
                series: ReplayCandidateV3::Delete,
                ticket: ReplayCandidateV3::Unchanged,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn settle(
    action: SeriesReplayActionV3,
    ticket_record: ContentId,
    expected_ticket_revision: u64,
    terminal: TicketPhaseV3,
    occurrence_count: u32,
    expected_series_revision: u64,
    series: SeriesStateV3,
    ticket_bytes: Option<&[u8]>,
) -> Result<SeriesReplayWitnessV3, SeriesReplayPlanErrorV3> {
    let ticket = required_ticket(ticket_bytes, ticket_record)?;
    let series_after = series.settle_current(expected_series_revision, occurrence_count)?;
    let ticket_after = ticket.settle(expected_ticket_revision, terminal)?;
    Ok(SeriesReplayWitnessV3 {
        action,
        series: ReplayCandidateV3::Replace(series_after.encode(occurrence_count)?),
        ticket: ReplayCandidateV3::Replace(ticket_after.encode()),
    })
}

fn required_ticket(
    ticket_bytes: Option<&[u8]>,
    ticket_record: ContentId,
) -> Result<TicketStateV3, SeriesReplayPlanErrorV3> {
    let ticket = TicketStateV3::decode(ticket_bytes.ok_or(SeriesReplayPlanErrorV3::Frame)?)?;
    if ticket.ticket_record_id() != ticket_record {
        return Err(SeriesReplayPlanErrorV3::TicketSubstitution);
    }
    Ok(ticket)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero")
    }

    fn replacement<const N: usize>(candidate: ReplayCandidateV3<N>) -> Option<[u8; N]> {
        match candidate {
            ReplayCandidateV3::Replace(bytes) => Some(bytes),
            ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => None,
        }
    }

    #[test]
    fn full_joint_lifecycle_is_atomic_and_terminal() {
        let ticket_record = id(7);
        let initial = SeriesStateV3::new(19).encode(1).expect("initial");
        let prepared = evaluate_replay_v3(
            SeriesReplayActionV3::Prepare { ticket_record },
            1,
            0,
            &initial,
            None,
        )
        .expect("prepare");
        let prepared_root = replacement(prepared.series()).expect("prepared root replacement");
        let prepared_ticket = replacement(prepared.ticket()).expect("prepared Ticket replacement");

        assert_eq!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Prepare { ticket_record },
                1,
                1,
                &prepared_root,
                None,
            ),
            Err(SeriesReplayPlanErrorV3::State(SeriesStateError::Replay))
        );

        let consumed = evaluate_replay_v3(
            SeriesReplayActionV3::Consume {
                ticket_record,
                expected_ticket_revision: 0,
            },
            1,
            1,
            &prepared_root,
            Some(&prepared_ticket),
        )
        .expect("consume");
        let consumed_root = replacement(consumed.series()).expect("consumed root replacement");
        let consumed_ticket = replacement(consumed.ticket()).expect("consumed Ticket replacement");
        assert_eq!(
            TicketStateV3::decode(&consumed_ticket)
                .expect("terminal Ticket")
                .phase(),
            TicketPhaseV3::Consumed
        );

        let retired = evaluate_replay_v3(
            SeriesReplayActionV3::Retire {
                ticket_record,
                expected_ticket_revision: 1,
            },
            1,
            2,
            &consumed_root,
            Some(&consumed_ticket),
        )
        .expect("retire");
        assert_eq!(retired.ticket(), ReplayCandidateV3::Delete);
        let retired_root = replacement(retired.series()).expect("retired root replacement");

        let closed = evaluate_replay_v3(SeriesReplayActionV3::Close, 1, 3, &retired_root, None)
            .expect("close");
        assert_eq!(closed.series(), ReplayCandidateV3::Delete);
        assert_eq!(closed.ticket(), ReplayCandidateV3::Unchanged);

        // Inputs were borrowed and remain byte-for-byte unchanged.
        assert_eq!(initial, SeriesStateV3::new(19).encode(1).expect("initial"));
    }

    #[test]
    fn hostile_frame_substitution_and_partial_terminals_refuse() {
        let ticket_record = id(8);
        let other_record = id(9);
        let initial = SeriesStateV3::new(5).encode(1).expect("initial");
        let prepared = evaluate_replay_v3(
            SeriesReplayActionV3::Prepare { ticket_record },
            1,
            0,
            &initial,
            None,
        )
        .expect("prepare");
        let prepared_root = replacement(prepared.series()).expect("prepared root replacement");
        let prepared_ticket = replacement(prepared.ticket()).expect("prepared Ticket replacement");

        assert_eq!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Expire {
                    ticket_record: other_record,
                    expected_ticket_revision: 0,
                },
                1,
                1,
                &prepared_root,
                Some(&prepared_ticket),
            ),
            Err(SeriesReplayPlanErrorV3::TicketSubstitution)
        );
        assert_eq!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Retire {
                    ticket_record,
                    expected_ticket_revision: 0,
                },
                1,
                1,
                &prepared_root,
                Some(&prepared_ticket),
            ),
            Err(SeriesReplayPlanErrorV3::LiveTicket)
        );
        assert_eq!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Close,
                1,
                1,
                &prepared_root,
                Some(&prepared_ticket),
            ),
            Err(SeriesReplayPlanErrorV3::Frame)
        );
        assert_eq!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Consume {
                    ticket_record,
                    expected_ticket_revision: 0,
                },
                1,
                1,
                &prepared_root,
                None,
            ),
            Err(SeriesReplayPlanErrorV3::Frame)
        );
    }
}
