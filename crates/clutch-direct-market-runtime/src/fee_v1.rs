// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact Direct projection of the canonical batch and revenue fee owners.
//!
//! This module does not choose rates or a treasury. It accepts only a complete
//! authenticated `FrozenPolicyV1`/`RevenuePolicyV1` pair, copies the minimum
//! coordinates needed by the Direct root, and rebinds both source digests on
//! every charge. Direct has one terminal owner-fee boundary: the buyer's
//! selected scalar payoff is assessed with the composite fee's terminal ceil.
//! The seller has no cash reservation and is never charged.

use clutch_batch::relation_v1::{
    composite_fee_quote, FeeBaseV1, FrozenPolicyV1, FEE_BPS_DENOMINATOR, MAX_OUTCOMES,
};
use clutch_batch::relation_v2::PricePreconditionV2;
use clutch_batch_policy_identity::revenue_policy_v1::{
    revenue_policy_digest, treasury_admits_fee_bearing, LamportSinkV1, RevenuePolicyV1,
    RevenueResidualV1, StandingMakerV1,
};
use clutch_batch_policy_identity::batch_policy_digest;

use crate::{require_live, DirectHashBackendV1, DirectMarketErrorV1};

const DIRECT_FEE_POLICY_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/fee-policy-projection/v1\0";

/// Exact Direct projection of externally owned fee-policy facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeePolicyV1 {
    /// Canonical complete batch-policy digest.
    pub batch_policy_id: [u8; 32],
    /// Canonical complete revenue-policy digest.
    pub revenue_policy_id: [u8; 32],
    /// Revenue recipient, possibly the structural unset sentinel at zero fee.
    pub treasury_owner: [u8; 32],
    /// Composite dispersion rate numerator over the canonical basis-point denominator.
    pub dispersion_bps: u32,
    /// Composite quotient-range rate numerator over the same denominator.
    pub floor_range_bps: u32,
    /// Standing-maker share numerator.
    pub maker_rebate_num: u32,
    /// Treasury share numerator; executor share is required to be zero.
    pub treasury_num: u32,
    /// Exact revenue split denominator.
    pub split_den: u32,
}

