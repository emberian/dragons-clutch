#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact integer portfolio value and full-simplex risk certificates.
//!
//! Dragon's Clutch issues complete sets of native Eggs against collateral. A
//! nonnegative coefficient vector over those Eggs pays the same convex
//! combination of its coefficients that the resolved Egg vector names. Its
//! minimum coefficient is consequently a guaranteed complete-set floor and
//! its maximum coefficient is an exact conservative cap over the full
//! simplex. An already-admitted simplex price vector marks that portfolio by
//! the same dot product.
//!
//! This crate proves only those arithmetic statements. It authenticates no
//! price, payoff, basis, account, oracle, or candidate, and it never describes
//! a supplied price as fair. Smooth-basis reachability may make the attainable
//! payout interval narrower than the full-simplex interval returned here.

pub use clutch_kernel::{MAX_OUTCOMES, MIN_OUTCOMES};

/// Refusals produced while validating or evaluating an economic certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// The active outcome prefix is smaller than two or wider than the kernel.
    InvalidOutcomeCount = 0,
    /// A Market or immutable Terms identity is the all-zero sentinel.
    ZeroIdentity = 1,
    /// An exact simplex cannot have a zero scale.
    ZeroPriceScale = 2,
    /// A price exceeds the declared scale.
    PriceExceedsScale = 3,
    /// An inactive price cell is nonzero.
    NoncanonicalPricePadding = 4,
    /// Active prices do not sum exactly to the declared scale.
    NoncanonicalSimplexSum = 5,
    /// An inactive payoff cell is nonzero.
    NoncanonicalPayoffPadding = 6,
    /// A zero payoff vector does not name a market instrument.
    ZeroPayoff = 7,
    /// A position certificate must cover at least one payoff-vector unit.
    ZeroPositionUnits = 8,
    /// Price and payoff capabilities name different Market/Terms/width facts.
    MismatchedPortfolioDomain = 9,
    /// A checked product or sum does not fit its frozen integer width.
    ArithmeticOverflow = 10,
}

/// Result type for exact market-quality arithmetic.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable Market and native-basis identity shared by prices and payoffs.
///
/// The byte identities are opaque to this arithmetic crate. An adapter must
/// authenticate their semantic owners before construction; this type prevents
/// accidentally valuing one Market's Eggs against another Market's prices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioDomainV1 {
    market_id: [u8; 32],
    terms_id: [u8; 32],
    outcome_count: u8,
}

impl PortfolioDomainV1 {
    /// Validate and capture an immutable Market/Terms/width binding.
    pub fn new(market_id: [u8; 32], terms_id: [u8; 32], outcome_count: u8) -> Result<Self> {
        validate_outcome_count(outcome_count)?;
        if is_zero_identity(&market_id) || is_zero_identity(&terms_id) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            market_id,
            terms_id,
            outcome_count,
        })
    }

    /// Adapter-authenticated Market identity.
    pub const fn market_id(&self) -> [u8; 32] {
        self.market_id
    }

    /// Adapter-authenticated immutable Terms/native-basis identity.
    pub const fn terms_id(&self) -> [u8; 32] {
        self.terms_id
    }

    /// Number of active native Eggs.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }
}

/// An already-quantized, exact, canonically padded simplex price vector.
///
/// Construction validates arithmetic shape only. It does not prove that the
/// price was selected by a market, admitted by a smooth-price measure
/// certificate, or observed from an economically meaningful venue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactSimplexPricesV1 {
    domain: PortfolioDomainV1,
    price_scale: u64,
    prices: [u64; MAX_OUTCOMES],
}

impl ExactSimplexPricesV1 {
    /// Validate and capture one exact integer simplex.
    pub fn new(
        domain: PortfolioDomainV1,
        price_scale: u64,
        prices: [u64; MAX_OUTCOMES],
    ) -> Result<Self> {
        let active = usize::from(domain.outcome_count);
        if price_scale == 0 {
            return Err(Error::ZeroPriceScale);
        }

        let mut sum = 0u128;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            let price = prices[outcome];
            if outcome < active {
                if price > price_scale {
                    return Err(Error::PriceExceedsScale);
                }
                sum = sum
                    .checked_add(u128::from(price))
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if price != 0 {
                return Err(Error::NoncanonicalPricePadding);
            }
            outcome += 1;
        }
        if sum != u128::from(price_scale) {
            return Err(Error::NoncanonicalSimplexSum);
        }

        Ok(Self {
            domain,
            price_scale,
            prices,
        })
    }

    /// Immutable Market/Terms/width binding for this price vector.
    pub const fn domain(&self) -> PortfolioDomainV1 {
        self.domain
    }

    /// Number of active price cells.
    pub const fn outcome_count(&self) -> u8 {
        self.domain.outcome_count
    }

