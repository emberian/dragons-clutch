//! Terms-to-payout derivation over typed window evidence.
//!
//! Status: MODEL. This module implements §2 of
//! `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md`, generalized by the
//! unified `TermsAccount` v3 revision of
//! `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §6, as pure,
//! allocation-free, checked functions over already-decoded values. It reads
//! no clock, no signer, and no account; decoding hostile bytes, account
//! authentication, and aliasing all happen in the adapter before anything
//! here is reachable.
//!
//! Evidence-gated is **not** authenticated. A [`WindowResult`] is honest
//! evidence about a fold, never evidence about a source: the feed identity
//! bytes it is bound to are opaque 32-byte values that no crate in this
//! repository can relate to a real adapter, deployment, subject, or quote.
//!
//! # What the frozen layout supplies
//!
//! The v2 `TermsAccount` carried no statistic, ambiguity policy, boundary
//! table, coverage parameter, repair generation, or source identity, so V1 of
//! this module **pinned** each of them and recorded every pin as an
//! obligation on a terms revision (obligation 18 of
//! `SOLANA_REFERENCE_ADAPTER.md`). The v3 terms carry all of them inside the
//! self-certifying digest, and [`ResolutionTerms::from_market_terms`] now
//! *decodes* the frozen values instead of pinning:
//!
//! - the statistic, ambiguity policy, and edge policy are stored registered
//!   ids;
//! - the partition is the frozen knot vector — at degree 0 the knots are the
//!   interior cell boundaries of plan §2.2 **verbatim**, so a threshold
//!   market finally resolves (obligation 18 closes);
//! - the source-adapter identity and the source/evaluator versions are
//!   stored fields;
//! - the repair generation is a stored value under `GEN-EXACT-01`;
//! - the coverage-policy parameter is stored, so `BOUNDED_GAPS` coverage is
//!   now expressible.
//!
//! Everything that could change the answer stays inside the terms digest,
//! caller-steerable never.
//!
//! # Derived-basis (degree ≥ 1) markets, and the one named residue
//!
//! [`clutch_bspline`] is the exact evaluator used by derived degrees one
//! through three and independently pins degree zero's equivalent closed-top
//! categorical basis. Degrees two and three use the standard open-clamped
//! expansion of the frozen distinct breakpoints, including every end pane; no
//! undocumented interior formula substitutes for that evaluator.
//! `WEIGHT-ROUND-01` is
//! deterministic largest remainder across every smooth degree: floor each
//! exact scaled basis value, then award the remaining at-most-`degree` atoms
//! to the largest exact fractional remainders, with lowest outcome index
//! breaking ties. Earlier host-only degree-one code used a directional
//! residual rule. It had no deployed compatibility authority and is
//! intentionally superseded.
//!
//! [`derive_payout_vector`] is the §5.1 sibling seam: it produces the
//! validated, member-shaped weight vector. The kernel's design §4 transition
//! `clutch_kernel::MarketState::resolve_with_vector` now exists, and
//! `crate::resolve_derived_market` joins the two: the derived vector is
//! installed verbatim, with **no preset-membership step anywhere on that
//! path**. The reachable lattice is therefore no longer capped at
//! `MAX_PAYOUTS` members — for a two-outcome degree-1 market the whole
//! `D + 1` lattice resolves, nine vectors at `D = 8` against eight preset
//! slots — and [`ResolutionRefusal::DerivedVectorUnrepresentable`] has no
//! site to arise at there.
//!
//! The `Action::Resolve` **account** path still resolves by index, and
//! [`derive_payout`] therefore keeps its degree-1 preset-membership bridge.
//! That is not a kernel limit any more; it is a layout one. A redemption
//! takes its authority from a `ResolutionAccount`, whose frozen layout names
//! a payout *index* and carries no resolved value, and the reference kernel
//! account carries neither a basis-mode byte nor a resolved-vector slot. Both
//! are byte-layout facts owned by crates this one does not revise, so the
//! residue is now exactly one half wide: the layout half.
//!
//! Degrees 2 and 3 admit only point evidence after edge handling. An interval
//! with distinct endpoints refuses `R-15 NonPointEvidence`; no midpoint or
//! endpoint is silently substituted.

use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, StatisticError, WindowDomain, WindowError, WindowResult,
    MAX_VALUE,
};
use clutch_bspline::{
    BasisSpec, EdgePolicy as BasisEdgePolicy, Error as BasisError,
    WeightVector as BasisWeightVector,
};
use clutch_solana_layout::{
    Hash32, MarketAccount, PayoutVectorBytes, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS,
};

pub use clutch_solana_layout::PAYOUT_MAP_UNUSED;

