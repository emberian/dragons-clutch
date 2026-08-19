//! Exact native coefficient-portfolio identity and settlement preflight.
//!
//! This module closes the pure arithmetic half of a deliberately missing live
//! transition.  [`crate::PortfolioRecord`] and
//! [`crate::reservation::ReservationPlan`] already own placement and exact
//! funding.  Here, two proportional encodings of the same native B-spline Egg
//! vector canonicalize to one claim identity, one full-fill pair is checked
//! against both ACTIVE reservations, and every post-state is staged before a
//! caller can mutate anything.
//!
//! It is **not** a live entitlement authority.  [`PortfolioEntitlementV1`] has
//! no account codec, PDA, initializer, instruction, or selection transition.
//! [`prepare_full_pair`] is suitable for an offline/reference adapter only
//! until every item in [`PORTFOLIO_RUNTIME_BLOCKERS_V1`] is discharged.  A
//! caller-created value with internally consistent fields proves content
//! consistency, not that the protocol selected or funded it.
//!
//! The coefficient vector is over the Market's native degree-zero through
//! degree-three B-spline Eggs.  The claim identity binds the exact Terms
//! digest, basis degree, denominator, and outcome order.  This module never
//! selects a categorical terminal cell and never wraps the vector as an NFT or
//! another token.

use crate::{
    check_hash, check_padded_amounts, digest, order_id_rank,
    reservation::{
        ReservationAccount, ReservationPlan, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED,
    },
    CodecError, Hash32, OrderSlot, PortfolioRecord, PositionAccount, TermsAccount,
    MAX_BASIS_DEGREE, MAX_OUTCOMES,
};

/// Entitlement content has not been consumed.
pub const PORTFOLIO_ENTITLEMENT_ACTIVE: u8 = 0;
/// Both reservations and the entitlement content have been consumed once.
pub const PORTFOLIO_ENTITLEMENT_CONSUMED: u8 = 1;

/// Refusals specific to the proposed coefficient-portfolio consumption seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioSettlementError {
    /// One of the existing hostile-byte-facing semantic owners refused.
    Codec(CodecError),
    /// The two records do not name the same canonical native payoff vector.
    ClaimMismatch,
    /// An identity, owner, generation, price, or reservation binding differs.
    MismatchedBinding,
    /// A checked product or sum exceeds its frozen integer width.
    ArithmeticOverflow,
    /// A cash conversion would require an unnamed rounding rule.
    InexactConsideration,
    /// A Position or reservation cannot fund the exact staged transition.
    InsufficientFunding,
    /// The supplied entitlement or either reservation was already consumed.
    AlreadyConsumed,
    /// Nonzero fees cannot settle until an authenticated carry owner exists.
    FeeCarryAuthorityUnavailable,
}

impl From<CodecError> for PortfolioSettlementError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

/// Result type for this module's pure seam.
pub type PortfolioResult<T> = core::result::Result<T, PortfolioSettlementError>;

/// Canonical primitive coefficient vector over one immutable native basis.
///
/// Proportional requested vectors share an identity.  For example, `(2, 4)`
/// compiles to primitive `(1, 2)` with coefficient scale `2`.  The scale is
/// returned separately and becomes part of the filled primitive-unit count;
/// it is intentionally absent from `claim`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePortfolioClaimV1 {
    /// Market whose native Eggs are the vector components.
    pub market: Hash32,
    /// Full immutable Terms digest, including knots and evaluator policy.
    pub terms: Hash32,
    /// Native B-spline degree, in `0..=3`.
    pub basis_degree: u8,
    /// Common exact payout denominator.
    pub denominator: u64,
    /// Active native Egg width.
    pub outcome_count: u8,
    /// GCD-one coefficient prefix with canonical zero padding.
    pub coefficients: [u64; MAX_OUTCOMES],
    /// Domain-separated digest of every preceding semantic field.
    pub claim: Hash32,
}

impl NativePortfolioClaimV1 {
    /// Canonicalize one requested vector under an already validated Terms set.
    ///
    /// The returned `u64` is the removed coefficient gcd.  Multiplying the
    /// requested lot count by it preserves the exact transferred vector.
    pub fn compile(
        market: Hash32,
        terms: &TermsAccount,
        requested: [u64; MAX_OUTCOMES],
    ) -> PortfolioResult<(Self, u64)> {
        terms.validate()?;
        check_hash(market)?;
        check_padded_amounts(&requested, usize::from(terms.outcome_count))?;

        let mut divisor = 0u64;
        let mut i = 0usize;
        while i < usize::from(terms.outcome_count) {
            divisor = gcd(divisor, requested[i]);
            i += 1;
        }
        if divisor == 0 {
            return Err(CodecError::ZeroValue.into());
        }

        let mut coefficients = [0u64; MAX_OUTCOMES];
        i = 0;
        while i < usize::from(terms.outcome_count) {
            coefficients[i] = requested[i] / divisor;
            i += 1;
        }
        let denominator = terms.payouts[0].denominator;
        let claim = canonical_native_portfolio_claim_id(
            market,
            terms.terms,
            terms.basis_degree,
            denominator,
            terms.outcome_count,
            &coefficients,
        );
        let value = Self {
            market,
            terms: terms.terms,
            basis_degree: terms.basis_degree,
            denominator,
            outcome_count: terms.outcome_count,
            coefficients,
            claim,
        };
        value.validate()?;
        Ok((value, divisor))
    }

