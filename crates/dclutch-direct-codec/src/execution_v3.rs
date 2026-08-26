//! Family-neutral Direct V3 request selector and hostile codec.
//!
//! [`DirectExecutionRequestV3`] is the sole request preimage selected through
//! `CapabilityProgramSetV1`. The set reads the canonical little-endian `u32`
//! action at offset 12; the selected RequestProfile independently rechecks the
//! same magic, version, action, reserved bytes, and exact Product-derived tail
//! width. Signed bodies reuse [`CompactIntentV2`] and [`CancelThroughV2`]
//! verbatim rather than defining another intent authority.

use crate::intent_v2::{
    CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
    CancelThroughV2, CompactIntentV2,
};

/// Domain-separating Direct execution-request magic.
pub const DIRECT_EXECUTION_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTDRQ3";
/// Exact successor request schema version.
pub const DIRECT_EXECUTION_REQUEST_VERSION_V3: u16 = 3;
/// Fixed request prefix selected by `CapabilityProgramSetV1`.
pub const DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3: usize = 32;
/// Offset of the canonical little-endian `u32` action selector.
pub const DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3: u32 = 12;
/// Width of one runtime complementary signed participant.
pub const DIRECT_SIGNED_PARTICIPANT_BYTES_V3: usize = 32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;
/// Exact fixed width of an inline ordinary Direct request.
pub const DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3: usize =
    DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 16;
/// Finalized-record schema label for [`DirectExecutionRequestV3`].
pub const DIRECT_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/direct-execution-request-v3";
/// SHA-256 of [`DIRECT_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x59, 0x04, 0x6a, 0xa7, 0x88, 0xe6, 0x05, 0x98, 0xfb, 0x4c, 0xad, 0xf3, 0x98, 0xcc, 0x5e, 0xc8,
    0x41, 0xd2, 0x01, 0x13, 0xea, 0xbc, 0x71, 0x52, 0x3c, 0xfc, 0x22, 0xad, 0x9f, 0x4d, 0x38, 0xc5,
];
/// Direct successor capability-kind label.
pub const DIRECT_SUCCESSOR_KIND_PREIMAGE_V3: &[u8] = b"dclutch/capability/direct-successor-v3";
/// SHA-256 of [`DIRECT_SUCCESSOR_KIND_PREIMAGE_V3`].
pub const DIRECT_SUCCESSOR_KIND_ID_V3: [u8; 32] = [
    0x2f, 0x9c, 0xf5, 0x05, 0xbd, 0x6a, 0x41, 0x7e, 0x88, 0x22, 0xce, 0xe2, 0xb4, 0x27, 0x24, 0x6d,
    0x7b, 0x2a, 0xd8, 0x25, 0x7a, 0x9a, 0xbe, 0xf5, 0xa8, 0x52, 0xc7, 0x24, 0x8f, 0x53, 0x31, 0x47,
];

const INLINE_ORDINARY_BODY_BYTES: usize = 80 + 2 * COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;
const REGISTRATION_BODY_BYTES: usize = 112 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;
const EXECUTION_BODY_BYTES: usize = 16;
const SIGNED_TERMINAL_BODY_BYTES: usize = 32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;
const CANCEL_THROUGH_BODY_BYTES: usize = 32 + CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2;

/// Descriptor selector values in strictly ascending canonical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DirectExecutionActionV3 {
    /// Immediate ordinary Sell/Buy match with two adjacent signatures.
    InlineOrdinary = 1,
    /// Admit one registered Sell and escrow claims in its record Position.
    RegisterSell = 2,
    /// Admit one registered Buy and deposit its worst-case collateral reserve.
    RegisterBuy = 3,
    /// Match one registered Sell and one registered Buy.
    FillRegisteredOrdinary = 4,
    /// Runtime-width registered Buy split against one Sell record per outcome.
    SplitRegistered = 5,
    /// Runtime-width registered Sell merge against one Buy record per outcome.
    MergeRegistered = 6,
    /// Maker-signed cancellation of one registered record.
    CancelRegistered = 7,
    /// Permissionless expiry after the signed inclusive slot interval.
    ExpireRegistered = 8,
    /// Permissionless close after maker `CancelThrough` invalidation.
    CloseInvalidated = 9,
    /// Maker-signed O(1) replay-root invalidation threshold.
    CancelThrough = 10,
    /// Close one terminal zero-live maker replay root.
    CloseMakerReplay = 11,
    /// Close the retiring zero-maker Direct capability root.
    CloseDirectRoot = 12,
    /// Runtime-width immediate complementary split.
    SplitInline = 13,
    /// Runtime-width immediate complementary merge.
    MergeInline = 14,
}

