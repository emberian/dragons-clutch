//! Named admissible Market prestates for one route's phase guard.
//!
//! A route that reads `state.phase` is answering one question: *may this
//! Market be asked to do this now?* Written inline as
//! `state.phase != Phase::Open`, the answer has no name, so nothing outside
//! the function can read it: not the route census, not a reference page, and
//! not a client deciding whether to bother building the transaction. Every
//! such consumer then re-derives the answer from prose about a refusal code,
//! which is how `evaluateCapabilityV1` came to report READY TO PREFLIGHT for
//! an act the chain would refuse on sight.
//!
//! This type gives that answer a name at the guard, and only at the guard. The
//! constant *is* the check — a consumer reading it is reading the conjunct the
//! program executes, not a second author's account of it.
//!
//! The set is over `(Phase, Readiness)` pairs rather than phases alone because
//! that is what the guards actually constrain: `VerifyFundReady` admits
//! `Founding + Prepaid`, `Founding + Ready` and `Open + Consumed`, and a
//! declaration that could only say `{Founding, Open}` would silently widen it.
//! [`MarketAdmissionV1::admits_phase`] is the projection a phase-only guard
//! and a phase-only consumer need, derived from the same constant rather than
//! written beside it.

use crate::{
    PHASE_FOUNDING_TAG, PHASE_OPEN_TAG, PHASE_RETIRED_TAG, PHASE_RETIRING_TAG, PHASE_TERMINAL_TAG,
    Phase, READINESS_CONSUMED_TAG, READINESS_PREPAID_TAG, READINESS_READY_TAG, Readiness,
};

/// Number of distinct `Phase` values.
const PHASE_COUNT: u16 = 5;
/// Number of distinct `Readiness` values.
const READINESS_COUNT: u16 = 3;

/// The wire tag of a phase, as a bit-index component.
const fn phase_tag(phase: Phase) -> u16 {
    match phase {
        Phase::Founding => PHASE_FOUNDING_TAG as u16,
        Phase::Open => PHASE_OPEN_TAG as u16,
        Phase::Terminal => PHASE_TERMINAL_TAG as u16,
        Phase::Retiring => PHASE_RETIRING_TAG as u16,
        Phase::Retired => PHASE_RETIRED_TAG as u16,
    }
}

/// The wire tag of a readiness, as a bit-index component.
const fn readiness_tag(readiness: Readiness) -> u16 {
    match readiness {
        Readiness::Prepaid => READINESS_PREPAID_TAG as u16,
        Readiness::Ready => READINESS_READY_TAG as u16,
        Readiness::Consumed => READINESS_CONSUMED_TAG as u16,
    }
}

/// Every `(phase, readiness)` pair occupies its own bit of a `u16`, so the
/// widest index this file can produce must fit. The tags are Lean-emitted, so
/// this is the check that a phase or readiness added upstream cannot silently
/// alias an existing pair's bit.
const _: () = assert!(PHASE_COUNT * READINESS_COUNT <= 16);
const _: () = assert!(phase_tag(Phase::Retired) < PHASE_COUNT);
const _: () = assert!(readiness_tag(Readiness::Consumed) < READINESS_COUNT);

/// Bit index of one prestate.
const fn index(phase: Phase, readiness: Readiness) -> u16 {
    phase_tag(phase) * READINESS_COUNT + readiness_tag(readiness)
}

/// The `(Phase, Readiness)` prestates in which one route is admissible.
///
/// This is a NECESSARY condition and never a sufficient one: a route admitted
/// by its prestate still authenticates its accounts, its release set, its
/// request and its child acknowledgement. A consumer may refuse an act because
/// its prestate is excluded; nothing may call an act ready because it is not.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MarketAdmissionV1 {
    prestates: u16,
}

impl MarketAdmissionV1 {
    /// The empty admission: no prestate at all.
    pub const NONE: Self = Self { prestates: 0 };

