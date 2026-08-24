// SPDX-License-Identifier: AGPL-3.0-or-later

//! Root-bound immutable General founding policy.
//!
//! Product stores only the semantic identity of this exact body in its
//! `general_founding_capability_id`. General hostile-decodes the caller-supplied
//! preimage and recomputes that identity before it may construct a current
//! MarketBinding. The body is therefore the sole owner of General timing,
//! admission, settlement, bond, penalty, and reward geometry.

use crate::{CodecError, Id32, MarketBindingV1, Reader, Sha256BackendV1, Writer, MAX_ORDERS, MAX_SLICES};

/// Domain-separated identity of one exact General founding-policy body.
pub const GENERAL_FOUNDING_POLICY_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general/founding-policy/v1\0";
/// Exact fixed body width: header, two policy IDs, and eighteen `u64` values.
pub const GENERAL_FOUNDING_POLICY_BYTES_V1: usize = 224;
const MAGIC_V1: &[u8; 8] = b"DCGENFP1";
const VERSION_V1: u8 = 1;
const HEADER_BYTES_V1: usize = 16;

/// Immutable General policy facts frozen before Product market-root creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFoundingPolicyV1 {
    /// Funded admission policy identity.
    pub admission_policy_id: Id32,
    /// Exact settlement/allocation policy identity.
    pub settlement_policy_id: Id32,
    /// Exact integer simplex scale.
    pub price_scale: u64,
    /// Commit subinterval span.
    pub commit_span_slots: u64,
    /// Reveal subinterval span.
    pub reveal_span_slots: u64,
    /// Verification interval span.
    pub verification_span_slots: u64,
    /// Per-node admission bond.
    pub bond_lamports: u64,
    /// Checked-invalidity penalty.
    pub invalidity_penalty: u64,
    /// Unrevealed-commitment abandonment penalty.
    pub abandonment_penalty: u64,
    /// Prepaid permissionless node cleanup reward.
    pub node_cleanup_reward: u64,
    /// Reward for price-certificate checking.
    pub price_check_reward: u64,
    /// Reward per newly checked order.
    pub order_reward: u64,
    /// Reward per newly checked settlement slice.
    pub slice_reward: u64,
    /// Reward for a completed verdict.
    pub completion_reward: u64,
    /// Reward for closing ClearWork.
    pub work_close_reward: u64,
    /// Reward for closing a feed/stage.
    pub feed_close_reward: u64,
    /// Root freeze reward.
    pub freeze_reward: u64,
    /// Root finalization reward.
    pub finalize_reward: u64,
    /// Unique selected-solver prize.
    pub solver_prize: u64,
    /// Root retirement reward.
    pub root_close_reward: u64,
}