    /// Exact integer simplex scale.
    pub const fn price_scale(&self) -> u64 {
        self.price_scale
    }

    /// Active price prefix followed by canonical zero padding.
    pub const fn prices(&self) -> &[u64; MAX_OUTCOMES] {
        &self.prices
    }
}

/// One nonnegative native Egg payoff vector per position unit.
///
/// A coefficient is measured in collateral atoms paid by its Egg component;
/// it is not a display decimal or a price-grid unit. The vector is allowed to
/// have zero active coefficients but cannot be entirely zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEggPortfolioV1 {
    domain: PortfolioDomainV1,
    egg_coefficients: [u64; MAX_OUTCOMES],
}

impl NativeEggPortfolioV1 {
    /// Validate canonical width, padding, and nonzero instrument shape.
    pub fn new(
        domain: PortfolioDomainV1,
        egg_coefficients: [u64; MAX_OUTCOMES],
    ) -> Result<Self> {
        let active = usize::from(domain.outcome_count);
        let mut nonzero = false;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            if outcome < active {
                nonzero |= egg_coefficients[outcome] != 0;
            } else if egg_coefficients[outcome] != 0 {
                return Err(Error::NoncanonicalPayoffPadding);
            }
            outcome += 1;
        }
        if !nonzero {
            return Err(Error::ZeroPayoff);
        }
        Ok(Self {
            domain,
            egg_coefficients,
        })
    }

    /// Immutable Market/Terms/width binding for this native portfolio.
    pub const fn domain(&self) -> PortfolioDomainV1 {
        self.domain
    }

    /// Number of active payoff cells.
    pub const fn outcome_count(&self) -> u8 {
        self.domain.outcome_count
    }

    /// Active collateral-atom coefficients followed by canonical zero padding.
    pub const fn egg_coefficients(&self) -> &[u64; MAX_OUTCOMES] {
        &self.egg_coefficients
    }
}

/// The sole conversion boundary in this kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RoundingBoundaryV1 {
    /// Divide once at the end and expose both adjacent collateral-atom values.
    FinalCollateralAtomFloorCeiling = 0,
}

/// Whole-atom envelope around one exact rational portfolio mark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralAtomEnvelopeV1 {
    /// Frozen rounding rule; callers cannot substitute per-leg rounding.
    pub boundary: RoundingBoundaryV1,
    /// Greatest whole collateral-atom value not above the exact mark.
    pub floor_atoms: u128,
    /// Smallest whole collateral-atom value not below the exact mark.
    pub ceiling_atoms: u128,
    /// Exact numerator retained after dividing by `price_scale`.
    pub remainder_price_atom_units: u64,
}

/// Checked arithmetic facts for one nonnegative coefficient-vector position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioValueRiskV1 {
    /// Immutable Market/Terms/width binding shared by both inputs.
    pub domain: PortfolioDomainV1,
    /// Number of payoff-vector units valued by this certificate.
    pub position_units: u64,
    /// Denominator of the exact position mark.
    pub price_scale: u64,
    /// `sum(price[i] * egg_coefficients[i])` for one portfolio unit.
    pub unit_mark_price_atom_numerator: u128,
    /// `position_units * unit_mark_price_atom_numerator` without division.
    pub position_mark_price_atom_numerator: u128,
    /// Sole whole-collateral-atom rounding envelope around the exact mark.
    pub mark_atoms: CollateralAtomEnvelopeV1,
    /// Exact full-simplex minimum payout, including complete-set value.
    pub guaranteed_floor_atoms: u128,
    /// Exact full-simplex maximum payout.
    pub worst_case_cap_atoms: u128,
    /// Exact contingent range: `worst_case_cap_atoms - guaranteed_floor_atoms`.
    pub contingent_range_atoms: u128,
}

