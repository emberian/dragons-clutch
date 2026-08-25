//! Exact SDK-free description of the Direct SBF adapter boundary.

use crate::state::{
    CancelThroughV1, DIRECT_INTENT_BYTES_V2, DirectCancelV2, DirectIntentRecordV2, DirectIntentV2,
    Side,
};
use crate::{Error, Result, array, nonzero, one, put, zeros};

/// Native Ed25519 program bytes pinned by `solana-sdk-ids = 3.0.0`.
pub const ED25519_PROGRAM_ID_3_0: [u8; 32] = [
    3, 125, 70, 214, 124, 147, 251, 190, 18, 249, 66, 143, 131, 141, 64, 255, 5, 112, 116, 73, 39,
    244, 138, 100, 252, 202, 112, 68, 128, 0, 0, 0,
];
/// Canonical single-signature descriptor width.
pub const ED25519_DESCRIPTOR_BYTES: usize = 14;
/// Canonical public-key width.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
/// Canonical signature width.
pub const ED25519_SIGNATURE_BYTES: usize = 64;
/// Same-instruction descriptor sentinel.
pub const ED25519_CURRENT_INSTRUCTION_INDEX: u16 = u16::MAX;

const MAX_AUTHORIZATION_MESSAGE_BYTES: usize = DIRECT_INTENT_BYTES_V2;

/// Sealed evidence for one immediately preceding canonical native Ed25519
/// instruction. Cryptographic validity follows from successful execution of
/// that preceding native instruction; the SBF adapter must read its indices and
/// bytes from the instructions sysvar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ed25519AuthorizationV2 {
    signer: [u8; 32],
    message: [u8; MAX_AUTHORIZATION_MESSAGE_BYTES],
    message_len: u16,
}

impl Ed25519AuthorizationV2 {
    pub(crate) fn authorizes_registration(self, intent: DirectIntentV2) -> Result<()> {
        self.matches(*intent.maker(), &intent.signed_preimage())
    }

    pub(crate) fn authorizes_cancellation(self, record: DirectIntentRecordV2) -> Result<()> {
        self.matches(
            *record.intent().maker(),
            &DirectCancelV2::for_record(record).signed_preimage(),
        )
    }

    pub(crate) fn authorizes_cancel_through(self, message: CancelThroughV1) -> Result<()> {
        self.matches(*message.maker(), &message.signed_preimage())
    }

    pub(crate) fn authorizes_inline(self, intent: DirectIntentV2) -> Result<()> {
        self.matches(*intent.maker(), &intent.signed_preimage())
    }

    fn matches(self, signer: [u8; 32], expected_message: &[u8]) -> Result<()> {
        if self.signer != signer {
            return Err(Error::SignatureSignerMismatch);
        }
        if usize::from(self.message_len) != expected_message.len()
            || self.message.get(..expected_message.len()) != Some(expected_message)
        {
            return Err(Error::SignatureMessageMismatch);
        }
        Ok(())
    }
}

/// Authenticated instruction-sysvar projection for native signature inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ed25519InstructionViewV2<'a> {
    /// Program ID of immediately preceding instruction.
    pub program_id: [u8; 32],
    /// Immediately preceding native instruction data.
    pub ed25519_data: &'a [u8],
    /// Immediately preceding instruction index.
    pub preceding_index: u16,
    /// Current Direct instruction index.
    pub current_index: u16,
    /// Current Direct instruction data containing exact message preimages.
    pub current_data: &'a [u8],
}

/// One exact signer/message slice expected from a native descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ed25519ExpectationV2<'a> {
    /// Offset of message in current Direct instruction data.
    pub message_offset: u16,
    /// Exact expected Ed25519 public key.
    pub signer: [u8; 32],
    /// Exact expected message bytes.
    pub message: &'a [u8],
}

/// Inspect one canonical cross-instruction native Ed25519 signature.
pub fn inspect_preceding_ed25519_v2(
    view: Ed25519InstructionViewV2<'_>,
    expectation: Ed25519ExpectationV2<'_>,
) -> Result<Ed25519AuthorizationV2> {
    let values = inspect_preceding_ed25519_batch_v2(view, [expectation])?;
    values
        .first()
        .copied()
        .ok_or(Error::InvalidSignatureInstruction)
}

/// Inspect one exact immediately preceding native Ed25519 batch whose messages
/// are nonoverlapping slices of the following Direct instruction.
///
/// Descriptor indices must point pubkey/signature into the Ed25519 instruction
/// itself and each message into the current Direct instruction. Counts,
/// offsets, payload order, adjacency, program ID, signers, messages, and total
/// length are exact. A nonzero cryptographic forgery is rejected by the native
/// program before the following Direct instruction executes.
pub fn inspect_preceding_ed25519_batch_v2<const N: usize>(
    view: Ed25519InstructionViewV2<'_>,
    expectations: [Ed25519ExpectationV2<'_>; N],
) -> Result<[Ed25519AuthorizationV2; N]> {
    if view.program_id != ED25519_PROGRAM_ID_3_0 {
        return Err(Error::InvalidSignatureProgram);
    }
    if view.preceding_index.checked_add(1) != Some(view.current_index) {
        return Err(Error::InvalidSignatureInstructionOrder);
    }
    if N == 0
        || expectations
            .iter()
            .any(|value| value.message.len() > MAX_AUTHORIZATION_MESSAGE_BYTES)
    {
        return Err(Error::InvalidSignatureInstruction);
    }
    let descriptor_bytes = N
        .checked_mul(ED25519_DESCRIPTOR_BYTES)
        .ok_or(Error::InvalidSignatureInstruction)?;
    let payload_start = 2usize
        .checked_add(descriptor_bytes)
        .ok_or(Error::InvalidSignatureInstruction)?;
    let expected_len = payload_start
        .checked_add(
            N.checked_mul(ED25519_PUBLIC_KEY_BYTES + ED25519_SIGNATURE_BYTES)
                .ok_or(Error::InvalidSignatureInstruction)?,
        )
        .ok_or(Error::InvalidSignatureInstruction)?;
    if view.ed25519_data.len() != expected_len || read_u16(view.ed25519_data, 0)? != u16_from(N)? {
        return Err(Error::InvalidSignatureInstruction);
    }
    let empty = Ed25519AuthorizationV2 {
        signer: [0; 32],
        message: [0; MAX_AUTHORIZATION_MESSAGE_BYTES],
        message_len: 0,
    };
    let mut output = [empty; N];
    for (index, expectation) in expectations.iter().enumerate() {
        let descriptor = 2usize
            .checked_add(
                index
                    .checked_mul(ED25519_DESCRIPTOR_BYTES)
                    .ok_or(Error::InvalidSignatureInstruction)?,
            )
            .ok_or(Error::InvalidSignatureInstruction)?;
        let public_key_offset = payload_start
            .checked_add(
                index
                    .checked_mul(96)
                    .ok_or(Error::InvalidSignatureInstruction)?,
            )
            .ok_or(Error::InvalidSignatureInstruction)?;
        let signature_offset = public_key_offset
            .checked_add(ED25519_PUBLIC_KEY_BYTES)
            .ok_or(Error::InvalidSignatureInstruction)?;
        let message = expectation.message;
        let message_start = usize::from(expectation.message_offset);
        let message_end = message_start
            .checked_add(message.len())
            .ok_or(Error::InvalidSignatureInstruction)?;
        for prior in expectations.iter().take(index) {
            let prior_start = usize::from(prior.message_offset);
            let prior_end = prior_start
                .checked_add(prior.message.len())
                .ok_or(Error::InvalidSignatureInstruction)?;
            if message_start < prior_end && prior_start < message_end {
                return Err(Error::InvalidSignatureInstruction);
            }
        }
        if read_u16(view.ed25519_data, descriptor)? != u16_from(signature_offset)?
            || read_u16(view.ed25519_data, descriptor + 2)? != ED25519_CURRENT_INSTRUCTION_INDEX
            || read_u16(view.ed25519_data, descriptor + 4)? != u16_from(public_key_offset)?
            || read_u16(view.ed25519_data, descriptor + 6)? != ED25519_CURRENT_INSTRUCTION_INDEX
            || read_u16(view.ed25519_data, descriptor + 8)? != expectation.message_offset
            || read_u16(view.ed25519_data, descriptor + 10)? != u16_from(message.len())?
            || read_u16(view.ed25519_data, descriptor + 12)? != view.current_index
        {
            return Err(Error::InvalidSignatureInstruction);
        }
        let signer = array(view.ed25519_data, public_key_offset)?;
        if signer != expectation.signer {
            return Err(Error::SignatureSignerMismatch);
        }
        let signature_end = signature_offset
            .checked_add(ED25519_SIGNATURE_BYTES)
            .ok_or(Error::InvalidSignatureInstruction)?;
        if view
            .ed25519_data
            .get(signature_offset..signature_end)
            .ok_or(Error::InvalidSignatureInstruction)?
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(Error::ForgedSignature);
        }
        if view.current_data.get(message_start..message_end) != Some(message) {
            return Err(Error::SignatureMessageMismatch);
        }
        let mut copied = [0; MAX_AUTHORIZATION_MESSAGE_BYTES];
        copied
            .get_mut(..message.len())
            .ok_or(Error::InvalidSignatureInstruction)?
            .copy_from_slice(message);
        let slot = output
            .get_mut(index)
            .ok_or(Error::InvalidSignatureInstruction)?;
        *slot = Ed25519AuthorizationV2 {
            signer,
            message: copied,
            message_len: u16_from(message.len())?,
        };
    }
    Ok(output)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn u16_from(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidSignatureInstruction)
}

