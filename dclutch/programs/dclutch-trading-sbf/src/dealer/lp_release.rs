//! Schema-bound V4 release artifacts for Dealer LP Open and Close.
//!
//! Selectors 7 and 8 used to stop at a V3 descriptor even though the admitted
//! accelerator boundary accepts only `CapabilityProgramV4`. This module keeps
//! the existing LP request and register semantics, upgrades lifecycle Rent to
//! the protected V5 quote boundary, wraps the local V3 Effect in Effect V4,
//! and finalizes both selectors into the sole mixed nine-entry Dealer set.

extern crate alloc;

use alloc::vec;

use dclutch_vm::account_profile::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
        Error as LifecycleErrorV3, HEADER_BYTES, IMMUTABLE_IDENTITY_BINDING_BYTES,
        PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV5,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
            LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
            LifecycleRefundSourceInputV3, LifecycleRegisterCoordinateV3, LifecycleSeedInputV3,
            encode_lifecycle_policy_v5_atomic,
        },
    },
    v2::{AccountPrestateV2, AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2},
};
use dclutch_market::capability_program::CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1;
use dclutch_market::capability_program::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_trading::dealer::config_v4::DEALER_CONFIG_SCHEMA_PREIMAGE_V4;
use dclutch_vm::effect::{
    v3::ProgramV3 as EffectProgramV3,
    v4::{
        BorrowedRangePolicyV4, HEADER_BYTES_V4 as EFFECT_V4_HEADER_BYTES, ProgramV4,
        SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4, encode_program_v4_atomic,
    },
};
use dclutch_market::execution_strategy::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_vm::request_profile::{
    RequestProfileV1, SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1,
};
use dclutch_vm::v3::{ProgramV3 as TransitionProgramV3, SCHEMA_RELEASE_ID};
use solana_program::hash::hash;

use super::{
    DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
    lp_artifacts::{
        DEALER_LP_IDENTITY_COUNT_V3, DEALER_LP_LIFECYCLE_BYTES_V3,
        DEALER_LP_REQUEST_PROFILE_BYTES_V3, DEALER_LP_SCALAR_COUNT_V3, DEALER_LP_STATE_ACCOUNT_V3,
        DealerLpAccountProfileInputV3, LP_BUMP_OBSERVATION_SCALAR_V3, LP_CANONICAL_BUMP_SCALAR_V3,
        LP_CHILD_ROOT_IDENTITY_V3, LP_CREATED_SCALAR_V3, LP_CURRENT_SLOT_SCALAR_V3,
        LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3, LP_LIFECYCLE_OWNER_IDENTITY_V3,
        LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3, LP_LIFECYCLE_STATE_IDENTITY_V3,
        LP_MARKET_IDENTITY_V3, LP_OBLIGATION_IDENTITY_V3, LP_OBSERVED_LAMPORTS_SCALAR_V3,
        LP_OBSERVED_REFUND_IDENTITY_V3, LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3, LP_OWNER_IDENTITY_V3,
        LP_RELEASE_IDENTITY_V3, LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3, dealer_lp_account_count_v3,
        dealer_lp_effect_bytes_v3, dealer_lp_transition_bytes_v3,
        encode_dealer_lp_account_profile_v3, encode_dealer_lp_effect_v3,
        encode_dealer_lp_request_profile_v3, encode_dealer_lp_transition_v3,
    },
    multi_lp::{DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3},
    lp_request::{DEALER_MULTI_LP_REQUEST_BYTES_V3, MultiLpRequestActionV3},
    release::dealer_request_schema_v3,
};

/// The V4 LP lifecycle keeps the V3 tables and adds one protected current-Rent quote.
pub const DEALER_LP_LIFECYCLE_BYTES_V5: usize =
    DEALER_LP_LIFECYCLE_BYTES_V3 + CURRENT_RENT_QUOTE_BYTES_V5;

/// Effect V4 adds one fixed successor header around the canonical local V3 effect.
pub const fn dealer_lp_effect_bytes_v4(action: MultiLpRequestActionV3) -> usize {
    EFFECT_V4_HEADER_BYTES + dealer_lp_effect_bytes_v3(action)
}

