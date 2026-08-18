//! Terms-to-payout derivation over typed window evidence.
//!
//! Status: MODEL. This module implements §2 of
//! `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md` as a pure, allocation
//! free, checked function over already-decoded values. It reads no clock, no
//! signer, and no account; decoding hostile bytes, account authentication, and
//! aliasing all happen in the adapter before anything here is reachable.
//!
//! Evidence-gated is **not** authenticated. A [`WindowResult`] is honest
//! evidence about a fold, never evidence about a source: the feed identity
//! bytes it is bound to are opaque 32-byte values that no crate in this
//! repository can relate to a real adapter, deployment, subject, or quote.
//!
//! # What the frozen layout does and does not supply
//!
//! Plan §2.1 proposed a 432-byte `ResolutionTerms` artifact digested straight
//! into `MarketAccount.terms`. The landed
//! [`clutch_solana_layout::TermsAccount`] took that digest slot with a
//! different body, so §2.1's artifact cannot also be `MarketAccount.terms`
//! without a hash collision. The bound inputs to a derivation are therefore
//! exactly the fields of the frozen `TermsAccount`, and everything else must
//! be **canonical** (derivable from those fields) or **refused**. It must not
//! be caller-supplied: an unbound boundary table would let a caller choose the
//! payout, which is the "signer and a hope" the plan exists to remove.
//!
//! [`ResolutionTerms::from_market_terms`] is that canonical derivation. It
//! pins, and documents as obligations on a later `TermsAccount` revision:
//!
//! - the statistic, to `STAT-TERMINAL-01`;
//! - the ambiguity policy, to `AMBIG-REFUSE-01`;
//! - the partition, to the ordinal cell family `C_i = [i, i+1)` closed at the
//!   top, with `n = outcome_count` cells and the identity payout map;
//! - the source and evaluator versions, to `1`;
//! - the source-adapter and feed-spec identities, both to `TermsAccount.feed`;
//! - the repair generation under `GEN-EXACT-01`, to `0`.
//!
//! Everything else that could change the answer is refused rather than
//! defaulted. [`ResolutionTerms`] itself keeps the general shape of plan §2.1
//! so the algorithm can be reviewed and adversarially tested at full
//! generality, and so a future `TermsAccount` that carries a boundary table,
//! a statistic id, and an ambiguity policy id can bind it without rewriting
//! the derivation.

use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, StatisticError, WindowDomain, WindowError, WindowResult,
    MAX_VALUE,
};
use clutch_solana_layout::{Hash32, MarketAccount, TermsAccount, MAX_OUTCOMES};

/// Registered statistic: the terminal accepted interval of the sealed window.
pub const STAT_TERMINAL_01: u16 = 1;
/// Registered statistic: the conservative sampled minimum.
pub const STAT_SAMPLED_MIN_02: u16 = 2;
/// Registered statistic: the conservative sampled maximum.
pub const STAT_SAMPLED_MAX_03: u16 = 3;
/// Registered statistic: the exact time-weighted average ratio interval.
pub const STAT_TWAP_04: u16 = 4;
/// Registered statistic: terminal relative to TWAP. **Inadmissible in V1.**
///
/// Plan §2.3: its comparison needs `low_numerator * scale` against
/// `boundary * low_denominator`, where both sides are already near
/// `8.64 x 10^34`. In `u128` that leaves a headroom factor of about
/// `3.9 x 10^3`, so admitting it requires a checked 256-bit comparison, a
/// narrowed `MAX_VALUE`, or a frozen scale small enough to prove the bound.
/// None of those exists, so this identifier is registered and refused.
pub const STAT_RELATIVE_TERMINAL_TWAP_05: u16 = 5;

/// Registered ambiguity policy `AMBIG-REFUSE-01`: the V1 default.
pub const AMBIG_REFUSE_01: u8 = 1;
/// Registered ambiguity policy `AMBIG-COMPATIBLE-SET-02`: not implemented.
///
/// The kernel has no representation for a compatible cell set, and the policy
/// inherits §P1-A of `docs/implementation/ADVERSARIAL_REVIEW_V0.md`.
pub const AMBIG_COMPATIBLE_SET_02: u8 = 2;

