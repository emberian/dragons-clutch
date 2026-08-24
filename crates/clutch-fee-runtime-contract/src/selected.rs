//! Immutable selection and exact owner-level assessment.

use clutch_batch::relation_v1::{
    composite_fee_quote, FeeBaseV1, FeeQuoteV1, FrozenPolicyV1, MAX_OUTCOMES,
};
use clutch_batch_policy_identity::revenue_policy_v1::{
    revenue_policy_digest, treasury_admits_fee_bearing, RevenuePolicyV1,
};
use clutch_batch_policy_identity::revenue_policy_v2::{
    revenue_policy_v2_digest, RevenuePolicyV2,
};
use clutch_batch_policy_identity::{batch_policy_digest, Identity32V1};

use crate::{add, independent, live, Error, Id, Result};

mod sealed {
    pub trait SelectedCompositeFeeSealed {}
}

/// Read-only access to one constructor-authenticated selected fee transcript.
/// The private sealing trait prevents caller-defined implementations from
/// entering shared carry and lifecycle transitions.
pub trait SelectedCompositeFeeAccess: sealed::SelectedCompositeFeeSealed {
    fn fee_record(&self) -> Id;
    fn realm(&self) -> Id;
    fn market(&self) -> Id;
    fn epoch(&self) -> Id;
    fn selected_candidate(&self) -> Id;
    fn batch_policy(&self) -> Id;
    fn revenue_policy(&self) -> Id;
    fn treasury_owner(&self) -> Id;
    fn treasury_position(&self) -> Id;
    fn price_scale(&self) -> u64;
    fn outcome_count(&self) -> u8;
    fn dispersion_bps(&self) -> u32;
    fn floor_range_bps(&self) -> u32;
    fn carry_denominator(&self) -> u128;
}

/// The single named rounding event for one owner-fee transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssessmentBoundaryV1 {
    /// An intermediate fragment pays the exact floor and persists its carry.
    FragmentFloor,
    /// The terminal owner event pays the exact ceiling and closes carry zero.
    TerminalCeil,
}

impl AssessmentBoundaryV1 {
    /// Canonical fixed-layout discriminant.
    pub const fn byte(self) -> u8 {
        match self {
            Self::FragmentFloor => 0,
            Self::TerminalCeil => 1,
        }
    }
}

/// Runtime-sized result of an exact `u128` composite quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAssessmentV1 {
    owner: Id,
    fee_record: Id,
    charged_atoms: u64,
    next_carry: u128,
    denominator: u128,
    boundary: AssessmentBoundaryV1,
}

impl OwnerFeeAssessmentV1 {
    pub const fn owner(&self) -> Id {
        self.owner
    }

    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn charged_atoms(&self) -> u64 {
        self.charged_atoms
    }

    pub const fn next_carry(&self) -> u128 {
        self.next_carry
    }

    pub const fn denominator(&self) -> u128 {
        self.denominator
    }

    pub const fn boundary(&self) -> AssessmentBoundaryV1 {
        self.boundary
    }
}

/// One immutable, nonzero composite-fee selection for a selected candidate.
///
/// This is an account-neutral semantic record.  An adapter must put it behind
/// a versioned codec and canonical PDA before it can authorize value movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedCompositeFeeV1 {
    fee_record: Id,
    realm: Id,
    market: Id,
    epoch: Id,
    selected_candidate: Id,
    batch_policy: Id,
    revenue_policy: Id,
    treasury_owner: Id,
    treasury_position: Id,
    price_scale: u64,
    outcome_count: u8,
    dispersion_bps: u32,
    floor_range_bps: u32,
    carry_denominator: u128,
}

impl SelectedCompositeFeeV1 {
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn realm(&self) -> Id {
        self.realm
    }

    pub const fn market(&self) -> Id {
        self.market
    }

    pub const fn epoch(&self) -> Id {
        self.epoch
    }

    pub const fn selected_candidate(&self) -> Id {
        self.selected_candidate
    }

