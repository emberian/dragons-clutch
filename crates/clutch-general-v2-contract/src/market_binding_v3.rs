// SPDX-License-Identifier: AGPL-3.0-or-later

//! Historical Product-family-authorized, rent-owned MarketBinding schema.
//!
//! V3 retains the complete V2 market/policy body and adds only immutable facts
//! authenticated by Product before the General account write. The Product
//! preauthorization commits the exact writable `0xaa` and `0xad` prestates;
//! this account deliberately does not persist the later Product admission
//! projection, avoiding a cyclic identity between the General and Product
//! postwrites. Its BundleV5/AttachmentV4 coordinates are decode-only after the
//! current Product graph advanced; V4 is the only live successor authority.

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, MarketBindingV2,
    MARKET_BINDING_ACCOUNT_BYTES_V2, MARKET_BINDING_ACCOUNT_BYTES_V3,
    MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION_V2,
    MARKET_BINDING_ACCOUNT_VERSION_V3,
};

const PRODUCT_ROOT_ACCOUNT_OFFSET: usize = MARKET_BINDING_ACCOUNT_BYTES_V2;
const PRODUCT_BINDING_ID_OFFSET: usize = PRODUCT_ROOT_ACCOUNT_OFFSET + 32;
const PRODUCT_GENERATION_OFFSET: usize = PRODUCT_BINDING_ID_OFFSET + 32;
const SERIES_LINK_ACCOUNT_OFFSET: usize = PRODUCT_GENERATION_OFFSET + 8;
const SERIES_ORDINAL_OFFSET: usize = SERIES_LINK_ACCOUNT_OFFSET + 32;
const COMPILER_BUNDLE_ID_OFFSET: usize = SERIES_ORDINAL_OFFSET + 4;
const ATTACHMENT_PLAN_ID_OFFSET: usize = COMPILER_BUNDLE_ID_OFFSET + 32;
const MARKET_LIABILITY_FOUNDING_ID_OFFSET: usize = ATTACHMENT_PLAN_ID_OFFSET + 32;
const CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET: usize = MARKET_LIABILITY_FOUNDING_ID_OFFSET + 32;
const CLAIM_ISSUANCE_BINDING_ID_OFFSET: usize = CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET + 32;
const GENERAL_FOUNDING_CAPABILITY_ID_OFFSET: usize = CLAIM_ISSUANCE_BINDING_ID_OFFSET + 32;
const PRODUCT_PREAUTHORIZATION_ID_OFFSET: usize = GENERAL_FOUNDING_CAPABILITY_ID_OFFSET + 32;
const RENT_OFFSET: usize = PRODUCT_PREAUTHORIZATION_ID_OFFSET + 32;

/// Immutable Product coordinates admitted before General's founding write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV3 {
    base: MarketBindingV2,
    product_market_root_account: Id32,
    product_market_binding_id: Id32,
    product_generation: u64,
    series_market_link_account: Id32,
    series_ordinal: u32,
    compiler_bundle_v5_id: Id32,
    attachment_plan_v4_id: Id32,
    market_liability_founding_id: Id32,
    claim_mint_founding_plan_id: Id32,
    claim_issuance_binding_id: Id32,
    general_founding_capability_id: Id32,
    product_preauthorization_id: Id32,
    rent: DeletableRentOwnerV1,
}

