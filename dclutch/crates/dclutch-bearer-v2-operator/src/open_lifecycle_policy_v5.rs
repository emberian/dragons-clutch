//! The open capability's state lifecycle policy, and the argument that it is
//! empty.
//!
//! # Why this module exists at all
//!
//! Every open-family `encode_lifecycle_policy_v5_atomic` call in the tree lived
//! in a `#[cfg(test)]` fixture, and every one of them encoded the SAME
//! decorated shape: one recipe, two seeds -- a literal
//! `b"dclutch/rational-open/dormant/v4"` and a canonical bump -- and one
//! `Authenticate` plan at `action: u32::MAX`.
//!
//! A release compiler reaching for this artifact must call something that has
//! made an argument, not copy the literal a test happened to use. That is the
//! lesson `encode_rational_lifecycle_policy_v5` was written for; this is the
//! same lesson for the layer Bearer and Structured share.
//!
//! # The fixture policy is unreachable, and that is a measured fact
//!
//! `StateLifecyclePolicyV5` selects plans by EXACT equality on the action tag:
//! both `action_plan_count` and `action_plan` compare `plan.action == action`
//! (`lifecycle_v3.rs:1173` and `:1187`). There is no wildcard tag and no
//! catch-all.
//!
//! The five open actions carry tags 1 through 5. `u32::MAX` is not one of them.
//! So the fixture's single plan is selected by NO action the capability can
//! dispatch -- the policy is semantically empty already, wearing a seed domain
//! and a recipe that nothing derives and a plan that nothing runs.
//!
//! Shipping that shape into a release would publish a `dormant` PDA derivation
//! this family never performs, inside an artifact whose digest every one of five
//! descriptors names. An unreachable plan is worse than no plan: it reads as a
//! design to anyone auditing the release, and it is not one.
//!
//! # The argument for empty
//!
//! A [`StateLifecyclePolicyV5`] describes TRADING-owned PDA derivation and state
//! planning: the plans Trading executes to create, authenticate, or close
//! accounts it owns on behalf of a selected capability. The five open actions
//! create no such account, and this is checkable rather than asserted.
//!
//! Each of the three open bundle builders encodes its effect with exactly one
//! route whose role is [`FixedRole::Claims`], and each of them REFUSES a program
//! that is shaped otherwise -- `open_selected_v3.rs:418`,
//! `open_structured_v3.rs:524` and `hot_effect_v3.rs:279` all pin
//! `route_count() == 1` and `role() == FixedRole::Claims`. There is no second
//! role in any of the five actions through which Trading-owned state could be
//! reached, so there is no such state for a policy to plan.
//!
//! The capability root is the one Trading-owned account in the frame, and the
//! open family does not plan it either: its derivation belongs to the shared
//! `CapabilityRootSeedsV1` and its creation to the activation route that spends
//! the manifest entry's prepaid Rent quote.
//!
//! # The part that is NOT settled here, named rather than buried
//!
//! Series reached a different answer for a neighbouring question: its Consume
//! policy carries one plan that AUTHENTICATES the root, so the lifecycle
//! machinery -- not only the caller's supplied digest -- has seen the root on the
//! action path. Whether the open family should acquire the same root-covering
//! plan is a live question about how much the optimistic-concurrency digest is
//! trusted to stand for. It is NOT decided by the argument above, which
//! establishes only that the open actions author no state of their own. If that
//! question is answered "yes", this module is where the plan goes -- and it
//! would go at the five REAL action tags, which is precisely what the fixture's
//! `u32::MAX` plan never did.

use dclutch_account_profile_contract::lifecycle_v3::{
    HEADER_BYTES as LIFECYCLE_HEADER_BYTES, encode::encode_lifecycle_policy_v5_atomic,
};

use crate::{Error, Result};

/// Exact encoded width of the open capability's lifecycle policy.
pub const OPEN_CAPABILITY_LIFECYCLE_POLICY_BYTES_V5: usize = LIFECYCLE_HEADER_BYTES;

