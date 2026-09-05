//! The compiler-shaped entrance C-02 names, on the live V2 path.
//!
//! [`crate::compile_product_records_v2`] takes cuts and coefficients as caller
//! authority. Something upstream of it has to turn a human question into those
//! numbers, and until now the only thing in the tree that did was
//! `dclutch-product-compiler`, whose categorical entrance emits the SUPERSEDED
//! V1 record family and has no caller — measured 2026-09-01: `compile`,
//! `project_to_categorical_v1`, `compile_graded_basis_admission_v3` and every
//! V1 record constructor (`FiniteResultDomainV1::new`, `InstanceV1::new`,
//! `OccurrenceV1::new`, `CategoricalUnitV1::new`, `PortfolioTemplateV1::new`)
//! have zero construction sites outside `#[cfg(test)]` anywhere in `crates`,
//! `programs` or `tools`.
//!
//! So the shape layer is a real capability with no successor, and the V1
//! record layer is a successor-less path beside a live one. This module is the
//! successor for the half worth keeping: a named question becomes cuts,
//! coefficients and a quality report, and nothing else in the entrance gets to
//! choose a cut by hand.
//!
//! Every SPOT question here places its cuts from [`FoundingBandV1`] — spot at
//! founding and the market's own window — so "centred on spot, width scaled to
//! volatility times window" is a property of construction rather than of the
//! author's arithmetic.
//!
//! [`MarketQuestionV1::Proposition`] is the member that does not name spot,
//! because its coordinate does not have one. It places no cuts at all: a
//! proposition is one ordinary cell and the Product's own disclosed failure
//! outcome, and its belief is the prior the author stated rather than a walk.

use dclutch_product::ContentId;
use dclutch_product_compiler::partition_quality::{
    BandProfileV1, FoundingBandV1, FoundingBeliefV1, PartitionQualityReportV1, centred_cuts_v1,
    require_interesting_partition_v1,
};
use solana_program::pubkey::Pubkey;

use crate::{
    CompiledProductRecordsV2, Error, ProductCompilationInputV2, Result, compile_product_records_v2,
    quality_error,
};

/// What a market asks, in the vocabulary a person uses to ask it.
///
/// Each variant fixes BOTH the partition and the payoff, because the two are
/// one product decision. A shape that let a caller supply coefficients
/// separately would let them build a market whose bands are beautifully
/// centred and whose payoff ignores every one of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketQuestionV1 {
    /// "Is the coordinate at or above spot by founding-band offset `ticks`?"
    ///
    /// One cut, two ordinary cells. `ticks` is signed and measured in
    /// coordinate units from spot, so a threshold AT spot is `0` and the
    /// market is the even-money question by construction.
    ThresholdFromSpot {
        /// Signed offset of the single cut from spot, in coordinate units.
        ticks: i128,
        /// What the at-or-above cell pays.
        payout: u64,
    },
    /// "Does the coordinate leave a band around spot?"
    ///
    /// Two cuts, three ordinary cells, and the payoff is the range-protection
    /// shape the tree already founds: pay in either tail, nothing inside.
    /// The edges come from the founding band, which is the whole point — the
    /// shipped defaults for this shape were typed in another unit entirely.
    CentredRangeProtection {
        /// How the two edges are spaced. `Uniform` is the symmetric band.
        profile: BandProfileV1,
        /// What either tail pays.
        payout: u64,
    },
    /// "Which of `ordinary_cells` bands around spot does it land in?"
    ///
    /// Cells are placed by [`centred_cuts_v1`]. The payoff pays only the
    /// lowest and highest bands nothing and steps across the interior, so the
    /// partition is economically live rather than decorative.
    CentredBands {
        /// Ordinary cells, at least three.
        ordinary_cells: u32,
        /// How the interior gaps vary across the band.
        profile: BandProfileV1,
        /// What the single most central band pays; outer bands pay less.
        peak_payout: u64,
    },
    /// "Is this proposition proved inside the window?"
    ///
    /// NO cuts: one ordinary cell for the proved observation, and the
    /// Product's own disclosed failure outcome for a window that closes
    /// unproved. This is the shape of the relayed graduation market, and it is
    /// the narrowest market the protocol can emit.
    ///
    /// The probability is deliberately NOT a field here. It is the belief, it
    /// lives on [`FoundingBeliefV1::StatedProposition`], and stating it twice
    /// would put two authors on one number and owe somebody a check that they
    /// agreed. This variant carries only what a *payoff* needs.
    Proposition {
        /// What the proved cell pays.
        payout: u64,
    },
}

