//! Exact covered dealer over signed net flow and explicitly contributed assets.
//!
//! LPs fund one immutable cash-and-Egg unit basket before activation. A
//! separate sponsor subsidy covers the generalized quadratic curve's global
//! adverse-selection loss. Trading moves only already-custodied Eggs and cash;
//! this model never splits, merges, mints, borrows, or touches Hoard principal.

use core::convert::TryFrom;

use clutch_batch::dealer_leg_v2::{
    DealerQuotePreconditionV2, VerifiedDealerLegV2, DEALER_LEG_VERSION_V2,
};

use super::{Id, PriceVectorV1, MAX_ATOMS, MAX_OUTCOMES, MAX_PRICE_DENOMINATOR};

/// Maximum LP positions in one fixed-capacity dealer.
pub const MAX_LPS: usize = 8;
/// Conservative maximum live aggregate pool cash from LP cash, subsidy, and curve.
pub const MAX_LIVE_POOL_ATOMS: u64 = 3 * MAX_ATOMS;
/// Conservative maximum terminal pool after redeeming bounded Egg custody.
pub const MAX_TERMINAL_POOL_ATOMS: u64 = 4 * MAX_ATOMS;

/// Checked refusal from a covered-dealer transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerError {
    /// A required identity is zero or an identity role aliases a forbidden role.
    InvalidIdentity,
    /// Outcome width or fixed-capacity padding is invalid.
    InvalidBasis,
    /// A required amount or denominator is zero.
    ZeroValue,
    /// A value exceeds the frozen arithmetic domain.
    ParameterOutOfRange,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// The immutable schedule or supplied slot is invalid.
    InvalidSchedule,
    /// Initial-price weights do not form the named exact simplex.
    InvalidPriceVector,
    /// A flow or contributed Egg quantity violates the universal settlement lot.
    NonIntegralLot,
    /// The full declared signed box leaves the nonnegative-price domain.
    PriceDomain,
    /// Present sponsor subsidy is below the global curve-loss bound.
    InsufficientSubsidy,
    /// Present LP assets cannot cover the declared inventory box.
    InsufficientCoverage,
    /// A transition is unavailable in the current phase.
    InvalidPhase,
    /// A caller does not match the bound sponsor or LP owner.
    MismatchedOwner,
    /// No fixed-capacity LP position is available.
    PositionLimit,
    /// A share operation is zero, inexact, above balance, or outside policy bounds.
    InvalidShares,
    /// An aggregate trade contains both directions on one Egg or no flow.
    NonCanonicalFlow,
    /// A signed endpoint exceeds its immutable buy or sell cap.
    InventoryLimit,
    /// Pool cash cannot fund the exact endpoint transition.
    InsufficientCash,
    /// Pool Egg custody cannot fund the exact endpoint transition.
    InsufficientEggs,
    /// An unwind-only transition increases or crosses an exposure.
    IncreasesExposure,
    /// A payout vector is not an exact nonnegative integer simplex.
    InvalidPayoutVector,
    /// A terminal LP allocation was already claimed.
    AlreadyClaimed,
    /// An authenticated dealer-leg projection named another facility, policy, or generation.
    DealerLegBindingMismatch,
    /// The dealer-leg aggregate receipt disagreed with exact facility recomputation.
    DealerLegReceiptMismatch,
    /// A persisted field disagrees with exact recomputation.
    InvariantViolation,
}

/// Result alias for total checked dealer operations.
pub type DealerResult<T> = core::result::Result<T, DealerError>;

/// Immutable policy for a covered two-sided dealer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDealerPolicyV1 {
    /// Digest of canonical policy bytes.
    pub policy_id: Id,
    /// Exact Market identity.
    pub market: Id,
    /// Digest of immutable native Terms bytes.
    pub terms_digest: Id,
    /// Deterministic recurring Instance identity.
    pub instance_id: Id,
    /// Native claim-domain binding shared with admitted wrappers.
    pub claim_domain_digest: Id,
    /// Active native Egg count.
    pub outcome_count: u8,
    /// Exact native payout denominator and conservative trade lot.
    pub payout_denominator: u64,
    /// Denominator shared by initial-price weights.
    pub initial_price_denominator: u64,
    /// Initial-price numerators followed by canonical zero padding.
    pub initial_price_weights: [u64; MAX_OUTCOMES],
    /// Immutable quadratic depth in raw Egg atoms.
    pub depth_atoms: u64,
    /// Maximum net Eggs bought by the dealer per outcome.
    pub max_net_buy: [u64; MAX_OUTCOMES],
    /// Maximum net Eggs sold by the dealer per outcome.
    pub max_net_sell: [u64; MAX_OUTCOMES],
    /// Cash atoms contributed by one LP share unit.
    pub capital_unit_cash_atoms: u64,
    /// Existing, already-backed Eggs contributed by one LP share unit.
    pub capital_unit_eggs: [u64; MAX_OUTCOMES],
    /// Minimum exact share units required for activation.
    pub minimum_lp_shares: u64,
    /// Maximum exact share units admitted by this policy.
    pub maximum_lp_shares: u64,
    /// Queued-share numerator required to trigger unwind-only mode.
    pub shutdown_queue_numerator: u64,
    /// Queued-share denominator required to trigger unwind-only mode.
    pub shutdown_queue_denominator: u64,
    /// First slot at which funding closes and activation/cancellation is admitted.
    pub funding_deadline_slot: u64,
    /// First slot at which ordinary two-sided trading is admitted.
    pub trading_open_slot: u64,
    /// First slot at which timed unwind-only mode is permissionlessly available.
    pub trading_close_slot: u64,
    /// First slot at which authenticated resolution is admitted.
    pub maturity_slot: u64,
}

impl SignedDealerPolicyV1 {
    /// Validate fixed bounds, exact price simplex, coverage units, and schedule.
    pub fn validate(&self) -> DealerResult<()> {
        check_id(self.policy_id)?;
        check_id(self.market)?;
        check_id(self.terms_digest)?;
        check_id(self.instance_id)?;
        check_id(self.claim_domain_digest)?;
        let n = usize::from(self.outcome_count);
        if self.outcome_count < 2 || n > MAX_OUTCOMES {
            return Err(DealerError::InvalidBasis);
        }
        if self.payout_denominator == 0
            || self.initial_price_denominator == 0
            || self.depth_atoms == 0
            || self.minimum_lp_shares == 0
            || self.maximum_lp_shares == 0
            || self.shutdown_queue_numerator == 0
            || self.shutdown_queue_denominator == 0
        {
            return Err(DealerError::ZeroValue);
        }
        if self.payout_denominator > MAX_ATOMS
            || self.initial_price_denominator > MAX_PRICE_DENOMINATOR
            || self.depth_atoms > MAX_ATOMS
            || self.maximum_lp_shares > MAX_ATOMS
            || self.capital_unit_cash_atoms > MAX_ATOMS
            || self.minimum_lp_shares > self.maximum_lp_shares
            || self.shutdown_queue_numerator > self.shutdown_queue_denominator
            || self.shutdown_queue_denominator > MAX_ATOMS
        {
            return Err(DealerError::ParameterOutOfRange);
        }
        if self.funding_deadline_slot == 0
            || self.funding_deadline_slot > self.trading_open_slot
            || self.trading_open_slot >= self.trading_close_slot
            || self.trading_close_slot >= self.maturity_slot
        {
            return Err(DealerError::InvalidSchedule);
        }
        validate_padding_u64(self.outcome_count, &self.initial_price_weights)?;
        validate_padding_u64(self.outcome_count, &self.max_net_buy)?;
        validate_padding_u64(self.outcome_count, &self.max_net_sell)?;
        validate_padding_u64(self.outcome_count, &self.capital_unit_eggs)?;

        let mut price_sum = 0u128;
        let mut has_capital = self.capital_unit_cash_atoms != 0;
        let mut has_flow = false;
        let mut i = 0usize;
        while i < n {
            price_sum = price_sum
                .checked_add(u128::from(self.initial_price_weights[i]))
                .ok_or(DealerError::ArithmeticOverflow)?;
            let values = [
                self.max_net_buy[i],
                self.max_net_sell[i],
                self.capital_unit_eggs[i],
            ];
            let mut j = 0usize;
            while j < values.len() {
                if values[j] > MAX_ATOMS {
                    return Err(DealerError::ParameterOutOfRange);
                }
                if !values[j].is_multiple_of(self.payout_denominator) {
                    return Err(DealerError::NonIntegralLot);
                }
                j += 1;
            }
            has_flow |= self.max_net_buy[i] != 0 || self.max_net_sell[i] != 0;
            has_capital |= self.capital_unit_eggs[i] != 0;
            checked_product_u64(self.capital_unit_eggs[i], self.maximum_lp_shares)?;
            i += 1;
        }
        if price_sum != u128::from(self.initial_price_denominator) {
            return Err(DealerError::InvalidPriceVector);
        }
        if !has_capital || !has_flow {
            return Err(DealerError::ZeroValue);
        }
        checked_product_u64(self.capital_unit_cash_atoms, self.maximum_lp_shares)?;

        self.validate_full_price_box()?;
        i = 0;
        while i < n {
            let minimum_eggs =
                checked_product_u64(self.capital_unit_eggs[i], self.minimum_lp_shares)?;
            let maximum_eggs =
                checked_product_u64(self.capital_unit_eggs[i], self.maximum_lp_shares)?;
            if minimum_eggs < self.max_net_sell[i]
                || maximum_eggs
                    .checked_add(self.max_net_buy[i])
                    .ok_or(DealerError::ArithmeticOverflow)?
                    > MAX_ATOMS
            {
                return Err(DealerError::InsufficientCoverage);
            }
            i += 1;
        }
        Ok(())
    }

