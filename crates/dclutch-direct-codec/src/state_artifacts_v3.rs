//! Data-defined maker replay lifecycle artifacts for inline Direct V3.
//!
//! The generic Trading lifecycle adapter owns PDA derivation, rent, canonical
//! bump selection, and current-program ownership. Direct contributes only the
//! immutable maker-state recipe and the register coordinates consumed by its
//! Transition and Effect programs.

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, IMMUTABLE_IDENTITY_BINDING_BYTES,
    PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV5,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
        LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3, LifecyclePlanInputV3,
        LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3, LifecycleRegisterCoordinateV3,
        LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
    },
};

use crate::{
    execution_v3::DirectExecutionActionV3,
    ordinary_v3::{
        IDENTITY_BUYER_MAKER_ROOT_V3, IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3,
        IDENTITY_BUYER_RENT_BENEFICIARY_V3, IDENTITY_BUYER_REQUEST_MAKER_V3,
        IDENTITY_BUYER_STATE_OWNER_V3, IDENTITY_MARKET_V3, IDENTITY_SELLER_MAKER_ROOT_V3,
        IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3, IDENTITY_SELLER_RENT_BENEFICIARY_V3,
        IDENTITY_SELLER_REQUEST_MAKER_V3, IDENTITY_SELLER_STATE_OWNER_V3,
        SCALAR_BUYER_BUMP_OBSERVATION_V3, SCALAR_BUYER_BUMP_V3, SCALAR_BUYER_CREATED_V3,
        SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3, SCALAR_BUYER_RENT_PRINCIPAL_V3,
        SCALAR_MAKER_CURRENT_RENT_MINIMUM_V5, SCALAR_MARKET_GENERATION_V3,
        SCALAR_SELLER_BUMP_OBSERVATION_V3, SCALAR_SELLER_BUMP_V3, SCALAR_SELLER_CREATED_V3,
        SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3, SCALAR_SELLER_RENT_PRINCIPAL_V3,
    },
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1, DirectMakerReplayLayoutV1,
    },
};

/// Seller replay-root state coordinate in the Direct AccountProfile.
pub const DIRECT_SELLER_MAKER_ACCOUNT_V3: u16 = 5;
/// Seller replay-root creation payer coordinate.
pub const DIRECT_SELLER_MAKER_PAYER_ACCOUNT_V3: u16 = 6;
/// Seller permanent RentCredit coordinate.
pub const DIRECT_SELLER_MAKER_RENT_CREDIT_ACCOUNT_V3: u16 = 7;
/// Buyer replay-root state coordinate in the Direct AccountProfile.
pub const DIRECT_BUYER_MAKER_ACCOUNT_V3: u16 = 8;
/// Buyer replay-root creation payer coordinate.
pub const DIRECT_BUYER_MAKER_PAYER_ACCOUNT_V3: u16 = 9;
/// Buyer permanent RentCredit coordinate.
pub const DIRECT_BUYER_MAKER_RENT_CREDIT_ACCOUNT_V3: u16 = 10;

const RECIPE_COUNT: usize = 2;
const SEEDS_PER_RECIPE: usize = 5;
const SEEDS_PER_RECIPE_U16: u16 = 5;
const SEEDS_PER_RECIPE_U8: u8 = 5;
const SEED_COUNT: usize = RECIPE_COUNT * SEEDS_PER_RECIPE;
const PLAN_COUNT: usize = 2;
const BINDING_COUNT: usize = 4;
const RENT_QUOTE_COUNT: usize = 1;
const _: () = assert!(DIRECT_MAKER_REPLAY_BYTES_V1 == 152);
const DIRECT_MAKER_REPLAY_BYTES_U32_V1: u32 = 152;