    /// Admit exactly the listed `(phase, readiness)` prestates.
    #[must_use]
    pub const fn prestates(pairs: &[(Phase, Readiness)]) -> Self {
        let mut prestates = 0u16;
        let mut rest = pairs;
        while let [(phase, readiness), tail @ ..] = rest {
            prestates |= 1u16 << index(*phase, *readiness);
            rest = tail;
        }
        Self { prestates }
    }

    /// Admit the listed phases under every readiness.
    ///
    /// The guards that use this are the ones whose written condition names no
    /// readiness at all: `state.phase != Phase::Retiring`. Declaring the wider
    /// set is the accurate reading of such a guard, not a weakening of it.
    #[must_use]
    pub const fn phases(phases: &[Phase]) -> Self {
        let mut prestates = 0u16;
        let mut rest = phases;
        while let [phase, tail @ ..] = rest {
            prestates |= 1u16 << index(*phase, Readiness::Prepaid);
            prestates |= 1u16 << index(*phase, Readiness::Ready);
            prestates |= 1u16 << index(*phase, Readiness::Consumed);
            rest = tail;
        }
        Self { prestates }
    }

    /// Whether this exact prestate is admitted.
    #[must_use]
    pub const fn admits(self, phase: Phase, readiness: Readiness) -> bool {
        self.prestates & (1u16 << index(phase, readiness)) != 0
    }

    /// Whether any readiness in `phase` is admitted.
    ///
    /// The phase projection of the set, derived here so a phase-only guard and
    /// a phase-only reader share the declaration with the exact one.
    #[must_use]
    pub const fn admits_phase(self, phase: Phase) -> bool {
        self.admits(phase, Readiness::Prepaid)
            || self.admits(phase, Readiness::Ready)
            || self.admits(phase, Readiness::Consumed)
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.prestates == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PHASES: [Phase; 5] = [
        Phase::Founding,
        Phase::Open,
        Phase::Terminal,
        Phase::Retiring,
        Phase::Retired,
    ];
    const ALL_READINESS: [Readiness; 3] =
        [Readiness::Prepaid, Readiness::Ready, Readiness::Consumed];

    #[test]
    fn every_prestate_has_its_own_bit() {
        for phase in ALL_PHASES {
            for readiness in ALL_READINESS {
                let one = MarketAdmissionV1::prestates(&[(phase, readiness)]);
                let mut admitted = 0;
                for other_phase in ALL_PHASES {
                    for other_readiness in ALL_READINESS {
                        if one.admits(other_phase, other_readiness) {
                            admitted += 1;
                        }
                    }
                }
                assert_eq!(admitted, 1, "{phase:?}/{readiness:?} aliases another bit");
                assert!(one.admits(phase, readiness));
            }
        }
    }

    #[test]
    fn phases_admits_every_readiness_and_nothing_else() {
        let set = MarketAdmissionV1::phases(&[Phase::Retiring]);
        for readiness in ALL_READINESS {
            assert!(set.admits(Phase::Retiring, readiness));
        }
        for phase in ALL_PHASES {
            assert_eq!(set.admits_phase(phase), phase == Phase::Retiring);
        }
    }

    #[test]
    fn phase_projection_is_the_union_over_readiness() {
        let set = MarketAdmissionV1::prestates(&[
            (Phase::Founding, Readiness::Prepaid),
            (Phase::Open, Readiness::Consumed),
        ]);
        assert!(set.admits_phase(Phase::Founding));
        assert!(set.admits_phase(Phase::Open));
        assert!(!set.admits(Phase::Founding, Readiness::Consumed));
        assert!(!set.admits_phase(Phase::Terminal));
        assert!(!set.admits_phase(Phase::Retiring));
        assert!(!set.admits_phase(Phase::Retired));
    }

    #[test]
    fn the_empty_set_admits_nothing() {
        assert!(MarketAdmissionV1::NONE.is_empty());
        assert!(MarketAdmissionV1::prestates(&[]).is_empty());
        for phase in ALL_PHASES {
            assert!(!MarketAdmissionV1::NONE.admits_phase(phase));
        }
    }
}
