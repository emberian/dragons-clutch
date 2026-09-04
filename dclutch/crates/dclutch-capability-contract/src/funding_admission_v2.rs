//! Named admissible states for one funding-ledger slot's lifecycle.
//!
//! [`FundingLedgerStatusV2`] is the ninth machine to take the shape
//! `MarketAdmissionV1` introduced (`315f1931`), and the first that is per-ROW
//! rather than per-account: one ledger holds a slot for every selected
//! manifest entry, and each of them walks `Pending` -> `Active` -> `Closed`
//! independently. Written inline as `slot.status != FundingLedgerStatusV2::Active`
//! the four transition guards had no name a reader outside the method could
//! reach, and the four host-side readers each wrote the same comparison again.
//!
//! ## Not the Market's phase, and not one per Market either
//!
//! A Market is `Open` while its capability children are still being activated
//! one at a time, and it stays `Open` after they close. So the Market's phase
//! answers nothing about whether a given capability may be activated now, and
//! a client that read it would report a second activation of an already-Active
//! entry as ready. Worse, there is no single answer per Market: two entries of
//! one manifest are routinely in different statuses, which is why the set is
//! checked against `slot(entry_index)` and never against the ledger.
//!
//! ## The tags are Lean-emitted, and the enum is still its own encoder
//!
//! `FundingLedgerStatusV2` carries `#[repr(u8)]` and
//! [`FundingLedgerStatusV2::byte`] writes its discriminants, so the
//! discriminant is the wire tag. Those discriminants are now the emitted
//! constants of `DClutchSemantics.CapabilityManifestV1Abi`'s
//! `LedgerSlotStatusV2` -- the module that already owned the byte they are
//! written at -- rather than three literals typed in `funding.rs`.
//! `the_bit_index_is_the_wire_encoding` pins the index against the `byte`/
//! `decode` pair as before, and now both sides of that comparison descend
//! from the emission.
//!
//! ## What is deliberately NOT a set here
//!
//! Three reads, and publishing any of them as an admissible prestate would be
//! a false claim rather than an under-count.
//!
//! `activate_in_place` re-reads the POSTSTATE and requires `Active`; that is
//! the transition's own postcondition, and as a prestate it would say the
//! opposite of what the route admits. `AuthenticatedFundingLedgerV2::slot`
//! matches all three statuses to check that the remaining and released
//! amounts are canonical FOR that status -- a shape check, not a gate.
//! `all_closed` folds `!= Closed` across every row to decide whether the
//! shared ACCOUNT may be freed, which is a fact about the ledger rather than
//! about one act on one slot.
//!
//! Every set is a NECESSARY condition and never a sufficient one: a slot
//! admitted by its status still has its manifest binding, its activation
//! policy, its deadline, its conservation and its physical custody checked.

use crate::funding::FundingLedgerStatusV2;

/// Number of distinct `FundingLedgerStatusV2` values, from the emission
/// rather than typed a second time beside it.
const STATE_COUNT: u8 = crate::generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_LIMIT_V2;

/// The wire tag of one slot status, as a bit index.
const fn state_tag(state: FundingLedgerStatusV2) -> u8 {
    state as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The discriminants are the wire tags, so this is the
/// check that a status added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(FundingLedgerStatusV2::Closed) < STATE_COUNT);

/// The funding-ledger slot statuses in which one route is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FundingLedgerAdmissionV2 {
    states: u8,
}

impl FundingLedgerAdmissionV2 {
    /// The empty admission: no status at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed statuses.
    #[must_use]
    pub const fn states(states: &[FundingLedgerStatusV2]) -> Self {
        let mut admitted = 0u8;
        let mut remaining = states;
        while let [state, rest @ ..] = remaining {
            admitted |= 1u8 << state_tag(*state);
            remaining = rest;
        }
        Self { states: admitted }
    }

