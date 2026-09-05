//! Named admissible states for the Dealer root's own lifecycle.
//!
//! [`DealerRootPhaseV1`] is the phase [`crate::dealer::root_tail::RootTail`] persists,
//! and nine guards read it: seven transitions in this crate and two account
//! authentications in Trading's V4 accelerators. Written inline as
//! `state.phase != Phase::Open` not one of them had a name a reader outside
//! the function could reach, so the census reported every route that drives a
//! Dealer as gated on the Market alone.
//!
//! ## `Open` is a word three machines in this tree use
//!
//! The Core Market has a `Phase::Open`, the Direct root has a
//! `DirectRootPhaseV1::Open`, and this is a third, and they are not the same
//! fact:
//!
//! * A Market is `Open` for the whole span in which a Dealer moves `Open` ->
//!   `Terminal` -> `Retired`. `enter_terminal` is driven by the MARKET
//!   (`request.actor_id != policy.market_id` refuses anyone else), so the two
//!   discriminants move on the same transaction and still disagree afterwards:
//!   a resolved Market whose Dealer has not yet unwound is `Terminal` on both,
//!   and a retired Dealer under a still-open Market is neither.
//! * The Direct root's `Open` is about NONCE ADMISSION in a different account
//!   under a different instruction set, and its `Retiring` has no counterpart
//!   here at all.
//!
//! The alias in this module is the repair for that: a set is indexed by a
//! discriminant, so its declaration has to name WHICH ONE, and a constant
//! written `Phase::Open` names a word rather than a machine. The census reads
//! `DealerRootPhaseV1` and refuses a bare `Phase` variant in this machine's
//! sets, which is the same rule that stops a Source state being passed to
//! `MarketAdmissionV1::phases`.
//!
//! ## The tags are Lean's, not the enum's
//!
//! Unlike the Dealer SCENARIO machines, whose discriminants this crate's own
//! `decode` authors, this machine's tags are emitted:
//! `generated_dealer_liquidity.rs` carries `PHASE_OPEN`, `PHASE_TERMINAL` and
//! `PHASE_RETIRED` from `EmitDealerLiquidityAbiRust.lean`. So the bit index is
//! [`crate::dealer::Phase::tag`] -- the emitted authority itself, called in `const`
//! position -- and not `as u8`, which would be a second numbering that happens
//! to agree today. `the_bit_index_is_the_emitted_tag` is what pins the two
//! together if Lean ever renumbers.
//!
//! ## What is deliberately NOT a set here
//!
//! `validate_state` reads the phase three more times -- the `Retired`
//! fee-custody exemption, the `Open` inventory-bounds conjunct, and the
//! per-phase canonicity match. Those are not admissible-prestate declarations
//! and must not be published as ones: they say what a canonical state of THAT
//! phase looks like, not which phases an act may be attempted in. A client
//! reading them as gates would refuse acts the program admits.
//!
//! Every set is a NECESSARY condition and never a sufficient one: a root
//! admitted by its phase still has its actor, its expiry, its curve, its
//! custody balances and its revision checked.

pub use crate::dealer::Phase as DealerRootPhaseV1;

/// One past the greatest emitted `DealerRootPhaseV1` tag.
const STATE_COUNT: u8 = 3;

/// The wire tag of one root phase, as a bit index.
///
/// [`crate::dealer::Phase::tag`] is the emitted authority, so this is not a second
/// numbering placed beside it.
const fn state_tag(state: DealerRootPhaseV1) -> u8 {
    state.tag()
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The tags are the wire tags, so this is the check that
/// a phase added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(DealerRootPhaseV1::Retired) < STATE_COUNT);

/// The Dealer root phases in which one route is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerRootAdmissionV1 {
    states: u8,
}

