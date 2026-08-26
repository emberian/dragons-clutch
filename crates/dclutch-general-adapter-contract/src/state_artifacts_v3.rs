//! Action-selected General local-state lifecycle artifacts.
//!
//! These artifacts derive every nonroot state address from authenticated
//! General registers and let the generic Trading lifecycle adapter own the
//! canonical bump, current-Rent principal, immutable RentCredit beneficiary,
//! state key, and Trading owner.  Family evaluation never supplies those
//! protected values.

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, HEADER_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
    StateLifecyclePolicyV3,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
        LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
        LifecycleRegisterCoordinateV3, LifecycleSeedInputV3,
        encode_lifecycle_policy_with_protected_outputs_v3_atomic,
    },
};
use dclutch_general_codec::Action;

use crate::{
    hot_candidate_v3::{identity, scalar},
    local_state_v3::GENERAL_LOCAL_STATE_HEADER_BYTES_V3,
    runtime_selection::RUNTIME_SELECTION_CURSOR_BYTES_V2,
    runtime_width::{SETTLEMENT_CURSOR_HEADER_BYTES_V2, SettlementCursorLayoutV2},
};

/// First action-selected nonroot General state account.
pub const GENERAL_PRIMARY_STATE_ACCOUNT_V3: u16 = 5;
/// Close-only terminal record following the settlement state.
pub const GENERAL_TERMINAL_STATE_ACCOUNT_V3: u16 = 6;
/// Payer coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_PAYER_ACCOUNT_V3: u16 = 6;
/// RentCredit coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3: u16 = 7;
/// Close-only terminal-record payer coordinate.
pub const GENERAL_CLOSE_PAYER_ACCOUNT_V3: u16 = 7;
/// Shared Close terminal-create and settlement-close RentCredit coordinate.
pub const GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3: u16 = 8;

/// AccountProfile-owned live primary bump observation.
pub const GENERAL_PRIMARY_BUMP_OBSERVATION_SCALAR_V3: u16 = 71;
/// AccountProfile-owned live primary historical-principal observation.
pub const GENERAL_PRIMARY_PRINCIPAL_OBSERVATION_SCALAR_V3: u16 = 72;
/// Lifecycle-owned primary created/authenticated branch output.
pub const GENERAL_PRIMARY_CREATED_SCALAR_V3: u16 = 73;
/// Lifecycle-owned primary canonical bump output.
pub const GENERAL_PRIMARY_BUMP_SCALAR_V3: u16 = 74;
/// Lifecycle-owned primary historical-principal output.
pub const GENERAL_PRIMARY_PRINCIPAL_SCALAR_V3: u16 = 75;
/// AccountProfile-owned live terminal bump observation.
pub const GENERAL_TERMINAL_BUMP_OBSERVATION_SCALAR_V3: u16 = 76;
/// AccountProfile-owned live terminal historical-principal observation.
pub const GENERAL_TERMINAL_PRINCIPAL_OBSERVATION_SCALAR_V3: u16 = 77;
/// Lifecycle-owned terminal created/authenticated branch output.
pub const GENERAL_TERMINAL_CREATED_SCALAR_V3: u16 = 78;
/// Lifecycle-owned terminal canonical bump output.
pub const GENERAL_TERMINAL_BUMP_SCALAR_V3: u16 = 79;
/// Lifecycle-owned terminal historical-principal output.
pub const GENERAL_TERMINAL_PRINCIPAL_SCALAR_V3: u16 = 80;

/// AccountProfile-owned live primary RentCredit beneficiary observation.
pub const GENERAL_PRIMARY_BENEFICIARY_OBSERVATION_IDENTITY_V3: u16 = 32;
/// Lifecycle-owned primary beneficiary output.
pub const GENERAL_PRIMARY_BENEFICIARY_IDENTITY_V3: u16 = 33;
/// Lifecycle-owned primary state-key output.
pub const GENERAL_PRIMARY_STATE_IDENTITY_V3: u16 = 34;
/// Lifecycle-owned primary Trading-owner output.
pub const GENERAL_PRIMARY_OWNER_IDENTITY_V3: u16 = 35;
/// AccountProfile-owned live terminal RentCredit beneficiary observation.
pub const GENERAL_TERMINAL_BENEFICIARY_OBSERVATION_IDENTITY_V3: u16 = 36;
/// Lifecycle-owned terminal beneficiary output.
pub const GENERAL_TERMINAL_BENEFICIARY_IDENTITY_V3: u16 = 37;
/// Lifecycle-owned terminal state-key output.
pub const GENERAL_TERMINAL_STATE_IDENTITY_V3: u16 = 38;
/// Lifecycle-owned terminal Trading-owner output.
pub const GENERAL_TERMINAL_OWNER_IDENTITY_V3: u16 = 39;