impl GeneralFoundingPolicyV1 {
    /// Hostile-validate identities, nonzero finite widths, penalty bounds, and
    /// every maximum-width reward multiplication used by General.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.admission_policy_id.is_zero()
            || self.settlement_policy_id.is_zero()
            || self.price_scale == 0
            || self.commit_span_slots == 0
            || self.reveal_span_slots == 0
            || self.verification_span_slots == 0
            || self.bond_lamports == 0
            || self.invalidity_penalty == 0
            || self.invalidity_penalty > self.bond_lamports
            || self.abandonment_penalty == 0
            || self.abandonment_penalty > self.bond_lamports
            || self.reward_values().iter().any(|value| *value == 0)
        {
            return Err(CodecError::InvalidState);
        }
        self.commit_span_slots
            .checked_add(self.reveal_span_slots)
            .and_then(|value| value.checked_add(self.verification_span_slots))
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.order_reward
            .checked_mul(MAX_ORDERS as u64)
            .and_then(|value| {
                self.slice_reward
                    .checked_mul(MAX_SLICES as u64)
                    .and_then(|slices| value.checked_add(slices))
            })
            .and_then(|value| value.checked_add(self.price_check_reward))
            .and_then(|value| value.checked_add(self.completion_reward))
            .and_then(|value| value.checked_add(self.work_close_reward))
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.bond_lamports
            .checked_add(self.node_cleanup_reward)
            .and_then(|value| value.checked_add(self.feed_close_reward))
            .and_then(|value| value.checked_add(self.freeze_reward))
            .and_then(|value| value.checked_add(self.finalize_reward))
            .and_then(|value| value.checked_add(self.solver_prize))
            .and_then(|value| value.checked_add(self.root_close_reward))
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Encode the one canonical 224-byte semantic preimage.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, GENERAL_FOUNDING_POLICY_BYTES_V1)?;
        writer.bytes(MAGIC_V1)?;
        writer.u8(VERSION_V1)?;
        writer.bytes(&[0u8; HEADER_BYTES_V1 - 9])?;
        writer.bytes(&self.admission_policy_id.bytes())?;
        writer.bytes(&self.settlement_policy_id.bytes())?;
        for value in self.scalar_values() {
            writer.u64(value)?;
        }
        writer.finish()
    }

    /// Decode and validate exactly one hostile 224-byte semantic preimage.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, GENERAL_FOUNDING_POLICY_BYTES_V1)?;
        if reader.array::<8>()? != *MAGIC_V1 || reader.u8()? != VERSION_V1 {
            return Err(CodecError::WrongVersion);
        }
        if reader.array::<{ HEADER_BYTES_V1 - 9 }>()? != [0u8; HEADER_BYTES_V1 - 9] {
            return Err(CodecError::NonCanonicalPadding);
        }
        let admission_policy_id = Id32::new(reader.array()?)?;
        let settlement_policy_id = Id32::new(reader.array()?)?;
        let value = Self {
            admission_policy_id,
            settlement_policy_id,
            price_scale: reader.u64()?,
            commit_span_slots: reader.u64()?,
            reveal_span_slots: reader.u64()?,
            verification_span_slots: reader.u64()?,
            bond_lamports: reader.u64()?,
            invalidity_penalty: reader.u64()?,
            abandonment_penalty: reader.u64()?,
            node_cleanup_reward: reader.u64()?,
            price_check_reward: reader.u64()?,
            order_reward: reader.u64()?,
            slice_reward: reader.u64()?,
            completion_reward: reader.u64()?,
            work_close_reward: reader.u64()?,
            feed_close_reward: reader.u64()?,
            freeze_reward: reader.u64()?,
            finalize_reward: reader.u64()?,
            solver_prize: reader.u64()?,
            root_close_reward: reader.u64()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Derive the exact Product-stored General founding capability identity.
    pub fn semantic_id<B: Sha256BackendV1>(self, backend: &B) -> Result<Id32, CodecError> {
        let mut body = [0u8; GENERAL_FOUNDING_POLICY_BYTES_V1];
        self.encode(&mut body)?;
        Id32::new(backend.sha256(&[GENERAL_FOUNDING_POLICY_ID_DOMAIN_V1, &body]))
    }

    /// Require the current MarketBinding to carry exactly this frozen policy.
    pub fn binds_market(self, market: &MarketBindingV1) -> Result<(), CodecError> {
        self.validate()?;
        market.validate()?;
        if self.admission_policy_id != market.admission_policy_id
            || self.settlement_policy_id != market.settlement_policy_id
            || self.scalar_values()
                != [
                    market.price_scale,
                    market.commit_span_slots,
                    market.reveal_span_slots,
                    market.verification_span_slots,
                    market.bond_lamports,
                    market.invalidity_penalty,
                    market.abandonment_penalty,
                    market.node_cleanup_reward,
                    market.price_check_reward,
                    market.order_reward,
                    market.slice_reward,
                    market.completion_reward,
                    market.work_close_reward,
                    market.feed_close_reward,
                    market.freeze_reward,
                    market.finalize_reward,
                    market.solver_prize,
                    market.root_close_reward,
                ]
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    const fn scalar_values(self) -> [u64; 18] {
        [
            self.price_scale,
            self.commit_span_slots,
            self.reveal_span_slots,
            self.verification_span_slots,
            self.bond_lamports,
            self.invalidity_penalty,
            self.abandonment_penalty,
            self.node_cleanup_reward,
            self.price_check_reward,
            self.order_reward,
            self.slice_reward,
            self.completion_reward,
            self.work_close_reward,
            self.feed_close_reward,
            self.freeze_reward,
            self.finalize_reward,
            self.solver_prize,
            self.root_close_reward,
        ]
    }

    const fn reward_values(self) -> [u64; 11] {
        [
            self.node_cleanup_reward,
            self.price_check_reward,
            self.order_reward,
            self.slice_reward,
            self.completion_reward,
            self.work_close_reward,
            self.feed_close_reward,
            self.freeze_reward,
            self.finalize_reward,
            self.solver_prize,
            self.root_close_reward,
        ]
    }
}

const _: () = assert!(GENERAL_FOUNDING_POLICY_BYTES_V1 == HEADER_BYTES_V1 + 64 + (18 * 8));

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Hash;

    impl Sha256BackendV1 for Hash {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
            let mut output = [0u8; 32];
            for part in parts {
                for (index, byte) in part.iter().enumerate() {
                    output[index % 32] = output[index % 32].wrapping_mul(31).wrapping_add(*byte);
                }
            }
            output
        }
    }

    fn policy() -> GeneralFoundingPolicyV1 {
        GeneralFoundingPolicyV1 {
            admission_policy_id: Id32::new([1; 32]).unwrap(),
            settlement_policy_id: Id32::new([2; 32]).unwrap(),
            price_scale: 1_000_000,
            commit_span_slots: 8,
            reveal_span_slots: 9,
            verification_span_slots: 10,
            bond_lamports: 10_000,
            invalidity_penalty: 1_000,
            abandonment_penalty: 2_000,
            node_cleanup_reward: 11,
            price_check_reward: 12,
            order_reward: 13,
            slice_reward: 14,
            completion_reward: 15,
            work_close_reward: 16,
            feed_close_reward: 17,
            freeze_reward: 18,
            finalize_reward: 19,
            solver_prize: 20,
            root_close_reward: 21,
        }
    }

    #[test]
    fn exact_round_trip_and_full_body_identity() {
        let value = policy();
        let mut body = [0u8; GENERAL_FOUNDING_POLICY_BYTES_V1];
        value.encode(&mut body).unwrap();
        assert_eq!(GeneralFoundingPolicyV1::decode(&body), Ok(value));
        let before = value.semantic_id(&Hash).unwrap();
        body[96] ^= 1;
        let changed = GeneralFoundingPolicyV1::decode(&body).unwrap();
        assert_ne!(changed.semantic_id(&Hash).unwrap(), before);
    }

    #[test]
    fn hostile_padding_and_maximum_reward_overflow_refuse() {
        let mut body = [0u8; GENERAL_FOUNDING_POLICY_BYTES_V1];
        policy().encode(&mut body).unwrap();
        body[15] = 1;
        assert_eq!(
            GeneralFoundingPolicyV1::decode(&body),
            Err(CodecError::NonCanonicalPadding)
        );
        let mut overflow = policy();
        overflow.slice_reward = u64::MAX;
        assert_eq!(overflow.validate(), Err(CodecError::ArithmeticOverflow));
    }
}