/// Canonical adapter instruction magic.
pub const DIRECT_ADAPTER_MAGIC_V2: [u8; 8] = *b"DCLTADP2";
/// Adapter instruction schema version.
pub const DIRECT_ADAPTER_SCHEMA_VERSION_V2: u16 = 2;
/// Common adapter instruction header width.
pub const DIRECT_ADAPTER_HEADER_BYTES_V2: usize = 16;
/// Exact register instruction data width.
pub const REGISTER_INSTRUCTION_BYTES_V2: usize = 248;
/// Exact cancel instruction data width.
pub const CANCEL_INSTRUCTION_BYTES_V2: usize = 112;
/// Exact O(1) cancel-through instruction width.
pub const CANCEL_THROUGH_INSTRUCTION_BYTES_V1: usize = 112;
/// Exact expire and root-lifecycle instruction data width.
pub const HEADER_ONLY_INSTRUCTION_BYTES_V2: usize = 16;
/// Exact ordinary settlement instruction data width.
pub const ORDINARY_INSTRUCTION_BYTES_V2: usize = 34;
/// Exact inline ordinary instruction data width.
pub const INLINE_ORDINARY_INSTRUCTION_BYTES_V2: usize = 498;
/// Fixed complementary data before N prices and N mode bytes.
pub const COMPLEMENTARY_INSTRUCTION_BASE_BYTES_V2: usize = 24;
const ACTION_OFFSET: usize = 10;
const PARTICIPANTS_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const BODY_OFFSET: usize = 16;

/// Hostile-decodable action with exact buy/sell custody geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdapterActionV2 {
    /// Register buy and create collateral escrow.
    RegisterBuy = 1,
    /// Register sell and reserve claims from Position.
    RegisterSell = 2,
    /// Cancel buy and refund escrow.
    CancelBuy = 3,
    /// Cancel sell and return claims.
    CancelSell = 4,
    /// Permissionlessly expire buy after inclusive deadline.
    ExpireBuy = 5,
    /// Permissionlessly expire sell after inclusive deadline.
    ExpireSell = 6,
    /// Match one persisted sell and one persisted buy.
    Ordinary = 7,
    /// Split one complete set across N persisted buys.
    Split = 8,
    /// Merge one complete set from N persisted sells.
    Merge = 9,
    /// Irreversibly close root registration inside Market retirement.
    CloseReplayRegistration = 10,
    /// Close zero-live replay root after authenticated Market retirement.
    CloseReplayRoot = 11,
    /// Immediate two-party FOK/IOC with two native signatures and no live records.
    InlineOrdinary = 12,
    /// Immediate N=2 complete-set split with native signatures and no live records.
    InlineSplit = 13,
    /// Immediate N=2 complete-set merge with native signatures and no live records.
    InlineMerge = 14,
    /// Maker-signed O(1) minimum-live-nonce advance.
    CancelThrough = 15,
    /// Permissionlessly unwind one invalidated registered Buy.
    CloseInvalidatedBuy = 16,
    /// Permissionlessly unwind one invalidated registered Sell.
    CloseInvalidatedSell = 17,
}

impl AdapterActionV2 {
    /// Decode one hostile action discriminator.
    pub fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::RegisterBuy),
            2 => Ok(Self::RegisterSell),
            3 => Ok(Self::CancelBuy),
            4 => Ok(Self::CancelSell),
            5 => Ok(Self::ExpireBuy),
            6 => Ok(Self::ExpireSell),
            7 => Ok(Self::Ordinary),
            8 => Ok(Self::Split),
            9 => Ok(Self::Merge),
            10 => Ok(Self::CloseReplayRegistration),
            11 => Ok(Self::CloseReplayRoot),
            12 => Ok(Self::InlineOrdinary),
            13 => Ok(Self::InlineSplit),
            14 => Ok(Self::InlineMerge),
            15 => Ok(Self::CancelThrough),
            16 => Ok(Self::CloseInvalidatedBuy),
            17 => Ok(Self::CloseInvalidatedSell),
            _ => Err(Error::UnknownAdapterAction),
        }
    }
    /// Return the canonical hostile action discriminator.
    pub const fn discriminator(self) -> u8 {
        match self {
            Self::RegisterBuy => 1,
            Self::RegisterSell => 2,
            Self::CancelBuy => 3,
            Self::CancelSell => 4,
            Self::ExpireBuy => 5,
            Self::ExpireSell => 6,
            Self::Ordinary => 7,
            Self::Split => 8,
            Self::Merge => 9,
            Self::CloseReplayRegistration => 10,
            Self::CloseReplayRoot => 11,
            Self::InlineOrdinary => 12,
            Self::InlineSplit => 13,
            Self::InlineMerge => 14,
            Self::CancelThrough => 15,
            Self::CloseInvalidatedBuy => 16,
            Self::CloseInvalidatedSell => 17,
        }
    }
}

/// Canonical Market-phase projection authenticated from the program-owned
/// Market account by the SBF adapter. This is not persisted Direct state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketPhaseV2 {
    /// Market has not opened.
    Founding,
    /// Market admits new trading and intent registration.
    Open,
    /// Market outcome is resolved; only live-intent unwind remains.
    Resolved,
    /// Market is retiring; unwind and replay-root closure remain.
    Retiring,
    /// Market is fully retired and admits no Direct action.
    Retired,
}

/// Enforce the exact Direct action/Market-phase matrix.
pub fn require_market_phase_v2(action: AdapterActionV2, phase: MarketPhaseV2) -> Result<()> {
    let accepted = match action {
        AdapterActionV2::RegisterBuy
        | AdapterActionV2::RegisterSell
        | AdapterActionV2::Ordinary
        | AdapterActionV2::Split
        | AdapterActionV2::Merge
        | AdapterActionV2::InlineOrdinary
        | AdapterActionV2::InlineSplit
        | AdapterActionV2::InlineMerge => phase == MarketPhaseV2::Open,
        AdapterActionV2::CancelBuy
        | AdapterActionV2::CancelSell
        | AdapterActionV2::ExpireBuy
        | AdapterActionV2::ExpireSell
        | AdapterActionV2::CancelThrough
        | AdapterActionV2::CloseInvalidatedBuy
        | AdapterActionV2::CloseInvalidatedSell => matches!(
            phase,
            MarketPhaseV2::Open | MarketPhaseV2::Resolved | MarketPhaseV2::Retiring
        ),
        AdapterActionV2::CloseReplayRegistration | AdapterActionV2::CloseReplayRoot => {
            phase == MarketPhaseV2::Retiring
        }
    };
    if accepted {
        Ok(())
    } else {
        Err(Error::MarketPhaseRefused)
    }
}

/// Decoded hostile common header used to route to one exact action codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterHeaderV2 {
    /// Routed action.
    pub action: AdapterActionV2,
    /// Exact participant count encoded by the action.
    pub participants: u8,
}

/// The two physical representations of the same signed Direct intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthorizationModeV2 {
    /// Native Ed25519 batch over intents embedded in this immediate instruction.
    Inline = 1,
    /// Program-owned live intent created from exact signed semantic intent.
    Persisted = 2,
}

impl AuthorizationModeV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::Inline => 1,
            Self::Persisted => 2,
        }
    }
}

/// Decoded ordinary adapter data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryAdapterInstructionV2 {
    /// Common fill.
    pub fill: u64,
    /// Exact execution price.
    pub execution_price: u64,
}

/// Decoded inline ordinary data including exact signed intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineOrdinaryAdapterInstructionV2 {
    /// Common immediate fill.
    pub fill: u64,
    /// Exact execution price.
    pub execution_price: u64,
    /// Signed seller intent referenced by native descriptor zero.
    pub seller_intent: DirectIntentV2,
    /// Signed buyer intent referenced by native descriptor one.
    pub buyer_intent: DirectIntentV2,
}

/// Decoded complementary adapter data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementaryAdapterInstructionV2<const N: usize> {
    /// Split or merge action.
    pub action: AdapterActionV2,
    /// Common fill.
    pub fill: u64,
    /// Prices in canonical outcome order.
    pub execution_prices: [u64; N],
}

