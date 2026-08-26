//! Packet-safe native Ed25519 evidence for Direct signed requests.
//!
//! Public keys and signatures are self-contained in the native instruction.
//! Message bytes remain in the exact authenticated current top-level
//! instruction: either Direct Hot itself or a Registry continuation containing
//! the unchanged nested Hot bytes after its fixed 128-byte header.

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        DirectExecutionActionV3, DirectExecutionRequestV3,
    },
    intent_v2::COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
};

const DESCRIPTOR_BYTES: usize = 14;
const SIGNATURES: usize = 2;
const HEADER_BYTES: usize = 2 + SIGNATURES * DESCRIPTOR_BYTES;
const PARTICIPANT_BYTES: usize = 32 + 64;
const HOT_MESSAGE_OFFSETS: [usize; SIGNATURES] = [192, 396];
const REQUEST_PARTICIPANT_OFFSETS: [usize; SIGNATURES] = [0, DIRECT_SIGNED_PARTICIPANT_BYTES_V3];

/// Exact compact native Ed25519 evidence width for two Direct participants.
pub const DIRECT_NATIVE_EVIDENCE_BYTES_V3: usize = HEADER_BYTES + SIGNATURES * PARTICIPANT_BYTES;
/// Direct Hot begins at byte zero of a top-level Trading instruction.
pub const DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3: usize = 0;
/// Registry's fixed continuation header precedes the unchanged nested Hot wire.
pub const DIRECT_NATIVE_EVIDENCE_REGISTRY_BIAS_V3: usize = 128;

/// Authenticated top-level instruction shape containing the signed request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectNativeEvidenceContainerV3 {
    /// Trading Hot is the current top-level instruction.
    TradingHot,
    /// Registry is current; unchanged Trading Hot follows its fixed header.
    RegistryContinuation,
}

impl DirectNativeEvidenceContainerV3 {
    const fn bias(self) -> usize {
        match self {
            Self::TradingHot => DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3,
            Self::RegistryContinuation => DIRECT_NATIVE_EVIDENCE_REGISTRY_BIAS_V3,
        }
    }
}

/// Stable packet-evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectNativeEvidenceErrorV3 {
    /// Output did not have the exact fixed native-instruction width.
    InvalidOutputWidth,
    /// Current instruction bytes did not contain one canonical InlineOrdinary request.
    InvalidCurrentInstruction,
    /// A maker identity or detached signature was zero.
    ZeroEvidence,
    /// A coordinate did not fit the native Ed25519 u16 wire.
    Coordinate,
}

/// Encode compact detached-message Ed25519 data atomically.
///
/// `current_instruction_index` is derived by the transaction assembler from
/// the complete top-level sequence. Callers cannot supply message offsets or a
/// raw bias: `container` selects one of the two authenticated protocol shapes.
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
    let bias = container.bias();
    let hot = current_instruction_data
        .get(bias..)
        .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let (_, request) = HotExecutionEnvelopeV3::split_instruction(hot)
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let decoded = DirectExecutionRequestV3::decode(request, u32::MAX)
        .map_err(|_| DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    if decoded.action() != DirectExecutionActionV3::InlineOrdinary {
        return Err(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction);
    }
    let body = request
        .get(DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3..)
        .ok_or(DirectNativeEvidenceErrorV3::InvalidCurrentInstruction)?;
    let mut candidate = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    candidate[0] = u8::try_from(SIGNATURES).map_err(|_| DirectNativeEvidenceErrorV3::Coordinate)?;
    for (index, participant_offset) in REQUEST_PARTICIPANT_OFFSETS.into_iter().enumerate() {
        let signature = signatures
            .get(index)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let hot_message_offset = HOT_MESSAGE_OFFSETS
            .get(index)
            .copied()
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        let public_key = range(body, participant_offset, 32)?;
        let message = range(
            body,
            participant_offset
                .checked_add(32)
                .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?,
            COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
        )?;
        if public_key.iter().all(|byte| *byte == 0)
            || signature.iter().all(|byte| *byte == 0)
            || message.iter().all(|byte| *byte == 0)
        {
            return Err(DirectNativeEvidenceErrorV3::ZeroEvidence);
        }
        let descriptor = 2 + index * DESCRIPTOR_BYTES;
        let native_public_key = HEADER_BYTES + index * PARTICIPANT_BYTES;
        let native_signature = native_public_key + 32;
        let message_offset = bias
            .checked_add(hot_message_offset)
            .ok_or(DirectNativeEvidenceErrorV3::Coordinate)?;
        for (offset, value) in [
            (descriptor, native_signature),
            (descriptor + 2, usize::from(u16::MAX)),
            (descriptor + 4, native_public_key),
            (descriptor + 6, usize::from(u16::MAX)),
            (descriptor + 8, message_offset),
            (descriptor + 10, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2),
        ] {
            put_u16(&mut candidate, offset, value)?;
        }
        put_u16(
            &mut candidate,
            descriptor + 12,
            usize::from(current_instruction_index),
        )?;
        put(&mut candidate, native_public_key, public_key)?;
        put(&mut candidate, native_signature, signature)?;
    }
    output.copy_from_slice(&candidate);
    Ok(())
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
        execution_v3::{DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3, encode_header_v3},
        intent_v2::CompactIntentV2,
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
        let envelope =
            HotExecutionEnvelopeV3::new(456, [3; 32], [7; 32], 9, [4; 32]).expect("envelope");
        let mut output = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
        output.extend_from_slice(&envelope.to_bytes());
        output.extend_from_slice(&request);
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
    fn direct_and_registry_use_exact_current_instruction_offsets() {
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
        let mut registry_data = vec![0_u8; DIRECT_NATIVE_EVIDENCE_REGISTRY_BIAS_V3];
        registry_data.extend_from_slice(&hot);
        let mut registry = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        encode_direct_native_evidence_v3_atomic(
            DirectNativeEvidenceContainerV3::RegistryContinuation,
            3,
            &registry_data,
            [[1; 64], [2; 64]],
            &mut registry,
        )
        .expect("Registry evidence");
        assert_eq!(direct.len(), 222);
        for (descriptor, direct_offset, registry_offset) in
            [(2_usize, 192_u16, 320_u16), (16, 396, 524)]
        {
            assert_eq!(read_u16(&direct, descriptor + 8), direct_offset);
            assert_eq!(read_u16(&direct, descriptor + 12), 2);
            assert_eq!(read_u16(&registry, descriptor + 8), registry_offset);
            assert_eq!(read_u16(&registry, descriptor + 12), 3);
            assert_eq!(read_u16(&direct, descriptor + 2), u16::MAX);
            assert_eq!(read_u16(&direct, descriptor + 6), u16::MAX);
        }
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
}
