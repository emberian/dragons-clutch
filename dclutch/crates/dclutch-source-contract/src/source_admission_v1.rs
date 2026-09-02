//! Named admissible Source resolution states for one route's guard.
//!
//! This is [`crate::SourceResolutionPhaseV1`]'s half of the same repair
//! `MarketAdmissionV1` made for the Core Market
//! (`crates/dclutch-market-core-codec/src/market_admission_v1.rs`, `315f1931`).
//! A route that reads `source_state.phase()` is answering "may this Source be
//! asked to do this now?", and written inline as
//! `phase() != SourceResolutionPhaseV1::Primary` the answer has no name, so
//! nothing outside the function can read it.
//!
//! ## Why a second type rather than a second use of the first
//!
//! The Market's phase cannot answer this question and never could. A Market
//! is `Open` for the whole span in which its Source moves `Primary` ->
//! `Recovery` -> `Resolved`, so a client that read only the Market gate would
//! report a sponsored capture as admissible on a Source that has already
//! resolved. One admission type per state machine is the point: the machine is
//! named in the declaration, a consumer that cannot observe that machine says
//! so, and no set is ever silently read against the wrong discriminant.
//!
//! The two types are deliberately the same shape -- a const-constructed bitset
//! indexed by the machine's own wire tags, with an `admits` and an `is_empty`
//! -- so the route census reads them with one enumerator parameterized by
//! machine rather than one parser per state machine.
//!
//! The set is a NECESSARY condition and never a sufficient one.

use crate::SourceResolutionPhaseV1;

/// Number of distinct `SourceResolutionPhaseV1` values.
const STATE_COUNT: u8 = 6;

/// The wire tag of one resolution state, as a bit index.
const fn state_tag(state: SourceResolutionPhaseV1) -> u8 {
    state as u8
}

/// Every state occupies its own bit of a `u8`, so the widest index this file
/// can produce must fit. The discriminants are the wire tags, so this is the
/// check that a state added upstream cannot silently alias an existing bit.
const _: () = assert!(STATE_COUNT <= 8);
const _: () = assert!(state_tag(SourceResolutionPhaseV1::Retired) < STATE_COUNT);

/// The Source resolution states in which one route is admissible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceAdmissionV1 {
    states: u8,
}

impl SourceAdmissionV1 {
    /// The empty admission: no state at all.
    pub const NONE: Self = Self { states: 0 };

    /// Admit exactly the listed states.
    #[must_use]
    pub const fn states(states: &[SourceResolutionPhaseV1]) -> Self {
        let mut admitted = 0u8;
        let mut position = 0;
        while position < states.len() {
            admitted |= 1u8 << state_tag(states[position]);
            position += 1;
        }
        Self { states: admitted }
    }

    /// Whether this exact state is admitted.
    #[must_use]
    pub const fn admits(self, state: SourceResolutionPhaseV1) -> bool {
        self.states & (1u8 << state_tag(state)) != 0
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.states == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATE: [SourceResolutionPhaseV1; 6] = [
        SourceResolutionPhaseV1::Primary,
        SourceResolutionPhaseV1::Recovery,
        SourceResolutionPhaseV1::Resolved,
        SourceResolutionPhaseV1::Exhausted,
        SourceResolutionPhaseV1::FailureCommitted,
        SourceResolutionPhaseV1::Retired,
    ];

    #[test]
    fn every_state_has_its_own_bit() {
        for state in EVERY_STATE {
            let one = SourceAdmissionV1::states(&[state]);
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
        assert!(SourceAdmissionV1::NONE.is_empty());
        assert!(SourceAdmissionV1::states(&[]).is_empty());
        for state in EVERY_STATE {
            assert!(!SourceAdmissionV1::NONE.admits(state));
        }
    }

    /// The tags are the wire discriminants, not a second numbering.
    ///
    /// `SourceResolutionPhaseV1::decode` maps bytes 0..=5 to these variants,
    /// and if a variant's discriminant ever moved without this moving with it
    /// the set would be indexed by one numbering and decoded by another.
    #[test]
    fn the_bit_index_is_the_wire_tag() {
        for (tag, state) in EVERY_STATE.into_iter().enumerate() {
            assert_eq!(u8::try_from(tag).unwrap(), state as u8);
        }
    }
}