/// Borrowed complementary data for one hostile runtime width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementaryAdapterInstructionViewV2<'a> {
    action: AdapterActionV2,
    fill: u64,
    outcome_count: u8,
    bytes: &'a [u8],
}

impl ComplementaryAdapterInstructionViewV2<'_> {
    /// Return the authenticated split or merge action.
    pub const fn action(self) -> AdapterActionV2 {
        self.action
    }

    /// Return the common positive fill field.
    pub const fn fill(self) -> u64 {
        self.fill
    }

    /// Return the exact authenticated outcome width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Decode one execution price in canonical outcome order.
    pub fn execution_price(self, index: usize) -> Result<u64> {
        if index >= usize::from(self.outcome_count) {
            return Err(Error::InvalidOutcome);
        }
        Ok(u64::from_le_bytes(array(self.bytes, price_offset(index)?)?))
    }
}

/// Encode registration data and choose exact custody action from signed side.
pub fn encode_register_instruction_v2(
    intent: DirectIntentV2,
) -> Result<[u8; REGISTER_INSTRUCTION_BYTES_V2]> {
    if intent.lifecycle() != crate::IntentLifecycleV2::Registered {
        return Err(Error::IntentLifecycleMismatch);
    }
    let mut output = [0; REGISTER_INSTRUCTION_BYTES_V2];
    let action = side_action(
        intent.side(),
        AdapterActionV2::RegisterBuy,
        AdapterActionV2::RegisterSell,
    );
    encode_header(&mut output, action, 1);
    put(&mut output, BODY_OFFSET, &intent.signed_preimage());
    Ok(output)
}

/// Decode registration data and require action/intent-side equivalence.
pub fn decode_register_instruction_v2(bytes: &[u8]) -> Result<DirectIntentV2> {
    if bytes.len() != REGISTER_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    let action = decode_common_header(bytes, 1)?;
    let intent = DirectIntentV2::decode_signed_preimage(
        bytes.get(BODY_OFFSET..).ok_or(Error::InvalidLength)?,
    )?;
    if intent.lifecycle() != crate::IntentLifecycleV2::Registered {
        return Err(Error::IntentLifecycleMismatch);
    }
    let expected = side_action(
        intent.side(),
        AdapterActionV2::RegisterBuy,
        AdapterActionV2::RegisterSell,
    );
    if action != expected {
        return Err(Error::UnknownAdapterAction);
    }
    Ok(intent)
}

/// Encode exact cancellation message with action matching persisted side.
pub fn encode_cancel_instruction_v2(
    side: Side,
    message: DirectCancelV2,
) -> [u8; CANCEL_INSTRUCTION_BYTES_V2] {
    let mut output = [0; CANCEL_INSTRUCTION_BYTES_V2];
    let action = side_action(
        side,
        AdapterActionV2::CancelBuy,
        AdapterActionV2::CancelSell,
    );
    encode_header(&mut output, action, 1);
    put(&mut output, BODY_OFFSET, &message.signed_preimage());
    output
}

/// Decode cancellation data and require exact expected persisted side.
pub fn decode_cancel_instruction_v2(bytes: &[u8], side: Side) -> Result<DirectCancelV2> {
    if bytes.len() != CANCEL_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    let expected = side_action(
        side,
        AdapterActionV2::CancelBuy,
        AdapterActionV2::CancelSell,
    );
    if decode_common_header(bytes, 1)? != expected {
        return Err(Error::UnknownAdapterAction);
    }
    DirectCancelV2::decode(bytes.get(BODY_OFFSET..).ok_or(Error::InvalidLength)?)
}

/// Encode the maker-signed O(1) replay-root invalidation threshold.
pub fn encode_cancel_through_instruction_v1(
    message: CancelThroughV1,
) -> [u8; CANCEL_THROUGH_INSTRUCTION_BYTES_V1] {
    let mut output = [0; CANCEL_THROUGH_INSTRUCTION_BYTES_V1];
    encode_header(&mut output, AdapterActionV2::CancelThrough, 1);
    put(&mut output, BODY_OFFSET, &message.signed_preimage());
    output
}

/// Decode the exact maker-signed replay-root invalidation threshold.
pub fn decode_cancel_through_instruction_v1(bytes: &[u8]) -> Result<CancelThroughV1> {
    if bytes.len() != CANCEL_THROUGH_INSTRUCTION_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(bytes, 1)? != AdapterActionV2::CancelThrough {
        return Err(Error::UnknownAdapterAction);
    }
    CancelThroughV1::decode(bytes.get(BODY_OFFSET..).ok_or(Error::InvalidLength)?)
}

/// Encode one permissionless invalidated-record unwind.
pub fn encode_close_invalidated_instruction_v1(
    side: Side,
) -> [u8; HEADER_ONLY_INSTRUCTION_BYTES_V2] {
    let action = side_action(
        side,
        AdapterActionV2::CloseInvalidatedBuy,
        AdapterActionV2::CloseInvalidatedSell,
    );
    encode_header_only(action)
}

/// Decode one exact permissionless invalidated-record unwind.
pub fn decode_close_invalidated_instruction_v1(bytes: &[u8], side: Side) -> Result<()> {
    let action = side_action(
        side,
        AdapterActionV2::CloseInvalidatedBuy,
        AdapterActionV2::CloseInvalidatedSell,
    );
    decode_header_only(bytes, action)
}

/// Encode exact post-expiry close action for persisted side.
pub fn encode_expire_instruction_v2(side: Side) -> [u8; HEADER_ONLY_INSTRUCTION_BYTES_V2] {
    let mut output = [0; HEADER_ONLY_INSTRUCTION_BYTES_V2];
    let action = side_action(
        side,
        AdapterActionV2::ExpireBuy,
        AdapterActionV2::ExpireSell,
    );
    encode_header(&mut output, action, 1);
    output
}

/// Decode exact post-expiry close action.
pub fn decode_expire_instruction_v2(bytes: &[u8], side: Side) -> Result<()> {
    if bytes.len() != HEADER_ONLY_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    let expected = side_action(
        side,
        AdapterActionV2::ExpireBuy,
        AdapterActionV2::ExpireSell,
    );
    if decode_common_header(bytes, 1)? != expected {
        return Err(Error::UnknownAdapterAction);
    }
    Ok(())
}

/// Encode the exact authenticated Market-retirement registration close.
pub fn encode_close_replay_registration_instruction_v2() -> [u8; HEADER_ONLY_INSTRUCTION_BYTES_V2] {
    encode_header_only(AdapterActionV2::CloseReplayRegistration)
}

/// Decode the exact authenticated Market-retirement registration close.
pub fn decode_close_replay_registration_instruction_v2(bytes: &[u8]) -> Result<()> {
    decode_header_only(bytes, AdapterActionV2::CloseReplayRegistration)
}

/// Encode the exact zero-live replay-root close.
pub fn encode_close_replay_root_instruction_v2() -> [u8; HEADER_ONLY_INSTRUCTION_BYTES_V2] {
    encode_header_only(AdapterActionV2::CloseReplayRoot)
}

/// Decode the exact zero-live replay-root close.
pub fn decode_close_replay_root_instruction_v2(bytes: &[u8]) -> Result<()> {
    decode_header_only(bytes, AdapterActionV2::CloseReplayRoot)
}

fn encode_header_only(action: AdapterActionV2) -> [u8; HEADER_ONLY_INSTRUCTION_BYTES_V2] {
    let mut output = [0; HEADER_ONLY_INSTRUCTION_BYTES_V2];
    encode_header(&mut output, action, 1);
    output
}

fn decode_header_only(bytes: &[u8], action: AdapterActionV2) -> Result<()> {
    if bytes.len() != HEADER_ONLY_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(bytes, 1)? != action {
        return Err(Error::UnknownAdapterAction);
    }
    Ok(())
}

/// Encode ordinary settlement data; both legs are necessarily persisted.
pub fn encode_ordinary_instruction_v2(
    fill: u64,
    execution_price: u64,
) -> [u8; ORDINARY_INSTRUCTION_BYTES_V2] {
    let mut output = [0; ORDINARY_INSTRUCTION_BYTES_V2];
    encode_header(&mut output, AdapterActionV2::Ordinary, 2);
    put(&mut output, BODY_OFFSET, &fill.to_le_bytes());
    put(&mut output, BODY_OFFSET + 8, &execution_price.to_le_bytes());
    output[BODY_OFFSET + 16] = AuthorizationModeV2::Persisted.byte();
    output[BODY_OFFSET + 17] = AuthorizationModeV2::Persisted.byte();
    output
}