    pub const fn batch_policy(&self) -> Id {
        self.batch_policy
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn treasury_owner(&self) -> Id {
        self.treasury_owner
    }

    pub const fn treasury_position(&self) -> Id {
        self.treasury_position
    }

    pub const fn price_scale(&self) -> u64 {
        self.price_scale
    }

    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }

    pub const fn dispersion_bps(&self) -> u32 {
        self.dispersion_bps
    }

    pub const fn floor_range_bps(&self) -> u32 {
        self.floor_range_bps
    }

    pub const fn carry_denominator(&self) -> u128 {
        self.carry_denominator
    }

    /// Bind the exact rated batch policy, revenue policy, treasury, and
    /// selected-candidate domain.  At least one rate must be nonzero.
    #[allow(clippy::too_many_arguments)]
    pub fn select(
        fee_record: Id,
        realm: Id,
        market: Id,
        epoch: Id,
        selected_candidate: Id,
        treasury_position: Id,
        price_scale: u64,
        outcome_count: u8,
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV1,
    ) -> Result<Self> {
        for identity in [
            fee_record,
            realm,
            market,
            epoch,
            selected_candidate,
            treasury_position,
        ] {
            live(identity)?;
        }
        if outcome_count < 2
            || usize::from(outcome_count) > MAX_OUTCOMES
            || price_scale == 0
        {
            return Err(Error::InvalidWidth);
        }
        batch.validate().map_err(|_| Error::InvalidPolicy)?;
        revenue.validate().map_err(|_| Error::InvalidPolicy)?;
        // RevenuePolicyV1 authenticates only the treasury recipient. Its
        // executor member is deliberately deferred, so a nonzero executor
        // share has no identity to credit and cannot enter this contract.
        if revenue.executor_num != 0 {
            return Err(Error::UnauthenticatedRecipient);
        }
        let (dispersion_bps, floor_range_bps) = match batch.fee_base {
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } if dispersion_bps != 0 || floor_range_bps != 0 => (dispersion_bps, floor_range_bps),
            FeeBaseV1::CompositeDispersionFloor { .. } => return Err(Error::ZeroRate),
            FeeBaseV1::None | FeeBaseV1::FlatNotional { .. } => return Err(Error::InvalidPolicy),
        };
        if !treasury_admits_fee_bearing(&revenue.treasury) {
            return Err(Error::TreasuryUnavailable);
        }
        let treasury_owner = Identity32V1(revenue.treasury);
        live(treasury_owner)?;
        let batch_policy = batch_policy_digest(batch).map_err(|_| Error::InvalidPolicy)?;
        let revenue_policy = revenue_policy_digest(revenue).map_err(|_| Error::InvalidPolicy)?;
        independent(&[
            fee_record,
            realm,
            market,
            epoch,
            selected_candidate,
            batch_policy,
            revenue_policy,
            treasury_owner,
            treasury_position,
        ])?;

        // Ask the relation itself for its denominator.  The zero-payoff quote
        // is economically zero but still validates the exact simplex, rates,
        // scale, and checked denominator used by every real assessment.
        let payoffs = [0u64; MAX_OUTCOMES];
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = price_scale;
        let denominator = composite_fee_quote(
            &payoffs,
            &prices,
            usize::from(outcome_count),
            price_scale,
            dispersion_bps,
            floor_range_bps,
            0,
        )
        .map_err(|_| Error::InvalidPolicy)?
        .base_denominator;

