//! The one admissible market geometry of the ordinary Direct family.
//!
//! A market's geometry is ONE number, not a pair. Product Runtime V2 pins
//! `region_count = cut_count + 1` on decode and defines
//! `outcome_count = region_count + 1` -- the ordinary regions plus the explicit
//! failure outcome -- so
//!
//! ```text
//! outcome_count = cut_count + 2
//! ```
//!
//! exactly. A `(claims, cuts)` pair that does not satisfy it is not a geometry
//! the protocol can found: the canonical three-outcome demo is ONE cut, and a
//! four-outcome market is TWO. This module is where that arithmetic lives, so
//! no caller has to rediscover it from a record width.
//!
//! Nothing in the Direct artifact family is emitted per geometry. Every
//! runtime-width account the AccountProfile pins is stated as an affine
//! `(base, stride)` rule against the transaction's own Product tail count, and
//! the family declares no per-item account, so one artifact set and one set of
//! content identities serve every geometry. What a geometry IS good for is the
//! other direction: deciding whether a set of account observations states one
//! consistent market at all, and telling a founder what widths its market's
//! accounts must have.

use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
};
use dclutch_product_runtime_v2::{
    DOMAIN_CUT_BYTES, DOMAIN_HEADER_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
};

/// Exact per-outcome row width of a LiabilityBasis aggregate or Position.
pub const DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3: usize = 8;

/// Affine base the AccountProfile states for the Product result domain.
///
/// The domain record is `DOMAIN_HEADER_BYTES + cut_count * DOMAIN_CUT_BYTES`
/// wide while the profile's rule is affine in the OUTCOME count, and outcomes
/// run two ahead of cuts. Subtracting two strides from the header moves that
/// constant offset into the base, so one affine rule resolves to the record's
/// exact width at every geometry.
pub const DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3: usize =
    DOMAIN_HEADER_BYTES - 2 * DOMAIN_CUT_BYTES;

/// The smallest geometry Product Runtime V2 admits.
///
/// Zero cuts is one ordinary region plus the explicit failure outcome. A
/// one-outcome market would need a negative cut count and a result-domain
/// record shorter than its own header.
pub const DIRECT_ORDINARY_MINIMUM_OUTCOMES_V3: u32 = 2;

/// Stable refusal from geometry construction or recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOrdinaryGeometryErrorV3 {
    /// The outcome or cut count is outside what the protocol can found.
    Outcomes,
    /// A record width overflowed the protocol's u32 account coordinate.
    Width,
    /// The observations do not state one geometry.
    Inconsistent,
}

/// One admissible ordinary Direct market geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DirectOrdinaryGeometryV3 {
    outcomes: u32,
}

impl DirectOrdinaryGeometryV3 {
    /// The canonical demo geometry: three outcomes, one cut.
    pub const CANONICAL: Self = Self { outcomes: 3 };

    /// Construct one geometry from a market's outcome count.
    pub const fn from_outcome_count(outcomes: u32) -> Result<Self, DirectOrdinaryGeometryErrorV3> {
        if outcomes < DIRECT_ORDINARY_MINIMUM_OUTCOMES_V3 {
            return Err(DirectOrdinaryGeometryErrorV3::Outcomes);
        }
        Ok(Self { outcomes })
    }

    /// Construct one geometry from a Product result domain's cut count.
    pub const fn from_cut_count(cuts: u32) -> Result<Self, DirectOrdinaryGeometryErrorV3> {
        match cuts.checked_add(2) {
            Some(outcomes) => Self::from_outcome_count(outcomes),
            None => Err(DirectOrdinaryGeometryErrorV3::Outcomes),
        }
    }

    /// Exact native outcome count: ordinary regions plus explicit failure.
    pub const fn outcome_count(self) -> u32 {
        self.outcomes
    }

    /// Exact ordinary region count.
    pub const fn region_count(self) -> u32 {
        self.outcomes - 1
    }

    /// Exact Product result-domain cut count.
    pub const fn cut_count(self) -> u32 {
        self.outcomes - 2
    }

