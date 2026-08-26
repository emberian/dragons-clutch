//! Packet-safe native Ed25519 evidence for Direct signed requests.
//!
//! Public keys and signatures are self-contained in the native instruction.
//! Message bytes remain in the exact authenticated current top-level
//! instruction. Direct Hot and the headerless Registry successor both carry
//! the exact Hot bytes beginning at byte zero; the Registry role is exposed by
//! a distinct successor encoder so the retired headered shape cannot replay.

use dclutch_capability_program_contract::hot_v3::{
    HOT_FAMILY_REQUEST_OFFSET_V3, HotExecutionEnvelopeV3,
};

use crate::execution_v3::{
    DirectExecutionActionV3, DirectExecutionRequestV3, native_signature_count_v3,
    native_signature_slice_v3,
};

const DESCRIPTOR_BYTES: usize = 14;
const SIGNATURES: usize = 2;
const HEADER_BYTES: usize = 2 + SIGNATURES * DESCRIPTOR_BYTES;
const PARTICIPANT_BYTES: usize = 32 + 64;

/// Exact compact native Ed25519 evidence width for two Direct participants.
pub const DIRECT_NATIVE_EVIDENCE_BYTES_V3: usize = HEADER_BYTES + SIGNATURES * PARTICIPANT_BYTES;
/// Direct Hot begins at byte zero of a top-level Trading instruction.
pub const DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3: usize = 0;

/// Authenticated top-level instruction shape containing the signed request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectNativeEvidenceContainerV3 {
    /// Trading Hot is the current top-level instruction.
    TradingHot,
}

impl DirectNativeEvidenceContainerV3 {
    const fn bias(self) -> usize {
        match self {
            Self::TradingHot => DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3,
        }
    }
}

/// Stable packet-evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectNativeEvidenceErrorV3 {
    /// Output did not have the exact fixed native-instruction width.
    InvalidOutputWidth,
    /// Current instruction bytes did not contain one canonical signed Direct request.
    InvalidCurrentInstruction,
    /// The selected action is unsigned or the detached signature count differed.
    SignatureCount,
    /// A maker identity or detached signature was zero.
    ZeroEvidence,
    /// A coordinate did not fit the native Ed25519 u16 wire.
    Coordinate,
}