        Ok(Self {
            fee_record,
            realm,
            market,
            epoch,
            selected_candidate,
            batch_policy,
            revenue_policy,
            treasury_owner,
            treasury_position,
            price_scale,
            outcome_count,
            dispersion_bps,
            floor_range_bps,
            carry_denominator: denominator,
        })
    }

    /// Rebind the revenue preimage instead of trusting the stored split words.
    pub fn binds_revenue_policy(&self, policy: &RevenuePolicyV1) -> Result<()> {
        policy.validate().map_err(|_| Error::InvalidPolicy)?;
        let digest = revenue_policy_digest(policy).map_err(|_| Error::InvalidPolicy)?;
        if digest != self.revenue_policy
            || Identity32V1(policy.treasury) != self.treasury_owner
            || !treasury_admits_fee_bearing(&policy.treasury)
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Quote one owner's whole filled buy-payoff vector.  Per-order quoting is
    /// deliberately unavailable because dispersion is subadditive.
    pub fn quote_owner(
        &self,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        prior_carry: u128,
    ) -> Result<FeeQuoteV1> {
        let quote = composite_fee_quote(
            payoffs,
            prices,
            usize::from(self.outcome_count),
            self.price_scale,
            self.dispersion_bps,
            self.floor_range_bps,
            prior_carry,
        )
        .map_err(|_| Error::InvalidPolicy)?;
        if quote.base_denominator != self.carry_denominator
            || quote.exact_denominator != self.carry_denominator
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(quote)
    }

    fn assess_owner(
        &self,
        owner: Id,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        prior_carry: u128,
        boundary: AssessmentBoundaryV1,
    ) -> Result<OwnerFeeAssessmentV1> {
        live(owner)?;
        let quote = self.quote_owner(payoffs, prices, prior_carry)?;
        let (charged, next_carry) = match boundary {
            AssessmentBoundaryV1::FragmentFloor => (quote.floor_atoms, quote.carry),
            AssessmentBoundaryV1::TerminalCeil => (quote.terminal_ceil_atoms, 0),
        };
        Ok(OwnerFeeAssessmentV1 {
            owner,
            fee_record: self.fee_record,
            charged_atoms: u64::try_from(charged).map_err(|_| Error::AmountOutOfRange)?,
            next_carry,
            denominator: self.carry_denominator,
            boundary,
        })
    }
}

impl sealed::SelectedCompositeFeeSealed for SelectedCompositeFeeV1 {}

impl SelectedCompositeFeeAccess for SelectedCompositeFeeV1 {
    fn fee_record(&self) -> Id { self.fee_record }
    fn realm(&self) -> Id { self.realm }
    fn market(&self) -> Id { self.market }
    fn epoch(&self) -> Id { self.epoch }
    fn selected_candidate(&self) -> Id { self.selected_candidate }
    fn batch_policy(&self) -> Id { self.batch_policy }
    fn revenue_policy(&self) -> Id { self.revenue_policy }
    fn treasury_owner(&self) -> Id { self.treasury_owner }
    fn treasury_position(&self) -> Id { self.treasury_position }
    fn price_scale(&self) -> u64 { self.price_scale }
    fn outcome_count(&self) -> u8 { self.outcome_count }
    fn dispersion_bps(&self) -> u32 { self.dispersion_bps }
    fn floor_range_bps(&self) -> u32 { self.floor_range_bps }
    fn carry_denominator(&self) -> u128 { self.carry_denominator }
}

/// Fresh selected composite-fee semantic transcript for RevenuePolicyV2.
/// It is intentionally a distinct type and codec from V1 even though the
/// fixed economic fields have the same widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedCompositeFeeV2 {
    fee_record: Id,
    realm: Id,
    market: Id,
    epoch: Id,
    settlement_candidate: Id,
    batch_policy: Id,
    revenue_policy: Id,
    treasury_owner: Id,
    treasury_position: Id,
    price_scale: u64,
    outcome_count: u8,
    dispersion_bps: u32,
    floor_range_bps: u32,
    carry_denominator: u128,
}

impl SelectedCompositeFeeV2 {
    pub const fn fee_record(&self) -> Id { self.fee_record }
    pub const fn realm(&self) -> Id { self.realm }
    pub const fn market(&self) -> Id { self.market }
    pub const fn epoch(&self) -> Id { self.epoch }
    pub const fn selected_candidate(&self) -> Id { self.settlement_candidate }
    pub const fn batch_policy(&self) -> Id { self.batch_policy }
    pub const fn revenue_policy(&self) -> Id { self.revenue_policy }
    pub const fn treasury_owner(&self) -> Id { self.treasury_owner }
    pub const fn treasury_position(&self) -> Id { self.treasury_position }
    pub const fn price_scale(&self) -> u64 { self.price_scale }
    pub const fn outcome_count(&self) -> u8 { self.outcome_count }
    pub const fn dispersion_bps(&self) -> u32 { self.dispersion_bps }
    pub const fn floor_range_bps(&self) -> u32 { self.floor_range_bps }
    pub const fn carry_denominator(&self) -> u128 { self.carry_denominator }