    /// Exact present subsidy covering worst-case curve loss under every outcome.
    pub fn minimum_sponsor_subsidy(&self) -> DealerResult<u64> {
        self.validate()?;
        minimum_sponsor_subsidy_for_price(self)
    }

    /// Least sponsor cash satisfying both loss and minimum-share bid financing.
    pub fn minimum_sponsor_capital(&self) -> DealerResult<u64> {
        self.validate()?;
        minimum_sponsor_capital_for_valid_policy(self)
    }

    fn validate_full_price_box(&self) -> DealerResult<()> {
        let n = u128::from(self.outcome_count);
        let price_denominator = u128::from(self.initial_price_denominator);
        let b = u128::from(self.depth_atoms);
        let mut total_sell = 0u128;
        let mut i = 0usize;
        while i < usize::from(self.outcome_count) {
            total_sell = total_sell
                .checked_add(u128::from(self.max_net_sell[i]))
                .ok_or(DealerError::ArithmeticOverflow)?;
            i += 1;
        }
        i = 0;
        while i < usize::from(self.outcome_count) {
            let initial = u128::from(self.initial_price_weights[i])
                .checked_mul(b)
                .and_then(|value| value.checked_mul(n))
                .ok_or(DealerError::ArithmeticOverflow)?;
            let own_buy = u128::from(self.max_net_buy[i])
                .checked_mul(n - 1)
                .ok_or(DealerError::ArithmeticOverflow)?;
            let other_sell = total_sell
                .checked_sub(u128::from(self.max_net_sell[i]))
                .ok_or(DealerError::InvariantViolation)?;
            let displacement = price_denominator
                .checked_mul(
                    own_buy
                        .checked_add(other_sell)
                        .ok_or(DealerError::ArithmeticOverflow)?,
                )
                .ok_or(DealerError::ArithmeticOverflow)?;
            if initial < displacement {
                return Err(DealerError::PriceDomain);
            }
            i += 1;
        }
        Ok(())
    }
}

/// Lifecycle of one covered dealer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SignedDealerPhase {
    /// LP unit-basket contributions and exact withdrawals are admitted.
    Funding = 0,
    /// Ordinary two-sided aggregate call-auction transitions are admitted.
    Trading = 1,
    /// Only componentwise exposure-reducing transitions are admitted.
    UnwindOnly = 2,
    /// Authenticated payout redeemed all custodied Eggs into terminal cash.
    Resolved = 3,
    /// Activation failed or became stale; contributors and sponsor may refund.
    Cancelled = 4,
}

/// One fixed-capacity LP share and terminal-claim record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpPositionV1 {
    /// Immutable LP owner, or zero for a canonical empty position.
    pub owner: Id,
    /// Exact unit-basket shares.
    pub shares: u64,
    /// Irrevocably queued shares requesting unwind-only mode.
    pub queued_shares: u64,
    /// Exact terminal cash assigned by the one Hamilton allocation.
    pub terminal_claim_atoms: u64,
    /// Whether the terminal claim was already withdrawn.
    pub claimed: bool,
}

impl LpPositionV1 {
    /// Canonical empty LP position.
    pub const EMPTY: Self = Self {
        owner: [0; 32],
        shares: 0,
        queued_shares: 0,
        terminal_claim_atoms: 0,
        claimed: false,
    };
}

/// Net native Egg movement in one aggregate dealer leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDealerTradeV1 {
    /// Custodied Eggs transferred from the dealer to users.
    pub sell_to_users: [u64; MAX_OUTCOMES],
    /// Existing Eggs transferred from users to dealer custody.
    pub buy_from_users: [u64; MAX_OUTCOMES],
}

impl SignedDealerTradeV1 {
    /// Canonical empty flow.
    pub const EMPTY: Self = Self {
        sell_to_users: [0; MAX_OUTCOMES],
        buy_from_users: [0; MAX_OUTCOMES],
    };
}

/// Exact asset recipe for one signed dealer endpoint transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDealerTradeReceiptV1 {
    /// Bound policy identity.
    pub policy_id: Id,
    /// Generation consumed by this transition.
    pub pre_generation: u64,
    /// Generation produced by this transition.
    pub post_generation: u64,
    /// Exact aggregate native flow.
    pub trade: SignedDealerTradeV1,
    /// Old cumulative net-sold vector.
    pub old_net_sold: [i64; MAX_OUTCOMES],
    /// New cumulative net-sold vector.
    pub new_net_sold: [i64; MAX_OUTCOMES],
    /// Collateral atoms paid by traders to the pool.
    pub trader_cash_in_atoms: u64,
    /// Collateral atoms paid by the pool to traders.
    pub trader_cash_out_atoms: u64,
    /// Exact post-transition pool cash.
    pub new_pool_cash_atoms: u64,
    /// Exact post-transition custodied Eggs.
    pub new_pool_eggs: [u64; MAX_OUTCOMES],
}

/// Exact contribution or funding-withdrawal basket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingBasketV1 {
    /// Bound LP owner.
    pub owner: Id,
    /// Shares minted or burned.
    pub shares: u64,
    /// Exact cash atoms transferred.
    pub cash_atoms: u64,
    /// Exact existing Egg atoms transferred.
    pub eggs: [u64; MAX_OUTCOMES],
}

/// Exact state of one pooled, covered, signed-inventory dealer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDealerStateV1 {
    /// Immutable dealer policy.
    pub policy: SignedDealerPolicyV1,
    /// Canonical semantic facility identity; a live account key is a separate adapter role.
    pub facility_id: Id,
    /// Sponsor/refund owner of the pre-activation sponsor capital.
    pub sponsor: Id,
    /// Present sponsor cash committed before any trade and donated on activation.
    pub sponsor_capital_atoms: u64,
    /// Sponsor capital refunded only after failed/stale funding cancellation.
    pub sponsor_capital_refunded_atoms: u64,
    /// Exact outstanding unit-basket shares.
    pub total_shares: u64,
    /// Canonical fixed-capacity LP records.
    pub positions: [LpPositionV1; MAX_LPS],
    /// Current pool cash, always outside Market Hoard.
    pub pool_cash_atoms: u64,
    /// Current custodied existing Eggs, already backed by Market Hoard.
    pub pool_eggs: [u64; MAX_OUTCOMES],
    /// Cumulative signed Eggs sold: positive sold, negative bought.
    pub net_sold: [i64; MAX_OUTCOMES],
    /// Authenticated terminal payout, zero before resolution.
    pub terminal_payout_weights: [u64; MAX_OUTCOMES],
    /// Total cash before any resolved LP claim is withdrawn.
    pub terminal_pool_atoms: u64,
    /// Monotone transition generation.
    pub generation: u64,
    /// Current lifecycle phase.
    pub phase: SignedDealerPhase,
}

