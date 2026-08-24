// SPDX-License-Identifier: AGPL-3.0-or-later

//! RevenuePolicyV2-native Direct fee authority.
//!
//! This projection is distinct from the historical V1 fee DTO. It binds the
//! exact Realm revenue record and treasury-Position derivation policy in
//! addition to the immutable policy digest and arithmetic words. Quote and
//! bilateral allocation arithmetic remain shared with `fee_v1`; no second
//! fee formula exists here.

use clutch_batch::relation_v1::{FeeBaseV1, FrozenPolicyV1, FEE_BPS_DENOMINATOR};
use clutch_batch::relation_v2::PricePreconditionV2;
use clutch_batch_policy_identity::batch_policy_digest;
use clutch_batch_policy_identity::revenue_policy_v2::{
    revenue_policy_record_v2_id, revenue_policy_v2_digest,
    treasury_position_derivation_policy_v2_id, LamportSinkV2,
    MakerWeightAuthorityV2, RevenuePolicyV2, RevenueResidualV2,
    TreasuryPositionDerivationPolicyV2,
};

use crate::fee_v1::{
    allocate_bilateral_terminal_fee_core, maximum_buyer_fee_atoms_core,
    terminal_buyer_fee_atoms_core, DirectFeeTerminalV1,
};
use crate::{require_live, DirectHashBackendV1, DirectMarketErrorV1};

const DIRECT_FEE_POLICY_PROJECTION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/fee-policy-projection/v2\0";

/// Exact Direct projection of the current RevenuePolicyV2 semantic owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeePolicyV2 {
    /// Canonical complete batch-policy digest.
    pub batch_policy_id: [u8; 32],
    /// Canonical RevenuePolicyV2 digest.
    pub revenue_policy_v2_digest: [u8; 32],
    /// Realm-bound RevenuePolicyRecordV2 semantic identity.
    pub revenue_policy_record_v2_id: [u8; 32],
    /// Immutable ordinary treasury Position owner.
    pub treasury_owner: [u8; 32],
    /// Exact current treasury Position/Replay/service-ledger derivation policy.
    pub treasury_position_derivation_policy_v2_id: [u8; 32],
    /// Composite dispersion rate numerator over 10,000.
    pub dispersion_bps: u32,
    /// Composite range-floor rate numerator over 10,000.
    pub floor_range_bps: u32,
    /// Certified-maker share numerator.
    pub maker_rebate_num: u32,
    /// Treasury share numerator; executor share is structurally zero.
    pub treasury_num: u32,
    /// Exact split denominator.
    pub split_den: u32,
}

