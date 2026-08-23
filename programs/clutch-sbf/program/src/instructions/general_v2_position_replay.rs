//! Current-state authentication for ordinary General Position/GEN1 Replay pairs.
//!
//! Settlement selectors deliberately carry no Replay ordinal. This adapter
//! decodes the exact purpose-owned Replay V3 envelope first, derives its sole
//! next ordinal from the authenticated body, and only then enters the shared
//! Position/Replay authenticator. A caller therefore cannot select an older or
//! future ordinal while preserving the same account identities.

use clutch_collateral_adapter_v2::BoundCollateralProfileV2;
use clutch_retirement::{PositionPurposeV3, ReplayV3Envelope, ReplayV3Lifecycle};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};

use super::collateral_position_v3::{
    authenticate_general_position_replay_v1, authenticate_general_position_replay_v2,
    GeneralPositionReplayAuthorityV1, GeneralPositionReplayAuthorityV2, RuntimeSha256,
};

fn current_general_replay_sequence_v1(
    replay_body: &[u8],
    expected_position: [u8; 32],
    expected_replay: [u8; 32],
    expected_binding: [u8; 32],
) -> Outcome<u64> {
    let envelope = ReplayV3Envelope::decode(replay_body, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    let header = envelope.header();
    require(
        header.lifecycle() == ReplayV3Lifecycle::Live
            && header.purpose() == PositionPurposeV3::General
            && header.position_account().bytes() == expected_position
            && header.replay_account().bytes() == expected_replay
            && header.purpose_binding_id().bytes() == expected_binding,
        ClutchError::Replay,
    )?;
    Ok(header.next_sequence())
}

/// Authenticate one current ordinary-General Position and its exact GEN1 Replay.
///
/// The shared authenticator rechecks program ownership, privileges, canonical
/// Position/Replay PDAs, full Position semantics, and the GEN1 extension. This
/// wrapper contributes only the body-owned next ordinal and the early exact
/// common-envelope partition.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_current_general_position_replay_v1(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
) -> Outcome<GeneralPositionReplayAuthorityV1> {
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let next_sequence = current_general_replay_sequence_v1(
        &replay_data,
        position_account.key.to_bytes(),
        replay_account.key.to_bytes(),
        market_runtime_account.key.to_bytes(),
    )?;
    drop(replay_data);

    authenticate_general_position_replay_v1(
        program_id,
        bound,
        market_binding_account,
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        next_sequence,
    )
}

/// Authenticate the current ordinary-General Position/Replay pair under the
/// sole live MarketBinding V2 account and derive the body-owned next ordinal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_current_general_position_replay_v2(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    expected_owner: [u8; 32],
) -> Outcome<GeneralPositionReplayAuthorityV2> {
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let next_sequence = current_general_replay_sequence_v1(
        &replay_data,
        position_account.key.to_bytes(),
        replay_account.key.to_bytes(),
        market_runtime_account.key.to_bytes(),
    )?;
    drop(replay_data);

    authenticate_general_position_replay_v2(
        program_id,
        bound,
        market_binding_account,
        market_runtime_account,
        position_account,
        replay_account,
        expected_owner,
        next_sequence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_retirement::{
        DeletableRentOwnerV1, Identity32V1, ReplayV3EnvelopeFields, ReplayV3EnvelopeHeader,
        ReplayV3ExtensionSchema, PURPOSE_REPLAY_V3_PREFIX_BYTES,
    };

    fn identity(byte: u8) -> Identity32V1 {
        Identity32V1::new([byte; 32]).unwrap()
    }

    fn replay_body(purpose: PositionPurposeV3) -> Vec<u8> {
        let extension = [9u8; 3];
        let header = ReplayV3EnvelopeHeader::new_live(
            ReplayV3EnvelopeFields {
                position_account: identity(1),
                replay_account: identity(2),
                purpose,
                purpose_binding_id: identity(3),
                position_generation: 7,
                next_sequence: 11,
                stored_bump: 4,
                rent: DeletableRentOwnerV1::from_persisted(identity(5), 10, 2).unwrap(),
            },
            ReplayV3ExtensionSchema::new(1).unwrap(),
            &extension,
            &RuntimeSha256,
        )
        .unwrap();
        let envelope = ReplayV3Envelope::from_header(header, &extension, &RuntimeSha256).unwrap();
        let mut body = vec![0u8; PURPOSE_REPLAY_V3_PREFIX_BYTES + extension.len()];
        envelope.encode_into(&mut body, &RuntimeSha256).unwrap();
        body
    }

    #[test]
    fn ordinal_is_body_owned_and_wrong_purpose_refuses() {
        let general = replay_body(PositionPurposeV3::General);
        assert_eq!(
            current_general_replay_sequence_v1(&general, [1; 32], [2; 32], [3; 32]).unwrap(),
            11
        );
        assert!(current_general_replay_sequence_v1(&general, [8; 32], [2; 32], [3; 32]).is_err());

        let dealer = replay_body(PositionPurposeV3::DealerFacility);
        assert!(current_general_replay_sequence_v1(&dealer, [1; 32], [2; 32], [3; 32]).is_err());
    }
}