/// Decode ordinary settlement data and refuse mixed/nonpersisted modes.
pub fn decode_ordinary_instruction_v2(bytes: &[u8]) -> Result<OrdinaryAdapterInstructionV2> {
    if bytes.len() != ORDINARY_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(bytes, 2)? != AdapterActionV2::Ordinary {
        return Err(Error::UnknownAdapterAction);
    }
    modes(
        bytes
            .get(BODY_OFFSET + 16..BODY_OFFSET + 18)
            .ok_or(Error::InvalidLength)?,
        AuthorizationModeV2::Persisted,
    )?;
    Ok(OrdinaryAdapterInstructionV2 {
        fill: u64::from_le_bytes(array(bytes, BODY_OFFSET)?),
        execution_price: u64::from_le_bytes(array(bytes, BODY_OFFSET + 8)?),
    })
}

/// Encode an inline ordinary instruction containing both exact signed intents.
pub fn encode_inline_ordinary_instruction_v2(
    fill: u64,
    execution_price: u64,
    seller_intent: DirectIntentV2,
    buyer_intent: DirectIntentV2,
) -> Result<[u8; INLINE_ORDINARY_INSTRUCTION_BYTES_V2]> {
    require_inline(seller_intent)?;
    require_inline(buyer_intent)?;
    let mut output = [0; INLINE_ORDINARY_INSTRUCTION_BYTES_V2];
    encode_header(&mut output, AdapterActionV2::InlineOrdinary, 2);
    put(&mut output, BODY_OFFSET, &fill.to_le_bytes());
    put(&mut output, BODY_OFFSET + 8, &execution_price.to_le_bytes());
    output[BODY_OFFSET + 16..BODY_OFFSET + 18].fill(AuthorizationModeV2::Inline.byte());
    put(
        &mut output,
        ORDINARY_INSTRUCTION_BYTES_V2,
        &seller_intent.signed_preimage(),
    );
    put(
        &mut output,
        ORDINARY_INSTRUCTION_BYTES_V2 + DIRECT_INTENT_BYTES_V2,
        &buyer_intent.signed_preimage(),
    );
    Ok(output)
}

/// Decode inline ordinary data and its nonoverlapping signed preimages.
pub fn decode_inline_ordinary_instruction_v2(
    bytes: &[u8],
) -> Result<InlineOrdinaryAdapterInstructionV2> {
    if bytes.len() != INLINE_ORDINARY_INSTRUCTION_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(bytes, 2)? != AdapterActionV2::InlineOrdinary {
        return Err(Error::UnknownAdapterAction);
    }
    modes(
        bytes
            .get(BODY_OFFSET + 16..BODY_OFFSET + 18)
            .ok_or(Error::InvalidLength)?,
        AuthorizationModeV2::Inline,
    )?;
    let seller_intent = DirectIntentV2::decode_signed_preimage(
        bytes
            .get(
                ORDINARY_INSTRUCTION_BYTES_V2
                    ..ORDINARY_INSTRUCTION_BYTES_V2 + DIRECT_INTENT_BYTES_V2,
            )
            .ok_or(Error::InvalidLength)?,
    )?;
    let buyer_intent = DirectIntentV2::decode_signed_preimage(
        bytes
            .get(ORDINARY_INSTRUCTION_BYTES_V2 + DIRECT_INTENT_BYTES_V2..)
            .ok_or(Error::InvalidLength)?,
    )?;
    require_inline(seller_intent)?;
    require_inline(buyer_intent)?;
    Ok(InlineOrdinaryAdapterInstructionV2 {
        fill: u64::from_le_bytes(array(bytes, BODY_OFFSET)?),
        execution_price: u64::from_le_bytes(array(bytes, BODY_OFFSET + 8)?),
        seller_intent,
        buyer_intent,
    })
}

/// Encode complementary split or merge data.
pub fn encode_complementary_instruction_v2<const N: usize>(
    action: AdapterActionV2,
    fill: u64,
    execution_prices: [u64; N],
    output: &mut [u8],
) -> Result<()> {
    if action != AdapterActionV2::Split && action != AdapterActionV2::Merge {
        return Err(Error::UnknownAdapterAction);
    }
    if output.len() != complementary_instruction_bytes_v2(N)? {
        return Err(Error::OutputLength);
    }
    output.fill(0);
    encode_header(
        output,
        action,
        u8::try_from(N).map_err(|_| Error::InvalidParticipantCount)?,
    );
    put(output, BODY_OFFSET, &fill.to_le_bytes());
    for (index, price) in execution_prices.iter().enumerate() {
        put(output, price_offset(index)?, &price.to_le_bytes());
    }
    let mode_offset = price_offset(N)?;
    output
        .get_mut(mode_offset..mode_offset + N)
        .ok_or(Error::OutputLength)?
        .fill(AuthorizationModeV2::Persisted.byte());
    Ok(())
}

