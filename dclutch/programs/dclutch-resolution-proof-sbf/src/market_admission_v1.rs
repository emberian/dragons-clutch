//! Named admissible Market prestates for Resolution's Core phase guards.
//!
//! Resolution reads the Core Market's `(phase, readiness)` on five guards and
//! the route census enumerated ZERO gates over its twenty-nine routes, because
//! every one of them was written inline inside a ten-conjunct disjunction --
//! `state.phase != CorePhase::Open || state.readiness !=
//! CoreReadiness::Consumed || ..` -- where nothing outside the function can
//! read it.
//!
//! [`MarketAdmissionV1`] is the vocabulary Core, Custody and Claims publish
//! (`315f1931`, `20a45ea1`, `9438c8a1`), and the constant *is* the check.
//!
//! Resolution's guards name the READINESS as well as the phase, so these are
//! exact `prestates` sets rather than phase projections: `Open` alone would
//! silently admit `Open + Prepaid` and `Open + Ready`, which every one of
//! these routes refuses.
//!
//! The set is a NECESSARY condition and never a sufficient one: a route
//! admitted by its prestate still authenticates its accounts, its release set,
//! its activation and its source records. A consumer may refuse an act because
//! its prestate is excluded; nothing may call an act ready because it is not.

// `Phase` and `Readiness` are imported unaliased on purpose: the route census
// reads these initializers structurally and checks that each variant names its
// own enum, and a local alias -- which the rest of this program uses, as
// `CorePhase`/`CoreReadiness` -- is unreadable to a scan that cannot resolve
// types, so it reports the constant as unclassified rather than guessing.
use dclutch_market::{MarketAdmissionV1, Phase, Readiness};
use dclutch_source::{SourceAdmissionV1, SourceResolutionPhaseV1};

/// A Market that has opened and consumed its founding readiness.
///
/// The live evidence routes: provider submission, provider execution, and the
/// relayed-record transport all authenticate a Market that is trading, and
/// `Open + Prepaid` or `Open + Ready` is a Market mid-transition whose
/// founding is not finished.
pub(crate) const RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[(Phase::Open, Readiness::Consumed)]);

/// A Market whose founding fund may still be created or verified.
///
/// `Founding + Prepaid` is the readiness ladder. `Open + Consumed` is the
/// atomic founding, whose commit-last `Open` goes from the first straight to
/// the second in one transition and therefore never passes the ladder; without
/// it every atomically founded Market is permanently unresolvable, because
/// these are the only routes that create a `SourceResolutionStateV2`.
///
/// Deferring the Source state's physical creation past `Open` defers no
/// decision: the manifest is a seed of the Market address and the Resolution
/// subset ledger was already initialized before Market Found. These routes
/// only consume that immutable authority.
pub(crate) const RESOLUTION_FUND_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[
        (Phase::Founding, Readiness::Prepaid),
        (Phase::Open, Readiness::Consumed),
    ]);

/// A Market whose retirement has begun: the fund close.
pub(crate) const RESOLUTION_CLOSE_FUND_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[(Phase::Retiring, Readiness::Consumed)]);

// --------------------------------------------------------- the Source machine

/// The Source resolution states in which a sponsored push may be attempted.
///
/// A SECOND state machine, and its own type on purpose. The Market's phase
/// cannot answer this question: a Market is `Open` for the whole span in which
/// its Source moves `Primary` -> `Recovery` -> `Resolved`, so a client reading
/// only the Market gate would report a sponsored capture as admissible on a
/// Source that has already resolved. Every sponsored route -- capture, settle
/// and the commit-failure walk -- admits `Primary` and nothing else.
///
/// It is Primary-only because the sponsored transport has no recovery
/// producer, not because a rung is unanswerable. That distinction used to be
/// invisible: this set was ALSO the complement of the reclaim set, so
/// "sponsored push admits only the primary" and "only a primary market can
/// consume a submission" were one constant saying two things. They are two
/// constants now, and the second one is
/// [`RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1`].
pub(crate) const RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1: SourceAdmissionV1 =
    SourceAdmissionV1::states(&[SourceResolutionPhaseV1::Primary]);