impl MarketBindingV3 {
    /// Construct the exact current Product/General binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base: MarketBindingV2,
        product_market_root_account: Id32,
        product_market_binding_id: Id32,
        product_generation: u64,
        series_market_link_account: Id32,
        series_ordinal: u32,
        compiler_bundle_v5_id: Id32,
        attachment_plan_v4_id: Id32,
        market_liability_founding_id: Id32,
        claim_mint_founding_plan_id: Id32,
        claim_issuance_binding_id: Id32,
        general_founding_capability_id: Id32,
        product_preauthorization_id: Id32,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self, CodecError> {
        let value = Self {
            base,
            product_market_root_account,
            product_market_binding_id,
            product_generation,
            series_market_link_account,
            series_ordinal,
            compiler_bundle_v5_id,
            attachment_plan_v4_id,
            market_liability_founding_id,
            claim_mint_founding_plan_id,
            claim_issuance_binding_id,
            general_founding_capability_id,
            product_preauthorization_id,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Complete owner-net candidate-cost MarketBinding body.
    pub const fn base(&self) -> &MarketBindingV2 {
        &self.base
    }

    /// Exact Product MarketLifecycleRoot account.
    pub const fn product_market_root_account(&self) -> Id32 {
        self.product_market_root_account
    }

    /// Semantic identity of Product's immutable MarketLifecycle binding.
    pub const fn product_market_binding_id(&self) -> Id32 {
        self.product_market_binding_id
    }

    /// Product Market generation shared with Series and Failure.
    pub const fn product_generation(&self) -> u64 {
        self.product_generation
    }

    /// Exact founder SeriesMarketLink account.
    pub const fn series_market_link_account(&self) -> Id32 {
        self.series_market_link_account
    }

    /// Zero-based Series ordinal authenticated by the Product link.
    pub const fn series_ordinal(&self) -> u32 {
        self.series_ordinal
    }

    /// Complete current Product compiler bundle identity.
    pub const fn compiler_bundle_v5_id(&self) -> Id32 {
        self.compiler_bundle_v5_id
    }

    /// Current Series attachment-plan identity.
    pub const fn attachment_plan_v4_id(&self) -> Id32 {
        self.attachment_plan_v4_id
    }

    /// Collateral-owned exact liability-founding transcript identity.
    pub const fn market_liability_founding_id(&self) -> Id32 {
        self.market_liability_founding_id
    }

    /// Exact phased claim-mint founding plan identity.
    pub const fn claim_mint_founding_plan_id(&self) -> Id32 {
        self.claim_mint_founding_plan_id
    }

    /// Exact Profile-selected claim-issuance release binding.
    pub const fn claim_issuance_binding_id(&self) -> Id32 {
        self.claim_issuance_binding_id
    }

    /// Product-authenticated General founding capability identity.
    pub const fn general_founding_capability_id(&self) -> Id32 {
        self.general_founding_capability_id
    }

    /// One-way Product preauthorization persisted before Product admits General.
    pub const fn product_preauthorization_id(&self) -> Id32 {
        self.product_preauthorization_id
    }

    /// Sole rent principal owner for this immutable account.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Validate the full V2 body, Product identities, generation, and rent.
    pub fn validate(&self) -> Result<(), CodecError> {
        self.base.validate()?;
        self.rent.validate()?;
        if self.product_generation == 0 {
            return Err(CodecError::InvalidState);
        }
        let product_ids = [
            self.product_market_root_account,
            self.product_market_binding_id,
            self.series_market_link_account,
            self.compiler_bundle_v5_id,
            self.attachment_plan_v4_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.general_founding_capability_id,
            self.product_preauthorization_id,
        ];
        for (index, id) in product_ids.iter().enumerate() {
            if id.is_zero()
                || product_ids[index + 1..].iter().any(|other| other == id)
            {
                return Err(CodecError::MismatchedBinding);
            }
        }
        let relation = self.base.base();
        let inherited_ids = [
            relation.market,
            relation.market_genesis_profile_v2_id,
            relation.market_instance_v2_id,
            relation.series_plan_v5_id,
            relation.series_funding_terms_v2_id,
            relation.relation_policy_id,
            relation.price_measure_policy_v1_id,
            relation.native_claim_basis_id,
            relation.admission_policy_id,
            relation.score_policy_id,
            relation.settlement_policy_id,
            relation.neutral_sink,
            self.base.batch_policy_id(),
        ];
        if product_ids
            .iter()
            .any(|id| inherited_ids.iter().any(|other| other == id))
            || inherited_ids
                .iter()
                .any(|id| self.rent.payer == *id)
            || product_ids.iter().any(|id| self.rent.payer == *id)
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exact tag `0x79`, version `3`, and 952 canonical bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != MARKET_BINDING_ACCOUNT_BYTES_V3 {
            return Err(CodecError::WrongLength);
        }
        self.base
            .encode(&mut output[..MARKET_BINDING_ACCOUNT_BYTES_V2])?;
        output[1] = MARKET_BINDING_ACCOUNT_VERSION_V3;
        put_id(
            output,
            PRODUCT_ROOT_ACCOUNT_OFFSET,
            self.product_market_root_account,
        );
        put_id(output, PRODUCT_BINDING_ID_OFFSET, self.product_market_binding_id);
        output[PRODUCT_GENERATION_OFFSET..PRODUCT_GENERATION_OFFSET + 8]
            .copy_from_slice(&self.product_generation.to_le_bytes());
        put_id(
            output,
            SERIES_LINK_ACCOUNT_OFFSET,
            self.series_market_link_account,
        );
        output[SERIES_ORDINAL_OFFSET..SERIES_ORDINAL_OFFSET + 4]
            .copy_from_slice(&self.series_ordinal.to_le_bytes());
        put_id(output, COMPILER_BUNDLE_ID_OFFSET, self.compiler_bundle_v5_id);
        put_id(output, ATTACHMENT_PLAN_ID_OFFSET, self.attachment_plan_v4_id);
        put_id(
            output,
            MARKET_LIABILITY_FOUNDING_ID_OFFSET,
            self.market_liability_founding_id,
        );
        put_id(
            output,
            CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET,
            self.claim_mint_founding_plan_id,
        );
        put_id(
            output,
            CLAIM_ISSUANCE_BINDING_ID_OFFSET,
            self.claim_issuance_binding_id,
        );
        put_id(
            output,
            GENERAL_FOUNDING_CAPABILITY_ID_OFFSET,
            self.general_founding_capability_id,
        );
        put_id(
            output,
            PRODUCT_PREAUTHORIZATION_ID_OFFSET,
            self.product_preauthorization_id,
        );
        put_id(output, RENT_OFFSET, self.rent.payer);
        output[RENT_OFFSET + 32..RENT_OFFSET + 40]
            .copy_from_slice(&self.rent.refundable_principal.to_le_bytes());
        output[RENT_OFFSET + 40..RENT_OFFSET + 48]
            .copy_from_slice(&self.rent.donation_floor.to_le_bytes());
        Ok(())
    }

    /// Decode only exact V3 bytes; V1/V2 cannot alias this successor.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != MARKET_BINDING_ACCOUNT_BYTES_V3 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != MARKET_BINDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != MARKET_BINDING_ACCOUNT_VERSION_V3 {
            return Err(CodecError::WrongVersion);
        }
        let mut prefix = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        prefix.copy_from_slice(&input[..MARKET_BINDING_ACCOUNT_BYTES_V2]);
        prefix[1] = MARKET_BINDING_ACCOUNT_VERSION_V2;
        let base = MarketBindingV2::decode(&prefix)?;
        let product_generation = u64::from_le_bytes(
            input[PRODUCT_GENERATION_OFFSET..PRODUCT_GENERATION_OFFSET + 8]
                .try_into()
                .map_err(|_| CodecError::WrongLength)?,
        );
        let series_ordinal = u32::from_le_bytes(
            input[SERIES_ORDINAL_OFFSET..SERIES_ORDINAL_OFFSET + 4]
                .try_into()
                .map_err(|_| CodecError::WrongLength)?,
        );
        let rent = DeletableRentOwnerV1 {
            payer: read_id(input, RENT_OFFSET)?,
            refundable_principal: u64::from_le_bytes(
                input[RENT_OFFSET + 32..RENT_OFFSET + 40]
                    .try_into()
                    .map_err(|_| CodecError::WrongLength)?,
            ),
            donation_floor: u64::from_le_bytes(
                input[RENT_OFFSET + 40..RENT_OFFSET + 48]
                    .try_into()
                    .map_err(|_| CodecError::WrongLength)?,
            ),
        };
        Self::new(
            base,
            read_id(input, PRODUCT_ROOT_ACCOUNT_OFFSET)?,
            read_id(input, PRODUCT_BINDING_ID_OFFSET)?,
            product_generation,
            read_id(input, SERIES_LINK_ACCOUNT_OFFSET)?,
            series_ordinal,
            read_id(input, COMPILER_BUNDLE_ID_OFFSET)?,
            read_id(input, ATTACHMENT_PLAN_ID_OFFSET)?,
            read_id(input, MARKET_LIABILITY_FOUNDING_ID_OFFSET)?,
            read_id(input, CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET)?,
            read_id(input, CLAIM_ISSUANCE_BINDING_ID_OFFSET)?,
            read_id(input, GENERAL_FOUNDING_CAPABILITY_ID_OFFSET)?,
            read_id(input, PRODUCT_PREAUTHORIZATION_ID_OFFSET)?,
            rent,
        )
    }
}

