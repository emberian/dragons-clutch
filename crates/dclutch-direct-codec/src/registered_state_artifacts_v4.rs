//! Data-defined maker and record first-use lifecycle for registered Direct V4.
//!
//! Registration uses two mandatory `AuthenticateOrCreate` plans.  The maker
//! replay root may be live or vacant, while the side-selected Transition
//! requires the registered record to be vacant.  Generic Trading owns PDA
//! derivation, current-Rent authentication, protected outputs, and creation;
//! this module contributes only the canonical Direct recipes and bindings.

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
    generated_intent_v2 as intent,
    registered_creation_artifacts_v4::{
        REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4, REGISTERED_IDENTITY_MAKER_STATE_OWNER_V4,
        REGISTERED_IDENTITY_MAKER_STATE_V4, REGISTERED_IDENTITY_MARKET_V4,
        REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4, REGISTERED_IDENTITY_RECORD_STATE_OWNER_V4,
        REGISTERED_IDENTITY_RECORD_STATE_V4, REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        REGISTERED_SCALAR_GENERATION_V4, REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4,
        REGISTERED_SCALAR_MAKER_BUMP_V4, REGISTERED_SCALAR_MAKER_CREATED_V4,
        REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4, REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4,
        REGISTERED_SCALAR_MAKER_PRINCIPAL_V4, REGISTERED_SCALAR_NONCE_V4,
        REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4, REGISTERED_SCALAR_RECORD_BUMP_V4,
        REGISTERED_SCALAR_RECORD_CREATED_V4, REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4,
        REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4, REGISTERED_SCALAR_RECORD_PRINCIPAL_V4,
    },
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2, DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2,
        DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2,
    },
};

/// Maker replay state coordinate in the registered creation AccountProfile.
pub const DIRECT_REGISTERED_MAKER_ACCOUNT_V4: u16 = 5;
/// Shared creation payer coordinate.
pub const DIRECT_REGISTERED_PAYER_ACCOUNT_V4: u16 = 6;
/// Maker replay RentCredit coordinate.
pub const DIRECT_REGISTERED_MAKER_RENT_CREDIT_ACCOUNT_V4: u16 = 7;
/// Registered record state coordinate.
pub const DIRECT_REGISTERED_RECORD_ACCOUNT_V4: u16 = 8;
/// Registered record RentCredit coordinate.
pub const DIRECT_REGISTERED_RECORD_RENT_CREDIT_ACCOUNT_V4: u16 = 10;

const RECIPE_COUNT: usize = 2;
const MAKER_SEED_COUNT: usize = 5;
const RECORD_SEED_COUNT: usize = 6;
const SEED_COUNT: usize = MAKER_SEED_COUNT + RECORD_SEED_COUNT;
const PLAN_COUNT: usize = 2;
const BINDING_COUNT: usize = 4;
const RENT_QUOTE_COUNT: usize = 2;
const MAKER_SEED_COUNT_U8: u8 = 5;
const RECORD_SEED_COUNT_U8: u8 = 6;
const RECORD_SEED_START: u16 = 5;
const MAKER_BYTES_U32: u32 = 152;
const RECORD_BYTES_U32: u32 = 268;
const _: () = assert!(DIRECT_MAKER_REPLAY_BYTES_V1 == MAKER_BYTES_U32 as usize);
const _: () = assert!(DIRECT_REGISTERED_RECORD_BYTES_V2 == RECORD_BYTES_U32 as usize);

/// Exact LifecycleV5 bytes for one side-selected registered creation action.
pub const DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5: usize = HEADER_BYTES
    + RECIPE_COUNT * RECIPE_BYTES
    + SEED_COUNT * SEED_BYTES
    + PLAN_COUNT * ACTION_PLAN_BYTES
    + PLAN_COUNT * PROTECTED_OUTPUT_BYTES
    + BINDING_COUNT * IMMUTABLE_IDENTITY_BINDING_BYTES
    + RENT_QUOTE_COUNT * CURRENT_RENT_QUOTE_BYTES_V5;