/// Decode complementary split or merge data.
pub fn decode_complementary_instruction_v2<const N: usize>(
    bytes: &[u8],
    expected_action: AdapterActionV2,
) -> Result<ComplementaryAdapterInstructionV2<N>> {
    if expected_action != AdapterActionV2::Split && expected_action != AdapterActionV2::Merge {
        return Err(Error::UnknownAdapterAction);
    }
    if bytes.len() != complementary_instruction_bytes_v2(N)? {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(
        bytes,
        u8::try_from(N).map_err(|_| Error::InvalidParticipantCount)?,
    )? != expected_action
    {
        return Err(Error::UnknownAdapterAction);
    }
    let mut execution_prices = [0; N];
    for (index, price) in execution_prices.iter_mut().enumerate() {
        *price = u64::from_le_bytes(array(bytes, price_offset(index)?)?);
    }
    modes(
        bytes.get(price_offset(N)?..).ok_or(Error::InvalidLength)?,
        AuthorizationModeV2::Persisted,
    )?;
    Ok(ComplementaryAdapterInstructionV2 {
        action: expected_action,
        fill: u64::from_le_bytes(array(bytes, BODY_OFFSET)?),
        execution_prices,
    })
}

/// Decode complementary data at a hostile runtime width without an array copy.
pub fn decode_complementary_instruction_view_v2(
    bytes: &[u8],
    expected_action: AdapterActionV2,
    outcome_count: u8,
) -> Result<ComplementaryAdapterInstructionViewV2<'_>> {
    if expected_action != AdapterActionV2::Split && expected_action != AdapterActionV2::Merge {
        return Err(Error::UnknownAdapterAction);
    }
    let count = usize::from(outcome_count);
    if bytes.len() != complementary_instruction_bytes_v2(count)? {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(bytes, outcome_count)? != expected_action {
        return Err(Error::UnknownAdapterAction);
    }
    modes(
        bytes
            .get(price_offset(count)?..)
            .ok_or(Error::InvalidLength)?,
        AuthorizationModeV2::Persisted,
    )?;
    Ok(ComplementaryAdapterInstructionViewV2 {
        action: expected_action,
        fill: u64::from_le_bytes(array(bytes, BODY_OFFSET)?),
        outcome_count,
        bytes,
    })
}

/// Return exact inline complementary width `24 + 241N`; only N=2 fits.
pub fn inline_complementary_instruction_bytes_v2(participants: usize) -> Result<usize> {
    if participants != 2 {
        return Err(Error::InvalidInlineWidth);
    }
    COMPLEMENTARY_INSTRUCTION_BASE_BYTES_V2
        .checked_add(
            participants
                .checked_mul(241)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Encode an inline N=2 complementary action with exact signed intents.
pub fn encode_inline_complementary_instruction_v2<const N: usize>(
    action: AdapterActionV2,
    fill: u64,
    execution_prices: [u64; N],
    intents: [DirectIntentV2; N],
    output: &mut [u8],
) -> Result<()> {
    if action != AdapterActionV2::InlineSplit && action != AdapterActionV2::InlineMerge {
        return Err(Error::UnknownAdapterAction);
    }
    if output.len() != inline_complementary_instruction_bytes_v2(N)? {
        return Err(Error::OutputLength);
    }
    output.fill(0);
    encode_header(
        output,
        action,
        u8::try_from(N).map_err(|_| Error::InvalidInlineWidth)?,
    );
    put(output, BODY_OFFSET, &fill.to_le_bytes());
    for (index, price) in execution_prices.iter().enumerate() {
        put(output, price_offset(index)?, &price.to_le_bytes());
    }
    let mode_offset = price_offset(N)?;
    output
        .get_mut(mode_offset..mode_offset + N)
        .ok_or(Error::OutputLength)?
        .fill(AuthorizationModeV2::Inline.byte());
    let intents_offset = mode_offset
        .checked_add(N)
        .ok_or(Error::ArithmeticOverflow)?;
    for (index, intent) in intents.iter().enumerate() {
        require_inline(*intent)?;
        let offset = intents_offset
            .checked_add(
                index
                    .checked_mul(DIRECT_INTENT_BYTES_V2)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        put(output, offset, &intent.signed_preimage());
    }
    Ok(())
}

/// Decode an inline N=2 complementary action with exact signed intents.
pub fn decode_inline_complementary_instruction_v2<const N: usize>(
    bytes: &[u8],
    expected_action: AdapterActionV2,
) -> Result<(ComplementaryAdapterInstructionV2<N>, [DirectIntentV2; N])> {
    if expected_action != AdapterActionV2::InlineSplit
        && expected_action != AdapterActionV2::InlineMerge
    {
        return Err(Error::UnknownAdapterAction);
    }
    if bytes.len() != inline_complementary_instruction_bytes_v2(N)? {
        return Err(Error::InvalidLength);
    }
    if decode_common_header(
        bytes,
        u8::try_from(N).map_err(|_| Error::InvalidInlineWidth)?,
    )? != expected_action
    {
        return Err(Error::UnknownAdapterAction);
    }
    let mut prices = [0; N];
    for (index, price) in prices.iter_mut().enumerate() {
        *price = u64::from_le_bytes(array(bytes, price_offset(index)?)?);
    }
    let mode_offset = price_offset(N)?;
    modes(
        bytes
            .get(mode_offset..mode_offset + N)
            .ok_or(Error::InvalidLength)?,
        AuthorizationModeV2::Inline,
    )?;
    let intents_offset = mode_offset
        .checked_add(N)
        .ok_or(Error::ArithmeticOverflow)?;
    let placeholder = DirectIntentV2::decode_signed_preimage(
        bytes
            .get(intents_offset..intents_offset + DIRECT_INTENT_BYTES_V2)
            .ok_or(Error::InvalidLength)?,
    )?;
    let mut intents = [placeholder; N];
    for (index, intent) in intents.iter_mut().enumerate() {
        let offset = intents_offset
            .checked_add(
                index
                    .checked_mul(DIRECT_INTENT_BYTES_V2)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        *intent = DirectIntentV2::decode_signed_preimage(
            bytes
                .get(offset..offset + DIRECT_INTENT_BYTES_V2)
                .ok_or(Error::InvalidLength)?,
        )?;
        require_inline(*intent)?;
    }
    Ok((
        ComplementaryAdapterInstructionV2 {
            action: expected_action,
            fill: u64::from_le_bytes(array(bytes, BODY_OFFSET)?),
            execution_prices: prices,
        },
        intents,
    ))
}

fn require_inline(intent: DirectIntentV2) -> Result<()> {
    match intent.lifecycle() {
        crate::IntentLifecycleV2::InlineFillOrKill
        | crate::IntentLifecycleV2::InlineImmediateOrCancel => Ok(()),
        crate::IntentLifecycleV2::Registered => Err(Error::IntentLifecycleMismatch),
    }
}

/// Return exact complementary data width `24 + 9N`.
pub fn complementary_instruction_bytes_v2(participants: usize) -> Result<usize> {
    if !(2..=16).contains(&participants) {
        return Err(Error::InvalidParticipantCount);
    }
    COMPLEMENTARY_INSTRUCTION_BASE_BYTES_V2
        .checked_add(
            participants
                .checked_mul(9)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn price_offset(index: usize) -> Result<usize> {
    BODY_OFFSET
        .checked_add(8)
        .and_then(|offset| offset.checked_add(index.checked_mul(8)?))
        .ok_or(Error::ArithmeticOverflow)
}

fn side_action(side: Side, buy: AdapterActionV2, sell: AdapterActionV2) -> AdapterActionV2 {
    match side {
        Side::Buy => buy,
        Side::Sell => sell,
    }
}

fn encode_header(output: &mut [u8], action: AdapterActionV2, participants: u8) {
    put(output, 0, &DIRECT_ADAPTER_MAGIC_V2);
    put(output, 8, &DIRECT_ADAPTER_SCHEMA_VERSION_V2.to_le_bytes());
    if let Some(value) = output.get_mut(ACTION_OFFSET) {
        *value = action.discriminator();
    }
    if let Some(value) = output.get_mut(PARTICIPANTS_OFFSET) {
        *value = participants;
    }
}

fn decode_common_header(bytes: &[u8], participants: u8) -> Result<AdapterActionV2> {
    let header = decode_adapter_header_v2(bytes)?;
    if header.participants != participants {
        return Err(Error::InvalidParticipantCount);
    }
    Ok(header.action)
}

/// Decode and validate the common header before action-specific routing.
/// Action codecs still enforce their exact total instruction-data width.
pub fn decode_adapter_header_v2(bytes: &[u8]) -> Result<AdapterHeaderV2> {
    if bytes.len() < DIRECT_ADAPTER_HEADER_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    if array::<8>(bytes, 0)? != DIRECT_ADAPTER_MAGIC_V2 {
        return Err(Error::InvalidMagic);
    }
    if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_ADAPTER_SCHEMA_VERSION_V2 {
        return Err(Error::UnsupportedSchema);
    }
    zeros(bytes, HEADER_RESERVED_OFFSET, 4)?;
    Ok(AdapterHeaderV2 {
        action: AdapterActionV2::decode(one(bytes, ACTION_OFFSET)?)?,
        participants: one(bytes, PARTICIPANTS_OFFSET)?,
    })
}

fn modes(bytes: &[u8], expected: AuthorizationModeV2) -> Result<()> {
    let first = *bytes.first().ok_or(Error::InvalidParticipantCount)?;
    if bytes.iter().any(|mode| *mode != first) {
        return Err(Error::MixedAuthorizationModes);
    }
    if first != expected.byte() {
        return Err(Error::AuthorizationLifecycleMismatch);
    }
    Ok(())
}

/// SDK-free projection of the authenticated Token/Token-2022 fields used for
/// one maker-authorized Buy debit. The SBF adapter must decode these fields
/// from the signed source account; callers do not attest them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuyDebitAuthorityV2 {
    /// Token account being debited.
    pub token_account: [u8; 32],
    /// Token account mint.
    pub mint: [u8; 32],
    /// Token account owner/authority.
    pub owner: [u8; 32],
    /// Token delegate recorded by Token/Token-2022.
    pub delegate: [u8; 32],
    /// Remaining delegated allowance before this debit.
    pub delegated_amount: u64,
}

/// Require an exact worst-case allowance for registered Buy reservation.
///
/// Registration transfers the signed maximum reserve into record-owned
/// custody. Exact equality leaves no residual delegate allowance after that
/// one reservation and is intentionally stricter than inline execution.
pub fn validate_registered_buy_debit_authority_v2(
    authority: BuyDebitAuthorityV2,
    intent: DirectIntentV2,
    replay_root_account: [u8; 32],
    collateral_mint: [u8; 32],
    worst_case_reserve: u64,
) -> Result<()> {
    validate_buy_debit_binding_v2(authority, intent, replay_root_account, collateral_mint)?;
    if authority.delegated_amount != worst_case_reserve {
        return Err(Error::InvalidBuyDebitAuthority);
    }
    Ok(())
}

/// Require sufficient replay-root delegation for one inline exact debit.
///
/// FOK price improvement and IOC partial fill debit less than the maker's
/// signed worst case. The exact signed intent, native signature, and replay
/// nonce authorize the debit; an allowance at least as large as the actual
/// debit is sufficient. Any residual allowance remains inert unless another
/// exact maker-signed intent passes replay and should be revoked by a maker who
/// does not intend to authorize later inline orders.
pub fn validate_inline_buy_debit_authority_v2(
    authority: BuyDebitAuthorityV2,
    intent: DirectIntentV2,
    replay_root_account: [u8; 32],
    collateral_mint: [u8; 32],
    actual_debit: u64,
) -> Result<()> {
    validate_buy_debit_binding_v2(authority, intent, replay_root_account, collateral_mint)?;
    if authority.delegated_amount < actual_debit {
        return Err(Error::InvalidBuyDebitAuthority);
    }
    Ok(())
}

fn validate_buy_debit_binding_v2(
    authority: BuyDebitAuthorityV2,
    intent: DirectIntentV2,
    replay_root_account: [u8; 32],
    collateral_mint: [u8; 32],
) -> Result<()> {
    if intent.side() != Side::Buy
        || authority.token_account != *intent.collateral_account()
        || authority.mint != collateral_mint
        || authority.owner != *intent.maker()
        || authority.delegate != replay_root_account
    {
        return Err(Error::InvalidBuyDebitAuthority);
    }
    nonzero(&replay_root_account)?;
    nonzero(&collateral_mint)?;
    Ok(())
}

/// SDK-free projection of one registered Buy escrow's Token/Token-2022 fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EscrowAuthorityV2 {
    /// Escrow token account.
    pub token_account: [u8; 32],
    /// Escrow mint.
    pub mint: [u8; 32],
    /// Escrow token authority.
    pub authority: [u8; 32],
}

/// Require a registered Buy escrow to be controlled by its exact live-record
/// PDA. The live-record PDA signs every release and the final token close.
pub fn validate_registered_escrow_authority_v2(
    authority: EscrowAuthorityV2,
    record: DirectIntentRecordV2,
    record_account: [u8; 32],
    escrow_account: [u8; 32],
    collateral_mint: [u8; 32],
) -> Result<()> {
    if record.intent().side() != Side::Buy
        || authority.token_account != escrow_account
        || authority.mint != collateral_mint
        || authority.authority != record_account
    {
        return Err(Error::InvalidEscrowAuthority);
    }
    nonzero(&record_account)?;
    nonzero(&escrow_account)?;
    nonzero(&collateral_mint)?;
    Ok(())
}

/// Canonical physical account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRoleV2 {
    /// Signer paying System account creation; never inferred from Position owner.
    SystemPayer,
    /// Pre-existing 48-byte RentCredit PDA from the persisted refund authority.
    RentCredit,
    /// Canonical Market account.
    Market,
    /// Immutable Realm selecting collateral mint and token program.
    Realm,
    /// Immutable Market-selected fee policy.
    VenuePolicy,
    /// Canonical raw-record staging cursor, required to be finalized-absent.
    VenuePolicyStagingCursor,
    /// Immutable capability manifest selecting the fee-policy content digest.
    CapabilityManifest,
    /// Maker replay-root PDA.
    ReplayRoot,
    /// Live intent-record PDA.
    IntentRecord,
    /// Buy collateral escrow.
    IntentEscrow,
    /// Maker native Position.
    Position,
    /// Maker-bound collateral token account.
    MakerCollateral,
    /// Market collateral vault.
    MarketVault,
    /// Canonical custody metadata PDA; the Market PDA is Vault token authority.
    Custody,
    /// Fee recipient token account named by policy.
    FeeRecipient,
    /// Realm collateral mint.
    CollateralMint,
    /// Realm-selected token program.
    TokenProgram,
    /// System program.
    SystemProgram,
    /// Rent sysvar.
    RentSysvar,
    /// Instructions sysvar for signature inspection.
    InstructionsSysvar,
}

/// One hostile account meta projected into SDK-free key and privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterAccountMetaV2 {
    /// Exact public-key bytes.
    pub key: [u8; 32],
    /// Whether instruction marks account signer.
    pub is_signer: bool,
    /// Whether instruction marks account writable.
    pub is_writable: bool,
}

/// Return exact instruction account count for action.
pub fn account_count_v2(action: AdapterActionV2, participants: usize) -> Result<usize> {
    match action {
        AdapterActionV2::RegisterBuy => one_participant(participants, 17),
        AdapterActionV2::RegisterSell => one_participant(participants, 12),
        AdapterActionV2::CancelBuy => one_participant(participants, 13),
        AdapterActionV2::CancelSell => one_participant(participants, 8),
        AdapterActionV2::ExpireBuy => one_participant(participants, 12),
        AdapterActionV2::ExpireSell => one_participant(participants, 7),
        AdapterActionV2::Ordinary => exact_participants(participants, 2, 21),
        AdapterActionV2::Split => settlement_count(participants, 12, 6),
        AdapterActionV2::Merge => settlement_count(participants, 12, 5),
        AdapterActionV2::CloseReplayRegistration => one_participant(participants, 2),
        AdapterActionV2::CloseReplayRoot => one_participant(participants, 5),
        AdapterActionV2::CancelThrough => one_participant(participants, 3),
        AdapterActionV2::CloseInvalidatedBuy => one_participant(participants, 12),
        AdapterActionV2::CloseInvalidatedSell => one_participant(participants, 7),
        AdapterActionV2::InlineOrdinary => exact_participants(participants, 2, 19),
        AdapterActionV2::InlineSplit | AdapterActionV2::InlineMerge => {
            if participants != 2 {
                return Err(Error::InvalidInlineWidth);
            }
            settlement_count(participants, 15, 3)
        }
    }
}

fn one_participant(participants: usize, accounts: usize) -> Result<usize> {
    if participants == 1 {
        Ok(accounts)
    } else {
        Err(Error::InvalidParticipantCount)
    }
}

fn exact_participants(participants: usize, expected: usize, accounts: usize) -> Result<usize> {
    if participants == expected {
        Ok(accounts)
    } else {
        Err(Error::InvalidParticipantCount)
    }
}

fn settlement_count(participants: usize, base: usize, per_participant: usize) -> Result<usize> {
    if !(2..=16).contains(&participants) {
        return Err(Error::InvalidParticipantCount);
    }
    base.checked_add(
        participants
            .checked_mul(per_participant)
            .ok_or(Error::ArithmeticOverflow)?,
    )
    .ok_or(Error::ArithmeticOverflow)
}

/// Return exact account role for action and index.
pub fn account_role_v2(
    action: AdapterActionV2,
    participants: usize,
    index: usize,
) -> Result<AccountRoleV2> {
    if index >= account_count_v2(action, participants)? {
        return Err(Error::InvalidAccountFrame);
    }
    match action {
        AdapterActionV2::RegisterBuy => role_at(
            index,
            &[
                AccountRoleV2::SystemPayer,
                AccountRoleV2::RentCredit,
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
        ),
        AdapterActionV2::RegisterSell => role_at(
            index,
            &[
                AccountRoleV2::SystemPayer,
                AccountRoleV2::RentCredit,
                AccountRoleV2::Market,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
        ),
        AdapterActionV2::CancelBuy => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
        ),
        AdapterActionV2::CancelSell => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::RentCredit,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
        ),
        AdapterActionV2::ExpireBuy => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
        ),
        AdapterActionV2::ExpireSell => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::RentCredit,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
        ),
        AdapterActionV2::Ordinary => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::FeeRecipient,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
            ],
        ),
        AdapterActionV2::Split => repeating_role(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::MarketVault,
                AccountRoleV2::Custody,
                AccountRoleV2::FeeRecipient,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
            &[
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
            ],
        ),
        AdapterActionV2::Merge => repeating_role(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::MarketVault,
                AccountRoleV2::Custody,
                AccountRoleV2::FeeRecipient,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
            &[
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
            ],
        ),
        AdapterActionV2::CloseReplayRegistration => {
            role_at(index, &[AccountRoleV2::Market, AccountRoleV2::ReplayRoot])
        }
        AdapterActionV2::CloseReplayRoot => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::RentCredit,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
        ),
        AdapterActionV2::CancelThrough => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::InstructionsSysvar,
            ],
        ),
        AdapterActionV2::CloseInvalidatedBuy => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::IntentEscrow,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
                AccountRoleV2::RentCredit,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
        ),
        AdapterActionV2::CloseInvalidatedSell => role_at(
            index,
            &[
                AccountRoleV2::Market,
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::IntentRecord,
                AccountRoleV2::Position,
                AccountRoleV2::RentCredit,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
            ],
        ),
        AdapterActionV2::InlineOrdinary => repeating_role(
            index,
            &[
                AccountRoleV2::SystemPayer,
                AccountRoleV2::RentCredit,
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::FeeRecipient,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
            &[
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
            ],
        ),
        AdapterActionV2::InlineSplit | AdapterActionV2::InlineMerge => repeating_role(
            index,
            &[
                AccountRoleV2::SystemPayer,
                AccountRoleV2::RentCredit,
                AccountRoleV2::Market,
                AccountRoleV2::Realm,
                AccountRoleV2::VenuePolicy,
                AccountRoleV2::VenuePolicyStagingCursor,
                AccountRoleV2::CapabilityManifest,
                AccountRoleV2::MarketVault,
                AccountRoleV2::Custody,
                AccountRoleV2::FeeRecipient,
                AccountRoleV2::CollateralMint,
                AccountRoleV2::TokenProgram,
                AccountRoleV2::SystemProgram,
                AccountRoleV2::RentSysvar,
                AccountRoleV2::InstructionsSysvar,
            ],
            &[
                AccountRoleV2::ReplayRoot,
                AccountRoleV2::Position,
                AccountRoleV2::MakerCollateral,
            ],
        ),
    }
}

