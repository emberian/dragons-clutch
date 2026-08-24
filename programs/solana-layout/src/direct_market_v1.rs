// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical frames for the disabled current Direct `0xb1..=0xb4/v1` family.
//!
//! This module owns only tag/version/bump/reserved framing. The semantic bodies
//! are interpreted exclusively by `clutch-direct-market-runtime`.

use crate::{registry, CodecError, Result};
use clutch_batch::direct_pair_v1::DirectEconomicCandidateV1;
use clutch_batch::{PartialPolicy, Side};

/// Exact action-2 owner-blind order payload bytes.
pub const DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1: usize = 80;
/// Exact action-5 compact candidate payload bytes.
pub const DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1: usize = 24;


/// Strict action-2 coordinates. Owner, Position, Replay, root, rent principal,
/// hostile prefund, Reservation PDA, and the current action ordinal all come
/// exclusively from authenticated accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAdmitOrderPayloadV1 {
    /// Caller-chosen owner-blind order identity, persisted exactly once by b4.
    pub order_id: [u8; 32],
    /// Buy or sell.
    pub side: Side,
    /// Native Egg outcome.
    pub outcome: u8,
    /// Partial-fill policy.
    pub partial_policy: PartialPolicy,
    /// Maximum Egg units.
    pub quantity: u64,
    /// Smallest admitted nonzero fill.
    pub minimum_fill: u64,
    /// Last eligible Direct generation.
    pub expiry_epoch: u64,
    /// Exact price units per Egg.
    pub limit_price_units_per_egg: u128,
}

impl DirectAdmitOrderPayloadV1 {
    /// Encode the exact canonical owner-blind order payload.
    pub fn encode_into(
        &self,
        output: &mut [u8; DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1],
    ) -> Result<()> {
        if self.order_id == [0; 32] {
            return Err(CodecError::ZeroValue);
        }
        output.fill(0);
        output[..32].copy_from_slice(&self.order_id);
        output[32] = match self.side {
            Side::Buy => 1,
            Side::Sell => 2,
        };
        output[33] = self.outcome;
        output[34] = match self.partial_policy {
            PartialPolicy::Allow => 1,
            PartialPolicy::AllOrNone => 2,
        };
        output[40..48].copy_from_slice(&self.quantity.to_le_bytes());
        output[48..56].copy_from_slice(&self.minimum_fill.to_le_bytes());
        output[56..64].copy_from_slice(&self.expiry_epoch.to_le_bytes());
        output[64..80].copy_from_slice(&self.limit_price_units_per_egg.to_le_bytes());
        Ok(())
    }

    /// Decode the exact canonical 80-byte order payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_len(input, DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1)?;
        let side = match input[32] {
            1 => Side::Buy,
            2 => Side::Sell,
            _ => return Err(CodecError::InvalidEnum),
        };
        let partial_policy = match input[34] {
            1 => PartialPolicy::Allow,
            2 => PartialPolicy::AllOrNone,
            _ => return Err(CodecError::InvalidEnum),
        };
        if input[35..40].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let mut order_id = [0u8; 32];
        order_id.copy_from_slice(&input[..32]);
        if order_id == [0; 32] {
            return Err(CodecError::ZeroValue);
        }
        Ok(Self {
            order_id,
            side,
            outcome: input[33],
            partial_policy,
            quantity: u64_at(input, 40),
            minimum_fill: u64_at(input, 48),
            expiry_epoch: u64_at(input, 56),
            limit_price_units_per_egg: u128_at(input, 64),
        })
    }
}

/// Strict action-5 compact two-row RelationV2 candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSubmitCandidatePayloadV1 {
    /// Exact compact candidate coordinates.
    pub candidate: DirectEconomicCandidateV1,
}

impl DirectSubmitCandidatePayloadV1 {
    /// Encode the exact two-row candidate with canonical zero padding.
    pub fn encode_into(
        &self,
        output: &mut [u8; DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1],
    ) {
        output.fill(0);
        output[0..8].copy_from_slice(&self.candidate.fills[0].to_le_bytes());
        output[8..16].copy_from_slice(&self.candidate.fills[1].to_le_bytes());
        output[16] = self.candidate.honored_aon_mask;
    }

