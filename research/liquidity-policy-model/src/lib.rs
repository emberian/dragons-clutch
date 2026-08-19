#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Fully funded, schedule-compiled passive range-liquidity semantics.
//!
//! This is an allocation-free executable **MODEL**, not a consensus codec or
//! live Solana transition. It owns immutable policy/tranche bindings, bounded
//! coefficient schedule compilation, conservative full-simplex liability,
//! quote reservation, tranche accounting, and exact fractional fee carry.
//! It does not authenticate digests, mint Eggs, move tokens, select a batch
//! candidate, or authorize settlement. See the crate README and
//! `MODEL_BOUNDARY.md` for the required integration authority.

/// Maximum active native B-spline Eggs in one modeled market.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum quote plans in one immutable schedule and tranche ledger.
pub const MAX_QUOTES: usize = 8;
/// Maximum tranches in one modeled fee-pot allocation.
pub const MAX_FEE_RECIPIENTS: usize = 8;
/// Largest collateral, inventory, share, fee-pot, or fee-credit value admitted.
///
/// Together with [`MAX_CARRY_DENOMINATOR`] this makes every documented `u128`
/// product smaller than `10^36 < 2^120`.
pub const MAX_ACCOUNTING_ATOMS: u64 = 1_000_000_000_000;
/// Largest accumulated capital-at-risk weight admitted for one tranche.
pub const MAX_CAPITAL_TIME_WEIGHT: u128 = 1_000_000_000_000;
/// Derived maximum aggregate weight across all eight fee recipients.
pub const MAX_AGGREGATE_FEE_WEIGHT: u128 = 8_000_000_000_000;
/// Frozen common denominator for every nonzero V1 fee carry.
pub const MAX_CARRY_DENOMINATOR: u128 = 1_000_000_000_000;

/// A fixed-width external identity or authenticated digest.
pub type Id = [u8; 32];

/// Canonical refusal from a checked model transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A required external identity is all zero.
    InvalidIdentity,
    /// The native Egg count or degree is outside the frozen bound.
    InvalidBasis,
    /// Fixed-capacity inactive entries are not canonical zero/empty padding.
    NonCanonicalPadding,
    /// A half-open range or triangle is empty or outside the native basis.
    InvalidRange,
    /// A required amount, denominator, quantity, or version is zero.
    ZeroValue,
    /// A value is outside the frozen arithmetic proof domain.
    ParameterOutOfRange,
    /// A checked fixed-width operation overflowed.
    ArithmeticOverflow,
    /// A policy, Terms, schedule, tranche, quote, or generation binding differs.
    MismatchedBinding,
    /// A time is outside the immutable batch interval or moves backwards.
    InvalidEpoch,
    /// A quote identifier is duplicated or no bounded slot remains.
    QuoteCapacity,
    /// The quote is not active or the requested fill is invalid.
    InvalidQuoteState,
    /// The requested clearing price violates the frozen all-in limit.
    LimitViolated,
    /// The transition would exceed the policy inventory vector.
    InventoryLimit,
    /// The transition would exceed the policy collateral cap.
    CollateralCap,
    /// Reserve cannot cover pending cash plus worst-case admitted liabilities.
    InsufficientReserve,
    /// A buy-back quote or fill could acquire more Eggs than outstanding debt.
    InsufficientInventory,
    /// The taker is the tranche owner, so the apparent volume is self-crossing.
    SelfCross,
    /// The share issue or terminal settlement needs an unnamed fractional atom.
    RemainderRequired,
    /// A withdrawal exceeds pro-rata equity or currently free collateral.
    WithdrawalLimit,
    /// The last owner accounting share cannot leave assets or fractional carry.
    LastShareLocked,
    /// A transition is inconsistent with the tranche's trading/resolved phase.
    InvalidPhase,
    /// A fee allocation has no positive integrated capital-at-risk weight.
    ZeroWeight,
    /// A fee allocation would be replayed or does not match the snapshotted state.
    FeeAllocationMismatch,
    /// New owner accounting shares cannot issue while exposure is live.
    ExposureActive,
    /// Reserve plus pending minimum sell proceeds exceeds its numeric domain.
    ReserveHeadroom,
    /// A supplied settlement vector is not the exact immutable integer simplex.
    InvalidPayoutVector,
    /// A cached accounting field disagrees with the authoritative quote ledger.
    InvariantViolation,
}

/// Result alias for total checked model operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Buy-back or fully backed write side of an ordinary Portfolio quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QuoteSide {
    /// Buy the exact coefficient portfolio to offset existing inventory.
    BuyOffset = 0,
    /// Sell a coefficient portfolio after its full delivery has been reserved.
    SellWrite = 1,
}

/// Immutable native B-spline Terms identity used by a liquidity policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTermsV1 {
    /// Market identity.
    pub market: Id,
    /// Digest of the complete immutable Terms bytes.
    pub terms_digest: Id,
    /// Native B-spline degree, exactly `0..=3`.
    pub basis_degree: u8,
    /// Number of active native Eggs.
    pub outcome_count: u8,
    /// Exact common settlement denominator.
    pub payout_denominator: u64,
}

impl NativeTermsV1 {
    /// Validate the nonzero identities and frozen native basis bounds.
    pub fn validate(&self) -> Result<()> {
        check_id(self.market)?;
        check_id(self.terms_digest)?;
        if self.basis_degree > 3
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
        {
            return Err(Error::InvalidBasis);
        }
        if self.payout_denominator == 0 {
            return Err(Error::ZeroValue);
        }
        Ok(())
    }
}

/// Immutable quote-generation and tranche-risk policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityPolicyV1 {
    /// Externally authenticated digest of this complete canonical policy.
    pub policy_id: Id,
    /// Exact native market and Terms identity.
    pub terms: NativeTermsV1,
    /// Digest of the admitted payoff/risk region artifact.
    pub payoff_region_digest: Id,
    /// Digest of the complete bounded quote schedule.
    pub quote_schedule_digest: Id,
    /// Maximum outstanding plus pending sell inventory, per native Egg.
    pub max_inventory: [u64; MAX_OUTCOMES],
    /// Maximum fresh contributed reserve and simultaneous encumbrance.
    ///
    /// Realized sell proceeds may make reserve exceed this cap, but can never
    /// expand the admitted liability envelope beyond it.
    pub collateral_cap: u64,
    /// First epoch in which a scheduled quote may be active.
    pub batch_start: u64,
    /// Last epoch in which a scheduled quote may be active or earn fee weight.
    pub batch_end: u64,
    /// Immutable fee allocation policy identity.
    pub fee_policy_id: Id,
    /// Immutable withdrawal convention identity.
    pub withdrawal_policy_id: Id,
    /// Frozen schedule compiler version.
    pub compiler_version: u32,
}

impl LiquidityPolicyV1 {
    /// Validate all immutable bindings, padding, and finite caps.
    pub fn validate(&self) -> Result<()> {
        check_id(self.policy_id)?;
        self.terms.validate()?;
        check_id(self.payoff_region_digest)?;
        check_id(self.quote_schedule_digest)?;
        check_id(self.fee_policy_id)?;
        check_id(self.withdrawal_policy_id)?;
        if self.collateral_cap == 0 || self.compiler_version == 0 {
            return Err(Error::ZeroValue);
        }
        if self.collateral_cap > MAX_ACCOUNTING_ATOMS {
            return Err(Error::ParameterOutOfRange);
        }
        if self.batch_start > self.batch_end || self.batch_end == u64::MAX {
            return Err(Error::InvalidEpoch);
        }
        let fee_window_epochs = u128::from(self.batch_end - self.batch_start) + 1;
        if fee_window_epochs
            .checked_mul(u128::from(self.collateral_cap))
            .ok_or(Error::ArithmeticOverflow)?
            > MAX_CAPITAL_TIME_WEIGHT
        {
            return Err(Error::ParameterOutOfRange);
        }
        validate_padding(self.terms.outcome_count, &self.max_inventory)?;
        let mut any = false;
        let mut i = 0usize;
        while i < usize::from(self.terms.outcome_count) {
            if self.max_inventory[i] > MAX_ACCOUNTING_ATOMS {
                return Err(Error::ParameterOutOfRange);
            }
            any |= self.max_inventory[i] != 0;
            i += 1;
        }
        if !any {
            return Err(Error::ZeroValue);
        }
        Ok(())
    }
}

/// A bounded coefficient shape compiled into native Egg atoms per lot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoefficientShapeV1 {
    /// Constant amount on a nonempty half-open native Egg range.
    HardRange {
        /// First included Egg.
        first: u8,
        /// First excluded Egg.
        end: u8,
        /// Egg atoms per lot inside the range.
        amount: u64,
    },
    /// Triangle sampled at integer Egg indices with exact floor interpolation.
    Triangle {
        /// Zero-height left anchor.
        left: u8,
        /// Full-height interior anchor.
        peak: u8,
        /// Zero-height right anchor.
        right: u8,
        /// Exact peak Egg atoms per lot.
        height: u64,
    },
    /// Exact precompiled bounded coefficient vector.
    Exact {
        /// Number of active entries, which must equal the Terms width.
        active_len: u8,
        /// Active coefficients followed by canonical zero padding.
        coefficients: [u64; MAX_OUTCOMES],
    },
}

