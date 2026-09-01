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
//! # The belief is a family, not a spot
//!
//! A spot and a volatility describe a coordinate that MOVES. Not every market
//! has one. "Did this token graduate?" is a four-state discriminant over a
//! terminal window; it has no spot, and a volatility in basis points *of spot*
//! denotes nothing about it. That is not a market without a belief — the
//! author believes `P(graduates) = x` as definitely as any price author
//! believes 200 bp/hour — it is a market whose belief is a different KIND.
//!
//! So [`FoundingBeliefV1`] is the family: [`FoundingBeliefV1::SpotBand`] is
//! the scalar member this module was born as, and
//! [`FoundingBeliefV1::StatedProposition`] is the propositional one. The
//! founding requirement is unchanged and total — **every market states a
//! belief, and there is no default in either kind**. What a founding path may
//! now do is match on the kind instead of assuming one.
//!
//! Two open holes close as consequences rather than as rulings. A zero-cut
//! partition scores 10,000 bps in its single ordinary cell under EVERY
//! possible spot band, because the triangular model assumes the coordinate
//! always lands somewhere in the partition; a proposition does not, and its
//! unproved mass lands on the Product's own disclosed failure outcome, which
//! [`PartitionQualityReportV1::unresolved_share_bps`] states. And the
//! narrowest market the protocol can emit — one ordinary cell plus that
//! failure outcome — is therefore a real question when its belief is
//! propositional and still a foregone conclusion when its belief is a spot the
//! partition cannot separate.
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
/// wall-clock hour at current Solana slot times.
///
/// This declaration is the SINGLE AUTHOR. The load simulator states its
/// per-market volatility against the same window and now READS this constant
/// out of this file (`tools/load-simulator/simlife.py`,
/// `_rust_u64_const`) rather than restating it; it refuses loudly if this
/// `pub const` is renamed or duplicated, so point that reader at the new owner
/// instead of copying the value back. The two-constants-agreeing-by-hand half
/// of the old lifting plan is done. What remains: one measured slot-time
/// profile to replace the provisional ten thousand.
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
/// Ember rules the ceiling the product actually wants.
///
/// **This is the ceiling on the author's ceiling.** A caller states its own
/// number, because which lopsidedness a product wants to admit is a product
/// decision — but it states it *at or below* this one, and
/// [`require_interesting_partition_v1`] refuses
/// [`CompileError::CellShareCeilingAboveMaximum`] above it.
///
/// It read as a default that bounded nothing until 2026-09-01: the author's
/// number was compared verbatim and bounded only `1..=10_000`, so an author
/// could state 10000 and admit every partition except an exactly-100% cell —
/// the gate switching itself off at the caller's word, with nothing in the
/// tree defending it. Every real caller already writes 9000, so making the
/// constant a real ceiling refuses nothing that is authored today.
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

/// The NAME of the model one report was measured under.
///
/// A name and nothing else. The model's parameters live on the belief that
/// selects it ([`FoundingBeliefV1`]), because a belief and its model are one
/// decision: carrying the parameters here too would make a spot band measured
/// under a categorical prior representable, and then someone would have to
/// check that the pair agreed. This enum exists so a report, a client and a
/// serialized measurement can say which approximation produced a number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionQualityModelV1 {
    /// Symmetric triangular mass over the plausible band, peaked at the
    /// founding coordinate and zero outside.
    ///
    /// Triangular rather than uniform on purpose: a uniform measure scores by
    /// interval *length*, which penalises exactly the shape a good market
    /// wants — fine cells near spot, coarse cells away from it.
    TriangularPlausibleBand,
    /// The author's own stated probability per ordinary cell, used verbatim.
    ///
    /// The most explicit modelling boundary this module has: there is no
    /// approximation in it at all, only the belief as written down.
    StatedCategoricalPrior,
}

