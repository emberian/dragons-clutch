//! Named admissible states for one occurrence ticket's replay phase.
//!
//! [`TicketPhaseV3`] is the eighth machine to take the shape
//! `MarketAdmissionV1` introduced (`315f1931`). Five guards read it -- the
//! kernel's own `settle`, Trading's hot expiry authentication, and three
//! reads across Core's two permit-expiry routes -- and written inline as
//! `ticket.phase() != TicketPhaseV3::Prepared` not one of them had a name a
//! reader outside the function could reach.
//!
//! ## The Market this ticket funds does not exist yet
//!
//! Which is why this cannot be a use of the Core Market's phase and never
//! could be. A ticket is `Prepared` precisely while its future Market is
//! VACANT -- the whole point of the record is to hold exact custody for a
//! Market that has not been founded -- and it leaves `Prepared` either by
//! being consumed INTO that founding or by expiring without one. So for this
//! machine's entire span there is no Market phase to read, and after it there
//! is no ticket left to ask about. Two machines, and one observation window
//! each.
//!
//! ## The tags are the enum's, and the enum is the encoder
//!
//! `TicketPhaseV3` carries `#[repr(u8)]` with explicit discriminants and
//! [`crate::replay::TicketStateV3::encode`] writes `self.phase as u8`
//! directly, so the discriminant IS the wire tag with no emitted constant in
//! between. `the_bit_index_is_the_wire_tag` pins the index against `decode`,
//! which is the other half of that pair.
//!
//! ## What is deliberately NOT a set here
//!
//! Two things, and both would be false claims rather than under-counts.
//!
//! `series_permit_expiry_precommit_v1` checks `ticket_after.phase()` against
//! `Expired` -- a POSTSTATE, read off a candidate the transition produced.
//! Published as an admissible-prestate constant it would tell a client the
//! route requires an already-expired ticket, which is the opposite of true:
//! the route's prestate is `Prepared` and it is the one that expires it.
//!
//! [`TicketPhaseV3::terminal`] -- "no economic retry remains" -- is the
//! complement of `Prepared` and already has a name of its own on the enum.
//! Four callers read it as a predicate rather than as a set of one route's
//! prestates, so it stays a method; a constant beside it would be a second
//! author for one fact.
//!
//! Every set is a NECESSARY condition and never a sufficient one: a ticket
//! admitted by its phase still has its record identity, its replay revision,
//! its derived address and its occurrence checked.

use crate::replay::TicketPhaseV3;

/// Number of distinct `TicketPhaseV3` values.
const STATE_COUNT: u8 = 3;

/// The wire tag of one ticket phase, as a bit index.
const fn state_tag(state: TicketPhaseV3) -> u8 {
    state as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The discriminants are the wire tags, so this is the
/// check that a phase added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(TicketPhaseV3::Expired) < STATE_COUNT);

/// The ticket replay phases in which one route is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SeriesTicketAdmissionV1 {
    states: u8,
}

impl SeriesTicketAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    #[must_use]
    pub const fn states(states: &[TicketPhaseV3]) -> Self {
        let mut admitted = 0u8;
        let mut remaining = states;
        while let [state, rest @ ..] = remaining {
            admitted |= 1u8 << state_tag(*state);
            remaining = rest;
        }
        Self { states: admitted }
    }

    /// Whether this exact phase is admitted.
    #[must_use]
    pub const fn admits(self, state: TicketPhaseV3) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// Every act that spends a ticket observes a prepared one.
///
/// Three guards: the kernel's `settle`, which is the sole author of the
/// terminal transition; Trading's hot expiry, which authenticates the replay
/// account before driving it; and Core's expiry precommit, which authenticates
/// the same account before proposing the candidate bytes.
///
/// The set is a single phase because the machine has exactly one non-terminal
/// state -- a ticket is retryable or it is finished -- and that is what makes
/// `revision` rather than `phase` carry the optimistic-concurrency weight.
pub const SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1: SeriesTicketAdmissionV1 =
    SeriesTicketAdmissionV1::states(&[TicketPhaseV3::Prepared]);

