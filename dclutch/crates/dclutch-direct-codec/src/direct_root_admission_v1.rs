//! Named admissible states for the global Direct lifecycle.
//!
//! [`DirectRootPhaseV1`] is the sixth machine to take the shape
//! `MarketAdmissionV1` introduced (`315f1931`), after the Core Market's phase,
//! the Source's resolution phase, the Dealer scenario's checkpoint phase and
//! reservation status, and the projected Custody ladder. Six guards read it --
//! five here in `successor.rs` and one in Trading's Direct token setup -- and
//! written inline as `root.phase != DirectRootPhaseV1::Open` not one of them
//! had a name a reader outside the function could reach.
//!
//! ## `Open` here is not the Market's `Open`
//!
//! This is the whole reason the machine gets its own type rather than a sixth
//! use of an existing one. A Core Market is `Open` for the entire span in
//! which its Direct root moves `Open` -> `Retiring` and drains its maker
//! replay accounts, and Direct's retirement is driven by
//! `direct_begin_retiring_v1` on a Market that is still trading. So a client
//! that read the Market's phase and concluded "Open, therefore a maker may
//! consume a nonce" would report a permanently refused act as ready. The two
//! discriminants are stored in different accounts, moved by different
//! instructions, and mean different things; only the machine's own name keeps
//! one set from being checked against the other's state.
//!
//! `Retiring` has no Market counterpart at all. It is terminal for admission
//! -- no new nonce is ever consumed again -- while remaining the ONLY phase in
//! which a maker replay root may be closed, so it is simultaneously the
//! narrowest and the widest thing to say about the root depending on which act
//! is asked about. That is exactly what a per-route set is for.
//!
//! ## The tags are the wire discriminants
//!
//! `DirectRootPhaseV1`'s discriminants are not Lean-emitted:
//! [`DirectRootPhaseV1::byte`] and [`DirectRootPhaseV1::decode`] are a
//! hand-written pair in this crate, so the Rust enum is the author and the bit
//! index is its discriminant. `the_bit_index_is_the_wire_encoding` pins the
//! index against `byte` itself rather than against a second hand-written
//! numbering, which is the strongest form of that check available here.
//!
//! Every set is a NECESSARY condition and never a sufficient one: a root
//! admitted by its phase still has its maker count, its rent, its nonce
//! ordering and its account derivation checked.

use crate::successor::DirectRootPhaseV1;

/// Number of distinct `DirectRootPhaseV1` values.
const STATE_COUNT: u8 = 2;

/// The wire tag of one root phase, as a bit index.
const fn state_tag(state: DirectRootPhaseV1) -> u8 {
    state as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The discriminants are the wire tags, so this is the
/// check that a phase added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(DirectRootPhaseV1::Retiring) < STATE_COUNT);

/// The Direct root phases in which one route is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectRootAdmissionV1 {
    states: u8,
}

impl DirectRootAdmissionV1 {
    /// The empty admission: no phase at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed phases.
    ///
    /// Walked by slice pattern rather than by index so the loop carries no
    /// index to bound, which is what the crate's `indexing_slicing` denial
    /// wants and what a `const fn` cannot express with `.get`.
    #[must_use]
    pub const fn states(states: &[DirectRootPhaseV1]) -> Self {
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
    pub const fn admits(self, state: DirectRootPhaseV1) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// Everything that admits a new maker nonce observes an open root.
///
/// Four guards: `consume_nonce_v2`, `preview_registered_fill_v2`,
/// `begin_retiring` (a root may begin retiring exactly once, from open), and
/// Trading's `direct_token_setup_v1`, which refuses to build a token
/// compartment for a root that has stopped admitting makers.
pub const DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1: DirectRootAdmissionV1 =
    DirectRootAdmissionV1::states(&[DirectRootPhaseV1::Open]);

/// Closing a maker replay root, and the global root itself, observes a
/// retiring one.
///
/// The mirror of the set above and disjoint from it: a maker root may be
/// closed only once no new nonce can ever be consumed against it, which is
/// what makes `open_maker_root_count` monotonically decreasing and the global
/// close safe. `fee_settlement_v1` is deliberately NOT gated on this machine
/// -- it is phase-free so that settle-then-close is always available -- and
/// that absence is a decision, recorded at `close_maker_replay_v2`.
pub const DIRECT_ROOT_RETIRING_ADMISSIBLE_STATES_V1: DirectRootAdmissionV1 =
    DirectRootAdmissionV1::states(&[DirectRootPhaseV1::Retiring]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [DirectRootPhaseV1; 2] =
        [DirectRootPhaseV1::Open, DirectRootPhaseV1::Retiring];

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = DirectRootAdmissionV1::states(&[state]);
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
        assert!(DirectRootAdmissionV1::NONE.is_empty());
        assert!(DirectRootAdmissionV1::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!DirectRootAdmissionV1::NONE.admits(state));
        }
    }

    /// A set of several states admits exactly those and nothing else.
    ///
    /// The cases above pass the empty slice and single-element slices, which a
    /// constructor that stopped after its first entry would satisfy. This
    /// machine has only two phases, so the only multi-element set is the
    /// universe -- which no real guard uses, and which is precisely why it has
    /// to be tested here rather than read off one.
    #[test]
    fn a_listed_set_admits_exactly_what_it_lists() {
        let both = DirectRootAdmissionV1::states(&EVERY_STATE);
        assert!(!both.is_empty());
        for state in EVERY_STATE {
            assert!(both.admits(state), "{state:?}");
        }
    }

    /// The bit index is what `DirectRootPhaseV1::byte` writes to the account.
    ///
    /// Not a second hand-written numbering compared against a third: the
    /// encoder itself. If a variant's discriminant ever moved without `byte`
    /// moving with it, a set indexed by `as u8` would be checked against a
    /// state decoded from a different number.
    #[test]
    fn the_bit_index_is_the_wire_encoding() {
        for state in EVERY_STATE {
            assert_eq!(
                state_tag(state),
                state.byte(),
                "{state:?} sits at an index that is not the byte it encodes as"
            );
            assert_eq!(DirectRootPhaseV1::decode(state.byte()), Ok(state));
        }
    }

    /// The two real sets are disjoint and together cover the machine.
    ///
    /// The ordering fact this ladder rests on: every act is admissible in
    /// exactly one phase, so no route is reachable both before and after
    /// retirement begins, and no phase leaves the root with nothing to do.
    #[test]
    fn the_two_sets_partition_the_machine() {
        for state in EVERY_STATE {
            assert_ne!(
                DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(state),
                DIRECT_ROOT_RETIRING_ADMISSIBLE_STATES_V1.admits(state),
                "{state:?} is admitted by both sets or by neither"
            );
        }
    }

    /// Each constant reproduces the exact condition that stood at its guards.
    ///
    /// Written out as the boolean the sites carried before the constants
    /// existed and checked over every one of the machine's phases, so the
    /// commit's "behaviour is unchanged" is a run and not an assertion.
    /// Control: widening `DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1` to both
    /// phases turns this red naming `Retiring`.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        for state in EVERY_STATE {
            // `consume_nonce_v2`, `preview_registered_fill_v2`,
            // `begin_retiring`, `direct_token_setup_v1`.
            assert_eq!(
                DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(state),
                state == DirectRootPhaseV1::Open,
                "{state:?}"
            );
            // `close_maker_replay_v2`, `require_closable`.
            assert_eq!(
                DIRECT_ROOT_RETIRING_ADMISSIBLE_STATES_V1.admits(state),
                state == DirectRootPhaseV1::Retiring,
                "{state:?}"
            );
        }
    }
}