    /// Whether this exact status is admitted.
    #[must_use]
    pub const fn admits(self, state: FundingLedgerStatusV2) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

/// Activation observes a slot whose prepaid quote has not been drawn on.
///
/// One guard in this crate -- `FundingLedgerV2::activate_in_place`, the sole
/// author of the transition -- and two host-side readers that authenticate the
/// same prestate before building the instruction.
pub const FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2: FundingLedgerAdmissionV2 =
    FundingLedgerAdmissionV2::states(&[FundingLedgerStatusV2::Pending]);

/// Releasing a compartment and closing an entry observe an activated slot.
///
/// Two guards here -- `release_in_place` and `close_slot_in_place` -- and
/// three host-side readers. They share a prestate and differ in what else they
/// require: a nonzero amount and a non-activation compartment for the release,
/// exact realm collateral and a rent-credit identity for the close.
pub const FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2: FundingLedgerAdmissionV2 =
    FundingLedgerAdmissionV2::states(&[FundingLedgerStatusV2::Active]);

/// Opening a Market observes a slot that has not closed.
///
/// The set that does NOT swallow the match it stands beside, which is the
/// `lock_hoard_and_close_source` shape (`8bf97477`) one machine over.
/// `validate_market_open` refuses on a PAIR -- the manifest entry's activation
/// policy and this status -- and only one conjunct of that pair is over this
/// machine: `Closed` is refused whatever the policy says. So the set takes
/// over excluding `Closed` and the match keeps the two policy cases that
/// remain, each with its own refusal. Written as one set over the pair it
/// would have had to invent a second axis; written as this it is the exact
/// necessary condition a client can check.
pub const FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2: FundingLedgerAdmissionV2 =
    FundingLedgerAdmissionV2::states(&[
        FundingLedgerStatusV2::Pending,
        FundingLedgerStatusV2::Active,
    ]);

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [FundingLedgerStatusV2; 3] = [
        FundingLedgerStatusV2::Pending,
        FundingLedgerStatusV2::Active,
        FundingLedgerStatusV2::Closed,
    ];

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = FundingLedgerAdmissionV2::states(&[state]);
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
        assert!(FundingLedgerAdmissionV2::NONE.is_empty());
        assert!(FundingLedgerAdmissionV2::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!FundingLedgerAdmissionV2::NONE.admits(state));
        }
    }

    /// A set of several states admits exactly those and nothing else.
    #[test]
    fn a_listed_set_admits_exactly_what_it_lists() {
        let listed = [
            FundingLedgerStatusV2::Pending,
            FundingLedgerStatusV2::Active,
        ];
        let set = FundingLedgerAdmissionV2::states(&listed);
        assert!(!set.is_empty());
        for state in EVERY_STATE {
            assert_eq!(set.admits(state), listed.contains(&state), "{state:?}");
        }
    }

    /// The bit index is the byte the status encodes and decodes as.
    #[test]
    fn the_bit_index_is_the_wire_encoding() {
        for state in EVERY_STATE {
            assert_eq!(state_tag(state), state.byte(), "{state:?}");
            assert_eq!(FundingLedgerStatusV2::decode(state.byte()), Ok(state));
        }
    }

    /// The ladder is ordered, and the Market-open set is exactly its
    /// non-terminal prefix.
    ///
    /// Three facts in one: the activation and release sets are disjoint, so no
    /// act is available both before and after a slot activates; a `Closed`
    /// slot admits nothing at all; and the Market-open set is the union of the
    /// other two, which is what makes it a projection of the ladder rather
    /// than a fourth opinion about it.
    #[test]
    fn the_ladder_is_ordered_and_market_open_is_its_prefix() {
        for state in EVERY_STATE {
            assert!(
                !(FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2.admits(state)
                    && FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(state)),
                "{state:?} is admitted by both transition sets"
            );
            assert_eq!(
                FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2.admits(state),
                FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2.admits(state)
                    || FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(state),
                "{state:?}"
            );
        }
        assert!(
            !FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2.admits(FundingLedgerStatusV2::Closed)
        );
    }

    /// Each constant reproduces the exact condition that stood at its guards.
    ///
    /// Written out as the boolean the sites carried before the constants
    /// existed and checked over every one of the machine's three statuses.
    /// Control: adding `Closed` to the active set turns two tests red, one of
    /// them here naming `Closed`.
    #[test]
    fn admissible_states_reproduce_the_guards_they_replaced() {
        for state in EVERY_STATE {
            // `activate_in_place`, and the two host readers of its prestate.
            assert_eq!(
                FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2.admits(state),
                state == FundingLedgerStatusV2::Pending,
                "{state:?}"
            );
            // `release_in_place`, `close_slot_in_place`, three host readers.
            assert_eq!(
                FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(state),
                state == FundingLedgerStatusV2::Active,
                "{state:?}"
            );
            // `validate_market_open`'s `(_, Closed) => InvalidFundingStatus`
            // arm, which the set now stands in for.
            assert_eq!(
                FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2.admits(state),
                state != FundingLedgerStatusV2::Closed,
                "{state:?}"
            );
        }
    }
}