impl DirectFeePolicyV1 {
    /// Project a complete authenticated policy pair without selecting any fact.
    pub fn from_policies(
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV1,
    ) -> Result<Self, DirectMarketErrorV1> {
        batch
            .validate()
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        revenue
            .validate()
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        let (dispersion_bps, floor_range_bps) = match batch.fee_base {
            FeeBaseV1::None => (0, 0),
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } => (dispersion_bps, floor_range_bps),
            FeeBaseV1::FlatNotional { .. } => {
                return Err(DirectMarketErrorV1::MismatchedBinding)
            }
        };
        if revenue.executor_num != 0
            || revenue.residual != RevenueResidualV1::Treasury
            || revenue.standing_maker != StandingMakerV1::AllRestingMakers
            || revenue.lamport_sink != LamportSinkV1::None
            || ((dispersion_bps != 0 || floor_range_bps != 0)
                && !treasury_admits_fee_bearing(&revenue.treasury))
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        let value = Self {
            batch_policy_id: batch_policy_digest(batch)
                .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
                .0,
            revenue_policy_id: revenue_policy_digest(revenue)
                .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
                .0,
            treasury_owner: revenue.treasury,
            dispersion_bps,
            floor_range_bps,
            maker_rebate_num: revenue.maker_rebate_num,
            treasury_num: revenue.treasury_num,
            split_den: revenue.split_den,
        };
        value.validate()?;
        Ok(value)
    }

    /// Refuse incomplete rate, split, digest, and treasury geometry.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        require_live(self.batch_policy_id)?;
        require_live(self.revenue_policy_id)?;
        if self.split_den == 0
            || u64::from(self.dispersion_bps) > FEE_BPS_DENOMINATOR
            || u64::from(self.floor_range_bps) > FEE_BPS_DENOMINATOR
            || self
                .maker_rebate_num
                .checked_add(self.treasury_num)
                != Some(self.split_den)
            || ((self.dispersion_bps != 0 || self.floor_range_bps != 0)
                && !treasury_admits_fee_bearing(&self.treasury_owner))
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Whether either authenticated composite rate is nonzero.
    pub const fn fee_bearing(self) -> bool {
        self.dispersion_bps != 0 || self.floor_range_bps != 0
    }

    /// Rebind the complete source policies instead of trusting copied words.
    pub fn binds_policies(
        self,
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV1,
    ) -> Result<(), DirectMarketErrorV1> {
        if self != Self::from_policies(batch, revenue)? {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical identity committed by the Direct root and Reservation rows.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let id = backend.sha256_parts(&[
            DIRECT_FEE_POLICY_PROJECTION_DOMAIN_V1,
            &self.batch_policy_id,
            &self.revenue_policy_id,
            &self.treasury_owner,
            &self.dispersion_bps.to_le_bytes(),
            &self.floor_range_bps.to_le_bytes(),
            &self.maker_rebate_num.to_le_bytes(),
            &self.treasury_num.to_le_bytes(),
            &self.split_den.to_le_bytes(),
        ]);
        require_live(id)?;
        Ok(id)
    }

    /// Exact maximum terminal-ceil buyer fee over every integer simplex price.
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

    /// Assess and split one complete selected Direct buyer payoff.
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
        revenue: &RevenuePolicyV1,
    ) -> Result<DirectFeeTerminalV1, DirectMarketErrorV1> {
        self.validate()?;
        require_live(buyer_position)?;
        require_live(seller_position)?;
        if quantity == 0
            || usize::from(outcome) >= usize::from(outcome_count)
            || !(2..=MAX_OUTCOMES).contains(&usize::from(outcome_count))
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        revenue
            .validate()
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        let revenue_id = revenue_policy_digest(revenue)
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?
            .0;
        if revenue_id != self.revenue_policy_id
            || revenue.treasury != self.treasury_owner
            || revenue.maker_rebate_num != self.maker_rebate_num
            || revenue.treasury_num != self.treasury_num
            || revenue.split_den != self.split_den
            || revenue.executor_num != 0
            || revenue.residual != RevenueResidualV1::Treasury
            || revenue.standing_maker != StandingMakerV1::AllRestingMakers
            || revenue.lamport_sink != LamportSinkV1::None
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        let charged_fee_atoms = terminal_buyer_fee_atoms_core(
            quantity,
            outcome,
            outcome_count,
            price_scale,
            price,
            self.dispersion_bps,
            self.floor_range_bps,
        )?;
        let split = revenue
            .allocate_split(charged_fee_atoms)
            .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
        if split.executor_atoms != 0 {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        allocate_bilateral_terminal_fee_core(
            charged_fee_atoms,
            maximum_fee_atoms,
            split.maker_rebate_atoms,
            split.treasury_atoms,
            buyer_position,
            seller_position,
        )
    }
}