impl SignedDealerStateV1 {
    /// Initialize funding with present, still-refundable sponsor capital.
    pub fn initialize(
        policy: SignedDealerPolicyV1,
        facility_id: Id,
        sponsor: Id,
        sponsor_capital_atoms: u64,
    ) -> DealerResult<Self> {
        policy.validate()?;
        check_id(facility_id)?;
        check_id(sponsor)?;
        if facility_id == sponsor {
            return Err(DealerError::InvalidIdentity);
        }
        if sponsor_capital_atoms < policy.minimum_sponsor_subsidy()? {
            return Err(DealerError::InsufficientSubsidy);
        }
        if sponsor_capital_atoms < policy.minimum_sponsor_capital()? {
            return Err(DealerError::InsufficientCoverage);
        }
        if sponsor_capital_atoms > MAX_ATOMS {
            return Err(DealerError::ParameterOutOfRange);
        }
        let value = Self {
            policy,
            facility_id,
            sponsor,
            sponsor_capital_atoms,
            sponsor_capital_refunded_atoms: 0,
            total_shares: 0,
            positions: [LpPositionV1::EMPTY; MAX_LPS],
            pool_cash_atoms: sponsor_capital_atoms,
            pool_eggs: [0; MAX_OUTCOMES],
            net_sold: [0; MAX_OUTCOMES],
            terminal_payout_weights: [0; MAX_OUTCOMES],
            terminal_pool_atoms: 0,
            generation: 0,
            phase: SignedDealerPhase::Funding,
        };
        value.validate()?;
        Ok(value)
    }