    /// Recompute identity, basis bounds, primitive scaling, and padding.
    pub fn validate(&self) -> PortfolioResult<()> {
        check_hash(self.market)?;
        check_hash(self.terms)?;
        check_hash(self.claim)?;
        if self.basis_degree > MAX_BASIS_DEGREE {
            return Err(CodecError::InvalidEnum.into());
        }
        if self.denominator == 0 {
            return Err(CodecError::ZeroValue.into());
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(CodecError::InvalidCount.into());
        }
        check_padded_amounts(&self.coefficients, usize::from(self.outcome_count))?;
        let mut divisor = 0u64;
        let mut i = 0usize;
        while i < usize::from(self.outcome_count) {
            divisor = gcd(divisor, self.coefficients[i]);
            i += 1;
        }
        if divisor != 1 {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        if self.claim
            != canonical_native_portfolio_claim_id(
                self.market,
                self.terms,
                self.basis_degree,
                self.denominator,
                self.outcome_count,
                &self.coefficients,
            )
        {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        Ok(())
    }
}

/// Exact funded interpretation of one already-admitted Portfolio record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioFundingV1 {
    /// Canonical basis-and-coefficient identity.
    pub claim: NativePortfolioClaimV1,
    /// GCD removed from the order's requested coefficient vector.
    pub coefficient_scale: u64,
    /// Order lots multiplied by `coefficient_scale`.
    pub primitive_units: u64,
    /// Exact Egg vector transferred by a full fill.
    pub internal: [u64; MAX_OUTCOMES],
    /// Maximum payout over the full simplex, in collateral atoms.
    ///
    /// The market's reachable B-spline vectors may be a strict subset of the
    /// simplex; this is the exact conservative maximum over all nonnegative
    /// weights summing to the Terms denominator.
    pub simplex_worst_case_payout_atoms: u64,
    /// Existing placement owner's exact reservation plan.
    pub reservation: ReservationPlan,
}

impl PortfolioFundingV1 {
    /// Recompute canonical identity and exact full-order funding.
    pub fn for_order(
        market: Hash32,
        terms: &TermsAccount,
        price_scale: u64,
        order: &PortfolioRecord,
        max_fee_atoms: u64,
    ) -> PortfolioResult<Self> {
        terms.validate()?;
        order.validate_on_scale(price_scale)?;
        if order.active_len > terms.outcome_count {
            return Err(PortfolioSettlementError::MismatchedBinding);
        }
        let (claim, coefficient_scale) =
            NativePortfolioClaimV1::compile(market, terms, order.coefficients)?;
        let primitive_units = order
            .lots
            .checked_mul(coefficient_scale)
            .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
        let mut internal = [0u64; MAX_OUTCOMES];
        let mut maximum = 0u64;
        let mut i = 0usize;
        while i < usize::from(terms.outcome_count) {
            internal[i] = order
                .lots
                .checked_mul(order.coefficients[i])
                .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
            let canonical = primitive_units
                .checked_mul(claim.coefficients[i])
                .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
            if internal[i] != canonical {
                return Err(PortfolioSettlementError::ClaimMismatch);
            }
            if internal[i] > maximum {
                maximum = internal[i];
            }
            i += 1;
        }
        let reservation = ReservationPlan::for_order(
            &OrderSlot::Portfolio(*order),
            terms.outcome_count,
            price_scale,
            max_fee_atoms,
        )?;
        Ok(Self {
            claim,
            coefficient_scale,
            primitive_units,
            internal,
            simplex_worst_case_payout_atoms: maximum,
            reservation,
        })
    }
}

/// Content of a proposed immutable vector entitlement.
///
/// This is a semantic record, not an account codec.  Its digest prevents field
/// substitution only after a future account plane authenticates who created
/// and froze it.  Current live code has no such authority and must not treat a
/// struct literal or digest match as selection evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioEntitlementV1 {
    /// Canonical digest of the immutable content below.
    pub entitlement: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Frozen epoch identity.
    pub epoch: Hash32,
    /// Selected candidate identity.
    pub candidate: Hash32,
    /// Immutable Terms/basis digest.
    pub terms: Hash32,
    /// Frozen price-grid identity.
    pub price_grid: Hash32,
    /// Frozen policy identity.
    pub policy: Hash32,
    /// Canonical native coefficient-claim identity.
    pub claim: Hash32,
    /// Buy Portfolio order identity.
    pub buy_order_id: Hash32,
    /// Sell Portfolio order identity.
    pub sell_order_id: Hash32,
    /// Frozen simplex vector, canonically padded.
    pub prices: [u64; MAX_OUTCOMES],
    /// Price-vector scale.
    pub price_scale: u64,
    /// Active basis width.
    pub outcome_count: u8,
    /// Filled units of the primitive coefficient vector.
    pub primitive_units: u64,
    /// Exact dot-product consideration before division by `price_scale`.
    pub consideration_price_units: u128,
    /// Active or consumed; see `PORTFOLIO_ENTITLEMENT_*`.
    pub state: u8,
}

impl PortfolioEntitlementV1 {
    /// Recompute identity, simplex shape, and consumption-state shape.
    pub fn validate(&self) -> PortfolioResult<()> {
        for identity in [
            self.entitlement,
            self.market,
            self.epoch,
            self.candidate,
            self.terms,
            self.price_grid,
            self.policy,
            self.claim,
            self.buy_order_id,
            self.sell_order_id,
        ] {
            check_hash(identity)?;
        }
        order_id_rank(self.buy_order_id)?;
        order_id_rank(self.sell_order_id)?;
        if self.buy_order_id == self.sell_order_id {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        validate_simplex(&self.prices, self.outcome_count, self.price_scale)?;
        if self.primitive_units == 0 {
            return Err(CodecError::ZeroValue.into());
        }
        if self.state > PORTFOLIO_ENTITLEMENT_CONSUMED {
            return Err(CodecError::InvalidEnum.into());
        }
        if self.entitlement
            != canonical_portfolio_entitlement_id(
                self.market,
                self.epoch,
                self.candidate,
                self.terms,
                self.price_grid,
                self.policy,
                self.claim,
                self.buy_order_id,
                self.sell_order_id,
                &self.prices,
                self.price_scale,
                self.outcome_count,
                self.primitive_units,
                self.consideration_price_units,
            )
        {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        Ok(())
    }
}

/// Exact policy fraction for the experimental simplex-dispersion fee.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispersionFeePolicyV1 {
    /// Numerator of `kappa`.
    pub kappa_numerator: u64,
    /// Denominator of `kappa`.
    pub kappa_denominator: u64,
}

/// One exact fee/carry update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispersionFeeStepV1 {
    /// Whole collateral atoms charged by this update.
    pub fee_atoms: u128,
    /// Fractional numerator retained by the same Position/policy domain.
    pub next_carry: u128,
    /// Common divisor against which `next_carry` is canonical.
    pub carry_denominator: u128,
    /// Exact state-contingent dispersion numerator before `kappa`.
    pub dispersion_numerator: u128,
}