const GENERAL_STATE_SEED_DOMAIN_V3: &[u8] = b"dclutch-general-state-v3";
const SELECTION_STATE_SEED_V3: &[u8] = b"selection";
const SETTLEMENT_STATE_SEED_V3: &[u8] = b"settlement";
const TERMINAL_STATE_SEED_V3: &[u8] = b"terminal";

/// Stable refusal from General lifecycle-artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralStateArtifactErrorV3 {
    /// A checked physical width overflowed.
    Geometry,
    /// The generic lifecycle semantic owner refused the candidate.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
}

/// Result alias for General state artifacts.
pub type Result<T> = core::result::Result<T, GeneralStateArtifactErrorV3>;

/// Exact lifecycle artifact width for one General action.
pub fn general_state_lifecycle_bytes_v3(action: Action) -> Result<usize> {
    let (recipes, seeds, plans) = lifecycle_counts(action);
    recipes
        .checked_mul(RECIPE_BYTES)
        .and_then(|value| value.checked_add(seeds.checked_mul(SEED_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(ACTION_PLAN_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(PROTECTED_OUTPUT_BYTES)?))
        .and_then(|value| HEADER_BYTES.checked_add(value))
        .ok_or(GeneralStateArtifactErrorV3::Geometry)
}

/// First child-route account after action-selected local lifecycle accounts.
pub const fn general_child_account_start_v3(action: Action) -> u16 {
    match action {
        Action::Consider | Action::Freeze => 8,
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 8,
        Action::Close => 9,
    }
}

/// Generate one complete protected-output lifecycle policy atomically.
pub fn encode_general_state_lifecycle_v3_atomic(
    action: Action,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_state_lifecycle_bytes_v3(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    if action == Action::Close {
        encode_close(action, scratch, output)
    } else {
        encode_primary(action, scratch, output)
    }
}

fn encode_primary(action: Action, scratch: &mut [u8], output: &mut [u8]) -> Result<()> {
    let selection = matches!(action, Action::Consider | Action::Freeze);
    let data_base = if selection {
        u32::try_from(
            GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                .checked_add(RUNTIME_SELECTION_CURSOR_BYTES_V2)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        )
        .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
    } else {
        u32::try_from(
            GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                .checked_add(SETTLEMENT_CURSOR_HEADER_BYTES_V2)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        )
        .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
    };
    let recipe = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: if selection { 4 } else { 5 },
        bump_offset: if selection { 3 } else { 4 },
        data_base,
        data_stride: if selection {
            0
        } else {
            SettlementCursorLayoutV2::inventory_stride()
        },
    }];
    let selection_seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::Literal(SELECTION_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let settlement_seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::Literal(SETTLEMENT_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plan = [LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe: 0,
        payer: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_PAYER_ACCOUNT_V3,
        )),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(
            GENERAL_PRIMARY_PRINCIPAL_OBSERVATION_SCALAR_V3,
        )),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(
            GENERAL_PRIMARY_BENEFICIARY_OBSERVATION_IDENTITY_V3,
        )),
        guard: LifecycleGuardInputV3::Always,
    }];
    let protected = [Some(primary_protected())];
    encode_lifecycle_policy_with_protected_outputs_v3_atomic(
        &recipe,
        if selection {
            &selection_seeds
        } else {
            &settlement_seeds
        },
        &plan,
        &protected,
        scratch,
        output,
    )
    .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV3::decode(output).map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