/// One immutable rung in a bounded quote schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteRungV1 {
    /// Externally canonicalized ordinary order identity.
    pub quote_id: Id,
    /// Buy-back or backed-write side.
    pub side: QuoteSide,
    /// Exact native coefficient shape.
    pub shape: CoefficientShapeV1,
    /// Maximum lots offered by this rung.
    pub lots: u64,
    /// Per-lot all-in cash bound: buy ceiling or sell floor.
    pub limit_collateral_per_lot: u64,
    /// Smallest partial fill, except that the final remainder may be smaller.
    pub minimum_fill_lots: u64,
    /// First eligible epoch.
    pub start_epoch: u64,
    /// Last eligible epoch, inclusive.
    pub expiry_epoch: u64,
    /// Exact order replay generation.
    pub generation: u64,
}

/// Immutable bounded schedule with canonical empty padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteScheduleV1 {
    /// Digest which must equal the owning policy's schedule digest.
    pub schedule_digest: Id,
    /// Number of populated prefix entries.
    pub rung_count: u8,
    /// Populated prefix followed by `None` padding.
    pub rungs: [Option<QuoteRungV1>; MAX_QUOTES],
}

/// Exact ordinary Portfolio quote plan emitted by the bounded compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioQuotePlanV1 {
    /// Owning policy identity.
    pub policy_id: Id,
    /// Segregated tranche identity.
    pub tranche_id: Id,
    /// Exact market identity.
    pub market: Id,
    /// Exact immutable Terms digest.
    pub terms_digest: Id,
    /// Exact payoff-region artifact digest.
    pub payoff_region_digest: Id,
    /// Exact complete schedule digest.
    pub quote_schedule_digest: Id,
    /// Native basis degree.
    pub basis_degree: u8,
    /// Active native Egg width.
    pub active_len: u8,
    /// Exact quote/order identity.
    pub quote_id: Id,
    /// Ordinary Portfolio order side.
    pub side: QuoteSide,
    /// Exact native Egg coefficients per lot.
    pub coefficients: [u64; MAX_OUTCOMES],
    /// Maximum quote lots.
    pub lots: u64,
    /// Per-lot all-in limit in collateral atoms.
    pub limit_collateral_per_lot: u64,
    /// Minimum partial fill, with a final-remainder exception.
    pub minimum_fill_lots: u64,
    /// First eligible epoch.
    pub start_epoch: u64,
    /// Last eligible epoch, inclusive.
    pub expiry_epoch: u64,
    /// Exact replay generation.
    pub generation: u64,
}

/// Compiler output for one complete bounded schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledScheduleV1 {
    /// Number of populated plan entries.
    pub plan_count: u8,
    /// Populated prefix followed by canonical `None` padding.
    pub plans: [Option<PortfolioQuotePlanV1>; MAX_QUOTES],
}

/// Compile a policy-bound range/shape schedule into ordinary Portfolio plans.
///
/// The whole schedule is conservatively checked as if all sell rungs and all
/// buy cash ceilings were live together. Buy inventory availability is checked
/// later against a particular tranche state. No claim is minted here.
pub fn compile_schedule(
    policy: &LiquidityPolicyV1,
    tranche_id: Id,
    schedule: &QuoteScheduleV1,
) -> Result<CompiledScheduleV1> {
    policy.validate()?;
    check_id(tranche_id)?;
    if schedule.schedule_digest != policy.quote_schedule_digest {
        return Err(Error::MismatchedBinding);
    }
    let count = usize::from(schedule.rung_count);
    if count == 0 || count > MAX_QUOTES {
        return Err(Error::InvalidRange);
    }
    let mut plans = [None; MAX_QUOTES];
    let mut aggregate_sell = [0u64; MAX_OUTCOMES];
    let mut aggregate_buy = [0u64; MAX_OUTCOMES];
    let mut aggregate_sell_floor_cash = 0u64;
    let mut aggregate_buy_cash = 0u64;
    let mut index = 0usize;
    while index < MAX_QUOTES {
        if index < count {
            let rung = schedule.rungs[index].ok_or(Error::NonCanonicalPadding)?;
            check_id(rung.quote_id)?;
            let mut prior = 0usize;
            while prior < index {
                let other = schedule.rungs[prior].ok_or(Error::NonCanonicalPadding)?;
                if other.quote_id == rung.quote_id {
                    return Err(Error::QuoteCapacity);
                }
                prior += 1;
            }
            validate_rung(policy, &rung)?;
            let coefficients = compile_shape(policy.terms.outcome_count, rung.shape)?;
            match rung.side {
                QuoteSide::SellWrite => {
                    add_scaled(
                        &mut aggregate_sell,
                        &coefficients,
                        rung.lots,
                        policy.terms.outcome_count,
                    )?;
                    aggregate_sell_floor_cash = aggregate_sell_floor_cash
                        .checked_add(checked_mul(rung.lots, rung.limit_collateral_per_lot)?)
                        .ok_or(Error::ArithmeticOverflow)?;
                }
                QuoteSide::BuyOffset => {
                    add_scaled(
                        &mut aggregate_buy,
                        &coefficients,
                        rung.lots,
                        policy.terms.outcome_count,
                    )?;
                    aggregate_buy_cash = aggregate_buy_cash
                        .checked_add(checked_mul(rung.lots, rung.limit_collateral_per_lot)?)
                        .ok_or(Error::ArithmeticOverflow)?;
                }
            }
            plans[index] = Some(PortfolioQuotePlanV1 {
                policy_id: policy.policy_id,
                tranche_id,
                market: policy.terms.market,
                terms_digest: policy.terms.terms_digest,
                payoff_region_digest: policy.payoff_region_digest,
                quote_schedule_digest: policy.quote_schedule_digest,
                basis_degree: policy.terms.basis_degree,
                active_len: policy.terms.outcome_count,
                quote_id: rung.quote_id,
                side: rung.side,
                coefficients,
                lots: rung.lots,
                limit_collateral_per_lot: rung.limit_collateral_per_lot,
                minimum_fill_lots: rung.minimum_fill_lots,
                start_epoch: rung.start_epoch,
                expiry_epoch: rung.expiry_epoch,
                generation: rung.generation,
            });
        } else if schedule.rungs[index].is_some() {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    check_inventory_limit(policy, &[0; MAX_OUTCOMES], &aggregate_sell)?;
    check_inventory_limit(policy, &[0; MAX_OUTCOMES], &aggregate_buy)?;
    if aggregate_sell_floor_cash > MAX_ACCOUNTING_ATOMS {
        return Err(Error::ParameterOutOfRange);
    }
    let encumbered = maximum(policy.terms.outcome_count, &aggregate_sell)?
        .checked_add(aggregate_buy_cash)
        .ok_or(Error::ArithmeticOverflow)?;
    if encumbered > policy.collateral_cap {
        return Err(Error::CollateralCap);
    }
    Ok(CompiledScheduleV1 {
        plan_count: schedule.rung_count,
        plans,
    })
}

/// Exact fixed-grid nonnegative fractional collateral-atom carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCarry {
    /// Proper-fraction numerator.
    pub numerator: u128,
    /// [`MAX_CARRY_DENOMINATOR`] for nonzero carry; `1` for canonical zero.
    pub denominator: u128,
}

impl FractionalCarry {
    /// Canonical zero carry.
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// Check the frozen common-grid canonical representation.
    pub fn validate(&self) -> Result<()> {
        if self.denominator == 0 || self.numerator >= self.denominator {
            return Err(Error::InvariantViolation);
        }
        if self.denominator > MAX_CARRY_DENOMINATOR {
            return Err(Error::ParameterOutOfRange);
        }
        if self.numerator == 0 {
            if self.denominator != 1 {
                return Err(Error::NonCanonicalPadding);
            }
        } else if self.denominator != MAX_CARRY_DENOMINATOR {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(())
    }
}

/// Lifecycle phase for a segregated tranche.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TranchePhase {
    /// Quotes and fills may occur inside the policy interval.
    Trading = 0,
    /// Native liabilities were exactly settled; only withdrawals remain.
    Resolved = 1,
}

/// Lifecycle phase of one immutable quote slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QuoteStatus {
    /// Quote is admitted and owns its remaining reservation.
    Active = 0,
    /// Every lot filled.
    Filled = 1,
    /// Owner cancelled the unfilled remainder.
    Cancelled = 2,
    /// The unfilled remainder lapsed after its expiry.
    Lapsed = 3,
}

/// Persisted modeled quote and its remaining reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteLedgerV1 {
    /// Exact immutable compiled plan.
    pub plan: PortfolioQuotePlanV1,
    /// Lots not yet filled or released.
    pub remaining_lots: u64,
    /// Current lifecycle status.
    pub status: QuoteStatus,
}

/// A transfer recipe emitted by a successful partial or complete fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillReceiptV1 {
    /// Quote that was consumed.
    pub quote_id: Id,
    /// Lots filled.
    pub lots: u64,
    /// Cash credited to the tranche for a sell.
    pub collateral_credit_atoms: u64,
    /// Cash debited from the tranche for a buy-back.
    pub collateral_debit_atoms: u64,
    /// Exact Egg vector delivered by a sell or received by a buy-back.
    pub eggs: [u64; MAX_OUTCOMES],
    /// Post-transition tranche generation.
    pub tranche_generation: u64,
}