    /// Bind the exact rated batch policy to the independently Realm-founded
    /// RevenuePolicyV2 and its immutable treasury Position.
    #[allow(clippy::too_many_arguments)]
    pub fn select(
        fee_record: Id,
        realm: Id,
        market: Id,
        epoch: Id,
        settlement_candidate: Id,
        treasury_position: Id,
        price_scale: u64,
        outcome_count: u8,
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV2,
    ) -> Result<Self> {
        for identity in [
            fee_record,
            realm,
            market,
            epoch,
            settlement_candidate,
            treasury_position,
        ] {
            live(identity)?;
        }
        if outcome_count < 2
            || usize::from(outcome_count) > MAX_OUTCOMES
            || price_scale == 0
        {
            return Err(Error::InvalidWidth);
        }
        batch.validate().map_err(|_| Error::InvalidPolicy)?;
        revenue.validate().map_err(|_| Error::InvalidPolicy)?;
        let (dispersion_bps, floor_range_bps) = match batch.fee_base {
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } if dispersion_bps == revenue.dispersion_bps
                && floor_range_bps == revenue.floor_range_bps =>
            {
                (dispersion_bps, floor_range_bps)
            }
            FeeBaseV1::CompositeDispersionFloor { .. } => {
                return Err(Error::MismatchedBinding);
            }
            FeeBaseV1::None | FeeBaseV1::FlatNotional { .. } => {
                return Err(Error::InvalidPolicy);
            }
        };
        let treasury_owner = Identity32V1(revenue.treasury_owner);
        live(treasury_owner)?;
        let batch_policy = batch_policy_digest(batch).map_err(|_| Error::InvalidPolicy)?;
        let revenue_policy = revenue_policy_v2_digest(revenue).map_err(|_| Error::InvalidPolicy)?;
        independent(&[
            fee_record,
            realm,
            market,
            epoch,
            settlement_candidate,
            batch_policy,
            revenue_policy,
            treasury_owner,
            treasury_position,
        ])?;
        let payoffs = [0u64; MAX_OUTCOMES];
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = price_scale;
        let denominator = composite_fee_quote(
            &payoffs,
            &prices,
            usize::from(outcome_count),
            price_scale,
            dispersion_bps,
            floor_range_bps,
            0,
        )
        .map_err(|_| Error::InvalidPolicy)?
        .base_denominator;
        Ok(Self {
            fee_record,
            realm,
            market,
            epoch,
            settlement_candidate,
            batch_policy,
            revenue_policy,
            treasury_owner,
            treasury_position,
            price_scale,
            outcome_count,
            dispersion_bps,
            floor_range_bps,
            carry_denominator: denominator,
        })
    }

    pub fn binds_revenue_policy(&self, policy: &RevenuePolicyV2) -> Result<()> {
        policy.validate().map_err(|_| Error::InvalidPolicy)?;
        let digest = revenue_policy_v2_digest(policy).map_err(|_| Error::InvalidPolicy)?;
        if digest != self.revenue_policy
            || Identity32V1(policy.treasury_owner) != self.treasury_owner
            || policy.dispersion_bps != self.dispersion_bps
            || policy.floor_range_bps != self.floor_range_bps
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    pub fn quote_owner(
        &self,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        prior_carry: u128,
    ) -> Result<FeeQuoteV1> {
        quote_owner_for_selected(self, payoffs, prices, prior_carry)
    }

    fn assess_owner(
        &self,
        owner: Id,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        prior_carry: u128,
        boundary: AssessmentBoundaryV1,
    ) -> Result<OwnerFeeAssessmentV1> {
        assess_owner_for_selected(
            self,
            owner,
            payoffs,
            prices,
            prior_carry,
            boundary,
        )
    }
}

impl sealed::SelectedCompositeFeeSealed for SelectedCompositeFeeV2 {}

