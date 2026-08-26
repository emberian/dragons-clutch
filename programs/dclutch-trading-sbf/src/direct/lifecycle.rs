//! Exact joins from Direct state candidates to the generic lifecycle kernel.
//!
//! Direct owns the state semantics and canonical PDA coordinates. The common
//! Trading outer owns System/Rent/RentCredit authentication and execution. This
//! module only proves that every generic plan is the exact root/maker/record
//! operation selected by the accepted Direct transition.

use dclutch_account_profile_contract::lifecycle_v3::{
    AuthenticateStatePlanV3, CreateStatePlanV3, StateLifecyclePlanV3,
};
use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2, DIRECT_ROOT_STATE_BYTES_V1,
    DirectCoordinatesV1, MakerReplayCreationPlanV1, MakerReplayRootV1, MakerReplaySeedsV1,
    RegisteredIntentCreationV2, RegisteredIntentSeedsV2,
};
use solana_program::pubkey::Pubkey;

use super::physical::{DirectPhysicalError, Result};

/// Mandatory generic lifecycle plans for one accepted registered creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisteredCreationLifecycleV3 {
    /// Existing composite capability root authentication.
    pub root: StateLifecyclePlanV3,
    /// Existing or first-use maker replay state.
    pub maker: StateLifecyclePlanV3,
    /// Newly created registered intent record.
    pub record: StateLifecyclePlanV3,
}

/// Require exact generic lifecycle plans for an accepted Direct registration.
pub fn validate_registered_creation_lifecycle_v3(
    creation: RegisteredIntentCreationV2,
    trading_program: [u8; 32],
    direct_root: [u8; 32],
    lifecycle: DirectRegisteredCreationLifecycleV3,
) -> Result<()> {
    if trading_program == [0; 32] || direct_root == [0; 32] {
        return Err(DirectPhysicalError::ZeroIdentity);
    }
    match lifecycle.root {
        StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state, data_bytes, ..
        }) if state == direct_root
            && usize::try_from(data_bytes).ok()
                == CAPABILITY_ROOT_HEADER_BYTES_V1.checked_add(DIRECT_ROOT_STATE_BYTES_V1) => {}
        StateLifecyclePlanV3::Authenticate(_)
        | StateLifecyclePlanV3::Create(_)
        | StateLifecyclePlanV3::Close(_) => return Err(DirectPhysicalError::State),
    }

    let coordinates = DirectCoordinatesV1::new(
        creation.record.intent().market,
        creation.record.intent().generation,
    )
    .map_err(|_| DirectPhysicalError::State)?;
    let maker_seeds = MakerReplaySeedsV1::new(coordinates, creation.record.maker())
        .map_err(|_| DirectPhysicalError::State)?;
    let (maker, maker_bump) = derive(trading_program, &maker_seeds.as_slices());
    if maker_bump != creation.maker_root.bump() {
        return Err(DirectPhysicalError::State);
    }
    validate_maker_plan(
        maker,
        creation.maker_root,
        creation.maker_creation,
        lifecycle.maker,
    )?;

    let record_seeds = RegisteredIntentSeedsV2::from_record(creation.record);
    let (record, record_bump) = derive(trading_program, &record_seeds.as_slices());
    if record_bump != creation.record.bump() || record == maker || record == direct_root {
        return Err(DirectPhysicalError::State);
    }
    validate_create(
        lifecycle.record,
        record,
        DIRECT_REGISTERED_RECORD_BYTES_V2,
        creation.record.rent_owner(),
        creation.record.rent_principal(),
        creation.record.bump(),
        creation.record_creation,
    )
}

fn validate_maker_plan(
    state: [u8; 32],
    maker: MakerReplayRootV1,
    creation: Option<MakerReplayCreationPlanV1>,
    lifecycle: StateLifecyclePlanV3,
) -> Result<()> {
    match creation {
        None => match lifecycle {
            StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                state: observed,
                data_bytes,
                bump,
                ..
            }) if observed == state
                && usize::try_from(data_bytes).ok() == Some(DIRECT_MAKER_REPLAY_BYTES_V1)
                && bump == maker.bump() =>
            {
                Ok(())
            }
            StateLifecyclePlanV3::Authenticate(_)
            | StateLifecyclePlanV3::Create(_)
            | StateLifecyclePlanV3::Close(_) => Err(DirectPhysicalError::State),
        },
        Some(creation) => validate_create(
            lifecycle,
            state,
            DIRECT_MAKER_REPLAY_BYTES_V1,
            maker.rent_owner(),
            maker.rent_principal(),
            maker.bump(),
            creation,
        ),
    }
}

fn validate_create(
    lifecycle: StateLifecyclePlanV3,
    expected_state: [u8; 32],
    expected_data_bytes: usize,
    expected_beneficiary: [u8; 32],
    expected_principal: u64,
    expected_bump: u8,
    creation: MakerReplayCreationPlanV1,
) -> Result<()> {
    match lifecycle {
        StateLifecyclePlanV3::Create(CreateStatePlanV3 {
            state,
            payer,
            rent_credit,
            beneficiary,
            target_data_bytes,
            historical_rent_principal,
            state_before,
            state_after,
            payer_debit,
            bump,
            ..
        }) if state == expected_state
            && payer != [0; 32]
            && rent_credit != [0; 32]
            && payer != state
            && rent_credit != state
            && payer != rent_credit
            && beneficiary == expected_beneficiary
            && usize::try_from(target_data_bytes).ok() == Some(expected_data_bytes)
            && historical_rent_principal == expected_principal
            && state_before == creation.observed_lamports
            && state_after == creation.post_lamports
            && payer_debit == creation.top_up_lamports
            && bump == expected_bump =>
        {
            Ok(())
        }
        StateLifecyclePlanV3::Authenticate(_)
        | StateLifecyclePlanV3::Create(_)
        | StateLifecyclePlanV3::Close(_) => Err(DirectPhysicalError::State),
    }
}

fn derive(program: [u8; 32], seeds: &[&[u8]]) -> ([u8; 32], u8) {
    let (key, bump) = Pubkey::find_program_address(seeds, &Pubkey::new_from_array(program));
    (key.to_bytes(), bump)
}