    /// Recompute every funding, custody, curve, share, and terminal identity.
    pub fn validate(&self) -> DealerResult<()> {
        self.policy.validate()?;
        check_id(self.facility_id)?;
        check_id(self.sponsor)?;
        if self.facility_id == self.sponsor
            || self.sponsor_capital_atoms < self.policy.minimum_sponsor_subsidy()?
            || self.sponsor_capital_atoms < self.policy.minimum_sponsor_capital()?
            || self.sponsor_capital_atoms > MAX_ATOMS
        {
            return Err(DealerError::InvariantViolation);
        }
        let (shares, queued) = validate_positions(self)?;
        if shares != self.total_shares || self.total_shares > self.policy.maximum_lp_shares {
            return Err(DealerError::InvariantViolation);
        }
        let base_cash = self.lp_base_cash()?;
        let base_eggs = self.lp_base_eggs()?;
        let aggregate_cap = if self.phase == SignedDealerPhase::Resolved {
            MAX_TERMINAL_POOL_ATOMS
        } else {
            MAX_LIVE_POOL_ATOMS
        };
        if self.pool_cash_atoms > aggregate_cap
            || self.terminal_pool_atoms > MAX_TERMINAL_POOL_ATOMS
        {
            return Err(DealerError::ParameterOutOfRange);
        }

        match self.phase {
            SignedDealerPhase::Funding | SignedDealerPhase::Cancelled => {
                if self.net_sold != [0; MAX_OUTCOMES]
                    || self.terminal_payout_weights != [0; MAX_OUTCOMES]
                    || self.terminal_pool_atoms != 0
                    || queued != 0
                    || any_terminal_position(&self.positions)
                {
                    return Err(DealerError::InvariantViolation);
                }
                if self.phase == SignedDealerPhase::Funding
                    && self.sponsor_capital_refunded_atoms != 0
                {
                    return Err(DealerError::InvariantViolation);
                }
                if self.phase == SignedDealerPhase::Cancelled
                    && self.sponsor_capital_refunded_atoms != 0
                    && self.sponsor_capital_refunded_atoms != self.sponsor_capital_atoms
                {
                    return Err(DealerError::InvariantViolation);
                }
                let expected_cash = base_cash
                    .checked_add(self.sponsor_capital_atoms)
                    .and_then(|value| value.checked_sub(self.sponsor_capital_refunded_atoms))
                    .ok_or(DealerError::InvariantViolation)?;
                if self.pool_cash_atoms != expected_cash || self.pool_eggs != base_eggs {
                    return Err(DealerError::InvariantViolation);
                }
            }
            SignedDealerPhase::Trading | SignedDealerPhase::UnwindOnly => {
                if self.total_shares < self.policy.minimum_lp_shares
                    || self.sponsor_capital_refunded_atoms != 0
                    || self.terminal_payout_weights != [0; MAX_OUTCOMES]
                    || self.terminal_pool_atoms != 0
                    || any_terminal_position(&self.positions)
                {
                    return Err(DealerError::InvariantViolation);
                }
                validate_signed_inventory(&self.policy, &self.net_sold)?;
                let expected_eggs = holdings_from_net(&self.policy, &base_eggs, &self.net_sold)?;
                let potential = signed_rounded_quadratic_potential(&self.policy, &self.net_sold)?;
                let expected_cash =
                    cash_with_potential(base_cash, self.sponsor_capital_atoms, potential)?;
                if self.pool_eggs != expected_eggs || self.pool_cash_atoms != expected_cash {
                    return Err(DealerError::InvariantViolation);
                }
                if self.phase == SignedDealerPhase::Trading
                    && queue_threshold_met(&self.policy, queued, self.total_shares)?
                {
                    return Err(DealerError::InvariantViolation);
                }
            }
            SignedDealerPhase::Resolved => {
                if self.total_shares < self.policy.minimum_lp_shares
                    || self.sponsor_capital_refunded_atoms != 0
                    || self.pool_eggs != [0; MAX_OUTCOMES]
                {
                    return Err(DealerError::InvariantViolation);
                }
                validate_signed_inventory(&self.policy, &self.net_sold)?;
                validate_payout(&self.policy, &self.terminal_payout_weights)?;
                let potential = signed_rounded_quadratic_potential(&self.policy, &self.net_sold)?;
                let live_cash =
                    cash_with_potential(base_cash, self.sponsor_capital_atoms, potential)?;
                let pre_resolution_eggs =
                    holdings_from_net(&self.policy, &base_eggs, &self.net_sold)?;
                let redeemed = exact_unsigned_payout(
                    self.policy.outcome_count,
                    &pre_resolution_eggs,
                    &self.terminal_payout_weights,
                    self.policy.payout_denominator,
                )?;
                let expected_terminal = live_cash
                    .checked_add(redeemed)
                    .ok_or(DealerError::ArithmeticOverflow)?;
                if self.terminal_pool_atoms != expected_terminal {
                    return Err(DealerError::InvariantViolation);
                }
                let allocations = allocate_terminal(self.terminal_pool_atoms, &self.positions)?;
                let mut unclaimed = 0u64;
                let mut i = 0usize;
                while i < MAX_LPS {
                    if self.positions[i].shares == 0 {
                        if allocations[i] != 0 {
                            return Err(DealerError::InvariantViolation);
                        }
                    } else if self.positions[i].terminal_claim_atoms != allocations[i] {
                        return Err(DealerError::InvariantViolation);
                    } else if !self.positions[i].claimed {
                        unclaimed = unclaimed
                            .checked_add(allocations[i])
                            .ok_or(DealerError::ArithmeticOverflow)?;
                    }
                    i += 1;
                }
                if self.pool_cash_atoms != unclaimed {
                    return Err(DealerError::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    /// Add an exact multiple of the immutable LP unit basket before funding closes.
    pub fn contribute(
        &mut self,
        slot: u64,
        owner: Id,
        shares: u64,
    ) -> DealerResult<FundingBasketV1> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Funding {
            return Err(DealerError::InvalidPhase);
        }
        if slot >= self.policy.funding_deadline_slot {
            return Err(DealerError::InvalidSchedule);
        }
        check_id(owner)?;
        if owner == self.sponsor || owner == self.facility_id {
            return Err(DealerError::InvalidIdentity);
        }
        if shares == 0 {
            return Err(DealerError::InvalidShares);
        }
        let new_total = self
            .total_shares
            .checked_add(shares)
            .ok_or(DealerError::ArithmeticOverflow)?;
        if new_total > self.policy.maximum_lp_shares {
            return Err(DealerError::InvalidShares);
        }
        let basket = funding_basket(&self.policy, owner, shares)?;
        let mut next = *self;
        let index = find_or_empty_position(&next.positions, owner)?;
        if next.positions[index].shares == 0 {
            next.positions[index].owner = owner;
        }
        next.positions[index].shares = next.positions[index]
            .shares
            .checked_add(shares)
            .ok_or(DealerError::ArithmeticOverflow)?;
        next.total_shares = new_total;
        next.pool_cash_atoms = next
            .pool_cash_atoms
            .checked_add(basket.cash_atoms)
            .ok_or(DealerError::ArithmeticOverflow)?;
        add_eggs(next.policy.outcome_count, &mut next.pool_eggs, &basket.eggs)?;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(basket)
    }

    /// Burn funding shares for the exact original unit basket before activation.
    pub fn withdraw_funding(
        &mut self,
        slot: u64,
        owner: Id,
        shares: u64,
    ) -> DealerResult<FundingBasketV1> {
        self.validate()?;
        let admitted = match self.phase {
            SignedDealerPhase::Funding => slot < self.policy.funding_deadline_slot,
            SignedDealerPhase::Cancelled => true,
            _ => false,
        };
        if !admitted {
            return Err(if self.phase == SignedDealerPhase::Funding {
                DealerError::InvalidSchedule
            } else {
                DealerError::InvalidPhase
            });
        }
        if shares == 0 {
            return Err(DealerError::InvalidShares);
        }
        let index = find_position(&self.positions, owner)?;
        if shares > self.positions[index].shares {
            return Err(DealerError::InvalidShares);
        }
        let basket = funding_basket(&self.policy, owner, shares)?;
        let mut next = *self;
        next.positions[index].shares -= shares;
        if next.positions[index].shares == 0 {
            next.positions[index] = LpPositionV1::EMPTY;
        }
        next.total_shares -= shares;
        next.pool_cash_atoms = next
            .pool_cash_atoms
            .checked_sub(basket.cash_atoms)
            .ok_or(DealerError::InsufficientCash)?;
        subtract_eggs(next.policy.outcome_count, &mut next.pool_eggs, &basket.eggs)?;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(basket)
    }

    /// Permissionlessly activate a fully funded dealer after funding closes.
    pub fn activate(&mut self, slot: u64) -> DealerResult<()> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Funding {
            return Err(DealerError::InvalidPhase);
        }
        if slot < self.policy.funding_deadline_slot || slot >= self.policy.trading_close_slot {
            return Err(DealerError::InvalidSchedule);
        }
        if self.total_shares < self.policy.minimum_lp_shares {
            return Err(DealerError::InsufficientCoverage);
        }
        let mut next = *self;
        next.phase = SignedDealerPhase::Trading;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Cancel underfunding after its deadline or any stale funding at trading close.
    pub fn cancel_funding(&mut self, slot: u64) -> DealerResult<()> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Funding {
            return Err(DealerError::InvalidPhase);
        }
        if slot < self.policy.funding_deadline_slot
            || (self.total_shares >= self.policy.minimum_lp_shares
                && slot < self.policy.trading_close_slot)
        {
            return Err(DealerError::InvalidSchedule);
        }
        let mut next = *self;
        next.phase = SignedDealerPhase::Cancelled;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Refund all sponsor capital only when activation never occurred.
    pub fn refund_cancelled_sponsor_capital(&mut self, sponsor: Id) -> DealerResult<u64> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Cancelled {
            return Err(DealerError::InvalidPhase);
        }
        if sponsor != self.sponsor {
            return Err(DealerError::MismatchedOwner);
        }
        if self.sponsor_capital_refunded_atoms != 0 {
            return Err(DealerError::AlreadyClaimed);
        }
        let amount = self.sponsor_capital_atoms;
        let mut next = *self;
        next.pool_cash_atoms = next
            .pool_cash_atoms
            .checked_sub(amount)
            .ok_or(DealerError::InsufficientCash)?;
        next.sponsor_capital_refunded_atoms = amount;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(amount)
    }

    /// Quote one aggregate call-auction dealer transition without mutation.
    pub fn quote_trade(
        &self,
        slot: u64,
        trade: SignedDealerTradeV1,
    ) -> DealerResult<SignedDealerTradeReceiptV1> {
        self.validate()?;
        match self.phase {
            SignedDealerPhase::Trading => {
                if slot < self.policy.trading_open_slot || slot >= self.policy.trading_close_slot {
                    return Err(DealerError::InvalidSchedule);
                }
            }
            SignedDealerPhase::UnwindOnly => {
                if slot >= self.policy.maturity_slot {
                    return Err(DealerError::InvalidSchedule);
                }
            }
            _ => return Err(DealerError::InvalidPhase),
        }
        validate_trade(&self.policy, &trade)?;
        let new_net_sold = apply_trade(self.policy.outcome_count, &self.net_sold, &trade)?;
        validate_signed_inventory(&self.policy, &new_net_sold)?;
        if self.phase == SignedDealerPhase::UnwindOnly {
            validate_unwind(self.policy.outcome_count, &self.net_sold, &new_net_sold)?;
        }
        let old_potential = signed_rounded_quadratic_potential(&self.policy, &self.net_sold)?;
        let new_potential = signed_rounded_quadratic_potential(&self.policy, &new_net_sold)?;
        let difference = new_potential
            .checked_sub(old_potential)
            .ok_or(DealerError::ArithmeticOverflow)?;
        let (cash_in, cash_out) = signed_cash_flow(difference)?;
        let staged_cash = self
            .pool_cash_atoms
            .checked_add(cash_in)
            .ok_or(DealerError::ArithmeticOverflow)?;
        let new_cash = staged_cash
            .checked_sub(cash_out)
            .ok_or(DealerError::InsufficientCash)?;
        let base_eggs = self.lp_base_eggs()?;
        let new_eggs = holdings_from_net(&self.policy, &base_eggs, &new_net_sold)?;
        validate_egg_flow(
            self.policy.outcome_count,
            &self.pool_eggs,
            &new_eggs,
            &trade,
        )?;
        let expected_cash = cash_with_potential(
            self.lp_base_cash()?,
            self.sponsor_capital_atoms,
            new_potential,
        )?;
        if new_cash != expected_cash {
            return Err(DealerError::InvariantViolation);
        }
        Ok(SignedDealerTradeReceiptV1 {
            policy_id: self.policy.policy_id,
            pre_generation: self.generation,
            post_generation: self
                .generation
                .checked_add(1)
                .ok_or(DealerError::ArithmeticOverflow)?,
            trade,
            old_net_sold: self.net_sold,
            new_net_sold,
            trader_cash_in_atoms: cash_in,
            trader_cash_out_atoms: cash_out,
            new_pool_cash_atoms: new_cash,
            new_pool_eggs: new_eggs,
        })
    }

    /// Reconcile an authenticated RelationV2 dealer verdict with this facility.
    ///
    /// Per-order fills, cash allocations, residual limit envelopes, and
    /// upstream-quoted external fee amounts are owned exclusively by
    /// `clutch_batch::dealer_leg_v2`. That relation does not prove fee funding,
    /// custody, recipients, or transfer conservation. This function
    /// deliberately does not reinterpret or rescan the verified allocations.
    /// Instead it binds the verified projection to this exact facility,
    /// policy, and pre-generation, then independently recomputes the aggregate
    /// curve receipt from state.
    ///
    /// `VerifiedDealerLegV2` is an unforgeable safe-Rust capability returned
    /// only after the full pure dealer relation succeeds. It does not itself
    /// authenticate the quote proof or accounts. A live adapter must
    /// authenticate those inputs, obtain the capability, and perform this
    /// reconciliation inside one atomic state transition.
    pub fn reconcile_authenticated_dealer_leg_v2(
        &self,
        slot: u64,
        quote: &DealerQuotePreconditionV2,
        verified: &VerifiedDealerLegV2,
    ) -> DealerResult<SignedDealerTradeReceiptV1> {
        if quote.facility.version != DEALER_LEG_VERSION_V2
            || quote.facility.facility_semantics_digest != self.facility_id
            || quote.facility.policy_semantics_digest != self.policy.policy_id
            || quote.facility.pre_generation != self.generation
            || verified.outcome_count() != self.policy.outcome_count
            || verified.trade() != &quote.trade
            || verified.dealer_quote_semantics_digest() != &quote.semantic_quote_digest
        {
            return Err(DealerError::DealerLegBindingMismatch);
        }

        let trade = SignedDealerTradeV1 {
            sell_to_users: quote.trade.sell_to_users,
            buy_from_users: quote.trade.buy_from_users,
        };
        let expected = self.quote_trade(slot, trade)?;
        if quote.receipt.dealer_net_cash_in_atoms != expected.trader_cash_in_atoms
            || quote.receipt.dealer_net_cash_out_atoms != expected.trader_cash_out_atoms
        {
            return Err(DealerError::DealerLegReceiptMismatch);
        }
        Ok(expected)
    }

    /// Apply one already-authenticated and independently reconciled dealer leg.
    ///
    /// The pure model commits the aggregate state only after both the batch
    /// verdict binding and the facility receipt have been checked. Runtime
    /// user transfers, fee transfers, and custody postconditions remain one
    /// separately named atomic adapter obligation.
    pub fn execute_authenticated_dealer_leg_v2(
        &mut self,
        slot: u64,
        quote: &DealerQuotePreconditionV2,
        verified: &VerifiedDealerLegV2,
    ) -> DealerResult<SignedDealerTradeReceiptV1> {
        let receipt = self.reconcile_authenticated_dealer_leg_v2(slot, quote, verified)?;
        self.commit_trade_receipt(receipt)?;
        Ok(receipt)
    }

    /// Execute one aggregate call-auction transition after exact recomputation.
    pub fn execute_trade(
        &mut self,
        slot: u64,
        trade: SignedDealerTradeV1,
    ) -> DealerResult<SignedDealerTradeReceiptV1> {
        let receipt = self.quote_trade(slot, trade)?;
        self.commit_trade_receipt(receipt)?;
        Ok(receipt)
    }

    fn commit_trade_receipt(&mut self, receipt: SignedDealerTradeReceiptV1) -> DealerResult<()> {
        let mut next = *self;
        next.net_sold = receipt.new_net_sold;
        next.pool_cash_atoms = receipt.new_pool_cash_atoms;
        next.pool_eggs = receipt.new_pool_eggs;
        next.generation = receipt.post_generation;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Irrevocably queue LP shares and trigger unwind when the frozen quorum is met.
    pub fn queue_exit(&mut self, slot: u64, owner: Id, shares: u64) -> DealerResult<bool> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Trading && self.phase != SignedDealerPhase::UnwindOnly {
            return Err(DealerError::InvalidPhase);
        }
        if slot >= self.policy.maturity_slot || shares == 0 {
            return Err(if shares == 0 {
                DealerError::InvalidShares
            } else {
                DealerError::InvalidSchedule
            });
        }
        let index = find_position(&self.positions, owner)?;
        let new_queued = self.positions[index]
            .queued_shares
            .checked_add(shares)
            .ok_or(DealerError::ArithmeticOverflow)?;
        if new_queued > self.positions[index].shares {
            return Err(DealerError::InvalidShares);
        }
        let mut next = *self;
        next.positions[index].queued_shares = new_queued;
        let (_, total_queued) = validate_positions(&next)?;
        let triggered = queue_threshold_met(&next.policy, total_queued, next.total_shares)?;
        if triggered {
            next.phase = SignedDealerPhase::UnwindOnly;
        }
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(triggered)
    }

    /// Enter unwind-only mode under sponsor authority.
    pub fn halt_by_sponsor(&mut self, sponsor: Id) -> DealerResult<()> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Trading {
            return Err(DealerError::InvalidPhase);
        }
        if sponsor != self.sponsor {
            return Err(DealerError::MismatchedOwner);
        }
        let mut next = *self;
        next.phase = SignedDealerPhase::UnwindOnly;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Permissionlessly enter unwind-only mode at or after the frozen close slot.
    pub fn close_trading(&mut self, slot: u64) -> DealerResult<()> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Trading {
            return Err(DealerError::InvalidPhase);
        }
        if slot < self.policy.trading_close_slot {
            return Err(DealerError::InvalidSchedule);
        }
        let mut next = *self;
        next.phase = SignedDealerPhase::UnwindOnly;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Exact terminal payoff of the LP-contributed basket before dealer yield.
    pub fn terminal_lp_principal(&self, payout_weights: &[u64; MAX_OUTCOMES]) -> DealerResult<u64> {
        self.validate()?;
        if self.phase == SignedDealerPhase::Funding || self.phase == SignedDealerPhase::Cancelled {
            return Err(DealerError::InvalidPhase);
        }
        validate_payout(&self.policy, payout_weights)?;
        let egg_principal = exact_unsigned_payout(
            self.policy.outcome_count,
            &self.lp_base_eggs()?,
            payout_weights,
            self.policy.payout_denominator,
        )?;
        self.lp_base_cash()?
            .checked_add(egg_principal)
            .ok_or(DealerError::ArithmeticOverflow)
    }

    /// Nonnegative terminal pool yield above the contributed basket payoff.
    pub fn terminal_pool_yield(&self, payout_weights: &[u64; MAX_OUTCOMES]) -> DealerResult<u64> {
        self.validate()?;
        if self.phase == SignedDealerPhase::Funding || self.phase == SignedDealerPhase::Cancelled {
            return Err(DealerError::InvalidPhase);
        }
        validate_payout(&self.policy, payout_weights)?;
        let potential = signed_rounded_quadratic_potential(&self.policy, &self.net_sold)?;
        let sold_payout = exact_signed_payout(
            self.policy.outcome_count,
            &self.net_sold,
            payout_weights,
            self.policy.payout_denominator,
        )?;
        let value = i128::from(self.sponsor_capital_atoms)
            .checked_add(i128::from(potential))
            .and_then(|amount| amount.checked_sub(i128::from(sold_payout)))
            .ok_or(DealerError::ArithmeticOverflow)?;
        u64::try_from(value).map_err(|_| DealerError::InvariantViolation)
    }

    /// Resolve all custodied Eggs and allocate terminal cash over frozen LP shares.
    pub fn resolve(&mut self, slot: u64, payout_weights: [u64; MAX_OUTCOMES]) -> DealerResult<u64> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Trading && self.phase != SignedDealerPhase::UnwindOnly {
            return Err(DealerError::InvalidPhase);
        }
        if slot < self.policy.maturity_slot {
            return Err(DealerError::InvalidSchedule);
        }
        validate_payout(&self.policy, &payout_weights)?;
        let redeemed = exact_unsigned_payout(
            self.policy.outcome_count,
            &self.pool_eggs,
            &payout_weights,
            self.policy.payout_denominator,
        )?;
        let terminal = self
            .pool_cash_atoms
            .checked_add(redeemed)
            .ok_or(DealerError::ArithmeticOverflow)?;
        if terminal > MAX_TERMINAL_POOL_ATOMS {
            return Err(DealerError::ParameterOutOfRange);
        }
        let allocations = allocate_terminal(terminal, &self.positions)?;
        let mut next = *self;
        next.pool_cash_atoms = terminal;
        next.pool_eggs = [0; MAX_OUTCOMES];
        next.terminal_payout_weights = payout_weights;
        next.terminal_pool_atoms = terminal;
        let mut i = 0usize;
        while i < MAX_LPS {
            next.positions[i].terminal_claim_atoms = allocations[i];
            i += 1;
        }
        next.phase = SignedDealerPhase::Resolved;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(terminal)
    }

    /// Withdraw one LP's frozen terminal cash allocation in any order.
    pub fn claim_terminal(&mut self, owner: Id) -> DealerResult<u64> {
        self.validate()?;
        if self.phase != SignedDealerPhase::Resolved {
            return Err(DealerError::InvalidPhase);
        }
        let index = find_position(&self.positions, owner)?;
        if self.positions[index].claimed {
            return Err(DealerError::AlreadyClaimed);
        }
        let amount = self.positions[index].terminal_claim_atoms;
        let mut next = *self;
        next.pool_cash_atoms = next
            .pool_cash_atoms
            .checked_sub(amount)
            .ok_or(DealerError::InsufficientCash)?;
        next.positions[index].claimed = true;
        bump(&mut next.generation)?;
        next.validate()?;
        *self = next;
        Ok(amount)
    }

    fn lp_base_cash(&self) -> DealerResult<u64> {
        checked_product_u64(self.policy.capital_unit_cash_atoms, self.total_shares)
    }

    fn lp_base_eggs(&self) -> DealerResult<[u64; MAX_OUTCOMES]> {
        let mut result = [0; MAX_OUTCOMES];
        let mut i = 0usize;
        while i < usize::from(self.policy.outcome_count) {
            result[i] = checked_product_u64(self.policy.capital_unit_eggs[i], self.total_shares)?;
            i += 1;
        }
        Ok(result)
    }
}

/// Canonical signed integer potential `ceil(C(q))`.
pub fn signed_rounded_quadratic_potential(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<i64> {
    policy.validate()?;
    signed_potential_for_valid_policy(policy, net_sold)
}

fn signed_potential_for_valid_policy(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<i64> {
    validate_signed_inventory(policy, net_sold)?;
    let n = i128::from(policy.outcome_count);
    let b = i128::from(policy.depth_atoms);
    let price_denominator = i128::from(policy.initial_price_denominator);
    let (sum, sum_squares, initial_dot) = signed_moments(policy, net_sold)?;
    let sum_squared = sum
        .checked_mul(sum)
        .ok_or(DealerError::ArithmeticOverflow)?;
    let variance = n
        .checked_mul(sum_squares)
        .and_then(|value| value.checked_sub(sum_squared))
        .ok_or(DealerError::InvariantViolation)?;
    let linear = b
        .checked_mul(2)
        .and_then(|value| value.checked_mul(n))
        .and_then(|value| value.checked_mul(initial_dot))
        .ok_or(DealerError::ArithmeticOverflow)?;
    let quadratic = variance
        .checked_mul(price_denominator)
        .ok_or(DealerError::ArithmeticOverflow)?;
    let numerator = linear
        .checked_add(quadratic)
        .ok_or(DealerError::ArithmeticOverflow)?;
    let denominator = b
        .checked_mul(2)
        .and_then(|value| value.checked_mul(n))
        .and_then(|value| value.checked_mul(price_denominator))
        .ok_or(DealerError::ArithmeticOverflow)?;
    i64::try_from(ceil_div_i128(numerator, denominator)?)
        .map_err(|_| DealerError::ArithmeticOverflow)
}

/// Exact rational instantaneous price vector of the signed dealer potential.
pub fn signed_quadratic_price_vector(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<PriceVectorV1> {
    policy.validate()?;
    validate_signed_inventory(policy, net_sold)?;
    let n = i128::from(policy.outcome_count);
    let b = i128::from(policy.depth_atoms);
    let price_denominator = i128::from(policy.initial_price_denominator);
    let (sum, _, _) = signed_moments(policy, net_sold)?;
    let denominator_i128 = b
        .checked_mul(n)
        .and_then(|value| value.checked_mul(price_denominator))
        .ok_or(DealerError::ArithmeticOverflow)?;
    let mut numerators = [0u128; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let value = i128::from(policy.initial_price_weights[i])
            .checked_mul(b)
            .and_then(|amount| amount.checked_mul(n))
            .and_then(|amount| {
                price_denominator
                    .checked_mul(
                        n.checked_mul(i128::from(net_sold[i]))
                            .and_then(|inner| inner.checked_sub(sum))?,
                    )
                    .and_then(|displacement| amount.checked_add(displacement))
            })
            .ok_or(DealerError::ArithmeticOverflow)?;
        numerators[i] = u128::try_from(value).map_err(|_| DealerError::PriceDomain)?;
        i += 1;
    }
    let result = PriceVectorV1 {
        numerators,
        denominator: u128::try_from(denominator_i128)
            .map_err(|_| DealerError::ArithmeticOverflow)?,
        outcome_count: policy.outcome_count,
    };
    result
        .validate()
        .map_err(|_| DealerError::InvariantViolation)?;
    Ok(result)
}

fn minimum_sponsor_subsidy_for_price(policy: &SignedDealerPolicyV1) -> DealerResult<u64> {
    validate_price_parameters(policy)?;
    let denominator = u128::from(policy.initial_price_denominator);
    let denominator_squared = denominator
        .checked_mul(denominator)
        .ok_or(DealerError::ArithmeticOverflow)?;
    let mut sum_squares = 0u128;
    let mut maximum_distance_numerator = 0u128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let weight = u128::from(policy.initial_price_weights[i]);
        sum_squares = sum_squares
            .checked_add(
                weight
                    .checked_mul(weight)
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    i = 0;
    while i < usize::from(policy.outcome_count) {
        let twice_vertex_dot = denominator
            .checked_mul(u128::from(policy.initial_price_weights[i]))
            .and_then(|value| value.checked_mul(2))
            .ok_or(DealerError::ArithmeticOverflow)?;
        let distance_numerator = denominator_squared
            .checked_add(sum_squares)
            .and_then(|value| value.checked_sub(twice_vertex_dot))
            .ok_or(DealerError::InvariantViolation)?;
        if distance_numerator > maximum_distance_numerator {
            maximum_distance_numerator = distance_numerator;
        }
        i += 1;
    }
    let numerator = u128::from(policy.depth_atoms)
        .checked_mul(maximum_distance_numerator)
        .ok_or(DealerError::ArithmeticOverflow)?;
    let divisor = denominator_squared
        .checked_mul(2)
        .ok_or(DealerError::ArithmeticOverflow)?;
    u64::try_from(ceil_div_u128(numerator, divisor)?).map_err(|_| DealerError::ArithmeticOverflow)
}

fn minimum_sponsor_capital_for_valid_policy(policy: &SignedDealerPolicyV1) -> DealerResult<u64> {
    let loss_subsidy = minimum_sponsor_subsidy_for_price(policy)?;
    let minimum_lp_cash =
        checked_product_u64(policy.capital_unit_cash_atoms, policy.minimum_lp_shares)?;
    let mut lower_corner = [0i64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        lower_corner[i] =
            -i64::try_from(policy.max_net_buy[i]).map_err(|_| DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    let lower_potential = signed_potential_for_valid_policy(policy, &lower_corner)?;
    let without_sponsor = i128::from(minimum_lp_cash)
        .checked_add(i128::from(lower_potential))
        .ok_or(DealerError::ArithmeticOverflow)?;
    let financing = if without_sponsor >= 0 {
        0
    } else {
        u64::try_from(
            without_sponsor
                .checked_neg()
                .ok_or(DealerError::ArithmeticOverflow)?,
        )
        .map_err(|_| DealerError::ArithmeticOverflow)?
    };
    Ok(core::cmp::max(loss_subsidy, financing))
}

fn validate_price_parameters(policy: &SignedDealerPolicyV1) -> DealerResult<()> {
    if policy.outcome_count < 2 || usize::from(policy.outcome_count) > MAX_OUTCOMES {
        return Err(DealerError::InvalidBasis);
    }
    if policy.payout_denominator == 0
        || policy.initial_price_denominator == 0
        || policy.depth_atoms == 0
    {
        return Err(DealerError::ZeroValue);
    }
    if policy.payout_denominator > MAX_ATOMS
        || policy.initial_price_denominator > MAX_PRICE_DENOMINATOR
        || policy.depth_atoms > MAX_ATOMS
    {
        return Err(DealerError::ParameterOutOfRange);
    }
    validate_padding_u64(policy.outcome_count, &policy.initial_price_weights)?;
    let mut sum = 0u128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        sum = sum
            .checked_add(u128::from(policy.initial_price_weights[i]))
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != u128::from(policy.initial_price_denominator) {
        return Err(DealerError::InvalidPriceVector);
    }
    Ok(())
}

fn validate_signed_inventory(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<()> {
    validate_padding_i64(policy.outcome_count, net_sold)?;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let value = i128::from(net_sold[i]);
        if value < -i128::from(policy.max_net_buy[i]) || value > i128::from(policy.max_net_sell[i])
        {
            return Err(DealerError::InventoryLimit);
        }
        if value % i128::from(policy.payout_denominator) != 0 {
            return Err(DealerError::NonIntegralLot);
        }
        i += 1;
    }
    validate_price_domain(policy, net_sold)
}

fn validate_price_domain(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<()> {
    let n = i128::from(policy.outcome_count);
    let b = i128::from(policy.depth_atoms);
    let price_denominator = i128::from(policy.initial_price_denominator);
    let (sum, _, _) = signed_moments(policy, net_sold)?;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let initial = i128::from(policy.initial_price_weights[i])
            .checked_mul(b)
            .and_then(|value| value.checked_mul(n))
            .ok_or(DealerError::ArithmeticOverflow)?;
        let displacement = price_denominator
            .checked_mul(
                n.checked_mul(i128::from(net_sold[i]))
                    .and_then(|value| value.checked_sub(sum))
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        if initial
            .checked_add(displacement)
            .ok_or(DealerError::ArithmeticOverflow)?
            < 0
        {
            return Err(DealerError::PriceDomain);
        }
        i += 1;
    }
    Ok(())
}

fn signed_moments(
    policy: &SignedDealerPolicyV1,
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<(i128, i128, i128)> {
    let mut sum = 0i128;
    let mut sum_squares = 0i128;
    let mut initial_dot = 0i128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let value = i128::from(net_sold[i]);
        sum = sum
            .checked_add(value)
            .ok_or(DealerError::ArithmeticOverflow)?;
        sum_squares = sum_squares
            .checked_add(
                value
                    .checked_mul(value)
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        initial_dot = initial_dot
            .checked_add(
                value
                    .checked_mul(i128::from(policy.initial_price_weights[i]))
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    Ok((sum, sum_squares, initial_dot))
}

fn funding_basket(
    policy: &SignedDealerPolicyV1,
    owner: Id,
    shares: u64,
) -> DealerResult<FundingBasketV1> {
    let mut eggs = [0; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        eggs[i] = checked_product_u64(policy.capital_unit_eggs[i], shares)?;
        i += 1;
    }
    Ok(FundingBasketV1 {
        owner,
        shares,
        cash_atoms: checked_product_u64(policy.capital_unit_cash_atoms, shares)?,
        eggs,
    })
}

fn holdings_from_net(
    policy: &SignedDealerPolicyV1,
    base_eggs: &[u64; MAX_OUTCOMES],
    net_sold: &[i64; MAX_OUTCOMES],
) -> DealerResult<[u64; MAX_OUTCOMES]> {
    let mut result = [0; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let value = i128::from(base_eggs[i])
            .checked_sub(i128::from(net_sold[i]))
            .ok_or(DealerError::ArithmeticOverflow)?;
        let amount = u64::try_from(value).map_err(|_| DealerError::InsufficientEggs)?;
        if amount > MAX_ATOMS {
            return Err(DealerError::ParameterOutOfRange);
        }
        result[i] = amount;
        i += 1;
    }
    Ok(result)
}

fn cash_with_potential(base_cash: u64, subsidy: u64, potential: i64) -> DealerResult<u64> {
    let value = i128::from(base_cash)
        .checked_add(i128::from(subsidy))
        .and_then(|amount| amount.checked_add(i128::from(potential)))
        .ok_or(DealerError::ArithmeticOverflow)?;
    u64::try_from(value).map_err(|_| DealerError::InsufficientCash)
}

fn validate_trade(policy: &SignedDealerPolicyV1, trade: &SignedDealerTradeV1) -> DealerResult<()> {
    validate_padding_u64(policy.outcome_count, &trade.sell_to_users)?;
    validate_padding_u64(policy.outcome_count, &trade.buy_from_users)?;
    let mut any = false;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        let sell = trade.sell_to_users[i];
        let buy = trade.buy_from_users[i];
        if sell != 0 && buy != 0 {
            return Err(DealerError::NonCanonicalFlow);
        }
        if !sell.is_multiple_of(policy.payout_denominator)
            || !buy.is_multiple_of(policy.payout_denominator)
        {
            return Err(DealerError::NonIntegralLot);
        }
        any |= sell != 0 || buy != 0;
        i += 1;
    }
    if !any {
        return Err(DealerError::NonCanonicalFlow);
    }
    Ok(())
}

fn apply_trade(
    outcome_count: u8,
    old: &[i64; MAX_OUTCOMES],
    trade: &SignedDealerTradeV1,
) -> DealerResult<[i64; MAX_OUTCOMES]> {
    let mut result = [0; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        let value = i128::from(old[i])
            .checked_add(i128::from(trade.sell_to_users[i]))
            .and_then(|amount| amount.checked_sub(i128::from(trade.buy_from_users[i])))
            .ok_or(DealerError::ArithmeticOverflow)?;
        result[i] = i64::try_from(value).map_err(|_| DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(result)
}

fn validate_unwind(
    outcome_count: u8,
    old: &[i64; MAX_OUTCOMES],
    new: &[i64; MAX_OUTCOMES],
) -> DealerResult<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        let admitted = if old[i] > 0 {
            new[i] >= 0 && new[i] <= old[i]
        } else if old[i] < 0 {
            new[i] <= 0 && new[i] >= old[i]
        } else {
            new[i] == 0
        };
        if !admitted {
            return Err(DealerError::IncreasesExposure);
        }
        i += 1;
    }
    Ok(())
}

fn validate_egg_flow(
    outcome_count: u8,
    old: &[u64; MAX_OUTCOMES],
    new: &[u64; MAX_OUTCOMES],
    trade: &SignedDealerTradeV1,
) -> DealerResult<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        let inputs = old[i]
            .checked_add(trade.buy_from_users[i])
            .ok_or(DealerError::ArithmeticOverflow)?;
        let outputs = new[i]
            .checked_add(trade.sell_to_users[i])
            .ok_or(DealerError::ArithmeticOverflow)?;
        if inputs != outputs {
            return Err(DealerError::InvariantViolation);
        }
        i += 1;
    }
    Ok(())
}

fn signed_cash_flow(difference: i64) -> DealerResult<(u64, u64)> {
    if difference >= 0 {
        Ok((
            u64::try_from(difference).map_err(|_| DealerError::ArithmeticOverflow)?,
            0,
        ))
    } else {
        let magnitude = i128::from(difference)
            .checked_neg()
            .ok_or(DealerError::ArithmeticOverflow)?;
        Ok((
            0,
            u64::try_from(magnitude).map_err(|_| DealerError::ArithmeticOverflow)?,
        ))
    }
}

fn validate_positions(state: &SignedDealerStateV1) -> DealerResult<(u64, u64)> {
    let mut shares = 0u64;
    let mut queued = 0u64;
    let mut i = 0usize;
    while i < MAX_LPS {
        let position = state.positions[i];
        if position.shares == 0 {
            if position != LpPositionV1::EMPTY {
                return Err(DealerError::InvariantViolation);
            }
        } else {
            check_id(position.owner)?;
            if position.owner == state.sponsor || position.owner == state.facility_id {
                return Err(DealerError::InvariantViolation);
            }
            if position.queued_shares > position.shares {
                return Err(DealerError::InvariantViolation);
            }
            let mut j = 0usize;
            while j < i {
                if state.positions[j].shares != 0 && state.positions[j].owner == position.owner {
                    return Err(DealerError::InvariantViolation);
                }
                j += 1;
            }
            shares = shares
                .checked_add(position.shares)
                .ok_or(DealerError::ArithmeticOverflow)?;
            queued = queued
                .checked_add(position.queued_shares)
                .ok_or(DealerError::ArithmeticOverflow)?;
        }
        i += 1;
    }
    Ok((shares, queued))
}

fn any_terminal_position(positions: &[LpPositionV1; MAX_LPS]) -> bool {
    let mut i = 0usize;
    while i < MAX_LPS {
        if positions[i].terminal_claim_atoms != 0 || positions[i].claimed {
            return true;
        }
        i += 1;
    }
    false
}

fn find_or_empty_position(positions: &[LpPositionV1; MAX_LPS], owner: Id) -> DealerResult<usize> {
    let mut empty = None;
    let mut i = 0usize;
    while i < MAX_LPS {
        if positions[i].shares != 0 && positions[i].owner == owner {
            return Ok(i);
        }
        if positions[i].shares == 0 && empty.is_none() {
            empty = Some(i);
        }
        i += 1;
    }
    empty.ok_or(DealerError::PositionLimit)
}

fn find_position(positions: &[LpPositionV1; MAX_LPS], owner: Id) -> DealerResult<usize> {
    let mut i = 0usize;
    while i < MAX_LPS {
        if positions[i].shares != 0 && positions[i].owner == owner {
            return Ok(i);
        }
        i += 1;
    }
    Err(DealerError::MismatchedOwner)
}

fn queue_threshold_met(
    policy: &SignedDealerPolicyV1,
    queued: u64,
    total: u64,
) -> DealerResult<bool> {
    if total == 0 {
        return Ok(false);
    }
    let left = u128::from(queued)
        .checked_mul(u128::from(policy.shutdown_queue_denominator))
        .ok_or(DealerError::ArithmeticOverflow)?;
    let right = u128::from(total)
        .checked_mul(u128::from(policy.shutdown_queue_numerator))
        .ok_or(DealerError::ArithmeticOverflow)?;
    Ok(left >= right)
}

fn allocate_terminal(
    total: u64,
    positions: &[LpPositionV1; MAX_LPS],
) -> DealerResult<[u64; MAX_LPS]> {
    let mut shares = 0u64;
    let mut i = 0usize;
    while i < MAX_LPS {
        shares = shares
            .checked_add(positions[i].shares)
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    if shares == 0 {
        return Err(DealerError::InvalidShares);
    }
    let mut result = [0u64; MAX_LPS];
    let mut remainders = [0u64; MAX_LPS];
    let mut assigned = 0u64;
    i = 0;
    while i < MAX_LPS {
        if positions[i].shares != 0 {
            let numerator = u128::from(total)
                .checked_mul(u128::from(positions[i].shares))
                .ok_or(DealerError::ArithmeticOverflow)?;
            result[i] = u64::try_from(numerator / u128::from(shares))
                .map_err(|_| DealerError::ArithmeticOverflow)?;
            remainders[i] = u64::try_from(numerator % u128::from(shares))
                .map_err(|_| DealerError::ArithmeticOverflow)?;
            assigned = assigned
                .checked_add(result[i])
                .ok_or(DealerError::ArithmeticOverflow)?;
        }
        i += 1;
    }
    let mut left = total
        .checked_sub(assigned)
        .ok_or(DealerError::InvariantViolation)?;
    let mut awarded = [false; MAX_LPS];
    while left != 0 {
        let mut winner = None;
        i = 0;
        while i < MAX_LPS {
            if positions[i].shares != 0 && !awarded[i] {
                winner = match winner {
                    None => Some(i),
                    Some(current) => {
                        if remainders[i] > remainders[current]
                            || (remainders[i] == remainders[current]
                                && positions[i].owner < positions[current].owner)
                        {
                            Some(i)
                        } else {
                            Some(current)
                        }
                    }
                };
            }
            i += 1;
        }
        let index = winner.ok_or(DealerError::InvariantViolation)?;
        result[index] = result[index]
            .checked_add(1)
            .ok_or(DealerError::ArithmeticOverflow)?;
        awarded[index] = true;
        left -= 1;
    }
    Ok(result)
}

fn validate_payout(
    policy: &SignedDealerPolicyV1,
    weights: &[u64; MAX_OUTCOMES],
) -> DealerResult<()> {
    validate_padding_u64(policy.outcome_count, weights)?;
    let mut sum = 0u128;
    let mut i = 0usize;
    while i < usize::from(policy.outcome_count) {
        sum = sum
            .checked_add(u128::from(weights[i]))
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != u128::from(policy.payout_denominator) {
        return Err(DealerError::InvalidPayoutVector);
    }
    Ok(())
}

fn exact_unsigned_payout(
    outcome_count: u8,
    eggs: &[u64; MAX_OUTCOMES],
    weights: &[u64; MAX_OUTCOMES],
    denominator: u64,
) -> DealerResult<u64> {
    let mut numerator = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        numerator = numerator
            .checked_add(
                u128::from(eggs[i])
                    .checked_mul(u128::from(weights[i]))
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    let denominator = u128::from(denominator);
    if !numerator.is_multiple_of(denominator) {
        return Err(DealerError::InvariantViolation);
    }
    u64::try_from(numerator / denominator).map_err(|_| DealerError::ArithmeticOverflow)
}

fn exact_signed_payout(
    outcome_count: u8,
    net_sold: &[i64; MAX_OUTCOMES],
    weights: &[u64; MAX_OUTCOMES],
    denominator: u64,
) -> DealerResult<i64> {
    let mut numerator = 0i128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        numerator = numerator
            .checked_add(
                i128::from(net_sold[i])
                    .checked_mul(i128::from(weights[i]))
                    .ok_or(DealerError::ArithmeticOverflow)?,
            )
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    let denominator = i128::from(denominator);
    if numerator % denominator != 0 {
        return Err(DealerError::InvariantViolation);
    }
    i64::try_from(numerator / denominator).map_err(|_| DealerError::ArithmeticOverflow)
}

fn add_eggs(
    outcome_count: u8,
    target: &mut [u64; MAX_OUTCOMES],
    values: &[u64; MAX_OUTCOMES],
) -> DealerResult<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        target[i] = target[i]
            .checked_add(values[i])
            .ok_or(DealerError::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(())
}

fn subtract_eggs(
    outcome_count: u8,
    target: &mut [u64; MAX_OUTCOMES],
    values: &[u64; MAX_OUTCOMES],
) -> DealerResult<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        target[i] = target[i]
            .checked_sub(values[i])
            .ok_or(DealerError::InsufficientEggs)?;
        i += 1;
    }
    Ok(())
}

fn validate_padding_u64(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> DealerResult<()> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(DealerError::InvalidBasis);
    }
    let mut i = usize::from(outcome_count);
    while i < MAX_OUTCOMES {
        if values[i] != 0 {
            return Err(DealerError::InvalidBasis);
        }
        i += 1;
    }
    Ok(())
}

fn validate_padding_i64(outcome_count: u8, values: &[i64; MAX_OUTCOMES]) -> DealerResult<()> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(DealerError::InvalidBasis);
    }
    let mut i = usize::from(outcome_count);
    while i < MAX_OUTCOMES {
        if values[i] != 0 {
            return Err(DealerError::InvalidBasis);
        }
        i += 1;
    }
    Ok(())
}

fn checked_product_u64(left: u64, right: u64) -> DealerResult<u64> {
    left.checked_mul(right)
        .ok_or(DealerError::ArithmeticOverflow)
        .and_then(|value| {
            if value > MAX_ATOMS {
                Err(DealerError::ParameterOutOfRange)
            } else {
                Ok(value)
            }
        })
}

fn ceil_div_u128(numerator: u128, denominator: u128) -> DealerResult<u128> {
    if denominator == 0 {
        return Err(DealerError::ZeroValue);
    }
    if numerator == 0 {
        return Ok(0);
    }
    numerator
        .checked_sub(1)
        .and_then(|value| value.checked_div(denominator))
        .and_then(|value| value.checked_add(1))
        .ok_or(DealerError::ArithmeticOverflow)
}

fn ceil_div_i128(numerator: i128, denominator: i128) -> DealerResult<i128> {
    if denominator <= 0 {
        return Err(DealerError::ZeroValue);
    }
    if numerator >= 0 {
        numerator
            .checked_add(denominator - 1)
            .and_then(|value| value.checked_div(denominator))
            .ok_or(DealerError::ArithmeticOverflow)
    } else {
        numerator
            .checked_div(denominator)
            .ok_or(DealerError::ArithmeticOverflow)
    }
}

fn bump(generation: &mut u64) -> DealerResult<()> {
    *generation = generation
        .checked_add(1)
        .ok_or(DealerError::ArithmeticOverflow)?;
    Ok(())
}

fn check_id(id: Id) -> DealerResult<()> {
    if id == [0; 32] {
        return Err(DealerError::InvalidIdentity);
    }
    Ok(())
}
