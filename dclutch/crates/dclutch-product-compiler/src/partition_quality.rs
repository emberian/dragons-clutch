//! Whether a compiled partition is a question anybody has to think about.
//!
//! Exhaustive, disjoint, ordered and canonical are properties of the *shape*
//! of a partition. All four hold of a SOL/USD market whose cuts sit three
//! orders of magnitude below the coordinate the source will actually report,
//! and that market resolves into the same cell every time. A product entrance
//! that emits such a partition has satisfied the vocabulary and failed the
//! product, so this module adds the missing property: **how the ex-ante
//! outcome mass is distributed across the cells.**
//!
//! The measure needs a founding observation and a window, because "one cell
//! dominates" is meaningless without them. [`FoundingBandV1`] carries the spot
//! coordinate at founding, the market's own window in slots, and a stated
//! volatility in basis points of spot over a reference window. From those the
//! module derives one **characteristic displacement** — how far the coordinate
//! is taken to be able to travel by the deadline — scaling with the SQUARE
//! ROOT of the window, which is the random walk's own scaling and the only one
//! that leaves neither a long market's band absurdly narrow nor a short one's
//! absurdly wide.
//!
//! [`PartitionQualityModelV1`] then names how mass is spread over that
//! displacement. It is an explicitly named modelling boundary in the sense
//! [`crate::graded::GradedRoundingBoundaryV1`] already established for
//! rounding: a stated approximation with a name, never a claim that the
//! coordinate is really distributed this way. Every arithmetic step is exact
//! signed integer arithmetic and every overflow is refused.
//!
//! The same displacement also *constructs* good partitions:
//! [`centred_cuts_v1`] places cells around spot at founding with the width the
//! band implies, so the entrance can emit an interesting market rather than
//! only judge one.

use core::convert::TryFrom;

use crate::{CanonicalPartition, CompileError};

/// Basis points in one whole unit.
pub const BASIS_POINTS_PER_UNIT_V1: u64 = 10_000;

/// The window a stated volatility is quoted against, in slots.
///
/// Chain-derived shape, provisional value: ten thousand slots is roughly a
/// wall-clock hour at current Solana slot times, and it is the reference the
/// load simulator already states its own per-market volatility against
/// (`tools/load-simulator/simlife.py`, `BAND_WINDOW_REFERENCE_SLOTS_V1`).
/// Lifting plan: one measured slot-time profile shared by the simulator and
/// this compiler, replacing two constants that agree by hand.
pub const BAND_WINDOW_REFERENCE_SLOTS_V1: u64 = 10_000;

/// Largest stated volatility this release will scale a band from.
///
/// Provisional: a hundred thousand basis points is a ten-fold move over the
/// reference window, past which the band stops describing a market and starts
/// describing a rewrite of the coordinate. Lifting plan: a measured provider
/// volatility profile per feed.
pub const MAX_BAND_VOLATILITY_BPS_V1: u32 = 100_000;

/// Ceiling on one cell's share of ex-ante outcome mass, in basis points.
///
/// Provisional. Nine thousand basis points refuses the convicted defect — a
/// partition whose cells all sit outside the plausible band, where one cell
/// takes the whole ten thousand — without refusing a legitimately lopsided
/// binary threshold placed a displacement or so away from spot. Lifting plan:
/// Ember rules the ceiling the product actually wants; until then callers
/// state their own, because the ceiling is a product decision and this
/// constant is only the one the entrance defaults to naming.
pub const MAX_CELL_EX_ANTE_SHARE_BPS_V1: u32 = 9_000;

/// How steeply a shaped profile's gaps grow, in tenths of the spacing, per
/// half-step away from the middle of the band.
const PROFILE_SLOPE_TENTHS_V1: u64 = 4;

/// Founding observation and window a partition's quality is measured against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingBandV1 {
    /// Spot coordinate numerator at founding. Must be positive: a volatility
    /// in basis points *of spot* does not denote anything at or below zero.
    pub anchor: i128,
    /// Shared coordinate denominator; must equal the partition's own.
    pub denominator: u64,
    /// Stated volatility in basis points of `anchor` over the reference window.
    pub volatility_bps: u32,
    /// This market's own window, in slots, from founding to deadline.
    pub window_slots: u64,
}