fn role_at(index: usize, roles: &[AccountRoleV2]) -> Result<AccountRoleV2> {
    roles.get(index).copied().ok_or(Error::InvalidAccountFrame)
}

fn repeating_role(
    index: usize,
    base: &[AccountRoleV2],
    participant: &[AccountRoleV2],
) -> Result<AccountRoleV2> {
    if index < base.len() {
        return role_at(index, base);
    }
    participant
        .get((index - base.len()) % participant.len())
        .copied()
        .ok_or(Error::InvalidAccountFrame)
}

/// Validate exact count, privileges, required nonzero keys, and aliases.
pub fn validate_account_frame_v2(
    action: AdapterActionV2,
    participants: usize,
    accounts: &[AdapterAccountMetaV2],
) -> Result<()> {
    if accounts.len() != account_count_v2(action, participants)? {
        return Err(Error::InvalidAccountFrame);
    }
    for (index, account) in accounts.iter().enumerate() {
        let role = account_role_v2(action, participants, index)?;
        if role != AccountRoleV2::SystemProgram {
            nonzero(&account.key)?;
        }
        for (prior_index, prior) in accounts
            .get(..index)
            .ok_or(Error::InvalidAccountFrame)?
            .iter()
            .enumerate()
        {
            if prior.key == account.key {
                let prior_role = account_role_v2(action, participants, prior_index)?;
                if role != AccountRoleV2::RentCredit || prior_role != AccountRoleV2::RentCredit {
                    return Err(Error::Alias);
                }
            }
        }
        let expected = expected_privileges(action, index, role);
        if (account.is_signer, account.is_writable) != expected {
            return Err(Error::InvalidAccountFrame);
        }
    }
    Ok(())
}