    /// Exact encoded Product result-domain record width.
    pub const fn result_domain_record_bytes(self) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
        affine(
            DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3,
            self.outcomes,
            DOMAIN_CUT_BYTES,
        )
    }

    /// Exact encoded Product portfolio record width.
    pub const fn portfolio_record_bytes(self) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
        affine(
            PORTFOLIO_HEADER_BYTES,
            self.outcomes,
            PORTFOLIO_COEFFICIENT_BYTES,
        )
    }

    /// Exact encoded Claims aggregate record width.
    pub const fn claims_aggregate_record_bytes(self) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
        affine(
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            self.outcomes,
            DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3,
        )
    }

    /// Exact encoded Claims Position record width.
    pub const fn claims_position_record_bytes(self) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
        affine(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            self.outcomes,
            DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3,
        )
    }

    /// Recover the one geometry a set of observed record widths states.
    ///
    /// The five widths are the runtime-width records the AccountProfile pins
    /// affinely, in profile coordinate order: Product portfolio, Claims
    /// aggregate, Product result domain, and the source and destination Claims
    /// Positions. Every one of them is affine in the SAME outcome count, so a
    /// set that resolves to more than one count does not describe a market and
    /// is refused rather than silently resolved to any of them.
    pub const fn from_observed_record_bytes(
        portfolio: u32,
        claims_aggregate: u32,
        result_domain: u32,
        source_position: u32,
        destination_position: u32,
    ) -> Result<Self, DirectOrdinaryGeometryErrorV3> {
        let outcomes = match count(
            portfolio,
            PORTFOLIO_HEADER_BYTES,
            PORTFOLIO_COEFFICIENT_BYTES,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let observed = [
            count(
                claims_aggregate,
                LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3,
            ),
            count(
                result_domain,
                DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3,
                DOMAIN_CUT_BYTES,
            ),
            count(
                source_position,
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3,
            ),
            count(
                destination_position,
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3,
            ),
        ];
        let mut index = 0;
        while index < observed.len() {
            match observed[index] {
                Ok(value) if value == outcomes => index += 1,
                Ok(_) => return Err(DirectOrdinaryGeometryErrorV3::Inconsistent),
                Err(error) => return Err(error),
            }
        }
        Self::from_outcome_count(outcomes)
    }
}

/// `base + count * stride`, refusing anything past the u32 account coordinate.
const fn affine(
    base: usize,
    count: u32,
    stride: usize,
) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
    let Some(tail) = (count as usize).checked_mul(stride) else {
        return Err(DirectOrdinaryGeometryErrorV3::Width);
    };
    let Some(total) = base.checked_add(tail) else {
        return Err(DirectOrdinaryGeometryErrorV3::Width);
    };
    if total > u32::MAX as usize {
        return Err(DirectOrdinaryGeometryErrorV3::Width);
    }
    Ok(total as u32)
}