/// Compute the proposed representation-invariant dispersion fee exactly once.
///
/// `payoff` is the *actual transferred vector*, not a display decomposition.
/// Adding a constant complete set, proportionally re-encoding lots, splitting
/// a price cell into identical-payoff subcells, or fragmenting a fill while
/// carrying the returned remainder cannot change the aggregate result.
///
/// This arithmetic does not authorize a live fee.  Current Position bytes have
/// no authenticated carry field, so [`prepare_full_pair`] accepts zero-fee
/// reservations only.
pub fn dispersion_fee_step(
    payoff: &[u64; MAX_OUTCOMES],
    prices: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
    price_scale: u64,
    policy: DispersionFeePolicyV1,
    prior_carry: u128,
) -> PortfolioResult<DispersionFeeStepV1> {
    validate_simplex(prices, outcome_count, price_scale)?;
    check_padded_amounts(payoff, usize::from(outcome_count))?;
    if policy.kappa_denominator == 0 {
        return Err(CodecError::ZeroValue.into());
    }
    let scale = u128::from(price_scale);
    let carry_denominator = u128::from(policy.kappa_denominator)
        .checked_mul(scale)
        .and_then(|value| value.checked_mul(scale))
        .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
    if prior_carry >= carry_denominator {
        return Err(CodecError::NonCanonicalPadding.into());
    }

    let mut dispersion = 0u128;
    let active = usize::from(outcome_count);
    let mut i = 0usize;
    while i < active {
        let mut j = i + 1;
        while j < active {
            let term = u128::from(prices[i])
                .checked_mul(u128::from(prices[j]))
                .and_then(|value| value.checked_mul(u128::from(payoff[i].abs_diff(payoff[j]))))
                .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
            dispersion = dispersion
                .checked_add(term)
                .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
            j += 1;
        }
        i += 1;
    }
    let fee_numerator = u128::from(policy.kappa_numerator)
        .checked_mul(dispersion)
        .and_then(|value| value.checked_add(prior_carry))
        .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
    Ok(DispersionFeeStepV1 {
        fee_atoms: fee_numerator / carry_denominator,
        next_carry: fee_numerator % carry_denominator,
        carry_denominator,
        dispersion_numerator: dispersion,
    })
}

/// Immutable inputs to a full, direct, paired Portfolio preflight.
///
/// A future live adapter must load both records from the complete frozen page
/// set and authenticate `entitlement` as a program-created selected-candidate
/// account.  This pure type cannot do either.
#[derive(Debug)]
pub struct PortfolioPairInputV1<'a> {
    /// Full immutable Terms account owning the native basis.
    pub terms: &'a TermsAccount,
    /// Buy Portfolio record from the frozen order set.
    pub buy_order: &'a PortfolioRecord,
    /// Sell Portfolio record from the frozen order set.
    pub sell_order: &'a PortfolioRecord,
    /// Buyer's current Position.
    pub buyer_position: &'a PositionAccount,
    /// Seller's current Position.
    pub seller_position: &'a PositionAccount,
    /// Buy order's exact ACTIVE reservation.
    pub buyer_reservation: &'a ReservationAccount,
    /// Sell order's exact ACTIVE reservation.
    pub seller_reservation: &'a ReservationAccount,
    /// Proposed immutable vector entitlement content.
    pub entitlement: &'a PortfolioEntitlementV1,
}

/// Audited scalars for one full paired transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairPlanV1 {
    /// Canonical native claim identity shared by both representations.
    pub claim: Hash32,
    /// Filled primitive units.
    pub primitive_units: u64,
    /// Exact native Egg vector moved from sell reservation to buyer Position.
    pub internal: [u64; MAX_OUTCOMES],
    /// Maximum payout of the transferred vector over the simplex.
    pub simplex_worst_case_payout_atoms: u64,
    /// Exact consideration numerator under the frozen simplex price vector.
    pub consideration_price_units: u128,
    /// Exact collateral atoms debited/credited; no rounding occurred.
    pub consideration_atoms: u64,
    /// Full buyer reservation released from the Position's reserved subset.
    pub buyer_reserved_release: u64,
}

/// Fully staged post-state of the proposed pure transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairPostV1 {
    /// Transition audit scalars.
    pub plan: PortfolioPairPlanV1,
    /// Buyer Position after cash debit, reservation release, and Egg credit.
    pub buyer_position: PositionAccount,
    /// Seller Position after cash credit.
    pub seller_position: PositionAccount,
    /// Buy reservation after one-time consumption.
    pub buyer_reservation: ReservationAccount,
    /// Sell reservation after one-time consumption.
    pub seller_reservation: ReservationAccount,
    /// Entitlement content after one-time consumption.
    pub entitlement: PortfolioEntitlementV1,
}