/// Registered statistic: the terminal accepted interval of the sealed window.
pub const STAT_TERMINAL_01: u16 = 1;
/// Registered statistic: the conservative sampled minimum.
pub const STAT_SAMPLED_MIN_02: u16 = 2;
/// Registered statistic: the conservative sampled maximum.
pub const STAT_SAMPLED_MAX_03: u16 = 3;
/// Registered statistic: the exact time-weighted average ratio interval.
///
/// Admissible for degree-0 (boundary-table) markets only. A degree ≥ 1
/// weight derivation needs `⌊D·(num − t_k·den)/(g·den)⌋`, whose intermediate
/// product overflows `u128` headroom for large `D`; design §2.6 defers TWAP
/// with `d ≥ 1` until a checked 256-bit path, a narrowed `MAX_VALUE`, or a
/// provably small frozen `D` exists.
pub const STAT_TWAP_04: u16 = 4;
/// Registered statistic: terminal relative to TWAP. **Inadmissible.**
///
/// Plan §2.3: its comparison needs `low_numerator * scale` against
/// `boundary * low_denominator`, where both sides are already near
/// `8.64 x 10^34`. In `u128` that leaves a headroom factor of about
/// `3.9 x 10^3`, so admitting it requires a checked 256-bit comparison, a
/// narrowed `MAX_VALUE`, or a frozen scale small enough to prove the bound.
/// None of those exists, so this identifier is registered and refused.
pub const STAT_RELATIVE_TERMINAL_TWAP_05: u16 = 5;

/// Registered ambiguity policy `AMBIG-REFUSE-01`: the default, generalized.
///
/// Degree 0: refuse unless the conservative interval lies in one cell.
/// Degree 1: refuse unless the derived weight vectors at both interval ends
/// agree — which, by the monotonicity of `φ(x) = Σ i·w_i(x)` (design §5.3),
/// implies the weights are constant on the whole interval, so endpoint
/// equality never hides interior movement.
/// Degrees 2 and 3: refuse every non-point interval after edge handling;
/// endpoint equality alone is not a sound constancy witness for a smooth
/// basis.
pub const AMBIG_REFUSE_01: u8 = 1;
/// Registered ambiguity policy `AMBIG-COMPATIBLE-SET-02`: not implemented.
///
/// The kernel has no representation for a compatible cell set, and the policy
/// inherits §P1-A of `docs/implementation/ADVERSARIAL_REVIEW_V0.md`.
pub const AMBIG_COMPATIBLE_SET_02: u8 = 2;

/// Registered edge policy `EDGE-CLAMP-01`: evaluation clamps to the knot
/// span, so the extreme claims pay their full weight (flat extrapolation)
/// and the map from resolved value to weights is total.
///
/// Degree-0 markets must freeze this policy: their §2.2 partition already
/// covers the whole admitted domain, and clamping is the identity on it.
pub const EDGE_CLAMP_01: u8 = 1;
/// Registered edge policy `EDGE-REFUSE-02`: a resolved value outside
/// `[t_0, t_{K-1}]` refuses into the frozen failure policy.
pub const EDGE_REFUSE_02: u8 = 2;

/// Registered failure policy `FAIL-UNIFORM-REFUND-01`: recorded, not executed.
pub const FAIL_UNIFORM_REFUND_01: u8 = 1;
/// Registered failure policy `FAIL-EXTENDED-WINDOW-02`: recorded, not executed.
pub const FAIL_EXTENDED_WINDOW_02: u8 = 2;

/// Registered generation policy `GEN-EXACT-01`: the default.
///
/// The v3 terms store the pinned generation in
/// `TermsAccount::repair_generation`; V1 pinned zero because the frozen terms
/// had no field.
pub const GEN_EXACT_01: u8 = 1;
/// Registered generation policy `GEN-FINAL-AT-MATURITY-02`: not implemented.
///
/// It is blocked on a sealed feed-epoch object that can prove "no further
/// repair is possible after maturity". That object does not exist.
pub const GEN_FINAL_AT_MATURITY_02: u8 = 2;

/// Source-adapter version the V1 fixtures pin; v3 terms store the value.
pub const V1_SOURCE_VERSION: u32 = 1;
/// Evaluator version the V1 fixtures pin; v3 terms store the value.
pub const V1_EVALUATOR_VERSION: u32 = 1;
/// Repair generation the V1 fixtures pin; v3 terms store the value.
pub const V1_EXACT_GENERATION: u64 = 0;

/// Largest number of ordered cells in a settlement partition.
pub const MAX_CELLS: usize = MAX_OUTCOMES;
/// Largest number of degree-0 interior boundaries, one fewer than
/// [`MAX_CELLS`].
pub const MAX_BOUNDARIES: usize = MAX_CELLS - 1;

