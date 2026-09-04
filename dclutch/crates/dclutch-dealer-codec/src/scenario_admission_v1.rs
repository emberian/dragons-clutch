//! Named admissible states for the two Dealer scenario state machines.
//!
//! This is `MarketAdmissionV1`'s repair (`315f1931`) applied to the machines a
//! scenario checkpoint actually turns on. A guard written inline as
//! `self.phase != DealerScenarioCheckpointPhaseV1::Evaluated` has no name, so
//! nothing outside the method reads it: not the route census, not the
//! reference, and not a client deciding whether a reserve is worth building.
//!
//! ## Two machines, not one, and neither of them is the Market's
//!
//! A Dealer scenario commit turns on THREE persisted discriminants at once:
//! the Core Market's `Phase` (already named, `TRADING_OPEN_MARKET_…`), the
//! checkpoint's own preparation phase, and each reservation's lifecycle
//! status. They move independently — a Market is `Open` for the whole span in
//! which a checkpoint runs `Collecting` → `Committed`, and a single
//! reservation goes `Active` → `Activated` inside the last step of that — so
//! one set read against the wrong machine is not a widening, it is an answer
//! to a different question. One admission type per machine is what makes that
//! impossible to write.
//!
//! Both types are deliberately the same shape as [`MarketAdmissionV1`] and
//! `SourceAdmissionV1`: a const-constructed bitset indexed by the machine's
//! own wire tags, with `states`, `admits` and `is_empty`. That is what lets
//! the route census read them with one enumerator parameterized by machine
//! rather than one parser per state machine.
//!
//! ## The tags are the wire discriminants
//!
//! Both machines' discriminants are Lean-emitted, since
//! `DClutchSemantics.DealerScenarioCheckpointV1Abi` and
//! `DClutchSemantics.DealerScenarioReservationStateV1Abi`:
//! [`DealerScenarioCheckpointPhaseV1`] names
//! `DEALER_SCENARIO_CHECKPOINT_PHASE_COLLECTING_V1` and its four siblings,
//! [`DealerScenarioReservationStateStatusV1`] names
//! `DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1` and its two, and both
//! `decode` matches admit them by the same names rather than by `1`, `2`, `3`.
//!
//! The bit index is still the enum's discriminant, which is now the emitted
//! authority reached one step later rather than a second numbering.
//! `every_state_has_its_own_bit` and `the_bit_index_is_the_wire_tag` are what
//! stop a variant added upstream from silently aliasing.
//!
//! Every set here is a NECESSARY condition and never a sufficient one: a
//! checkpoint admitted by its phase still has its slot window, its ordinals,
//! its digests and its accounts checked.

use crate::{
    scenario_checkpoint_v1::DealerScenarioCheckpointPhaseV1,
    scenario_custody_reservation_v1::DealerScenarioReservationStateStatusV1,
};

/// One past the greatest `DealerScenarioCheckpointPhaseV1` discriminant.
///
/// Emitted, because it is a fact about the tags and not about this bitset: the
/// machine numbers from one, so bit zero is never occupied and the bound is one
/// past the last variant rather than the number of variants.
use crate::generated_scenario_checkpoint_v1::DEALER_SCENARIO_CHECKPOINT_PHASE_LIMIT_V1 as CHECKPOINT_PHASE_LIMIT;
/// One past the greatest `DealerScenarioReservationStateStatusV1` discriminant.
///
/// Emitted, for the same reason as the checkpoint's above.
use crate::generated_scenario_reservation_state_v1::DEALER_SCENARIO_RESERVATION_STATUS_LIMIT_V1 as RESERVATION_STATUS_LIMIT;

/// The wire tag of one checkpoint phase, as a bit index.
const fn checkpoint_tag(phase: DealerScenarioCheckpointPhaseV1) -> u8 {
    phase as u8
}

/// The wire tag of one reservation status, as a bit index.
const fn reservation_tag(status: DealerScenarioReservationStateStatusV1) -> u8 {
    status as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index either
/// machine can produce must fit. Both machines number from one, so bit zero is
/// never occupied and the limit is one past the last variant.
const _: () = assert!(CHECKPOINT_PHASE_LIMIT <= 8);
const _: () = assert!(RESERVATION_STATUS_LIMIT <= 8);
const _: () =
    assert!(checkpoint_tag(DealerScenarioCheckpointPhaseV1::Committed) < CHECKPOINT_PHASE_LIMIT);
const _: () = assert!(
    reservation_tag(DealerScenarioReservationStateStatusV1::Activated) < RESERVATION_STATUS_LIMIT
);

/// The checkpoint preparation phases in which one act is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerScenarioCheckpointAdmissionV1 {
    states: u8,
}

impl DealerScenarioCheckpointAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    #[must_use]
    pub const fn states(states: &[DealerScenarioCheckpointPhaseV1]) -> Self {
        let mut admitted = 0u8;
        let mut position = 0;
        while position < states.len() {
            admitted |= 1u8 << checkpoint_tag(states[position]);
            position += 1;
        }
        Self { states: admitted }
    }

    /// Whether this exact phase is admitted.
    #[must_use]
    pub const fn admits(self, phase: DealerScenarioCheckpointPhaseV1) -> bool {
        self.states & (1u8 << checkpoint_tag(phase)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// The per-effect reservation statuses in which one act is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerScenarioReservationAdmissionV1 {
    states: u8,
}

impl DealerScenarioReservationAdmissionV1 {
    /// The empty admission: no status at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed statuses.
    #[must_use]
    pub const fn states(states: &[DealerScenarioReservationStateStatusV1]) -> Self {
        let mut admitted = 0u8;
        let mut position = 0;
        while position < states.len() {
            admitted |= 1u8 << reservation_tag(states[position]);
            position += 1;
        }
        Self { states: admitted }
    }

    /// Whether this exact status is admitted.
    #[must_use]
    pub const fn admits(self, status: DealerScenarioReservationStateStatusV1) -> bool {
        self.states & (1u8 << reservation_tag(status)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

// ------------------------------------------------------- the declared sets
//
// Every set over these two machines lives here rather than in the program
// that reads it, because unlike the Market's phase both machines are OWNED by
// this crate: the checkpoint transitions are methods on
// `DealerScenarioCheckpointV1`, and Trading and Custody each execute half of
// the same ladder against them. Two programs writing their own set over one
// codec-owned machine is how two authors of one fact appear.

/// Page collection and evaluation sealing observe a collecting checkpoint.
///
/// The narrowest set on the ladder, and the one that makes the ladder ordered:
/// no page may be appended and no evaluation sealed once anything downstream
/// has begun.
pub const DEALER_SCENARIO_COLLECTING_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 =
    DealerScenarioCheckpointAdmissionV1::states(&[DealerScenarioCheckpointPhaseV1::Collecting]);

/// Appending a Custody reservation receipt observes a sealed evaluation.
pub const DEALER_SCENARIO_RESERVE_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 =
    DealerScenarioCheckpointAdmissionV1::states(&[DealerScenarioCheckpointPhaseV1::Evaluated]);

/// Appending a reverse-order rollback receipt, in the codec.
///
/// Wider than the Custody-side set below by `Evaluated`, and deliberately: a
/// checkpoint that expired part-way through reserving is still `Evaluated`
/// when its first rollback arrives, and refusing that would strand every
/// reservation it had already taken.
pub const DEALER_SCENARIO_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 = DealerScenarioCheckpointAdmissionV1::states(&[
    DealerScenarioCheckpointPhaseV1::Evaluated,
    DealerScenarioCheckpointPhaseV1::Reserved,
    DealerScenarioCheckpointPhaseV1::RollingBack,
]);

/// Custody's own rollback prestate, which is NARROWER than the codec's.
///
/// Custody reaches its rollback arm only through a batch that already holds a
/// reservation, so `Evaluated` — a checkpoint with no reservation yet taken —
/// cannot be one of its prestates. The two sets are pinned against each other
/// by `the_custody_rollback_set_is_contained_in_the_codec_one`, so narrowing
/// the codec's below Custody's would go red rather than making a Custody route
/// unreachable in a phase its own program admits.
pub const DEALER_SCENARIO_CUSTODY_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 = DealerScenarioCheckpointAdmissionV1::states(&[
    DealerScenarioCheckpointPhaseV1::Reserved,
    DealerScenarioCheckpointPhaseV1::RollingBack,
]);

/// The atomic commit observes a fully reserved checkpoint.
pub const DEALER_SCENARIO_COMMIT_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 =
    DealerScenarioCheckpointAdmissionV1::states(&[DealerScenarioCheckpointPhaseV1::Reserved]);

/// Activating a reserved Custody effect observes a committed checkpoint.
pub const DEALER_SCENARIO_COMMITTED_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 =
    DealerScenarioCheckpointAdmissionV1::states(&[DealerScenarioCheckpointPhaseV1::Committed]);

/// Permissionless cleanup observes anything that did not commit.
///
/// Written as the complement of `Committed` because that is what the guard
/// says, and the complement is enumerated rather than negated: a phase added
/// upstream is then EXCLUDED from cleanup until someone decides otherwise,
/// which is the safe direction for a route that closes an account.
pub const DEALER_SCENARIO_CLEANUP_CHECKPOINT_ADMISSIBLE_STATES_V1:
    DealerScenarioCheckpointAdmissionV1 = DealerScenarioCheckpointAdmissionV1::states(&[
    DealerScenarioCheckpointPhaseV1::Collecting,
    DealerScenarioCheckpointPhaseV1::Evaluated,
    DealerScenarioCheckpointPhaseV1::Reserved,
    DealerScenarioCheckpointPhaseV1::RollingBack,
]);

/// Rolling back or activating one effect observes a live escrow.
pub const DEALER_SCENARIO_ACTIVE_RESERVATION_ADMISSIBLE_STATES_V1:
    DealerScenarioReservationAdmissionV1 =
    DealerScenarioReservationAdmissionV1::states(&[DealerScenarioReservationStateStatusV1::Active]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_PHASE: [DealerScenarioCheckpointPhaseV1; 5] = [
        DealerScenarioCheckpointPhaseV1::Collecting,
        DealerScenarioCheckpointPhaseV1::Evaluated,
        DealerScenarioCheckpointPhaseV1::Reserved,
        DealerScenarioCheckpointPhaseV1::RollingBack,
        DealerScenarioCheckpointPhaseV1::Committed,
    ];

    const EVERY_STATUS: [DealerScenarioReservationStateStatusV1; 3] = [
        DealerScenarioReservationStateStatusV1::Active,
        DealerScenarioReservationStateStatusV1::RolledBack,
        DealerScenarioReservationStateStatusV1::Activated,
    ];

    #[test]
    fn every_state_has_its_own_bit() {
        for phase in EVERY_PHASE {
            let one = DealerScenarioCheckpointAdmissionV1::states(&[phase]);
            let admitted = EVERY_PHASE
                .iter()
                .filter(|other| one.admits(**other))
                .count();
            assert_eq!(admitted, 1, "{phase:?} aliases another bit");
            assert!(one.admits(phase));
        }
        for status in EVERY_STATUS {
            let one = DealerScenarioReservationAdmissionV1::states(&[status]);
            let admitted = EVERY_STATUS
                .iter()
                .filter(|other| one.admits(**other))
                .count();
            assert_eq!(admitted, 1, "{status:?} aliases another bit");
            assert!(one.admits(status));
        }
    }

    #[test]
    fn the_empty_set_admits_nothing() {
        assert!(DealerScenarioCheckpointAdmissionV1::NONE.is_empty());
        assert!(DealerScenarioCheckpointAdmissionV1::states(&[]).is_empty());
        assert!(DealerScenarioReservationAdmissionV1::NONE.is_empty());
        assert!(DealerScenarioReservationAdmissionV1::states(&[]).is_empty());
        for phase in EVERY_PHASE {
            assert!(!DealerScenarioCheckpointAdmissionV1::NONE.admits(phase));
        }
        for status in EVERY_STATUS {
            assert!(!DealerScenarioReservationAdmissionV1::NONE.admits(status));
        }
    }

    /// The bit index is the discriminant the decoder reads, not a second
    /// numbering. Both machines number from one, so bit zero stays vacant.
    #[test]
    fn the_bit_index_is_the_wire_tag() {
        for (position, phase) in EVERY_PHASE.into_iter().enumerate() {
            assert_eq!(u8::try_from(position).unwrap() + 1, phase as u8);
        }
        for (position, status) in EVERY_STATUS.into_iter().enumerate() {
            assert_eq!(u8::try_from(position).unwrap() + 1, status as u8);
        }
    }

    /// Every set this file declares, checked against the exact boolean that
    /// stood at the guard site before it was named.
    ///
    /// This is the behaviour proof, and it is a check rather than an
    /// assertion: each closure is the condition the program executed, and the
    /// loop runs it against every phase the machine has.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        let checkpoint_cases: [(
            &str,
            DealerScenarioCheckpointAdmissionV1,
            fn(DealerScenarioCheckpointPhaseV1) -> bool,
        ); 6] = [
            (
                "require_live_collecting",
                DEALER_SCENARIO_COLLECTING_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| phase == DealerScenarioCheckpointPhaseV1::Collecting,
            ),
            (
                "append_reservation",
                DEALER_SCENARIO_RESERVE_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| phase == DealerScenarioCheckpointPhaseV1::Evaluated,
            ),
            (
                "append_rollback",
                DEALER_SCENARIO_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| {
                    matches!(
                        phase,
                        DealerScenarioCheckpointPhaseV1::Evaluated
                            | DealerScenarioCheckpointPhaseV1::Reserved
                            | DealerScenarioCheckpointPhaseV1::RollingBack
                    )
                },
            ),
            (
                "custody rollback arm",
                DEALER_SCENARIO_CUSTODY_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| {
                    matches!(
                        phase,
                        DealerScenarioCheckpointPhaseV1::Reserved
                            | DealerScenarioCheckpointPhaseV1::RollingBack
                    )
                },
            ),
            (
                "admit_commit",
                DEALER_SCENARIO_COMMIT_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| phase == DealerScenarioCheckpointPhaseV1::Reserved,
            ),
            (
                "cleanup_beneficiary",
                DEALER_SCENARIO_CLEANUP_CHECKPOINT_ADMISSIBLE_STATES_V1,
                |phase| phase != DealerScenarioCheckpointPhaseV1::Committed,
            ),
        ];
        for (name, declared, stood) in checkpoint_cases {
            for phase in EVERY_PHASE {
                assert_eq!(
                    declared.admits(phase),
                    stood(phase),
                    "{name} moved at {phase:?}"
                );
            }
        }
        for status in EVERY_STATUS {
            assert_eq!(
                DEALER_SCENARIO_ACTIVE_RESERVATION_ADMISSIBLE_STATES_V1.admits(status),
                status == DealerScenarioReservationStateStatusV1::Active,
                "active reservation moved at {status:?}"
            );
        }
    }

    /// Custody's rollback prestates are a subset of the codec's.
    ///
    /// The pinning the pair needs: narrowing the codec's set below Custody's
    /// would leave a Custody route unreachable in a phase its own program
    /// still admits, and nothing else in the tree would have gone red.
    #[test]
    fn the_custody_rollback_set_is_contained_in_the_codec_one() {
        for phase in EVERY_PHASE {
            if DEALER_SCENARIO_CUSTODY_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase) {
                assert!(
                    DEALER_SCENARIO_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase),
                    "{phase:?} is a Custody rollback prestate the codec refuses"
                );
            }
        }
        assert!(
            DEALER_SCENARIO_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1
                .admits(DealerScenarioCheckpointPhaseV1::Evaluated)
        );
        assert!(
            !DEALER_SCENARIO_CUSTODY_ROLLBACK_CHECKPOINT_ADMISSIBLE_STATES_V1
                .admits(DealerScenarioCheckpointPhaseV1::Evaluated)
        );
    }

    /// The ladder is ordered and its steps are disjoint where they must be.
    #[test]
    fn the_ladder_steps_do_not_overlap() {
        for phase in EVERY_PHASE {
            assert!(
                !(DEALER_SCENARIO_COLLECTING_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)
                    && DEALER_SCENARIO_RESERVE_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)),
                "{phase:?} both collects and reserves"
            );
            assert!(
                !(DEALER_SCENARIO_COMMIT_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)
                    && DEALER_SCENARIO_COMMITTED_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)),
                "{phase:?} both admits a commit and is already committed"
            );
            assert!(
                !(DEALER_SCENARIO_CLEANUP_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)
                    && DEALER_SCENARIO_COMMITTED_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(phase)),
                "{phase:?} is cleaned up after committing"
            );
        }
    }
}