/// Registered failure policy `FAIL-UNIFORM-REFUND-01`: recorded, not executed.
pub const FAIL_UNIFORM_REFUND_01: u8 = 1;
/// Registered failure policy `FAIL-EXTENDED-WINDOW-02`: recorded, not executed.
pub const FAIL_EXTENDED_WINDOW_02: u8 = 2;

/// Registered generation policy `GEN-EXACT-01`: the V1 default.
pub const GEN_EXACT_01: u8 = 1;
/// Registered generation policy `GEN-FINAL-AT-MATURITY-02`: not implemented.
///
/// It is blocked on a sealed feed-epoch object that can prove "no further
/// repair is possible after maturity". That object does not exist.
pub const GEN_FINAL_AT_MATURITY_02: u8 = 2;

/// Source-adapter version pinned by V1, because the frozen terms carry none.
pub const V1_SOURCE_VERSION: u32 = 1;
/// Statistic-evaluator version pinned by V1, because the frozen terms carry none.
pub const V1_EVALUATOR_VERSION: u32 = 1;
/// Repair generation pinned by `GEN-EXACT-01`: the original, unrepaired fold.
pub const V1_EXACT_GENERATION: u64 = 0;

/// Largest number of ordered cells in a settlement partition.
pub const MAX_CELLS: usize = MAX_OUTCOMES;
/// Largest number of interior boundaries, one fewer than [`MAX_CELLS`].
pub const MAX_BOUNDARIES: usize = MAX_CELLS - 1;
/// Payout-map entry meaning "this cell index is not live".
pub const PAYOUT_MAP_UNUSED: u8 = 0xFF;

/// Every path out of [`derive_payout`] other than a selected payout index.
///
/// The classes are distinguishable on purpose: "the window was wrong" and
/// "the interval was ambiguous" have different operational responses. The
/// numbering is the `R-01..R-11` registry of plan §2.5, exposed by
/// [`ResolutionRefusal::class`] so that a taxonomy code never depends on the
/// enum's own discriminants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionRefusal {
    /// R-01: the terms artifact is not the one this market's digest binds.
    TermsDigestMismatch,
    /// R-02: unregistered statistic, policy, or otherwise malformed terms.
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
    StatisticUnsupported,
    /// R-06: the conservative interval straddles cells under `AMBIG-REFUSE-01`.
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
}

impl ResolutionRefusal {
    /// The `R-nn` class number of plan §2.5.
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
        }
    }
}