/// A stated categorical prior over a partition's ordinary cells.
///
/// The propositional member of the belief family. A proposition market's
/// coordinate is a DISCRIMINANT rather than a scalar with a metric — the
/// relayed family's only observable returns the discriminant of a four-state
/// enum — so "volatility in basis points of spot" denotes nothing about it.
/// What it does have is a belief, and that belief is exactly as authored,
/// exactly as legible and exactly as bindable as a volatility is.
///
/// # The mass that lands on no ordinary cell
///
/// `cell_probability_bps` is NOT required to sum to unity, and the shortfall
/// is the whole point rather than slack. A terminal-window proposition is only
/// ever *proved* by becoming true; a market that is never proved walks its
/// deadline to the Product's own disclosed failure outcome, which is not an
/// ordinary cell of the partition. `10_000 - sum` is therefore real, stated,
/// ex-ante mass on a real outcome, and it is measured for degeneracy beside
/// the cells rather than discarded: a proposition the author believes at 500
/// bps is a market about its own failure, and refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatedPropositionV1 {
    /// Shared coordinate denominator; must equal the partition's own.
    pub denominator: u64,
    /// Ex-ante probability of each ordinary cell in basis points, in canonical
    /// partition order. Exactly `cuts.len() + 1` entries; sum at most 10,000.
    pub cell_probability_bps: Vec<u32>,
}

/// What the author believes about the outcome, and the model that measures it.
///
/// A belief and its model are ONE decision. "Symmetric-triangular over three
/// characteristic displacements of a 200 bp walk from spot 150.00" is a single
/// distribution, and splitting it into a band and a model was tenable only
/// while every market's belief was spot-shaped — it made the mismatched pair
/// representable and then owed somebody a check that the two agreed.
///
/// **The founding requirement is unchanged and total: every market states a
/// belief.** What changed is that they do not all state the same KIND of
/// belief. An absent belief still refuses by name, and there is still no
/// default — in either kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FoundingBeliefV1 {
    /// A coordinate that MOVES: spot at founding, a stated volatility over
    /// this market's own window, and how far the band is taken to reach.
    SpotBand {
        /// Founding observation, denominator, volatility and window.
        band: FoundingBandV1,
        /// How many characteristic displacements the band reaches each way.
        plausible_half_widths: u32,
    },
    /// A proposition that RESOLVES: the author's prior over the cells.
    StatedProposition(StatedPropositionV1),
}

impl FoundingBeliefV1 {
    /// The named model this belief is measured under.
    pub const fn model(&self) -> PartitionQualityModelV1 {
        match self {
            Self::SpotBand { .. } => PartitionQualityModelV1::TriangularPlausibleBand,
            Self::StatedProposition(_) => PartitionQualityModelV1::StatedCategoricalPrior,
        }
    }

    /// The coordinate denominator this belief is quoted over.
    ///
    /// A belief on another denominator would measure a different market than
    /// the one being compiled, which is why every entrance compares this
    /// against the partition's own rather than rescaling.
    pub const fn denominator(&self) -> u64 {
        match self {
            Self::SpotBand { band, .. } => band.denominator,
            Self::StatedProposition(prior) => prior.denominator,
        }
    }
}

/// How a stated shape distributes gap widths across a constructed band.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BandProfileV1 {
    /// Every finite cell the same width.
    Uniform,
    /// Fine near spot, coarse away from it, rescaled to the same total width.
    TightCentre,
}

