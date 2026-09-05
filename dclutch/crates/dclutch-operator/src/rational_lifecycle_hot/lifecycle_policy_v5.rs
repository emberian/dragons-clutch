//! Rational's state lifecycle policy, and the argument that it is empty.
//!
//! # Why this module exists at all
//!
//! Every `encode_lifecycle_policy_v5_atomic` call in the tree for Rational
//! passed six empty slices, and every one of them was `#[cfg(test)]`. Six empty
//! vectors in a fixture and six empty vectors in a release look identical, and
//! only one of them is defensible -- so a release compiler reaching for this
//! artifact should call something that has made the argument, not repeat the
//! literal that a test happened to use.
//!
//! An empty policy is the most dangerous shape a policy can have if it is
//! wrong: it authenticates perfectly against itself, its digest joins its
//! descriptor cleanly, and it derives no address at all. Nothing downstream can
//! tell "this family owns no Trading state" apart from "nobody wrote the seeds
//! yet".
//!
//! # The argument
//!
//! A [`StateLifecyclePolicyV5`] describes TRADING-owned PDA derivation and
//! state planning: the plans Trading executes to create, authenticate, or close
//! accounts it owns on behalf of a selected capability. Rational's four
//! lifecycle actions create no such account.
//!
//! Every resource a Rational action touches is CLAIMS-owned and created by
//! Claims through its own CPI, under Claims-owned seeds:
//!
//! - the closeable shard Mint, at `RATIONAL_SHARD_MINT_SEED_V2`,
//! - the Token-2022 Structured custody account, at
//!   `RATIONAL_STRUCTURED_CUSTODY_SEED_V2`,
//! - the capability custody owner, at `PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2`,
//! - the LBV2 ProtocolPosition and its admission record, at
//!   `PROTOCOL_POSITION_STATE_SEED_V2` and `PROTOCOL_POSITION_ADMISSION_SEED_V2`.
//!
//! Correspondingly, the family's effect programs declare exactly one route, and
//! its role is [`FixedRole::Claims`]. There is no second role through which
//! Trading-owned state could be reached.
//!
//! The capability root is the one Trading-owned account in the frame, and
//! Rational does not plan it either: its derivation is owned by the shared
//! `CapabilityRootSeedsV1`, its creation by the activation route that spends
//! the manifest entry's prepaid Rent quote, and the family observes it only as
//! an optimistic-concurrency digest over its current bytes (see
//! `RationalLifecycleHotStateV3::root_data`). Rational's account profiles never
//! reference the root coordinate at all.
//!
//! # The part that is NOT settled here, named rather than buried
//!
//! Series reached a different answer for a neighbouring question: its Consume
//! policy carries one plan that AUTHENTICATES the root at coordinate 0, so the
//! lifecycle machinery -- not only the caller's supplied digest -- has seen the
//! root on the action path. Whether Rational should acquire the same
//! root-covering Authenticate plan is a live question about how much the
//! optimistic-concurrency digest is trusted to stand for, and it is NOT decided
//! by the argument above, which only establishes that Rational authors no state
//! of its own. If that question is answered "yes", this module is where the
//! plan goes, and the Series precedent (a root-header-projecting profile, so
//! the policy's seeds have an honest single author) is the shape to follow.

use dclutch_vm::account_profile::lifecycle_v3::{
    HEADER_BYTES as LIFECYCLE_HEADER_BYTES, encode::encode_lifecycle_policy_v5_atomic,
};

use crate::rational_lifecycle_hot::{Error, Result};

/// Exact encoded width of the Rational lifecycle policy.
pub const RATIONAL_LIFECYCLE_POLICY_BYTES_V5: usize = LIFECYCLE_HEADER_BYTES;

/// Encode Rational's state lifecycle policy.
///
/// Deliberately empty -- no recipes, seeds, plans, protected outputs, immutable
/// identity bindings, or rent quotes -- because Rational's actions author no
/// Trading-owned account. See this module's header for the argument and for the
/// one adjacent question it does not settle.
///
/// Takes no parameters, which is the point: a policy that could be
/// PARAMETERIZED into naming a seed would be a second author for a derivation
/// this family does not perform.
pub fn encode_rational_lifecycle_policy_v5() -> Result<Vec<u8>> {
    let mut scratch = vec![0_u8; RATIONAL_LIFECYCLE_POLICY_BYTES_V5];
    let mut output = vec![0_u8; RATIONAL_LIFECYCLE_POLICY_BYTES_V5];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
        .map_err(Error::LifecycleArtifact)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_vm::account_profile::lifecycle_v3::StateLifecyclePolicyV5;
    use solana_program::hash::hash;

    /// The emitted policy really is empty, and says so when decoded.
    ///
    /// Pins the claim the module header argues for, so that if Rational ever
    /// does acquire a plan, the change has to be made here deliberately rather
    /// than arrived at.
    #[test]
    fn the_rational_lifecycle_policy_plans_nothing_for_every_action() {
        let policy = encode_rational_lifecycle_policy_v5().expect("policy");
        assert_eq!(policy.len(), RATIONAL_LIFECYCLE_POLICY_BYTES_V5);
        // Both coordinates are the policy's own content identity -- the
        // descriptor selects it by digest, exactly as the bundle validator
        // does with `descriptor.lifecycle().program()`.
        let identity = hash(&policy).to_bytes();
        let decoded =
            StateLifecyclePolicyV5::decode_selected(identity, identity, &policy).expect("decode");
        // All four lifecycle action tags, plus a tag no action uses: none of
        // them plans anything, so no action can be quietly carrying state.
        for action in 0..5_u32 {
            assert_eq!(decoded.action_plan_count(action), Ok(0));
        }
    }

    /// It is a pure function: same bytes every call.
    ///
    /// The policy's digest is named by every action descriptor, so a policy
    /// that varied between builds would move four descriptor identities and the
    /// ProgramSet identity above them.
    #[test]
    fn the_policy_is_byte_stable() {
        assert_eq!(
            encode_rational_lifecycle_policy_v5().expect("first"),
            encode_rational_lifecycle_policy_v5().expect("second")
        );
    }
}