/// One authored partition, its payoff, and how the two look from spot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredProductV1 {
    /// Strictly increasing interior cuts over the band's denominator.
    pub cuts: Vec<i128>,
    /// One payout per outcome; the last is the explicit failure outcome.
    pub coefficients: Vec<u64>,
    /// Ex-ante outcome mass across the cells.
    pub report: PartitionQualityReportV1,
    /// Whether the payoff actually distinguishes the ordinary cells.
    ///
    /// A payoff constant across every ordinary cell makes the partition
    /// decorative: the market resolves somewhere, and the holder is paid the
    /// same either way. That is a legitimate product exactly once — resolution
    /// -failure cover, which needs no cuts at all — so this is reported rather
    /// than refused, and no question in [`MarketQuestionV1`] emits one.
    pub payoff_distinguishes_cells: bool,
}

/// Irreducible identities an authored product still has to be told.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoredIdentitiesV1 {
    /// Stable Product semantic identity.
    pub product_id: ContentId,
    /// Exact source coordinate domain.
    pub coordinate_domain_id: ContentId,
    /// Exact result unit.
    pub result_unit_id: ContentId,
    /// Exact native claim basis.
    pub claim_basis_id: ContentId,
    /// Product-selected liability basis.
    pub liability_basis_id: ContentId,
    /// Product-selected representation semantic release.
    pub representation_release_id: ContentId,
    /// Product-selected coordinate mapping semantic release.
    pub mapping_release_id: ContentId,
    /// Positive common exact portfolio denominator.
    pub portfolio_denominator: u64,
}

/// Turn one named question into cuts, coefficients and a quality report.
pub fn author_product_v1(
    belief: &FoundingBeliefV1,
    ceiling_bps: u32,
    question: MarketQuestionV1,
) -> Result<AuthoredProductV1> {
    // A spot question needs a spot; a proposition needs a prior. The pair is
    // checked here rather than made unrepresentable because the question and
    // the belief are two genuinely different authorings -- what the market
    // pays, and what the author thinks will happen -- and a market may state
    // either one first.
    let spot: Option<FoundingBandV1> = match belief {
        FoundingBeliefV1::SpotBand { band, .. } => Some(*band),
        FoundingBeliefV1::StatedProposition(_) => None,
    };
    let (cuts, coefficients) = match question {
        MarketQuestionV1::ThresholdFromSpot { ticks, payout } => {
            let band = spot.ok_or(Error::BeliefKindMismatch)?;
            if payout == 0 {
                return Err(Error::FoundingBand);
            }
            let cut = band.anchor.checked_add(ticks).ok_or(Error::WidthMismatch)?;
            (vec![cut], vec![0, payout, 0])
        }
        MarketQuestionV1::CentredRangeProtection { profile, payout } => {
            let band = spot.ok_or(Error::BeliefKindMismatch)?;
            if payout == 0 {
                return Err(Error::FoundingBand);
            }
            let cuts = centred_cuts_v1(&band, 3, profile).map_err(quality_error)?;
            (cuts, vec![payout, 0, payout, 0])
        }
        MarketQuestionV1::CentredBands {
            ordinary_cells,
            profile,
            peak_payout,
        } => {
            let band = spot.ok_or(Error::BeliefKindMismatch)?;
            if ordinary_cells < 3 || peak_payout == 0 {
                return Err(Error::FoundingBand);
            }
            let cuts = centred_cuts_v1(&band, ordinary_cells, profile).map_err(quality_error)?;
            (cuts, tent_payouts(ordinary_cells, peak_payout)?)
        }
        MarketQuestionV1::Proposition { payout } => {
            if spot.is_some() {
                // A proposition measured against a random walk around a
                // positive spot is exactly the market this gate exists to
                // refuse: its one ordinary cell takes 10,000 bps under every
                // possible band.
                return Err(Error::BeliefKindMismatch);
            }
            if payout == 0 {
                return Err(Error::FoundingBand);
            }
            (Vec::new(), vec![payout, 0])
        }
    };
    let report =
        require_interesting_partition_v1(&cuts, belief, ceiling_bps).map_err(quality_error)?;
    let ordinary = coefficients
        .len()
        .checked_sub(1)
        .ok_or(Error::WidthMismatch)?;
    let head = *coefficients.first().ok_or(Error::WidthMismatch)?;
    let payoff_distinguishes_cells = coefficients
        .get(..ordinary)
        .ok_or(Error::WidthMismatch)?
        .iter()
        .any(|payout| *payout != head);
    Ok(AuthoredProductV1 {
        cuts,
        coefficients,
        report,
        payoff_distinguishes_cells,
    })
}

