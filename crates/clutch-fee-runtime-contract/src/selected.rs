//! Immutable selection and exact owner-level assessment.

use clutch_batch::relation_v1::{
    composite_fee_quote, FeeBaseV1, FeeQuoteV1, FrozenPolicyV1, MAX_OUTCOMES,
};
use clutch_batch_policy_identity::revenue_policy_v1::{
    revenue_policy_digest, treasury_admits_fee_bearing, RevenuePolicyV1,
};
use clutch_batch_policy_identity::{batch_policy_digest, Identity32V1};

use crate::{add, independent, live, Error, Id, Result};

/// The single named rounding event for one owner-fee transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssessmentBoundaryV1 {
    /// An intermediate fragment pays the exact floor and persists its carry.
    FragmentFloor,
    /// The terminal owner event pays the exact ceiling and closes carry zero.
    TerminalCeil,
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
        if !(2..=MAX_OUTCOMES as u8).contains(&outcome_count) || price_scale == 0 {
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
    pub fn admit(selected: &SelectedCompositeFeeV1, owner: Id) -> Result<Self> {
        Self::restore(selected, owner, 0, 0, false)
    }

    /// Validate and restore a future adapter's persistent state. An open carry
    /// is canonical only below its exact denominator; a closed carry is zero.
    pub fn restore(
        selected: &SelectedCompositeFeeV1,
        owner: Id,
        remainder: u128,
        paid_atoms: u64,
        closed: bool,
    ) -> Result<Self> {
        live(owner)?;
        if remainder >= selected.carry_denominator || (closed && remainder != 0) {
            return Err(Error::NonCanonicalCarry);
        }
        Ok(Self {
            fee_record: selected.fee_record,
            owner,
            denominator: selected.carry_denominator,
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

    fn authenticate(&self, selected: &SelectedCompositeFeeV1) -> Result<()> {
        if self.closed {
            return Err(Error::AlreadyClosed);
        }
        if self.fee_record != selected.fee_record || self.denominator != selected.carry_denominator
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
    pub fn assess(
        mut self,
        selected: &SelectedCompositeFeeV1,
        payoffs: &[u64; MAX_OUTCOMES],
        prices: &[u64; MAX_OUTCOMES],
        boundary: AssessmentBoundaryV1,
    ) -> Result<(Self, OwnerFeeAssessmentV1)> {
        self.authenticate(selected)?;
        let assessment =
            selected.assess_owner(self.owner, payoffs, prices, self.remainder, boundary)?;
        self.remainder = assessment.next_carry;
        self.paid_atoms = add(self.paid_atoms, assessment.charged_atoms)?;
        if boundary == AssessmentBoundaryV1::TerminalCeil {
            self.closed = true;
        }
        Ok((self, assessment))
    }
}