impl SelectedCompositeFeeAccess for SelectedCompositeFeeV2 {
    fn fee_record(&self) -> Id { self.fee_record }
    fn realm(&self) -> Id { self.realm }
    fn market(&self) -> Id { self.market }
    fn epoch(&self) -> Id { self.epoch }
    fn selected_candidate(&self) -> Id { self.settlement_candidate }
    fn batch_policy(&self) -> Id { self.batch_policy }
    fn revenue_policy(&self) -> Id { self.revenue_policy }
    fn treasury_owner(&self) -> Id { self.treasury_owner }
    fn treasury_position(&self) -> Id { self.treasury_position }
    fn price_scale(&self) -> u64 { self.price_scale }
    fn outcome_count(&self) -> u8 { self.outcome_count }
    fn dispersion_bps(&self) -> u32 { self.dispersion_bps }
    fn floor_range_bps(&self) -> u32 { self.floor_range_bps }
    fn carry_denominator(&self) -> u128 { self.carry_denominator }
}

fn quote_owner_for_selected<S: SelectedCompositeFeeAccess + ?Sized>(
    selected: &S,
    payoffs: &[u64; MAX_OUTCOMES],
    prices: &[u64; MAX_OUTCOMES],
    prior_carry: u128,
) -> Result<FeeQuoteV1> {
    let quote = composite_fee_quote(
        payoffs,
        prices,
        usize::from(selected.outcome_count()),
        selected.price_scale(),
        selected.dispersion_bps(),
        selected.floor_range_bps(),
        prior_carry,
    )
    .map_err(|_| Error::InvalidPolicy)?;
    if quote.base_denominator != selected.carry_denominator()
        || quote.exact_denominator != selected.carry_denominator()
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(quote)
}

fn assess_owner_for_selected<S: SelectedCompositeFeeAccess + ?Sized>(
    selected: &S,
    owner: Id,
    payoffs: &[u64; MAX_OUTCOMES],
    prices: &[u64; MAX_OUTCOMES],
    prior_carry: u128,
    boundary: AssessmentBoundaryV1,
) -> Result<OwnerFeeAssessmentV1> {
    live(owner)?;
    let quote = quote_owner_for_selected(selected, payoffs, prices, prior_carry)?;
    let (charged, next_carry) = match boundary {
        AssessmentBoundaryV1::FragmentFloor => (quote.floor_atoms, quote.carry),
        AssessmentBoundaryV1::TerminalCeil => (quote.terminal_ceil_atoms, 0),
    };
    Ok(OwnerFeeAssessmentV1 {
        owner,
        fee_record: selected.fee_record(),
        charged_atoms: u64::try_from(charged).map_err(|_| Error::AmountOutOfRange)?,
        next_carry,
        denominator: selected.carry_denominator(),
        boundary,
    })
}

/// The one persistent carry for an owner's selected-candidate fee rational.
///
/// Composite dispersion is owner-netted and subadditive, so an intent-scoped
/// carry is not an admissible semantic owner for this fee base. This state is
/// bound to the immutable selected-fee record and can create assessments only
/// through the relation-derived denominator stored by that record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeCarryV1 {
    fee_record: Id,
    owner: Id,
    denominator: u128,
    remainder: u128,
    paid_atoms: u64,
    closed: bool,
}

impl OwnerFeeCarryV1 {
    pub fn admit<S: SelectedCompositeFeeAccess + ?Sized>(selected: &S, owner: Id) -> Result<Self> {
        Self::restore(selected, owner, 0, 0, false)
    }

    /// Validate and restore a future adapter's persistent state. An open carry
    /// is canonical only below its exact denominator; a closed carry is zero.
    pub fn restore<S: SelectedCompositeFeeAccess + ?Sized>(
        selected: &S,
        owner: Id,
        remainder: u128,
        paid_atoms: u64,
        closed: bool,
    ) -> Result<Self> {
        live(owner)?;
        if remainder >= selected.carry_denominator() || (closed && remainder != 0) {
            return Err(Error::NonCanonicalCarry);
        }
        Ok(Self {
            fee_record: selected.fee_record(),
            owner,
            denominator: selected.carry_denominator(),
            remainder,
            paid_atoms,
            closed,
        })
    }

    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn owner(&self) -> Id {
        self.owner
    }

