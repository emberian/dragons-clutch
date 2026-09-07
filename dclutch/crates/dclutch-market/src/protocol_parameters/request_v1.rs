//! The one request wire for the four governance acts.
//!
//! A header naming the act, then the proposed BODY in the record's own field
//! order. `found` carries the genesis body with the founder's key as authority
//! and the adapter refuses any other; `withdraw` carries zeros; `propose` and
//! `apply` carry the value. The body is exactly what
//! [`ProtocolParametersV1::body_digest`] hashes, so a proposal and its apply
//! are compared over these bytes and nothing else.

use super::generated::{
    PROTOCOL_PARAMETERS_REQUEST_ACT_OFFSET, PROTOCOL_PARAMETERS_REQUEST_CHANGE_DELAY_SLOTS_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_CLOSER_CARVE_BASIS_POINTS_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_CLOSER_REWARD_CAP_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_CRANK_REWARD_CAP_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_GOVERNANCE_AUTHORITY_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_MAX_FEE_BASIS_POINTS_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_PROTOCOL_BENEFICIARY_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_RESERVED_HEADER_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_RESERVED_TAIL_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_TAKE_BASIS_POINTS_OFFSET,
    PROTOCOL_PARAMETERS_REQUEST_VERSION_OFFSET,
};
use super::{
    Error, PROTOCOL_GOVERNANCE_APPLY_TAG_V1, PROTOCOL_GOVERNANCE_FOUND_TAG_V1,
    PROTOCOL_GOVERNANCE_PROPOSE_TAG_V1, PROTOCOL_GOVERNANCE_WITHDRAW_TAG_V1,
    PROTOCOL_PARAMETERS_ABI_VERSION_V1, PROTOCOL_PARAMETERS_REQUEST_BYTES_V1,
    PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1, ProtocolParametersV1, Result, array_at, put_array,
    put_u16, put_u64, u16_at, u64_at,
};

const RESERVED_HEADER_BYTES: usize = 5;
const RESERVED_TAIL_BYTES: usize = 2;

/// The four acts. Lean twin: `GovernanceAct`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GovernanceActV1 {
    /// Write the genesis record once, with the founder as placeholder
    /// authority (decision 0024 section 3).
    Found = PROTOCOL_GOVERNANCE_FOUND_TAG_V1,
    /// The authority's act: stage a value behind the delay.
    Propose = PROTOCOL_GOVERNANCE_PROPOSE_TAG_V1,
    /// The authority's act: take a standing proposal back.
    Withdraw = PROTOCOL_GOVERNANCE_WITHDRAW_TAG_V1,
    /// Anybody's act: install a matured proposal and write its receipt.
    Apply = PROTOCOL_GOVERNANCE_APPLY_TAG_V1,
}

impl GovernanceActV1 {
    /// Decode one act byte.
    pub const fn decode(value: u8) -> Result<Self> {
        match value {
            PROTOCOL_GOVERNANCE_FOUND_TAG_V1 => Ok(Self::Found),
            PROTOCOL_GOVERNANCE_PROPOSE_TAG_V1 => Ok(Self::Propose),
            PROTOCOL_GOVERNANCE_WITHDRAW_TAG_V1 => Ok(Self::Withdraw),
            PROTOCOL_GOVERNANCE_APPLY_TAG_V1 => Ok(Self::Apply),
            _ => Err(Error::UnknownAct),
        }
    }

    /// The canonical one-byte tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Whether the act carries a body at all. A withdraw carries zeros.
    #[must_use]
    pub const fn carries_body(self) -> bool {
        !matches!(self, Self::Withdraw)
    }
}

/// One exact governance request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolParametersRequestV1 {
    /// Which act.
    pub act: GovernanceActV1,
    /// The proposed value; the genesis body for a founding; all-zero for a
    /// withdraw, where it is not read.
    pub body: ProtocolParametersV1,
}

impl ProtocolParametersRequestV1 {
    /// The all-zero body a withdraw carries.
    pub const EMPTY_BODY: ProtocolParametersV1 = ProtocolParametersV1 {
        governance_authority: [0; 32],
        protocol_beneficiary: [0; 32],
        generation: 0,
        activation_slot: 0,
        change_delay_slots: 0,
        closer_reward_cap_lamports: 0,
        crank_reward_cap_lamports: 0,
        max_fee_basis_points: 0,
        protocol_take_basis_points: 0,
        closer_carve_basis_points: 0,
    };

    /// A withdraw request.
    pub const WITHDRAW: Self = Self {
        act: GovernanceActV1::Withdraw,
        body: Self::EMPTY_BODY,
    };