/// Shared exact maximum-fee arithmetic for historical and current policy
/// projections. Policy authentication remains owned by the caller.
pub(crate) fn maximum_buyer_fee_atoms_core(
    quantity: u64,
    outcome_count: u8,
    price_scale: u64,
    dispersion_bps: u32,
    floor_range_bps: u32,
) -> Result<u64, DirectMarketErrorV1> {
    if quantity == 0
        || !(2..=MAX_OUTCOMES).contains(&usize::from(outcome_count))
        || price_scale == 0
    {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    if dispersion_bps == 0 && floor_range_bps == 0 {
        return Ok(0);
    }
    let mut payoffs = [0u64; MAX_OUTCOMES];
    payoffs[0] = quantity;
    let mut prices = [0u64; MAX_OUTCOMES];
    let left = price_scale / 2;
    prices[0] = left;
    prices[1] = price_scale
        .checked_sub(left)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let quote = composite_fee_quote(
        &payoffs,
        &prices,
        usize::from(outcome_count),
        price_scale,
        dispersion_bps,
        floor_range_bps,
        0,
    )
    .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
    u64::try_from(quote.terminal_ceil_atoms).map_err(|_| DirectMarketErrorV1::Arithmetic)
}

/// Shared one-shot composite-fee quote at the named terminal-ceil boundary.
pub(crate) fn terminal_buyer_fee_atoms_core(
    quantity: u64,
    outcome: u8,
    outcome_count: u8,
    price_scale: u64,
    price: &PricePreconditionV2,
    dispersion_bps: u32,
    floor_range_bps: u32,
) -> Result<u64, DirectMarketErrorV1> {
    if quantity == 0
        || usize::from(outcome) >= usize::from(outcome_count)
        || !(2..=MAX_OUTCOMES).contains(&usize::from(outcome_count))
        || price_scale == 0
    {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut payoffs = [0u64; MAX_OUTCOMES];
    payoffs[usize::from(outcome)] = quantity;
    let quote = composite_fee_quote(
        &payoffs,
        &price.prices,
        usize::from(outcome_count),
        price_scale,
        dispersion_bps,
        floor_range_bps,
        0,
    )
    .map_err(|_| DirectMarketErrorV1::MismatchedBinding)?;
    u64::try_from(quote.terminal_ceil_atoms).map_err(|_| DirectMarketErrorV1::Arithmetic)
}

/// Allocate the maker pool by equal authenticated bilateral composite
/// numerators and apply the sole Hamilton remainder at Position ordering.
/// The Direct pair proves equal quantity, outcome, and price for the two rows,
/// so the two owner-netted numerator weights are equal and nonzero.
pub(crate) fn allocate_bilateral_terminal_fee_core(
    charged_fee_atoms: u64,
    maximum_fee_atoms: u64,
    maker_rebate_atoms: u64,
    treasury_atoms: u64,
    buyer_position: [u8; 32],
    seller_position: [u8; 32],
) -> Result<DirectFeeTerminalV1, DirectMarketErrorV1> {
    require_live(buyer_position)?;
    require_live(seller_position)?;
    if buyer_position == seller_position || charged_fee_atoms > maximum_fee_atoms {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let buyer_floor = maker_rebate_atoms / 2;
    let seller_floor = maker_rebate_atoms / 2;
    let dust = maker_rebate_atoms
        .checked_sub(
            buyer_floor
                .checked_add(seller_floor)
                .ok_or(DirectMarketErrorV1::Arithmetic)?,
        )
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let (buyer_rebate_atoms, seller_rebate_atoms) = if dust == 0 {
        (buyer_floor, seller_floor)
    } else if buyer_position < seller_position {
        (
            buyer_floor
                .checked_add(dust)
                .ok_or(DirectMarketErrorV1::Arithmetic)?,
            seller_floor,
        )
    } else {
        (
            buyer_floor,
            seller_floor
                .checked_add(dust)
                .ok_or(DirectMarketErrorV1::Arithmetic)?,
        )
    };
    let distributed = buyer_rebate_atoms
        .checked_add(seller_rebate_atoms)
        .and_then(|value| value.checked_add(treasury_atoms))
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    if distributed != charged_fee_atoms {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(DirectFeeTerminalV1 {
        charged_fee_atoms,
        buyer_rebate_atoms,
        seller_rebate_atoms,
        treasury_atoms,
        refunded_headroom_atoms: maximum_fee_atoms
            .checked_sub(charged_fee_atoms)
            .ok_or(DirectMarketErrorV1::Arithmetic)?,
        boundary: DirectFeeBoundaryV1::TerminalCeil,
    })
}

/// The only Direct fee-rounding boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectFeeBoundaryV1 {
    /// Assess the complete buyer payoff once and round the exact rational up.
    TerminalCeil,
}

impl DirectFeeBoundaryV1 {
    /// Stable transcript byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::TerminalCeil => 1,
        }
    }
}

/// Exact charge, recipient split, and unused signed-envelope refund.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeTerminalV1 {
    /// Buyer cash atoms charged at the one named boundary.
    pub charged_fee_atoms: u64,
    /// Buyer-position share of the all-resting-maker pool.
    pub buyer_rebate_atoms: u64,
    /// Seller-position share of the all-resting-maker pool.
    pub seller_rebate_atoms: u64,
    /// Exact treasury-position credit.
    pub treasury_atoms: u64,
    /// Reserved buyer headroom released at terminalization.
    pub refunded_headroom_atoms: u64,
    /// One exhaustive rounding boundary.
    pub boundary: DirectFeeBoundaryV1,
}

impl DirectFeeTerminalV1 {
    /// Canonical fixed transcript committed by Direct transition receipts.
    pub fn canonical_transcript(self) -> [u8; 41] {
        let mut output = [0u8; 41];
        output[0..8].copy_from_slice(&self.charged_fee_atoms.to_le_bytes());
        output[8..16].copy_from_slice(&self.buyer_rebate_atoms.to_le_bytes());
        output[16..24].copy_from_slice(&self.seller_rebate_atoms.to_le_bytes());
        output[24..32].copy_from_slice(&self.treasury_atoms.to_le_bytes());
        output[32..40].copy_from_slice(&self.refunded_headroom_atoms.to_le_bytes());
        output[40] = self.boundary.byte();
        output
    }
}