/// Everything the authoring step needs, in the shape
/// [`ProductCompilationInputV2`] already states the compilation step's inputs.
///
/// The two steps are one call, so they take one input value; splitting the
/// belief, the ceiling, the question and the identities back out into loose
/// positional arguments is how a caller silently transposes two of them.
pub struct AuthoredCompilationInputV1<'a> {
    /// The founding belief the payoff is authored from.
    pub belief: &'a FoundingBeliefV1,
    /// Ex-ante share ceiling, in basis points, every cell must respect.
    pub ceiling_bps: u32,
    /// The question whose partition and payouts are authored.
    pub question: MarketQuestionV1,
    /// Canonical identities the compiled record graph is written under.
    pub identities: AuthoredIdentitiesV1,
}

/// Author one question and compile its live V2 record graph in one step.
pub fn compile_authored_product_records_v2(
    registry_program: Pubkey,
    input: AuthoredCompilationInputV1<'_>,
    product_output: &mut [u8],
    domain_output: &mut [u8],
    portfolio_output: &mut [u8],
) -> Result<(CompiledProductRecordsV2, AuthoredProductV1)> {
    let AuthoredCompilationInputV1 {
        belief,
        ceiling_bps,
        question,
        identities,
    } = input;
    let authored = author_product_v1(belief, ceiling_bps, question)?;
    let compiled = compile_product_records_v2(
        registry_program,
        ProductCompilationInputV2 {
            product_id: identities.product_id,
            coordinate_domain_id: identities.coordinate_domain_id,
            result_unit_id: identities.result_unit_id,
            claim_basis_id: identities.claim_basis_id,
            liability_basis_id: identities.liability_basis_id,
            representation_release_id: identities.representation_release_id,
            mapping_release_id: identities.mapping_release_id,
            cut_denominator: belief.denominator(),
            cuts: &authored.cuts,
            portfolio_denominator: identities.portfolio_denominator,
            coefficients: &authored.coefficients,
        },
        product_output,
        domain_output,
        portfolio_output,
    )?;
    Ok((compiled, authored))
}