/// Segregated passive-liquidity tranche state.
///
/// `reserve_atoms` is LP-owned tranche value, not Hoard principal. `inventory`
/// is the nonnegative native Egg payout exposure already written. Active sell
/// quotes are added before taking the simplex maximum; active buy cash is then
/// added separately. The resulting stronger bound prevents pending orders from
/// being treated as future relief.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrancheStateV1 {
    /// Complete immutable quote-generation policy.
    pub policy: LiquidityPolicyV1,
    /// Immutable segregated tranche identity.
    pub tranche_id: Id,
    /// Beneficial LP owner identity used for self-cross refusal.
    pub owner: Id,
    /// Total segregated tranche collateral atoms.
    pub reserve_atoms: u64,
    /// Outstanding nonnegative native Egg payout exposure.
    pub inventory: [u64; MAX_OUTCOMES],
    /// Nontransferable owner accounting-share supply.
    pub lp_share_supply: u64,
    /// Cash ceiling reserved for all active buy-back quotes.
    pub reserved_buy_cash_atoms: u64,
    /// Minimum sell proceeds whose receipt must fit the reserve domain.
    pub reserved_sell_floor_cash_atoms: u64,
    /// Egg exposure reserved by all active write quotes.
    pub reserved_sell_inventory: [u64; MAX_OUTCOMES],
    /// Egg quantities reserved by all active buy-back quotes.
    pub reserved_buy_inventory: [u64; MAX_OUTCOMES],
    /// Integrated `(buy cash + H(q + executable pending sells)) * epochs`.
    ///
    /// This V1 model freezes the multiplier to one. Any non-unit multiplier is
    /// owned by a future authenticated fee-policy preimage, not an extra field
    /// beside `fee_policy_id`.
    pub capital_time_weight: u128,
    /// Epoch through which capital-at-risk has been integrated.
    pub last_weight_epoch: u64,
    /// Exact funded sub-atom remainder from the terminal fee allocation.
    pub fee_carry: FractionalCarry,
    /// Last consumed fee allocation generation.
    pub fee_allocation_generation: u64,
    /// Last consumed external fee-batch identity; zero before the first batch.
    pub last_fee_allocation_id: Id,
    /// Exact collateral paid when native inventory settled.
    pub settled_payout_atoms: u64,
    /// Monotone local transition generation.
    pub generation: u64,
    /// Trading or resolved phase.
    pub phase: TranchePhase,
    /// Bounded quote ledger; empty slots are canonical `None`.
    pub quotes: [Option<QuoteLedgerV1>; MAX_QUOTES],
}

impl TrancheStateV1 {
    /// Construct an empty segregated tranche at the policy's first epoch.
    pub fn initialize(policy: LiquidityPolicyV1, tranche_id: Id, owner: Id) -> Result<Self> {
        policy.validate()?;
        check_id(tranche_id)?;
        check_id(owner)?;
        if tranche_id == owner {
            return Err(Error::InvalidIdentity);
        }
        let value = Self {
            policy,
            tranche_id,
            owner,
            reserve_atoms: 0,
            inventory: [0; MAX_OUTCOMES],
            lp_share_supply: 0,
            reserved_buy_cash_atoms: 0,
            reserved_sell_floor_cash_atoms: 0,
            reserved_sell_inventory: [0; MAX_OUTCOMES],
            reserved_buy_inventory: [0; MAX_OUTCOMES],
            capital_time_weight: 0,
            last_weight_epoch: policy.batch_start,
            fee_carry: FractionalCarry::ZERO,
            fee_allocation_generation: 0,
            last_fee_allocation_id: [0; 32],
            settled_payout_atoms: 0,
            generation: 0,
            phase: TranchePhase::Trading,
            quotes: [None; MAX_QUOTES],
        };
        value.validate()?;
        Ok(value)
    }

