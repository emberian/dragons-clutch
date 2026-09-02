//! Named admissible Market prestates for Trading's Core phase guards.
//!
//! Trading authenticates the Core Market on nine guards spread over the Direct
//! venue and the Dealer accelerators, and the route census enumerated ZERO
//! gates over its twenty-eight routes, because every one is written inline
//! inside a disjunction that also joins market ids, generations, release sets
//! and digests -- a shape nothing outside the function can read.
//!
//! [`MarketAdmissionV1`] is the vocabulary Core, Custody, Claims and
//! Resolution publish (`315f1931`, `20a45ea1`, `9438c8a1`, `f47c25fe`), and
//! the constant *is* the check.
//!
//! These sets are phase projections rather than exact prestates because the
//! guards they replace name no readiness at all. Declaring the wider set is
//! the accurate reading of such a guard, not a weakening of it, and
//! [`MarketAdmissionV1::admits_phase`] is the projection derived from the same
//! declaration rather than written beside it.
//!
//! The set is a NECESSARY condition and never a sufficient one: a route
//! admitted by its prestate still authenticates its accounts, its release set,
//! its root and its request. A consumer may refuse an act because its prestate
//! is excluded; nothing may call an act ready because it is not.

// `Phase` is imported unaliased on purpose: the route census reads these
// initializers structurally and checks that each variant names `Phase`, and a
// local alias -- which the Dealer modules use, as `CorePhase` -- is
// unreadable to a scan that cannot resolve types, so it reports the constant
// as unclassified rather than guessing.
use dclutch_market_core_codec::{MarketAdmissionV1, Phase};

/// A Market open for trading.
///
/// Every route that creates, funds or moves a live position: Direct token
/// setup, Direct replay setup, the inline Direct fill, both complementary
/// legs, and the Dealer scenario and equity accelerators' Core join.
pub const TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Open]);

/// A Market whose retirement has begun.
///
/// Direct's own retirement transition and the maker close that follows it both
/// refuse a Market that has not entered `Retiring` -- and, unlike Claims'
/// settlement routes, refuse `Terminal` as well, because both of these routes
/// exist to tear down what retirement has already started.
pub const TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Retiring]);

/// A Market that has opened, whether or not it has since resolved.
///
/// The Direct escrow contexts' TERMINAL arm. An escrow opened while the Market
/// traded must still be closable after the Market resolves, or the escrowed
/// principal is stranded by the resolution it was posted against; the
/// non-terminal arm of the same guard names
/// [`TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1`] and is the narrower set.
/// `Founding` stays out because no escrow can exist yet, and `Retired` stays
/// out because closure has already demanded that none does.
pub const TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Open, Phase::Terminal, Phase::Retiring]);

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market_core_codec::Readiness;

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
    fn agrees(name: &str, declared: MarketAdmissionV1, inline: impl Fn(Phase) -> bool) {
        for phase in EVERY_PHASE {
            for readiness in EVERY_READINESS {
                assert_eq!(
                    declared.admits(phase, readiness),
                    inline(phase),
                    "{name} disagrees with the guard it replaced at {phase:?}/{readiness:?}"
                );
            }
        }
    }

    #[test]
    fn admissible_prestates() {
        // direct_token_setup_v1, direct_replay_setup_v1:
        // `state.phase != Phase::Open`. direct/inline, direct/complementary
        // (both legs): `core_market.phase() != Phase::Open`.
        // dealer/v3_accelerator_accounts, dealer/v4_equity_accelerator_accounts:
        // `core.phase != CorePhase::Open`. Both escrow contexts'
        // non-terminal arm: `core_market.phase() == Phase::Open`.
        agrees(
            "TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1",
            TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| phase == Phase::Open,
        );
        // direct_begin_retiring_v1, direct_close_maker_v1:
        // `state.phase != Phase::Retiring`.
        agrees(
            "TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1",
            TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| phase == Phase::Retiring,
        );
        // direct/buy_escrow, direct/sell_escrow, terminal arm:
        // `matches!(core_market.phase(), Phase::Open | Phase::Terminal | Phase::Retiring)`.
        agrees(
            "TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1",
            TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| matches!(phase, Phase::Open | Phase::Terminal | Phase::Retiring),
        );
    }

    /// The escrow's terminal arm is a strict widening of its own other arm.
    ///
    /// The two arms are one guard written as a selection, and the whole reason
    /// the terminal arm exists is that an escrow posted while the Market traded
    /// must survive the Market resolving. If someone narrows the terminal set
    /// below the open one, this fails rather than leaving an escrow reachable
    /// in a phase its own opening was not.
    #[test]
    fn the_terminal_escrow_arm_contains_the_open_one() {
        for phase in EVERY_PHASE {
            if TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(phase) {
                assert!(
                    TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(phase),
                    "the terminal escrow arm refuses {phase:?}, which the open arm admits"
                );
            }
        }
        assert!(!TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(Phase::Founding));
        assert!(!TRADING_OPENED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(Phase::Retired));
    }

    /// Retirement and trading are disjoint, and neither admits a dead Market.
    #[test]
    fn the_retiring_set_shares_no_phase_with_the_open_one() {
        for phase in EVERY_PHASE {
            assert!(
                !(TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(phase)
                    && TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(phase)),
                "{phase:?} is admitted by both the open and the retiring set"
            );
        }
        assert!(!TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(Phase::Retired));
        assert!(!TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(Phase::Retired));
    }
}