/// Validate and stage one full paired coefficient-vector transfer.
///
/// Both raw order representations may differ by a positive scalar, but their
/// canonical claim and filled primitive units must match.  Both orders fill in
/// full.  Exact divisibility is required; partial lots, nonzero fee envelopes,
/// and any already-consumed owner refuse.  No input is mutated.
pub fn prepare_full_pair(input: &PortfolioPairInputV1<'_>) -> PortfolioResult<PortfolioPairPostV1> {
    input.terms.validate()?;
    input.entitlement.validate()?;
    if input.entitlement.state != PORTFOLIO_ENTITLEMENT_ACTIVE {
        return Err(PortfolioSettlementError::AlreadyConsumed);
    }
    if input.buy_order.side != 0
        || input.sell_order.side != 1
        || input.buy_order.owner == input.sell_order.owner
    {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }
    if input.buyer_reservation.max_fee_atoms != 0 || input.seller_reservation.max_fee_atoms != 0 {
        return Err(PortfolioSettlementError::FeeCarryAuthorityUnavailable);
    }

    let market = input.entitlement.market;
    let buy = PortfolioFundingV1::for_order(
        market,
        input.terms,
        input.entitlement.price_scale,
        input.buy_order,
        0,
    )?;
    let sell = PortfolioFundingV1::for_order(
        market,
        input.terms,
        input.entitlement.price_scale,
        input.sell_order,
        0,
    )?;
    if buy.claim != sell.claim
        || buy.primitive_units != sell.primitive_units
        || buy.internal != sell.internal
        || buy.claim.claim != input.entitlement.claim
        || buy.primitive_units != input.entitlement.primitive_units
    {
        return Err(PortfolioSettlementError::ClaimMismatch);
    }
    if input.entitlement.terms != input.terms.terms
        || input.entitlement.price_grid != input.terms.price_grid
        || input.entitlement.outcome_count != input.terms.outcome_count
        || input.entitlement.buy_order_id != input.buy_order.order_id
        || input.entitlement.sell_order_id != input.sell_order.order_id
    {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }

    let consideration_price_units = exact_portfolio_value_price_units(
        &buy.internal,
        &input.entitlement.prices,
        input.terms.outcome_count,
    )?;
    if consideration_price_units != input.entitlement.consideration_price_units {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }
    let scale = u128::from(input.entitlement.price_scale);
    let buy_limit = u128::from(input.buy_order.lots)
        .checked_mul(u128::from(input.buy_order.limit_collateral_per_lot))
        .and_then(|value| value.checked_mul(scale))
        .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
    let sell_limit = u128::from(input.sell_order.lots)
        .checked_mul(u128::from(input.sell_order.limit_collateral_per_lot))
        .and_then(|value| value.checked_mul(scale))
        .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
    if consideration_price_units > buy_limit || consideration_price_units < sell_limit {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }
    if consideration_price_units % scale != 0 {
        return Err(PortfolioSettlementError::InexactConsideration);
    }
    let consideration_atoms = u64::try_from(consideration_price_units / scale)
        .map_err(|_| PortfolioSettlementError::ArithmeticOverflow)?;

    validate_position(
        input.buyer_position,
        market,
        input.buy_order.owner,
        input.terms.outcome_count,
    )?;
    validate_position(
        input.seller_position,
        market,
        input.sell_order.owner,
        input.terms.outcome_count,
    )?;
    validate_reservation(
        input.buyer_reservation,
        input.entitlement,
        input.buy_order,
        input.buyer_position,
        &buy,
    )?;
    validate_reservation(
        input.seller_reservation,
        input.entitlement,
        input.sell_order,
        input.seller_position,
        &sell,
    )?;

    let mut buyer_position = *input.buyer_position;
    let mut seller_position = *input.seller_position;
    buyer_position.cash_atoms = buyer_position
        .cash_atoms
        .checked_sub(consideration_atoms)
        .ok_or(PortfolioSettlementError::InsufficientFunding)?;
    buyer_position.reserved_cash_atoms = buyer_position
        .reserved_cash_atoms
        .checked_sub(input.buyer_reservation.remaining_cash_atoms)
        .ok_or(PortfolioSettlementError::InsufficientFunding)?;
    seller_position.cash_atoms = seller_position
        .cash_atoms
        .checked_add(consideration_atoms)
        .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        buyer_position.internal[i] = buyer_position.internal[i]
            .checked_add(buy.internal[i])
            .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
        i += 1;
    }
    buyer_position.validate()?;
    seller_position.validate()?;

    let mut buyer_reservation = *input.buyer_reservation;
    let mut seller_reservation = *input.seller_reservation;
    consume_reservation(&mut buyer_reservation);
    consume_reservation(&mut seller_reservation);
    buyer_reservation.validate()?;
    seller_reservation.validate()?;
    let mut entitlement = *input.entitlement;
    entitlement.state = PORTFOLIO_ENTITLEMENT_CONSUMED;
    entitlement.validate()?;

    Ok(PortfolioPairPostV1 {
        plan: PortfolioPairPlanV1 {
            claim: buy.claim.claim,
            primitive_units: buy.primitive_units,
            internal: buy.internal,
            simplex_worst_case_payout_atoms: buy.simplex_worst_case_payout_atoms,
            consideration_price_units,
            consideration_atoms,
            buyer_reserved_release: input.buyer_reservation.remaining_cash_atoms,
        },
        buyer_position,
        seller_position,
        buyer_reservation,
        seller_reservation,
        entitlement,
    })
}

/// Apply the pure staged transition after every fallible check succeeds.
///
/// This helper does not make the proposed entitlement authoritative.  It is an
/// offline/reference state-machine operation.  The live program must remain
/// disconnected until [`PORTFOLIO_RUNTIME_BLOCKERS_V1`] is empty.
#[allow(clippy::too_many_arguments)]
pub fn apply_full_pair(
    terms: &TermsAccount,
    buy_order: &PortfolioRecord,
    sell_order: &PortfolioRecord,
    buyer_position: &mut PositionAccount,
    seller_position: &mut PositionAccount,
    buyer_reservation: &mut ReservationAccount,
    seller_reservation: &mut ReservationAccount,
    entitlement: &mut PortfolioEntitlementV1,
) -> PortfolioResult<PortfolioPairPlanV1> {
    let post = prepare_full_pair(&PortfolioPairInputV1 {
        terms,
        buy_order,
        sell_order,
        buyer_position,
        seller_position,
        buyer_reservation,
        seller_reservation,
        entitlement,
    })?;
    *buyer_position = post.buyer_position;
    *seller_position = post.seller_position;
    *buyer_reservation = post.buyer_reservation;
    *seller_reservation = post.seller_reservation;
    *entitlement = post.entitlement;
    Ok(post.plan)
}

