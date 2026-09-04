//! Named admissible projected-custody phases for one transition's guard.
//!
//! [`ProjectedCustodyPhaseV1`] is the fourth machine to take this shape after
//! `MarketAdmissionV1` (`315f1931`), `SourceAdmissionV1` and the two Dealer
//! scenario machines, and it is the one a founding walks end to end: a
//! projection is `Initialized`, opens a Hoard, may fund a source compartment,
//! locks, and is then either realized into a founding or aborted back out.
//! Eight guards in this crate and four more in Core and Trading read that
//! phase, and written inline inside ten-conjunct disjunctions not one of them
//! had a name a reader outside the function could reach.
//!
//! ## Not the Market's phase, and not the Custody replay's either
//!
//! A Market is VACANT for the whole span this machine runs -- `market_vacant`
//! is a conjunct of nearly every guard here -- so the Core phase cannot answer
//! a single question this one answers. That is the reason one admission type
//! per machine exists rather than one bare `Phase` per program: a set checked
//! against the wrong discriminant is not a widening, it is an answer to a
//! different question.
//!
//! The type is deliberately the same shape as the other three -- a
//! const-constructed bitset indexed by the machine's own wire tags, with
//! `states`, `admits` and `is_empty` -- so the route census reads it with one
//! enumerator parameterized by machine rather than one parser per machine.
//!
//! The discriminants ARE Lean-emitted, since
//! `DClutchSemantics.ProjectedCustodyStateV2Abi`:
//! [`ProjectedCustodyPhaseV1`] names `PROJECTED_CUSTODY_PHASE_INITIALIZED_V1`
//! and its three siblings rather than writing `1`, `2`, `3`, `4`, and its
//! `decode` match admits them by the same names. So the bit index below is
//! still the enum's own discriminant, which is now the emitted authority
//! reached one step later rather than a second numbering;
//! `the_bit_index_is_the_wire_tag` pins the pair.
//!
//! Every set is a NECESSARY condition and never a sufficient one. A projection
//! admitted by its phase still has its request digest, its revision, its
//! amounts, its expiry slot and its accounts checked.

use crate::projected::ProjectedCustodyPhaseV1;

/// One past the greatest `ProjectedCustodyPhaseV1` discriminant.
///
/// Emitted, because it is a fact about the tags and not about this bitset: the
/// machine numbers from one, so bit zero is never occupied and the bound is one
/// past the last variant rather than the number of variants.
use crate::generated_projected_state_v2::PROJECTED_CUSTODY_PHASE_LIMIT_V1 as PHASE_LIMIT;

/// The wire tag of one projected phase, as a bit index.
const fn phase_tag(phase: ProjectedCustodyPhaseV1) -> u8 {
    phase as u8
}

/// Every phase occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The machine numbers from one, so bit zero is never
/// occupied and the limit is one past the last variant.
const _: () = assert!(PHASE_LIMIT <= 8);
const _: () = assert!(phase_tag(ProjectedCustodyPhaseV1::SourceFunded) < PHASE_LIMIT);

/// The projected-custody phases in which one transition is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectedCustodyAdmissionV1 {
    states: u8,
}

