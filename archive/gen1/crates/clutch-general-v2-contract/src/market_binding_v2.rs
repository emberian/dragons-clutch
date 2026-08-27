// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable MarketBinding successor for owner-net candidate-cost selection.
//!
//! V2 retains the exact V1 semantic body under the same account tag and adds
//! one content identity: the immutable batch policy used by the preselection
//! cost certificate. It does not allocate a second market-policy account.

use crate::{
    CodecError, Id32, MarketBindingV1, MARKET_BINDING_ACCOUNT_BYTES,
    MARKET_BINDING_ACCOUNT_BYTES_V2, MARKET_BINDING_ACCOUNT_TAG,
    MARKET_BINDING_ACCOUNT_VERSION, MARKET_BINDING_ACCOUNT_VERSION_V2,
    SCORE_V2_Q_ACTIVE_RANK_BYTES, SCORE_V2_Q_COST_ACTIVE_RANK_BYTES,
};

/// Byte offset of `rank_key_len` inside the exact V1 prefix.
const MARKET_BINDING_RANK_KEY_LEN_OFFSET: usize =
    2 + (12 * 32) + (18 * 8) + 4 + 2;

/// Breaking immutable MarketBinding successor.
///
/// `base` remains the sole owner of every V1 fact. It carries the V2 active
/// rank width while [`Self::relation_projection`] supplies the exact V1-width
/// projection needed by owner-blind RelationV2 code that never reads rank
/// bytes. `batch_policy_id` is a canonical content identity, not a fee record,
/// revenue policy, or mutable policy selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV2 {
    base: MarketBindingV1,
    batch_policy_id: Id32,
}

impl MarketBindingV2 {
    /// Construct the successor from the complete immutable V1 fact set and
    /// one independently content-addressed batch policy.
    pub fn new(
        base: MarketBindingV1,
        batch_policy_id: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            base,
            batch_policy_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Complete immutable market facts, with the 96-byte V2 rank width.
    pub const fn base(&self) -> &MarketBindingV1 {
        &self.base
    }

    /// Canonical immutable batch-policy content identity.
    pub const fn batch_policy_id(&self) -> Id32 {
        self.batch_policy_id
    }

    /// Owner-blind RelationV2 projection of the same facts.
    ///
    /// Only the legacy rank-length byte changes. No semantic ID, timing,
    /// reward, Product binding, candidate family, bump, or flags are dropped.
    pub fn relation_projection(&self) -> MarketBindingV1 {
        let mut projection = self.base;
        projection.rank_key_len = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
        projection
    }

    /// Validate V1 geometry, the breaking rank width, and disjoint policy IDs.
    pub fn validate(&self) -> Result<(), CodecError> {
        if usize::from(self.base.rank_key_len) != SCORE_V2_Q_COST_ACTIVE_RANK_BYTES
            || self.batch_policy_id.is_zero()
        {
            return Err(CodecError::InvalidState);
        }
        self.relation_projection().validate()?;
        for other in [
            self.base.market,
            self.base.market_genesis_profile_v2_id,
            self.base.market_instance_v2_id,
            self.base.series_plan_v5_id,
            self.base.series_funding_terms_v2_id,
            self.base.relation_policy_id,
            self.base.price_measure_policy_v1_id,
            self.base.native_claim_basis_id,
            self.base.admission_policy_id,
            self.base.score_policy_id,
            self.base.settlement_policy_id,
            self.base.neutral_sink,
        ] {
            if self.batch_policy_id == other {
                return Err(CodecError::MismatchedBinding);
            }
        }
        Ok(())
    }

    /// Encode tag `0x79`, version `2`, the exact V1 semantic prefix, then the
    /// immutable batch-policy identity.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != MARKET_BINDING_ACCOUNT_BYTES_V2 {
            return Err(CodecError::WrongLength);
        }
        let mut prefix = [0u8; MARKET_BINDING_ACCOUNT_BYTES];
        self.relation_projection().encode(&mut prefix)?;
        prefix[1] = MARKET_BINDING_ACCOUNT_VERSION_V2;
        prefix[MARKET_BINDING_RANK_KEY_LEN_OFFSET] = self.base.rank_key_len;
        output[..MARKET_BINDING_ACCOUNT_BYTES].copy_from_slice(&prefix);
        output[MARKET_BINDING_ACCOUNT_BYTES..].copy_from_slice(&self.batch_policy_id.bytes());
        Ok(())
    }

    /// Decode only the exact V2 width, tag, version, rank width, and live
    /// immutable batch-policy identity.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != MARKET_BINDING_ACCOUNT_BYTES_V2 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != MARKET_BINDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != MARKET_BINDING_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        if usize::from(input[MARKET_BINDING_RANK_KEY_LEN_OFFSET])
            != SCORE_V2_Q_COST_ACTIVE_RANK_BYTES
        {
            return Err(CodecError::InvalidState);
        }
        let mut prefix = [0u8; MARKET_BINDING_ACCOUNT_BYTES];
        prefix.copy_from_slice(&input[..MARKET_BINDING_ACCOUNT_BYTES]);
        prefix[1] = MARKET_BINDING_ACCOUNT_VERSION;
        prefix[MARKET_BINDING_RANK_KEY_LEN_OFFSET] = SCORE_V2_Q_ACTIVE_RANK_BYTES as u8;
        let mut base = MarketBindingV1::decode(&prefix)?;
        base.rank_key_len = SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8;
        let batch_policy_id = Id32::new(
            input[MARKET_BINDING_ACCOUNT_BYTES..]
                .try_into()
                .map_err(|_| CodecError::WrongLength)?,
        )?;
        Self::new(base, batch_policy_id)
    }
}