/// Recompute exact full-vector value with one sum and no intermediate division.
pub fn exact_portfolio_value_price_units(
    payoff: &[u64; MAX_OUTCOMES],
    prices: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
) -> PortfolioResult<u128> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(CodecError::InvalidCount.into());
    }
    check_padded_amounts(payoff, usize::from(outcome_count))?;
    check_padded_amounts(prices, usize::from(outcome_count))?;
    let mut value = 0u128;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        value = value
            .checked_add(u128::from(payoff[i]) * u128::from(prices[i]))
            .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
        i += 1;
    }
    Ok(value)
}

/// Validate a canonically padded integer simplex vector.
pub fn validate_simplex(
    prices: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
    price_scale: u64,
) -> PortfolioResult<()> {
    if outcome_count < 2 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(CodecError::InvalidCount.into());
    }
    if price_scale == 0 {
        return Err(CodecError::ZeroValue.into());
    }
    check_padded_amounts(prices, usize::from(outcome_count))?;
    let mut sum = 0u64;
    let mut i = 0usize;
    while i < usize::from(outcome_count) {
        if prices[i] > price_scale {
            return Err(CodecError::InvalidPriceGrid.into());
        }
        sum = sum
            .checked_add(prices[i])
            .ok_or(PortfolioSettlementError::ArithmeticOverflow)?;
        i += 1;
    }
    if sum != price_scale {
        return Err(CodecError::InvalidPriceGrid.into());
    }
    Ok(())
}

/// Derive one canonical native coefficient-claim identity.
pub fn canonical_native_portfolio_claim_id(
    market: Hash32,
    terms: Hash32,
    basis_degree: u8,
    denominator: u64,
    outcome_count: u8,
    coefficients: &[u64; MAX_OUTCOMES],
) -> Hash32 {
    let mut packed = [0u8; MAX_OUTCOMES * 8];
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        let word = coefficients[i].to_le_bytes();
        packed[i * 8..(i + 1) * 8].copy_from_slice(&word);
        i += 1;
    }
    digest(
        b"dragons-clutch/native-portfolio-claim/v1",
        &[
            &market.0,
            &terms.0,
            &[basis_degree],
            &denominator.to_le_bytes(),
            &[outcome_count],
            &packed,
        ],
    )
}

/// Derive immutable content identity for a proposed portfolio entitlement.
#[allow(clippy::too_many_arguments)]
pub fn canonical_portfolio_entitlement_id(
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    terms: Hash32,
    price_grid: Hash32,
    policy: Hash32,
    claim: Hash32,
    buy_order_id: Hash32,
    sell_order_id: Hash32,
    prices: &[u64; MAX_OUTCOMES],
    price_scale: u64,
    outcome_count: u8,
    primitive_units: u64,
    consideration_price_units: u128,
) -> Hash32 {
    let mut packed = [0u8; MAX_OUTCOMES * 8];
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        let word = prices[i].to_le_bytes();
        packed[i * 8..(i + 1) * 8].copy_from_slice(&word);
        i += 1;
    }
    digest(
        b"dragons-clutch/portfolio-entitlement/v1",
        &[
            &market.0,
            &epoch.0,
            &candidate.0,
            &terms.0,
            &price_grid.0,
            &policy.0,
            &claim.0,
            &buy_order_id.0,
            &sell_order_id.0,
            &packed,
            &price_scale.to_le_bytes(),
            &[outcome_count],
            &primitive_units.to_le_bytes(),
            &consideration_price_units.to_le_bytes(),
        ],
    )
}

/// Ranked missing live-runtime obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioRuntimeBlockerV1 {
    /// Candidate verification and best-valid-submitted selection must be live.
    CandidateSelection,
    /// The complete frozen order set must join one exact reservation per order.
    ReservationSetClosure,
    /// A stable vector-receipt codec must bind claim, units, prices, and value.
    VectorReceiptCodec,
    /// The receipt must be created only by candidate finalization, before use.
    EntitlementInitialization,
    /// The adapter must load both frozen Portfolio records by exact order id.
    FrozenPageProvenance,
    /// An immutable policy preimage must authorize fee and rounding families.
    FrozenPolicyPreimage,
    /// Nonzero fee support needs one authenticated per-Position/policy carry.
    FeeCarryAccount,
    /// Final closure must prove every reservation and receipt is consumed once.
    TerminalClosure,
}

/// Dependency order for promoting this pure seam into a live instruction.
pub const PORTFOLIO_RUNTIME_BLOCKERS_V1: [PortfolioRuntimeBlockerV1; 8] = [
    PortfolioRuntimeBlockerV1::CandidateSelection,
    PortfolioRuntimeBlockerV1::ReservationSetClosure,
    PortfolioRuntimeBlockerV1::VectorReceiptCodec,
    PortfolioRuntimeBlockerV1::EntitlementInitialization,
    PortfolioRuntimeBlockerV1::FrozenPageProvenance,
    PortfolioRuntimeBlockerV1::FrozenPolicyPreimage,
    PortfolioRuntimeBlockerV1::FeeCarryAccount,
    PortfolioRuntimeBlockerV1::TerminalClosure,
];