/// Stable refusal from V4 LP artifact generation or finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerLpReleaseErrorV4 {
    /// Exact action-specific artifact geometry differed.
    Geometry,
    /// A finalized artifact was absent, substituted, or hostile-decode invalid.
    Artifact,
    /// The selected strategy was not admitted AOT over the exact transition.
    Strategy,
    /// Descriptor schemas or immutable Dealer identities differed.
    Descriptor,
    /// The per-action lifecycle/AccountProfile join refused, carrying its cause.
    ///
    /// The join is the one conjunct in this module that already knows which
    /// clause of the policy the profile failed, so it is the one that must not
    /// be flattened into `Geometry` with the fifteen sites that do not.
    ///
    /// Unreachable today, and deliberately kept: `validate_generated_artifacts`
    /// pins BOTH operands byte-for-byte before it joins them -- the profile
    /// against `encode_dealer_lp_account_profile_v3` of the same input, the
    /// policy against `encode_dealer_lp_lifecycle_v5` -- so a substitution
    /// refuses at the pin with `Geometry` and never reaches here. Measured, not
    /// assumed: a sweep of every caller-controlled logical data length over
    /// both actions finalized cleanly or was refused by the encoder, and none
    /// reached the join. The V3 twin orders the join BEFORE its pin and its
    /// hostile does fire (`release`), which is why the cause is carried in
    /// both: the day this pin moves, the answer is already here.
    ProfileJoin(LifecycleErrorV3),
}

/// Exact finalized inputs for one selector-7/8 V4 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpFinalizedArtifactsV4<'a> {
    /// Open or Close physical shape and exact account data lengths.
    pub account_profile_input: DealerLpAccountProfileInputV3<'a>,
    /// Exact lifecycle-bound AccountProfile V2 artifact.
    pub account_profile: &'a [u8],
    /// Exact current-Rent StateLifecyclePolicy V5 artifact.
    pub lifecycle_policy: &'a [u8],
    /// Exact immutable physical capacity profile.
    pub capacity_profile: &'a [u8],
    /// Exact Effect V4 artifact around the local V3 program.
    pub effect: &'a [u8],
    /// Exact fixed-width RequestProfile V1 artifact.
    pub request_profile: &'a [u8],
    /// Exact admitted-AOT ExecutionStrategy V2 artifact.
    pub execution_strategy: &'a [u8],
    /// Exact Transition V3 artifact selected by the strategy.
    pub transition: &'a [u8],
}