const _: () = assert!(MARKET_BINDING_RANK_KEY_LEN_OFFSET == 536);
const _: () = assert!(MARKET_BINDING_ACCOUNT_BYTES_V2 == MARKET_BINDING_ACCOUNT_BYTES + 32);
const _: () = assert!(SCORE_V2_Q_COST_ACTIVE_RANK_BYTES == 96);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn binding() -> MarketBindingV2 {
        MarketBindingV2::new(
            MarketBindingV1 {
                market: id(1),
                market_genesis_profile_v2_id: id(2),
                market_instance_v2_id: id(3),
                series_plan_v5_id: id(4),
                series_funding_terms_v2_id: id(5),
                relation_policy_id: id(6),
                price_measure_policy_v1_id: id(7),
                native_claim_basis_id: id(8),
                admission_policy_id: id(9),
                score_policy_id: id(10),
                settlement_policy_id: id(11),
                neutral_sink: id(12),
                price_scale: 10_000,
                commit_span_slots: 1,
                reveal_span_slots: 2,
                verification_span_slots: 3,
                bond_lamports: 100,
                invalidity_penalty: 10,
                abandonment_penalty: 11,
                node_cleanup_reward: 1,
                price_check_reward: 1,
                order_reward: 1,
                slice_reward: 1,
                completion_reward: 1,
                work_close_reward: 1,
                feed_close_reward: 1,
                freeze_reward: 1,
                finalize_reward: 1,
                solver_prize: 1,
                root_close_reward: 1,
                relation_version: 2,
                outcome_count: 3,
                basis_degree: 2,
                rank_key_len: SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8,
                candidate_kind_mask: 1,
                stored_bump: 13,
                flags: 0,
            },
            id(14),
        )
        .unwrap()
    }

    #[test]
    fn v2_round_trip_preserves_one_v1_owner_and_batch_policy() {
        let value = binding();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        value.encode(&mut bytes).unwrap();
        assert_eq!(bytes[0], MARKET_BINDING_ACCOUNT_TAG);
        assert_eq!(bytes[1], MARKET_BINDING_ACCOUNT_VERSION_V2);
        assert_eq!(bytes[MARKET_BINDING_RANK_KEY_LEN_OFFSET], 96);
        assert_eq!(MarketBindingV2::decode(&bytes), Ok(value));
        assert_eq!(value.relation_projection().rank_key_len, 88);
    }

    #[test]
    fn policy_alias_and_hostile_v1_reinterpretation_are_refused() {
        let value = binding();
        assert_eq!(
            MarketBindingV2::new(*value.base(), value.base().score_policy_id),
            Err(CodecError::MismatchedBinding)
        );
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        value.encode(&mut bytes).unwrap();
        bytes[1] = MARKET_BINDING_ACCOUNT_VERSION;
        assert_eq!(MarketBindingV2::decode(&bytes), Err(CodecError::WrongVersion));
    }
}