/// Stable registered lifecycle artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredStateArtifactErrorV4 {
    /// An action, account, register, or byte coordinate was invalid.
    Coordinate,
    /// The semantic-owner lifecycle encoder or hostile decoder refused.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
}

/// Emit the exact maker/record lifecycle for RegisterSell or RegisterBuy.
pub fn encode_direct_registered_creation_lifecycle_v5_atomic(
    action: DirectExecutionActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredStateArtifactErrorV4> {
    require_creation_action(action)?;
    if scratch.len() != DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5
        || output.len() != DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5
    {
        return Err(DirectRegisteredStateArtifactErrorV4::Coordinate);
    }
    let recipes = [
        recipe(
            DIRECT_REGISTERED_MAKER_ACCOUNT_V4,
            0,
            MAKER_SEED_COUNT_U8,
            MAKER_BYTES_U32,
        ),
        recipe(
            DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
            RECORD_SEED_START,
            RECORD_SEED_COUNT_U8,
            RECORD_BYTES_U32,
        ),
    ];
    let seeds = [
        LifecycleSeedInputV3::Literal(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2),
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar(REGISTERED_SCALAR_NONCE_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [
        plan(
            action,
            0,
            DIRECT_REGISTERED_MAKER_RENT_CREDIT_ACCOUNT_V4,
            REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4,
            REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4,
        )?,
        plan(
            action,
            1,
            DIRECT_REGISTERED_RECORD_RENT_CREDIT_ACCOUNT_V4,
            REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4,
            REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4,
        )?,
    ];
    let protected = [
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(REGISTERED_SCALAR_MAKER_CREATED_V4)?,
            bump_observation: scalar(REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4)?,
            bump: scalar(REGISTERED_SCALAR_MAKER_BUMP_V4)?,
            historical_rent_principal: scalar(REGISTERED_SCALAR_MAKER_PRINCIPAL_V4)?,
            beneficiary: identity(REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4)?,
            state: identity(REGISTERED_IDENTITY_MAKER_STATE_V4)?,
            owner: identity(REGISTERED_IDENTITY_MAKER_STATE_OWNER_V4)?,
        }),
        Some(LifecycleProtectedOutputsInputV3 {
            created: scalar(REGISTERED_SCALAR_RECORD_CREATED_V4)?,
            bump_observation: scalar(REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4)?,
            bump: scalar(REGISTERED_SCALAR_RECORD_BUMP_V4)?,
            historical_rent_principal: scalar(REGISTERED_SCALAR_RECORD_PRINCIPAL_V4)?,
            beneficiary: identity(REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4)?,
            state: identity(REGISTERED_IDENTITY_RECORD_STATE_V4)?,
            owner: identity(REGISTERED_IDENTITY_RECORD_STATE_OWNER_V4)?,
        }),
    ];
    let bindings = [
        binding(
            0,
            DirectMakerReplayLayoutV1::MARKET,
            REGISTERED_IDENTITY_MARKET_V4,
        )?,
        binding(
            0,
            DirectMakerReplayLayoutV1::MAKER,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        )?,
        binding(
            1,
            DirectRegisteredRecordLayoutV2::MAKER,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        )?,
        binding(
            1,
            DirectRegisteredRecordLayoutV2::INTENT + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            REGISTERED_IDENTITY_MARKET_V4,
        )?,
    ];
    let rent_quotes = [
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: MAKER_BYTES_U32,
            scalar_destination: scalar(REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4)?,
        },
        LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: RECORD_BYTES_U32,
            scalar_destination: scalar(REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4)?,
        },
    ];
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
    .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(DirectRegisteredStateArtifactErrorV4::Lifecycle)?;
    Ok(())
}

const fn recipe(
    state: u16,
    seed_start: u16,
    seed_count: u8,
    data_base: u32,
) -> LifecycleRecipeInputV3 {
    LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(state),
        seed_start,
        seed_count,
        bump_offset: seed_count - 1,
        data_base,
        data_stride: 0,
    }
}