/// Encode the canonical selector-7/8 lifecycle policy under the V5 Rent schema.
pub fn encode_dealer_lp_lifecycle_v5(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpReleaseErrorV4> {
    if scratch.len() != DEALER_LP_LIFECYCLE_BYTES_V5 || output.len() != DEALER_LP_LIFECYCLE_BYTES_V5
    {
        return Err(DealerLpReleaseErrorV4::Geometry);
    }
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(DEALER_LP_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: 4,
        bump_offset: 3,
        data_base: u32::try_from(DEALER_LP_POSITION_BYTES_V3)
            .map_err(|_| DealerLpReleaseErrorV4::Geometry)?,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(DEALER_LP_POSITION_PDA_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(LP_CHILD_ROOT_IDENTITY_V3),
        LifecycleSeedInputV3::CommonIdentity(LP_OWNER_IDENTITY_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [
        LifecyclePlanInputV3 {
            action: u32::from(MultiLpRequestActionV3::Open.selector()),
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 0,
            payer: Some(LifecycleAccountCoordinateV3::fixed(7)),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(8)),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_REFUND_IDENTITY_V3,
            )),
            // An LP position is one owner's own account. Its PDA is seeded by
            // `LP_OWNER_IDENTITY_V3`, its Open debits that owner, the Effect
            // grammar requires the lifecycle beneficiary output to equal the
            // owner (operation 12), and the immutable identity binding below
            // says offset 152 holds the owner. Four authors already agreed the
            // refund is the owner's; only the kernel dissented, because the
            // Market-scoped RentCredit names one wallet per generation and the
            // kernel handed that wallet to every state it created. That is why
            // the family admitted exactly one LP owner per generation.
            refund_source: LifecycleRefundSourceInputV3::Payer,
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: u32::from(MultiLpRequestActionV3::Close.selector()),
            operation: LifecycleOperationInputV3::Close,
            recipe: 0,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(7)),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_REFUND_IDENTITY_V3,
            )),
            // Symmetric with the Open: the AccountProfile projects the closing
            // position's own bytes at offset 152 into this register, so the
            // close reads back the create's recorded answer instead of
            // re-deriving one the create never used.
            refund_source: LifecycleRefundSourceInputV3::Payer,
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [
        Some(LifecycleProtectedOutputsInputV3 {
            created: LP_CREATED_SCALAR_V3,
            bump_observation: LP_BUMP_OBSERVATION_SCALAR_V3,
            bump: LP_CANONICAL_BUMP_SCALAR_V3,
            historical_rent_principal: LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3,
            beneficiary: LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3,
            state: LP_LIFECYCLE_STATE_IDENTITY_V3,
            owner: LP_LIFECYCLE_OWNER_IDENTITY_V3,
        }),
        None,
    ];
    let bindings = [
        (24, LP_RELEASE_IDENTITY_V3),
        (56, LP_MARKET_IDENTITY_V3),
        (88, LP_CHILD_ROOT_IDENTITY_V3),
        (120, LP_OWNER_IDENTITY_V3),
        (152, LP_OWNER_IDENTITY_V3),
        (184, LP_OBLIGATION_IDENTITY_V3),
    ]
    .map(
        |(data_offset, identity)| LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset,
            canonical: LifecycleRegisterCoordinateV3::common(identity),
        },
    );
    let quotes = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: u32::try_from(DEALER_LP_POSITION_BYTES_V3)
            .map_err(|_| DealerLpReleaseErrorV4::Geometry)?,
        // Scalar ten was reserved as the observed-lamports slot but never
        // written by the LP AccountProfile. V4 makes that unused coordinate
        // the protected current-Rent value without changing bank width.
        scalar_destination: LP_OBSERVED_LAMPORTS_SCALAR_V3,
        action: None,
    }];
    encode_lifecycle_policy_v5_atomic(
        &recipes, &seeds, &plans, &protected, &bindings, &quotes, scratch, output,
    )
    .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    let id = digest(output);
    StateLifecyclePolicyV5::decode_selected(id, id, output)
        .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    Ok(())
}

/// Encode the canonical local LP Effect inside the sole Effect V4 schema.
pub fn encode_dealer_lp_effect_v4(
    action: MultiLpRequestActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpReleaseErrorV4> {
    let expected = dealer_lp_effect_bytes_v4(action);
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerLpReleaseErrorV4::Geometry);
    }
    let base_bytes = dealer_lp_effect_bytes_v3(action);
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_dealer_lp_effect_v3(action, &mut base_scratch, &mut base)
        .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(DEALER_MULTI_LP_REQUEST_BYTES_V3)
            .map_err(|_| DealerLpReleaseErrorV4::Geometry)?,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    let decoded = ProgramV4::decode(output).map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    if decoded.base().bytes() != base.as_slice()
        || EffectProgramV3::decode(decoded.base().bytes()).is_err()
    {
        return Err(DealerLpReleaseErrorV4::Artifact);
    }
    Ok(())
}