/// Encode the open capability's state lifecycle policy.
///
/// Deliberately empty -- no recipes, seeds, plans, protected outputs, immutable
/// identity bindings, or rent quotes -- because the five open actions author no
/// Trading-owned account. See this module's header for the argument, for the
/// measurement behind it, and for the one adjacent question it does not settle.
///
/// Takes no parameters, which is the point: a policy that could be PARAMETERIZED
/// into naming a seed would be a second author for a derivation this family does
/// not perform.
pub fn encode_open_capability_lifecycle_policy_v5() -> Result<Vec<u8>> {
    let mut scratch = vec![0_u8; OPEN_CAPABILITY_LIFECYCLE_POLICY_BYTES_V5];
    let mut output = vec![0_u8; OPEN_CAPABILITY_LIFECYCLE_POLICY_BYTES_V5];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
        .map_err(Error::LifecycleArtifact)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::StateLifecyclePolicyV5;
    use dclutch_rational_representation_v2_contract::RepresentationActionV2;
    use solana_program::hash::hash;

    fn decoded(policy: &[u8]) -> StateLifecyclePolicyV5<'_> {
        // Both coordinates are the policy's own content identity -- the
        // descriptor selects it by digest, exactly as the bundle validators do
        // with `descriptor.lifecycle().program()`.
        let identity = hash(policy).to_bytes();
        StateLifecyclePolicyV5::decode_selected(identity, identity, policy).expect("decode")
    }

    /// The emitted policy really is empty, and says so when decoded.
    ///
    /// Pins the claim the module header argues for, so that if the open family
    /// ever does acquire a plan, the change has to be made here deliberately
    /// rather than arrived at.
    #[test]
    fn the_open_lifecycle_policy_plans_nothing_for_every_action() {
        let policy = encode_open_capability_lifecycle_policy_v5().expect("policy");
        assert_eq!(policy.len(), OPEN_CAPABILITY_LIFECYCLE_POLICY_BYTES_V5);
        let decoded = decoded(&policy);
        for action in [
            RepresentationActionV2::Denominate,
            RepresentationActionV2::Reconstitute,
            RepresentationActionV2::IssueStructured,
            RepresentationActionV2::UnwrapStructured,
            RepresentationActionV2::RedeemTerminal,
        ] {
            assert_eq!(decoded.action_plan_count(action as u32), Ok(0));
        }
        // A tag no action uses, and the sentinel the fixture policy parked its
        // one plan at. Neither selects anything here either.
        assert_eq!(decoded.action_plan_count(0), Ok(0));
        assert_eq!(decoded.action_plan_count(u32::MAX), Ok(0));
    }

    /// The FIXTURE policy's plan is selected by no action the capability can
    /// dispatch, which is why shipping it would have published a design that is
    /// not one.
    ///
    /// This is the measurement the module header rests on, made executable so
    /// the claim cannot rot: it encodes the exact decorated shape every open
    /// test fixture uses and shows that all five real action tags select zero
    /// plans from it, while `u32::MAX` selects the one. If the lifecycle codec
    /// ever grows a wildcard tag, this test goes red and names the assumption.
    #[test]
    fn the_fixture_policys_only_plan_is_unreachable_from_every_real_action() {
        use dclutch_account_profile_contract::lifecycle_v3::{
            ACTION_PLAN_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
            encode::{
                LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
                LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
                LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
            },
        };

        let recipes = [LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(0),
            seed_start: 0,
            seed_count: 2,
            bump_offset: 1,
            data_base: 8,
            data_stride: 0,
        }];
        let seeds = [
            LifecycleSeedInputV3::Literal(b"dclutch/rational-open/dormant/v4"),
            LifecycleSeedInputV3::CanonicalBump,
        ];
        let plans = [LifecyclePlanInputV3 {
            action: u32::MAX,
            operation: LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            refund_source: LifecycleRefundSourceInputV3::Credit,
            guard: LifecycleGuardInputV3::Always,
        }];
        let width = LIFECYCLE_HEADER_BYTES
            + RECIPE_BYTES
            + 2 * SEED_BYTES
            + ACTION_PLAN_BYTES
            + PROTECTED_OUTPUT_BYTES;
        let mut scratch = vec![0_u8; width];
        let mut fixture = vec![0_u8; width];
        encode_lifecycle_policy_v5_atomic(
            &recipes,
            &seeds,
            &plans,
            &[None],
            &[],
            &[],
            &mut scratch,
            &mut fixture,
        )
        .expect("fixture policy");

        let decoded = decoded(&fixture);
        // The plan exists...
        assert_eq!(decoded.action_plan_count(u32::MAX), Ok(1));
        // ...and no action that can actually be dispatched selects it.
        for action in [
            RepresentationActionV2::Denominate,
            RepresentationActionV2::Reconstitute,
            RepresentationActionV2::IssueStructured,
            RepresentationActionV2::UnwrapStructured,
            RepresentationActionV2::RedeemTerminal,
        ] {
            assert_eq!(
                decoded.action_plan_count(action as u32),
                Ok(0),
                "the fixture policy must plan nothing for a real action"
            );
        }
    }

    /// It is a pure function: same bytes every call.
    ///
    /// The policy's digest is named by every action descriptor, so a policy that
    /// varied between builds would move five descriptor identities and the
    /// ProgramSet identity above them.
    #[test]
    fn the_policy_is_byte_stable() {
        assert_eq!(
            encode_open_capability_lifecycle_policy_v5().expect("first"),
            encode_open_capability_lifecycle_policy_v5().expect("second")
        );
    }
}