/// The immutable inputs a payout derivation is allowed to read.
///
/// Fields are public because this is a pure model value with no invariant of
/// its own: [`ResolutionTerms::validate`] is total and [`derive_payout`] runs
/// it before reading anything, so an ill-formed value is a refusal rather than
/// an unsound derivation. The adapter never accepts one from a caller; it
/// builds one with [`ResolutionTerms::from_market_terms`] from bytes the
/// market's frozen digest already commits to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionTerms {
    /// Market identity these terms were frozen for.
    pub market: Hash32,
    /// The exact expected window domain, §1.5 of the plan.
    pub window: WindowDomain,
    /// Registered statistic identifier.
    pub statistic: u16,
    /// Number of ordered cells `n`, in `2..=MAX_CELLS`.
    pub cell_count: u8,
    /// Interior boundaries `b_0 .. b_{n-2}`; entries at `>= n-1` are zero.
    pub boundaries: [u128; MAX_BOUNDARIES],
    /// Cell to payout-set index; entries at `>= n` are [`PAYOUT_MAP_UNUSED`].
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
        // A bounded-gap policy needs a gap bound, and the frozen terms carry
        // no field for one. Rather than default the bound, the policy refuses.
        let coverage = CoveragePolicy::from_registry(coverage_id, 0)
            .map_err(|_| ResolutionRefusal::TermsMalformed)?;
        let feed = FeedIdentity::new(
            terms.feed.bytes(),
            terms.feed.bytes(),
            V1_SOURCE_VERSION,
            V1_EVALUATOR_VERSION,
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
            V1_EXACT_GENERATION,
            coverage,
        )
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;

        // The ordinal cell family: C_i = [i, i+1) for i < n-1 and
        // C_{n-1} = [n-1, MAX_VALUE]. It is exhaustive, disjoint, ordered, has
        // no empty cell, and is fully determined by outcome_count, so it needs
        // no field the frozen terms lack.
        let cell_count = terms.outcome_count;
        let mut boundaries = [0u128; MAX_BOUNDARIES];
        let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_CELLS];
        let cells = usize::from(cell_count);
        if !(2..=MAX_CELLS).contains(&cells) {
            return Err(ResolutionRefusal::PartitionMalformed);
        }
        let mut index = 0usize;
        while index < cells {
            if index + 1 < cells {
                boundaries[index] = (index as u128) + 1;
            }
            payout_map[index] =
                u8::try_from(index).map_err(|_| ResolutionRefusal::PartitionMalformed)?;
            index += 1;
        }
        let value = Self {
            market: market.market,
            window,
            statistic: STAT_TERMINAL_01,
            cell_count,
            boundaries,
            payout_map,
            ambiguity_policy: AMBIG_REFUSE_01,
            failure_policy: u8::try_from(terms.failure_policy_id)
                .map_err(|_| ResolutionRefusal::TermsMalformed)?,
            generation_policy: GEN_EXACT_01,
            payout_count: terms.payout_count,
        };
        value.validate()?;
        Ok(value)
    }

    /// Check the registry membership, the partition, and the payout map.
    ///
    /// Total, allocation free, and checked. Plan §2.2: strict increase gives
    /// disjointness and ordering, `0 < b_0` and the closed top cell give
    /// exhaustiveness with every cell non-empty. An empty cell is refused
    /// because it would mint a liability that can never pay.
    pub fn validate(&self) -> Result<(), ResolutionRefusal> {
        match self.statistic {
            STAT_TERMINAL_01 | STAT_SAMPLED_MIN_02 | STAT_SAMPLED_MAX_03 | STAT_TWAP_04 => {}
            STAT_RELATIVE_TERMINAL_TWAP_05 => return Err(ResolutionRefusal::StatisticUnsupported),
            _ => return Err(ResolutionRefusal::TermsMalformed),
        }
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
        if self.window.generation() != V1_EXACT_GENERATION {
            return Err(ResolutionRefusal::TermsMalformed);
        }
        let cells = usize::from(self.cell_count);
        if !(2..=MAX_CELLS).contains(&cells) {
            return Err(ResolutionRefusal::PartitionMalformed);
        }
        let mut previous = 0u128;
        let mut index = 0usize;
        while index < MAX_BOUNDARIES {
            let boundary = self.boundaries[index];
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
        if self.payout_count == 0 {
            return Err(ResolutionRefusal::TermsMalformed);
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

    /// The unique cell index containing `value`; at most 15 comparisons.
    fn cell_of(&self, value: u128) -> u8 {
        let cells = usize::from(self.cell_count);
        let mut index = 0usize;
        while index + 1 < cells {
            if value < self.boundaries[index] {
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
            let scaled = self.boundaries[index]
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
}

fn map_statistic(error: StatisticError) -> ResolutionRefusal {
    match error {
        StatisticError::NoAcceptedCoverage => ResolutionRefusal::NoAcceptedCoverage,
        StatisticError::UnsupportedPredicate => ResolutionRefusal::StatisticUnsupported,
        StatisticError::AmbiguousDenominator => ResolutionRefusal::AmbiguousDenominator,
        StatisticError::ArithmeticOverflow => ResolutionRefusal::ArithmeticOverflow,
    }
}

/// Derive the payout-set index selected by one sealed window under one terms.
///
/// `derive_payout : (ResolutionTerms, WindowResult) -> Result<PayoutIndex, R>`.
/// Both inputs are immutable. The function is total, allocation free, and
/// performs at most 15 comparisons plus one bounded statistic evaluation.
///
/// One-hot resolution succeeds only when the entire conservative interval lies
/// inside one cell. `payout_map` is not injective — `MAX_OUTCOMES` is 16 while
/// `MAX_PAYOUTS` is 8, so several cells may legitimately pay the same vector —
/// and must never be inverted to recover a cell.
///
/// `NotSealed`, `NotMature`, and `IncompleteDomain` cannot appear here: a
/// [`WindowResult`] cannot exist in those states.
pub fn derive_payout(
    terms: &ResolutionTerms,
    window: &WindowResult,
) -> Result<u8, ResolutionRefusal> {
    terms.validate()?;
    window
        .check_domain(&terms.window)
        .map_err(ResolutionRefusal::WindowDomainMismatch)?;
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
    Ok(payout)
}