/// What a partition looks like from the belief that founds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionQualityReportV1 {
    /// The model the shares were measured under.
    pub model: PartitionQualityModelV1,
    /// Characteristic displacement in coordinate numerator units.
    ///
    /// `None` under a stated prior: a proposition has no displacement, and a
    /// zero here would read as one that was measured and came out zero.
    pub characteristic_displacement: Option<i128>,
    /// Half-width of the plausible band in coordinate numerator units.
    ///
    /// `None` under a stated prior, for the same reason.
    pub plausible_half_width: Option<i128>,
    /// Ordinary cell holding the most ex-ante mass.
    pub dominant_cell: u32,
    /// That cell's share, in basis points.
    pub dominant_share_bps: u32,
    /// Every ordinary cell's share, in canonical partition order.
    pub cell_share_bps: Vec<u32>,
    /// Stated ex-ante mass that lands on NO ordinary cell, in basis points.
    ///
    /// Under the triangular spot model this is exactly zero, by construction
    /// and not by arithmetic: the plausible band lies entirely inside the
    /// partition, because the outermost cells are open tails. The cells' own
    /// shares can still sum to slightly under 10,000 there, and that residue
    /// is floor-rounding rather than mass on another outcome — attributing it
    /// to the failure outcome would be this module inventing a belief.
    ///
    /// Under a stated prior it is the author's own `10_000 - sum`, and it is
    /// the disclosed failure outcome's ex-ante share.
    pub unresolved_share_bps: u32,
}