/// The Source resolution states in which a provider submission may be made and
/// CONSUMED.
///
/// `Recovery` belongs here, and until the ladder had a capture producer it did
/// not: a market standing on a funded rung is a market that can still be
/// answered, by the alternative source it paid for, until that attempt's own
/// committed deadline. Every clause of the reclaim hazard applies to it exactly
/// as it applies to a primary market -- a submission a Source can still consume
/// is a market's answer, and letting a stranger close it for a transaction fee
/// is how holders get the pre-disclosed failure outcome instead of the real
/// one.
pub(crate) const RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1: SourceAdmissionV1 =
    SourceAdmissionV1::states(&[
        SourceResolutionPhaseV1::Primary,
        SourceResolutionPhaseV1::Recovery,
    ]);

/// The Source resolution states in which a provider submission may be
/// RECLAIMED, which is the complement of the one above.
///
/// Written out rather than derived from a `not`, because these constants are
/// read by a census that evaluates no operators -- but pinned against the
/// other by `the_reclaim_set_is_exactly_the_complement_of_the_capture_set`, so
/// the two cannot drift into overlapping or into leaving a state unclaimed.
///
/// The distinction is load-bearing and was nearly published backwards. The
/// guard here is `source_can_no_longer_consume`, whose sense is INVERTED
/// against every other Source guard in this program: reclaim requires that the
/// Source can no longer consume, so declaring the capture set at this site
/// would have told a client the reclaim route admits exactly the one state it
/// refuses.
pub(crate) const RESOLUTION_RECLAIMABLE_SOURCE_ADMISSIBLE_STATES_V1: SourceAdmissionV1 =
    SourceAdmissionV1::states(&[
        SourceResolutionPhaseV1::Resolved,
        SourceResolutionPhaseV1::Exhausted,
        SourceResolutionPhaseV1::FailureCommitted,
        SourceResolutionPhaseV1::Retired,
    ]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_PHASE: [Phase; 5] = [
        Phase::Founding,
        Phase::Open,
        Phase::Terminal,
        Phase::Retiring,
        Phase::Retired,
    ];
    const EVERY_READINESS: [Readiness; 3] =
        [Readiness::Prepaid, Readiness::Ready, Readiness::Consumed];

    /// Assert a constant agrees with the inline expression it replaced over
    /// every one of the fifteen prestates.
    ///
    /// The inline expressions are written out here as they stood at the guard
    /// before the constant existed, so this is a behaviour-identity check and
    /// not a restatement of the constant.
    fn agrees(name: &str, declared: MarketAdmissionV1, inline: impl Fn(Phase, Readiness) -> bool) {
        for phase in EVERY_PHASE {
            for readiness in EVERY_READINESS {
                assert_eq!(
                    declared.admits(phase, readiness),
                    inline(phase, readiness),
                    "{name} disagrees with the guard it replaced at {phase:?}/{readiness:?}"
                );
            }
        }
    }

    #[test]
    fn admissible_prestates() {
        // provider_transport_v3::authenticate_current_submission,
        // provider_instruction_v3::authenticate_market_and_infrastructure,
        // relay_transport_v1::authenticate_market, and core_effect's
        // AdmitTerminal arm:
        // `phase != CorePhase::Open || readiness != CoreReadiness::Consumed`.
        agrees(
            "RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1",
            RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| phase == Phase::Open && readiness == Readiness::Consumed,
        );
        // core_effect::authenticate_direct_market and the
        // `CreateFund | VerifyFundReady` arm of core_effect::authenticate_core:
        // `!matches!((phase, readiness), (Founding, Prepaid) | (Open, Consumed))`.
        agrees(
            "RESOLUTION_FUND_ADMISSIBLE_PRESTATES_V1",
            RESOLUTION_FUND_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| {
                matches!(
                    (phase, readiness),
                    (Phase::Founding, Readiness::Prepaid) | (Phase::Open, Readiness::Consumed)
                )
            },
        );
        // core_effect::authenticate_direct_close_market and the CloseFund arm
        // of core_effect::authenticate_core:
        // `phase != CorePhase::Retiring || readiness != CoreReadiness::Consumed`.
        agrees(
            "RESOLUTION_CLOSE_FUND_ADMISSIBLE_PRESTATES_V1",
            RESOLUTION_CLOSE_FUND_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| phase == Phase::Retiring && readiness == Readiness::Consumed,
        );
    }

    /// The Source set agrees with the guards it replaced, over all six states.
    #[test]
    fn admissible_source_states() {
        for state in [
            SourceResolutionPhaseV1::Primary,
            SourceResolutionPhaseV1::Recovery,
            SourceResolutionPhaseV1::Resolved,
            SourceResolutionPhaseV1::Exhausted,
            SourceResolutionPhaseV1::FailureCommitted,
            SourceResolutionPhaseV1::Retired,
        ] {
            // sponsored_push_v1 process_capture, process_settle and
            // process_commit_failure: `phase() != SourceResolutionPhaseV1::Primary`.
            assert_eq!(
                RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                state == SourceResolutionPhaseV1::Primary,
                "the Source set disagrees with the guard it replaced at {state:?}"
            );
            // provider_transport_v3 process_submit and provider_v3
            // select_rung: a submission may be made and consumed on the two
            // states in which this market can still be answered honestly.
            assert_eq!(
                RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                matches!(
                    state,
                    SourceResolutionPhaseV1::Primary | SourceResolutionPhaseV1::Recovery
                ),
                "the capture set disagrees with the routes it gates at {state:?}"
            );
            // provider_transport_v3 source_can_no_longer_consume, whose sense
            // is the inverse: the RECLAIM condition is returned rather than
            // checked as a refusal.
            assert_eq!(
                RESOLUTION_RECLAIMABLE_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                !matches!(
                    state,
                    SourceResolutionPhaseV1::Primary | SourceResolutionPhaseV1::Recovery
                ),
                "the reclaim set disagrees with the guard it replaced at {state:?}"
            );
        }
    }

    /// The two Source sets partition the machine, and that is checked.
    ///
    /// One is written as a list and the other as the complement of the same
    /// list, so nothing but this makes them stay opposite. An overlap would
    /// publish a state as both capturable and reclaimable; a gap would leave a
    /// state no route claims, and the census would report both as ordinary.
    #[test]
    fn the_reclaim_set_is_exactly_the_complement_of_the_capture_set() {
        for state in [
            SourceResolutionPhaseV1::Primary,
            SourceResolutionPhaseV1::Recovery,
            SourceResolutionPhaseV1::Resolved,
            SourceResolutionPhaseV1::Exhausted,
            SourceResolutionPhaseV1::FailureCommitted,
            SourceResolutionPhaseV1::Retired,
        ] {
            assert_ne!(
                RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                RESOLUTION_RECLAIMABLE_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                "{state:?} is in both Source sets or in neither"
            );
            // And the sponsored set is a SUBSET of the capturable one rather
            // than its equal, which is the fact the two constants were merged
            // over: a rung is capturable and is not sponsored-pushable.
            assert!(
                !RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1.admits(state)
                    || RESOLUTION_CAPTURABLE_SOURCE_ADMISSIBLE_STATES_V1.admits(state),
                "{state:?} is sponsored-pushable and not capturable"
            );
        }
    }

    /// Every one of these sets is exact in the readiness axis.
    ///
    /// A phase projection would be a silent widening here, and this is the
    /// check that nobody rewrites one of them as `phases(&[..])` on the theory
    /// that a phase is what a phase gate names.
    #[test]
    fn no_set_admits_a_phase_under_every_readiness() {
        for set in [
            RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1,
            RESOLUTION_FUND_ADMISSIBLE_PRESTATES_V1,
            RESOLUTION_CLOSE_FUND_ADMISSIBLE_PRESTATES_V1,
        ] {
            for phase in EVERY_PHASE {
                let admitted = EVERY_READINESS
                    .iter()
                    .filter(|readiness| set.admits(phase, **readiness))
                    .count();
                assert!(
                    admitted <= 1,
                    "{phase:?} is admitted under {admitted} readiness values"
                );
            }
        }
    }
}