fn validate_position(
    position: &PositionAccount,
    market: Hash32,
    owner: Hash32,
    outcome_count: u8,
) -> PortfolioResult<()> {
    position.validate()?;
    check_padded_amounts(&position.internal, usize::from(outcome_count))?;
    if position.close_state != 0 || position.market != market || position.owner != owner {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }
    Ok(())
}

fn validate_reservation(
    reservation: &ReservationAccount,
    entitlement: &PortfolioEntitlementV1,
    order: &PortfolioRecord,
    position: &PositionAccount,
    funding: &PortfolioFundingV1,
) -> PortfolioResult<()> {
    reservation.validate()?;
    if reservation.state != RESERVATION_STATE_ACTIVE {
        return Err(PortfolioSettlementError::AlreadyConsumed);
    }
    if reservation.market != entitlement.market
        || reservation.epoch != entitlement.epoch
        || reservation.owner != order.owner
        || reservation.owner != position.owner
        || reservation.order_id != order.order_id
        || reservation.position_generation != position.generation
        || reservation.order_generation != order.generation
        || reservation.terms != entitlement.terms
        || reservation.price_grid != entitlement.price_grid
        || reservation.policy != entitlement.policy
        || reservation.outcome_count != entitlement.outcome_count
        || reservation.max_fee_atoms != 0
        || reservation.release_generation != 0
        || reservation.initial_cash_atoms != funding.reservation.cash_atoms
        || reservation.remaining_cash_atoms != funding.reservation.cash_atoms
        || reservation.initial_internal != funding.reservation.internal
        || reservation.remaining_internal != funding.reservation.internal
        || reservation.order_kind != funding.reservation.order_kind
        || reservation.side != funding.reservation.side
    {
        return Err(PortfolioSettlementError::MismatchedBinding);
    }
    Ok(())
}