/// Payouts that peak in the central band and fall away symmetrically.
///
/// Exact integers, floor division, and the peak is always reached, so the
/// vector always distinguishes at least two ordinary cells.
fn tent_payouts(ordinary_cells: u32, peak_payout: u64) -> Result<Vec<u64>> {
    let cells = usize::try_from(ordinary_cells).map_err(|_| Error::WidthMismatch)?;
    let last = cells.checked_sub(1).ok_or(Error::WidthMismatch)?;
    let mut payouts = Vec::with_capacity(cells.checked_add(1).ok_or(Error::WidthMismatch)?);
    for cell in 0..cells {
        // Distance from the middle in half-steps, so an even width has two
        // equally central cells rather than an arbitrary one.
        let doubled = cell.checked_mul(2).ok_or(Error::WidthMismatch)?;
        let half_steps = doubled.abs_diff(last);
        let reach = u64::try_from(last)
            .map_err(|_| Error::WidthMismatch)?
            .max(1);
        let fall = peak_payout
            .checked_mul(u64::try_from(half_steps).map_err(|_| Error::WidthMismatch)?)
            .and_then(|value| value.checked_div(reach))
            .ok_or(Error::WidthMismatch)?;
        payouts.push(peak_payout.saturating_sub(fall));
    }
    payouts.push(0);
    Ok(payouts)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// Raw signed price atoms at exponent -8, as the committed local Pyth
    /// fixture reports them.
    const SOL_USD_ANCHOR: i128 = 100_000_000;
    const CEILING: u32 = dclutch_product_compiler::partition_quality::MAX_CELL_EX_ANTE_SHARE_BPS_V1;

    fn band() -> FoundingBeliefV1 {
        FoundingBeliefV1::SpotBand {
            band: FoundingBandV1 {
                anchor: SOL_USD_ANCHOR,
                denominator: 1,
                volatility_bps: 200,
                window_slots: 10_000,
            },
            plausible_half_widths: 2,
        }
    }

    fn proposition(cell_probability_bps: &[u32]) -> FoundingBeliefV1 {
        FoundingBeliefV1::StatedProposition(
            dclutch_product_compiler::partition_quality::StatedPropositionV1 {
                denominator: 1,
                cell_probability_bps: cell_probability_bps.to_vec(),
            },
        )
    }

    #[test]
    fn a_threshold_at_spot_is_the_even_money_question() {
        let authored = author_product_v1(
            &band(),
            CEILING,
            MarketQuestionV1::ThresholdFromSpot {
                ticks: 0,
                payout: 1,
            },
        )
        .expect("threshold at spot");
        assert_eq!(authored.cuts, vec![SOL_USD_ANCHOR]);
        assert_eq!(authored.coefficients, vec![0, 1, 0]);
        // A symmetric triangular band split at its own peak is exactly even.
        assert_eq!(authored.report.cell_share_bps, vec![5_000, 5_000]);
        assert!(authored.payoff_distinguishes_cells);
    }

    #[test]
    fn a_threshold_far_from_spot_is_refused_rather_than_founded() {
        // Ten characteristic displacements out: the market has an answer
        // already, and this is exactly the shape ember named.
        let far = author_product_v1(
            &band(),
            CEILING,
            MarketQuestionV1::ThresholdFromSpot {
                ticks: 20_000_000,
                payout: 1,
            },
        );
        assert_eq!(far, Err(Error::DegenerateOutcomePartition));
        // POSITIVE CONTROL in the same run: one displacement out still founds,
        // so the refusal above is about placement and not about the checker.
        let near = author_product_v1(
            &band(),
            CEILING,
            MarketQuestionV1::ThresholdFromSpot {
                ticks: 2_000_000,
                payout: 1,
            },
        )
        .expect("one displacement out is still a question");
        assert_eq!(near.report.cell_share_bps, vec![8_750, 1_250]);
    }

    #[test]
    fn centred_range_protection_replaces_a_hand_typed_band() {
        let authored = author_product_v1(
            &band(),
            CEILING,
            MarketQuestionV1::CentredRangeProtection {
                profile: BandProfileV1::Uniform,
                payout: 1,
            },
        )
        .expect("centred range protection");
        assert_eq!(authored.coefficients, vec![1, 0, 1, 0]);
        // Two edges, symmetric about spot, from the band rather than a keyboard.
        // 2,000,000 of displacement over three cells is a 666,666-wide middle
        // band, and its midpoint is spot exactly.
        assert_eq!(authored.cuts, vec![99_666_667, 100_333_333]);
        assert_eq!((authored.cuts[0] + authored.cuts[1]) / 2, SOL_USD_ANCHOR);
        assert!(authored.payoff_distinguishes_cells);
        assert!(!authored.report.is_degenerate(CEILING));
    }

    #[test]
    fn centred_bands_pay_across_the_partition_they_state() {
        let authored = author_product_v1(
            &band(),
            CEILING,
            MarketQuestionV1::CentredBands {
                ordinary_cells: 5,
                profile: BandProfileV1::Uniform,
                peak_payout: 100,
            },
        )
        .expect("centred bands");
        assert_eq!(authored.cuts.len(), 4);
        // A tent over the cells: the central band pays most, the tails least,
        // and the explicit failure outcome pays nothing.
        // Five ordinary cells and the explicit failure outcome, which a market
        // nobody would buy protection from must never pay.
        assert_eq!(authored.coefficients, vec![0, 50, 100, 50, 0, 0]);
        assert!(authored.payoff_distinguishes_cells);
        assert_eq!(
            authored.report.cell_share_bps,
            vec![3_612, 900, 975, 900, 3_612]
        );
    }

    #[test]
    fn an_inert_payoff_is_reported_and_no_question_emits_one() {
        for question in [
            MarketQuestionV1::ThresholdFromSpot {
                ticks: 0,
                payout: 1,
            },
            MarketQuestionV1::CentredRangeProtection {
                profile: BandProfileV1::TightCentre,
                payout: 7,
            },
            MarketQuestionV1::CentredBands {
                ordinary_cells: 7,
                profile: BandProfileV1::TightCentre,
                peak_payout: 60,
            },
        ] {
            let authored = author_product_v1(&band(), CEILING, question).expect("authored");
            assert!(
                authored.payoff_distinguishes_cells,
                "{question:?} emitted a payoff its own partition cannot change"
            );
        }
        // The flag has teeth: the resolution-failure-cover payoff, which is
        // the one legitimate inert shape, reads as inert.
        let inert = [1_u64, 1, 1, 0];
        let head = inert[0];
        assert!(!inert[..3].iter().any(|payout| *payout != head));
    }

    /// The market this whole unit exists for: zero cuts, one ordinary cell,
    /// the Product's disclosed failure outcome, and a belief that is a stated
    /// probability rather than a walk around a spot.
    #[test]
    fn a_proposition_is_authored_from_its_prior_and_not_from_a_spot() {
        let authored = author_product_v1(
            &proposition(&[3_500]),
            CEILING,
            MarketQuestionV1::Proposition { payout: 1 },
        )
        .expect("a stated proposition is a question");
        assert!(authored.cuts.is_empty());
        assert_eq!(authored.coefficients, vec![1, 0]);
        assert_eq!(authored.report.cell_share_bps, vec![3_500]);
        assert_eq!(authored.report.unresolved_share_bps, 6_500);
        // One ordinary cell and the failure outcome: the payoff distinguishes
        // nothing ACROSS ordinary cells because there is only one, and that is
        // reported rather than refused, exactly as the flag's doc says.
        assert!(!authored.payoff_distinguishes_cells);
    }

    /// The gate did not become an exemption. A proposition whose author states
    /// a near-certain prior is refused, and so is one whose disclosed failure
    /// outcome takes the market.
    #[test]
    fn a_foregone_proposition_is_refused_and_a_genuine_one_is_not() {
        for prior in [vec![9_500_u32], vec![400]] {
            assert_eq!(
                author_product_v1(
                    &proposition(&prior),
                    CEILING,
                    MarketQuestionV1::Proposition { payout: 1 }
                ),
                Err(Error::DegenerateOutcomePartition),
                "prior {prior:?} is a foregone conclusion"
            );
        }
        author_product_v1(
            &proposition(&[4_200]),
            CEILING,
            MarketQuestionV1::Proposition { payout: 1 },
        )
        .expect("POSITIVE CONTROL: a 42% proposition is a question");
    }

    #[test]
    fn a_question_and_a_belief_of_different_kinds_refuse_by_their_own_name() {
        // A spot question against a prior: there is no spot to place cuts from.
        for question in [
            MarketQuestionV1::ThresholdFromSpot {
                ticks: 0,
                payout: 1,
            },
            MarketQuestionV1::CentredRangeProtection {
                profile: BandProfileV1::Uniform,
                payout: 1,
            },
            MarketQuestionV1::CentredBands {
                ordinary_cells: 5,
                profile: BandProfileV1::Uniform,
                peak_payout: 1,
            },
        ] {
            assert_eq!(
                author_product_v1(&proposition(&[3_500]), CEILING, question),
                Err(Error::BeliefKindMismatch),
                "{question:?} names spot and the belief has none"
            );
        }
        // And a proposition against a spot band, which is the exact market the
        // partition gate exists to refuse.
        assert_eq!(
            author_product_v1(
                &band(),
                CEILING,
                MarketQuestionV1::Proposition { payout: 1 }
            ),
            Err(Error::BeliefKindMismatch)
        );
    }

    #[test]
    fn zero_payouts_and_impossible_widths_refuse_by_name() {
        assert_eq!(
            author_product_v1(
                &band(),
                CEILING,
                MarketQuestionV1::ThresholdFromSpot {
                    ticks: 0,
                    payout: 0
                }
            ),
            Err(Error::FoundingBand)
        );
        assert_eq!(
            author_product_v1(
                &band(),
                CEILING,
                MarketQuestionV1::CentredBands {
                    ordinary_cells: 2,
                    profile: BandProfileV1::Uniform,
                    peak_payout: 1
                }
            ),
            Err(Error::FoundingBand)
        );
        assert_eq!(
            author_product_v1(
                &proposition(&[3_500]),
                CEILING,
                MarketQuestionV1::Proposition { payout: 0 }
            ),
            Err(Error::FoundingBand)
        );
    }
}