const fn expected_privileges(
    action: AdapterActionV2,
    index: usize,
    role: AccountRoleV2,
) -> (bool, bool) {
    match role {
        AccountRoleV2::SystemPayer => (true, true),
        AccountRoleV2::RentCredit
            if matches!(
                action,
                AdapterActionV2::RegisterBuy
                    | AdapterActionV2::RegisterSell
                    | AdapterActionV2::InlineOrdinary
                    | AdapterActionV2::InlineSplit
                    | AdapterActionV2::InlineMerge
            ) =>
        {
            (false, false)
        }
        AccountRoleV2::Market
            if matches!(
                action,
                AdapterActionV2::RegisterBuy
                    | AdapterActionV2::RegisterSell
                    | AdapterActionV2::Split
                    | AdapterActionV2::Merge
                    | AdapterActionV2::InlineOrdinary
                    | AdapterActionV2::InlineSplit
                    | AdapterActionV2::InlineMerge
                    | AdapterActionV2::CloseReplayRegistration
                    | AdapterActionV2::CloseReplayRoot
            ) =>
        {
            (false, true)
        }
        AccountRoleV2::Position
            if matches!(
                action,
                AdapterActionV2::RegisterBuy
                    | AdapterActionV2::CancelBuy
                    | AdapterActionV2::ExpireBuy
                    | AdapterActionV2::CloseInvalidatedBuy
                    | AdapterActionV2::Merge
            ) || (matches!(action, AdapterActionV2::Ordinary) && index == 12) =>
        {
            (false, false)
        }
        AccountRoleV2::ReplayRoot
        | AccountRoleV2::IntentRecord
        | AccountRoleV2::IntentEscrow
        | AccountRoleV2::Position
        | AccountRoleV2::MakerCollateral
        | AccountRoleV2::MarketVault
        | AccountRoleV2::FeeRecipient
        | AccountRoleV2::RentCredit => (false, true),
        AccountRoleV2::Market
        | AccountRoleV2::Realm
        | AccountRoleV2::VenuePolicy
        | AccountRoleV2::VenuePolicyStagingCursor
        | AccountRoleV2::CapabilityManifest
        | AccountRoleV2::Custody
        | AccountRoleV2::CollateralMint
        | AccountRoleV2::TokenProgram
        | AccountRoleV2::SystemProgram
        | AccountRoleV2::RentSysvar
        | AccountRoleV2::InstructionsSysvar => (false, false),
    }
}

/// Solana packet bytes pinned by `solana-packet = 3.0.0`.
pub const SOLANA_PACKET_DATA_SIZE_3_0: usize = 1_232;
/// Account-lock ceiling pinned by transitive `solana-transaction = 3.1.0`.
pub const SOLANA_MAX_TX_ACCOUNT_LOCKS_3_1: usize = 128;
/// Address lookup tables in measured v0 profile.
pub const MEASURED_LOOKUP_TABLES_V2: usize = 1;
/// Transaction signatures in measured permissionless profile.
pub const MEASURED_TRANSACTION_SIGNATURES_V2: usize = 1;
/// Measured ordinary serialized v0 bytes.
pub const MEASURED_ORDINARY_V0_BYTES_V2: usize = 312;
/// Measured buy-registration serialized v0 bytes with cross-instruction message.
pub const MEASURED_BUY_REGISTRATION_V0_BYTES_V2: usize = 634;
/// Measured sell-registration serialized v0 bytes.
pub const MEASURED_SELL_REGISTRATION_V0_BYTES_V2: usize = 624;
/// Measured Buy cancellation serialized v0 bytes.
pub const MEASURED_CANCEL_BUY_V0_BYTES_V2: usize = 521;
/// Measured Sell cancellation serialized v0 bytes.
pub const MEASURED_CANCEL_SELL_V0_BYTES_V2: usize = 511;
/// Measured Buy expiry serialized v0 bytes.
pub const MEASURED_EXPIRE_BUY_V0_BYTES_V2: usize = 276;
/// Measured Sell expiry serialized v0 bytes.
pub const MEASURED_EXPIRE_SELL_V0_BYTES_V2: usize = 266;
/// Measured registration-close serialized v0 bytes.
pub const MEASURED_CLOSE_REGISTRATION_V0_BYTES_V2: usize = 256;
/// Measured replay-root close serialized v0 bytes.
pub const MEASURED_CLOSE_ROOT_V0_BYTES_V2: usize = 262;
/// Measured O(1) cancel-through serialized v0 bytes.
pub const MEASURED_CANCEL_THROUGH_V0_BYTES_V1: usize = 501;
/// Measured immediate ordinary serialized v0 bytes.
pub const MEASURED_INLINE_ORDINARY_V0_BYTES_V2: usize = 999;
/// Measured immediate N=2 complementary serialized v0 bytes.
pub const MEASURED_INLINE_COMPLEMENTARY_N2_V0_BYTES_V2: usize = 1_011;
/// Measured inline complementary reference bytes for N=2..16. Admission still
/// refuses every entry after N=2 because it exceeds the 1,232-byte packet.
pub const MEASURED_INLINE_COMPLEMENTARY_REFERENCE_V0_BYTES_V2: [usize; 15] = [
    1_011, 1_368, 1_725, 2_082, 2_439, 2_796, 3_153, 3_510, 3_867, 4_224, 4_581, 4_938, 5_295,
    5_652, 6_009,
];
/// Measured split serialized v0 bytes for N=2..16.
pub const MEASURED_SPLIT_V0_BYTES_V2: [usize; 15] = [
    326, 347, 368, 389, 410, 431, 452, 473, 494, 515, 537, 558, 579, 600, 621,
];
/// Measured merge serialized v0 bytes for N=2..16.
pub const MEASURED_MERGE_V0_BYTES_V2: [usize; 15] = [
    322, 341, 360, 379, 398, 417, 436, 455, 474, 493, 513, 532, 551, 570, 589,
];

/// One pinned measured transaction shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasuredEnvelopeV2 {
    /// Action.
    pub action: AdapterActionV2,
    /// Participant count.
    pub participants: usize,
    /// Direct instruction account metas.
    pub instruction_accounts: usize,
    /// Direct instruction data bytes.
    pub instruction_data_bytes: usize,
    /// Total unique account locks including Direct program.
    pub total_account_locks: usize,
    /// Serialized v0 bytes in pinned one-ALT profile.
    pub serialized_transaction_bytes: usize,
}

/// Return the exact Direct instruction-data width for one routed action.
pub fn instruction_data_bytes_v2(action: AdapterActionV2, participants: usize) -> Result<usize> {
    account_count_v2(action, participants)?;
    let bytes = match action {
        AdapterActionV2::RegisterBuy | AdapterActionV2::RegisterSell => {
            REGISTER_INSTRUCTION_BYTES_V2
        }
        AdapterActionV2::CancelBuy | AdapterActionV2::CancelSell => CANCEL_INSTRUCTION_BYTES_V2,
        AdapterActionV2::CancelThrough => CANCEL_THROUGH_INSTRUCTION_BYTES_V1,
        AdapterActionV2::ExpireBuy
        | AdapterActionV2::ExpireSell
        | AdapterActionV2::CloseReplayRegistration
        | AdapterActionV2::CloseReplayRoot
        | AdapterActionV2::CloseInvalidatedBuy
        | AdapterActionV2::CloseInvalidatedSell => HEADER_ONLY_INSTRUCTION_BYTES_V2,
        AdapterActionV2::Ordinary => ORDINARY_INSTRUCTION_BYTES_V2,
        AdapterActionV2::Split | AdapterActionV2::Merge => {
            complementary_instruction_bytes_v2(participants)?
        }
        AdapterActionV2::InlineOrdinary => INLINE_ORDINARY_INSTRUCTION_BYTES_V2,
        AdapterActionV2::InlineSplit | AdapterActionV2::InlineMerge => {
            inline_complementary_instruction_bytes_v2(participants)?
        }
    };
    Ok(bytes)
}