/// Exact Lifecycle V5 width for inline ordinary maker first-use.
pub const DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5: usize = HEADER_BYTES
    + RECIPE_COUNT * RECIPE_BYTES
    + SEED_COUNT * SEED_BYTES
    + PLAN_COUNT * ACTION_PLAN_BYTES
    + PLAN_COUNT * PROTECTED_OUTPUT_BYTES
    + BINDING_COUNT * IMMUTABLE_IDENTITY_BINDING_BYTES
    + RENT_QUOTE_COUNT * CURRENT_RENT_QUOTE_BYTES_V5;

/// Stable refusal from Direct maker lifecycle artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectStateArtifactErrorV3 {
    /// A register or account coordinate did not fit the shared ABI.
    Coordinate,
    /// The semantic-owner lifecycle encoder or hostile decoder refused.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
}

/// Emit the exact lifecycle policy for inline ordinary maker replay roots.
pub fn encode_direct_inline_ordinary_lifecycle_v5_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectStateArtifactErrorV3> {
    if scratch.len() != DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5
        || output.len() != DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5
    {
        return Err(DirectStateArtifactErrorV3::Coordinate);
    }
    let recipes = [
        maker_recipe(DIRECT_SELLER_MAKER_ACCOUNT_V3, 0),
        maker_recipe(DIRECT_BUYER_MAKER_ACCOUNT_V3, SEEDS_PER_RECIPE_U16),
    ];
    let seeds = [
        LifecycleSeedInputV3::Literal(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(identity(IDENTITY_MARKET_V3)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(SCALAR_MARKET_GENERATION_V3)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(IDENTITY_SELLER_REQUEST_MAKER_V3)?),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(identity(IDENTITY_MARKET_V3)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(SCALAR_MARKET_GENERATION_V3)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(IDENTITY_BUYER_REQUEST_MAKER_V3)?),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [
        maker_plan(
            0,
            DIRECT_SELLER_MAKER_PAYER_ACCOUNT_V3,
            DIRECT_SELLER_MAKER_RENT_CREDIT_ACCOUNT_V3,
            SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3,
            IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3,
        )?,
        maker_plan(
            1,
            DIRECT_BUYER_MAKER_PAYER_ACCOUNT_V3,
            DIRECT_BUYER_MAKER_RENT_CREDIT_ACCOUNT_V3,
            SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3,
            IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3,
        )?,
    ];
    let protected = [
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(SCALAR_SELLER_CREATED_V3)?,
            bump_observation: scalar(SCALAR_SELLER_BUMP_OBSERVATION_V3)?,
            bump: scalar(SCALAR_SELLER_BUMP_V3)?,
            historical_rent_principal: scalar(SCALAR_SELLER_RENT_PRINCIPAL_V3)?,
            beneficiary: identity(IDENTITY_SELLER_RENT_BENEFICIARY_V3)?,
            state: identity(IDENTITY_SELLER_MAKER_ROOT_V3)?,
            owner: identity(IDENTITY_SELLER_STATE_OWNER_V3)?,
        }),
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(SCALAR_BUYER_CREATED_V3)?,
            bump_observation: scalar(SCALAR_BUYER_BUMP_OBSERVATION_V3)?,
            bump: scalar(SCALAR_BUYER_BUMP_V3)?,
            historical_rent_principal: scalar(SCALAR_BUYER_RENT_PRINCIPAL_V3)?,
            beneficiary: identity(IDENTITY_BUYER_RENT_BENEFICIARY_V3)?,
            state: identity(IDENTITY_BUYER_MAKER_ROOT_V3)?,
            owner: identity(IDENTITY_BUYER_STATE_OWNER_V3)?,
        }),
    ];
    let bindings = [
        immutable_binding(0, DirectMakerReplayLayoutV1::MARKET, IDENTITY_MARKET_V3)?,
        immutable_binding(
            0,
            DirectMakerReplayLayoutV1::MAKER,
            IDENTITY_SELLER_REQUEST_MAKER_V3,
        )?,
        immutable_binding(1, DirectMakerReplayLayoutV1::MARKET, IDENTITY_MARKET_V3)?,
        immutable_binding(
            1,
            DirectMakerReplayLayoutV1::MAKER,
            IDENTITY_BUYER_REQUEST_MAKER_V3,
        )?,
    ];
    let rent_quotes = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: DIRECT_MAKER_REPLAY_BYTES_U32_V1,
        scalar_destination: scalar(SCALAR_MAKER_CURRENT_RENT_MINIMUM_V5)?,
    }];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &bindings,
        &rent_quotes,
        scratch,
        output,
    )
    .map_err(DirectStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(DirectStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

const fn maker_recipe(state: u16, seed_start: u16) -> LifecycleRecipeInputV3 {
    LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(state),
        seed_start,
        seed_count: SEEDS_PER_RECIPE_U8,
        bump_offset: SEEDS_PER_RECIPE_U8 - 1,
        data_base: DIRECT_MAKER_REPLAY_BYTES_U32_V1,
        data_stride: 0,
    }
}

fn maker_plan(
    recipe: u16,
    payer: u16,
    rent_credit: u16,
    principal: usize,
    beneficiary: usize,
) -> Result<LifecyclePlanInputV3, DirectStateArtifactErrorV3> {
    Ok(LifecyclePlanInputV3 {
        action: DirectExecutionActionV3::InlineOrdinary as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe,
        payer: Some(LifecycleAccountCoordinateV3::fixed(payer)),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(rent_credit)),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar(principal)?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity(
            beneficiary,
        )?)),
        guard: LifecycleGuardInputV3::Always,
    })
}