    pub const fn denominator(&self) -> u128 {
        self.denominator
    }

    pub const fn remainder(&self) -> u128 {
        self.remainder
    }

    pub const fn paid_atoms(&self) -> u64 {
        self.paid_atoms
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    fn authenticate<S: SelectedCompositeFeeAccess + ?Sized>(&self, selected: &S) -> Result<()> {
        if self.closed {
            return Err(Error::AlreadyClosed);
        }
        if self.fee_record != selected.fee_record()
            || self.denominator != selected.carry_denominator()
        {
            return Err(Error::MismatchedBinding);
        }
        if self.remainder >= self.denominator {
            return Err(Error::NonCanonicalCarry);
        }
        Ok(())
    }

    /// Assess one owner-wide payoff fragment at the single named rounding
    /// boundary. No caller can independently supply or mutate prior carry.
    pub fn assess<S: SelectedCompositeFeeAccess + ?Sized>(
        mut self,
        selected: &S,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        boundary: AssessmentBoundaryV1,
    ) -> Result<(Self, OwnerFeeAssessmentV1)> {
        self.authenticate(selected)?;
        let assessment = assess_owner_for_selected(
            selected,
            self.owner,
            payoffs,
            prices,
            self.remainder,
            boundary,
        )?;
        self.remainder = assessment.next_carry;
        self.paid_atoms = add(self.paid_atoms, assessment.charged_atoms)?;
        if boundary == AssessmentBoundaryV1::TerminalCeil {
            self.closed = true;
        }
        Ok((self, assessment))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{
        AllocationPolicyV1, AonPolicyV1, PairingWitnessPolicyV1,
        PortfolioLotPolicyV1, ResidualSettlementV1, RoundingBoundaryV1,
        ScorePolicyV1, SelfCrossPolicyV1, TransferPhaseV1,
    };
    use clutch_batch::DustPolicy;

    fn id(byte: u8) -> Id {
        Id([byte; 32])
    }

    fn batch(dispersion_bps: u32, floor_range_bps: u32) -> FrozenPolicyV1 {
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross: SelfCrossPolicyV1::RefuseOverlap,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
            transfer_phase: TransferPhaseV1::ActiveOrResolved,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            },
        }
    }

    fn selected_v2() -> SelectedCompositeFeeV2 {
        SelectedCompositeFeeV2::select(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            10_000,
            2,
            &batch(40, 10),
            &RevenuePolicyV2::successor_development([9; 32]),
        )
        .unwrap()
    }

    #[test]
    fn selected_v2_binds_policy_rates_and_shared_carry() {
        let selected = selected_v2();
        assert_eq!(selected.dispersion_bps(), 40);
        assert_eq!(selected.floor_range_bps(), 10);
        assert_eq!(selected.treasury_owner(), id(9));
        let carry = OwnerFeeCarryV1::admit(&selected, id(20)).unwrap();
        assert_eq!(carry.fee_record(), selected.fee_record());
        assert_eq!(carry.denominator(), selected.carry_denominator());
    }

    #[test]
    fn selected_v2_refuses_batch_revenue_rate_rebinding() {
        assert_eq!(
            SelectedCompositeFeeV2::select(
                id(1),
                id(2),
                id(3),
                id(4),
                id(5),
                id(6),
                10_000,
                2,
                &batch(39, 10),
                &RevenuePolicyV2::successor_development([9; 32]),
            ),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn selected_v2_codec_is_disjoint_from_v1() {
        let selected = selected_v2();
        let bytes = crate::codec::encode_fee_record_v2(&selected).unwrap();
        assert_eq!(
            crate::codec::decode_fee_record_v2(
                &bytes,
                &batch(40, 10),
                &RevenuePolicyV2::successor_development([9; 32]),
            )
            .unwrap(),
            selected
        );
        let mut wrong_version = bytes;
        wrong_version[8..10].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            crate::codec::decode_fee_record_v2(
                &wrong_version,
                &batch(40, 10),
                &RevenuePolicyV2::successor_development([9; 32]),
            ),
            Err(Error::WrongVersion)
        );
    }
}
