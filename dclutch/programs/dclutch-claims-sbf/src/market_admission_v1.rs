//! Named admissible Market prestates for Claims' Core phase guards.
//!
//! Claims reads the Core Market's `phase` on nineteen routes and, until this
//! module, said so in three different vocabularies: a `CorePhaseGateV3`
//! threaded through six signatures as a parameter, a bare
//! `core.phase != Phase::Open` inside a fourteen-conjunct `||` chain, and
//! a `matches!(core.phase, Terminal | Retiring)` with the argument written out
//! in a comment. None of the three is readable from outside the function that
//! computes it, so the route census enumerated `claims` as having zero phase
//! gates over thirty-three routes and `/workbench` reported READY TO PREFLIGHT
//! for acts a Founding Market refuses on sight.
//!
//! [`MarketAdmissionV1`] is the vocabulary Core and Custody already publish
//! (`315f1931`, `20a45ea1`), and the constant *is* the check: a consumer
//! reading it reads the conjunct the program executes, not a second author's
//! account of it.
//!
//! ## Named for the set, not for the routes
//!
//! There are four sets across nineteen Claims routes, and they are named for
//! what they admit rather than for who uses them -- the rule `CorePhaseGateV3`
//! already argued for itself, so that a future phase cannot join a set by the
//! name staying plausible. Five settlement routes sharing one constant is one
//! semantic owner for one fact, not five copies of it drifting apart.
//!
//! The set is a NECESSARY condition and never a sufficient one: a route
//! admitted by its prestate still authenticates its accounts, its release set,
//! its Product graph and its aggregate. A consumer may refuse an act because
//! its prestate is excluded; nothing may call an act ready because it is not.

// `Phase` is imported unaliased on purpose. The route census reads these
// initializers structurally and checks that each variant names `Phase`; a
// local alias is legitimate Rust and unreadable to a scan that cannot resolve
// types, so it reports the constant as unclassified rather than guessing --
// which is how this module's first draft, written `Phase as CorePhase`, left
// four constants out of the census while every test stayed green.
use dclutch_market_core_codec::{MarketAdmissionV1, Phase};

/// A Market open for trading: the live-supply routes.
///
/// Affine batches, signed deltas, the atomic whole-unwrap open, protocol
/// position admission, sparse native transfer, the non-redeeming
/// representation actions and lifecycle activation all name exactly this.
pub(crate) const CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Open]);

/// A Market still founding: the routes that mint the first complete set.
///
/// `founding_v5` and the foundational split both run before any supply
/// exists, and both refuse a Market that has already opened.
pub(crate) const CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Founding]);

/// A resolved Market, whether or not retirement has begun.
///
/// Redemption and settlement name two phases, and must: Core's
/// `begin_retiring` is permissionless and refuses all signers
/// (`programs/dclutch-core-sbf/src/begin_retiring.rs:57`), so while every
/// redemption route demanded exact equality with `Phase::Terminal`, any
/// stranger could end every holder's redemption right for one transaction fee
/// -- and brick the Market doing it, because retirement needs zero outstanding
/// supply (`market_closure_v1.rs:669-681`) and redemption is the only thing
/// that drives supply to zero.
///
/// Two-phase tolerance is the documented intent rather than a relaxation. The
/// transition's own codec doc reads "Begin retiring while retaining
/// permissionless redemption"
/// (`crates/dclutch-market-core-codec/src/generated.rs:1030`), it moves `phase`
/// and nothing else -- `terminal_winner` and `terminal_receipt` both survive
/// it (`:1041-1047`) -- and `phases_join` already admits
/// `(Phase::Retiring, EconomicPhase::Retiring(w))`
/// (`crate::phases_join`, `lib.rs:1204-1213`).
///
/// Widening the phase widens nothing else. `Retiring` is reachable only from
/// `Terminal` and carries the same `terminal_winner`, and the payout
/// derivation independently refuses a coordinate whose supply or balance is
/// already drained (`product_basis_terminal_v3.rs:416-424`), so a holder
/// admitted here is paid exactly what the same holder was owed one phase
/// earlier, once.
///
/// `Retired` stays out: by then `market_closure_v1` has demanded zero
/// outstanding supply, so there is nothing left to redeem.
pub(crate) const CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Terminal, Phase::Retiring]);

/// A Market whose retirement has begun, and only that.
///
/// Market closure and lifecycle deactivation are the two routes that run
/// *during* retirement rather than up to it, and neither admits `Terminal`:
/// closure demands the retirement Core already entered, and deactivation
/// tears down what activation built.
pub(crate) const CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Retiring]);

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
    /// every one of the fifteen `(Phase, Readiness)` prestates.
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
        // affine_batch_v2, signed_delta_v3, fractional_atomic_v3,
        // protocol_position_v2, sparse_native_transfer_v1,
        // rational_product_v3 (non-redeeming), rational_lifecycle_v2
        // (activating): `phase_gate.admits(core.phase)` with
        // `CorePhaseGateV3::Exactly(Phase::Open)`, and
        // `core.phase != Phase::Open`.
        agrees(
            "CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1",
            CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| phase == Phase::Open,
        );
        // founding_v5: `CorePhaseGateV3::Exactly(Phase::Founding)`.
        agrees(
            "CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1",
            CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| phase == Phase::Founding,
        );
        // terminal_settlement_v3, rational_terminal_v3,
        // fractional_retirement_v3, fractional_claim_check_v1,
        // rational_product_v3 (RedeemTerminal):
        // `CorePhaseGateV3::TerminalOrRetiring.admits(phase)`.
        // claim_check_compaction_v1:
        // `matches!(core.phase, Phase::Terminal | Phase::Retiring)`.
        agrees(
            "CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1",
            CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| matches!(phase, Phase::Terminal | Phase::Retiring),
        );
        // market_closure_v1: `core.phase != Phase::Retiring`.
        // rational_lifecycle_v2 (deactivating): `core.phase != expected_phase`
        // with `expected_phase = Phase::Retiring`.
        agrees(
            "CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1",
            CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase| phase == Phase::Retiring,
        );
    }

    #[test]
    fn the_settled_set_is_exactly_the_pair_the_claims_phase_model_joins() {
        // The set is not an independent opinion about which phases redeem. It
        // is the Core half of `phases_join`'s two winner-bearing pairs
        // (`crate::phases_join`), which is the Claims phase model's own
        // statement that a resolved Market keeps its winner across
        // `begin_retiring`. If someone narrows the join, this fails rather than
        // leaving redemption admitting a phase the model no longer joins.
        for phase in EVERY_PHASE {
            let joins_a_winner_bearing_claims_phase =
                crate::phases_join(phase, 7, dclutch_economic_slice_kernel::Phase::Terminal(7))
                    || crate::phases_join(
                        phase,
                        7,
                        dclutch_economic_slice_kernel::Phase::Retiring(7),
                    );
            assert_eq!(
                CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(phase),
                joins_a_winner_bearing_claims_phase,
                "the redemption set and the phase model disagree on {phase:?}"
            );
        }
    }

    #[test]
    fn the_four_sets_are_distinct_and_none_admits_retired() {
        let sets = [
            CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
            CLAIMS_FOUNDING_MARKET_ADMISSIBLE_PRESTATES_V1,
            CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1,
            CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1,
        ];
        for (index, left) in sets.iter().enumerate() {
            assert!(
                !left.admits_phase(Phase::Retired),
                "set {index} admits a Retired Market"
            );
            for (other, right) in sets.iter().enumerate() {
                assert_eq!(index == other, left == right, "sets {index} and {other}");
            }
        }
    }
}