fn encode_close(action: Action, scratch: &mut [u8], output: &mut [u8]) -> Result<()> {
    let settlement_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(SETTLEMENT_CURSOR_HEADER_BYTES_V2)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            seed_start: 0,
            seed_count: 5,
            bump_offset: 4,
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            seed_start: 5,
            seed_count: 6,
            bump_offset: 5,
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
    ];
    let seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::Literal(SETTLEMENT_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar_u16(scalar::CURSOR_TERMINAL_COORDINATE)?,
            width: 8,
        },
        LifecycleSeedInputV3::Literal(TERMINAL_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    // Same action entries remain canonical by operation tag: Close precedes
    // AuthenticateOrCreate. Creation is nevertheless applied before Effects
    // by the generic Trading lifecycle adapter.
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Close,
            recipe: 0,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                GENERAL_PRIMARY_PRINCIPAL_OBSERVATION_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                GENERAL_PRIMARY_BENEFICIARY_OBSERVATION_IDENTITY_V3,
            )),
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 1,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                GENERAL_TERMINAL_PRINCIPAL_OBSERVATION_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                GENERAL_TERMINAL_BENEFICIARY_OBSERVATION_IDENTITY_V3,
            )),
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [None, Some(terminal_protected())];
    encode_lifecycle_policy_with_protected_outputs_v3_atomic(
        &recipes, &seeds, &plans, &protected, scratch, output,
    )
    .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV3::decode(output).map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    Ok(())
}

const fn primary_protected() -> LifecycleProtectedOutputsInputV3 {
    LifecycleProtectedOutputsInputV3 {
        created: GENERAL_PRIMARY_CREATED_SCALAR_V3,
        bump_observation: GENERAL_PRIMARY_BUMP_OBSERVATION_SCALAR_V3,
        bump: GENERAL_PRIMARY_BUMP_SCALAR_V3,
        historical_rent_principal: GENERAL_PRIMARY_PRINCIPAL_SCALAR_V3,
        beneficiary: GENERAL_PRIMARY_BENEFICIARY_IDENTITY_V3,
        state: GENERAL_PRIMARY_STATE_IDENTITY_V3,
        owner: GENERAL_PRIMARY_OWNER_IDENTITY_V3,
    }
}

const fn terminal_protected() -> LifecycleProtectedOutputsInputV3 {
    LifecycleProtectedOutputsInputV3 {
        created: GENERAL_TERMINAL_CREATED_SCALAR_V3,
        bump_observation: GENERAL_TERMINAL_BUMP_OBSERVATION_SCALAR_V3,
        bump: GENERAL_TERMINAL_BUMP_SCALAR_V3,
        historical_rent_principal: GENERAL_TERMINAL_PRINCIPAL_SCALAR_V3,
        beneficiary: GENERAL_TERMINAL_BENEFICIARY_IDENTITY_V3,
        state: GENERAL_TERMINAL_STATE_IDENTITY_V3,
        owner: GENERAL_TERMINAL_OWNER_IDENTITY_V3,
    }
}

const fn lifecycle_counts(action: Action) -> (usize, usize, usize) {
    match action {
        Action::Consider | Action::Freeze => (1, 4, 1),
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => (1, 5, 1),
        Action::Close => (2, 11, 2),
    }
}

fn scalar_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

fn identity_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    const ACTIONS: [Action; 7] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
    ];

    #[test]
    fn all_actions_encode_exact_canonical_lifecycle_geometry() {
        for action in ACTIONS {
            let width = general_state_lifecycle_bytes_v3(action).expect("width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output)
                .expect("lifecycle artifact");
            let policy = StateLifecyclePolicyV3::decode(&output).expect("decode");
            assert_eq!(
                policy.action_plan_count(action as u32),
                Ok(if action == Action::Close { 2 } else { 1 })
            );
            assert_eq!(
                policy
                    .action_plan(action as u32, if action == Action::Close { 1 } else { 0 })
                    .expect("selected")
                    .uses_canonical_bump(),
                Ok(true)
            );
        }
    }

    #[test]
    fn nonexact_capacity_preserves_output() {
        let action = Action::Close;
        let width = general_state_lifecycle_bytes_v3(action).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width - 1];
        let before = output.clone();
        assert_eq!(
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }
}