impl DirectExecutionActionV3 {
    fn decode(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::InlineOrdinary),
            2 => Ok(Self::RegisterSell),
            3 => Ok(Self::RegisterBuy),
            4 => Ok(Self::FillRegisteredOrdinary),
            5 => Ok(Self::SplitRegistered),
            6 => Ok(Self::MergeRegistered),
            7 => Ok(Self::CancelRegistered),
            8 => Ok(Self::ExpireRegistered),
            9 => Ok(Self::CloseInvalidated),
            10 => Ok(Self::CancelThrough),
            11 => Ok(Self::CloseMakerReplay),
            12 => Ok(Self::CloseDirectRoot),
            13 => Ok(Self::SplitInline),
            14 => Ok(Self::MergeInline),
            _ => Err(DirectExecutionRequestErrorV3::UnknownAction),
        }
    }
}

/// Stable hostile-request refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectExecutionRequestErrorV3 {
    /// Request width differed from the selected exact affine width.
    InvalidLength,
    /// Magic selected another request family.
    InvalidMagic,
    /// Schema version was not the Direct successor version.
    UnsupportedVersion,
    /// Reserved or inactive bytes were nonzero.
    NonCanonical,
    /// Selector did not name a supported successor action.
    UnknownAction,
    /// A required maker, beneficiary, quantity, or price was zero.
    InvalidField,
    /// An embedded CompactIntent or CancelThrough message refused.
    InvalidSignedBody,
    /// Signed side/lifecycle did not match the selected action.
    ActionMismatch,
    /// Product-derived affine width arithmetic overflowed.
    Arithmetic,
}

/// Result alias for Direct execution requests.
pub type Result<T> = core::result::Result<T, DirectExecutionRequestErrorV3>;

/// One canonical signed-message slice relative to the start of the complete
/// [`DirectExecutionRequestV3`] bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectNativeSignatureSliceV3 {
    /// Relative byte offset of the exact signed preimage, excluding the maker key.
    pub message_offset: u32,
    /// Exact signed-preimage width.
    pub message_bytes: u16,
    /// Canonical participant ordinal; zero for every non-affine signed action.
    pub participant: u32,
}

/// Number of required native signatures for one selected Direct action.
pub const fn native_signature_count_v3(action: DirectExecutionActionV3, tail_count: u32) -> u32 {
    match action {
        DirectExecutionActionV3::InlineOrdinary => 2,
        DirectExecutionActionV3::RegisterSell
        | DirectExecutionActionV3::RegisterBuy
        | DirectExecutionActionV3::CancelRegistered
        | DirectExecutionActionV3::CancelThrough => 1,
        DirectExecutionActionV3::SplitInline | DirectExecutionActionV3::MergeInline => tail_count,
        DirectExecutionActionV3::FillRegisteredOrdinary
        | DirectExecutionActionV3::SplitRegistered
        | DirectExecutionActionV3::MergeRegistered
        | DirectExecutionActionV3::ExpireRegistered
        | DirectExecutionActionV3::CloseInvalidated
        | DirectExecutionActionV3::CloseMakerReplay
        | DirectExecutionActionV3::CloseDirectRoot => 0,
    }
}