fn immutable_binding(
    plan: u16,
    data_offset: usize,
    canonical: usize,
) -> Result<LifecycleImmutableIdentityBindingInputV4, DirectStateArtifactErrorV3> {
    Ok(LifecycleImmutableIdentityBindingInputV4 {
        plan,
        data_offset: u32::try_from(data_offset)
            .map_err(|_| DirectStateArtifactErrorV3::Coordinate)?,
        canonical: LifecycleRegisterCoordinateV3::common(identity(canonical)?),
    })
}

fn scalar(value: usize) -> Result<u16, DirectStateArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectStateArtifactErrorV3::Coordinate)
}

fn identity(value: usize) -> Result<u16, DirectStateArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectStateArtifactErrorV3::Coordinate)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::LifecycleOperationV3;
    use sha2::{Digest, Sha256};

    fn artifact() -> [u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5] {
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5];
        let mut output = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5];
        encode_direct_inline_ordinary_lifecycle_v5_atomic(&mut scratch, &mut output)
            .expect("lifecycle");
        output
    }

    #[test]
    fn lifecycle_v5_round_trips_two_first_use_plans_and_one_rent_quote() {
        assert_eq!(DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5, 664);
        let bytes = artifact();
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let policy = StateLifecyclePolicyV5::decode_selected(digest, digest, &bytes)
            .expect("successor decode");
        assert_eq!(
            policy
                .action_plan_count(DirectExecutionActionV3::InlineOrdinary as u32)
                .expect("plan count"),
            2
        );
        for ordinal in 0..2 {
            let selected = policy
                .action_plan(DirectExecutionActionV3::InlineOrdinary as u32, ordinal)
                .expect("plan");
            assert_eq!(
                selected.operation(),
                LifecycleOperationV3::AuthenticateOrCreate
            );
            assert!(selected.protected_outputs().expect("protected").is_some());
        }
        assert_eq!(policy.current_rent_quote_count(), 1);
        let quote = policy.current_rent_quote(0).expect("rent quote");
        assert_eq!(quote.exact_data_len(), DIRECT_MAKER_REPLAY_BYTES_U32_V1);
        assert_eq!(quote.scalar_destination().index(), 64);
    }

    #[test]
    fn wrong_width_refuses_without_output_mutation() {
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5];
        let mut output = [0x55_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5 - 1];
        let before = output;
        assert_eq!(
            encode_direct_inline_ordinary_lifecycle_v5_atomic(&mut scratch, &mut output),
            Err(DirectStateArtifactErrorV3::Coordinate)
        );
        assert_eq!(output, before);
    }
}