/// Every path out of [`derive_payout`] other than a selected payout index.
///
/// The classes are distinguishable on purpose: "the window was wrong" and
/// "the interval was ambiguous" have different operational responses. The
/// numbering is the `R-01..R-11` registry of plan §2.5 extended by the
/// design's §5.5 classes, exposed by [`ResolutionRefusal::class`] so that a
/// taxonomy code never depends on the enum's own discriminants. Class 15
/// `R-15 NonPointEvidence` is the load-bearing smooth-basis ambiguity refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionRefusal {
    /// R-01: the terms artifact is not the one this market's digest binds.
    TermsDigestMismatch,
    /// R-02: unregistered statistic, policy, or otherwise malformed terms.
    ///
    /// Also the class for registered but unimplemented policy variants:
    /// `AMBIG-COMPATIBLE-SET-02` and `GEN-FINAL-AT-MATURITY-02`.
    TermsMalformed,
    /// R-03: cell count out of range, non-increasing boundaries, empty cell.
    PartitionMalformed,
    /// R-04: the window result is bound to a different domain.
    ///
    /// Carries the accumulator's field-level reason, so a wrong feed, window,
    /// maturity bound, generation, grid, or coverage policy stays a
    /// distinguishable refusal instead of one opaque failure.
    WindowDomainMismatch(WindowError),
    /// R-05: the statistic is registered but not admitted for this family.
    ///
    /// Includes the §2.6 per-degree deferral: `STAT-TWAP-04` with degree ≥ 1.
    StatisticUnsupported,
    /// R-06: the conservative interval is ambiguous under `AMBIG-REFUSE-01` —
    /// it straddles cells (degree 0) or its endpoint weight vectors differ
    /// (degree 1). Degrees 2 and 3 use R-15 for every non-point interval.
    AmbiguousInterval,
    /// R-07: the sealed window carries no accepted bucket.
    NoAcceptedCoverage,
    /// R-08: a ratio statistic's denominator interval includes zero.
    AmbiguousDenominator,
    /// R-09: the selected cell maps outside the frozen payout set.
    PayoutIndexOutOfRange,
    /// R-10: the kernel market is not in `Phase::Active`.
    ///
    /// Enforced by the adapter before the derivation runs; the class exists
    /// here so the refusal taxonomy has exactly one owner.
    MarketNotActive,
    /// R-11: a checked comparison product overflowed.
    ArithmeticOverflow,
    /// R-12: the frozen basis shape is malformed — degree outside `0..=3`,
    /// the per-degree knot count rule broken, knots non-increasing or outside
    /// the admitted domain, a lying uniform-spacing declaration, a live
    /// payout map on a derived-basis market, or a freeze-time arithmetic
    /// bound (design §2.5) violation.
    BasisMalformed,
    /// R-13: a checked product in the weight derivation overflowed, or a
    /// constructed weight vector failed its own member-shape validation.
    ///
    /// Unreachable for terms admitted by the §2.5 freeze-time bound; kept for
    /// the same reason the TWAP product check is kept.
    WeightDerivationOverflow,
    /// R-14: `EDGE-REFUSE-02` and the resolved value lies outside
    /// `[t_0, t_{K-1}]`; control passes to the frozen failure policy.
    ValueOutOfRange,
    /// R-15: a degree-two or degree-three basis received non-point evidence
    /// after edge handling. Smooth interval semantics are not guessed.
    NonPointEvidence,
    /// R-16: the derived weight vector is valid but is not a member of the
    /// frozen preset set, so the *index-shaped* resolution record cannot name
    /// it.
    ///
    /// Unreachable on the derived-basis seam. `crate::resolve_derived_market`
    /// installs the derived vector through the kernel's `resolve_with_vector`
    /// and never consults the preset set, so no input to that path can reach
    /// this refusal; the variant is kept, not retired, because
    /// [`derive_payout`] still serves the `Action::Resolve` account path,
    /// where it remains live and load-bearing.
    ///
    /// What it now reports is the *layout* half of the design §4 residue: a
    /// `ResolutionAccount` names a payout index and carries no resolved value
    /// (design §6.3), and the reference kernel account carries neither a
    /// basis-mode byte nor a resolved-vector slot, so a market resolved to a
    /// non-preset vector has nowhere in the accounts to record what it
    /// resolved to. Fail-closed until those bytes exist; never approximated to
    /// the nearest preset.
    DerivedVectorUnrepresentable,
    /// R-17: the wrong resolution seam for this market's basis mode —
    /// [`derive_payout_vector`] invoked on a degree-0 (categorical) market,
    /// which resolves by index and has no derived vector.
    WrongResolutionMode,
}

impl ResolutionRefusal {
    /// The `R-nn` class number of plan §2.5 and design §5.5.
    pub const fn class(self) -> u8 {
        match self {
            Self::TermsDigestMismatch => 1,
            Self::TermsMalformed => 2,
            Self::PartitionMalformed => 3,
            Self::WindowDomainMismatch(_) => 4,
            Self::StatisticUnsupported => 5,
            Self::AmbiguousInterval => 6,
            Self::NoAcceptedCoverage => 7,
            Self::AmbiguousDenominator => 8,
            Self::PayoutIndexOutOfRange => 9,
            Self::MarketNotActive => 10,
            Self::ArithmeticOverflow => 11,
            Self::BasisMalformed => 12,
            Self::WeightDerivationOverflow => 13,
            Self::ValueOutOfRange => 14,
            Self::NonPointEvidence => 15,
            Self::DerivedVectorUnrepresentable => 16,
            Self::WrongResolutionMode => 17,
        }
    }
}