    /// Decode two fills, one AON mask, and seven canonical zero bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_len(input, DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1)?;
        if input[17..24].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(Self {
            candidate: DirectEconomicCandidateV1 {
                fills: [u64_at(input, 0), u64_at(input, 8)],
                honored_aon_mask: input[16],
            },
        })
    }
}

/// Require the exact empty payload used by actions 3 and 6..=13.
pub fn decode_direct_empty_payload_v1(input: &[u8]) -> Result<()> {
    require_len(input, 0)
}

fn require_len(input: &[u8], expected: usize) -> Result<()> {
    if input.len() == expected { Ok(()) } else { Err(CodecError::WrongLength) }
}

fn u64_at(input: &[u8], start: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[start..start + 8]);
    u64::from_le_bytes(bytes)
}

fn u128_at(input: &[u8], start: usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&input[start..start + 16]);
    u128::from_le_bytes(bytes)
}

/// Exact `0xb1/1` semantic body bytes.
pub const DIRECT_MARKET_ROOT_BODY_BYTES_V1: usize =
    registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES - 4;
/// Exact `0xb2/1` semantic body bytes.
pub const DIRECT_SELECTION_BODY_BYTES_V1: usize =
    registry::DIRECT_SELECTION_ACCOUNT_BYTES - 4;
/// Exact `0xb3/1` semantic body bytes.
pub const DIRECT_ACTION_REPLAY_BODY_BYTES_V1: usize =
    registry::DIRECT_ACTION_REPLAY_ACCOUNT_BYTES - 4;
/// Exact `0xb4/1` semantic body bytes.
pub const DIRECT_RESERVATION_BODY_BYTES_V1: usize =
    registry::DIRECT_RESERVATION_ACCOUNT_BYTES - 4;

/// Physical `0xb1/1` Direct root frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketRootAccountV1 {
    bump: u8,
    semantic_body: [u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1],
}

impl DirectMarketRootAccountV1 {
    /// Frame one semantic-owner-validated body.
    pub fn new(bump: u8, semantic_body: [u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1]) -> Result<Self> {
        require_nonzero(&semantic_body)?;
        Ok(Self { bump, semantic_body })
    }
    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 { self.bump }
    /// Exact opaque semantic body.
    pub const fn semantic_body(&self) -> &[u8; DIRECT_MARKET_ROOT_BODY_BYTES_V1] {
        &self.semantic_body
    }
    /// Encode the exact fixed frame.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES],
    ) -> Result<()> {
        encode_frame(
            registry::DIRECT_MARKET_ROOT_ACCOUNT_TAG,
            registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION,
            self.bump,
            &self.semantic_body,
            output,
        )
    }
    /// Hostile-decode the physical frame.
    pub fn decode(input: &[u8; registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES]) -> Result<Self> {
        let (bump, semantic_body) = decode_frame::<DIRECT_MARKET_ROOT_BODY_BYTES_V1,
            { registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES }>(
            registry::DIRECT_MARKET_ROOT_ACCOUNT_TAG,
            registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION,
            input,
        )?;
        Self::new(bump, semantic_body)
    }
}

/// Physical `0xb2/1` Direct Selection frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionAccountV1 {
    bump: u8,
    semantic_body: [u8; DIRECT_SELECTION_BODY_BYTES_V1],
}

impl DirectSelectionAccountV1 {
    /// Frame one semantic-owner-validated body.
    pub fn new(bump: u8, semantic_body: [u8; DIRECT_SELECTION_BODY_BYTES_V1]) -> Result<Self> {
        require_nonzero(&semantic_body)?;
        Ok(Self { bump, semantic_body })
    }
    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 { self.bump }
    /// Exact opaque semantic body.
    pub const fn semantic_body(&self) -> &[u8; DIRECT_SELECTION_BODY_BYTES_V1] {
        &self.semantic_body
    }
    /// Encode the exact fixed frame.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::DIRECT_SELECTION_ACCOUNT_BYTES],
    ) -> Result<()> {
        encode_frame(
            registry::DIRECT_SELECTION_ACCOUNT_TAG,
            registry::DIRECT_SELECTION_ACCOUNT_VERSION,
            self.bump,
            &self.semantic_body,
            output,
        )
    }
    /// Hostile-decode the physical frame.
    pub fn decode(input: &[u8; registry::DIRECT_SELECTION_ACCOUNT_BYTES]) -> Result<Self> {
        let (bump, semantic_body) = decode_frame::<DIRECT_SELECTION_BODY_BYTES_V1,
            { registry::DIRECT_SELECTION_ACCOUNT_BYTES }>(
            registry::DIRECT_SELECTION_ACCOUNT_TAG,
            registry::DIRECT_SELECTION_ACCOUNT_VERSION,
            input,
        )?;
        Self::new(bump, semantic_body)
    }
}