/// Return the exact packet-safe native Ed25519 instruction-data width.
///
/// Product width remains `u32`; refusal here is only the physical native
/// instruction envelope, whose signature count field is one byte.
pub fn direct_native_evidence_bytes_v3(
    action: DirectExecutionActionV3,
    tail_count: u32,
) -> Result<usize, DirectNativeEvidenceErrorV3> {
    let signatures = native_signature_count_v3(action, tail_count);
    let count = usize::try_from(signatures).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
    if signatures == 0 || u8::try_from(signatures).is_err() {
        return Err(DirectNativeEvidenceErrorV3::SignatureCount);
    }
    2_usize
        .checked_add(
            count
                .checked_mul(DESCRIPTOR_BYTES)
                .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .and_then(|header| {
            count
                .checked_mul(PARTICIPANT_BYTES)
                .and_then(|payload| header.checked_add(payload))
        })
        .ok_or(DirectNativeEvidenceErrorV3::Coordinate)
}

/// Encode packet-safe native evidence for the exact action-selected request.
///
/// The signed message locations and widths come only from
/// `native_signature_slice_v3`; callers provide signatures but cannot supply
/// offsets, public keys, message bytes, or container bias. `output` remains
/// unchanged on every refusal.
#[allow(clippy::too_many_arguments)]
pub fn encode_direct_native_evidence_many_v3_atomic(
    container: DirectNativeEvidenceContainerV3,
    current_instruction_index: u16,
    current_instruction_data: &[u8],
    tail_count: u32,
    signatures: &[[u8; 64]],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectNativeEvidenceErrorV3> {
    let bias = container.bias();
    let hot = current_instruction_data
        .get(bias..)
        .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let (_, request) = HotExecutionEnvelopeV3::split_instruction(hot)
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let decoded = DirectExecutionRequestV3::decode(request, tail_count)
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let signature_count = native_signature_count_v3(decoded.action(), tail_count);
    let count =
        usize::try_from(signature_count).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
    let expected = direct_native_evidence_bytes_v3(decoded.action(), tail_count)?;
    if signatures.len() != count {
        return Err(DirectNativeEvidenceErrorV3::SignatureCount);
    }
    if scratch.len() != expected || output.len() != expected {
        return Err(DirectNativeEvidenceErrorV3::InvalidOutputWidth);
    }
    let header_bytes = 2_usize
        .checked_add(
            count
                .checked_mul(DESCRIPTOR_BYTES)
                .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;

    // Prevalidate every slice and every physical u16 coordinate before
    // touching scratch, so the second pass is infallible for these inputs.
    let mut participant = 0_usize;
    while participant < count {
        let slice = native_signature_slice_v3(
            decoded.action(),
            tail_count,
            u32::try_from(participant).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
        let message_offset = usize::try_from(slice.message_offset)
            .map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
        let public_key_offset = message_offset
            .checked_sub(32)
            .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
        let public_key = range(request, public_key_offset, 32)?;
        let message = range(request, message_offset, usize::from(slice.message_bytes))?;
        let signature = signatures
            .get(participant)
            .ok_or(DirectNativeEvidenceErrorV3::SignatureCount)?;
        if public_key.iter().all(|byte| *byte == 0)
            || signature.iter().all(|byte| *byte == 0)
            || message.iter().all(|byte| *byte == 0)
        {
            return Err(DirectNativeEvidenceErrorV3::ZeroEvidence);
        }
        let native_public_key = header_bytes
            .checked_add(
                participant
                    .checked_mul(PARTICIPANT_BYTES)
                    .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
            )
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let native_signature = native_public_key
            .checked_add(32)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let absolute_message = bias
            .checked_add(HOT_FAMILY_REQUEST_OFFSET_V3)
            .and_then(|offset| offset.checked_add(message_offset))
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        for value in [
            native_public_key,
            native_signature,
            absolute_message,
            usize::from(slice.message_bytes),
            usize::from(current_instruction_index),
        ] {
            u16::try_from(value).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
        }
        participant = participant
            .checked_add(1)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
    }

    scratch.fill(0);
    *scratch
        .get_mut(0)
        .ok_or(DirectNativeEvidenceErrorV3::InvalidOutputWidth)? =
        u8::try_from(signature_count).map_err(|_| DirectNativeEvidenceErrorV3::SignatureCount)?;
    participant = 0;
    while participant < count {
        let slice = native_signature_slice_v3(
            decoded.action(),
            tail_count,
            u32::try_from(participant).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
        let message_offset = usize::try_from(slice.message_offset)
            .map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
        let public_key_offset = message_offset
            .checked_sub(32)
            .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
        let descriptor = 2_usize
            .checked_add(
                participant
                    .checked_mul(DESCRIPTOR_BYTES)
                    .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
            )
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let native_public_key = header_bytes
            .checked_add(
                participant
                    .checked_mul(PARTICIPANT_BYTES)
                    .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
            )
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let native_signature = native_public_key
            .checked_add(32)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let absolute_message = bias
            .checked_add(HOT_FAMILY_REQUEST_OFFSET_V3)
            .and_then(|offset| offset.checked_add(message_offset))
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        for (offset, value) in [
            (descriptor, native_signature),
            (descriptor + 2, usize::from(u16::MAX)),
            (descriptor + 4, native_public_key),
            (descriptor + 6, usize::from(u16::MAX)),
            (descriptor + 8, absolute_message),
            (descriptor + 10, usize::from(slice.message_bytes)),
            (descriptor + 12, usize::from(current_instruction_index)),
        ] {
            put_u16(scratch, offset, value)?;
        }
        put(
            scratch,
            native_public_key,
            range(request, public_key_offset, 32)?,
        )?;
        put(
            scratch,
            native_signature,
            signatures
                .get(participant)
                .ok_or(DirectNativeEvidenceErrorV3::SignatureCount)?,
        )?;
        participant = participant
            .checked_add(1)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
    }
    output.copy_from_slice(scratch);
    Ok(())
}

/// Encode compact detached-message Ed25519 data atomically.
///
/// `current_instruction_index` is derived by the transaction assembler from
/// the complete top-level sequence. Callers cannot supply message offsets or a
/// raw bias. Headerless Registry evidence uses the distinct V4 encoder below.
pub fn encode_direct_native_evidence_v3_atomic(
    container: DirectNativeEvidenceContainerV3,
    current_instruction_index: u16,
    current_instruction_data: &[u8],
    signatures: [[u8; 64]; SIGNATURES],
    output: &mut [u8],
) -> Result<(), DirectNativeEvidenceErrorV3> {
    if output.len() != DIRECT_NATIVE_EVIDENCE_BYTES_V3 {
        return Err(DirectNativeEvidenceErrorV3::InvalidOutputWidth);
    }
    let mut scratch = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_native_evidence_many_v3_atomic(
        container,
        current_instruction_index,
        current_instruction_data,
        u32::MAX,
        &signatures,
        &mut scratch,
        output,
    )
}

/// Encode headerless Registry native evidence for an action-selected request.
///
/// The current outer Registry instruction is byte-identical to the selected
/// Hot instruction, so all message coordinates are Hot-relative with bias
/// zero. This successor is intentionally distinct from V3's retired headered
/// Registry container: callers cannot supply a bias or a nested byte offset.
#[allow(clippy::too_many_arguments)]
pub fn encode_direct_headerless_registry_native_evidence_many_v4_atomic(
    current_registry_instruction_index: u16,
    current_registry_instruction_data: &[u8],
    tail_count: u32,
    signatures: &[[u8; 64]],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectNativeEvidenceErrorV3> {
    encode_direct_native_evidence_many_v3_atomic(
        DirectNativeEvidenceContainerV3::TradingHot,
        current_registry_instruction_index,
        current_registry_instruction_data,
        tail_count,
        signatures,
        scratch,
        output,
    )
}

/// Encode the fixed two-participant headerless Registry native evidence.
pub fn encode_direct_headerless_registry_native_evidence_v4_atomic(
    current_registry_instruction_index: u16,
    current_registry_instruction_data: &[u8],
    signatures: [[u8; 64]; SIGNATURES],
    output: &mut [u8],
) -> Result<(), DirectNativeEvidenceErrorV3> {
    if output.len() != DIRECT_NATIVE_EVIDENCE_BYTES_V3 {
        return Err(DirectNativeEvidenceErrorV3::InvalidOutputWidth);
    }
    let mut scratch = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_headerless_registry_native_evidence_many_v4_atomic(
        current_registry_instruction_index,
        current_registry_instruction_data,
        u32::MAX,
        &signatures,
        &mut scratch,
        output,
    )
}

fn range(input: &[u8], offset: usize, bytes: usize) -> Result<&[u8], DirectNativeEvidenceErrorV3> {
    input
        .get(
            offset
                ..offset
                    .checked_add(bytes)
                    .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), DirectNativeEvidenceErrorV3> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
        )
        .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u16(
    output: &mut [u8],
    offset: usize,
    value: usize,
) -> Result<(), DirectNativeEvidenceErrorV3> {
    put(
        output,
        offset,
        &u16::try_from(value)
            .map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?
            .to_le_bytes(),
    )
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{
        execution_v3::{
            DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
            encode_header_v3,
        },
        intent_v2::{COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2, CompactIntentV2},
    };
    use dclutch_capability_program_contract::hot_v3::{
        HOT_FAMILY_REQUEST_OFFSET_V3, HotExecutionEnvelopeV3,
    };
    use std::{vec, vec::Vec};

    fn intent(side: u8, maker: u8) -> CompactIntentV2 {
        CompactIntentV2 {
            side,
            lifecycle: 1,
            outcome: 1,
            market: [7; 32],
            generation: 9,
            nonce: 0,
            valid_from: 1,
            valid_through: 10,
            maximum_fill: 5,
            limit_price: if side == 0 { 2 } else { 3 },
            fee_basis_points: 25,
            collateral_account: [maker.wrapping_add(20); 32],
        }
    }

    fn hot() -> Vec<u8> {
        let mut request = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3];
        let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut request)
            .expect("header");
        for (offset, maker, value) in [(0, 11_u8, intent(0, 11)), (204, 12, intent(1, 12))] {
            body.get_mut(offset..offset + 32)
                .expect("maker")
                .fill(maker);
            body.get_mut(offset + 32..offset + 32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2)
                .expect("message")
                .copy_from_slice(&value.signed_preimage().expect("preimage"));
        }
        body.get_mut(408..416)
            .expect("fill")
            .copy_from_slice(&5_u64.to_le_bytes());
        body.get_mut(416..424)
            .expect("price")
            .copy_from_slice(&2_u64.to_le_bytes());
        wrap_request(&request)
    }

    fn registration_hot() -> Vec<u8> {
        let mut request = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
        let body =
            encode_header_v3(DirectExecutionActionV3::RegisterBuy, &mut request).expect("header");
        body.get_mut(..32).expect("maker").fill(12);
        body.get_mut(32..32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2)
            .expect("message")
            .copy_from_slice(
                &CompactIntentV2 {
                    lifecycle: 2,
                    ..intent(1, 12)
                }
                .signed_preimage()
                .expect("preimage"),
            );
        body.get_mut(204..236).expect("maker credit").fill(31);
        body.get_mut(236..268).expect("record credit").fill(32);
        body.get_mut(268..276)
            .expect("maker rent")
            .copy_from_slice(&9_u64.to_le_bytes());
        body.get_mut(276..284)
            .expect("record rent")
            .copy_from_slice(&10_u64.to_le_bytes());
        wrap_request(&request)
    }

    fn wrap_request(request: &[u8]) -> Vec<u8> {
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(request.len()).expect("request width"),
            [3; 32],
            [7; 32],
            9,
            [4; 32],
        )
        .expect("envelope");
        let mut output = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
        output.extend_from_slice(&envelope.to_bytes());
        output.extend_from_slice(request);
        output
    }

    fn read_u16(input: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(
            input
                .get(offset..offset + 2)
                .expect("u16")
                .try_into()
                .expect("array"),
        )
    }

    #[test]
    fn direct_and_headerless_registry_use_exact_current_instruction_offsets() {
        let hot = hot();
        let mut direct = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        encode_direct_native_evidence_v3_atomic(
            DirectNativeEvidenceContainerV3::TradingHot,
            2,
            &hot,
            [[1; 64], [2; 64]],
            &mut direct,
        )
        .expect("direct evidence");
        let mut registry = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        encode_direct_headerless_registry_native_evidence_v4_atomic(
            3,
            &hot,
            [[1; 64], [2; 64]],
            &mut registry,
        )
        .expect("Registry evidence");
        assert_eq!(direct.len(), 222);
        for (descriptor, expected_offset) in [(2_usize, 192_u16), (16, 396)] {
            assert_eq!(read_u16(&direct, descriptor + 8), expected_offset);
            assert_eq!(read_u16(&direct, descriptor + 12), 2);
            assert_eq!(read_u16(&registry, descriptor + 8), expected_offset);
            assert_eq!(read_u16(&registry, descriptor + 12), 3);
            assert_eq!(read_u16(&direct, descriptor + 2), u16::MAX);
            assert_eq!(read_u16(&direct, descriptor + 6), u16::MAX);
        }
    }

    #[test]
    fn retired_headered_registry_shape_refuses_without_output_mutation() {
        let hot = hot();
        let mut headered = vec![0_u8; 128];
        headered.extend_from_slice(&hot);
        let mut output = [0x5a_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        let before = output;
        assert_eq!(
            encode_direct_headerless_registry_native_evidence_v4_atomic(
                3,
                &headered,
                [[1; 64], [2; 64]],
                &mut output,
            ),
            Err(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn malformed_current_bytes_refuse_without_output_mutation() {
        let mut output = [0x55_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        let before = output;
        assert_eq!(
            encode_direct_native_evidence_v3_atomic(
                DirectNativeEvidenceContainerV3::TradingHot,
                1,
                hot().get(..128).expect("short Hot"),
                [[1; 64], [2; 64]],
                &mut output,
            ),
            Err(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)
        );
        assert_eq!(output, before);
        assert_eq!(
            encode_direct_native_evidence_v3_atomic(
                DirectNativeEvidenceContainerV3::TradingHot,
                1,
                &hot(),
                [[0; 64], [2; 64]],
                &mut output,
            ),
            Err(DirectNativeEvidenceErrorV3::ZeroEvidence)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn one_signed_registration_uses_the_same_canonical_owner() {
        let hot = registration_hot();
        let expected = direct_native_evidence_bytes_v3(DirectExecutionActionV3::RegisterBuy, 0)
            .expect("one signature width");
        assert_eq!(expected, 112);
        let mut scratch = [0_u8; 112];
        let mut output = [0_u8; 112];
        encode_direct_native_evidence_many_v3_atomic(
            DirectNativeEvidenceContainerV3::TradingHot,
            4,
            &hot,
            0,
            &[[3; 64]],
            &mut scratch,
            &mut output,
        )
        .expect("registration evidence");
        assert_eq!(output[0], 1);
        assert_eq!(read_u16(&output, 10), 192);
        assert_eq!(read_u16(&output, 14), 4);
        assert_eq!(read_u16(&output, 4), u16::MAX);
        assert_eq!(read_u16(&output, 8), u16::MAX);

        let before = [0x77_u8; 112];
        output = before;
        assert_eq!(
            encode_direct_native_evidence_many_v3_atomic(
                DirectNativeEvidenceContainerV3::TradingHot,
                4,
                &hot,
                0,
                &[],
                &mut scratch,
                &mut output,
            ),
            Err(DirectNativeEvidenceErrorV3::SignatureCount)
        );
        assert_eq!(output, before);
    }
}