/// The immutable inputs a payout derivation is allowed to read.
///
/// Fields are public because this is a pure model value with no invariant of
/// its own: [`ResolutionTerms::validate`] is total and every derivation entry
/// point runs it before reading anything, so an ill-formed value is a refusal
/// rather than an unsound derivation. The adapter never accepts one from a
/// caller; it builds one with [`ResolutionTerms::from_market_terms`] from
/// bytes the market's frozen digest already commits to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionTerms {
    /// Market identity these terms were frozen for.
    pub market: Hash32,
    /// The exact expected window domain, §1.5 of the plan.
    pub window: WindowDomain,
    /// Registered statistic identifier.
    pub statistic: u16,
    /// Number of claims `n`, in `2..=MAX_CELLS`; at degree 0 this is the
    /// ordered cell count.
    pub cell_count: u8,
    /// B-spline basis degree; 0 is the boundary-table (categorical) market.
    pub basis_degree: u8,
    /// Registered edge-policy identifier.
    pub edge_policy: u8,
    /// Active knot count `K` under the per-degree count rule.
    pub knot_count: u8,
    /// Uniform-spacing declaration: `s` when every gap is `2^s`, else
    /// [`UNIFORM_SPACING_NONE`]. The knot array stays the semantic owner.
    pub uniform_log2_spacing: u8,
    /// Knot vector: degree-0 interior boundaries `b_0 .. b_{n-2}` (so
    /// `K = n − 1`), or distinct degree-`d` breakpoints with
    /// `K = n + 1 − d`. Entries at `>= K` are zero. For degrees two and
    /// three the breakpoints must declare uniform power-of-two spacing.
    pub knots: [u128; MAX_KNOTS],
    /// The payout set's common denominator `D`; the weight scale for
    /// degree ≥ 1.
    pub denominator: u64,
    /// Cell to payout-set index; entries at `>= n` are
    /// [`PAYOUT_MAP_UNUSED`], and every entry is unused for degree ≥ 1.
    pub payout_map: [u8; MAX_CELLS],
    /// Registered ambiguity policy identifier.
    pub ambiguity_policy: u8,
    /// Registered failure policy identifier; recorded, never executed here.
    pub failure_policy: u8,
    /// Registered generation policy identifier.
    pub generation_policy: u8,
    /// Number of live vectors in the frozen payout set.
    pub payout_count: u8,
}