/// Return one exact Direct-relative native signed-message slice.
pub fn native_signature_slice_v3(
    action: DirectExecutionActionV3,
    tail_count: u32,
    participant: u32,
) -> Result<DirectNativeSignatureSliceV3> {
    if participant >= native_signature_count_v3(action, tail_count) {
        return Err(DirectExecutionRequestErrorV3::InvalidField);
    }
    let (message_offset, message_bytes) = match action {
        DirectExecutionActionV3::InlineOrdinary => {
            let stride = u32::try_from(DIRECT_SIGNED_PARTICIPANT_BYTES_V3)
                .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?;
            let offset = 64_u32
                .checked_add(
                    participant
                        .checked_mul(stride)
                        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?,
                )
                .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
            (offset, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2)
        }
        DirectExecutionActionV3::RegisterSell
        | DirectExecutionActionV3::RegisterBuy
        | DirectExecutionActionV3::CancelRegistered => {
            (64, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2)
        }
        DirectExecutionActionV3::CancelThrough => (64, CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2),
        DirectExecutionActionV3::SplitInline | DirectExecutionActionV3::MergeInline => {
            let stride = u32::try_from(DIRECT_SIGNED_PARTICIPANT_BYTES_V3)
                .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?;
            let offset = 80_u32
                .checked_add(
                    participant
                        .checked_mul(stride)
                        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?,
                )
                .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
            (offset, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2)
        }
        DirectExecutionActionV3::FillRegisteredOrdinary
        | DirectExecutionActionV3::SplitRegistered
        | DirectExecutionActionV3::MergeRegistered
        | DirectExecutionActionV3::ExpireRegistered
        | DirectExecutionActionV3::CloseInvalidated
        | DirectExecutionActionV3::CloseMakerReplay
        | DirectExecutionActionV3::CloseDirectRoot => {
            return Err(DirectExecutionRequestErrorV3::InvalidField);
        }
    };
    Ok(DirectNativeSignatureSliceV3 {
        message_offset,
        message_bytes: u16::try_from(message_bytes)
            .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?,
        participant,
    })
}

/// One signer and the exact signed CompactIntent DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSignedParticipantV3 {
    /// Public key authenticated by the adjacent native Ed25519 instruction.
    pub maker: [u8; 32],
    /// Exact V2 signed semantic intent.
    pub intent: CompactIntentV2,
}

/// Immediate ordinary execution request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryRequestV3 {
    /// Seller signature and intent.
    pub seller: DirectSignedParticipantV3,
    /// Buyer signature and intent.
    pub buyer: DirectSignedParticipantV3,
    /// Exact positive fill selected by the matcher.
    pub fill: u64,
    /// Exact positive execution price at the immutable config scale.
    pub execution_price: u64,
}

/// Signed first-use registration request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegistrationRequestV3 {
    /// Maker signature and registered intent.
    pub participant: DirectSignedParticipantV3,
    /// Canonical beneficiary RentCredit for the maker replay account.
    pub maker_rent_credit: [u8; 32],
    /// Canonical beneficiary RentCredit for the registered record.
    pub record_rent_credit: [u8; 32],
    /// Current maker replay rent minimum persisted as principal.
    pub maker_rent_principal: u64,
    /// Current record rent minimum persisted as principal.
    pub record_rent_principal: u64,
}

/// Matcher-selected registered or complementary execution quantities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExecutionQuantityV3 {
    /// Exact positive fill.
    pub fill: u64,
    /// Exact positive execution price at the immutable config scale.
    pub execution_price: u64,
}

/// Maker-signed terminal request for one exact registered intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSignedTerminalRequestV3 {
    /// Maker authenticated by the adjacent native Ed25519 instruction.
    pub maker: [u8; 32],
    /// Exact signed registered intent naming the record nonce.
    pub intent: CompactIntentV2,
}

/// Maker-signed O(1) invalidation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCancelThroughRequestV3 {
    /// Maker authenticated by the adjacent native Ed25519 instruction.
    pub maker: [u8; 32],
    /// Exact signed threshold DTO.
    pub cancellation: CancelThroughV2,
}

/// Borrowed runtime-width immediate complementary request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineComplementaryRequestV3<'a> {
    /// Split or merge.
    pub action: DirectExecutionActionV3,
    /// Exact positive common fill and price.
    pub execution: DirectExecutionQuantityV3,
    participants: &'a [u8],
    tail_count: u32,
}

impl DirectInlineComplementaryRequestV3<'_> {
    /// Product-authenticated participant count.
    pub const fn tail_count(self) -> u32 {
        self.tail_count
    }

    /// Decode one canonical outcome-ordered signed participant.
    pub fn participant(self, outcome: u32) -> Result<DirectSignedParticipantV3> {
        if outcome >= self.tail_count {
            return Err(DirectExecutionRequestErrorV3::InvalidLength);
        }
        let offset = usize::try_from(outcome)
            .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?
            .checked_mul(DIRECT_SIGNED_PARTICIPANT_BYTES_V3)
            .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
        decode_participant(slice(
            self.participants,
            offset,
            DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        )?)
    }
}