/// Physical permanent `0xb3/1` Direct replay frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectActionReplayAccountV1 {
    bump: u8,
    semantic_body: [u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1],
}

impl DirectActionReplayAccountV1 {
    /// Frame one semantic-owner-validated body.
    pub fn new(bump: u8, semantic_body: [u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1]) -> Result<Self> {
        require_nonzero(&semantic_body)?;
        Ok(Self { bump, semantic_body })
    }
    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 { self.bump }
    /// Exact opaque semantic body.
    pub const fn semantic_body(&self) -> &[u8; DIRECT_ACTION_REPLAY_BODY_BYTES_V1] {
        &self.semantic_body
    }
    /// Encode the exact fixed frame.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::DIRECT_ACTION_REPLAY_ACCOUNT_BYTES],
    ) -> Result<()> {
        encode_frame(
            registry::DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            registry::DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            self.bump,
            &self.semantic_body,
            output,
        )
    }
    /// Hostile-decode the physical frame.
    pub fn decode(input: &[u8; registry::DIRECT_ACTION_REPLAY_ACCOUNT_BYTES]) -> Result<Self> {
        let (bump, semantic_body) = decode_frame::<DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
            { registry::DIRECT_ACTION_REPLAY_ACCOUNT_BYTES }>(
            registry::DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            registry::DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            input,
        )?;
        Self::new(bump, semantic_body)
    }
}

/// Physical `0xb4/1` Direct Reservation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAccountV1 {
    bump: u8,
    semantic_body: [u8; DIRECT_RESERVATION_BODY_BYTES_V1],
}

impl DirectReservationAccountV1 {
    /// Frame one semantic-owner-validated body.
    pub fn new(bump: u8, semantic_body: [u8; DIRECT_RESERVATION_BODY_BYTES_V1]) -> Result<Self> {
        require_nonzero(&semantic_body)?;
        Ok(Self { bump, semantic_body })
    }
    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 { self.bump }
    /// Exact opaque semantic body.
    pub const fn semantic_body(&self) -> &[u8; DIRECT_RESERVATION_BODY_BYTES_V1] {
        &self.semantic_body
    }
    /// Encode the exact fixed frame.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::DIRECT_RESERVATION_ACCOUNT_BYTES],
    ) -> Result<()> {
        encode_frame(
            registry::DIRECT_RESERVATION_ACCOUNT_TAG,
            registry::DIRECT_RESERVATION_ACCOUNT_VERSION,
            self.bump,
            &self.semantic_body,
            output,
        )
    }
    /// Hostile-decode the physical frame.
    pub fn decode(input: &[u8; registry::DIRECT_RESERVATION_ACCOUNT_BYTES]) -> Result<Self> {
        let (bump, semantic_body) = decode_frame::<DIRECT_RESERVATION_BODY_BYTES_V1,
            { registry::DIRECT_RESERVATION_ACCOUNT_BYTES }>(
            registry::DIRECT_RESERVATION_ACCOUNT_TAG,
            registry::DIRECT_RESERVATION_ACCOUNT_VERSION,
            input,
        )?;
        Self::new(bump, semantic_body)
    }
}

fn encode_frame<const BODY: usize, const ACCOUNT: usize>(
    tag: u8,
    version: u8,
    bump: u8,
    body: &[u8; BODY],
    output: &mut [u8; ACCOUNT],
) -> Result<()> {
    if ACCOUNT != BODY.checked_add(4).ok_or(CodecError::ArithmeticOverflow)? {
        return Err(CodecError::WrongLength);
    }
    output[0] = tag;
    output[1] = version;
    output[2] = bump;
    output[3] = 0;
    output[4..].copy_from_slice(body);
    Ok(())
}