impl ResolutionTerms {
    /// Canonical derivation from the market's frozen, digest-bound terms.
    ///
    /// Every value read here is inside `MarketAccount.terms`, which is why the
    /// result cannot be steered by a caller. The caller must still have proved
    /// [`TermsAccount::binds_market`]; this function re-checks the market
    /// identity so `R-01` has exactly one owner.
    pub fn from_market_terms(
        market: &MarketAccount,
        terms: &TermsAccount,
    ) -> Result<Self, ResolutionRefusal> {
        if market.terms != terms.terms {
            return Err(ResolutionRefusal::TermsDigestMismatch);
        }
        if terms.repair_policy_id != u32::from(GEN_EXACT_01) {
            // GEN-FINAL-AT-MATURITY-02 and every unregistered id refuse: the
            // generation a window must carry has to be pinned by immutable
            // terms, and only GEN-EXACT-01 pins one this build can name.
            return Err(ResolutionRefusal::TermsMalformed);
        }
        if terms.failure_policy_id != u32::from(FAIL_UNIFORM_REFUND_01)
            && terms.failure_policy_id != u32::from(FAIL_EXTENDED_WINDOW_02)
        {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        let coverage_id = u16::try_from(terms.coverage_policy_id)
            .map_err(|_| ResolutionRefusal::TermsMalformed)?;
        // The v3 terms carry the coverage parameter, so BOUNDED_GAPS is now
        // expressible; the registry still refuses an unregistered id or a
        // parameter outside its registered domain (a zero gap bound, or a
        // nonzero parameter under COMPLETE_REQUIRED).
        let coverage = CoveragePolicy::from_registry(coverage_id, terms.coverage_policy_parameter)
            .map_err(|_| ResolutionRefusal::TermsMalformed)?;
        let feed = FeedIdentity::new(
            terms.source_adapter_id.bytes(),
            terms.feed.bytes(),
            terms.source_version,
            terms.evaluator_version,
        )
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;
        let grid = Grid::new(
            terms.grid_family_id,
            terms.grid_version,
            terms.bucket_seconds,
        )
        .map_err(|_| ResolutionRefusal::TermsMalformed)?;
        let maturity = terms
            .expected_start_bucket
            .checked_add(terms.maturity_horizon_buckets)
            .ok_or(ResolutionRefusal::ArithmeticOverflow)?;
        let window = WindowDomain::new(
            feed,
            grid,
            terms.expected_start_bucket,
            terms.expected_end_bucket_exclusive,
            maturity,
            terms.repair_generation,
            coverage,
        )
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;

        let value = Self {
            market: market.market,
            window,
            statistic: terms.statistic_id,
            cell_count: terms.outcome_count,
            basis_degree: terms.basis_degree,
            edge_policy: terms.edge_policy_id,
            knot_count: terms.knot_count,
            uniform_log2_spacing: terms.uniform_log2_spacing,
            knots: terms.knots,
            denominator: terms.payouts[0].denominator,
            payout_map: terms.payout_map,
            ambiguity_policy: terms.ambiguity_policy_id,
            failure_policy: u8::try_from(terms.failure_policy_id)
                .map_err(|_| ResolutionRefusal::TermsMalformed)?,
            generation_policy: GEN_EXACT_01,
            payout_count: terms.payout_count,
        };
        value.validate()?;
        Ok(value)
    }

    /// Check the registry membership, the basis shape, and the payout map.
    ///
    /// Total, allocation free, and checked. Plan §2.2 for degree 0: strict
    /// increase gives disjointness and ordering, `0 < b_0` and the closed top
    /// cell give exhaustiveness with every cell non-empty. An empty cell is
    /// refused because it would mint a liability that can never pay. Design
    /// §2.1/§2.5 for derived degrees: strictly increasing breakpoints in
    /// the admitted domain, the required truthful spacing declaration, and
    /// the freeze-time arithmetic bound that makes every runtime overflow
    /// refusal defense in depth.
    pub fn validate(&self) -> Result<(), ResolutionRefusal> {
        if self.ambiguity_policy != AMBIG_REFUSE_01 {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        if self.generation_policy != GEN_EXACT_01 {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        if self.failure_policy != FAIL_UNIFORM_REFUND_01
            && self.failure_policy != FAIL_EXTENDED_WINDOW_02
        {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        let cells = usize::from(self.cell_count);
        if !(2..=MAX_CELLS).contains(&cells) {
            return Err(ResolutionRefusal::PartitionMalformed);
        }
        if self.payout_count == 0 {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        match self.basis_degree {
            0 => self.validate_degree_zero(cells),
            1..=3 => self.validate_derived(),
            _ => Err(ResolutionRefusal::BasisMalformed),
        }
    }

    fn validate_degree_zero(&self, cells: usize) -> Result<(), ResolutionRefusal> {
        match self.statistic {
            STAT_TERMINAL_01 | STAT_SAMPLED_MIN_02 | STAT_SAMPLED_MAX_03 | STAT_TWAP_04 => {}
            STAT_RELATIVE_TERMINAL_TWAP_05 => return Err(ResolutionRefusal::StatisticUnsupported),
            _ => return Err(ResolutionRefusal::TermsMalformed),
        }
        // The §2.2 partition already covers the whole admitted value domain,
        // so clamping is the identity on it; a refusing edge policy would
        // contradict the covering family and is not a boundary-table market.
        if self.edge_policy != EDGE_CLAMP_01 {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        if usize::from(self.knot_count) != cells - 1 {
            return Err(ResolutionRefusal::PartitionMalformed);
        }
        let mut previous = 0u128;
        let mut index = 0usize;
        while index < MAX_KNOTS {
            let boundary = self.knots[index];
            if index + 1 < cells {
                if boundary <= previous || boundary > MAX_VALUE {
                    return Err(ResolutionRefusal::PartitionMalformed);
                }
                previous = boundary;
            } else if boundary != 0 {
                return Err(ResolutionRefusal::PartitionMalformed);
            }
            index += 1;
        }
        let mut index = 0usize;
        while index < MAX_CELLS {
            let entry = self.payout_map[index];
            if index < cells {
                if entry >= self.payout_count {
                    return Err(ResolutionRefusal::PayoutIndexOutOfRange);
                }
            } else if entry != PAYOUT_MAP_UNUSED {
                return Err(ResolutionRefusal::PartitionMalformed);
            }
            index += 1;
        }
        Ok(())
    }

    fn validate_derived(&self) -> Result<(), ResolutionRefusal> {
        match self.statistic {
            STAT_TERMINAL_01 | STAT_SAMPLED_MIN_02 | STAT_SAMPLED_MAX_03 => {}
            // §2.6: TWAP with degree >= 1 is deferred until an overflow-proof
            // comparison path exists; it stays registered and refused.
            STAT_TWAP_04 | STAT_RELATIVE_TERMINAL_TWAP_05 => {
                return Err(ResolutionRefusal::StatisticUnsupported)
            }
            _ => return Err(ResolutionRefusal::TermsMalformed),
        }
        if self.edge_policy != EDGE_CLAMP_01 && self.edge_policy != EDGE_REFUSE_02 {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        // Derived mode has no payout map.
        let mut index = 0usize;
        while index < MAX_CELLS {
            if self.payout_map[index] != PAYOUT_MAP_UNUSED {
                return Err(ResolutionRefusal::BasisMalformed);
            }
            index += 1;
        }
        self.basis_spec()?;
        Ok(())
    }

    fn basis_spec(&self) -> Result<BasisSpec, ResolutionRefusal> {
        let edge_policy = match self.edge_policy {
            EDGE_CLAMP_01 => BasisEdgePolicy::Clamp,
            EDGE_REFUSE_02 => BasisEdgePolicy::Refuse,
            _ => return Err(ResolutionRefusal::TermsMalformed),
        };
        let spec = BasisSpec {
            outcome_count: self.cell_count,
            degree: self.basis_degree,
            knot_count: self.knot_count,
            uniform_log2_spacing: self.uniform_log2_spacing,
            denominator: self.denominator,
            domain_max: MAX_VALUE,
            edge_policy,
            knots: self.knots,
        };
        spec.validate().map_err(map_basis_error)?;
        Ok(spec)
    }

    /// The unique cell index containing `value`; at most 15 comparisons.
    ///
    /// Degree 0 only: the active knots are the interior boundaries.
    fn cell_of(&self, value: u128) -> u8 {
        let cells = usize::from(self.cell_count);
        let mut index = 0usize;
        while index + 1 < cells {
            if value < self.knots[index] {
                return index as u8;
            }
            index += 1;
        }
        (cells - 1) as u8
    }

    /// [`ResolutionTerms::cell_of`] for an exact ratio `numerator/denominator`.
    ///
    /// Plan §2.3: the product is bounded by `MAX_VALUE * MAX_BUCKETS *
    /// MAX_BUCKET_SECONDS ~ 8.64 x 10^34 < 2^128`, so the refusal below is
    /// unreachable for admitted terms. That bound is the reason the check is
    /// unreachable, not a reason to drop it.
    fn cell_of_ratio(&self, numerator: u128, denominator: u128) -> Result<u8, ResolutionRefusal> {
        if denominator == 0 {
            return Err(ResolutionRefusal::AmbiguousDenominator);
        }
        let cells = usize::from(self.cell_count);
        let mut index = 0usize;
        while index + 1 < cells {
            let scaled = self.knots[index]
                .checked_mul(denominator)
                .ok_or(ResolutionRefusal::ArithmeticOverflow)?;
            if numerator < scaled {
                return Ok(index as u8);
            }
            index += 1;
        }
        Ok((cells - 1) as u8)
    }

    /// Evaluate the registered statistic into a conservative pair of cells.
    fn conservative_cells(&self, window: &WindowResult) -> Result<(u8, u8), ResolutionRefusal> {
        match self.statistic {
            STAT_TERMINAL_01 => {
                let interval = window.terminal().map_err(map_statistic)?;
                Ok((self.cell_of(interval.low()), self.cell_of(interval.high())))
            }
            STAT_SAMPLED_MIN_02 => {
                let interval = window
                    .sampled_min()
                    .ok_or(ResolutionRefusal::NoAcceptedCoverage)?;
                Ok((self.cell_of(interval.low()), self.cell_of(interval.high())))
            }
            STAT_SAMPLED_MAX_03 => {
                let interval = window
                    .sampled_max()
                    .ok_or(ResolutionRefusal::NoAcceptedCoverage)?;
                Ok((self.cell_of(interval.low()), self.cell_of(interval.high())))
            }
            STAT_TWAP_04 => {
                let ratio = window.twap().map_err(map_statistic)?;
                let denominator = ratio.denominator();
                Ok((
                    self.cell_of_ratio(ratio.numerator_low(), denominator)?,
                    self.cell_of_ratio(ratio.numerator_high(), denominator)?,
                ))
            }
            STAT_RELATIVE_TERMINAL_TWAP_05 => Err(ResolutionRefusal::StatisticUnsupported),
            _ => Err(ResolutionRefusal::TermsMalformed),
        }
    }

    /// Evaluate the registered statistic into a conservative value interval.
    ///
    /// Degree ≥ 1 only: the integer-valued statistics. TWAP is deferred by
    /// §2.6 and refused at [`ResolutionTerms::validate`]; the arm here keeps
    /// the refusal reachable for a value that skipped validation.
    fn conservative_values(
        &self,
        window: &WindowResult,
    ) -> Result<(u128, u128), ResolutionRefusal> {
        match self.statistic {
            STAT_TERMINAL_01 => {
                let interval = window.terminal().map_err(map_statistic)?;
                Ok((interval.low(), interval.high()))
            }
            STAT_SAMPLED_MIN_02 => {
                let interval = window
                    .sampled_min()
                    .ok_or(ResolutionRefusal::NoAcceptedCoverage)?;
                Ok((interval.low(), interval.high()))
            }
            STAT_SAMPLED_MAX_03 => {
                let interval = window
                    .sampled_max()
                    .ok_or(ResolutionRefusal::NoAcceptedCoverage)?;
                Ok((interval.low(), interval.high()))
            }
            _ => Err(ResolutionRefusal::StatisticUnsupported),
        }
    }

    /// Apply the frozen edge policy to a conservative interval.
    fn apply_edge_policy(&self, low: u128, high: u128) -> Result<(u128, u128), ResolutionRefusal> {
        let first = self.knots[0];
        let last = self.knots[usize::from(self.knot_count) - 1];
        match self.edge_policy {
            EDGE_CLAMP_01 => Ok((low.clamp(first, last), high.clamp(first, last))),
            EDGE_REFUSE_02 => {
                if low < first || high > last {
                    return Err(ResolutionRefusal::ValueOutOfRange);
                }
                Ok((low, high))
            }
            _ => Err(ResolutionRefusal::TermsMalformed),
        }
    }

    /// The exact degree-one through degree-three basis at one edge-handled value.
    ///
    /// `clutch-bspline` is the semantic owner of knot expansion, clamped end
    /// panes, exact rational recurrence, and `WEIGHT-ROUND-01`.
    fn weights_at(&self, value: u128) -> Result<[u64; MAX_OUTCOMES], ResolutionRefusal> {
        self.basis_spec()?.evaluate(value).map_or_else(
            |error| Err(map_basis_error(error)),
            |vector: BasisWeightVector| Ok(vector.weights),
        )
    }

    /// Derive, edge-handle, and ambiguity-check the weight vector.
    ///
    /// Assumes [`ResolutionTerms::validate`] and the window domain check have
    /// already run; the public entry points do both.
    fn derived_vector(
        &self,
        window: &WindowResult,
    ) -> Result<PayoutVectorBytes, ResolutionRefusal> {
        let (low, high) = self.conservative_values(window)?;
        let (low, high) = self.apply_edge_policy(low, high)?;
        if self.basis_degree >= 2 && low != high {
            return Err(ResolutionRefusal::NonPointEvidence);
        }
        let low_weights = self.weights_at(low)?;
        let high_weights = self.weights_at(high)?;
        if low_weights != high_weights {
            // The generalized AMBIG-REFUSE-01 (§5.3): for degree 1, endpoint
            // equality implies the weights are constant on the whole interval
            // by the monotonicity of φ(x) = Σ i·w_i(x), so this refusal never
            // approves an interval whose interior moved.
            return Err(ResolutionRefusal::AmbiguousInterval);
        }
        let vector = PayoutVectorBytes {
            denominator: self.denominator,
            weights: low_weights,
        };
        // Member shape, checked: nonnegative (structural), bounded by D, zero
        // beyond the active prefix, sum exactly D. True by construction;
        // refused rather than assumed.
        vector
            .validate_active(self.cell_count, self.denominator)
            .map_err(|_| ResolutionRefusal::WeightDerivationOverflow)?;
        Ok(vector)
    }
}

fn map_statistic(error: StatisticError) -> ResolutionRefusal {
    match error {
        StatisticError::NoAcceptedCoverage => ResolutionRefusal::NoAcceptedCoverage,
        StatisticError::UnsupportedPredicate => ResolutionRefusal::StatisticUnsupported,
        StatisticError::AmbiguousDenominator => ResolutionRefusal::AmbiguousDenominator,
        StatisticError::ArithmeticOverflow => ResolutionRefusal::ArithmeticOverflow,
    }
}

fn map_basis_error(error: BasisError) -> ResolutionRefusal {
    match error {
        BasisError::ValueOutOfRange => ResolutionRefusal::ValueOutOfRange,
        BasisError::ArithmeticOverflow | BasisError::InvalidWeights => {
            ResolutionRefusal::WeightDerivationOverflow
        }
        BasisError::InvalidOutcomeCount
        | BasisError::InvalidDegree
        | BasisError::InvalidDenominator
        | BasisError::InvalidKnotCount
        | BasisError::InvalidKnot
        | BasisError::NonCanonicalPadding
        | BasisError::UniformSpacingMismatch
        | BasisError::UniformSpacingRequired
        | BasisError::InvalidEdgePolicy
        | BasisError::ArithmeticBound => ResolutionRefusal::BasisMalformed,
    }
}

/// Derive the payout-set index selected by one sealed window under one terms.
///
/// `derive_payout : (ResolutionTerms, PayoutSet bytes, WindowResult) ->
/// Result<PayoutIndex, R>`. Every input is immutable and digest-bound:
/// `payouts` must be the frozen terms' own payout vectors — the adapter reads
/// them from the digest-bound `TermsAccount`, and its payout-set-binding gate
/// step has already proved the kernel's set equal to them.
///
/// Degree 0: one-hot resolution succeeds only when the entire conservative
/// interval lies inside one cell. `payout_map` is not injective —
/// `MAX_OUTCOMES` is 16 while `MAX_PAYOUTS` is 8, so several cells may
/// legitimately pay the same vector — and must never be inverted to recover a
/// cell.
///
/// Degrees 1 through 3: the derived, validated weight vector must be a member of the
/// frozen preset set (see the module docs and
/// [`ResolutionRefusal::DerivedVectorUnrepresentable`]); the returned index
/// is the member's, so the kernel's resolve-by-index installs exactly the
/// derived vector.
///
/// The function is total, allocation free, and performs at most 15
/// comparisons plus one bounded statistic evaluation, one bounded basis
/// evaluation per admitted endpoint, and at most `MAX_PAYOUTS` vector
/// comparisons.
///
/// `NotSealed`, `NotMature`, and `IncompleteDomain` cannot appear here: a
/// [`WindowResult`] cannot exist in those states.
pub fn derive_payout(
    terms: &ResolutionTerms,
    payouts: &[PayoutVectorBytes; MAX_PAYOUTS],
    window: &WindowResult,
) -> Result<u8, ResolutionRefusal> {
    terms.validate()?;
    window
        .check_domain(&terms.window)
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;
    if terms.basis_degree == 0 {
        let (first, last) = terms.conservative_cells(window)?;
        if first != last {
            // AMBIG-REFUSE-01. AMBIG-MIDPOINT, AMBIG-CONSERVATIVE-LOW, and
            // AMBIG-CONSERVATIVE-HIGH are never registered: each converts
            // uncertainty into a definite claim with no evidence.
            return Err(ResolutionRefusal::AmbiguousInterval);
        }
        let payout = terms.payout_map[usize::from(first)];
        if payout >= terms.payout_count {
            return Err(ResolutionRefusal::PayoutIndexOutOfRange);
        }
        return Ok(payout);
    }
    let vector = terms.derived_vector(window)?;
    let mut index = 0usize;
    while index < usize::from(terms.payout_count) {
        if payouts[index].denominator == vector.denominator
            && payouts[index].weights == vector.weights
        {
            return Ok(index as u8);
        }
        index += 1;
    }
    Err(ResolutionRefusal::DerivedVectorUnrepresentable)
}

/// Derive the validated weight vector selected by one sealed window under one
/// derived-basis terms.
///
/// The §5.1 sibling seam, with [`derive_payout`]'s discipline: pure, total,
/// allocation free, checked, reads no clock, signer, or account. The result
/// is member-shaped by construction — weights in `[0, D]`, zero beyond the
/// active prefix, summing to exactly `D` — and is validated before it is
/// returned, so the kernel's own `PayoutVector` validation would accept it
/// verbatim. A degree-0 market has no derived vector and refuses
/// [`ResolutionRefusal::WrongResolutionMode`]: one resolution seam per mode,
/// never both.
pub fn derive_payout_vector(
    terms: &ResolutionTerms,
    window: &WindowResult,
) -> Result<PayoutVectorBytes, ResolutionRefusal> {
    terms.validate()?;
    if terms.basis_degree == 0 {
        return Err(ResolutionRefusal::WrongResolutionMode);
    }
    window
        .check_domain(&terms.window)
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;
    terms.derived_vector(window)
}

#[cfg(test)]
mod native_bspline_tests {
    use super::*;
    use clutch_accumulator::{Observation, WindowAccumulator};

    fn domain() -> WindowDomain {
        let feed = FeedIdentity::new([1; 32], [2; 32], 1, 1).unwrap();
        let grid = Grid::new(7, 1, 1).unwrap();
        WindowDomain::new(feed, grid, 0, 1, 1, 0, CoveragePolicy::COMPLETE_REQUIRED).unwrap()
    }

    fn window(domain: WindowDomain, low: u128, high: u128) -> WindowResult {
        let mut accumulator = WindowAccumulator::open(domain);
        accumulator
            .observe(Observation::accepted(0, low, high))
            .unwrap();
        accumulator.seal().unwrap();
        accumulator.result().unwrap()
    }

    fn derived_terms(degree: u8) -> ResolutionTerms {
        let mut knots = [0_u128; MAX_KNOTS];
        knots[..3].copy_from_slice(&[0, 4, 8]);
        ResolutionTerms {
            market: Hash32::ZERO,
            window: domain(),
            statistic: STAT_TERMINAL_01,
            cell_count: degree + 2,
            basis_degree: degree,
            edge_policy: EDGE_CLAMP_01,
            knot_count: 3,
            uniform_log2_spacing: 2,
            knots,
            denominator: 64,
            payout_map: [PAYOUT_MAP_UNUSED; MAX_CELLS],
            ambiguity_policy: AMBIG_REFUSE_01,
            failure_policy: FAIL_UNIFORM_REFUND_01,
            generation_policy: GEN_EXACT_01,
            payout_count: 1,
        }
    }

    #[test]
    fn categorical_point_evidence_remains_the_degree_zero_basis() {
        let mut terms = derived_terms(1);
        terms.cell_count = 2;
        terms.basis_degree = 0;
        terms.knot_count = 1;
        terms.uniform_log2_spacing = clutch_bspline::UNIFORM_SPACING_NONE;
        terms.knots = [0; MAX_KNOTS];
        terms.knots[0] = 4;
        terms.denominator = 1;
        terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_CELLS];
        terms.payout_map[0] = 0;
        terms.payout_map[1] = 1;
        terms.payout_count = 2;
        let payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];

        let below = window(terms.window, 3, 3);
        assert_eq!(derive_payout(&terms, &payouts, &below), Ok(0));
        let at_boundary = window(terms.window, 4, 4);
        assert_eq!(derive_payout(&terms, &payouts, &at_boundary), Ok(1));
    }

    #[test]
    fn quadratic_and_cubic_point_evidence_reach_native_reference_vectors() {
        let quadratic = derived_terms(2);
        let point = window(quadratic.window, 2, 2);
        let vector = derive_payout_vector(&quadratic, &point).unwrap();
        assert_eq!(vector.denominator, 64);
        assert_eq!(&vector.weights[..4], &[16, 40, 8, 0]);
        assert_eq!(vector.weights.iter().copied().sum::<u64>(), 64);

        let cubic = derived_terms(3);
        let point = window(cubic.window, 2, 2);
        let vector = derive_payout_vector(&cubic, &point).unwrap();
        assert_eq!(vector.denominator, 64);
        assert_eq!(&vector.weights[..5], &[8, 38, 16, 2, 0]);
        assert_eq!(vector.weights.iter().copied().sum::<u64>(), 64);
    }

    #[test]
    fn smooth_nonpoint_evidence_refuses_before_quantized_endpoint_comparison() {
        let mut quadratic = derived_terms(2);
        quadratic.denominator = 1;
        let uncertain = window(quadratic.window, 1, 2);
        assert_eq!(
            derive_payout_vector(&quadratic, &uncertain),
            Err(ResolutionRefusal::NonPointEvidence)
        );

        let cubic = derived_terms(3);
        let uncertain = window(cubic.window, 2, 3);
        assert_eq!(
            derive_payout_vector(&cubic, &uncertain),
            Err(ResolutionRefusal::NonPointEvidence)
        );
    }

    #[test]
    fn edge_handling_precedes_the_smooth_point_evidence_gate() {
        let quadratic = derived_terms(2);
        let clamped = window(quadratic.window, 9, 12);
        let vector = derive_payout_vector(&quadratic, &clamped).unwrap();
        assert_eq!(&vector.weights[..4], &[0, 0, 0, 64]);

        let mut refusing = quadratic;
        refusing.edge_policy = EDGE_REFUSE_02;
        assert_eq!(
            derive_payout_vector(&refusing, &clamped),
            Err(ResolutionRefusal::ValueOutOfRange)
        );
    }

    #[test]
    fn smooth_twap_and_nonuniform_breakpoints_remain_explicit_refusals() {
        let mut twap = derived_terms(3);
        twap.statistic = STAT_TWAP_04;
        assert_eq!(
            twap.validate(),
            Err(ResolutionRefusal::StatisticUnsupported)
        );

        let mut nonuniform = derived_terms(2);
        nonuniform.uniform_log2_spacing = clutch_bspline::UNIFORM_SPACING_NONE;
        assert_eq!(
            nonuniform.validate(),
            Err(ResolutionRefusal::BasisMalformed)
        );
    }
}