/// Exact hostile-decoded Direct execution request selected by the program set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectExecutionRequestV3<'a> {
    /// Immediate ordinary match.
    InlineOrdinary(DirectInlineOrdinaryRequestV3),
    /// Registered Sell admission.
    RegisterSell(DirectRegistrationRequestV3),
    /// Registered Buy admission.
    RegisterBuy(DirectRegistrationRequestV3),
    /// Registered ordinary match.
    FillRegisteredOrdinary(DirectExecutionQuantityV3),
    /// Runtime-width registered complementary split.
    SplitRegistered(DirectExecutionQuantityV3),
    /// Runtime-width registered complementary merge.
    MergeRegistered(DirectExecutionQuantityV3),
    /// Maker-signed terminal cancellation.
    CancelRegistered(DirectSignedTerminalRequestV3),
    /// Permissionless expiry.
    ExpireRegistered,
    /// Permissionless invalidated-record close.
    CloseInvalidated,
    /// Maker-signed replay invalidation threshold.
    CancelThrough(DirectCancelThroughRequestV3),
    /// Terminal maker replay close.
    CloseMakerReplay,
    /// Retiring Direct root close.
    CloseDirectRoot,
    /// Runtime-width immediate complementary execution.
    InlineComplementary(DirectInlineComplementaryRequestV3<'a>),
}

