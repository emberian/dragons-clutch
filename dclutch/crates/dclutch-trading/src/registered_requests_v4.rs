//! Canonical request emitters for the registered Direct successor.
//!
//! These functions encode the same [`crate::execution_v3::DirectExecutionRequestV3`]
//! selected by `CapabilityProgramSetV2`; they do not expose the legacy
//! `Registered*InstructionV1` controller wire. Every candidate is hostile-
//! decoded before the caller's output changes.

use crate::{
    execution_v3::{
        DIRECT_CANCEL_THROUGH_REQUEST_BYTES_V3, DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3,
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3,
        DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3, DirectCancelThroughRequestV3, DirectExecutionActionV3,
        DirectExecutionQuantityV3, DirectExecutionRequestV3, DirectRegistrationRequestV3,
        DirectSignedParticipantV3, DirectSignedTerminalRequestV3, encode_header_v3,
    },
    intent_v2::{CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2},
};

/// Stable registered request construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredRequestErrorV4 {
    /// Output had another exact action-selected width.
    InvalidLength,
    /// Action did not belong to the requested registered operation class.
    InvalidAction,
    /// Signed intent, side, lifecycle, quantity, or canonical field refused.
    InvalidRequest,
    /// Checked coordinate arithmetic refused.
    Arithmetic,
}