fn plan(
    action: DirectExecutionActionV3,
    recipe: u16,
    rent_credit: u16,
    principal_observation: usize,
    beneficiary_observation: usize,
) -> Result<LifecyclePlanInputV3, DirectRegisteredStateArtifactErrorV4> {
    Ok(LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe,
        payer: Some(LifecycleAccountCoordinateV3::fixed(
            DIRECT_REGISTERED_PAYER_ACCOUNT_V4,
        )),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(rent_credit)),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar(
            principal_observation,
        )?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity(
            beneficiary_observation,
        )?)),
        guard: LifecycleGuardInputV3::Always,
    })
}

fn binding(
    plan: u16,
    data_offset: usize,
    canonical: usize,
) -> Result<LifecycleImmutableIdentityBindingInputV4, DirectRegisteredStateArtifactErrorV4> {
    Ok(LifecycleImmutableIdentityBindingInputV4 {
        plan,
        data_offset: u32::try_from(data_offset)
            .map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)?,
        canonical: LifecycleRegisterCoordinateV3::common(identity(canonical)?),
    })
}

const fn require_creation_action(
    action: DirectExecutionActionV3,
) -> Result<(), DirectRegisteredStateArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy => Ok(()),
        _ => Err(DirectRegisteredStateArtifactErrorV4::Coordinate),
    }
}

fn scalar(value: usize) -> Result<u16, DirectRegisteredStateArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)
}

fn identity(value: usize) -> Result<u16, DirectRegisteredStateArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredStateArtifactErrorV4::Coordinate)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::LifecycleOperationV3;
    use sha2::{Digest, Sha256};

    fn artifact(
        action: DirectExecutionActionV3,
    ) -> [u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5] {
        let mut scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        let mut output = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        encode_direct_registered_creation_lifecycle_v5_atomic(action, &mut scratch, &mut output)
            .expect("registered lifecycle");
        output
    }

    #[test]
    fn both_sides_select_two_protected_first_use_plans_and_exact_rent_quotes() {
        for action in [
            DirectExecutionActionV3::RegisterSell,
            DirectExecutionActionV3::RegisterBuy,
        ] {
            let bytes = artifact(action);
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            let policy = StateLifecyclePolicyV5::decode_selected(digest, digest, &bytes)
                .expect("lifecycle decode");
            assert_eq!(policy.action_plan_count(action as u32).expect("count"), 2);
            for ordinal in 0..2 {
                let selected = policy.action_plan(action as u32, ordinal).expect("plan");
                assert_eq!(
                    selected.operation(),
                    LifecycleOperationV3::AuthenticateOrCreate
                );
                assert!(selected.protected_outputs().expect("protected").is_some());
            }
            assert_eq!(policy.current_rent_quote_count(), 2);
            let maker = policy.current_rent_quote(0).expect("maker rent");
            let record = policy.current_rent_quote(1).expect("record rent");
            assert_eq!(maker.exact_data_len(), MAKER_BYTES_U32);
            assert_eq!(record.exact_data_len(), RECORD_BYTES_U32);
            assert_eq!(maker.scalar_destination().index(), 52);
            assert_eq!(record.scalar_destination().index(), 53);
        }
    }

    #[test]
    fn unsupported_action_or_wrong_width_preserves_output() {
        let mut scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        let mut output = [0x55_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
        let before = output;
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::FillRegisteredOrdinary,
                &mut scratch,
                &mut output,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::Coordinate)
        );
        assert_eq!(output, before);

        let mut short = [0x55_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5 - 1];
        let short_before = short;
        assert_eq!(
            encode_direct_registered_creation_lifecycle_v5_atomic(
                DirectExecutionActionV3::RegisterBuy,
                &mut scratch,
                &mut short,
            ),
            Err(DirectRegisteredStateArtifactErrorV4::Coordinate)
        );
        assert_eq!(short, short_before);
    }
}