/// The exact affine item count one observed width states, or a refusal.
const fn count(
    bytes: u32,
    base: usize,
    stride: usize,
) -> Result<u32, DirectOrdinaryGeometryErrorV3> {
    if base > u32::MAX as usize || stride == 0 || stride > u32::MAX as usize {
        return Err(DirectOrdinaryGeometryErrorV3::Width);
    }
    let Some(tail) = bytes.checked_sub(base as u32) else {
        return Err(DirectOrdinaryGeometryErrorV3::Inconsistent);
    };
    if tail % (stride as u32) != 0 {
        return Err(DirectOrdinaryGeometryErrorV3::Inconsistent);
    }
    Ok(tail / (stride as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical demo geometry is three outcomes and exactly one cut.
    #[test]
    fn the_canonical_geometry_is_three_outcomes_and_one_cut() {
        assert_eq!(DirectOrdinaryGeometryV3::CANONICAL.outcome_count(), 3);
        assert_eq!(DirectOrdinaryGeometryV3::CANONICAL.region_count(), 2);
        assert_eq!(DirectOrdinaryGeometryV3::CANONICAL.cut_count(), 1);
        assert_eq!(
            DirectOrdinaryGeometryV3::from_cut_count(1),
            Ok(DirectOrdinaryGeometryV3::CANONICAL)
        );
    }

    /// The geometry named as the journey's trading wall, and its widths.
    ///
    /// Four outcomes is two cuts. These are the exact account widths a
    /// four-outcome market must present to the Direct entry.
    #[test]
    fn the_four_outcome_geometry_is_two_cuts_at_named_widths() {
        let geometry = DirectOrdinaryGeometryV3::from_outcome_count(4).expect("four outcomes");
        assert_eq!(geometry.cut_count(), 2);
        assert_eq!(geometry.region_count(), 3);
        // 240 header + 2 cuts * 16.
        assert_eq!(geometry.result_domain_record_bytes(), Ok(272));
        // 208 header + 4 coefficients * 8.
        assert_eq!(geometry.portfolio_record_bytes(), Ok(240));
        // 256 header + 4 rows * 8.
        assert_eq!(geometry.claims_aggregate_record_bytes(), Ok(288));
        // 128 header + 4 rows * 8.
        assert_eq!(geometry.claims_position_record_bytes(), Ok(160));
    }

    /// The result-domain rule's affine base really is the record's own header
    /// two strides down, at every geometry -- not only at the canonical one.
    #[test]
    fn the_result_domain_rule_resolves_to_the_records_real_width() {
        for outcomes in DIRECT_ORDINARY_MINIMUM_OUTCOMES_V3..=64 {
            let geometry =
                DirectOrdinaryGeometryV3::from_outcome_count(outcomes).expect("geometry");
            let real = DOMAIN_HEADER_BYTES + (geometry.cut_count() as usize) * DOMAIN_CUT_BYTES;
            assert_eq!(
                geometry.result_domain_record_bytes(),
                Ok(u32::try_from(real).expect("width")),
                "the affine rule missed the real record width at {outcomes} outcomes"
            );
        }
    }

    /// Every geometry round-trips through its own observed widths.
    #[test]
    fn every_geometry_is_recovered_from_the_widths_it_states() {
        for outcomes in DIRECT_ORDINARY_MINIMUM_OUTCOMES_V3..=64 {
            let geometry =
                DirectOrdinaryGeometryV3::from_outcome_count(outcomes).expect("geometry");
            assert_eq!(
                DirectOrdinaryGeometryV3::from_observed_record_bytes(
                    geometry.portfolio_record_bytes().expect("portfolio"),
                    geometry.claims_aggregate_record_bytes().expect("aggregate"),
                    geometry.result_domain_record_bytes().expect("domain"),
                    geometry.claims_position_record_bytes().expect("source"),
                    geometry
                        .claims_position_record_bytes()
                        .expect("destination"),
                ),
                Ok(geometry)
            );
        }
    }

    /// A market below the protocol floor is refused, not resolved.
    ///
    /// One outcome needs a negative cut count, and its result-domain record
    /// would be one stride SHORTER than the header it must carry.
    #[test]
    fn a_market_below_the_protocol_floor_is_refused() {
        for outcomes in [0_u32, 1] {
            assert_eq!(
                DirectOrdinaryGeometryV3::from_outcome_count(outcomes),
                Err(DirectOrdinaryGeometryErrorV3::Outcomes)
            );
        }
        assert_eq!(
            DirectOrdinaryGeometryV3::from_observed_record_bytes(
                u32::try_from(PORTFOLIO_HEADER_BYTES + PORTFOLIO_COEFFICIENT_BYTES).expect("width"),
                u32::try_from(
                    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3
                )
                .expect("width"),
                u32::try_from(DIRECT_ORDINARY_DOMAIN_AFFINE_BASE_BYTES_V3 + DOMAIN_CUT_BYTES)
                    .expect("width"),
                u32::try_from(
                    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3
                )
                .expect("width"),
                u32::try_from(
                    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + DIRECT_ORDINARY_CLAIMS_ROW_BYTES_V3
                )
                .expect("width"),
            ),
            Err(DirectOrdinaryGeometryErrorV3::Outcomes)
        );
    }

    /// One record off the common count is refused rather than resolved to
    /// whichever of the two counts happens to be read first.
    #[test]
    fn observations_that_state_two_geometries_are_refused() {
        let three = DirectOrdinaryGeometryV3::CANONICAL;
        let four = DirectOrdinaryGeometryV3::from_outcome_count(4).expect("four");
        assert_eq!(
            DirectOrdinaryGeometryV3::from_observed_record_bytes(
                three.portfolio_record_bytes().expect("portfolio"),
                four.claims_aggregate_record_bytes().expect("aggregate"),
                three.result_domain_record_bytes().expect("domain"),
                three.claims_position_record_bytes().expect("source"),
                three.claims_position_record_bytes().expect("destination"),
            ),
            Err(DirectOrdinaryGeometryErrorV3::Inconsistent)
        );
        // And the reverse: the portfolio is read first, so a portfolio that
        // disagrees with four consistent records is refused just as loudly.
        assert_eq!(
            DirectOrdinaryGeometryV3::from_observed_record_bytes(
                four.portfolio_record_bytes().expect("portfolio"),
                three.claims_aggregate_record_bytes().expect("aggregate"),
                three.result_domain_record_bytes().expect("domain"),
                three.claims_position_record_bytes().expect("source"),
                three.claims_position_record_bytes().expect("destination"),
            ),
            Err(DirectOrdinaryGeometryErrorV3::Inconsistent)
        );
    }

    /// A width that is not on any item boundary is refused.
    #[test]
    fn a_width_off_the_item_stride_is_refused() {
        let three = DirectOrdinaryGeometryV3::CANONICAL;
        assert_eq!(
            DirectOrdinaryGeometryV3::from_observed_record_bytes(
                three.portfolio_record_bytes().expect("portfolio") + 1,
                three.claims_aggregate_record_bytes().expect("aggregate"),
                three.result_domain_record_bytes().expect("domain"),
                three.claims_position_record_bytes().expect("source"),
                three.claims_position_record_bytes().expect("destination"),
            ),
            Err(DirectOrdinaryGeometryErrorV3::Inconsistent)
        );
        // Shorter than the header the record must carry at all.
        assert_eq!(
            DirectOrdinaryGeometryV3::from_observed_record_bytes(
                0,
                three.claims_aggregate_record_bytes().expect("aggregate"),
                three.result_domain_record_bytes().expect("domain"),
                three.claims_position_record_bytes().expect("source"),
                three.claims_position_record_bytes().expect("destination"),
            ),
            Err(DirectOrdinaryGeometryErrorV3::Inconsistent)
        );
    }
}