/// Encode one signed registered Sell or Buy admission atomically.
pub fn encode_direct_registration_request_v3_atomic(
    action: DirectExecutionActionV3,
    request: DirectRegistrationRequestV3,
    output: &mut [u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    if !matches!(
        action,
        DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy
    ) || output.len() != DIRECT_REGISTRATION_REQUEST_BYTES_V3
    {
        return Err(if output.len() != DIRECT_REGISTRATION_REQUEST_BYTES_V3 {
            DirectRegisteredRequestErrorV4::InvalidLength
        } else {
            DirectRegisteredRequestErrorV4::InvalidAction
        });
    }
    let mut candidate = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
    let body = encode_header_v3(action, &mut candidate)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    encode_participant(request.participant, body, 0)?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        &request.maker_rent_credit,
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32,
        &request.record_rent_credit,
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 64,
        &request.maker_rent_principal.to_le_bytes(),
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 72,
        &request.record_rent_principal.to_le_bytes(),
    )?;
    require_decoded(action, &candidate, 0)?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one matcher-selected registered ordinary, split, or merge quantity.
pub fn encode_direct_registered_execution_request_v3_atomic(
    action: DirectExecutionActionV3,
    request: DirectExecutionQuantityV3,
    tail_count: u32,
    output: &mut [u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    if !matches!(
        action,
        DirectExecutionActionV3::FillRegisteredOrdinary
            | DirectExecutionActionV3::SplitRegistered
            | DirectExecutionActionV3::MergeRegistered
    ) || output.len() != DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3
    {
        return Err(if output.len() != DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3 {
            DirectRegisteredRequestErrorV4::InvalidLength
        } else {
            DirectRegisteredRequestErrorV4::InvalidAction
        });
    }
    let mut candidate = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3];
    let body = encode_header_v3(action, &mut candidate)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    put(body, 0, &request.fill.to_le_bytes())?;
    put(body, 8, &request.execution_price.to_le_bytes())?;
    require_decoded(action, &candidate, tail_count)?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one maker-signed registered cancellation atomically.
pub fn encode_direct_registered_cancel_request_v3_atomic(
    request: DirectSignedTerminalRequestV3,
    tail_count: u32,
    output: &mut [u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    if output.len() != DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3 {
        return Err(DirectRegisteredRequestErrorV4::InvalidLength);
    }
    let mut candidate = [0_u8; DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::CancelRegistered, &mut candidate)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    encode_participant(
        DirectSignedParticipantV3 {
            maker: request.maker,
            intent: request.intent,
        },
        body,
        0,
    )?;
    require_decoded(
        DirectExecutionActionV3::CancelRegistered,
        &candidate,
        tail_count,
    )?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one maker-signed O(1) replay threshold update atomically.
pub fn encode_direct_cancel_through_request_v3_atomic(
    request: DirectCancelThroughRequestV3,
    tail_count: u32,
    output: &mut [u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    if output.len() != DIRECT_CANCEL_THROUGH_REQUEST_BYTES_V3 {
        return Err(DirectRegisteredRequestErrorV4::InvalidLength);
    }
    let mut candidate = [0_u8; DIRECT_CANCEL_THROUGH_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::CancelThrough, &mut candidate)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    put(body, 0, &request.maker)?;
    put(
        body,
        32,
        &request
            .cancellation
            .signed_preimage()
            .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?,
    )?;
    require_decoded(
        DirectExecutionActionV3::CancelThrough,
        &candidate,
        tail_count,
    )?;
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one permissionless or terminal empty-body registered action.
pub fn encode_direct_empty_action_request_v3_atomic(
    action: DirectExecutionActionV3,
    tail_count: u32,
    output: &mut [u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    if !matches!(
        action,
        DirectExecutionActionV3::ExpireRegistered
            | DirectExecutionActionV3::CloseInvalidated
            | DirectExecutionActionV3::CloseMakerReplay
            | DirectExecutionActionV3::CloseDirectRoot
    ) || output.len() != DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3
    {
        return Err(if output.len() != DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3 {
            DirectRegisteredRequestErrorV4::InvalidLength
        } else {
            DirectRegisteredRequestErrorV4::InvalidAction
        });
    }
    let mut candidate = [0_u8; DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3];
    encode_header_v3(action, &mut candidate)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    require_decoded(action, &candidate, tail_count)?;
    output.copy_from_slice(&candidate);
    Ok(())
}

fn encode_participant(
    participant: DirectSignedParticipantV3,
    output: &mut [u8],
    offset: usize,
) -> Result<(), DirectRegisteredRequestErrorV4> {
    put(output, offset, &participant.maker)?;
    let message = participant
        .intent
        .signed_preimage()
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    if message.len() != COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2 {
        return Err(DirectRegisteredRequestErrorV4::InvalidRequest);
    }
    put(
        output,
        offset
            .checked_add(32)
            .ok_or(DirectRegisteredRequestErrorV4::Arithmetic)?,
        &message,
    )
}

fn require_decoded(
    action: DirectExecutionActionV3,
    bytes: &[u8],
    tail_count: u32,
) -> Result<(), DirectRegisteredRequestErrorV4> {
    let decoded = DirectExecutionRequestV3::decode(bytes, tail_count)
        .map_err(|_| DirectRegisteredRequestErrorV4::InvalidRequest)?;
    if decoded.action() != action {
        return Err(DirectRegisteredRequestErrorV4::InvalidAction);
    }
    Ok(())
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), DirectRegisteredRequestErrorV4> {
    let end = offset
        .checked_add(value.len())
        .ok_or(DirectRegisteredRequestErrorV4::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(DirectRegisteredRequestErrorV4::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

const _: () =
    assert!(DIRECT_SIGNED_PARTICIPANT_BYTES_V3 == 32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
const _: () = assert!(
    DIRECT_CANCEL_THROUGH_REQUEST_BYTES_V3
        == DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 32 + CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_v2::{CancelThroughV2, CompactIntentV2};

    fn intent(side: u8) -> CompactIntentV2 {
        CompactIntentV2 {
            side,
            lifecycle: 2,
            outcome: 3,
            market: [7; 32],
            generation: 9,
            nonce: 4,
            valid_from: 10,
            valid_through: 20,
            maximum_fill: 11,
            limit_price: 5,
            fee_basis_points: 25,
            collateral_account: [8; 32],
        }
    }

    #[test]
    fn registration_fill_terminal_and_empty_requests_round_trip() {
        for (action, side) in [
            (DirectExecutionActionV3::RegisterSell, 0),
            (DirectExecutionActionV3::RegisterBuy, 1),
        ] {
            let mut bytes = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
            encode_direct_registration_request_v3_atomic(
                action,
                DirectRegistrationRequestV3 {
                    participant: DirectSignedParticipantV3 {
                        maker: [6; 32],
                        intent: intent(side),
                    },
                    maker_rent_credit: [10; 32],
                    record_rent_credit: [11; 32],
                    maker_rent_principal: 12,
                    record_rent_principal: 13,
                },
                &mut bytes,
            )
            .expect("registration");
            assert_eq!(
                DirectExecutionRequestV3::decode(&bytes, 17)
                    .expect("decode")
                    .action(),
                action
            );
        }

        let mut fill = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3];
        encode_direct_registered_execution_request_v3_atomic(
            DirectExecutionActionV3::FillRegisteredOrdinary,
            DirectExecutionQuantityV3 {
                fill: 4,
                execution_price: 5,
            },
            17,
            &mut fill,
        )
        .expect("fill");

        let mut cancel = [0_u8; DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3];
        encode_direct_registered_cancel_request_v3_atomic(
            DirectSignedTerminalRequestV3 {
                maker: [6; 32],
                intent: intent(0),
            },
            17,
            &mut cancel,
        )
        .expect("cancel");

        let mut cancel_through = [0_u8; DIRECT_CANCEL_THROUGH_REQUEST_BYTES_V3];
        encode_direct_cancel_through_request_v3_atomic(
            DirectCancelThroughRequestV3 {
                maker: [6; 32],
                cancellation: CancelThroughV2 {
                    market: [7; 32],
                    generation: 9,
                    minimum_live_nonce: 5,
                },
            },
            17,
            &mut cancel_through,
        )
        .expect("CancelThrough");

        for action in [
            DirectExecutionActionV3::ExpireRegistered,
            DirectExecutionActionV3::CloseInvalidated,
            DirectExecutionActionV3::CloseMakerReplay,
            DirectExecutionActionV3::CloseDirectRoot,
        ] {
            let mut bytes = [0_u8; DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3];
            encode_direct_empty_action_request_v3_atomic(action, 17, &mut bytes)
                .expect("empty action");
        }
    }

    #[test]
    fn action_side_width_and_signed_substitutions_preserve_output() {
        let mut output = [0x55_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
        let before = output;
        assert_eq!(
            encode_direct_registration_request_v3_atomic(
                DirectExecutionActionV3::RegisterSell,
                DirectRegistrationRequestV3 {
                    participant: DirectSignedParticipantV3 {
                        maker: [6; 32],
                        intent: intent(1),
                    },
                    maker_rent_credit: [10; 32],
                    record_rent_credit: [11; 32],
                    maker_rent_principal: 12,
                    record_rent_principal: 13,
                },
                &mut output,
            ),
            Err(DirectRegisteredRequestErrorV4::InvalidRequest)
        );
        assert_eq!(output, before);

        let mut short = [0x44_u8; DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3 - 1];
        let short_before = short;
        assert_eq!(
            encode_direct_registered_execution_request_v3_atomic(
                DirectExecutionActionV3::FillRegisteredOrdinary,
                DirectExecutionQuantityV3 {
                    fill: 1,
                    execution_price: 1,
                },
                2,
                &mut short,
            ),
            Err(DirectRegisteredRequestErrorV4::InvalidLength)
        );
        assert_eq!(short, short_before);
    }
}
