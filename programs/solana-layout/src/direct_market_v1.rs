// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical frames for the disabled current Direct `0xb1..=0xb4/v1` family.
//!
//! This module owns only tag/version/bump/reserved framing. The semantic bodies
//! are interpreted exclusively by `clutch-direct-market-runtime`.

use crate::{registry, CodecError, Result};

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

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V1 == 754);
const _: () = assert!(DIRECT_SELECTION_BODY_BYTES_V1 == 1_497);
const _: () = assert!(DIRECT_ACTION_REPLAY_BODY_BYTES_V1 == 289);
const _: () = assert!(DIRECT_RESERVATION_BODY_BYTES_V1 == 421);

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
}