impl DealerRootAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    #[must_use]
    pub const fn states(states: &[DealerRootPhaseV1]) -> Self {
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
    pub const fn admits(self, state: DealerRootPhaseV1) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// Everything that quotes, fills, replaces or funds observes an open Dealer.
///
/// Seven guards: `schedule`, `activate`, `fill`, `enter_terminal`, the shared
/// add/remove liquidity body, and Trading's two V4 accelerator account
/// authentications. `enter_terminal` belongs here rather than with the set
/// below because a phase set is a PRESTATE: closing fills is the act that
/// leaves `Open`, so the state it observes is the open one.
pub const DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1: DealerRootAdmissionV1 =
    DealerRootAdmissionV1::states(&[DealerRootPhaseV1::Open]);

/// The terminal walk and its final close observe a terminal Dealer.
///
/// `unwind` retires one coordinate per crank and `retire` closes what is left,
/// so the two share a prestate and differ in what else they require --
/// affordable work funding for the crank, zero inventory and no pending
/// candidate for the close.
pub const DEALER_ROOT_TERMINAL_ADMISSIBLE_STATES_V1: DealerRootAdmissionV1 =
    DealerRootAdmissionV1::states(&[DealerRootPhaseV1::Terminal]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [DealerRootPhaseV1; 3] = [
        DealerRootPhaseV1::Open,
        DealerRootPhaseV1::Terminal,
        DealerRootPhaseV1::Retired,
    ];

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = DealerRootAdmissionV1::states(&[state]);
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
        assert!(DealerRootAdmissionV1::NONE.is_empty());
        assert!(DealerRootAdmissionV1::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!DealerRootAdmissionV1::NONE.admits(state));
        }
    }

    /// A set of several states admits exactly those and nothing else.
    ///
    /// The cases above pass the empty slice and single-element slices, which a
    /// constructor that stopped after its first entry would satisfy.
    #[test]
    fn a_listed_set_admits_exactly_what_it_lists() {
        let listed = [DealerRootPhaseV1::Open, DealerRootPhaseV1::Retired];
        let set = DealerRootAdmissionV1::states(&listed);
        assert!(!set.is_empty());
        for state in EVERY_STATE {
            assert_eq!(set.admits(state), listed.contains(&state), "{state:?}");
        }
    }

    /// The bit index is the tag Lean emitted, not a second numbering.
    ///
    /// `Phase::tag` reads `generated_dealer_liquidity.rs`, so if Lean ever
    /// renumbers a phase the set and the decoder move together or this goes
    /// red. Checking `as u8` instead would agree today and say nothing.
    #[test]
    fn the_bit_index_is_the_emitted_tag() {
        for state in EVERY_STATE {
            assert_eq!(state_tag(state), state.tag(), "{state:?}");
            assert_eq!(DealerRootPhaseV1::decode(state.tag()), Ok(state));
        }
    }

    /// The two sets are disjoint, and `Retired` is admitted by neither.
    ///
    /// The ordering fact the ladder rests on: no act is available both before
    /// and after fills close, and a retired Dealer accepts nothing at all --
    /// which is a claim the machine makes and the Market's phase cannot, since
    /// a Market may still be `Open` over a fully retired Dealer.
    #[test]
    fn the_ladder_is_ordered_and_retired_admits_nothing() {
        assert!(DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(DealerRootPhaseV1::Open));
        assert!(DEALER_ROOT_TERMINAL_ADMISSIBLE_STATES_V1.admits(DealerRootPhaseV1::Terminal));
        for state in EVERY_STATE {
            assert!(
                !(DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(state)
                    && DEALER_ROOT_TERMINAL_ADMISSIBLE_STATES_V1.admits(state)),
                "{state:?} is admitted by both sets"
            );
        }
        assert!(!DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(DealerRootPhaseV1::Retired));
        assert!(!DEALER_ROOT_TERMINAL_ADMISSIBLE_STATES_V1.admits(DealerRootPhaseV1::Retired));
    }

    /// Each constant reproduces the exact condition that stood at its guards.
    ///
    /// Written out as the boolean the sites carried before the constants
    /// existed and checked over every one of the machine's three phases, so
    /// "behaviour is unchanged" is a run and not an assertion. Control:
    /// adding `Retired` to the open set turns this red naming `Retired`.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        for state in EVERY_STATE {
            // `schedule`, `activate`, `fill`, `enter_terminal`, liquidity, and
            // Trading's two V4 accelerator account authentications.
            assert_eq!(
                DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(state),
                state == DealerRootPhaseV1::Open,
                "{state:?}"
            );
            // `unwind` and `retire`.
            assert_eq!(
                DEALER_ROOT_TERMINAL_ADMISSIBLE_STATES_V1.admits(state),
                state == DealerRootPhaseV1::Terminal,
                "{state:?}"
            );
        }
    }
}
