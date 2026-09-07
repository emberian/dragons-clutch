//! Named admissible phases for the recurring Series ROOT.
//!
//! The sibling of [`crate::series::ticket_admission_v1`], one machine up. Three
//! of the root's four replay acts guarded their phase inline -- `self.phase !=
//! SeriesPhaseV3::Active`, `self.phase == SeriesPhaseV3::Terminal` -- so no
//! reader outside `SeriesStateV3` could name which phases an act is available
//! in, and the SDK's state-machine generator had no root machine to publish
//! even after the tail's ABI gained a Lean owner.
//!
//! ## The count is the emission's, not this file's
//!
//! `STATE_COUNT` is `SERIES_PHASE_LIMIT_V3`, emitted by
//! `DClutchSemantics.SeriesStateV3Abi` alongside the two tags the enum's
//! discriminants already are. The ticket sibling wrote its own `3` beside an
//! emitted `SERIES_TICKET_PHASE_LIMIT_V3` that nothing read -- two authors for
//! one count, agreeing -- and that is now fixed there too.
//!
//! ## What is deliberately NOT a set here
//!
//! [`crate::series::replay::SeriesStateV3::retire_ticket`] has **no phase
//! guard at all**, and that is a property of the machine rather than an
//! omission: a terminal ticket account is retired for its rent in either root
//! phase, and the root's own `outstanding_ticket_accounts` is what bounds the
//! act. Publishing `[Active, Terminal]` for it would say "every phase" in a
//! vocabulary meant for restrictions, and a client reading it would learn
//! nothing the absence does not already say.
//!
//! `validate`'s phase arms are not a set either. They are a joint condition
//! over the phase AND `next_occurrence` -- Active below the occurrence count,
//! Terminal exactly at it -- so a phase set alone would be a weaker claim
//! wearing the same name.
//!
//! Every set is a NECESSARY condition and never a sufficient one: a root
//! admitted by its phase still has its revision, its occurrence cursor and its
//! outstanding ticket count checked.

use crate::series::generated_series_state_v3::SERIES_PHASE_LIMIT_V3;
use crate::series::replay::SeriesPhaseV3;

/// Number of distinct `SeriesPhaseV3` values, from the emission that owns the
/// tags themselves.
const STATE_COUNT: u8 = SERIES_PHASE_LIMIT_V3;

/// The wire tag of one root phase, as a bit index.
const fn state_tag(state: SeriesPhaseV3) -> u8 {
    state as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The discriminants are the wire tags, so this is the
/// check that a phase added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(SeriesPhaseV3::Terminal) < STATE_COUNT);

/// The root phases in which one replay act is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SeriesRootAdmissionV1 {
    states: u8,
}

impl SeriesRootAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    #[must_use]
    pub const fn states(states: &[SeriesPhaseV3]) -> Self {
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
    pub const fn admits(self, state: SeriesPhaseV3) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// Every act that moves the occurrence cursor observes an active root.
///
/// Two guards, and they are the whole occurrence lifecycle:
/// [`crate::series::replay::SeriesStateV3::prepare_ticket`], which creates the
/// occurrence's Ticket replay, and
/// [`crate::series::replay::SeriesStateV3::settle_current`], which is the one
/// author of both the Consume and the Expire settlement. A root that has
/// settled its last occurrence has no occurrence left to prepare or settle.
pub const SERIES_ROOT_OCCURRENCE_ADMISSIBLE_STATES_V1: SeriesRootAdmissionV1 =
    SeriesRootAdmissionV1::states(&[SeriesPhaseV3::Active]);

/// Closing the root observes a terminal one.
///
/// [`crate::series::replay::SeriesStateV3::admit_close`]. Disjoint from the
/// occurrence set, which is the ordering fact
/// `the_two_act_sets_are_disjoint` pins: no act is available both while
/// occurrences remain and after the root is finished with them.
pub const SERIES_ROOT_CLOSE_ADMISSIBLE_STATES_V1: SeriesRootAdmissionV1 =
    SeriesRootAdmissionV1::states(&[SeriesPhaseV3::Terminal]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [SeriesPhaseV3; 2] = [SeriesPhaseV3::Active, SeriesPhaseV3::Terminal];

    /// The machine's phases are exactly the emission's count of them.
    #[test]
    fn the_state_count_is_the_emissions() {
        assert_eq!(
            u8::try_from(EVERY_STATE.len()).expect("two phases"),
            STATE_COUNT
        );
    }

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = SeriesRootAdmissionV1::states(&[state]);
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
        assert!(SeriesRootAdmissionV1::NONE.is_empty());
        assert!(SeriesRootAdmissionV1::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!SeriesRootAdmissionV1::NONE.admits(state));
        }
    }

    /// A set of several states admits exactly those and nothing else.
    ///
    /// The cases above pass the empty slice and single-element slices, which a
    /// constructor that stopped after its first entry would satisfy.
    #[test]
    fn a_listed_set_admits_exactly_what_it_lists() {
        let set = SeriesRootAdmissionV1::states(&EVERY_STATE);
        assert!(!set.is_empty());
        for state in EVERY_STATE {
            assert!(set.admits(state), "{state:?}");
        }
    }

    /// The two act sets are disjoint and together cover every phase.
    #[test]
    fn the_two_act_sets_are_disjoint() {
        for state in EVERY_STATE {
            assert!(
                SERIES_ROOT_OCCURRENCE_ADMISSIBLE_STATES_V1.admits(state)
                    ^ SERIES_ROOT_CLOSE_ADMISSIBLE_STATES_V1.admits(state),
                "{state:?} is admitted by both sets or by neither"
            );
        }
    }

    /// Each constant reproduces the exact condition that stood at its guards.
    ///
    /// Written out as the boolean the sites carried before the constants
    /// existed and checked over both phases. Control: swapping the two
    /// constants turns this red, and `the_two_act_sets_are_disjoint` green,
    /// which is why both are here.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        for state in EVERY_STATE {
            // `SeriesStateV3::prepare_ticket` and `::settle_current`, which
            // both read `self.phase != SeriesPhaseV3::Active`.
            assert_eq!(
                SERIES_ROOT_OCCURRENCE_ADMISSIBLE_STATES_V1.admits(state),
                state == SeriesPhaseV3::Active,
                "{state:?}"
            );
            // `SeriesStateV3::admit_close`, which read
            // `self.phase == SeriesPhaseV3::Terminal`.
            assert_eq!(
                SERIES_ROOT_CLOSE_ADMISSIBLE_STATES_V1.admits(state),
                state == SeriesPhaseV3::Terminal,
                "{state:?}"
            );
        }
    }
}