impl PartitionQualityReportV1 {
    /// Whether one outcome takes at least `ceiling_bps` of the market.
    ///
    /// The second disjunct is not a second gate; it is the same gate applied
    /// to the outcome the partition does not name. A proposition whose author
    /// believes it 5% likely puts 9,500 bps on the disclosed failure outcome,
    /// and a market that is almost certainly going to fail to resolve is
    /// exactly as much a foregone conclusion as one whose middle cell takes
    /// everything. It cannot fire under the spot model, where
    /// `unresolved_share_bps` is zero.
    pub const fn is_degenerate(&self, ceiling_bps: u32) -> bool {
        self.dominant_share_bps >= ceiling_bps || self.unresolved_share_bps >= ceiling_bps
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

/// Refuse a cut vector that is not the strictly increasing canonical one.
///
/// Both members of the family need this and neither may skip it: a prior
/// stated cell by cell is meaningless against boundaries that do not order.
fn require_canonical_cuts(cuts: &[i128]) -> Result<(), CompileError> {
    let mut previous = None;
    for cut in cuts.iter().copied() {
        if previous.is_some_and(|prior| cut <= prior) {
            return Err(CompileError::NonCanonicalPartition);
        }
        previous = Some(cut);
    }
    Ok(())
}

/// Measure how one partition's ex-ante mass is spread across its cells.
///
/// `cuts` are the strictly increasing interior boundaries over
/// `belief.denominator()`; the partition has `cuts.len() + 1` ordinary cells.
/// Which model runs is the belief's own to say — there is one author of that
/// choice, [`FoundingBeliefV1::model`], and no caller passes a model in.
pub fn assess_partition_quality_v1(
    cuts: &[i128],
    belief: &FoundingBeliefV1,
) -> Result<PartitionQualityReportV1, CompileError> {
    require_canonical_cuts(cuts)?;
    match belief {
        FoundingBeliefV1::SpotBand {
            band,
            plausible_half_widths,
        } => assess_triangular_plausible_band(cuts, band, *plausible_half_widths),
        FoundingBeliefV1::StatedProposition(prior) => assess_stated_prior(cuts, prior),
    }
}

/// The scalar member: symmetric triangular mass over the plausible band.
fn assess_triangular_plausible_band(
    cuts: &[i128],
    band: &FoundingBandV1,
    plausible_half_widths: u32,
) -> Result<PartitionQualityReportV1, CompileError> {
    let characteristic_displacement = characteristic_displacement_v1(band)?;
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

    let mut edges = Vec::with_capacity(cuts.len());
    for cut in cuts.iter().copied() {
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
        model: PartitionQualityModelV1::TriangularPlausibleBand,
        characteristic_displacement: Some(characteristic_displacement),
        plausible_half_width: Some(half_width),
        dominant_cell,
        dominant_share_bps,
        cell_share_bps,
        // Zero by CONSTRUCTION: the plausible band lies wholly inside the
        // partition, whose outer cells are open tails. See the field's own doc
        // for why the cells' floor-rounding residue is not put here.
        unresolved_share_bps: 0,
    })
}

/// The propositional member: the author's own stated prior, used verbatim.
fn assess_stated_prior(
    cuts: &[i128],
    prior: &StatedPropositionV1,
) -> Result<PartitionQualityReportV1, CompileError> {
    let ordinary_cells = cuts.len().checked_add(1).ok_or(CompileError::CountOverflow)?;
    if prior.denominator == 0 {
        return Err(CompileError::ZeroCoordinateDenominator);
    }
    if prior.cell_probability_bps.len() != ordinary_cells {
        return Err(CompileError::MismatchedPriorWidth);
    }
    let mut stated = 0_u32;
    let mut dominant_cell = 0;
    let mut dominant_share_bps = 0;
    for (cell, share) in prior.cell_probability_bps.iter().copied().enumerate() {
        stated = stated
            .checked_add(share)
            .ok_or(CompileError::ArithmeticOverflow)?;
        if u64::from(stated) > BASIS_POINTS_PER_UNIT_V1 {
            // A prior that sums past unity is not a shortfall onto the failure
            // outcome; it is a belief that does not describe a probability.
            return Err(CompileError::PriorMassExceedsUnity);
        }
        if share > dominant_share_bps {
            dominant_share_bps = share;
            dominant_cell = u32::try_from(cell).map_err(|_| CompileError::CountOverflow)?;
        }
    }
    let unresolved_share_bps = u32::try_from(BASIS_POINTS_PER_UNIT_V1)
        .map_err(|_| CompileError::ArithmeticOverflow)?
        .checked_sub(stated)
        .ok_or(CompileError::ArithmeticOverflow)?;
    Ok(PartitionQualityReportV1 {
        model: PartitionQualityModelV1::StatedCategoricalPrior,
        characteristic_displacement: None,
        plausible_half_width: None,
        dominant_cell,
        dominant_share_bps,
        cell_share_bps: prior.cell_probability_bps.clone(),
        unresolved_share_bps,
    })
}

/// Measure a compiler partition, refusing when one cell takes the market.
///
/// The ceiling is a caller argument on purpose. A default that admitted
/// everything would be a check that never runs, and a hidden default would put
/// a product ruling inside a library.
///
/// It is a caller argument WITHIN A BOUND. The caller may state any ceiling at
/// or below [`MAX_CELL_EX_ANTE_SHARE_BPS_V1`]; above it the gate would be
/// weaker than the release admits, which is `CellShareCeilingAboveMaximum` and
/// not a band problem. Zero stays `UnsupportedFoundingBand`: a ceiling of zero
/// refuses every partition, so it is a malformed parameter rather than an
/// attempt to widen the gate.
pub fn require_interesting_partition_v1(
    cuts: &[i128],
    belief: &FoundingBeliefV1,
    ceiling_bps: u32,
) -> Result<PartitionQualityReportV1, CompileError> {
    if ceiling_bps == 0 {
        return Err(CompileError::UnsupportedFoundingBand);
    }
    if ceiling_bps > MAX_CELL_EX_ANTE_SHARE_BPS_V1 {
        return Err(CompileError::CellShareCeilingAboveMaximum);
    }
    let report = assess_partition_quality_v1(cuts, belief)?;
    if report.is_degenerate(ceiling_bps) {
        return Err(CompileError::DegenerateOutcomePartition);
    }
    Ok(report)
}

/// Measure one [`CanonicalPartition`] against its own founding belief.
pub fn assess_canonical_partition_v1(
    partition: &CanonicalPartition,
    belief: &FoundingBeliefV1,
) -> Result<PartitionQualityReportV1, CompileError> {
    if partition.domain().denominator != belief.denominator() {
        return Err(CompileError::MismatchedFoundingDenominator);
    }
    assess_partition_quality_v1(partition.cuts(), belief)
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

    fn sol_usd_band(window_slots: u64, volatility_bps: u32) -> FoundingBandV1 {
        FoundingBandV1 {
            anchor: LOCAL_PYTH_FIXTURE_COORDINATE,
            denominator: 1,
            volatility_bps,
            window_slots,
        }
    }

    /// The scalar belief these tests measure against: two displacements wide.
    fn two_wide(band: FoundingBandV1) -> FoundingBeliefV1 {
        FoundingBeliefV1::SpotBand {
            band,
            plausible_half_widths: 2,
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
        let report = assess_partition_quality_v1(&historical, &two_wide(band)).expect("assessed");
        assert_eq!(report.cell_share_bps, vec![0, 0, 0, 0, 10_000]);
        assert_eq!(report.dominant_cell, 4);
        assert_eq!(report.dominant_share_bps, 10_000);
        assert_eq!(
            require_interesting_partition_v1(
                &historical,
                &two_wide(band),
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
        let healthy =
            require_interesting_partition_v1(&centred, &two_wide(band), MAX_CELL_EX_ANTE_SHARE_BPS_V1)
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
                    assess_partition_quality_v1(&cuts, &two_wide(band)).expect("assessed band");
                assert_eq!(report.unresolved_share_bps, 0);
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
            assess_partition_quality_v1(&[100, 100], &two_wide(band)),
            Err(CompileError::NonCanonicalPartition)
        );
        assert_eq!(
            assess_partition_quality_v1(
                &[100],
                &FoundingBeliefV1::SpotBand {
                    band,
                    plausible_half_widths: 0
                }
            ),
            Err(CompileError::UnsupportedFoundingBand)
        );
        assert_eq!(
            require_interesting_partition_v1(&[100_000_000], &two_wide(band), 0),
            Err(CompileError::UnsupportedFoundingBand)
        );
        assert_eq!(
            require_interesting_partition_v1(&[100_000_000], &two_wide(band), 10_001),
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
        let direct = assess_partition_quality_v1(&cuts, &two_wide(band)).expect("direct");
        assert_eq!(
            assess_canonical_partition_v1(&partition, &two_wide(band)),
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
            assess_canonical_partition_v1(&mismatched, &two_wide(band)),
            Err(CompileError::MismatchedFoundingDenominator)
        );
    }

    /// One stated probability per ordinary cell, on the graduation market's
    /// own denominator.
    fn proposition(cell_probability_bps: &[u32]) -> FoundingBeliefV1 {
        FoundingBeliefV1::StatedProposition(StatedPropositionV1 {
            denominator: 1,
            cell_probability_bps: cell_probability_bps.to_vec(),
        })
    }

    /// THE ZERO-CUT HOLE, closed. The narrowest market the protocol can emit —
    /// one ordinary cell and the Product's disclosed failure outcome — is the
    /// shape of the tree's only non-price market, and it was refusable under
    /// EVERY possible spot band because the triangular model assumes the
    /// coordinate always lands in the partition.
    #[test]
    fn a_zero_cut_market_is_degenerate_under_a_spot_band_and_a_question_under_a_prior() {
        let band = sol_usd_band(10_000, 200);
        // The arithmetic certainty, restated as a test rather than as prose:
        // no band, however wide or narrow, moves this off 10,000 bps.
        for volatility_bps in [1_u32, 200, 5_000, MAX_BAND_VOLATILITY_BPS_V1] {
            for plausible_half_widths in [1_u32, 2, 3, 40] {
                let belief = FoundingBeliefV1::SpotBand {
                    band: FoundingBandV1 {
                        volatility_bps,
                        ..band
                    },
                    plausible_half_widths,
                };
                let report = assess_partition_quality_v1(&[], &belief).expect("assessed");
                assert_eq!(report.cell_share_bps, vec![10_000]);
                assert_eq!(
                    require_interesting_partition_v1(&[], &belief, MAX_CELL_EX_ANTE_SHARE_BPS_V1),
                    Err(CompileError::DegenerateOutcomePartition)
                );
            }
        }
        // The same partition, under the belief a graduation market actually
        // holds. P(graduates) = 35%: the cell takes 3,500 bps and the disclosed
        // failure outcome takes the other 6,500. Neither takes the market.
        let graduation = proposition(&[3_500]);
        let report = require_interesting_partition_v1(
            &[],
            &graduation,
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        )
        .expect("a stated proposition is a question");
        assert_eq!(report.model, PartitionQualityModelV1::StatedCategoricalPrior);
        assert_eq!(report.cell_share_bps, vec![3_500]);
        assert_eq!(report.unresolved_share_bps, 6_500);
        assert_eq!(report.dominant_share_bps, 3_500);
        assert_eq!(report.characteristic_displacement, None);
        assert_eq!(report.plausible_half_width, None);
    }

    /// The gate did not weaken to admit propositions; it grew a second kind
    /// and kept its teeth in both directions.
    #[test]
    fn a_foregone_proposition_refuses_from_either_end() {
        // Near-certain: the cell takes the market.
        assert_eq!(
            require_interesting_partition_v1(
                &[],
                &proposition(&[9_500]),
                MAX_CELL_EX_ANTE_SHARE_BPS_V1
            ),
            Err(CompileError::DegenerateOutcomePartition)
        );
        // Near-hopeless: the DISCLOSED FAILURE OUTCOME takes the market, which
        // no measure over ordinary cells alone could ever have seen.
        let hopeless = assess_partition_quality_v1(&[], &proposition(&[500])).expect("assessed");
        assert_eq!(hopeless.dominant_share_bps, 500);
        assert_eq!(hopeless.unresolved_share_bps, 9_500);
        assert_eq!(
            require_interesting_partition_v1(
                &[],
                &proposition(&[500]),
                MAX_CELL_EX_ANTE_SHARE_BPS_V1
            ),
            Err(CompileError::DegenerateOutcomePartition)
        );
        // POSITIVE CONTROL in the same run, so the two refusals above are
        // about placement rather than about the checker.
        require_interesting_partition_v1(&[], &proposition(&[3_500]), MAX_CELL_EX_ANTE_SHARE_BPS_V1)
            .expect("a 35% proposition is a question");
    }

    /// The stated ceiling is bounded by the release's own ceiling.
    ///
    /// `MAX_CELL_EX_ANTE_SHARE_BPS_V1` had ZERO production readers until
    /// 2026-09-01: the author's number was compared verbatim and bounded only
    /// `1..=10_000`, at the compiler and at both author-side validators. So an
    /// author could state 10000 and admit every partition except an
    /// exactly-100% cell — the gate switching itself off at the caller's word,
    /// with nothing in the tree defending it.
    ///
    /// The last two assertions are the ones that matter: they show the accused
    /// shape being ADMITTED under the old bound, so the refusal above them is
    /// about the ceiling and not about the checker.
    #[test]
    fn a_stated_ceiling_above_the_maximum_refuses_by_its_own_name() {
        let question = proposition(&[3_500]);
        // The number every real caller writes still admits.
        require_interesting_partition_v1(&[], &question, MAX_CELL_EX_ANTE_SHARE_BPS_V1)
            .expect("9000 is the release ceiling and admits");
        // One basis point above it, same partition, refuses by a name that says
        // which parameter is wrong rather than by the band's coarse code.
        assert_eq!(
            require_interesting_partition_v1(&[], &question, MAX_CELL_EX_ANTE_SHARE_BPS_V1 + 1),
            Err(CompileError::CellShareCeilingAboveMaximum)
        );
        assert_eq!(
            require_interesting_partition_v1(&[], &question, 10_000),
            Err(CompileError::CellShareCeilingAboveMaximum)
        );
        // Zero is still a malformed parameter, not an attempt to widen: a
        // ceiling of zero refuses every partition there is.
        assert_eq!(
            require_interesting_partition_v1(&[], &question, 0),
            Err(CompileError::UnsupportedFoundingBand)
        );
        // What the old bound cost. A 95% proposition is a foregone conclusion
        // this release refuses -- and a stated 10000 ADMITTED it.
        let foregone = assess_partition_quality_v1(&[], &proposition(&[9_500])).expect("assessed");
        assert!(foregone.is_degenerate(MAX_CELL_EX_ANTE_SHARE_BPS_V1));
        assert!(
            !foregone.is_degenerate(10_000),
            "a stated 10000 admitted a 95% foregone conclusion; that is what the bound now refuses"
        );
    }

    /// The B4/B6 tension: the narrowest EXECUTABLE Structured width is one
    /// ordinary cell plus the failure outcome, and the partition gate refused
    /// exactly that width. It is a real question when its belief is
    /// propositional and a foregone conclusion when its belief is a spot the
    /// partition cannot separate — which is a property of the market, not a
    /// width exemption.
    #[test]
    fn width_two_is_answered_by_the_kind_of_belief_and_never_by_an_exemption() {
        let spot = two_wide(sol_usd_band(10_000, 200));
        assert_eq!(
            require_interesting_partition_v1(&[], &spot, MAX_CELL_EX_ANTE_SHARE_BPS_V1),
            Err(CompileError::DegenerateOutcomePartition),
            "a width-two SPOT market is still degenerate, and no exemption saves it"
        );
        require_interesting_partition_v1(&[], &proposition(&[4_200]), MAX_CELL_EX_ANTE_SHARE_BPS_V1)
            .expect("a width-two PROPOSITIONAL market states a belief and passes");
        // And a width-three proposition splits across cells the same way.
        let three = require_interesting_partition_v1(
            &[7],
            &proposition(&[2_500, 3_000]),
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        )
        .expect("two cells and a failure outcome");
        assert_eq!(three.cell_share_bps, vec![2_500, 3_000]);
        assert_eq!(three.unresolved_share_bps, 4_500);
        assert_eq!(three.dominant_cell, 1);
    }

    #[test]
    fn a_malformed_prior_refuses_by_name_rather_than_being_renormalized() {
        // Rescaling a prior that does not describe a probability would answer
        // a question nobody asked, exactly as rescaling a denominator would.
        assert_eq!(
            assess_partition_quality_v1(&[], &proposition(&[6_000, 6_000])),
            Err(CompileError::MismatchedPriorWidth)
        );
        assert_eq!(
            assess_partition_quality_v1(&[7], &proposition(&[6_000, 6_000])),
            Err(CompileError::PriorMassExceedsUnity)
        );
        assert_eq!(
            assess_partition_quality_v1(&[7, 7], &proposition(&[1, 1, 1])),
            Err(CompileError::NonCanonicalPartition)
        );
        assert_eq!(
            assess_partition_quality_v1(
                &[],
                &FoundingBeliefV1::StatedProposition(StatedPropositionV1 {
                    denominator: 0,
                    cell_probability_bps: vec![3_500],
                })
            ),
            Err(CompileError::ZeroCoordinateDenominator)
        );
    }

    /// THE RELEASE LANE'S CONTROL. The SOL/USD market the devnet ladder is
    /// founding right now compiles to the same report it did before the belief
    /// became a family. Measured before the change and pinned here after.
    #[test]
    fn the_shipped_sol_usd_market_still_measures_3024_3950_3024() {
        let belief = FoundingBeliefV1::SpotBand {
            band: FoundingBandV1 {
                anchor: 15_000,
                denominator: 100,
                volatility_bps: 200,
                window_slots: 10_000,
            },
            plausible_half_widths: 3,
        };
        let report = require_interesting_partition_v1(&[14_800, 15_200], &belief, 9_000)
            .expect("the shipped SOL/USD band and cuts must compile");
        assert_eq!(report.cell_share_bps, vec![3_024, 3_950, 3_024]);
        assert_eq!(report.dominant_cell, 1);
        assert_eq!(report.dominant_share_bps, 3_950);
        assert_eq!(report.characteristic_displacement, Some(300));
        assert_eq!(report.plausible_half_width, Some(900));
        assert_eq!(report.model, PartitionQualityModelV1::TriangularPlausibleBand);
        // The new disjunct in `is_degenerate` cannot reach the price path.
        assert_eq!(report.unresolved_share_bps, 0);
    }
}