    /// Recompute every cached reservation and solvency invariant.
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        check_id(self.tranche_id)?;
        check_id(self.owner)?;
        if self.tranche_id == self.owner || self.last_weight_epoch < self.policy.batch_start {
            return Err(Error::InvariantViolation);
        }
        if self.reserve_atoms > MAX_ACCOUNTING_ATOMS
            || self.lp_share_supply > MAX_ACCOUNTING_ATOMS
            || self.capital_time_weight > MAX_CAPITAL_TIME_WEIGHT
        {
            return Err(Error::ParameterOutOfRange);
        }
        self.fee_carry.validate()?;
        if (self.fee_allocation_generation == 0 && self.last_fee_allocation_id != [0; 32])
            || (self.fee_allocation_generation != 0 && self.last_fee_allocation_id == [0; 32])
        {
            return Err(Error::InvariantViolation);
        }
        validate_padding(self.policy.terms.outcome_count, &self.inventory)?;
        validate_padding(
            self.policy.terms.outcome_count,
            &self.reserved_sell_inventory,
        )?;
        validate_padding(
            self.policy.terms.outcome_count,
            &self.reserved_buy_inventory,
        )?;
        let mut sell = [0u64; MAX_OUTCOMES];
        let mut buy = [0u64; MAX_OUTCOMES];
        let mut sell_floor_cash = 0u64;
        let mut cash = 0u64;
        let mut slot = 0usize;
        while slot < MAX_QUOTES {
            if let Some(quote) = self.quotes[slot] {
                validate_plan(&self.policy, self.tranche_id, &quote.plan)?;
                if quote.remaining_lots > quote.plan.lots {
                    return Err(Error::InvariantViolation);
                }
                let mut prior = 0usize;
                while prior < slot {
                    if let Some(other) = self.quotes[prior] {
                        if other.plan.quote_id == quote.plan.quote_id {
                            return Err(Error::QuoteCapacity);
                        }
                    }
                    prior += 1;
                }
                if quote.status == QuoteStatus::Active {
                    if quote.remaining_lots == 0 {
                        return Err(Error::InvariantViolation);
                    }
                    match quote.plan.side {
                        QuoteSide::SellWrite => {
                            add_scaled(
                                &mut sell,
                                &quote.plan.coefficients,
                                quote.remaining_lots,
                                self.policy.terms.outcome_count,
                            )?;
                            sell_floor_cash = sell_floor_cash
                                .checked_add(checked_mul(
                                    quote.remaining_lots,
                                    quote.plan.limit_collateral_per_lot,
                                )?)
                                .ok_or(Error::ArithmeticOverflow)?;
                        }
                        QuoteSide::BuyOffset => {
                            add_scaled(
                                &mut buy,
                                &quote.plan.coefficients,
                                quote.remaining_lots,
                                self.policy.terms.outcome_count,
                            )?;
                            cash = cash
                                .checked_add(checked_mul(
                                    quote.remaining_lots,
                                    quote.plan.limit_collateral_per_lot,
                                )?)
                                .ok_or(Error::ArithmeticOverflow)?;
                        }
                    }
                } else if quote.remaining_lots != 0 {
                    return Err(Error::InvariantViolation);
                }
            }
            slot += 1;
        }
        if sell != self.reserved_sell_inventory
            || buy != self.reserved_buy_inventory
            || sell_floor_cash != self.reserved_sell_floor_cash_atoms
            || cash != self.reserved_buy_cash_atoms
        {
            return Err(Error::InvariantViolation);
        }
        check_inventory_limit(&self.policy, &self.inventory, &sell)?;
        ensure_componentwise_at_most(self.policy.terms.outcome_count, &buy, &self.inventory)?;
        let encumbered = self.encumbered_collateral()?;
        if encumbered > self.policy.collateral_cap {
            return Err(Error::CollateralCap);
        }
        if self.reserve_atoms < encumbered {
            return Err(Error::InsufficientReserve);
        }
        if self.reserved_sell_floor_cash_atoms > MAX_ACCOUNTING_ATOMS
            || self.reserve_atoms > MAX_ACCOUNTING_ATOMS - self.reserved_sell_floor_cash_atoms
        {
            return Err(Error::ReserveHeadroom);
        }
        if self.lp_share_supply == 0
            && (self.reserve_atoms != 0
                || any_nonzero(self.policy.terms.outcome_count, &self.inventory)
                || any_nonzero(
                    self.policy.terms.outcome_count,
                    &self.reserved_sell_inventory,
                )
                || self.reserved_buy_cash_atoms != 0
                || self.fee_carry != FractionalCarry::ZERO
                || self.capital_time_weight != 0)
        {
            return Err(Error::LastShareLocked);
        }
        if self.phase == TranchePhase::Resolved
            && (any_nonzero(self.policy.terms.outcome_count, &self.inventory)
                || any_nonzero(
                    self.policy.terms.outcome_count,
                    &self.reserved_sell_inventory,
                )
                || self.reserved_buy_cash_atoms != 0)
        {
            return Err(Error::InvalidPhase);
        }
        if self.phase == TranchePhase::Trading && self.settled_payout_atoms != 0 {
            return Err(Error::InvalidPhase);
        }
        Ok(())
    }

    /// Full-simplex liability `H_K(q) = max_i(q_i)` for settled inventory.
    pub fn inventory_liability(&self) -> Result<u64> {
        maximum(self.policy.terms.outcome_count, &self.inventory)
    }

    /// Strong quote-aware encumbrance: buy cash plus `H(q + pending sells)`.
    pub fn encumbered_collateral(&self) -> Result<u64> {
        let combined = checked_add_vectors(
            self.policy.terms.outcome_count,
            &self.inventory,
            &self.reserved_sell_inventory,
        )?;
        maximum(self.policy.terms.outcome_count, &combined)?
            .checked_add(self.reserved_buy_cash_atoms)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Currently withdrawable collateral before the pro-rata share cap.
    pub fn free_collateral(&self) -> Result<u64> {
        self.reserve_atoms
            .checked_sub(self.encumbered_collateral()?)
            .ok_or(Error::InsufficientReserve)
    }

    /// Conservative net equity numerator over `fee_carry.denominator`.
    pub fn conservative_equity_numerator(&self) -> Result<u128> {
        let base = self
            .reserve_atoms
            .checked_sub(self.inventory_liability()?)
            .ok_or(Error::InsufficientReserve)?;
        u128::from(base)
            .checked_mul(self.fee_carry.denominator)
            .and_then(|value| value.checked_add(self.fee_carry.numerator))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Integrate capital at risk through `epoch`, capped at `batch_end + 1`.
    pub fn accrue_risk(&mut self, epoch: u64) -> Result<()> {
        let mut next = *self;
        next.accrue_risk_inner(epoch)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Deposit owner collateral and mint exact pro-rata accounting shares.
    ///
    /// One immutable beneficial owner controls every V1 share; shares are not
    /// transferable holder claims. The first deposit mints one share per atom.
    /// Later deposits additionally require zero live exposure and exact integer
    /// shares. No issuance is admitted after the batch interval.
    pub fn deposit(&mut self, owner: Id, epoch: u64, collateral_atoms: u64) -> Result<u64> {
        check_id(owner)?;
        if owner != self.owner {
            return Err(Error::MismatchedBinding);
        }
        if collateral_atoms == 0 {
            return Err(Error::ZeroValue);
        }
        if epoch > self.policy.batch_end {
            return Err(Error::InvalidEpoch);
        }
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        if next.encumbered_collateral()? != 0 {
            return Err(Error::ExposureActive);
        }
        let new_reserve = next
            .reserve_atoms
            .checked_add(collateral_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        if new_reserve > next.policy.collateral_cap {
            return Err(Error::CollateralCap);
        }
        let minted = if next.lp_share_supply == 0 {
            if next.reserve_atoms != 0
                || any_nonzero(next.policy.terms.outcome_count, &next.inventory)
                || next.fee_carry != FractionalCarry::ZERO
            {
                return Err(Error::InvariantViolation);
            }
            collateral_atoms
        } else {
            let equity = next.conservative_equity_numerator()?;
            if equity == 0 {
                return Err(Error::InsufficientReserve);
            }
            let numerator = u128::from(collateral_atoms)
                .checked_mul(next.fee_carry.denominator)
                .and_then(|value| value.checked_mul(u128::from(next.lp_share_supply)))
                .ok_or(Error::ArithmeticOverflow)?;
            if numerator % equity != 0 {
                return Err(Error::RemainderRequired);
            }
            let shares = numerator / equity;
            if shares == 0 || shares > u128::from(MAX_ACCOUNTING_ATOMS) {
                return Err(Error::ParameterOutOfRange);
            }
            u64::try_from(shares).map_err(|_| Error::ArithmeticOverflow)?
        };
        next.reserve_atoms = new_reserve;
        next.lp_share_supply = next
            .lp_share_supply
            .checked_add(minted)
            .ok_or(Error::ArithmeticOverflow)?;
        if next.lp_share_supply > MAX_ACCOUNTING_ATOMS {
            return Err(Error::ParameterOutOfRange);
        }
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(minted)
    }

    /// Admit every plan in a compiled schedule as one atomic reservation step.
    pub fn admit_schedule(&mut self, epoch: u64, schedule: &CompiledScheduleV1) -> Result<()> {
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        let count = usize::from(schedule.plan_count);
        if count == 0 || count > MAX_QUOTES {
            return Err(Error::InvalidRange);
        }
        let mut index = 0usize;
        while index < MAX_QUOTES {
            if index < count {
                let plan = schedule.plans[index].ok_or(Error::NonCanonicalPadding)?;
                next.admit_plan_inner(epoch, plan)?;
            } else if schedule.plans[index].is_some() {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Admit one compiler-produced plan as an atomic reservation step.
    ///
    /// This permits a frozen two-sided schedule to replenish its buy-back rung
    /// only after sell fills create offsettable inventory. The future adapter
    /// must prove membership in the authenticated complete schedule artifact;
    /// this model checks every copied schedule and policy digest but owns no
    /// Merkle or account authentication.
    pub fn admit_plan(&mut self, epoch: u64, plan: PortfolioQuotePlanV1) -> Result<()> {
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        next.admit_plan_inner(epoch, plan)?;
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Fill part or all of one active quote and return the exact transfer recipe.
    pub fn fill_quote(
        &mut self,
        epoch: u64,
        quote_id: Id,
        taker: Id,
        lots: u64,
        clearing_collateral_per_lot: u64,
    ) -> Result<FillReceiptV1> {
        check_id(quote_id)?;
        check_id(taker)?;
        if taker == self.owner || taker == self.tranche_id {
            return Err(Error::SelfCross);
        }
        if lots == 0 {
            return Err(Error::ZeroValue);
        }
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        let slot = next.active_quote_slot(quote_id)?;
        let mut quote = next.quotes[slot].ok_or(Error::InvalidQuoteState)?;
        if epoch < quote.plan.start_epoch || epoch > quote.plan.expiry_epoch {
            return Err(Error::InvalidEpoch);
        }
        if lots > quote.remaining_lots
            || (lots < quote.plan.minimum_fill_lots && lots != quote.remaining_lots)
        {
            return Err(Error::InvalidQuoteState);
        }
        match quote.plan.side {
            QuoteSide::BuyOffset
                if clearing_collateral_per_lot > quote.plan.limit_collateral_per_lot =>
            {
                return Err(Error::LimitViolated);
            }
            QuoteSide::SellWrite
                if clearing_collateral_per_lot < quote.plan.limit_collateral_per_lot =>
            {
                return Err(Error::LimitViolated);
            }
            _ => {}
        }
        let consideration = checked_mul(lots, clearing_collateral_per_lot)?;
        let eggs = scaled_vector(
            next.policy.terms.outcome_count,
            &quote.plan.coefficients,
            lots,
        )?;
        match quote.plan.side {
            QuoteSide::SellWrite => {
                let released_floor = checked_mul(lots, quote.plan.limit_collateral_per_lot)?;
                next.reserved_sell_floor_cash_atoms = next
                    .reserved_sell_floor_cash_atoms
                    .checked_sub(released_floor)
                    .ok_or(Error::InvariantViolation)?;
                subtract_vector(
                    next.policy.terms.outcome_count,
                    &mut next.reserved_sell_inventory,
                    &eggs,
                )?;
                add_vector(next.policy.terms.outcome_count, &mut next.inventory, &eggs)?;
                next.reserve_atoms = next
                    .reserve_atoms
                    .checked_add(consideration)
                    .ok_or(Error::ArithmeticOverflow)?;
                if next.reserve_atoms > MAX_ACCOUNTING_ATOMS - next.reserved_sell_floor_cash_atoms {
                    return Err(Error::ReserveHeadroom);
                }
            }
            QuoteSide::BuyOffset => {
                let released_ceiling = checked_mul(lots, quote.plan.limit_collateral_per_lot)?;
                next.reserved_buy_cash_atoms = next
                    .reserved_buy_cash_atoms
                    .checked_sub(released_ceiling)
                    .ok_or(Error::InvariantViolation)?;
                subtract_vector(
                    next.policy.terms.outcome_count,
                    &mut next.reserved_buy_inventory,
                    &eggs,
                )?;
                subtract_vector(next.policy.terms.outcome_count, &mut next.inventory, &eggs)?;
                next.reserve_atoms = next
                    .reserve_atoms
                    .checked_sub(consideration)
                    .ok_or(Error::InsufficientReserve)?;
            }
        }
        quote.remaining_lots -= lots;
        if quote.remaining_lots == 0 {
            quote.status = QuoteStatus::Filled;
        }
        next.quotes[slot] = Some(quote);
        next.bump_generation()?;
        next.validate()?;
        let receipt = FillReceiptV1 {
            quote_id,
            lots,
            collateral_credit_atoms: if quote.plan.side == QuoteSide::SellWrite {
                consideration
            } else {
                0
            },
            collateral_debit_atoms: if quote.plan.side == QuoteSide::BuyOffset {
                consideration
            } else {
                0
            },
            eggs,
            tranche_generation: next.generation,
        };
        *self = next;
        Ok(receipt)
    }

    /// Cancel and release an owner-controlled active quote.
    pub fn cancel_quote(&mut self, owner: Id, epoch: u64, quote_id: Id) -> Result<()> {
        check_id(owner)?;
        if owner != self.owner {
            return Err(Error::MismatchedBinding);
        }
        self.release_quote(epoch, quote_id, QuoteStatus::Cancelled, false)
    }

    /// Release an unfilled quote only after its inclusive expiry has passed.
    pub fn lapse_quote(&mut self, epoch: u64, quote_id: Id) -> Result<()> {
        self.release_quote(epoch, quote_id, QuoteStatus::Lapsed, true)
    }

    /// Burn owner accounting shares within pro-rata equity and free cash.
    pub fn withdraw(
        &mut self,
        owner: Id,
        epoch: u64,
        burn_shares: u64,
        collateral_atoms: u64,
    ) -> Result<()> {
        check_id(owner)?;
        if owner != self.owner {
            return Err(Error::MismatchedBinding);
        }
        if burn_shares == 0 || collateral_atoms == 0 {
            return Err(Error::ZeroValue);
        }
        let mut next = *self;
        next.accrue_risk_inner(epoch)?;
        if burn_shares > next.lp_share_supply {
            return Err(Error::WithdrawalLimit);
        }
        if burn_shares == next.lp_share_supply {
            if next.inventory_liability()? != 0
                || next.encumbered_collateral()? != 0
                || next.fee_carry != FractionalCarry::ZERO
                || next.capital_time_weight != 0
                || collateral_atoms != next.reserve_atoms
            {
                return Err(Error::LastShareLocked);
            }
        } else {
            let equity = next.conservative_equity_numerator()?;
            let denominator = u128::from(next.lp_share_supply)
                .checked_mul(next.fee_carry.denominator)
                .ok_or(Error::ArithmeticOverflow)?;
            let entitlement = equity
                .checked_mul(u128::from(burn_shares))
                .ok_or(Error::ArithmeticOverflow)?
                / denominator;
            if u128::from(collateral_atoms) > entitlement {
                return Err(Error::WithdrawalLimit);
            }
        }
        if collateral_atoms > next.free_collateral()? {
            return Err(Error::WithdrawalLimit);
        }
        next.reserve_atoms -= collateral_atoms;
        next.lp_share_supply -= burn_shares;
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Snapshot accumulated weight for the one terminal fee allocation.
    pub fn fee_input(&self) -> FeeAllocationInputV1 {
        FeeAllocationInputV1 {
            tranche_id: self.tranche_id,
            owner: self.owner,
            fee_policy_id: self.policy.fee_policy_id,
            snapshot_epoch: self.last_weight_epoch,
            fee_window_end: self.policy.batch_end + 1,
            lp_share_supply: self.lp_share_supply,
            reserve_atoms: self.reserve_atoms,
            fee_allocation_generation: self.fee_allocation_generation,
            last_fee_allocation_id: self.last_fee_allocation_id,
            tranche_generation: self.generation,
            capital_time_weight: self.capital_time_weight,
            carry: self.fee_carry,
        }
    }

    /// Consume the authority-produced terminal fee allocation exactly once.
    pub fn apply_fee_allocation(
        &mut self,
        allocation_generation: u64,
        output: FeeAllocationOutputV1,
    ) -> Result<()> {
        let mut next = *self;
        let expected_generation = next
            .fee_allocation_generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if allocation_generation != expected_generation
            || output.allocation_id == next.last_fee_allocation_id
            || output.tranche_id != next.tranche_id
            || output.owner != next.owner
            || output.fee_policy_id != next.policy.fee_policy_id
            || output.snapshot_epoch != next.last_weight_epoch
            || output.fee_window_end != next.policy.batch_end + 1
            || output.lp_share_supply != next.lp_share_supply
            || output.reserve_atoms != next.reserve_atoms
            || output.fee_allocation_generation != next.fee_allocation_generation
            || output.last_fee_allocation_id != next.last_fee_allocation_id
            || output.tranche_generation != next.generation
            || output.consumed_weight != next.capital_time_weight
            || output.old_carry != next.fee_carry
        {
            return Err(Error::FeeAllocationMismatch);
        }
        output.new_carry.validate()?;
        next.fee_carry = output.new_carry;
        next.capital_time_weight = 0;
        next.fee_allocation_generation = allocation_generation;
        next.last_fee_allocation_id = output.allocation_id;
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Settle all outstanding inventory against one exact native payout vector.
    ///
    /// Every quote must already be filled, cancelled, or lapsed. A fractional
    /// collateral-atom result refuses instead of flooring.
    pub fn settle(
        &mut self,
        epoch: u64,
        payout_denominator: u64,
        payout_weights: [u64; MAX_OUTCOMES],
    ) -> Result<u64> {
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        if epoch <= next.policy.batch_end {
            return Err(Error::InvalidEpoch);
        }
        if next.reserved_buy_cash_atoms != 0
            || any_nonzero(
                next.policy.terms.outcome_count,
                &next.reserved_sell_inventory,
            )
        {
            return Err(Error::InvalidQuoteState);
        }
        validate_payout(
            next.policy.terms.outcome_count,
            next.policy.terms.payout_denominator,
            payout_denominator,
            &payout_weights,
        )?;
        let numerator = dot(
            next.policy.terms.outcome_count,
            &next.inventory,
            &payout_weights,
        )?;
        if numerator % u128::from(payout_denominator) != 0 {
            return Err(Error::RemainderRequired);
        }
        let payout = numerator / u128::from(payout_denominator);
        if payout > u128::from(u64::MAX) {
            return Err(Error::ArithmeticOverflow);
        }
        let payout = u64::try_from(payout).map_err(|_| Error::ArithmeticOverflow)?;
        next.reserve_atoms = next
            .reserve_atoms
            .checked_sub(payout)
            .ok_or(Error::InsufficientReserve)?;
        next.inventory = [0; MAX_OUTCOMES];
        next.settled_payout_atoms = next
            .settled_payout_atoms
            .checked_add(payout)
            .ok_or(Error::ArithmeticOverflow)?;
        next.phase = TranchePhase::Resolved;
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(payout)
    }

    fn require_trading(&self) -> Result<()> {
        if self.phase != TranchePhase::Trading {
            return Err(Error::InvalidPhase);
        }
        Ok(())
    }

    fn accrue_risk_inner(&mut self, epoch: u64) -> Result<()> {
        if epoch < self.last_weight_epoch {
            return Err(Error::InvalidEpoch);
        }
        let fee_window_end = self
            .policy
            .batch_end
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let accrual_end = if epoch < fee_window_end {
            epoch
        } else {
            fee_window_end
        };
        let mut cursor = if self.last_weight_epoch < accrual_end {
            self.last_weight_epoch
        } else {
            accrual_end
        };
        while cursor < accrual_end {
            let mut boundary = accrual_end;
            let mut slot = 0usize;
            while slot < MAX_QUOTES {
                if let Some(quote) = self.quotes[slot] {
                    if quote.status == QuoteStatus::Active && quote.plan.expiry_epoch >= cursor {
                        let after_expiry = quote
                            .plan
                            .expiry_epoch
                            .checked_add(1)
                            .ok_or(Error::ArithmeticOverflow)?;
                        if after_expiry < boundary {
                            boundary = after_expiry;
                        }
                    }
                }
                slot += 1;
            }
            let exposure = self.risk_exposure_at(cursor)?;
            let increment = u128::from(boundary - cursor)
                .checked_mul(u128::from(exposure))
                .ok_or(Error::ArithmeticOverflow)?;
            self.capital_time_weight = self
                .capital_time_weight
                .checked_add(increment)
                .ok_or(Error::ArithmeticOverflow)?;
            if self.capital_time_weight > MAX_CAPITAL_TIME_WEIGHT {
                return Err(Error::ParameterOutOfRange);
            }
            cursor = boundary;
        }
        self.last_weight_epoch = epoch;
        Ok(())
    }

    fn risk_exposure_at(&self, epoch: u64) -> Result<u64> {
        let mut combined = self.inventory;
        let mut buy_cash = 0u64;
        let mut slot = 0usize;
        while slot < MAX_QUOTES {
            if let Some(quote) = self.quotes[slot] {
                if quote.status == QuoteStatus::Active
                    && quote.plan.start_epoch <= epoch
                    && epoch <= quote.plan.expiry_epoch
                {
                    match quote.plan.side {
                        QuoteSide::SellWrite => add_scaled(
                            &mut combined,
                            &quote.plan.coefficients,
                            quote.remaining_lots,
                            self.policy.terms.outcome_count,
                        )?,
                        QuoteSide::BuyOffset => {
                            buy_cash = buy_cash
                                .checked_add(checked_mul(
                                    quote.remaining_lots,
                                    quote.plan.limit_collateral_per_lot,
                                )?)
                                .ok_or(Error::ArithmeticOverflow)?;
                        }
                    }
                }
            }
            slot += 1;
        }
        maximum(self.policy.terms.outcome_count, &combined)?
            .checked_add(buy_cash)
            .ok_or(Error::ArithmeticOverflow)
    }

    fn admit_plan_inner(&mut self, epoch: u64, plan: PortfolioQuotePlanV1) -> Result<()> {
        validate_plan(&self.policy, self.tranche_id, &plan)?;
        if epoch < plan.start_epoch || epoch > plan.expiry_epoch {
            return Err(Error::InvalidEpoch);
        }
        let mut free_slot = None;
        let mut index = 0usize;
        while index < MAX_QUOTES {
            match self.quotes[index] {
                Some(existing) if existing.plan.quote_id == plan.quote_id => {
                    return Err(Error::QuoteCapacity);
                }
                None if free_slot.is_none() => free_slot = Some(index),
                _ => {}
            }
            index += 1;
        }
        let slot = free_slot.ok_or(Error::QuoteCapacity)?;
        match plan.side {
            QuoteSide::SellWrite => {
                add_scaled(
                    &mut self.reserved_sell_inventory,
                    &plan.coefficients,
                    plan.lots,
                    self.policy.terms.outcome_count,
                )?;
                self.reserved_sell_floor_cash_atoms = self
                    .reserved_sell_floor_cash_atoms
                    .checked_add(checked_mul(plan.lots, plan.limit_collateral_per_lot)?)
                    .ok_or(Error::ArithmeticOverflow)?;
                if self.reserved_sell_floor_cash_atoms > MAX_ACCOUNTING_ATOMS
                    || self.reserve_atoms
                        > MAX_ACCOUNTING_ATOMS - self.reserved_sell_floor_cash_atoms
                {
                    return Err(Error::ReserveHeadroom);
                }
            }
            QuoteSide::BuyOffset => {
                add_scaled(
                    &mut self.reserved_buy_inventory,
                    &plan.coefficients,
                    plan.lots,
                    self.policy.terms.outcome_count,
                )?;
                ensure_componentwise_at_most(
                    self.policy.terms.outcome_count,
                    &self.reserved_buy_inventory,
                    &self.inventory,
                )?;
                self.reserved_buy_cash_atoms = self
                    .reserved_buy_cash_atoms
                    .checked_add(checked_mul(plan.lots, plan.limit_collateral_per_lot)?)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        self.quotes[slot] = Some(QuoteLedgerV1 {
            plan,
            remaining_lots: plan.lots,
            status: QuoteStatus::Active,
        });
        Ok(())
    }

    fn active_quote_slot(&self, quote_id: Id) -> Result<usize> {
        let mut index = 0usize;
        while index < MAX_QUOTES {
            if let Some(quote) = self.quotes[index] {
                if quote.plan.quote_id == quote_id {
                    return if quote.status == QuoteStatus::Active {
                        Ok(index)
                    } else {
                        Err(Error::InvalidQuoteState)
                    };
                }
            }
            index += 1;
        }
        Err(Error::InvalidQuoteState)
    }

    fn release_quote(
        &mut self,
        epoch: u64,
        quote_id: Id,
        final_status: QuoteStatus,
        require_expired: bool,
    ) -> Result<()> {
        check_id(quote_id)?;
        let mut next = *self;
        next.require_trading()?;
        next.accrue_risk_inner(epoch)?;
        let slot = next.active_quote_slot(quote_id)?;
        let mut quote = next.quotes[slot].ok_or(Error::InvalidQuoteState)?;
        if require_expired && epoch <= quote.plan.expiry_epoch {
            return Err(Error::InvalidEpoch);
        }
        let released = scaled_vector(
            next.policy.terms.outcome_count,
            &quote.plan.coefficients,
            quote.remaining_lots,
        )?;
        match quote.plan.side {
            QuoteSide::SellWrite => {
                subtract_vector(
                    next.policy.terms.outcome_count,
                    &mut next.reserved_sell_inventory,
                    &released,
                )?;
                next.reserved_sell_floor_cash_atoms = next
                    .reserved_sell_floor_cash_atoms
                    .checked_sub(checked_mul(
                        quote.remaining_lots,
                        quote.plan.limit_collateral_per_lot,
                    )?)
                    .ok_or(Error::InvariantViolation)?;
            }
            QuoteSide::BuyOffset => {
                subtract_vector(
                    next.policy.terms.outcome_count,
                    &mut next.reserved_buy_inventory,
                    &released,
                )?;
                next.reserved_buy_cash_atoms = next
                    .reserved_buy_cash_atoms
                    .checked_sub(checked_mul(
                        quote.remaining_lots,
                        quote.plan.limit_collateral_per_lot,
                    )?)
                    .ok_or(Error::InvariantViolation)?;
            }
        }
        quote.remaining_lots = 0;
        quote.status = final_status;
        next.quotes[slot] = Some(quote);
        next.bump_generation()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn bump_generation(&mut self) -> Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// One tranche's exact input to a fee-pot allocation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeAllocationInputV1 {
    /// Segregated tranche identity.
    pub tranche_id: Id,
    /// Immutable beneficial owner paid every whole-atom fee credit directly.
    pub owner: Id,
    /// Immutable fee policy identity shared by the allocation set.
    pub fee_policy_id: Id,
    /// Epoch through which this weight was integrated.
    pub snapshot_epoch: u64,
    /// First epoch after the immutable fee window.
    pub fee_window_end: u64,
    /// Owner accounting-share supply entitled to this historical weight.
    pub lp_share_supply: u64,
    /// Exact reserve bound into the pre-state snapshot.
    pub reserve_atoms: u64,
    /// Last consumed fee-allocation generation at this snapshot.
    pub fee_allocation_generation: u64,
    /// Last consumed allocation identity, or zero before the first allocation.
    pub last_fee_allocation_id: Id,
    /// Exact tranche generation at the terminal fee snapshot.
    pub tranche_generation: u64,
    /// Frozen time-integrated capital-at-risk weight.
    pub capital_time_weight: u128,
    /// Carry entering this allocation; terminal V1 requires canonical zero.
    pub carry: FractionalCarry,
}

/// Direct owner credit and funded terminal fractional remainder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeAllocationOutputV1 {
    /// Externally authenticated identity of the realized fee-pot allocation.
    allocation_id: Id,
    /// Segregated tranche identity.
    tranche_id: Id,
    /// Immutable beneficial owner receiving the direct whole-atom payout.
    owner: Id,
    /// Immutable fee policy identity authenticated by the input.
    fee_policy_id: Id,
    /// Epoch through which the consumed weight was integrated.
    snapshot_epoch: u64,
    /// First epoch after the immutable fee window.
    fee_window_end: u64,
    /// Owner accounting-share supply entitled to this historical weight.
    lp_share_supply: u64,
    /// Exact reserve authenticated by this state snapshot.
    reserve_atoms: u64,
    /// Last consumed fee-allocation generation at the snapshot.
    fee_allocation_generation: u64,
    /// Last consumed allocation identity at the snapshot.
    last_fee_allocation_id: Id,
    /// Exact tranche generation at the terminal fee snapshot.
    tranche_generation: u64,
    /// Weight consumed by this output.
    consumed_weight: u128,
    /// Carry authenticated by the allocation input.
    old_carry: FractionalCarry,
    /// Whole collateral atoms paid directly to the immutable owner.
    credited_atoms: u64,
    /// Fixed-grid terminal fraction backed by retained carry escrow.
    new_carry: FractionalCarry,
}

impl FeeAllocationOutputV1 {
    /// Externally authenticated identity of the realized fee-pot allocation.
    pub const fn allocation_id(&self) -> Id {
        self.allocation_id
    }

    /// Segregated tranche identity.
    pub const fn tranche_id(&self) -> Id {
        self.tranche_id
    }

    /// Immutable beneficial owner receiving the direct whole-atom payout.
    pub const fn owner(&self) -> Id {
        self.owner
    }

    /// Immutable fee policy identity.
    pub const fn fee_policy_id(&self) -> Id {
        self.fee_policy_id
    }

    /// Epoch through which the consumed weight was integrated.
    pub const fn snapshot_epoch(&self) -> u64 {
        self.snapshot_epoch
    }

    /// First epoch after the immutable fee window.
    pub const fn fee_window_end(&self) -> u64 {
        self.fee_window_end
    }

    /// Owner accounting-share supply entitled to this historical weight.
    pub const fn lp_share_supply(&self) -> u64 {
        self.lp_share_supply
    }

    /// Reserve authenticated by the state snapshot.
    pub const fn reserve_atoms(&self) -> u64 {
        self.reserve_atoms
    }

    /// Last consumed fee-allocation generation at the snapshot.
    pub const fn fee_allocation_generation(&self) -> u64 {
        self.fee_allocation_generation
    }

    /// Last consumed allocation identity at the snapshot.
    pub const fn last_fee_allocation_id(&self) -> Id {
        self.last_fee_allocation_id
    }

    /// Exact tranche generation at the terminal fee snapshot.
    pub const fn tranche_generation(&self) -> u64 {
        self.tranche_generation
    }

    /// Time-integrated capital-at-risk weight consumed by this output.
    pub const fn consumed_weight(&self) -> u128 {
        self.consumed_weight
    }

    /// Exact carry entering this allocation.
    pub const fn old_carry(&self) -> FractionalCarry {
        self.old_carry
    }

    /// Whole collateral atoms paid directly to the immutable owner.
    pub const fn credited_atoms(&self) -> u64 {
        self.credited_atoms
    }

    /// Exact fixed-grid terminal carry after this allocation.
    pub const fn new_carry(&self) -> FractionalCarry {
        self.new_carry
    }
}

/// Complete bounded fee allocation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeAllocationBatchV1 {
    /// Externally authenticated identity of this realized-pot allocation.
    allocation_id: Id,
    /// Number of populated outputs.
    recipient_count: u8,
    /// Exact allocated fee pot.
    fee_pot_atoms: u64,
    /// Sum of all positive capital-at-risk weights.
    total_weight: u128,
    /// Whole atoms entering from prior carry; always zero in terminal V1.
    prior_carry_escrow_atoms: u64,
    /// Whole atoms retained to back every new fractional carry exactly.
    retained_carry_escrow_atoms: u64,
    /// Sum of whole-atom credits across the complete recipient set.
    credited_atoms: u64,
    /// Populated prefix followed by canonical `None` padding.
    outputs: [Option<FeeAllocationOutputV1>; MAX_FEE_RECIPIENTS],
}

impl FeeAllocationBatchV1 {
    /// Externally authenticated identity of this realized-pot allocation.
    pub const fn allocation_id(&self) -> Id {
        self.allocation_id
    }

    /// Number of populated outputs.
    pub const fn recipient_count(&self) -> u8 {
        self.recipient_count
    }

    /// Exact realized fee pot allocated by this batch.
    pub const fn fee_pot_atoms(&self) -> u64 {
        self.fee_pot_atoms
    }

    /// Sum of all input capital-at-risk weights.
    pub const fn total_weight(&self) -> u128 {
        self.total_weight
    }

    /// Whole atoms entering from prior carry; always zero in terminal V1.
    pub const fn prior_carry_escrow_atoms(&self) -> u64 {
        self.prior_carry_escrow_atoms
    }

    /// Whole atoms retained to back all output fractional carries.
    pub const fn retained_carry_escrow_atoms(&self) -> u64 {
        self.retained_carry_escrow_atoms
    }

    /// Sum of whole collateral credits across all outputs.
    pub const fn credited_atoms(&self) -> u64 {
        self.credited_atoms
    }

    /// Return one immutable output in the populated prefix.
    pub fn output(&self, index: usize) -> Option<FeeAllocationOutputV1> {
        if index < usize::from(self.recipient_count) {
            self.outputs[index]
        } else {
            None
        }
    }
}

/// Apportion the one terminal realized fee pot on a frozen common grid.
///
/// Raw weights are aggregated by beneficial owner and Hamilton-normalized to
/// exactly `10^12` units, with remainder ties broken by owner identity. Each
/// direct owner credit is the whole part of `pot * units / 10^12`; the terminal
/// fraction uses that common denominator.
/// Inputs must be after the fee window, have unique tranche identities, and have
/// no prior allocation or carry. Inputs with the same beneficial owner are
/// aggregated before apportionment; their credit and carry are assigned to that
/// owner's lexicographically smallest tranche identity.
/// [`FeeAllocationBatchV1`] checks `pot = direct credits + retained carry escrow`.
/// The external authority must
/// own that pot, pay every bound owner, retain the reported escrow, and consume
/// every output atomically; this model does not authenticate that account set.
pub fn allocate_fee_pot(
    allocation_id: Id,
    fee_pot_atoms: u64,
    recipient_count: u8,
    inputs: &[Option<FeeAllocationInputV1>; MAX_FEE_RECIPIENTS],
) -> Result<FeeAllocationBatchV1> {
    check_id(allocation_id)?;
    if fee_pot_atoms > MAX_ACCOUNTING_ATOMS {
        return Err(Error::ParameterOutOfRange);
    }
    let count = usize::from(recipient_count);
    if count == 0 || count > MAX_FEE_RECIPIENTS {
        return Err(Error::ZeroValue);
    }
    let mut total_weight = 0u128;
    let mut index = 0usize;
    while index < MAX_FEE_RECIPIENTS {
        if index < count {
            let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
            check_id(input.tranche_id)?;
            check_id(input.owner)?;
            check_id(input.fee_policy_id)?;
            input.carry.validate()?;
            if input.fee_window_end == 0 || input.snapshot_epoch < input.fee_window_end {
                return Err(Error::InvalidEpoch);
            }
            if input.carry != FractionalCarry::ZERO {
                return Err(Error::FeeAllocationMismatch);
            }
            if input.lp_share_supply == 0 {
                return Err(Error::LastShareLocked);
            }
            if input.lp_share_supply > MAX_ACCOUNTING_ATOMS
                || input.reserve_atoms > MAX_ACCOUNTING_ATOMS
                || input.capital_time_weight > MAX_CAPITAL_TIME_WEIGHT
            {
                return Err(Error::ParameterOutOfRange);
            }
            if input.tranche_generation == u64::MAX {
                return Err(Error::ParameterOutOfRange);
            }
            if input.fee_allocation_generation != 0 || input.last_fee_allocation_id != [0; 32] {
                return Err(Error::FeeAllocationMismatch);
            }
            let mut prior = 0usize;
            while prior < index {
                let other = inputs[prior].ok_or(Error::NonCanonicalPadding)?;
                if other.tranche_id == input.tranche_id {
                    return Err(Error::MismatchedBinding);
                }
                if other.fee_policy_id != input.fee_policy_id
                    || other.snapshot_epoch != input.snapshot_epoch
                    || other.fee_window_end != input.fee_window_end
                {
                    return Err(Error::MismatchedBinding);
                }
                prior += 1;
            }
            total_weight = total_weight
                .checked_add(input.capital_time_weight)
                .ok_or(Error::ArithmeticOverflow)?;
        } else if inputs[index].is_some() {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if total_weight == 0 {
        return Err(Error::ZeroWeight);
    }
    if total_weight > MAX_AGGREGATE_FEE_WEIGHT {
        return Err(Error::ParameterOutOfRange);
    }
    let prior_carry_escrow_atoms = integer_sum_of_carries(inputs, count)?;
    let fee_units = normalized_fee_units(inputs, count)?;
    let mut outputs = [None; MAX_FEE_RECIPIENTS];
    let mut credited_atoms = 0u64;
    index = 0;
    while index < count {
        let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
        let (credited, new_carry) = add_weighted_fee(input.carry, fee_pot_atoms, fee_units[index])?;
        outputs[index] = Some(FeeAllocationOutputV1 {
            allocation_id,
            tranche_id: input.tranche_id,
            owner: input.owner,
            fee_policy_id: input.fee_policy_id,
            snapshot_epoch: input.snapshot_epoch,
            fee_window_end: input.fee_window_end,
            lp_share_supply: input.lp_share_supply,
            reserve_atoms: input.reserve_atoms,
            fee_allocation_generation: input.fee_allocation_generation,
            last_fee_allocation_id: input.last_fee_allocation_id,
            tranche_generation: input.tranche_generation,
            consumed_weight: input.capital_time_weight,
            old_carry: input.carry,
            credited_atoms: credited,
            new_carry,
        });
        credited_atoms = credited_atoms
            .checked_add(credited)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    let retained_carry_escrow_atoms = integer_sum_of_output_carries(&outputs, count)?;
    let available = fee_pot_atoms
        .checked_add(prior_carry_escrow_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let retained_and_credited = credited_atoms
        .checked_add(retained_carry_escrow_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    if available != retained_and_credited {
        return Err(Error::InvariantViolation);
    }
    Ok(FeeAllocationBatchV1 {
        allocation_id,
        recipient_count,
        fee_pot_atoms,
        total_weight,
        prior_carry_escrow_atoms,
        retained_carry_escrow_atoms,
        credited_atoms,
        outputs,
    })
}

/// Conservative full-simplex liability of a coefficient inventory.
///
/// For every nonnegative integer weight vector summing to `denominator`, the
/// payout is at most this maximum coefficient. A simplex vertex attains it.
pub fn full_simplex_liability(outcome_count: u8, inventory: &[u64; MAX_OUTCOMES]) -> Result<u64> {
    validate_padding(outcome_count, inventory)?;
    maximum(outcome_count, inventory)
}

/// Exact payout numerator `sum_i inventory_i * weight_i`.
pub fn payout_numerator(
    outcome_count: u8,
    inventory: &[u64; MAX_OUTCOMES],
    weights: &[u64; MAX_OUTCOMES],
) -> Result<u128> {
    validate_padding(outcome_count, inventory)?;
    validate_padding(outcome_count, weights)?;
    dot(outcome_count, inventory, weights)
}

fn validate_rung(policy: &LiquidityPolicyV1, rung: &QuoteRungV1) -> Result<()> {
    if rung.lots == 0 || rung.limit_collateral_per_lot == 0 || rung.generation == 0 {
        return Err(Error::ZeroValue);
    }
    if rung.minimum_fill_lots == 0 || rung.minimum_fill_lots > rung.lots {
        return Err(Error::InvalidRange);
    }
    if rung.lots > MAX_ACCOUNTING_ATOMS
        || checked_mul(rung.lots, rung.limit_collateral_per_lot)? > MAX_ACCOUNTING_ATOMS
    {
        return Err(Error::ParameterOutOfRange);
    }
    if rung.start_epoch < policy.batch_start
        || rung.expiry_epoch > policy.batch_end
        || rung.start_epoch > rung.expiry_epoch
    {
        return Err(Error::InvalidEpoch);
    }
    Ok(())
}

fn validate_plan(
    policy: &LiquidityPolicyV1,
    tranche_id: Id,
    plan: &PortfolioQuotePlanV1,
) -> Result<()> {
    check_id(plan.quote_id)?;
    if plan.policy_id != policy.policy_id
        || plan.tranche_id != tranche_id
        || plan.market != policy.terms.market
        || plan.terms_digest != policy.terms.terms_digest
        || plan.payoff_region_digest != policy.payoff_region_digest
        || plan.quote_schedule_digest != policy.quote_schedule_digest
        || plan.basis_degree != policy.terms.basis_degree
        || plan.active_len != policy.terms.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    validate_padding(plan.active_len, &plan.coefficients)?;
    if !any_nonzero(plan.active_len, &plan.coefficients) {
        return Err(Error::ZeroValue);
    }
    validate_rung(
        policy,
        &QuoteRungV1 {
            quote_id: plan.quote_id,
            side: plan.side,
            shape: CoefficientShapeV1::Exact {
                active_len: plan.active_len,
                coefficients: plan.coefficients,
            },
            lots: plan.lots,
            limit_collateral_per_lot: plan.limit_collateral_per_lot,
            minimum_fill_lots: plan.minimum_fill_lots,
            start_epoch: plan.start_epoch,
            expiry_epoch: plan.expiry_epoch,
            generation: plan.generation,
        },
    )
}

fn compile_shape(outcome_count: u8, shape: CoefficientShapeV1) -> Result<[u64; MAX_OUTCOMES]> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidBasis);
    }
    let count = usize::from(outcome_count);
    let mut coefficients = [0u64; MAX_OUTCOMES];
    match shape {
        CoefficientShapeV1::HardRange { first, end, amount } => {
            if amount == 0 || first >= end || usize::from(end) > count {
                return Err(Error::InvalidRange);
            }
            let mut i = usize::from(first);
            while i < usize::from(end) {
                coefficients[i] = amount;
                i += 1;
            }
        }
        CoefficientShapeV1::Triangle {
            left,
            peak,
            right,
            height,
        } => {
            if height == 0 || !(left < peak && peak < right) || usize::from(right) >= count {
                return Err(Error::InvalidRange);
            }
            let mut i = usize::from(left);
            while i <= usize::from(right) {
                coefficients[i] = if i <= usize::from(peak) {
                    let numerator = u128::from(height)
                        .checked_mul(
                            u128::try_from(i - usize::from(left))
                                .map_err(|_| Error::ArithmeticOverflow)?,
                        )
                        .ok_or(Error::ArithmeticOverflow)?;
                    let value = numerator / u128::from(peak - left);
                    u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)?
                } else {
                    let numerator = u128::from(height)
                        .checked_mul(
                            u128::try_from(usize::from(right) - i)
                                .map_err(|_| Error::ArithmeticOverflow)?,
                        )
                        .ok_or(Error::ArithmeticOverflow)?;
                    let value = numerator / u128::from(right - peak);
                    u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)?
                };
                i += 1;
            }
        }
        CoefficientShapeV1::Exact {
            active_len,
            coefficients: exact,
        } => {
            if active_len != outcome_count {
                return Err(Error::MismatchedBinding);
            }
            validate_padding(active_len, &exact)?;
            if !any_nonzero(active_len, &exact) {
                return Err(Error::ZeroValue);
            }
            coefficients = exact;
        }
    }
    Ok(coefficients)
}

fn integer_sum_of_carries(
    inputs: &[Option<FeeAllocationInputV1>; MAX_FEE_RECIPIENTS],
    count: usize,
) -> Result<u64> {
    let mut numerator = 0u128;
    let mut index = 0usize;
    while index < count {
        let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
        input.carry.validate()?;
        numerator = numerator
            .checked_add(input.carry.numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    integer_grid_sum(numerator)
}

fn integer_sum_of_output_carries(
    outputs: &[Option<FeeAllocationOutputV1>; MAX_FEE_RECIPIENTS],
    count: usize,
) -> Result<u64> {
    let mut numerator = 0u128;
    let mut index = 0usize;
    while index < count {
        let output = outputs[index].ok_or(Error::NonCanonicalPadding)?;
        output.new_carry.validate()?;
        numerator = numerator
            .checked_add(output.new_carry.numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    integer_grid_sum(numerator)
}

fn integer_grid_sum(numerator: u128) -> Result<u64> {
    if !numerator.is_multiple_of(MAX_CARRY_DENOMINATOR) {
        return Err(Error::InvariantViolation);
    }
    u64::try_from(numerator / MAX_CARRY_DENOMINATOR).map_err(|_| Error::ArithmeticOverflow)
}

fn normalized_fee_units(
    inputs: &[Option<FeeAllocationInputV1>; MAX_FEE_RECIPIENTS],
    count: usize,
) -> Result<[u128; MAX_FEE_RECIPIENTS]> {
    let mut total_weight = 0u128;
    let mut index = 0usize;
    while index < count {
        let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
        total_weight = total_weight
            .checked_add(input.capital_time_weight)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if total_weight == 0 || total_weight > MAX_AGGREGATE_FEE_WEIGHT {
        return Err(Error::ZeroWeight);
    }

    // Aggregate every beneficial owner's weight before rounding. Only the
    // lexicographically smallest tranche for an owner receives that owner's
    // units; the other tranche outputs consume their weights with zero credit
    // and carry. This makes splitting one owner's weight across tranche
    // identities economically neutral without requiring global creation-time
    // uniqueness state in this isolated model.
    let mut owner_weights = [0u128; MAX_FEE_RECIPIENTS];
    let mut representatives = [false; MAX_FEE_RECIPIENTS];
    index = 0;
    while index < count {
        let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
        let mut representative = index;
        let mut cursor = 0usize;
        while cursor < count {
            let other = inputs[cursor].ok_or(Error::NonCanonicalPadding)?;
            let current = inputs[representative].ok_or(Error::NonCanonicalPadding)?;
            if other.owner == input.owner && other.tranche_id < current.tranche_id {
                representative = cursor;
            }
            cursor += 1;
        }
        owner_weights[representative] = owner_weights[representative]
            .checked_add(input.capital_time_weight)
            .ok_or(Error::ArithmeticOverflow)?;
        representatives[representative] = true;
        index += 1;
    }

    let mut units = [0u128; MAX_FEE_RECIPIENTS];
    let mut remainders = [0u128; MAX_FEE_RECIPIENTS];
    let mut assigned = 0u128;
    index = 0;
    while index < count {
        let scaled = owner_weights[index]
            .checked_mul(MAX_CARRY_DENOMINATOR)
            .ok_or(Error::ArithmeticOverflow)?;
        units[index] = scaled / total_weight;
        remainders[index] = scaled % total_weight;
        assigned = assigned
            .checked_add(units[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    let mut remaining = MAX_CARRY_DENOMINATOR
        .checked_sub(assigned)
        .ok_or(Error::InvariantViolation)?;
    if remaining > count as u128 {
        return Err(Error::InvariantViolation);
    }
    let mut awarded = [false; MAX_FEE_RECIPIENTS];
    while remaining != 0 {
        let mut best = None;
        index = 0;
        while index < count {
            let input = inputs[index].ok_or(Error::NonCanonicalPadding)?;
            if !awarded[index] && representatives[index] && owner_weights[index] != 0 {
                best = match best {
                    None => Some(index),
                    Some(current) => {
                        let current_input = inputs[current].ok_or(Error::NonCanonicalPadding)?;
                        if remainders[index] > remainders[current]
                            || (remainders[index] == remainders[current]
                                && input.owner < current_input.owner)
                        {
                            Some(index)
                        } else {
                            Some(current)
                        }
                    }
                };
            }
            index += 1;
        }
        let selected = best.ok_or(Error::InvariantViolation)?;
        units[selected] = units[selected]
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        awarded[selected] = true;
        remaining -= 1;
    }
    Ok(units)
}

fn add_weighted_fee(
    carry: FractionalCarry,
    pot: u64,
    fee_units: u128,
) -> Result<(u64, FractionalCarry)> {
    carry.validate()?;
    if fee_units > MAX_CARRY_DENOMINATOR {
        return Err(Error::InvariantViolation);
    }
    let numerator = u128::from(pot)
        .checked_mul(fee_units)
        .and_then(|value| value.checked_add(carry.numerator))
        .ok_or(Error::ArithmeticOverflow)?;
    let whole = numerator / MAX_CARRY_DENOMINATOR;
    let remainder = numerator % MAX_CARRY_DENOMINATOR;
    let new_carry = if remainder == 0 {
        FractionalCarry::ZERO
    } else {
        FractionalCarry {
            numerator: remainder,
            denominator: MAX_CARRY_DENOMINATOR,
        }
    };
    Ok((
        u64::try_from(whole).map_err(|_| Error::ArithmeticOverflow)?,
        new_carry,
    ))
}

fn validate_payout(
    outcome_count: u8,
    expected_denominator: u64,
    denominator: u64,
    weights: &[u64; MAX_OUTCOMES],
) -> Result<()> {
    if denominator == 0 || denominator != expected_denominator {
        return Err(Error::InvalidPayoutVector);
    }
    validate_padding(outcome_count, weights).map_err(|_| Error::InvalidPayoutVector)?;
    let mut sum = 0u64;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if weights[i] > denominator {
            return Err(Error::InvalidPayoutVector);
        }
        sum = sum
            .checked_add(weights[i])
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != denominator {
        return Err(Error::InvalidPayoutVector);
    }
    Ok(())
}

fn check_inventory_limit(
    policy: &LiquidityPolicyV1,
    inventory: &[u64; MAX_OUTCOMES],
    pending_sell: &[u64; MAX_OUTCOMES],
) -> Result<()> {
    let mut i = 0usize;
    while i < usize::from(policy.terms.outcome_count) {
        let combined = inventory[i]
            .checked_add(pending_sell[i])
            .ok_or(Error::ArithmeticOverflow)?;
        if combined > policy.max_inventory[i] {
            return Err(Error::InventoryLimit);
        }
        i += 1;
    }
    Ok(())
}

fn checked_add_vectors(
    outcome_count: u8,
    left: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        output[i] = left[i]
            .checked_add(right[i])
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(output)
}

fn scaled_vector(
    outcome_count: u8,
    coefficients: &[u64; MAX_OUTCOMES],
    lots: u64,
) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        output[i] = checked_mul(coefficients[i], lots)?;
        i += 1;
    }
    Ok(output)
}

fn add_scaled(
    destination: &mut [u64; MAX_OUTCOMES],
    coefficients: &[u64; MAX_OUTCOMES],
    lots: u64,
    outcome_count: u8,
) -> Result<()> {
    let scaled = scaled_vector(outcome_count, coefficients, lots)?;
    add_vector(outcome_count, destination, &scaled)
}

fn add_vector(
    outcome_count: u8,
    destination: &mut [u64; MAX_OUTCOMES],
    amount: &[u64; MAX_OUTCOMES],
) -> Result<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        destination[i] = destination[i]
            .checked_add(amount[i])
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(())
}

fn subtract_vector(
    outcome_count: u8,
    destination: &mut [u64; MAX_OUTCOMES],
    amount: &[u64; MAX_OUTCOMES],
) -> Result<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        destination[i] = destination[i]
            .checked_sub(amount[i])
            .ok_or(Error::InsufficientInventory)?;
        i += 1;
    }
    Ok(())
}

fn ensure_componentwise_at_most(
    outcome_count: u8,
    left: &[u64; MAX_OUTCOMES],
    right: &[u64; MAX_OUTCOMES],
) -> Result<()> {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if left[i] > right[i] {
            return Err(Error::InsufficientInventory);
        }
        i += 1;
    }
    Ok(())
}

fn maximum(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> Result<u64> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidBasis);
    }
    let mut maximum = 0u64;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if values[i] > maximum {
            maximum = values[i];
        }
        i += 1;
    }
    Ok(maximum)
}

fn dot(outcome_count: u8, left: &[u64; MAX_OUTCOMES], right: &[u64; MAX_OUTCOMES]) -> Result<u128> {
    let mut total = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        total = total
            .checked_add(
                u128::from(left[i])
                    .checked_mul(u128::from(right[i]))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(total)
}

fn validate_padding(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    let count = usize::from(outcome_count);
    if !(2..=MAX_OUTCOMES).contains(&count) {
        return Err(Error::InvalidBasis);
    }
    let mut i = count;
    while i < MAX_OUTCOMES {
        if values[i] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        i += 1;
    }
    Ok(())
}

fn any_nonzero(outcome_count: u8, values: &[u64; MAX_OUTCOMES]) -> bool {
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if values[i] != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn checked_mul(left: u64, right: u64) -> Result<u64> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

fn check_id(id: Id) -> Result<()> {
    if id == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}