    /// Hostile-decode one exact request.
    ///
    /// The bookkeeping fields of the body (`generation`, `activation_slot`) are
    /// not on the wire: a proposal is a value, and those are what applying it
    /// writes. They decode as zero.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROTOCOL_PARAMETERS_REQUEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1.as_slice())
            || u16_at(input, PROTOCOL_PARAMETERS_REQUEST_VERSION_OFFSET)
                != PROTOCOL_PARAMETERS_ABI_VERSION_V1
        {
            return Err(Error::InvalidHeader);
        }
        for (offset, width) in [
            (
                PROTOCOL_PARAMETERS_REQUEST_RESERVED_HEADER_OFFSET,
                RESERVED_HEADER_BYTES,
            ),
            (
                PROTOCOL_PARAMETERS_REQUEST_RESERVED_TAIL_OFFSET,
                RESERVED_TAIL_BYTES,
            ),
        ] {
            if input
                .get(offset..offset + width)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(Error::NonCanonical);
            }
        }
        let act = GovernanceActV1::decode(
            *input
                .get(PROTOCOL_PARAMETERS_REQUEST_ACT_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let body = ProtocolParametersV1 {
            governance_authority: array_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_GOVERNANCE_AUTHORITY_OFFSET,
            ),
            protocol_beneficiary: array_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_PROTOCOL_BENEFICIARY_OFFSET,
            ),
            generation: 0,
            activation_slot: 0,
            change_delay_slots: u64_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_CHANGE_DELAY_SLOTS_OFFSET,
            ),
            closer_reward_cap_lamports: u64_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_CLOSER_REWARD_CAP_OFFSET,
            ),
            crank_reward_cap_lamports: u64_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_CRANK_REWARD_CAP_OFFSET,
            ),
            max_fee_basis_points: u16_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_MAX_FEE_BASIS_POINTS_OFFSET,
            ),
            protocol_take_basis_points: u16_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_TAKE_BASIS_POINTS_OFFSET,
            ),
            closer_carve_basis_points: u16_at(
                input,
                PROTOCOL_PARAMETERS_REQUEST_CLOSER_CARVE_BASIS_POINTS_OFFSET,
            ),
        };
        if !act.carries_body() && body != Self::EMPTY_BODY {
            return Err(Error::NonCanonical);
        }
        Ok(Self { act, body })
    }

    /// Encode one canonical request.
    #[must_use]
    pub fn to_bytes(self) -> [u8; PROTOCOL_PARAMETERS_REQUEST_BYTES_V1] {
        let mut output = [0_u8; PROTOCOL_PARAMETERS_REQUEST_BYTES_V1];
        output[..8].copy_from_slice(&PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1);
        put_u16(
            &mut output,
            PROTOCOL_PARAMETERS_REQUEST_VERSION_OFFSET,
            PROTOCOL_PARAMETERS_ABI_VERSION_V1,
        );
        output[PROTOCOL_PARAMETERS_REQUEST_ACT_OFFSET] = self.act.tag();
        let body = if self.act.carries_body() {
            self.body
        } else {
            Self::EMPTY_BODY
        };
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_REQUEST_GOVERNANCE_AUTHORITY_OFFSET,
            &body.governance_authority,
        );
        put_array(
            &mut output,
            PROTOCOL_PARAMETERS_REQUEST_PROTOCOL_BENEFICIARY_OFFSET,
            &body.protocol_beneficiary,
        );
        for (offset, value) in [
            (
                PROTOCOL_PARAMETERS_REQUEST_CHANGE_DELAY_SLOTS_OFFSET,
                body.change_delay_slots,
            ),
            (
                PROTOCOL_PARAMETERS_REQUEST_CLOSER_REWARD_CAP_OFFSET,
                body.closer_reward_cap_lamports,
            ),
            (
                PROTOCOL_PARAMETERS_REQUEST_CRANK_REWARD_CAP_OFFSET,
                body.crank_reward_cap_lamports,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        for (offset, value) in [
            (
                PROTOCOL_PARAMETERS_REQUEST_MAX_FEE_BASIS_POINTS_OFFSET,
                body.max_fee_basis_points,
            ),
            (
                PROTOCOL_PARAMETERS_REQUEST_TAKE_BASIS_POINTS_OFFSET,
                body.protocol_take_basis_points,
            ),
            (
                PROTOCOL_PARAMETERS_REQUEST_CLOSER_CARVE_BASIS_POINTS_OFFSET,
                body.closer_carve_basis_points,
            ),
        ] {
            put_u16(&mut output, offset, value);
        }
        output
    }
}