impl<'a> DirectExecutionRequestV3<'a> {
    /// Hostile-decode after Product authentication supplies the exact tail count.
    pub fn decode(input: &'a [u8], tail_count: u32) -> Result<Self> {
        if input.len() < DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 {
            return Err(DirectExecutionRequestErrorV3::InvalidLength);
        }
        if slice(input, 0, 8)? != DIRECT_EXECUTION_REQUEST_MAGIC_V3 {
            return Err(DirectExecutionRequestErrorV3::InvalidMagic);
        }
        if read_u16(input, 8)? != DIRECT_EXECUTION_REQUEST_VERSION_V3 {
            return Err(DirectExecutionRequestErrorV3::UnsupportedVersion);
        }
        if read_u16(input, 10)? != 0 || !all_zero(slice(input, 20, 12)?) {
            return Err(DirectExecutionRequestErrorV3::NonCanonical);
        }
        let action = DirectExecutionActionV3::decode(read_u32(input, 12)?)?;
        let body_bytes = usize::try_from(read_u32(input, 16)?)
            .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?;
        let expected = expected_body_bytes(action, tail_count)?;
        if body_bytes != expected
            || input.len()
                != DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3
                    .checked_add(expected)
                    .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?
        {
            return Err(DirectExecutionRequestErrorV3::InvalidLength);
        }
        let body = slice(input, DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, expected)?;
        match action {
            DirectExecutionActionV3::InlineOrdinary => {
                let seller =
                    decode_participant(slice(body, 0, DIRECT_SIGNED_PARTICIPANT_BYTES_V3)?)?;
                let buyer = decode_participant(slice(
                    body,
                    DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
                    DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
                )?)?;
                let quantities = decode_execution(body, 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3)?;
                require_inline(seller, 0)?;
                require_inline(buyer, 1)?;
                if seller.maker == buyer.maker {
                    return Err(DirectExecutionRequestErrorV3::ActionMismatch);
                }
                Ok(Self::InlineOrdinary(DirectInlineOrdinaryRequestV3 {
                    seller,
                    buyer,
                    fill: quantities.fill,
                    execution_price: quantities.execution_price,
                }))
            }
            DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy => {
                let participant =
                    decode_participant(slice(body, 0, DIRECT_SIGNED_PARTICIPANT_BYTES_V3)?)?;
                let expected_side = u8::from(action == DirectExecutionActionV3::RegisterBuy);
                if participant.intent.side != expected_side || participant.intent.lifecycle != 2 {
                    return Err(DirectExecutionRequestErrorV3::ActionMismatch);
                }
                let maker_rent_credit = array(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3)?;
                let record_rent_credit = array(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32)?;
                let maker_rent_principal = read_u64(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 64)?;
                let record_rent_principal =
                    read_u64(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 72)?;
                if maker_rent_credit == [0; 32]
                    || record_rent_credit == [0; 32]
                    || maker_rent_principal == 0
                    || record_rent_principal == 0
                {
                    return Err(DirectExecutionRequestErrorV3::InvalidField);
                }
                let request = DirectRegistrationRequestV3 {
                    participant,
                    maker_rent_credit,
                    record_rent_credit,
                    maker_rent_principal,
                    record_rent_principal,
                };
                if action == DirectExecutionActionV3::RegisterSell {
                    Ok(Self::RegisterSell(request))
                } else {
                    Ok(Self::RegisterBuy(request))
                }
            }
            DirectExecutionActionV3::FillRegisteredOrdinary
            | DirectExecutionActionV3::SplitRegistered
            | DirectExecutionActionV3::MergeRegistered => {
                let execution = decode_execution(body, 0)?;
                match action {
                    DirectExecutionActionV3::FillRegisteredOrdinary => {
                        Ok(Self::FillRegisteredOrdinary(execution))
                    }
                    DirectExecutionActionV3::SplitRegistered => {
                        Ok(Self::SplitRegistered(execution))
                    }
                    DirectExecutionActionV3::MergeRegistered => {
                        Ok(Self::MergeRegistered(execution))
                    }
                    _ => Err(DirectExecutionRequestErrorV3::UnknownAction),
                }
            }
            DirectExecutionActionV3::CancelRegistered => {
                let participant = decode_participant(body)?;
                if participant.intent.lifecycle != 2 {
                    return Err(DirectExecutionRequestErrorV3::ActionMismatch);
                }
                Ok(Self::CancelRegistered(DirectSignedTerminalRequestV3 {
                    maker: participant.maker,
                    intent: participant.intent,
                }))
            }
            DirectExecutionActionV3::CancelThrough => {
                let maker = array(body, 0)?;
                require_nonzero(maker)?;
                let cancellation = CancelThroughV2::decode_signed_preimage(slice(
                    body,
                    32,
                    CANCEL_THROUGH_SIGNED_PREIMAGE_BYTES_V2,
                )?)
                .map_err(|_| DirectExecutionRequestErrorV3::InvalidSignedBody)?;
                Ok(Self::CancelThrough(DirectCancelThroughRequestV3 {
                    maker,
                    cancellation,
                }))
            }
            DirectExecutionActionV3::SplitInline | DirectExecutionActionV3::MergeInline => {
                let execution = decode_execution(body, 0)?;
                let participants = slice(
                    body,
                    EXECUTION_BODY_BYTES,
                    body.len() - EXECUTION_BODY_BYTES,
                )?;
                let request = DirectInlineComplementaryRequestV3 {
                    action,
                    execution,
                    participants,
                    tail_count,
                };
                let mut outcome = 0_u32;
                while outcome < tail_count {
                    let participant = request.participant(outcome)?;
                    let side = u8::from(action == DirectExecutionActionV3::SplitInline);
                    require_inline(participant, side)?;
                    if participant.intent.outcome != outcome {
                        return Err(DirectExecutionRequestErrorV3::ActionMismatch);
                    }
                    outcome = outcome
                        .checked_add(1)
                        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
                }
                Ok(Self::InlineComplementary(request))
            }
            DirectExecutionActionV3::ExpireRegistered => Ok(Self::ExpireRegistered),
            DirectExecutionActionV3::CloseInvalidated => Ok(Self::CloseInvalidated),
            DirectExecutionActionV3::CloseMakerReplay => Ok(Self::CloseMakerReplay),
            DirectExecutionActionV3::CloseDirectRoot => Ok(Self::CloseDirectRoot),
        }
    }