/// Named ex-ante displacement model. Not a claim about the real distribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionQualityModelV1 {
    /// Symmetric triangular mass over `±plausible_half_widths` characteristic
    /// displacements, peaked at the founding coordinate and zero outside.
    ///
    /// Triangular rather than uniform on purpose: a uniform measure scores by
    /// interval *length*, which penalises exactly the shape a good market
    /// wants — fine cells near spot, coarse cells away from it.
    TriangularPlausibleBand {
        /// How many characteristic displacements the band reaches each way.
        plausible_half_widths: u32,
    },
}

/// How a stated shape distributes gap widths across a constructed band.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BandProfileV1 {
    /// Every finite cell the same width.
    Uniform,
    /// Fine near spot, coarse away from it, rescaled to the same total width.
    TightCentre,
}

/// What a partition looks like from the founding coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionQualityReportV1 {
    /// The model the shares were measured under.
    pub model: PartitionQualityModelV1,
    /// Characteristic displacement in coordinate numerator units.
    pub characteristic_displacement: i128,
    /// Half-width of the plausible band in coordinate numerator units.
    pub plausible_half_width: i128,
    /// Ordinary cell holding the most ex-ante mass.
    pub dominant_cell: u32,
    /// That cell's share, in basis points of the whole band.
    pub dominant_share_bps: u32,
    /// Every ordinary cell's share, in canonical partition order.
    pub cell_share_bps: Vec<u32>,
}

impl PartitionQualityReportV1 {
    /// Whether one cell takes at least `ceiling_bps` of the band.
    pub const fn is_degenerate(&self, ceiling_bps: u32) -> bool {
        self.dominant_share_bps >= ceiling_bps
    }
}

/// Exact characteristic displacement of one founding band.
///
/// `volatility_bps × sqrt(window / reference)` of the anchor, floored, and at
/// least one coordinate unit so a band is never empty.
pub fn characteristic_displacement_v1(band: &FoundingBandV1) -> Result<i128, CompileError> {
    if band.denominator == 0 {
        return Err(CompileError::ZeroCoordinateDenominator);
    }
    if band.anchor <= 0 {
        return Err(CompileError::NonPositiveFoundingAnchor);
    }
    if band.volatility_bps == 0
        || band.volatility_bps > MAX_BAND_VOLATILITY_BPS_V1
        || band.window_slots == 0
    {
        return Err(CompileError::UnsupportedFoundingBand);
    }
    let span_bps = u128::from(band.volatility_bps)
        .checked_mul(u128::from(band.window_slots.isqrt()))
        .ok_or(CompileError::ArithmeticOverflow)?
        .checked_div(u128::from(BAND_WINDOW_REFERENCE_SLOTS_V1.isqrt().max(1)))
        .ok_or(CompileError::ArithmeticOverflow)?
        .max(1);
    let displacement = band
        .anchor
        .unsigned_abs()
        .checked_mul(span_bps)
        .ok_or(CompileError::ArithmeticOverflow)?
        .checked_div(u128::from(BASIS_POINTS_PER_UNIT_V1))
        .ok_or(CompileError::ArithmeticOverflow)?
        .max(1);
    i128::try_from(displacement).map_err(|_| CompileError::ArithmeticOverflow)
}