impl ProjectedCustodyAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    #[must_use]
    pub const fn states(states: &[ProjectedCustodyPhaseV1]) -> Self {
        let mut admitted = 0u8;
        let mut rest = states;
        while let [state, tail @ ..] = rest {
            admitted |= 1u8 << phase_tag(*state);
            rest = tail;
        }
        Self { states: admitted }
    }

    /// Whether this exact phase is admitted.
    #[must_use]
    pub const fn admits(self, phase: ProjectedCustodyPhaseV1) -> bool {
        self.states & (1u8 << phase_tag(phase)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

// ------------------------------------------------------- the declared sets
//
// Declared here, in the crate that owns the machine, rather than in each of
// the three programs that read it. Custody executes the transitions, Trading
// drives the founding ladder and Core consumes the realized projection; three
// programs writing their own set over one contract-owned machine is how one
// fact acquires three authors who cannot see each other.

/// Opening the projected Hoard observes a projection that has nothing yet.
pub const PROJECTED_CUSTODY_OPEN_HOARD_ADMISSIBLE_STATES_V1: ProjectedCustodyAdmissionV1 =
    ProjectedCustodyAdmissionV1::states(&[ProjectedCustodyPhaseV1::Initialized]);

/// An empty Hoard is the prestate of every act that puts value in or gives up.
///
/// Three transitions share it exactly -- open the source compartment, lock the
/// Hoard directly, and abort an unfunded projection -- and Trading's own
/// staging check reads the same set.
pub const PROJECTED_CUSTODY_HOARD_OPEN_ADMISSIBLE_STATES_V1: ProjectedCustodyAdmissionV1 =
    ProjectedCustodyAdmissionV1::states(&[ProjectedCustodyPhaseV1::HoardOpen]);

/// The atomic founding lock admits two disjoint prestates.
///
/// A family whose principal was already custodied elsewhere arrives at
/// `HoardOpen` holding nothing under this authority; a generic founding
/// arrives at `SourceFunded` holding exactly the principal its own
/// `OpenSourceCompartment` put in the source vault. The AMOUNT each must hold
/// differs, which is why the guard keeps a match beside this set: the set says
/// which phases may ask, the match says what each must hold.
pub const PROJECTED_CUSTODY_FOUNDING_LOCK_ADMISSIBLE_STATES_V1: ProjectedCustodyAdmissionV1 =
    ProjectedCustodyAdmissionV1::states(&[
        ProjectedCustodyPhaseV1::HoardOpen,
        ProjectedCustodyPhaseV1::SourceFunded,
    ]);

/// A locked projection is what a founding realizes and what a refund returns.
///
/// The set Core reads too: `generic_founding_v1` and `series_consume` both
/// refuse a projection that is not holding its principal, and this is the
/// same set they were each spelling inline.
pub const PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1: ProjectedCustodyAdmissionV1 =
    ProjectedCustodyAdmissionV1::states(&[ProjectedCustodyPhaseV1::HoardLocked]);

/// Aborting back out of a funded source compartment.
///
/// `SourceFunded` and nothing else. The phase is the whole admission: it is a
/// value no previously reachable state can hold, so this admits nothing that
/// was refused before it existed.
pub const PROJECTED_CUSTODY_SOURCE_FUNDED_ADMISSIBLE_STATES_V1: ProjectedCustodyAdmissionV1 =
    ProjectedCustodyAdmissionV1::states(&[ProjectedCustodyPhaseV1::SourceFunded]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_PHASE: [ProjectedCustodyPhaseV1; 4] = [
        ProjectedCustodyPhaseV1::Initialized,
        ProjectedCustodyPhaseV1::HoardOpen,
        ProjectedCustodyPhaseV1::HoardLocked,
        ProjectedCustodyPhaseV1::SourceFunded,
    ];

    #[test]
    fn every_phase_has_its_own_bit() {
        for phase in EVERY_PHASE {
            let one = ProjectedCustodyAdmissionV1::states(&[phase]);
            let admitted = EVERY_PHASE
                .iter()
                .filter(|other| one.admits(**other))
                .count();
            assert_eq!(admitted, 1, "{phase:?} aliases another bit");
            assert!(one.admits(phase));
        }
    }

    #[test]
    fn the_empty_set_admits_nothing() {
        assert!(ProjectedCustodyAdmissionV1::NONE.is_empty());
        assert!(ProjectedCustodyAdmissionV1::states(&[]).is_empty());
        for phase in EVERY_PHASE {
            assert!(!ProjectedCustodyAdmissionV1::NONE.admits(phase));
        }
    }

    /// The bit index is the discriminant the decoder reads, not a second
    /// numbering. The machine numbers from one, so bit zero stays vacant.
    #[test]
    fn the_bit_index_is_the_wire_tag() {
        assert_eq!(ProjectedCustodyPhaseV1::Initialized as u8, 1);
        assert_eq!(ProjectedCustodyPhaseV1::HoardOpen as u8, 2);
        assert_eq!(ProjectedCustodyPhaseV1::HoardLocked as u8, 3);
        assert_eq!(ProjectedCustodyPhaseV1::SourceFunded as u8, 4);
    }

    /// Every set, checked against the exact condition that stood at its guard.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        /// One guard the admission set replaced: its name, the set, and the
        /// exact predicate that stood at the guard before it.
        type GuardCase = (
            &'static str,
            ProjectedCustodyAdmissionV1,
            fn(ProjectedCustodyPhaseV1) -> bool,
        );
        let cases: [GuardCase; 5] = [
            (
                "open_hoard",
                PROJECTED_CUSTODY_OPEN_HOARD_ADMISSIBLE_STATES_V1,
                |phase| phase == ProjectedCustodyPhaseV1::Initialized,
            ),
            (
                "open_source_compartment / lock_hoard / abort_open_and_close",
                PROJECTED_CUSTODY_HOARD_OPEN_ADMISSIBLE_STATES_V1,
                |phase| phase == ProjectedCustodyPhaseV1::HoardOpen,
            ),
            (
                "lock_hoard_and_close_source",
                PROJECTED_CUSTODY_FOUNDING_LOCK_ADMISSIBLE_STATES_V1,
                |phase| {
                    matches!(
                        phase,
                        ProjectedCustodyPhaseV1::HoardOpen | ProjectedCustodyPhaseV1::SourceFunded
                    )
                },
            ),
            (
                "refund_and_close / realize_and_close / Core's two",
                PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1,
                |phase| phase == ProjectedCustodyPhaseV1::HoardLocked,
            ),
            (
                "abort_source_and_close",
                PROJECTED_CUSTODY_SOURCE_FUNDED_ADMISSIBLE_STATES_V1,
                |phase| phase == ProjectedCustodyPhaseV1::SourceFunded,
            ),
        ];
        for (name, declared, stood) in cases {
            for phase in EVERY_PHASE {
                assert_eq!(
                    declared.admits(phase),
                    stood(phase),
                    "{name} moved at {phase:?}"
                );
            }
        }
    }

    /// The ladder is ordered: no phase both opens a Hoard and locks one, and
    /// the two-prestate founding lock contains each single-prestate set that
    /// reaches it.
    #[test]
    fn the_ladder_steps_do_not_overlap_and_the_lock_contains_its_two() {
        for phase in EVERY_PHASE {
            assert!(
                !(PROJECTED_CUSTODY_OPEN_HOARD_ADMISSIBLE_STATES_V1.admits(phase)
                    && PROJECTED_CUSTODY_HOARD_OPEN_ADMISSIBLE_STATES_V1.admits(phase)),
                "{phase:?} both precedes and follows the Hoard opening"
            );
            assert!(
                !(PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1.admits(phase)
                    && PROJECTED_CUSTODY_FOUNDING_LOCK_ADMISSIBLE_STATES_V1.admits(phase)),
                "{phase:?} is both a lock prestate and already locked"
            );
            if PROJECTED_CUSTODY_HOARD_OPEN_ADMISSIBLE_STATES_V1.admits(phase)
                || PROJECTED_CUSTODY_SOURCE_FUNDED_ADMISSIBLE_STATES_V1.admits(phase)
            {
                assert!(
                    PROJECTED_CUSTODY_FOUNDING_LOCK_ADMISSIBLE_STATES_V1.admits(phase),
                    "{phase:?} reaches the founding lock and the lock refuses it"
                );
            }
        }
    }
}