    /// Selected canonical action.
    pub const fn action(self) -> DirectExecutionActionV3 {
        match self {
            Self::InlineOrdinary(_) => DirectExecutionActionV3::InlineOrdinary,
            Self::RegisterSell(_) => DirectExecutionActionV3::RegisterSell,
            Self::RegisterBuy(_) => DirectExecutionActionV3::RegisterBuy,
            Self::FillRegisteredOrdinary(_) => DirectExecutionActionV3::FillRegisteredOrdinary,
            Self::SplitRegistered(_) => DirectExecutionActionV3::SplitRegistered,
            Self::MergeRegistered(_) => DirectExecutionActionV3::MergeRegistered,
            Self::CancelRegistered(_) => DirectExecutionActionV3::CancelRegistered,
            Self::ExpireRegistered => DirectExecutionActionV3::ExpireRegistered,
            Self::CloseInvalidated => DirectExecutionActionV3::CloseInvalidated,
            Self::CancelThrough(_) => DirectExecutionActionV3::CancelThrough,
            Self::CloseMakerReplay => DirectExecutionActionV3::CloseMakerReplay,
            Self::CloseDirectRoot => DirectExecutionActionV3::CloseDirectRoot,
            Self::InlineComplementary(value) => value.action,
        }
    }
}

/// Encode a header into an exact caller-owned request buffer.
pub fn encode_header_v3(action: DirectExecutionActionV3, output: &mut [u8]) -> Result<&mut [u8]> {
    if output.len() < DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 {
        return Err(DirectExecutionRequestErrorV3::InvalidLength);
    }
    output.fill(0);
    let body_bytes = output
        .len()
        .checked_sub(DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3)
        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
    put(output, 0, &DIRECT_EXECUTION_REQUEST_MAGIC_V3)?;
    put(
        output,
        8,
        &DIRECT_EXECUTION_REQUEST_VERSION_V3.to_le_bytes(),
    )?;
    put(output, 12, &(action as u32).to_le_bytes())?;
    put(
        output,
        16,
        &u32::try_from(body_bytes)
            .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?
            .to_le_bytes(),
    )?;
    slice_mut(output, DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, body_bytes)
}

fn expected_body_bytes(action: DirectExecutionActionV3, tail_count: u32) -> Result<usize> {
    match action {
        DirectExecutionActionV3::InlineOrdinary => Ok(INLINE_ORDINARY_BODY_BYTES),
        DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy => {
            Ok(REGISTRATION_BODY_BYTES)
        }
        DirectExecutionActionV3::FillRegisteredOrdinary
        | DirectExecutionActionV3::SplitRegistered
        | DirectExecutionActionV3::MergeRegistered => Ok(EXECUTION_BODY_BYTES),
        DirectExecutionActionV3::CancelRegistered => Ok(SIGNED_TERMINAL_BODY_BYTES),
        DirectExecutionActionV3::CancelThrough => Ok(CANCEL_THROUGH_BODY_BYTES),
        DirectExecutionActionV3::SplitInline | DirectExecutionActionV3::MergeInline => {
            usize::try_from(tail_count)
                .map_err(|_| DirectExecutionRequestErrorV3::Arithmetic)?
                .checked_mul(DIRECT_SIGNED_PARTICIPANT_BYTES_V3)
                .and_then(|tail| EXECUTION_BODY_BYTES.checked_add(tail))
                .ok_or(DirectExecutionRequestErrorV3::Arithmetic)
        }
        DirectExecutionActionV3::ExpireRegistered
        | DirectExecutionActionV3::CloseInvalidated
        | DirectExecutionActionV3::CloseMakerReplay
        | DirectExecutionActionV3::CloseDirectRoot => Ok(0),
    }
}

fn decode_participant(input: &[u8]) -> Result<DirectSignedParticipantV3> {
    if input.len() != DIRECT_SIGNED_PARTICIPANT_BYTES_V3 {
        return Err(DirectExecutionRequestErrorV3::InvalidLength);
    }
    let maker = array(input, 0)?;
    require_nonzero(maker)?;
    let intent = CompactIntentV2::decode_signed_preimage(slice(
        input,
        32,
        COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
    )?)
    .map_err(|_| DirectExecutionRequestErrorV3::InvalidSignedBody)?;
    Ok(DirectSignedParticipantV3 { maker, intent })
}

fn require_inline(participant: DirectSignedParticipantV3, side: u8) -> Result<()> {
    if participant.intent.side != side || participant.intent.lifecycle > 1 {
        Err(DirectExecutionRequestErrorV3::ActionMismatch)
    } else {
        Ok(())
    }
}