/// Measure how one partition's ex-ante mass is spread across its cells.
///
/// `cuts` are the strictly increasing interior boundaries over
/// `band.denominator`; the partition has `cuts.len() + 1` ordinary cells.
pub fn assess_partition_quality_v1(
    cuts: &[i128],
    band: &FoundingBandV1,
    model: PartitionQualityModelV1,
) -> Result<PartitionQualityReportV1, CompileError> {
    let characteristic_displacement = characteristic_displacement_v1(band)?;
    let PartitionQualityModelV1::TriangularPlausibleBand {
        plausible_half_widths,
    } = model;
    if plausible_half_widths == 0 {
        return Err(CompileError::UnsupportedFoundingBand);
    }
    let half_width = characteristic_displacement
        .checked_mul(i128::from(plausible_half_widths))
        .ok_or(CompileError::ArithmeticOverflow)?;
    let total = half_width
        .checked_mul(half_width)
        .and_then(|square| square.checked_mul(2))
        .ok_or(CompileError::ArithmeticOverflow)?;

    let mut previous = None;
    let mut edges = Vec::with_capacity(cuts.len());
    for cut in cuts.iter().copied() {
        if previous.is_some_and(|prior| cut <= prior) {
            return Err(CompileError::NonCanonicalPartition);
        }
        previous = Some(cut);
        edges.push(
            cut.checked_sub(band.anchor)
                .ok_or(CompileError::ArithmeticOverflow)?,
        );
    }

    let mut cell_share_bps = Vec::with_capacity(edges.len().saturating_add(1));
    let mut dominant_cell = 0;
    let mut dominant_share_bps = 0;
    for cell in 0..=edges.len() {
        let lower = match cell.checked_sub(1).and_then(|index| edges.get(index)) {
            Some(edge) => *edge,
            None => -half_width,
        };
        let upper = match edges.get(cell) {
            Some(edge) => *edge,
            None => half_width,
        };
        let mass = triangular_antiderivative(half_width, upper)?
            .checked_sub(triangular_antiderivative(half_width, lower)?)
            .ok_or(CompileError::ArithmeticOverflow)?
            .max(0);
        let share = mass
            .checked_mul(i128::from(BASIS_POINTS_PER_UNIT_V1))
            .ok_or(CompileError::ArithmeticOverflow)?
            .checked_div(total)
            .ok_or(CompileError::ArithmeticOverflow)?;
        let share = u32::try_from(share).map_err(|_| CompileError::ArithmeticOverflow)?;
        if share > dominant_share_bps {
            dominant_share_bps = share;
            dominant_cell = u32::try_from(cell).map_err(|_| CompileError::CountOverflow)?;
        }
        cell_share_bps.push(share);
    }
    Ok(PartitionQualityReportV1 {
        model,
        characteristic_displacement,
        plausible_half_width: half_width,
        dominant_cell,
        dominant_share_bps,
        cell_share_bps,
    })
}

/// Measure a compiler partition, refusing when one cell takes the market.
///
/// The ceiling is a caller argument on purpose. A default that admitted
/// everything would be a check that never runs, and a hidden default would put
/// a product ruling inside a library.
pub fn require_interesting_partition_v1(
    cuts: &[i128],
    band: &FoundingBandV1,
    model: PartitionQualityModelV1,
    ceiling_bps: u32,
) -> Result<PartitionQualityReportV1, CompileError> {
    if ceiling_bps == 0 || u64::from(ceiling_bps) > BASIS_POINTS_PER_UNIT_V1 {
        return Err(CompileError::UnsupportedFoundingBand);
    }
    let report = assess_partition_quality_v1(cuts, band, model)?;
    if report.is_degenerate(ceiling_bps) {
        return Err(CompileError::DegenerateOutcomePartition);
    }
    Ok(report)
}

/// Measure one [`CanonicalPartition`] against its own founding band.
pub fn assess_canonical_partition_v1(
    partition: &CanonicalPartition,
    band: &FoundingBandV1,
    model: PartitionQualityModelV1,
) -> Result<PartitionQualityReportV1, CompileError> {
    if partition.domain().denominator != band.denominator {
        return Err(CompileError::MismatchedFoundingDenominator);
    }
    assess_partition_quality_v1(partition.cuts(), band, model)
}

/// Place `ordinary_cells` cells around spot at founding with the band's width.
///
/// The finite span of the partition is one characteristic displacement wide,
/// centred on the anchor, so the two open tails carry the rest of the plausible
/// band. `profile` shapes how the gaps vary and never rescales the total: a
/// profile is a shape over a width, not a second width.
pub fn centred_cuts_v1(
    band: &FoundingBandV1,
    ordinary_cells: u32,
    profile: BandProfileV1,
) -> Result<Vec<i128>, CompileError> {
    if ordinary_cells < 2 {
        return Err(CompileError::PartitionTooSmall);
    }
    let displacement = characteristic_displacement_v1(band)?;
    let cut_count = usize::try_from(ordinary_cells.saturating_sub(1))
        .map_err(|_| CompileError::CountOverflow)?;
    let spacing = displacement
        .checked_div(i128::from(ordinary_cells))
        .ok_or(CompileError::ArithmeticOverflow)?
        .max(1);
    let gaps = gap_widths(profile, spacing, cut_count.saturating_sub(1))?;
    let width: i128 = gaps
        .iter()
        .copied()
        .try_fold(0_i128, |total, gap| total.checked_add(gap))
        .ok_or(CompileError::ArithmeticOverflow)?;
    let mut cursor = band
        .anchor
        .checked_sub(width / 2)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let mut cuts = Vec::with_capacity(cut_count);
    cuts.push(cursor);
    for gap in gaps {
        cursor = cursor
            .checked_add(gap)
            .ok_or(CompileError::ArithmeticOverflow)?;
        cuts.push(cursor);
    }
    Ok(cuts)
}

