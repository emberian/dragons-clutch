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
use dclutch_market_core_codec::{MarketAdmissionV1, Phase, Readiness};

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