impl DirectFeePolicyV2 {
    /// Project only from the complete batch and RevenuePolicyV2 preimages.
    pub fn from_policies(
        realm: [u8; 32],
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV2,
    ) -> Result<Self, DirectMarketErrorV1> {
        require_live(realm)?;
        batch
            .validate()
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        revenue
            .validate()
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        let (dispersion_bps, floor_range_bps) = match batch.fee_base {
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } if dispersion_bps == revenue.dispersion_bps
                && floor_range_bps == revenue.floor_range_bps =>
            {
                (dispersion_bps, floor_range_bps)
            }
            _ => return Err(DirectMarketErrorV1::MismatchedBinding),
        };
        let value = Self {
            batch_policy_id: batch_policy_digest(batch)
                .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
                .0,
            revenue_policy_v2_digest: revenue_policy_v2_digest(revenue)
                .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
                .0,
            revenue_policy_record_v2_id: revenue_policy_record_v2_id(realm, revenue)
                .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
                .0,
            treasury_owner: revenue.treasury_owner,
            treasury_position_derivation_policy_v2_id:
                treasury_position_derivation_policy_v2_id(
                    revenue.treasury_position_derivation,
                )
                .0,
            dispersion_bps,
            floor_range_bps,
            maker_rebate_num: revenue.maker_rebate_num,
            treasury_num: revenue.treasury_num,
            split_den: revenue.split_den,
        };
        value.validate()?;
        Ok(value)
    }

    /// Refuse incomplete identities or a shape not expressible by V2.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        for id in [
            self.batch_policy_id,
            self.revenue_policy_v2_digest,
            self.revenue_policy_record_v2_id,
            self.treasury_owner,
            self.treasury_position_derivation_policy_v2_id,
        ] {
            require_live(id)?;
        }
        if self.dispersion_bps == 0 && self.floor_range_bps == 0 {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        if self.split_den == 0
            || u64::from(self.dispersion_bps) > FEE_BPS_DENOMINATOR
            || u64::from(self.floor_range_bps) > FEE_BPS_DENOMINATOR
            || self
                .maker_rebate_num
                .checked_add(self.treasury_num)
                != Some(self.split_den)
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Reauthenticate all copied words and both policy identities.
    pub fn binds_policies(
        self,
        realm: [u8; 32],
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV2,
    ) -> Result<(), DirectMarketErrorV1> {
        if self != Self::from_policies(realm, batch, revenue)?
            || revenue.residual != RevenueResidualV2::Treasury
            || revenue.maker_weight_authority
                != MakerWeightAuthorityV2::CertifiedOwnerNettedCompositeNumerator
            || revenue.lamport_sink != LamportSinkV2::None
            || revenue.treasury_position_derivation
                != TreasuryPositionDerivationPolicyV2::PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical identity committed by b1/v2 and its reservation descendants.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let id = backend.sha256_parts(&[
            DIRECT_FEE_POLICY_PROJECTION_DOMAIN_V2,
            &self.batch_policy_id,
            &self.revenue_policy_v2_digest,
            &self.revenue_policy_record_v2_id,
            &self.treasury_owner,
            &self.treasury_position_derivation_policy_v2_id,
            &self.dispersion_bps.to_le_bytes(),
            &self.floor_range_bps.to_le_bytes(),
            &self.maker_rebate_num.to_le_bytes(),
            &self.treasury_num.to_le_bytes(),
            &self.split_den.to_le_bytes(),
        ]);
        require_live(id)?;
        Ok(id)
    }

    /// Exact worst-case terminal-ceil buyer fee at this current policy.
    pub fn maximum_buyer_fee_atoms(
        self,
        quantity: u64,
        outcome_count: u8,
        price_scale: u64,
    ) -> Result<u64, DirectMarketErrorV1> {
        self.validate()?;
        maximum_buyer_fee_atoms_core(
            quantity,
            outcome_count,
            price_scale,
            self.dispersion_bps,
            self.floor_range_bps,
        )
    }

    /// Assess one complete Direct pair with equal certified bilateral
    /// composite-numerator weights and one Position-sorted Hamilton remainder.
    #[allow(clippy::too_many_arguments)]
    pub fn assess_terminal_buyer(
        self,
        quantity: u64,
        outcome: u8,
        outcome_count: u8,
        price_scale: u64,
        price: &PricePreconditionV2,
        buyer_position: [u8; 32],
        seller_position: [u8; 32],
        maximum_fee_atoms: u64,
        realm: [u8; 32],
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV2,
    ) -> Result<DirectFeeTerminalV1, DirectMarketErrorV1> {
        self.binds_policies(realm, batch, revenue)?;
        let charged = terminal_buyer_fee_atoms_core(
            quantity,
            outcome,
            outcome_count,
            price_scale,
            price,
            self.dispersion_bps,
            self.floor_range_bps,
        )?;
        let split = revenue
            .allocate_split(charged)
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        if split.executor_atoms != 0 {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        allocate_bilateral_terminal_fee_core(
            charged,
            maximum_fee_atoms,
            split.maker_rebate_atoms,
            split.treasury_atoms,
            buyer_position,
            seller_position,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::FeeBaseV1;
    use clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_FEE_SHAPE_V1;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn policies() -> (FrozenPolicyV1, RevenuePolicyV2) {
        let revenue = RevenuePolicyV2::successor_development(id(9));
        let batch = FrozenPolicyV1 {
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: revenue.dispersion_bps,
                floor_range_bps: revenue.floor_range_bps,
            },
            ..GENERAL_CLEARING_FEE_SHAPE_V1
        };
        (batch, revenue)
    }

    #[test]
    fn v2_projection_rebinds_realm_record_and_derivation_policy() {
        let (batch, revenue) = policies();
        let projected = DirectFeePolicyV2::from_policies(id(1), &batch, &revenue).unwrap();
        assert_eq!(projected.binds_policies(id(1), &batch, &revenue), Ok(()));
        assert_ne!(
            projected,
            DirectFeePolicyV2::from_policies(id(2), &batch, &revenue).unwrap(),
        );
        let hostile = RevenuePolicyV2 {
            maker_rebate_num: 59,
            treasury_num: 41,
            ..revenue
        };
        assert_eq!(
            projected.binds_policies(id(1), &batch, &hostile),
            Err(DirectMarketErrorV1::MismatchedBinding),
        );
    }

    #[test]
    fn v2_terminal_uses_one_ceil_and_position_sorted_hamilton_dust() {
        let (batch, revenue) = policies();
        let projected = DirectFeePolicyV2::from_policies(id(1), &batch, &revenue).unwrap();
        let price = PricePreconditionV2 {
            policy_digest: id(2),
            semantic_price_digest: id(3),
            prices: {
                let mut value = [0; clutch_batch::relation_v1::MAX_OUTCOMES];
                value[0] = 5_000;
                value[1] = 5_000;
                value
            },
        };
        let maximum = projected.maximum_buyer_fee_atoms(10_000, 2, 10_000).unwrap();
        let terminal = projected
            .assess_terminal_buyer(
                10_000,
                0,
                2,
                10_000,
                &price,
                id(4),
                id(5),
                maximum,
                id(1),
                &batch,
                &revenue,
            )
            .unwrap();
        assert_eq!(
            terminal
                .buyer_rebate_atoms
                .checked_add(terminal.seller_rebate_atoms)
                .and_then(|value| value.checked_add(terminal.treasury_atoms)),
            Some(terminal.charged_fee_atoms),
        );
        assert!(terminal.buyer_rebate_atoms >= terminal.seller_rebate_atoms);
        assert_eq!(terminal.boundary, crate::fee_v1::DirectFeeBoundaryV1::TerminalCeil);
    }
}