fn put_id(output: &mut [u8], offset: usize, id: Id32) {
    output[offset..offset + 32].copy_from_slice(&id.bytes());
}

fn read_id(input: &[u8], offset: usize) -> Result<Id32, CodecError> {
    Id32::new(
        input[offset..offset + 32]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?,
    )
}

const _: () = assert!(RENT_OFFSET + 48 == MARKET_BINDING_ACCOUNT_BYTES_V3);
const _: () = assert!(MARKET_BINDING_ACCOUNT_BYTES_V3 == 952);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarketBindingV1, SCORE_V2_Q_COST_ACTIVE_RANK_BYTES};

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn value() -> MarketBindingV3 {
        let base = MarketBindingV2::new(
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
        .unwrap();
        MarketBindingV3::new(
            base,
            id(15),
            id(16),
            17,
            id(18),
            0,
            id(19),
            id(20),
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            DeletableRentOwnerV1 {
                payer: id(26),
                refundable_principal: 952,
                donation_floor: 27,
            },
        )
        .unwrap()
    }

    #[test]
    fn exact_round_trip_preserves_product_authority_and_rent() {
        let value = value();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V3];
        value.encode(&mut bytes).unwrap();
        assert_eq!(&bytes[..2], &[MARKET_BINDING_ACCOUNT_TAG, 3]);
        assert_eq!(MarketBindingV3::decode(&bytes), Ok(value));
        assert_eq!(value.base().base().series_plan_v5_id, id(4));
        assert_eq!(value.series_ordinal(), 0);
    }

    #[test]
    fn hostile_version_alias_and_product_identity_alias_refuse() {
        let value = value();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V3];
        value.encode(&mut bytes).unwrap();
        bytes[1] = MARKET_BINDING_ACCOUNT_VERSION_V2;
        assert_eq!(MarketBindingV3::decode(&bytes), Err(CodecError::WrongVersion));
        assert_eq!(
            MarketBindingV3::decode(&bytes[..951]),
            Err(CodecError::WrongLength)
        );
        assert_eq!(
            MarketBindingV3::new(
                *value.base(),
                value.product_market_root_account(),
                value.product_market_root_account(),
                value.product_generation(),
                value.series_market_link_account(),
                value.series_ordinal(),
                value.compiler_bundle_v5_id(),
                value.attachment_plan_v4_id(),
                value.market_liability_founding_id(),
                value.claim_mint_founding_plan_id(),
                value.claim_issuance_binding_id(),
                value.general_founding_capability_id(),
                value.product_preauthorization_id(),
                value.rent(),
            ),
            Err(CodecError::MismatchedBinding),
        );
    }

    #[test]
    fn hostile_absent_preauthorization_and_v2_decoder_alias_refuse() {
        let value = value();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V3];
        value.encode(&mut bytes).unwrap();
        bytes[PRODUCT_PREAUTHORIZATION_ID_OFFSET..PRODUCT_PREAUTHORIZATION_ID_OFFSET + 32]
            .fill(0);
        assert_eq!(MarketBindingV3::decode(&bytes), Err(CodecError::ZeroIdentity));

        value.encode(&mut bytes).unwrap();
        assert_eq!(MarketBindingV2::decode(&bytes), Err(CodecError::WrongLength));
        bytes[0] ^= 1;
        assert_eq!(MarketBindingV3::decode(&bytes), Err(CodecError::WrongTag));
    }
}