fn gap_widths(
    profile: BandProfileV1,
    spacing: i128,
    gaps: usize,
) -> Result<Vec<i128>, CompileError> {
    if gaps == 0 {
        return Ok(Vec::new());
    }
    let count = u64::try_from(gaps).map_err(|_| CompileError::CountOverflow)?;
    let raw: Vec<u64> = match profile {
        BandProfileV1::Uniform => vec![10; gaps],
        BandProfileV1::TightCentre => (0..gaps)
            .map(|index| {
                let doubled = u64::try_from(index)
                    .unwrap_or(0)
                    .saturating_mul(2)
                    .saturating_add(1);
                let half_steps = doubled.abs_diff(count);
                10_u64.saturating_add(PROFILE_SLOPE_TENTHS_V1.saturating_mul(half_steps) / 2)
            })
            .collect(),
    };
    let total: u64 = raw
        .iter()
        .copied()
        .try_fold(0_u64, |sum, tenths| sum.checked_add(tenths))
        .ok_or(CompileError::ArithmeticOverflow)?;
    if total == 0 {
        return Err(CompileError::ArithmeticOverflow);
    }
    // Rescaled against the uniform total, so a profile is a shape over a width
    // and never a second width. The rescale is largest-remainder rather than
    // plain flooring: seven floors lose enough tenths to visibly narrow a band,
    // which would make the profile a scale again by the back door.
    let target = count
        .checked_mul(10)
        .ok_or(CompileError::ArithmeticOverflow)?;
    let mut scaled = Vec::with_capacity(gaps);
    let mut remainders = Vec::with_capacity(gaps);
    let mut assigned = 0_u64;
    for (index, tenths) in raw.iter().copied().enumerate() {
        let numerator = tenths
            .checked_mul(target)
            .ok_or(CompileError::ArithmeticOverflow)?;
        let whole = numerator / total;
        scaled.push(whole);
        remainders.push((numerator % total, index));
        assigned = assigned
            .checked_add(whole)
            .ok_or(CompileError::ArithmeticOverflow)?;
    }
    remainders.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    for (_, index) in remainders {
        if assigned >= target {
            break;
        }
        if let Some(entry) = scaled.get_mut(index) {
            *entry = entry.saturating_add(1);
            assigned = assigned.saturating_add(1);
        }
    }
    scaled
        .into_iter()
        .map(|tenths| {
            let width = spacing
                .checked_mul(i128::try_from(tenths).map_err(|_| CompileError::ArithmeticOverflow)?)
                .and_then(|value| value.checked_div(10))
                .ok_or(CompileError::ArithmeticOverflow)?;
            Ok(width.max(1))
        })
        .collect()
}