/// Return the exact pinned one-ALT v0 envelope for any Direct action.
pub fn measured_action_envelope_v2(
    action: AdapterActionV2,
    participants: usize,
) -> Result<MeasuredEnvelopeV2> {
    let accounts = account_count_v2(action, participants)?;
    let data = instruction_data_bytes_v2(action, participants)?;
    let measured = match action {
        AdapterActionV2::RegisterBuy => MEASURED_BUY_REGISTRATION_V0_BYTES_V2,
        AdapterActionV2::RegisterSell => MEASURED_SELL_REGISTRATION_V0_BYTES_V2,
        AdapterActionV2::CancelBuy => MEASURED_CANCEL_BUY_V0_BYTES_V2,
        AdapterActionV2::CancelSell => MEASURED_CANCEL_SELL_V0_BYTES_V2,
        AdapterActionV2::ExpireBuy => MEASURED_EXPIRE_BUY_V0_BYTES_V2,
        AdapterActionV2::ExpireSell => MEASURED_EXPIRE_SELL_V0_BYTES_V2,
        AdapterActionV2::CloseReplayRegistration => MEASURED_CLOSE_REGISTRATION_V0_BYTES_V2,
        AdapterActionV2::CloseReplayRoot => MEASURED_CLOSE_ROOT_V0_BYTES_V2,
        AdapterActionV2::CancelThrough => MEASURED_CANCEL_THROUGH_V0_BYTES_V1,
        AdapterActionV2::CloseInvalidatedBuy => MEASURED_EXPIRE_BUY_V0_BYTES_V2,
        AdapterActionV2::CloseInvalidatedSell => MEASURED_EXPIRE_SELL_V0_BYTES_V2,
        AdapterActionV2::Ordinary => MEASURED_ORDINARY_V0_BYTES_V2,
        AdapterActionV2::Split => measured_at(&MEASURED_SPLIT_V0_BYTES_V2, participants)?,
        AdapterActionV2::Merge => measured_at(&MEASURED_MERGE_V0_BYTES_V2, participants)?,
        AdapterActionV2::InlineOrdinary => MEASURED_INLINE_ORDINARY_V0_BYTES_V2,
        AdapterActionV2::InlineSplit | AdapterActionV2::InlineMerge => {
            MEASURED_INLINE_COMPLEMENTARY_N2_V0_BYTES_V2
        }
    };
    let extra_locks = match action {
        AdapterActionV2::RegisterBuy
        | AdapterActionV2::RegisterSell
        | AdapterActionV2::InlineOrdinary
        | AdapterActionV2::InlineSplit
        | AdapterActionV2::InlineMerge => 2,
        AdapterActionV2::CancelBuy
        | AdapterActionV2::CancelSell
        | AdapterActionV2::CancelThrough => 3,
        AdapterActionV2::ExpireBuy
        | AdapterActionV2::ExpireSell
        | AdapterActionV2::Ordinary
        | AdapterActionV2::Split
        | AdapterActionV2::Merge
        | AdapterActionV2::CloseReplayRegistration
        | AdapterActionV2::CloseReplayRoot
        | AdapterActionV2::CloseInvalidatedBuy
        | AdapterActionV2::CloseInvalidatedSell => 2,
    };
    Ok(MeasuredEnvelopeV2 {
        action,
        participants,
        instruction_accounts: accounts,
        instruction_data_bytes: data,
        total_account_locks: accounts
            .checked_add(extra_locks)
            .ok_or(Error::ArithmeticOverflow)?,
        serialized_transaction_bytes: measured,
    })
}

/// Return exact measured persisted/immediate settlement profile.
pub fn measured_settlement_envelope_v2(
    action: AdapterActionV2,
    participants: usize,
) -> Result<MeasuredEnvelopeV2> {
    if !matches!(
        action,
        AdapterActionV2::Ordinary
            | AdapterActionV2::Split
            | AdapterActionV2::Merge
            | AdapterActionV2::InlineOrdinary
            | AdapterActionV2::InlineSplit
            | AdapterActionV2::InlineMerge
    ) {
        return Err(Error::UnknownAdapterAction);
    }
    measured_action_envelope_v2(action, participants)
}

fn measured_at(table: &[usize; 15], participants: usize) -> Result<usize> {
    let index = participants
        .checked_sub(2)
        .ok_or(Error::InvalidParticipantCount)?;
    table
        .get(index)
        .copied()
        .ok_or(Error::InvalidParticipantCount)
}

/// Return the measured inline complementary reference size for N=2..16,
/// including refused oversize profiles.
pub fn measured_inline_complementary_reference_v2(participants: usize) -> Result<usize> {
    measured_at(
        &MEASURED_INLINE_COMPLEMENTARY_REFERENCE_V0_BYTES_V2,
        participants,
    )
}

/// Transaction facts the SBF adapter computes from live v0 message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketAdmissionV2 {
    /// Serialized transaction bytes.
    pub serialized_transaction_bytes: usize,
    /// Direct instruction account count.
    pub instruction_accounts: usize,
    /// Direct instruction data width.
    pub instruction_data_bytes: usize,
    /// Total locks after v0 lookup resolution.
    pub total_account_locks: usize,
    /// Transaction signature count.
    pub transaction_signatures: usize,
    /// Address lookup table count.
    pub address_lookup_tables: usize,
}

/// Refuse transaction outside exact persisted profile or physical ceilings.
pub fn admit_settlement_packet_v2(
    action: AdapterActionV2,
    participants: usize,
    packet: PacketAdmissionV2,
) -> Result<()> {
    let expected = measured_settlement_envelope_v2(action, participants)?;
    if packet.serialized_transaction_bytes > SOLANA_PACKET_DATA_SIZE_3_0
        || packet.total_account_locks > SOLANA_MAX_TX_ACCOUNT_LOCKS_3_1
    {
        return Err(Error::PacketEnvelopeExceeded);
    }
    if packet.instruction_accounts != expected.instruction_accounts
        || packet.instruction_data_bytes != expected.instruction_data_bytes
        || packet.transaction_signatures != MEASURED_TRANSACTION_SIGNATURES_V2
        || packet.address_lookup_tables != MEASURED_LOOKUP_TABLES_V2
    {
        return Err(Error::PacketProfileMismatch);
    }
    Ok(())
}

/// Minimum native Ed25519 instruction bytes for N independent signatures over
/// one perfectly shared message, before transaction framing or settlement.
pub fn stateless_shared_message_ed25519_minimum_v2(
    participants: usize,
    shared_message_bytes: usize,
) -> Result<usize> {
    2usize
        .checked_add(
            participants
                .checked_mul(
                    ED25519_DESCRIPTOR_BYTES
                        .checked_add(ED25519_PUBLIC_KEY_BYTES)
                        .and_then(|value| value.checked_add(ED25519_SIGNATURE_BYTES))
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .and_then(|value| value.checked_add(shared_message_bytes))
        .ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
pub(crate) fn canonical_ed25519_test_instruction<const N: usize>(
    signers: [[u8; 32]; N],
    message_offsets: [u16; N],
    message_lengths: [u16; N],
    current_instruction_index: u16,
) -> [u8; 1_762] {
    let mut output = [0; 1_762];
    put(&mut output, 0, &u16::try_from(N).unwrap_or(0).to_le_bytes());
    let payload_start = 2 + N * ED25519_DESCRIPTOR_BYTES;
    for (index, ((signer, message_offset), message_length)) in signers
        .iter()
        .zip(message_offsets.iter())
        .zip(message_lengths.iter())
        .enumerate()
    {
        let descriptor = 2 + index * ED25519_DESCRIPTOR_BYTES;
        let public_key_offset = payload_start + index * 96;
        let signature_offset = public_key_offset + ED25519_PUBLIC_KEY_BYTES;
        put(
            &mut output,
            descriptor,
            &u16_from(signature_offset).unwrap_or(0).to_le_bytes(),
        );
        put(
            &mut output,
            descriptor + 2,
            &ED25519_CURRENT_INSTRUCTION_INDEX.to_le_bytes(),
        );
        put(
            &mut output,
            descriptor + 4,
            &u16_from(public_key_offset).unwrap_or(0).to_le_bytes(),
        );
        put(
            &mut output,
            descriptor + 6,
            &ED25519_CURRENT_INSTRUCTION_INDEX.to_le_bytes(),
        );
        put(&mut output, descriptor + 8, &message_offset.to_le_bytes());
        put(&mut output, descriptor + 10, &message_length.to_le_bytes());
        put(
            &mut output,
            descriptor + 12,
            &current_instruction_index.to_le_bytes(),
        );
        put(&mut output, public_key_offset, signer);
        if let Some(signature) =
            output.get_mut(signature_offset..signature_offset + ED25519_SIGNATURE_BYTES)
        {
            signature.fill(7);
        }
    }
    output
}

#[cfg(test)]
pub(crate) const fn canonical_ed25519_test_instruction_len(signatures: usize) -> usize {
    2 + signatures * (ED25519_DESCRIPTOR_BYTES + ED25519_PUBLIC_KEY_BYTES + ED25519_SIGNATURE_BYTES)
}