fn decode_execution(input: &[u8], offset: usize) -> Result<DirectExecutionQuantityV3> {
    let fill = read_u64(input, offset)?;
    let execution_price = read_u64(input, offset + 8)?;
    if fill == 0 || execution_price == 0 {
        return Err(DirectExecutionRequestErrorV3::InvalidField);
    }
    Ok(DirectExecutionQuantityV3 {
        fill,
        execution_price,
    })
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value == [0; 32] {
        Err(DirectExecutionRequestErrorV3::InvalidField)
    } else {
        Ok(())
    }
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    slice(input, offset, 32)?
        .try_into()
        .map_err(|_| DirectExecutionRequestErrorV3::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| DirectExecutionRequestErrorV3::InvalidLength)?,
    ))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(input, offset, 4)?
            .try_into()
            .map_err(|_| DirectExecutionRequestErrorV3::InvalidLength)?,
    ))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| DirectExecutionRequestErrorV3::InvalidLength)?,
    ))
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
    input
        .get(offset..end)
        .ok_or(DirectExecutionRequestErrorV3::InvalidLength)
}

fn slice_mut(input: &mut [u8], offset: usize, width: usize) -> Result<&mut [u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(DirectExecutionRequestErrorV3::Arithmetic)?;
    input
        .get_mut(offset..end)
        .ok_or(DirectExecutionRequestErrorV3::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    slice_mut(output, offset, bytes.len())?.copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn intent(side: u8, lifecycle: u8, outcome: u32) -> CompactIntentV2 {
        CompactIntentV2 {
            side,
            lifecycle,
            outcome,
            market: id(1),
            generation: 4,
            nonce: u64::from(outcome),
            valid_from: 2,
            valid_through: 20,
            maximum_fill: 100,
            limit_price: if side == 0 { 40 } else { 60 },
            fee_basis_points: 100,
            collateral_account: id(20 + u8::try_from(outcome).unwrap_or(0)),
        }
    }

    fn participant(body: &mut [u8], offset: usize, maker: [u8; 32], value: CompactIntentV2) {
        put(body, offset, &maker).expect("maker");
        put(
            body,
            offset + 32,
            &value.signed_preimage().expect("intent encoding"),
        )
        .expect("intent");
    }

    #[test]
    fn program_set_selector_and_registered_requests_are_exact() {
        let mut fill = [0_u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + EXECUTION_BODY_BYTES];
        let body = encode_header_v3(DirectExecutionActionV3::FillRegisteredOrdinary, &mut fill)
            .expect("header");
        put(body, 0, &20_u64.to_le_bytes()).expect("fill");
        put(body, 8, &50_u64.to_le_bytes()).expect("price");
        assert_eq!(read_u32(&fill, 12), Ok(4));
        assert_eq!(
            DirectExecutionRequestV3::decode(&fill, 3),
            Ok(DirectExecutionRequestV3::FillRegisteredOrdinary(
                DirectExecutionQuantityV3 {
                    fill: 20,
                    execution_price: 50,
                },
            ))
        );

        let mut registration =
            vec![0_u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + REGISTRATION_BODY_BYTES];
        let body = encode_header_v3(DirectExecutionActionV3::RegisterSell, &mut registration)
            .expect("header");
        participant(body, 0, id(2), intent(0, 2, 1));
        put(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3, &id(80)).expect("maker rent");
        put(body, DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32, &id(81)).expect("record rent");
        put(
            body,
            DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 64,
            &100_u64.to_le_bytes(),
        )
        .expect("maker principal");
        put(
            body,
            DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 72,
            &110_u64.to_le_bytes(),
        )
        .expect("record principal");
        assert!(matches!(
            DirectExecutionRequestV3::decode(&registration, 3),
            Ok(DirectExecutionRequestV3::RegisterSell(_))
        ));
        let mut hostile = registration;
        *hostile.get_mut(12).expect("selector") = DirectExecutionActionV3::RegisterBuy as u8;
        assert_eq!(
            DirectExecutionRequestV3::decode(&hostile, 3),
            Err(DirectExecutionRequestErrorV3::ActionMismatch)
        );
    }

    #[test]
    fn runtime_inline_complement_is_product_width_and_outcome_ordered() {
        let tail_count = 3_u32;
        let body_bytes = EXECUTION_BODY_BYTES + 3 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
        let mut request = vec![0_u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + body_bytes];
        let body =
            encode_header_v3(DirectExecutionActionV3::SplitInline, &mut request).expect("header");
        put(body, 0, &10_u64.to_le_bytes()).expect("fill");
        put(body, 8, &50_u64.to_le_bytes()).expect("price");
        for outcome in 0..tail_count {
            participant(
                body,
                EXECUTION_BODY_BYTES
                    + usize::try_from(outcome).expect("outcome")
                        * DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
                id(2 + u8::try_from(outcome).expect("maker")),
                intent(1, 1, outcome),
            );
        }
        let decoded =
            DirectExecutionRequestV3::decode(&request, tail_count).expect("runtime-width request");
        if let DirectExecutionRequestV3::InlineComplementary(decoded) = decoded {
            assert_eq!(decoded.tail_count(), 3);
            assert_eq!(decoded.participant(2).expect("item").intent.outcome, 2);
        } else {
            assert!(matches!(
                decoded,
                DirectExecutionRequestV3::InlineComplementary(_)
            ));
        }

        let before = request.clone();
        assert_eq!(
            DirectExecutionRequestV3::decode(&request, 2),
            Err(DirectExecutionRequestErrorV3::InvalidLength)
        );
        assert_eq!(request, before);
        let last = request.len() - COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;
        let outcome_offset = last + 32 + 16;
        *request.get_mut(outcome_offset).expect("outcome") = 1;
        assert_eq!(
            DirectExecutionRequestV3::decode(&request, tail_count),
            Err(DirectExecutionRequestErrorV3::ActionMismatch)
        );
    }

    #[test]
    fn reserved_short_zero_and_legacy_v1_refuse() {
        let mut request = [0_u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3];
        encode_header_v3(DirectExecutionActionV3::ExpireRegistered, &mut request).expect("header");
        for offset in [0_usize, 8, 10, 20] {
            let mut hostile = request;
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(DirectExecutionRequestV3::decode(&hostile, 3).is_err());
        }
        assert_eq!(
            DirectExecutionRequestV3::decode(&request[..31], 3),
            Err(DirectExecutionRequestErrorV3::InvalidLength)
        );
        let mut legacy = request;
        legacy[..8].copy_from_slice(b"DCLTREQ1");
        assert_eq!(
            DirectExecutionRequestV3::decode(&legacy, 3),
            Err(DirectExecutionRequestErrorV3::InvalidMagic)
        );
    }

    #[test]
    fn native_signature_slices_are_exact_and_runtime_width() {
        assert_eq!(
            native_signature_slice_v3(DirectExecutionActionV3::InlineOrdinary, 0, 0),
            Ok(DirectNativeSignatureSliceV3 {
                message_offset: 64,
                message_bytes: 172,
                participant: 0,
            })
        );
        assert_eq!(
            native_signature_slice_v3(DirectExecutionActionV3::InlineOrdinary, 0, 1),
            Ok(DirectNativeSignatureSliceV3 {
                message_offset: 268,
                message_bytes: 172,
                participant: 1,
            })
        );
        assert_eq!(
            native_signature_slice_v3(DirectExecutionActionV3::RegisterBuy, 0, 0),
            Ok(DirectNativeSignatureSliceV3 {
                message_offset: 64,
                message_bytes: 172,
                participant: 0,
            })
        );
        assert_eq!(
            native_signature_slice_v3(DirectExecutionActionV3::CancelThrough, 0, 0),
            Ok(DirectNativeSignatureSliceV3 {
                message_offset: 64,
                message_bytes: 96,
                participant: 0,
            })
        );

        assert_eq!(
            native_signature_count_v3(DirectExecutionActionV3::SplitInline, u32::MAX),
            u32::MAX
        );
        assert_eq!(
            native_signature_slice_v3(DirectExecutionActionV3::MergeInline, 4, 3),
            Ok(DirectNativeSignatureSliceV3 {
                message_offset: 692,
                message_bytes: 172,
                participant: 3,
            })
        );
        assert!(native_signature_slice_v3(DirectExecutionActionV3::MergeInline, 4, 4).is_err());
        assert_eq!(
            native_signature_count_v3(DirectExecutionActionV3::ExpireRegistered, 16),
            0
        );
        assert!(
            native_signature_slice_v3(DirectExecutionActionV3::ExpireRegistered, 16, 0).is_err()
        );
    }
}