/// Finalize one selector-7/8 `CapabilityProgramV4` after exact artifact rederivation.
pub fn finalize_dealer_lp_descriptor_v4(
    artifacts: DealerLpFinalizedArtifactsV4<'_>,
) -> Result<[u8; CAPABILITY_PROGRAM_V4_BYTES], DealerLpReleaseErrorV4> {
    let action = artifacts.account_profile_input.action;
    validate_generated_artifacts(artifacts)?;
    if artifacts.capacity_profile.is_empty() {
        return Err(DealerLpReleaseErrorV4::Artifact);
    }
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.execution_strategy)
        .map_err(|_| DealerLpReleaseErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transition_schema().to_bytes() != SCHEMA_RELEASE_ID
        || strategy.transition_program().to_bytes() != digest(artifacts.transition)
    {
        return Err(DealerLpReleaseErrorV4::Strategy);
    }
    let lifecycle_program = content(digest(artifacts.lifecycle_policy))?;
    let descriptor = CapabilityProgramV4::new(
        content(digest(DEALER_KIND_PREIMAGE_V2))?,
        content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))?,
        dealer_request_schema_v3(action.selector())
            .map_err(|_| DealerLpReleaseErrorV4::Descriptor)?,
        content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2))?,
        // Per-root constant; see the twin in `equity_release`. LP Open was
        // the selector whose lifecycle digest the manifest entry happened to be
        // built from, which is why it alone executed.
        content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)?,
        content(digest(artifacts.capacity_profile))?,
        CapabilityArtifactsV4 {
            account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V2, artifacts.account_profile)?,
            request_profile: reference(REQUEST_PROFILE_SCHEMA_ID_V1, artifacts.request_profile)?,
            lifecycle: ArtifactReferenceV4::new(
                content(CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5)?,
                lifecycle_program,
            ),
            strategy: reference(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                artifacts.execution_strategy,
            )?,
            transition: reference(SCHEMA_RELEASE_ID, artifacts.transition)?,
            effect: reference(EFFECT_SCHEMA_ID_V4, artifacts.effect)?,
        },
        u32::try_from(dclutch_trading::dealer::root_tail::ROOT_TAIL_BYTES)
            .map_err(|_| DealerLpReleaseErrorV4::Geometry)?,
    )
    .map_err(|_| DealerLpReleaseErrorV4::Descriptor)?;
    strategy
        .validate_descriptor_selection_v4(
            content(digest(artifacts.execution_strategy))?,
            descriptor,
        )
        .map_err(|_| DealerLpReleaseErrorV4::Strategy)?;
    Ok(descriptor.encode())
}

fn validate_generated_artifacts(
    artifacts: DealerLpFinalizedArtifactsV4<'_>,
) -> Result<(), DealerLpReleaseErrorV4> {
    let action = artifacts.account_profile_input.action;
    if artifacts.account_profile_input.logical_data_lengths.len()
        != usize::from(dealer_lp_account_count_v3(action))
    {
        return Err(DealerLpReleaseErrorV4::Geometry);
    }
    let expected_profile = encode_dealer_lp_account_profile_v3(artifacts.account_profile_input)
        .map_err(|_| DealerLpReleaseErrorV4::Geometry)?;
    require_exact(&expected_profile, artifacts.account_profile)?;
    let profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    if profile.fixed_account_count() != dealer_lp_account_count_v3(action)
        || profile.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || profile.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || profile.trusted_current_slot_scalar() != Some(LP_CURRENT_SLOT_SCALAR_V3)
        || profile
            .rule(false, DEALER_LP_STATE_ACCOUNT_V3)
            .map_err(|_| DealerLpReleaseErrorV4::Geometry)?
            .prestate()
            != AccountPrestateV2::LifecycleBound
    {
        return Err(DealerLpReleaseErrorV4::Geometry);
    }

    let mut lifecycle_scratch = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
    let mut lifecycle = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
    encode_dealer_lp_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)?;
    require_exact(&lifecycle, artifacts.lifecycle_policy)?;
    let lifecycle_id = digest(artifacts.lifecycle_policy);
    let policy = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        artifacts.lifecycle_policy,
    )
    .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;
    // FOR THIS ACTION, because the LP AccountProfile is per-action and the LP
    // lifecycle policy is not. `encode_dealer_lp_account_profile_v3` takes the
    // action, and the frame it builds puts the Open payer and the Close
    // RentCredit at the SAME fixed slot 7 -- a payer must be debitable, a
    // RentCredit creditable, and no one permission set is both. Validating the
    // whole policy against one action's profile therefore refused the Close plan
    // against the Open frame with `ProfileMismatch`, which is a category error
    // and not a finding about the artifacts.
    policy
        .validate_account_profile_for_action(profile, u32::from(action.selector()))
        .map_err(DealerLpReleaseErrorV4::ProfileJoin)?;
    if policy.action_plan_count(u32::from(action.selector())) != Ok(1)
        || policy.current_rent_quote_count() != 1
    {
        return Err(DealerLpReleaseErrorV4::Geometry);
    }

    let mut request_scratch = vec![0_u8; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
    let mut request = vec![0_u8; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
    encode_dealer_lp_request_profile_v3(action, &mut request_scratch, &mut request)
        .map_err(|_| DealerLpReleaseErrorV4::Geometry)?;
    require_exact(&request, artifacts.request_profile)?;
    RequestProfileV1::decode(artifacts.request_profile)
        .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;

    let transition_bytes = dealer_lp_transition_bytes_v3(action);
    let mut transition_scratch = vec![0_u8; transition_bytes];
    let mut transition = vec![0_u8; transition_bytes];
    encode_dealer_lp_transition_v3(action, &mut transition_scratch, &mut transition)
        .map_err(|_| DealerLpReleaseErrorV4::Geometry)?;
    require_exact(&transition, artifacts.transition)?;
    TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| DealerLpReleaseErrorV4::Artifact)?;

    let effect_bytes = dealer_lp_effect_bytes_v4(action);
    let mut effect_scratch = vec![0_u8; effect_bytes];
    let mut effect = vec![0_u8; effect_bytes];
    encode_dealer_lp_effect_v4(action, &mut effect_scratch, &mut effect)?;
    require_exact(&effect, artifacts.effect)
}