/// Certify the exact simplex mark and full-simplex payoff bounds of a position.
///
/// The mark is the exact rational
/// `position_units * sum(price[i] * payoff[i]) / price_scale`. Products and the
/// sum are checked before the function returns. Division occurs exactly once,
/// after the complete dot product and position multiplication, and both
/// adjacent whole-atom values are retained.
///
/// The floor and cap are exact over the full simplex because its vertices
/// attain the minimum and maximum coefficient. They are conservative for a
/// smooth basis whose reachable vectors form a strict simplex subset.
pub fn certify_portfolio_value_risk_v1(
    prices: ExactSimplexPricesV1,
    payoff: NativeEggPortfolioV1,
    position_units: u64,
) -> Result<PortfolioValueRiskV1> {
    if prices.domain != payoff.domain {
        return Err(Error::MismatchedPortfolioDomain);
    }
    if position_units == 0 {
        return Err(Error::ZeroPositionUnits);
    }

    let active = usize::from(prices.domain.outcome_count);
    let mut minimum = u64::MAX;
    let mut maximum = 0u64;
    let mut unit_numerator = 0u128;
    let mut outcome = 0usize;
    while outcome < active {
        let coefficient = payoff.egg_coefficients[outcome];
        if coefficient < minimum {
            minimum = coefficient;
        }
        if coefficient > maximum {
            maximum = coefficient;
        }
        let term = u128::from(prices.prices[outcome])
            .checked_mul(u128::from(coefficient))
            .ok_or(Error::ArithmeticOverflow)?;
        unit_numerator = unit_numerator
            .checked_add(term)
            .ok_or(Error::ArithmeticOverflow)?;
        outcome += 1;
    }

    let units = u128::from(position_units);
    let position_numerator = unit_numerator
        .checked_mul(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let scale = u128::from(prices.price_scale);
    let floor_atoms = position_numerator / scale;
    let remainder = position_numerator % scale;
    let ceiling_atoms = if remainder == 0 {
        floor_atoms
    } else {
        floor_atoms
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?
    };

    let guaranteed_floor_atoms = u128::from(minimum)
        .checked_mul(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let worst_case_cap_atoms = u128::from(maximum)
        .checked_mul(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let contingent_range_atoms = u128::from(maximum - minimum)
        .checked_mul(units)
        .ok_or(Error::ArithmeticOverflow)?;
    let remainder_price_atom_units =
        u64::try_from(remainder).map_err(|_| Error::ArithmeticOverflow)?;

    Ok(PortfolioValueRiskV1 {
        domain: prices.domain,
        position_units,
        price_scale: prices.price_scale,
        unit_mark_price_atom_numerator: unit_numerator,
        position_mark_price_atom_numerator: position_numerator,
        mark_atoms: CollateralAtomEnvelopeV1 {
            boundary: RoundingBoundaryV1::FinalCollateralAtomFloorCeiling,
            floor_atoms,
            ceiling_atoms,
            remainder_price_atom_units,
        },
        guaranteed_floor_atoms,
        worst_case_cap_atoms,
        contingent_range_atoms,
    })
}

fn validate_outcome_count(outcome_count: u8) -> Result<usize> {
    if outcome_count < MIN_OUTCOMES || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(usize::from(outcome_count))
}

fn is_zero_identity(identity: &[u8; 32]) -> bool {
    let mut index = 0usize;
    while index < identity.len() {
        if identity[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(outcome_count: u8) -> PortfolioDomainV1 {
        PortfolioDomainV1::new([1u8; 32], [2u8; 32], outcome_count).unwrap()
    }

    fn cells(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0u64; MAX_OUTCOMES];
        values[0] = first;
        values[1] = second;
        values
    }

    #[test]
    fn exact_mark_and_full_simplex_bounds_share_one_atom_boundary() {
        let domain = domain(2);
        let prices = ExactSimplexPricesV1::new(domain, 4, cells(1, 3)).unwrap();
        let payoff = NativeEggPortfolioV1::new(domain, cells(2, 6)).unwrap();
        let certificate = certify_portfolio_value_risk_v1(prices, payoff, 3).unwrap();

        assert_eq!(certificate.domain, domain);
        assert_eq!(certificate.unit_mark_price_atom_numerator, 20);
        assert_eq!(certificate.position_mark_price_atom_numerator, 60);
        assert_eq!(certificate.price_scale, 4);
        assert_eq!(certificate.mark_atoms.floor_atoms, 15);
        assert_eq!(certificate.mark_atoms.ceiling_atoms, 15);
        assert_eq!(certificate.mark_atoms.remainder_price_atom_units, 0);
        assert_eq!(certificate.guaranteed_floor_atoms, 6);
        assert_eq!(certificate.worst_case_cap_atoms, 18);
        assert_eq!(certificate.contingent_range_atoms, 12);
    }

    #[test]
    fn inexact_mark_exposes_floor_ceiling_and_exact_remainder() {
        let domain = domain(2);
        let prices = ExactSimplexPricesV1::new(domain, 3, cells(1, 2)).unwrap();
        let payoff = NativeEggPortfolioV1::new(domain, cells(1, 2)).unwrap();
        let certificate = certify_portfolio_value_risk_v1(prices, payoff, 1).unwrap();

        assert_eq!(certificate.position_mark_price_atom_numerator, 5);
        assert_eq!(certificate.mark_atoms.floor_atoms, 1);
        assert_eq!(certificate.mark_atoms.ceiling_atoms, 2);
        assert_eq!(certificate.mark_atoms.remainder_price_atom_units, 2);
        assert_eq!(
            certificate.mark_atoms.boundary,
            RoundingBoundaryV1::FinalCollateralAtomFloorCeiling
        );
    }

    #[test]
    fn complete_set_shift_changes_value_but_not_contingent_range() {
        let domain = domain(2);
        let prices = ExactSimplexPricesV1::new(domain, 10, cells(3, 7)).unwrap();
        let base = NativeEggPortfolioV1::new(domain, cells(1, 4)).unwrap();
        let shifted = NativeEggPortfolioV1::new(domain, cells(6, 9)).unwrap();

        let base_certificate = certify_portfolio_value_risk_v1(prices, base, 2).unwrap();
        let shifted_certificate =
            certify_portfolio_value_risk_v1(prices, shifted, 2).unwrap();

        assert_eq!(base_certificate.contingent_range_atoms, 6);
        assert_eq!(shifted_certificate.contingent_range_atoms, 6);
        assert_eq!(shifted_certificate.guaranteed_floor_atoms, 12);
        assert_eq!(base_certificate.guaranteed_floor_atoms, 2);
        assert_eq!(
            shifted_certificate.position_mark_price_atom_numerator
                - base_certificate.position_mark_price_atom_numerator,
            100
        );
    }

    #[test]
    fn zero_price_can_hide_mark_but_never_the_risk_cap() {
        let domain = domain(2);
        let prices = ExactSimplexPricesV1::new(domain, 10, cells(10, 0)).unwrap();
        let payoff = NativeEggPortfolioV1::new(domain, cells(0, 99)).unwrap();
        let certificate = certify_portfolio_value_risk_v1(prices, payoff, 1).unwrap();

        assert_eq!(certificate.position_mark_price_atom_numerator, 0);
        assert_eq!(certificate.mark_atoms.floor_atoms, 0);
        assert_eq!(certificate.worst_case_cap_atoms, 99);
        assert_eq!(certificate.contingent_range_atoms, 99);
    }

    #[test]
    fn malformed_simplexes_and_padding_refuse() {
        assert_eq!(
            PortfolioDomainV1::new([1u8; 32], [2u8; 32], 1),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            PortfolioDomainV1::new([0u8; 32], [2u8; 32], 2),
            Err(Error::ZeroIdentity)
        );
        let domain = domain(2);
        assert_eq!(
            ExactSimplexPricesV1::new(domain, 0, cells(0, 0)),
            Err(Error::ZeroPriceScale)
        );
        assert_eq!(
            ExactSimplexPricesV1::new(domain, 10, cells(4, 5)),
            Err(Error::NoncanonicalSimplexSum)
        );
        assert_eq!(
            ExactSimplexPricesV1::new(domain, 10, cells(11, 0)),
            Err(Error::PriceExceedsScale)
        );

        let mut padded_price = cells(4, 6);
        padded_price[2] = 1;
        assert_eq!(
            ExactSimplexPricesV1::new(domain, 10, padded_price),
            Err(Error::NoncanonicalPricePadding)
        );

        let mut padded_payoff = cells(1, 2);
        padded_payoff[2] = 1;
        assert_eq!(
            NativeEggPortfolioV1::new(domain, padded_payoff),
            Err(Error::NoncanonicalPayoffPadding)
        );
        assert_eq!(
            NativeEggPortfolioV1::new(domain, cells(0, 0)),
            Err(Error::ZeroPayoff)
        );
    }

    #[test]
    fn domain_mismatch_zero_units_and_position_overflow_refuse() {
        let domain_two = domain(2);
        let prices = ExactSimplexPricesV1::new(domain_two, 1, cells(1, 0)).unwrap();
        let other_market = PortfolioDomainV1::new([3u8; 32], [2u8; 32], 2).unwrap();
        let foreign_payoff = NativeEggPortfolioV1::new(other_market, cells(1, 1)).unwrap();
        assert_eq!(
            certify_portfolio_value_risk_v1(prices, foreign_payoff, 1),
            Err(Error::MismatchedPortfolioDomain)
        );

        let mut three_payoffs = [0u64; MAX_OUTCOMES];
        three_payoffs[0] = 1;
        let payoff_three = NativeEggPortfolioV1::new(domain(3), three_payoffs).unwrap();
        assert_eq!(
            certify_portfolio_value_risk_v1(prices, payoff_three, 1),
            Err(Error::MismatchedPortfolioDomain)
        );

        let payoff = NativeEggPortfolioV1::new(domain_two, cells(1, 1)).unwrap();
        assert_eq!(
            certify_portfolio_value_risk_v1(prices, payoff, 0),
            Err(Error::ZeroPositionUnits)
        );

        let widest_prices = ExactSimplexPricesV1::new(
            domain_two,
            u64::MAX,
            cells(u64::MAX, 0),
        )
        .unwrap();
        let widest_payoff =
            NativeEggPortfolioV1::new(domain_two, cells(u64::MAX, 0)).unwrap();
        assert_eq!(
            certify_portfolio_value_risk_v1(widest_prices, widest_payoff, u64::MAX),
            Err(Error::ArithmeticOverflow)
        );
    }
}