fn decode_frame<const BODY: usize, const ACCOUNT: usize>(
    tag: u8,
    version: u8,
    input: &[u8; ACCOUNT],
) -> Result<(u8, [u8; BODY])> {
    if ACCOUNT != BODY.checked_add(4).ok_or(CodecError::ArithmeticOverflow)? {
        return Err(CodecError::WrongLength);
    }
    if input[0] != tag {
        return Err(CodecError::WrongTag);
    }
    if input[1] != version {
        return Err(CodecError::WrongVersion);
    }
    if input[3] != 0 {
        return Err(CodecError::NonCanonicalPadding);
    }
    let mut body = [0u8; BODY];
    body.copy_from_slice(&input[4..]);
    require_nonzero(&body)?;
    Ok((input[2], body))
}

fn require_nonzero(value: &[u8]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(CodecError::ZeroValue)
    } else {
        Ok(())
    }
}

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V1 == 1_078);
const _: () = assert!(DIRECT_SELECTION_BODY_BYTES_V1 == 1_497);
const _: () = assert!(DIRECT_ACTION_REPLAY_BODY_BYTES_V1 == 321);
const _: () = assert!(DIRECT_RESERVATION_BODY_BYTES_V1 == 453);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_frames_refuse_cross_class_and_reserved_bytes() {
        let mut body = [0u8; DIRECT_RESERVATION_BODY_BYTES_V1];
        body[0] = 1;
        let value = DirectReservationAccountV1::new(7, body).unwrap();
        let mut encoded = [0u8; registry::DIRECT_RESERVATION_ACCOUNT_BYTES];
        value.encode_into(&mut encoded).unwrap();
        assert_eq!(DirectReservationAccountV1::decode(&encoded), Ok(value));
        encoded[0] = registry::DIRECT_SELECTION_ACCOUNT_TAG;
        assert_eq!(
            DirectReservationAccountV1::decode(&encoded),
            Err(CodecError::WrongTag)
        );
        encoded[0] = registry::DIRECT_RESERVATION_ACCOUNT_TAG;
        encoded[3] = 1;
        assert_eq!(
            DirectReservationAccountV1::decode(&encoded),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[test]
    fn direct_payloads_refuse_truncation_enums_and_padding() {
        let mut order = [0u8; DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1];
        order[..32].fill(1);
        order[32] = 1;
        order[34] = 1;
        order[40..48].copy_from_slice(&1u64.to_le_bytes());
        assert!(DirectAdmitOrderPayloadV1::decode(&order).is_ok());
        order[32] = 3;
        assert_eq!(
            DirectAdmitOrderPayloadV1::decode(&order),
            Err(CodecError::InvalidEnum)
        );
        order[32] = 1;
        order[35] = 1;
        assert_eq!(
            DirectAdmitOrderPayloadV1::decode(&order),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut candidate = [0u8; DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1];
        candidate[23] = 1;
        assert_eq!(
            DirectSubmitCandidatePayloadV1::decode(&candidate),
            Err(CodecError::NonCanonicalPadding)
        );
        assert_eq!(decode_direct_empty_payload_v1(&[0]), Err(CodecError::WrongLength));
    }

    #[test]
    fn client_encoders_round_trip_zero_price_and_canonical_padding() {
        let order = DirectAdmitOrderPayloadV1 {
            order_id: [7; 32],
            side: Side::Buy,
            outcome: 0,
            partial_policy: PartialPolicy::Allow,
            quantity: 9,
            minimum_fill: 1,
            expiry_epoch: 4,
            limit_price_units_per_egg: 0,
        };
        let mut encoded = [0xff; DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1];
        order.encode_into(&mut encoded).unwrap();
        assert_eq!(DirectAdmitOrderPayloadV1::decode(&encoded), Ok(order));
        assert_eq!(&encoded[35..40], &[0; 5]);

        let candidate = DirectSubmitCandidatePayloadV1 {
            candidate: DirectEconomicCandidateV1 {
                fills: [9, 9],
                honored_aon_mask: 0,
            },
        };
        let mut candidate_bytes = [0xff; DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1];
        candidate.encode_into(&mut candidate_bytes);
        assert_eq!(
            DirectSubmitCandidatePayloadV1::decode(&candidate_bytes),
            Ok(candidate)
        );
        assert_eq!(&candidate_bytes[17..], &[0; 7]);
    }
}