/// Closing an occurrence's permit observes an already-expired ticket.
///
/// Core's `series_permit_expiry` runs AFTER the expiry it settles, so its
/// prestate is the poststate of the route above. The two sets are disjoint,
/// which is the ordering fact `the_two_route_sets_are_disjoint` pins: no act
/// is available both before and after a ticket stops being retryable.
///
/// `Consumed` is admitted by neither, and that is the whole content of the
/// machine: a ticket that founded its Market has no further act at all, and
/// the permit it consumed is closed by the founding rather than by an expiry.
pub const SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1: SeriesTicketAdmissionV1 =
    SeriesTicketAdmissionV1::states(&[TicketPhaseV3::Expired]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [TicketPhaseV3; 3] = [
        TicketPhaseV3::Prepared,
        TicketPhaseV3::Consumed,
        TicketPhaseV3::Expired,
    ];

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = SeriesTicketAdmissionV1::states(&[state]);
            let admitted = EVERY_STATE
                .iter()
                .filter(|other| one.admits(**other))
                .count();
            assert_eq!(admitted, 1, "{state:?} aliases another bit");
            assert!(one.admits(state));
        }
    }

    #[test]
    fn the_empty_set_admits_nothing() {
        assert!(SeriesTicketAdmissionV1::NONE.is_empty());
        assert!(SeriesTicketAdmissionV1::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!SeriesTicketAdmissionV1::NONE.admits(state));
        }
    }

    /// A set of several states admits exactly those and nothing else.
    ///
    /// The cases above pass the empty slice and single-element slices, which a
    /// constructor that stopped after its first entry would satisfy.
    #[test]
    fn a_listed_set_admits_exactly_what_it_lists() {
        let listed = [TicketPhaseV3::Consumed, TicketPhaseV3::Expired];
        let set = SeriesTicketAdmissionV1::states(&listed);
        assert!(!set.is_empty());
        for state in EVERY_STATE {
            assert_eq!(set.admits(state), listed.contains(&state), "{state:?}");
        }
    }

    /// The bit index is the byte `encode` writes, checked through `decode`.
    #[test]
    fn the_bit_index_is_the_wire_tag() {
        for (tag, state) in EVERY_STATE.into_iter().enumerate() {
            assert_eq!(
                u8::try_from(tag).expect("three phases"),
                state_tag(state),
                "{state:?} sits at an index that is not its wire tag"
            );
            assert_eq!(TicketPhaseV3::decode(state_tag(state)), Ok(state));
        }
    }

    /// The two route sets are disjoint, and `Consumed` admits nothing.
    #[test]
    fn the_two_route_sets_are_disjoint() {
        for state in EVERY_STATE {
            assert!(
                !(SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(state)
                    && SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1.admits(state)),
                "{state:?} is admitted by both sets"
            );
        }
        assert!(!SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(TicketPhaseV3::Consumed));
        assert!(!SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1.admits(TicketPhaseV3::Consumed));
    }

    /// The prepared set is exactly the complement of `terminal`.
    ///
    /// Two names for one partition, so they are pinned to each other rather
    /// than left to drift: adding a fourth retryable phase to the enum without
    /// adding it here turns this red.
    #[test]
    fn the_prepared_set_is_the_complement_of_terminal() {
        for state in EVERY_STATE {
            assert_eq!(
                SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(state),
                !state.terminal(),
                "{state:?}"
            );
        }
    }

    /// Each constant reproduces the exact condition that stood at its guards.
    ///
    /// Written out as the boolean the sites carried before the constants
    /// existed and checked over every one of the machine's three phases.
    /// Control: adding `Consumed` to the prepared set turns three tests red,
    /// one of them here naming `Consumed`.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        for state in EVERY_STATE {
            // `TicketStateV3::settle`, Trading's `hot_v3` expiry
            // authentication, Core's `series_permit_expiry_precommit_v1`.
            assert_eq!(
                SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(state),
                state == TicketPhaseV3::Prepared,
                "{state:?}"
            );
            // Core's `series_permit_expiry`.
            assert_eq!(
                SERIES_TICKET_EXPIRED_ADMISSIBLE_STATES_V1.admits(state),
                state == TicketPhaseV3::Expired,
                "{state:?}"
            );
        }
    }
}