fn consume_reservation(reservation: &mut ReservationAccount) {
    reservation.remaining_cash_atoms = 0;
    reservation.remaining_internal = [0; MAX_OUTCOMES];
    reservation.release_generation = 0;
    reservation.state = RESERVATION_STATE_CONSUMED;
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canonical_order_id, reservation::ReservationAccount, PayoutVectorBytes, MAX_KNOTS,
        MAX_PAYOUTS, PAYOUT_MAP_UNUSED, UNIFORM_SPACING_NONE,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn terms_for_degree(basis_degree: u8) -> TermsAccount {
        let outcome_count = if basis_degree == 3 { 4 } else { 3 };
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut failure = [0u64; MAX_OUTCOMES];
        failure[0] = 100;
        payouts[0] = PayoutVectorBytes {
            denominator: 100,
            weights: failure,
        };
        let mut knots = [0u128; MAX_KNOTS];
        let knot_count = match basis_degree {
            0 | 2 | 3 => 2,
            _ => 3,
        };
        if basis_degree == 0 {
            knots[..2].copy_from_slice(&[8, 16]);
        } else if knot_count == 2 {
            knots[..2].copy_from_slice(&[0, 8]);
        } else {
            knots[..3].copy_from_slice(&[0, 8, 16]);
        }
        let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        if basis_degree == 0 {
            payout_map[..usize::from(outcome_count)].fill(0);
        }
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: h(2),
            profile: h(3),
            feed: h(4),
            price_grid: h(5),
            outcome_count,
            payout_count: 1,
            payouts,
            grid_family_id: 7,
            grid_version: 1,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 130,
            maturity_horizon_buckets: 30,
            coverage_policy_id: 11,
            repair_policy_id: 12,
            failure_policy_id: 13,
            statistic_id: 1,
            ambiguity_policy_id: 1,
            edge_policy_id: 1,
            basis_degree,
            knot_count,
            uniform_log2_spacing: if basis_degree == 0 {
                UNIFORM_SPACING_NONE
            } else {
                3
            },
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: 5,
            source_version: 1,
            evaluator_version: 1,
            source_adapter_id: h(6),
            payout_map,
            knots,
            collateral_cap: 1_000_000,
            stored_bump: 9,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().unwrap();
        value.validate().unwrap();
        value
    }

    fn terms() -> TermsAccount {
        terms_for_degree(1)
    }

    fn order(rank: u64, owner: u8, side: u8, coefficients: [u64; 3], lots: u64) -> PortfolioRecord {
        let mut padded = [0u64; MAX_OUTCOMES];
        padded[..3].copy_from_slice(&coefficients);
        PortfolioRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            side,
            active_len: 3,
            flags: 0,
            coefficients: padded,
            lots,
            limit_collateral_per_lot: if side == 0 { 12 } else { 1 },
            minimum_fill_lots: 0,
            generation: 4,
            expiry_epoch: 9,
        }
    }

    #[test]
    fn proportional_encodings_share_one_native_claim_and_exact_funding() {
        let terms = terms();
        let left = order(1, 20, 0, [2, 4, 0], 3);
        let right = order(2, 21, 1, [1, 2, 0], 6);
        let a = PortfolioFundingV1::for_order(h(1), &terms, 100, &left, 0).unwrap();
        let b = PortfolioFundingV1::for_order(h(1), &terms, 100, &right, 0).unwrap();
        assert_eq!(a.claim, b.claim);
        assert_eq!(a.coefficient_scale, 2);
        assert_eq!(b.coefficient_scale, 1);
        assert_eq!(a.primitive_units, 6);
        assert_eq!(a.primitive_units, b.primitive_units);
        assert_eq!(&a.internal[..3], &[6, 12, 0]);
        assert_eq!(a.internal, b.internal);
        assert_eq!(a.simplex_worst_case_payout_atoms, 12);
        assert_eq!(a.reservation.cash_atoms, 36);
        assert_eq!(b.reservation.internal, b.internal);

        let other_degree = terms_for_degree(0);
        let (different, _) =
            NativePortfolioClaimV1::compile(h(1), &other_degree, left.coefficients).unwrap();
        assert_ne!(a.claim.claim, different.claim);
    }

    #[test]
    fn claim_identity_accepts_and_distinguishes_every_native_degree() {
        let mut requested = [0u64; MAX_OUTCOMES];
        requested[..4].copy_from_slice(&[2, 4, 6, 8]);
        let mut identities = [Hash32::ZERO; 4];
        let mut degree = 0u8;
        while degree <= 3 {
            let terms = terms_for_degree(degree);
            let mut vector = requested;
            if terms.outcome_count == 3 {
                vector[3] = 0;
            }
            let (claim, scale) = NativePortfolioClaimV1::compile(h(1), &terms, vector).unwrap();
            assert_eq!(claim.basis_degree, degree);
            assert_eq!(scale, 2);
            assert_eq!(claim.coefficients[0], 1);
            identities[usize::from(degree)] = claim.claim;
            degree += 1;
        }
        let mut i = 0usize;
        while i < identities.len() {
            let mut j = i + 1;
            while j < identities.len() {
                assert_ne!(identities[i], identities[j]);
                j += 1;
            }
            i += 1;
        }
    }

    #[test]
    fn fee_geometry_is_representation_complete_set_and_refinement_invariant() {
        let policy = DispersionFeePolicyV1 {
            kappa_numerator: 1,
            kappa_denominator: 1,
        };
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[..3].copy_from_slice(&[20, 30, 50]);
        let mut payoff = [0u64; MAX_OUTCOMES];
        payoff[..3].copy_from_slice(&[6, 12, 0]);
        let whole = dispersion_fee_step(&payoff, &prices, 3, 100, policy, 0).unwrap();

        let mut shifted = payoff;
        shifted[..3].iter_mut().for_each(|value| *value += 9);
        let complete_set = dispersion_fee_step(&shifted, &prices, 3, 100, policy, 0).unwrap();
        assert_eq!(whole, complete_set);

        let mut refined_prices = [0u64; MAX_OUTCOMES];
        refined_prices[..4].copy_from_slice(&[20, 10, 20, 50]);
        let mut refined_payoff = [0u64; MAX_OUTCOMES];
        refined_payoff[..4].copy_from_slice(&[6, 12, 12, 0]);
        let refined =
            dispersion_fee_step(&refined_payoff, &refined_prices, 4, 100, policy, 0).unwrap();
        assert_eq!(whole.dispersion_numerator, refined.dispersion_numerator);

        let mut first_half = [0u64; MAX_OUTCOMES];
        first_half[..3].copy_from_slice(&[3, 6, 0]);
        let first = dispersion_fee_step(&first_half, &prices, 3, 100, policy, 0).unwrap();
        let second =
            dispersion_fee_step(&first_half, &prices, 3, 100, policy, first.next_carry).unwrap();
        assert_eq!(first.fee_atoms + second.fee_atoms, whole.fee_atoms);
        assert_eq!(second.next_carry, whole.next_carry);
    }

    #[derive(Clone, Copy)]
    struct Fixture {
        terms: TermsAccount,
        buy: PortfolioRecord,
        sell: PortfolioRecord,
        buyer_position: PositionAccount,
        seller_position: PositionAccount,
        buyer_reservation: ReservationAccount,
        seller_reservation: ReservationAccount,
        entitlement: PortfolioEntitlementV1,
    }

    impl Fixture {
        fn new() -> Self {
            let terms = terms();
            let buy = order(1, 20, 0, [2, 4, 0], 3);
            let sell = order(2, 21, 1, [1, 2, 0], 6);
            let market = h(1);
            let epoch = h(7);
            let policy = h(8);
            let buy_plan =
                ReservationPlan::for_order(&OrderSlot::Portfolio(buy), terms.outcome_count, 100, 0)
                    .unwrap();
            let sell_plan = ReservationPlan::for_order(
                &OrderSlot::Portfolio(sell),
                terms.outcome_count,
                100,
                0,
            )
            .unwrap();
            let buyer_position = PositionAccount {
                market,
                owner: buy.owner,
                generation: 3,
                internal: [0; MAX_OUTCOMES],
                cash_atoms: 100,
                reserved_cash_atoms: buy_plan.cash_atoms,
                stored_bump: 1,
                close_state: 0,
            };
            let seller_position = PositionAccount {
                market,
                owner: sell.owner,
                generation: 5,
                internal: [0; MAX_OUTCOMES],
                cash_atoms: 10,
                reserved_cash_atoms: 0,
                stored_bump: 2,
                close_state: 0,
            };
            let buyer_reservation = ReservationAccount::active(
                market,
                epoch,
                buy.owner,
                buy.order_id,
                terms.price_grid,
                terms.terms,
                policy,
                buyer_position.generation,
                buy.generation,
                0,
                4,
                buy_plan,
            )
            .unwrap();
            let seller_reservation = ReservationAccount::active(
                market,
                epoch,
                sell.owner,
                sell.order_id,
                terms.price_grid,
                terms.terms,
                policy,
                seller_position.generation,
                sell.generation,
                0,
                5,
                sell_plan,
            )
            .unwrap();
            let funding = PortfolioFundingV1::for_order(market, &terms, 100, &buy, 0).unwrap();
            let mut prices = [0u64; MAX_OUTCOMES];
            prices[..3].copy_from_slice(&[20, 30, 50]);
            let consideration =
                exact_portfolio_value_price_units(&funding.internal, &prices, 3).unwrap();
            assert_eq!(consideration, 480);
            // Exact collateral conversion is required, so use a scale which
            // divides the chosen vector value while retaining a simplex.
            prices[..3].copy_from_slice(&[20, 30, 30]);
            let price_scale = 80;
            let consideration =
                exact_portfolio_value_price_units(&funding.internal, &prices, 3).unwrap();
            assert_eq!(consideration, 480);
            let entitlement_id = canonical_portfolio_entitlement_id(
                market,
                epoch,
                h(9),
                terms.terms,
                terms.price_grid,
                policy,
                funding.claim.claim,
                buy.order_id,
                sell.order_id,
                &prices,
                price_scale,
                3,
                funding.primitive_units,
                consideration,
            );
            let entitlement = PortfolioEntitlementV1 {
                entitlement: entitlement_id,
                market,
                epoch,
                candidate: h(9),
                terms: terms.terms,
                price_grid: terms.price_grid,
                policy,
                claim: funding.claim.claim,
                buy_order_id: buy.order_id,
                sell_order_id: sell.order_id,
                prices,
                price_scale,
                outcome_count: 3,
                primitive_units: funding.primitive_units,
                consideration_price_units: consideration,
                state: PORTFOLIO_ENTITLEMENT_ACTIVE,
            };
            Self {
                terms,
                buy,
                sell,
                buyer_position,
                seller_position,
                buyer_reservation,
                seller_reservation,
                entitlement,
            }
        }

        fn apply(&mut self) -> PortfolioResult<PortfolioPairPlanV1> {
            apply_full_pair(
                &self.terms,
                &self.buy,
                &self.sell,
                &mut self.buyer_position,
                &mut self.seller_position,
                &mut self.buyer_reservation,
                &mut self.seller_reservation,
                &mut self.entitlement,
            )
        }
    }

    #[test]
    fn full_pair_moves_the_vector_and_consideration_once() {
        let mut f = Fixture::new();
        // Rebuild reservations on the entitlement's actual scale.
        for (order, reservation) in [
            (f.buy, &mut f.buyer_reservation),
            (f.sell, &mut f.seller_reservation),
        ] {
            let plan = ReservationPlan::for_order(
                &OrderSlot::Portfolio(order),
                3,
                f.entitlement.price_scale,
                0,
            )
            .unwrap();
            reservation.initial_cash_atoms = plan.cash_atoms;
            reservation.remaining_cash_atoms = plan.cash_atoms;
            reservation.initial_internal = plan.internal;
            reservation.remaining_internal = plan.internal;
        }
        f.buyer_position.reserved_cash_atoms = f.buyer_reservation.initial_cash_atoms;
        let plan = f.apply().unwrap();
        assert_eq!(plan.claim, f.entitlement.claim);
        assert_eq!(&plan.internal[..3], &[6, 12, 0]);
        assert_eq!(plan.consideration_atoms, 6);
        assert_eq!(&f.buyer_position.internal[..3], &[6, 12, 0]);
        assert_eq!(f.buyer_position.cash_atoms, 94);
        assert_eq!(f.buyer_position.reserved_cash_atoms, 0);
        assert_eq!(f.seller_position.cash_atoms, 16);
        assert_eq!(f.buyer_reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(f.seller_reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(f.entitlement.state, PORTFOLIO_ENTITLEMENT_CONSUMED);
        assert_eq!(f.apply(), Err(PortfolioSettlementError::AlreadyConsumed));
    }

    #[test]
    fn every_refusal_is_validate_before_mutate() {
        let mut base = Fixture::new();
        // Rebuild the buy plan at the entitlement scale, as the successful
        // fixture test does.
        let plan = ReservationPlan::for_order(
            &OrderSlot::Portfolio(base.buy),
            3,
            base.entitlement.price_scale,
            0,
        )
        .unwrap();
        base.buyer_reservation.initial_cash_atoms = plan.cash_atoms;
        base.buyer_reservation.remaining_cash_atoms = plan.cash_atoms;
        base.buyer_position.reserved_cash_atoms = plan.cash_atoms;

        let cases: [fn(&mut Fixture); 6] = [
            |f| f.entitlement.claim = h(0xaa),
            |f| f.entitlement.prices[0] += 1,
            |f| f.sell.coefficients[1] += 1,
            |f| f.buyer_reservation.remaining_cash_atoms -= 1,
            |f| f.buyer_position.cash_atoms = 0,
            |f| f.buyer_reservation.max_fee_atoms = 1,
        ];
        for mutate in cases {
            let mut f = base;
            mutate(&mut f);
            let before = f;
            assert!(f.apply().is_err());
            assert_eq!(f.buyer_position, before.buyer_position);
            assert_eq!(f.seller_position, before.seller_position);
            assert_eq!(f.buyer_reservation, before.buyer_reservation);
            assert_eq!(f.seller_reservation, before.seller_reservation);
            assert_eq!(f.entitlement, before.entitlement);
        }
    }

    #[test]
    fn entitlement_identity_binds_every_economic_coordinate() {
        let f = Fixture::new();
        assert_eq!(f.entitlement.validate(), Ok(()));
        let mutations: [fn(&mut PortfolioEntitlementV1); 8] = [
            |v| v.market = h(0xa0),
            |v| v.terms = h(0xa1),
            |v| v.claim = h(0xa2),
            |v| v.buy_order_id = canonical_order_id(3),
            |v| v.prices[0] -= 1,
            |v| v.price_scale -= 1,
            |v| v.primitive_units += 1,
            |v| v.consideration_price_units += 1,
        ];
        for mutate in mutations {
            let mut hostile = f.entitlement;
            mutate(&mut hostile);
            assert!(hostile.validate().is_err());
        }
    }

    #[test]
    fn live_promotion_stays_explicitly_blocked() {
        assert_eq!(PORTFOLIO_RUNTIME_BLOCKERS_V1.len(), 8);
        assert_eq!(
            PORTFOLIO_RUNTIME_BLOCKERS_V1[0],
            PortfolioRuntimeBlockerV1::CandidateSelection
        );
        assert_eq!(
            PORTFOLIO_RUNTIME_BLOCKERS_V1[7],
            PortfolioRuntimeBlockerV1::TerminalClosure
        );
    }
}