fn reference(
    schema: [u8; 32],
    bytes: &[u8],
) -> Result<ArtifactReferenceV4, DealerLpReleaseErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(digest(bytes))?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DealerLpReleaseErrorV4> {
    ContentId::new(bytes).map_err(|_| DealerLpReleaseErrorV4::Artifact)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

fn require_exact(expected: &[u8], actual: &[u8]) -> Result<(), DealerLpReleaseErrorV4> {
    if expected == actual {
        Ok(())
    } else {
        Err(DealerLpReleaseErrorV4::Geometry)
    }
}

const _: () = {
    assert!(
        DEALER_LP_LIFECYCLE_BYTES_V5
            == HEADER_BYTES
                + RECIPE_BYTES
                + 4 * SEED_BYTES
                + 2 * ACTION_PLAN_BYTES
                + 2 * PROTECTED_OUTPUT_BYTES
                + 6 * IMMUTABLE_IDENTITY_BINDING_BYTES
                + CURRENT_RENT_QUOTE_BYTES_V5
    );
};

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use dclutch_market::execution_strategy::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    };
    use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;

    use super::*;
    use crate::dealer::lp_artifacts::{
        DEALER_LP_LINKED_BASIS_ACCOUNT_V3, DEALER_LP_OBLIGATION_ACCOUNT_V3,
    };

    fn admitted_strategy(transition: &[u8]) -> Vec<u8> {
        ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            content(SCHEMA_RELEASE_ID).expect("transition schema"),
            content(digest(transition)).expect("transition program"),
            content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("certificate schema"),
            Some(content([0x71; 32]).expect("certificate")),
            content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("admission schema"),
            Some(content([0x72; 32]).expect("admission")),
            content(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("request schema"),
            content(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("ack schema"),
        )
        .expect("admitted strategy")
        .to_bytes()
        .to_vec()
    }

    fn physical_lengths(action: MultiLpRequestActionV3) -> Vec<u32> {
        let mut lengths = vec![0; usize::from(dealer_lp_account_count_v3(action))];
        lengths[usize::from(DEALER_LP_LINKED_BASIS_ACCOUNT_V3)] = 64;
        lengths[usize::from(DEALER_LP_OBLIGATION_ACCOUNT_V3)] = 208;
        lengths[usize::from(DEALER_LP_STATE_ACCOUNT_V3)] =
            u32::try_from(DEALER_LP_POSITION_BYTES_V3).expect("position width");
        let credit = match action {
            MultiLpRequestActionV3::Open => 8,
            MultiLpRequestActionV3::Close => 7,
        };
        lengths[credit] = u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width");
        lengths
    }

    #[test]
    fn v5_lifecycle_owns_current_rent_without_growing_the_register_bank() {
        let mut scratch = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        let mut output = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        encode_dealer_lp_lifecycle_v5(&mut scratch, &mut output).expect("LP V5 lifecycle");
        let id = digest(&output);
        let lifecycle = StateLifecyclePolicyV5::decode_selected(id, id, &output).expect("decode");
        assert_eq!(lifecycle.current_rent_quote_count(), 1);
        let quote = lifecycle.current_rent_quote(0).expect("quote");
        assert_eq!(quote.exact_data_len(), 256);
        assert_eq!(
            quote.scalar_destination().index(),
            LP_OBSERVED_LAMPORTS_SCALAR_V3
        );
        assert_eq!(DEALER_LP_SCALAR_COUNT_V3, 20);
    }

    #[test]
    fn both_lp_effects_are_exact_v4_programs_with_full_semantic_coverage() {
        for action in [MultiLpRequestActionV3::Open, MultiLpRequestActionV3::Close] {
            let bytes = dealer_lp_effect_bytes_v4(action);
            let mut scratch = vec![0_u8; bytes];
            let mut output = vec![0_u8; bytes];
            encode_dealer_lp_effect_v4(action, &mut scratch, &mut output).expect("Effect V4");
            let effect = ProgramV4::decode(&output).expect("decode");
            assert_eq!(effect.semantic_prefix_bytes(), 312);
            assert_eq!(effect.range_count(), 0);
            assert_eq!(
                effect.base().fixed_account_count(),
                dealer_lp_account_count_v3(action)
            );
        }
    }

    #[test]
    fn both_lp_v4_descriptors_rederive_every_selected_artifact() {
        let mut lifecycle_scratch = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        let mut lifecycle = vec![0_u8; DEALER_LP_LIFECYCLE_BYTES_V5];
        encode_dealer_lp_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle).expect("lifecycle");
        for action in [MultiLpRequestActionV3::Open, MultiLpRequestActionV3::Close] {
            let lengths = physical_lengths(action);
            let profile_input = DealerLpAccountProfileInputV3 {
                action,
                logical_data_lengths: &lengths,
            };
            let profile = encode_dealer_lp_account_profile_v3(profile_input).expect("profile");
            let mut request_scratch = vec![0_u8; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            let mut request = vec![0_u8; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            encode_dealer_lp_request_profile_v3(action, &mut request_scratch, &mut request)
                .expect("request profile");
            let transition_bytes = dealer_lp_transition_bytes_v3(action);
            let mut transition_scratch = vec![0_u8; transition_bytes];
            let mut transition = vec![0_u8; transition_bytes];
            encode_dealer_lp_transition_v3(action, &mut transition_scratch, &mut transition)
                .expect("transition");
            let effect_bytes = dealer_lp_effect_bytes_v4(action);
            let mut effect_scratch = vec![0_u8; effect_bytes];
            let mut effect = vec![0_u8; effect_bytes];
            encode_dealer_lp_effect_v4(action, &mut effect_scratch, &mut effect).expect("effect");
            let strategy = admitted_strategy(&transition);
            let descriptor = finalize_dealer_lp_descriptor_v4(DealerLpFinalizedArtifactsV4 {
                account_profile_input: profile_input,
                account_profile: &profile,
                lifecycle_policy: &lifecycle,
                capacity_profile: &[1],
                effect: &effect,
                request_profile: &request,
                execution_strategy: &strategy,
                transition: &transition,
            })
            .expect("V4 descriptor");
            let decoded = CapabilityProgramV4::decode(&descriptor).expect("decode descriptor");
            assert_eq!(
                decoded.request_schema(),
                dealer_request_schema_v3(action.selector()).expect("request schema")
            );
            assert_eq!(decoded.effect().schema().to_bytes(), EFFECT_SCHEMA_ID_V4);
            // The per-root constant, NOT this selector's lifecycle digest. A
            // manifest carries one `child_derivation_id` per root, so the old
            // per-action law admitted exactly one selector per root.
            assert_eq!(
                decoded.derivation_policy().to_bytes(),
                CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1
            );
            assert_ne!(decoded.derivation_policy().to_bytes(), digest(&lifecycle));
            // The descriptor still binds its own lifecycle by content digest --
            // that binding never lived in `derivation_policy`.
            assert_eq!(decoded.lifecycle().program().to_bytes(), digest(&lifecycle));

            let last = effect.len().checked_sub(1).expect("effect byte");
            *effect.get_mut(last).expect("effect byte") ^= 1;
            assert_eq!(
                finalize_dealer_lp_descriptor_v4(DealerLpFinalizedArtifactsV4 {
                    account_profile_input: profile_input,
                    account_profile: &profile,
                    lifecycle_policy: &lifecycle,
                    capacity_profile: &[1],
                    effect: &effect,
                    request_profile: &request,
                    execution_strategy: &strategy,
                    transition: &transition,
                }),
                Err(DealerLpReleaseErrorV4::Geometry)
            );
        }
    }
}