/// Signed antiderivative of the symmetric triangular mass, clamped to the band.
///
/// With half-width `w` the density is `w - |d|` on `[-w, w]`; this returns
/// twice its integral from zero, so every value stays an exact integer. The
/// whole band therefore measures `2w²`.
fn triangular_antiderivative(half_width: i128, displacement: i128) -> Result<i128, CompileError> {
    let clamped = displacement.clamp(-half_width, half_width);
    let magnitude = clamped.unsigned_abs();
    let width = half_width.unsigned_abs();
    let area = width
        .checked_mul(magnitude)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_sub(magnitude.checked_mul(magnitude)?))
        .ok_or(CompileError::ArithmeticOverflow)?;
    let area = i128::try_from(area).map_err(|_| CompileError::ArithmeticOverflow)?;
    Ok(if clamped < 0 { -area } else { area })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScaledDomain;

    /// The coordinate the committed local Pyth fixture actually reports: raw
    /// signed price atoms at exponent -8, so one SOL is 100,000,000.
    const LOCAL_PYTH_FIXTURE_COORDINATE: i128 = 100_000_000;

    const TWO_WIDE: PartitionQualityModelV1 = PartitionQualityModelV1::TriangularPlausibleBand {
        plausible_half_widths: 2,
    };

    fn sol_usd_band(window_slots: u64, volatility_bps: u32) -> FoundingBandV1 {
        FoundingBandV1 {
            anchor: LOCAL_PYTH_FIXTURE_COORDINATE,
            denominator: 1,
            volatility_bps,
            window_slots,
        }
    }

    #[test]
    fn characteristic_displacement_scales_with_the_square_root_of_the_window() {
        let short = characteristic_displacement_v1(&sol_usd_band(10_000, 200)).expect("short");
        let long = characteristic_displacement_v1(&sol_usd_band(200_000, 200)).expect("long");
        // 200 bp of 100,000,000 over the reference window.
        assert_eq!(short, 2_000_000);
        // Twenty times the window is about four and a half times the band, not
        // twenty: isqrt(200_000) = 447, isqrt(10_000) = 100.
        assert_eq!(long, 8_940_000);
        assert_eq!(
            characteristic_displacement_v1(&FoundingBandV1 {
                anchor: 0,
                ..sol_usd_band(10_000, 200)
            }),
            Err(CompileError::NonPositiveFoundingAnchor)
        );
        assert_eq!(
            characteristic_displacement_v1(&FoundingBandV1 {
                volatility_bps: 0,
                ..sol_usd_band(10_000, 200)
            }),
            Err(CompileError::UnsupportedFoundingBand)
        );
    }

    #[test]
    fn the_convicted_sol_usd_partition_is_refused_and_a_centred_one_is_not() {
        let band = sol_usd_band(10_000, 200);
        // THE DEFECT, exactly as it was: cuts drawn in USD cents per SOL while
        // the source reports price atoms, three orders of magnitude below the
        // coordinate. Every cut sits below the plausible band, so the top cell
        // takes the whole of it and the market resolves the same way forever.
        let historical = [4_000_i128, 12_000, 25_000, 40_000];
        let report = assess_partition_quality_v1(&historical, &band, TWO_WIDE).expect("assessed");
        assert_eq!(report.cell_share_bps, vec![0, 0, 0, 0, 10_000]);
        assert_eq!(report.dominant_cell, 4);
        assert_eq!(report.dominant_share_bps, 10_000);
        assert_eq!(
            require_interesting_partition_v1(
                &historical,
                &band,
                TWO_WIDE,
                MAX_CELL_EX_ANTE_SHARE_BPS_V1
            ),
            Err(CompileError::DegenerateOutcomePartition)
        );

        // THE POSITIVE CONTROL, in the same run: the same band, the same width,
        // cuts placed by this module instead. If the refusal above proved only
        // that the checker refuses everything, this line would refuse too.
        // Five cells over a 2,000,000-atom displacement: 400,000 per cell, a
        // 1,200,000-wide finite span whose midpoint is spot exactly.
        let centred = centred_cuts_v1(&band, 5, BandProfileV1::Uniform).expect("centred band");
        assert_eq!(
            centred,
            vec![99_400_000, 99_800_000, 100_200_000, 100_600_000]
        );
        assert_eq!((centred[0] + centred[3]) / 2, LOCAL_PYTH_FIXTURE_COORDINATE);
        let healthy = require_interesting_partition_v1(
            &centred,
            &band,
            TWO_WIDE,
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        )
        .expect("a centred band is not degenerate");
        // The band states a width of one characteristic displacement, so under
        // a two-displacement plausible band the open tails legitimately hold
        // most of the mass. What matters is that no single cell takes the
        // market: the largest is a tail at 36.12%.
        assert_eq!(healthy.cell_share_bps, vec![3_612, 900, 975, 900, 3_612]);
        assert_eq!(healthy.dominant_cell, 0);
        assert_eq!(healthy.dominant_share_bps, 3_612);
    }

    #[test]
    fn shares_are_exhaustive_symmetric_and_centred() {
        let band = sol_usd_band(10_000, 200);
        for cells in 2_u32..=12 {
            for profile in [BandProfileV1::Uniform, BandProfileV1::TightCentre] {
                let cuts = centred_cuts_v1(&band, cells, profile).expect("centred band");
                assert_eq!(
                    u32::try_from(cuts.len()).expect("width"),
                    cells.saturating_sub(1)
                );
                let report =
                    assess_partition_quality_v1(&cuts, &band, TWO_WIDE).expect("assessed band");
                let total: u32 = report.cell_share_bps.iter().copied().sum();
                // Floored shares, so the residue is bounded by the cell count.
                assert!(
                    total <= 10_000 && total + cells >= 10_000,
                    "{cells} cells on {profile:?} summed to {total}"
                );
                assert!(
                    !report.is_degenerate(MAX_CELL_EX_ANTE_SHARE_BPS_V1),
                    "{cells} cells on {profile:?} put {} bp in cell {}",
                    report.dominant_share_bps,
                    report.dominant_cell
                );
                // A centred band spends more mass inside than in either tail.
                let interior: u32 = report
                    .cell_share_bps
                    .get(1..report.cell_share_bps.len().saturating_sub(1))
                    .unwrap_or(&[])
                    .iter()
                    .copied()
                    .sum();
                let tails = report.cell_share_bps.first().copied().unwrap_or(0)
                    + report.cell_share_bps.last().copied().unwrap_or(0);
                if cells >= 4 {
                    assert!(
                        interior > 0 && tails < 10_000,
                        "{cells} cells on {profile:?} left nothing inside the band"
                    );
                }
            }
        }
    }

    #[test]
    fn a_tight_centre_profile_is_a_shape_and_never_a_second_width() {
        let band = sol_usd_band(10_000, 200);
        let uniform = centred_cuts_v1(&band, 9, BandProfileV1::Uniform).expect("uniform");
        let tight = centred_cuts_v1(&band, 9, BandProfileV1::TightCentre).expect("tight");
        let span = |cuts: &[i128]| cuts.last().copied().unwrap_or(0) - cuts[0];
        // Rescaled, so the two bands cover the same width to within the
        // per-gap flooring residue.
        assert!(span(&uniform).abs_diff(span(&tight)) <= 8);
        // And the middle gap is genuinely finer than the outermost one.
        let gap = |cuts: &[i128], index: usize| cuts[index + 1] - cuts[index];
        assert!(gap(&tight, 3) < gap(&tight, 0));
        assert_eq!(gap(&uniform, 3), gap(&uniform, 0));
    }

    #[test]
    fn hostile_partitions_bands_and_ceilings_refuse_by_name() {
        let band = sol_usd_band(10_000, 200);
        assert_eq!(
            assess_partition_quality_v1(&[100, 100], &band, TWO_WIDE),
            Err(CompileError::NonCanonicalPartition)
        );
        assert_eq!(
            assess_partition_quality_v1(
                &[100],
                &band,
                PartitionQualityModelV1::TriangularPlausibleBand {
                    plausible_half_widths: 0
                }
            ),
            Err(CompileError::UnsupportedFoundingBand)
        );
        assert_eq!(
            require_interesting_partition_v1(&[100_000_000], &band, TWO_WIDE, 0),
            Err(CompileError::UnsupportedFoundingBand)
        );
        assert_eq!(
            require_interesting_partition_v1(&[100_000_000], &band, TWO_WIDE, 10_001),
            Err(CompileError::UnsupportedFoundingBand)
        );
        assert_eq!(
            centred_cuts_v1(&band, 1, BandProfileV1::Uniform),
            Err(CompileError::PartitionTooSmall)
        );
        assert_eq!(
            characteristic_displacement_v1(&FoundingBandV1 {
                volatility_bps: MAX_BAND_VOLATILITY_BPS_V1 + 1,
                ..band
            }),
            Err(CompileError::UnsupportedFoundingBand)
        );
    }

    #[test]
    fn a_canonical_partition_is_measured_against_its_own_denominator() {
        let band = sol_usd_band(10_000, 200);
        let cuts = centred_cuts_v1(&band, 4, BandProfileV1::Uniform).expect("centred");
        let domain = ScaledDomain {
            lower: 0,
            upper: 200_000_000,
            denominator: 1,
        };
        let partition = CanonicalPartition::new(domain, cuts.clone()).expect("canonical");
        let direct = assess_partition_quality_v1(&cuts, &band, TWO_WIDE).expect("direct");
        assert_eq!(
            assess_canonical_partition_v1(&partition, &band, TWO_WIDE),
            Ok(direct)
        );
        let mismatched = CanonicalPartition::new(
            ScaledDomain {
                denominator: 100,
                ..domain
            },
            cuts,
        )
        .expect("canonical");
        assert_eq!(
            assess_canonical_partition_v1(&mismatched, &band, TWO_WIDE),
            Err(CompileError::MismatchedFoundingDenominator)
        );
    }
}
