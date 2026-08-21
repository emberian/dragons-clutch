//! `BatchRelationV1` — the coupled host-model batch relation.
//!
//! This module is IMPLEMENTED host-model code following the PROPOSED design in
//! `docs/implementation/BATCH_RELATION_V1_DESIGN.md`.  It is **not** verified,
//! **not** a Solana (SVM) relation, and **not** an authorization for any onchain
//! or financial action.  Nothing here is a proof: the pairing-feasibility
//! theorem and the constructor invariant are design arguments backed by bounded
//! exhaustive oracles in this crate's test module, not by a proof assistant.
//!
//! The relation answers one question, exactly and without search:
//!
//! ```text
//! BatchRelationV1(frozen domain, frozen book, candidate witness)
//!     = Valid(summary) | Refusal
//! ```
//!
//! An accepted candidate is the **best valid submitted candidate** of its
//! proposal window under the frozen score.  It is never described as optimal:
//! no optimality certificate exists, and the constructors in this module search
//! only the bounded coordinates they are told to search.
//!
//! # What the coupled relation repairs
//!
//! The scalar relation in the crate root (retained unchanged as a regression
//! lab) clears one grid tick over side totals with owner and outcome erased.  A
//! buy bound to outcome 0 and a sell bound to outcome 1 therefore produced
//! matched volume that no executable transfer could ever realize.  Here, every
//! fill is bound to `(owner, outcome, side)` and two structural mechanisms make
//! that impossible:
//!
//! * **per-outcome conservation with a single global virtual split/merge pair**
//!   (`V4`): fills must carry the *same* net imbalance `c` on every active
//!   outcome, so a cross-outcome "match" has no solution at all; and
//! * **the pairing-feasibility gate** (`V5`): an exact integer inequality that
//!   is necessary and sufficient for a complete executable pairing with
//!   distinct bound owners.
//!
//! # Stages
//!
//! ```text
//! V0 domain + admission + normalization      (orders -> bound legs)
//! V1 simplex validation                      (prices exact on the scaled simplex)
//! V2 eligibility classification              (strict / marginal / ineligible)
//! V3 canonical fill derivation + exact equality
//! V4 virtual complete-set conservation       (per-outcome closure, sigma/mu)
//! V5 pairing feasibility gate                (complete executable pairing)
//! V6 portfolio valuation + consideration     (one named rounding boundary)
//! V7 fee relation                            (payer debited; carry)
//! V8 per-asset conservation closure          (collateral + every Egg)
//! V9 score recomputation + total tie order + candidate digest
//! ```
//!
//! ## Documented deviations from the design document
//!
//! These are deliberate, and each is exercised by a test:
//!
//! 1. **Refusal precedence between V3 and V4.**  The witness-level conservation
//!    identity (`V4`) is checked *before* the canonical-allocation exact
//!    equality (`V3`), so a cross-outcome forgery refuses with the diagnostic
//!    [`ErrorV1::OutcomeConservationMismatch`] instead of the generic
//!    [`ErrorV1::CandidateMismatch`].  The accepted set is identical either way;
//!    only which refusal is reported changes.
//! 2. **Constructor slack is never floored at 1.**  §8.4 of the design floors
//!    the slack term at 1.  Doing so can emit a slice that strands the residue
//!    (the `A/C`–`B/C` book), so this implementation refuses with
//!    [`ErrorV1::ConstructorStalled`] instead.  Under the two-largest greedy the
//!    slack is provably at least 1 whenever the feasibility inequality holds,
//!    and the exhaustive oracle finds no stall in the searched domain.
//! 3. **Rounding direction.**  §9.2 names a floor.  A floor applied to *both*
//!    directions cannot conserve, so debits round up and credits round down;
//!    both remainders are credited to one named, non-negative rounding pot.
//! 4. **Honored minimum-fill under AON variant 2b** means honored *at full
//!    size*, the same as an honored all-or-none order.  Honoring exactly the
//!    minimum is a distinct sub-variant and is not implemented.
//! 5. **Score component 2** ("subtract self-overlap volume") is implemented as a
//!    dispersion-weighted subtraction inside component 1, because the design's
//!    raw-volume subtraction mixes units.  Under `N-a`/`N-b` the term is
//!    identically zero.
//! 6. **`RoundingBoundaryV1::ReceiptFloor`** rounds once per *filled leg*, the
//!    finest granularity that exists at verification time.  A slice-granular
//!    settlement layer owns the per-receipt event and must re-derive it.

use core::cmp::Ordering;

use crate::{seeded_rank, DustPolicy, PartialPolicy, Side, MAX_ORDERS};

/// Frozen relation version tag.  A domain naming any other version is refused.
pub const RELATION_VERSION_V1: u32 = 1;
/// Kernel-aligned outcome bound.
pub const MAX_OUTCOMES: usize = 16;
/// PROPOSED capacity parameter, not economics.
pub const MAX_PORTFOLIO_ORDERS: usize = 8;
/// Every admitted order lowers to at most `MAX_OUTCOMES` bound legs.
pub const MAX_LEGS: usize = MAX_ORDERS + MAX_PORTFOLIO_ORDERS * MAX_OUTCOMES;
/// Safe (not tight) bound on the canonical slice decomposition.
pub const MAX_SLICES: usize = 2 * MAX_LEGS + 2 * MAX_OUTCOMES;
/// At most one distinct owner per admitted order can appear in a frozen book.
pub const MAX_OWNER_SLOTS: usize = MAX_ORDERS;
/// PROPOSED price scale.  It is a domain parameter, never a canonized constant.
pub const PRICE_SCALE: u64 = 10_000;
/// Fee rates are exact basis points over this denominator.
pub const FEE_BPS_DENOMINATOR: u64 = 10_000;
/// Largest price scale whose simplex sum cannot overflow a `u64` accumulator.
pub const MAX_PRICE_SCALE: u64 = u64::MAX / (MAX_OUTCOMES as u64);
/// Largest price scale at which the composite fee base's common denominator
/// `kappa_den * S^2 * kappa'_den` is representable in `u128`.
///
/// A round bound, deliberately far inside the exact one
/// (`floor(sqrt(u128::MAX / FEE_BPS_DENOMINATOR^2))`, about `1.844e15`):
/// `10^8 * (10^15)^2 = 10^38 < u128::MAX`.  It bounds the **denominator**
/// only.  The numerator stays checked arithmetic, not an audited envelope —
/// freezing the five (now six) width bounds is `FEE_GEOMETRY.md` §3's still
/// unpaid debt, and this constant does not discharge it.
pub const MAX_COMPOSITE_PRICE_SCALE: u64 = 1_000_000_000_000_000;

// Compile-time accumulator width bounds (§5 of the design).
const _: () = assert!(PRICE_SCALE <= MAX_PRICE_SCALE);
const _: () = assert!((MAX_OUTCOMES as u64).checked_mul(PRICE_SCALE).is_some());
const _: () = assert!(MAX_LEGS == 192);
const _: () = assert!(MAX_SLICES == 416);
const _: () = assert!(MAX_ORDERS <= 64, "the honored-AON mask is a u64 bitmask");
const _: () = assert!(MAX_COMPOSITE_PRICE_SCALE <= MAX_PRICE_SCALE);
const _: () = assert!(
    (FEE_BPS_DENOMINATOR as u128)
        .pow(2)
        .checked_mul((MAX_COMPOSITE_PRICE_SCALE as u128).pow(2))
        .is_some(),
    "the composite common denominator must fit u128 at every admitted price scale"
);
// Both composite rates ride one `u32` wire slot in both policy codecs
// (`relation_v1_stream::put_policy` and `clutch-batch-policy-identity`), the
// floor rate shifted into the high half.  That packing is only lossless while
// each rate fits 16 bits.
const _: () = assert!(FEE_BPS_DENOMINATOR < 0x1_0000);

/// Canonical fill allocation family (§7.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationPolicyV1 {
    /// **A** — strict orders fill fully; the marginal set absorbs the residual
    /// pro-rata by largest remainder and seeded canonical rank.
    PricePriorityMarginalProRata,
    /// **B** — full pro-rata over every eligible order, as the scalar lab does.
    FullProRata,
}

/// Self-cross normalization family (§4.4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelfCrossPolicyV1 {
    /// **N-a** — refuse any book where one owner stands on both sides of one
    /// outcome.
    RefuseOverlap,
    /// **N-b** — cancel `min(buy, sell)` of each same-owner overlap at
    /// admission, price-independently, and refund the cancelled reservation.
    NetAtAdmission,
    /// **N-c** — allow the overlap and let the V5 feasibility gate refuse it.
    AllowGateAtPairing,
}

/// All-or-none / minimum-fill family (§10).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AonPolicyV1 {
    /// **2a** — refuse all-or-none and `minimum_fill > 1` at admission.
    RefuseAdmission,
    /// **2b** — the untrusted solver submits the honored subset as a mask; the
    /// verifier only ever *checks* a mask and never computes one.
    WitnessedHonoredMask,
    /// **2c** — count all-or-none at full size and refuse when the canonical
    /// allocator cannot make it whole (the scalar lab's landed behavior).
    FullSizeCounting,
}

/// The one named price-unit to collateral-atom boundary (§9.2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingBoundaryV1 {
    /// **R-a** — exact or refuse; no remainder may exist.
    None,
    /// **R-b** — one conversion per owner per epoch.
    TerminalOwnerFloor,
    /// **R-c** — one conversion per receipt; here, per filled leg.
    ReceiptFloor,
}

/// Residual-pair settlement family (§13).  Recorded by this crate and consumed
/// by the settlement layer; this crate only freezes the slice universe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidualSettlementV1 {
    /// **1a** — full-pair-only receipts over the frozen slices.
    FullPairOnly,
    /// **1b-canonical** — cumulative remaining quantity over the frozen slices.
    CumulativePairCanonical,
    /// **1b-free** — cumulative remaining over any executable pair.  Carries a
    /// documented strand hazard that this crate does not discharge.
    CumulativePairFree,
    /// **1c** — unique match-slice receipts frozen at clear time.
    UniqueSliceReceipts,
}

/// Kernel `transfer_internal` phase gate (§14.2).  Recorded, not enforced here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferPhaseV1 {
    /// **T-a** — Active phase only.
    ActiveOnly,
    /// **T-b** — Active or Resolved.
    ActiveOrResolved,
}

/// Portfolio lot rationing family (§7.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioLotPolicyV1 {
    /// **P-a** — portfolios fill whole, in whole lots, only when strict.
    StrictWholeOrder,
    /// **P-b** — marginal portfolios receive a pro-rata lot quotient.  Research
    /// gated; selecting it refuses with [`ErrorV1::PolicyVariantUnimplemented`].
    MarginalProRataLots,
}

/// Who carries the pairing proof (§8.3 / §8.5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingWitnessPolicyV1 {
    /// Verification accepts on the feasibility inequality alone and the
    /// canonical constructor runs once at candidate finalization.
    RecomputedConstructor,
    /// The candidate carries an explicit slice list which verification checks
    /// slice by slice.  Both variants refuse the same books.
    ExplicitSlices,
}

/// Score family (§11).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScorePolicyV1 {
    /// Lexicographic: dispersion-weighted direct volume net of self-overlap,
    /// then limit surplus, then distinct owners, then least churn, then the
    /// candidate digest ascending.
    LexicographicDispersionV1,
}

/// Fee base family (§9.3).  The economics remain gated; only the flat-notional
/// control and the zero-fee control are implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeBaseV1 {
    /// No fee is charged anywhere.
    None,
    /// Flat basis points on the buy-side consideration, debited from the payer.
    FlatNotional {
        /// Exact basis points over [`FEE_BPS_DENOMINATOR`].
        bps: u32,
    },
    /// The composite fee-base SHAPE selected 2026-08-20 (ADOPTED item 9 on
    /// `docs/decisions/REPORT_fee-base-selection_2026-08-20.md`):
    /// `kappa*G(a,p) + kappa'*R(a)` — atomic simplex dispersion with a
    /// price-free quotient-norm floor.
    ///
    /// The arithmetic is **implemented** ([`composite_fee_quote`]): one exact
    /// rational over the common denominator `kappa_den * S^2 * kappa'_den`,
    /// one carry, one terminal ceiling, quoted owner-level over the filled
    /// payoff vector.  Nonzero rates therefore verify rather than refuse.
    ///
    /// **The production rates remain UNDECIDED and are ember's alone.**  Every
    /// frozen production const in this tree still pins `(0, 0)`; the only
    /// nonzero rates that exist are the `TEST_COMPOSITE_*` laboratory pairs
    /// below, which are never frozen and never digested as production.  A
    /// production rate is a new frozen const with a new digest behind its own
    /// ember decision, plus `FEE_GEOMETRY.md` §3's still-owed width bounds.
    CompositeDispersionFloor {
        /// Dispersion rate `kappa` numerator over [`FEE_BPS_DENOMINATOR`].
        dispersion_bps: u32,
        /// Quotient-norm floor rate `kappa'` numerator over
        /// [`FEE_BPS_DENOMINATOR`] per unit of model-free range.
        floor_range_bps: u32,
    },
}

/// The zero-rate composite shape: the regression anchor.  Every byte and every
/// verdict under this pair is identical to [`FeeBaseV1::None`]'s economics.
pub const TEST_COMPOSITE_ZERO: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: 0,
    floor_range_bps: 0,
};
/// The laboratory comparison calibration of the selection report §3.1 —
/// `kappa = 40 bp`, `kappa' = 10 bp of range`.  **A comparison arm, never a
/// proposed production rate** (report §1: "every rate in this report is a lab
/// comparison calibration, not a proposal").
pub const TEST_COMPOSITE_LAB: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: 40,
    floor_range_bps: 10,
};
/// The smallest nonzero rate pair: one basis point on each term.  Exercises
/// the sub-atom regime the carry exists for.
pub const TEST_COMPOSITE_SMALL: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: 1,
    floor_range_bps: 1,
};
/// The admissible-rate boundary: both rates at [`FEE_BPS_DENOMINATOR`].
pub const TEST_COMPOSITE_BOUNDARY: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: FEE_BPS_DENOMINATOR as u32,
    floor_range_bps: FEE_BPS_DENOMINATOR as u32,
};
/// Dispersion only — the pure-`G` arm, which is feeless on the zero-price
/// channel (`FEE_GEOMETRY.md` §5).
pub const TEST_COMPOSITE_DISPERSION_ONLY: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: 40,
    floor_range_bps: 0,
};
/// Floor only — the pure-`R` arm, which charges the zero-price channel.
pub const TEST_COMPOSITE_FLOOR_ONLY: FeeBaseV1 = FeeBaseV1::CompositeDispersionFloor {
    dispersion_bps: 0,
    floor_range_bps: 10,
};

/// One exact composite fee quote, field for field the laboratory's `FeeQuote`
/// (`research/economics-admission/model.py`).
///
/// `floor_atoms` and `terminal_ceil_atoms` are **collateral atoms**, not price
/// units: `G_num` already carries the `S^2` the denominator divides out, so the
/// quotient is the charge in the same units a complete set is counted in
/// (`FEE_GEOMETRY.md` §4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeQuoteV1 {
    /// `kappa_num*G_num*kappa'_den + kappa'_num*R*kappa_den*S^2`, before the
    /// prior carry is folded in.  Both rates are folded into the base fraction
    /// because the composite has no single rate to factor out.
    pub base_numerator: u128,
    /// `kappa_den * S^2 * kappa'_den`.
    pub base_denominator: u128,
    /// `base_numerator + prior_carry`.
    pub exact_numerator: u128,
    /// Equal to [`Self::base_denominator`]; carried so the quote is the whole
    /// rational and a reader never has to know they coincide.
    pub exact_denominator: u128,
    /// `floor(exact_numerator / exact_denominator)` — what a non-terminal fill
    /// pays now.
    pub floor_atoms: u128,
    /// `ceil(exact_numerator / exact_denominator)` — the terminal-ceil close.
    pub terminal_ceil_atoms: u128,
    /// `exact_numerator mod exact_denominator`, the remainder that persists in
    /// the owner's carry so fragmentation cannot round a fee to nothing.
    pub carry: u128,
}

/// The composite fee base in exact integer arithmetic.
///
/// `kappa*G(a,p) + kappa'*R(a)` over one common denominator with one carry, the
/// single-`FeeBaseV1`-variant runtime shape the selection report costed (§3.5:
/// "two parallel pipelines would pay up to one extra terminal atom per
/// intent").  With
///
/// ```text
/// G_num(a,p) = sum_{i<j} p_i * p_j * abs(a_i - a_j)
/// R(a)       = max_i a_i - min_i a_i
/// ```
///
/// the quote is
///
/// ```text
/// numerator   = kappa_num*G_num*kappa'_den + kappa'_num*R*kappa_den*S^2 + carry_in
/// denominator = kappa_den * S^2 * kappa'_den
/// ```
///
/// The common-denominator form is kept unreduced on purpose: it is the form the
/// laboratory quotes, so the carry — which is only meaningful against its own
/// denominator — is directly comparable across the two implementations.
///
/// `payoffs` is the **owner-level filled payoff vector**, not one order's: `G`
/// is subadditive, so quoting per order would overcharge a netted portfolio and
/// hand fragmentation a discount.
///
/// Every step is checked; nothing wraps.  Refusals:
///
/// - [`ErrorV1::InvalidOutcome`] — width outside `2 ..= MAX_OUTCOMES`;
/// - [`ErrorV1::InvalidPriceScale`] — zero, or past
///   [`MAX_COMPOSITE_PRICE_SCALE`];
/// - [`ErrorV1::PriceOutOfRange`] / [`ErrorV1::SimplexSumMismatch`] — the
///   prices are off the exact simplex;
/// - [`ErrorV1::FeeMismatch`] — a prior carry that is not canonical
///   (`>= denominator`), which no honest carry ledger can produce;
/// - [`ErrorV1::ArithmeticOverflow`] — the exact rational does not fit `u128`.
pub fn composite_fee_quote(
    payoffs: &[u64; MAX_OUTCOMES],
    prices: &[u64; MAX_OUTCOMES],
    outcomes: usize,
    price_scale: u64,
    dispersion_bps: u32,
    floor_range_bps: u32,
    prior_carry: u128,
) -> Result<FeeQuoteV1, ErrorV1> {
    if outcomes < 2 || outcomes > MAX_OUTCOMES {
        return Err(ErrorV1::InvalidOutcome);
    }
    if price_scale == 0 || price_scale > MAX_COMPOSITE_PRICE_SCALE {
        return Err(ErrorV1::InvalidPriceScale);
    }
    let scale = price_scale as u128;
    let mut price_sum = 0u128;
    let mut i = 0usize;
    while i < outcomes {
        if prices[i] > price_scale {
            return Err(ErrorV1::PriceOutOfRange);
        }
        price_sum += prices[i] as u128;
        i += 1;
    }
    if price_sum != scale {
        return Err(ErrorV1::SimplexSumMismatch);
    }

    // The dispersion numerator: the fixed pairwise loop of FEE_GEOMETRY §2,
    // with no intermediate truncation anywhere.
    let mut dispersion_numerator = 0u128;
    let mut left = 0usize;
    while left < outcomes {
        let mut right = left + 1;
        while right < outcomes {
            let gap = if payoffs[left] >= payoffs[right] {
                (payoffs[left] - payoffs[right]) as u128
            } else {
                (payoffs[right] - payoffs[left]) as u128
            };
            let term = (prices[left] as u128)
                .checked_mul(prices[right] as u128)
                .and_then(|value| value.checked_mul(gap))
                .ok_or(ErrorV1::ArithmeticOverflow)?;
            dispersion_numerator = dispersion_numerator
                .checked_add(term)
                .ok_or(ErrorV1::ArithmeticOverflow)?;
            right += 1;
        }
        left += 1;
    }

    // The price-free quotient norm: two comparisons per outcome.
    let mut lowest = payoffs[0];
    let mut highest = payoffs[0];
    let mut i = 1usize;
    while i < outcomes {
        if payoffs[i] < lowest {
            lowest = payoffs[i];
        }
        if payoffs[i] > highest {
            highest = payoffs[i];
        }
        i += 1;
    }
    let range = (highest - lowest) as u128;

    let rate_denominator = FEE_BPS_DENOMINATOR as u128;
    let scale_squared = scale
        .checked_mul(scale)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let base_denominator = rate_denominator
        .checked_mul(scale_squared)
        .and_then(|value| value.checked_mul(rate_denominator))
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let dispersion_term = (dispersion_bps as u128)
        .checked_mul(dispersion_numerator)
        .and_then(|value| value.checked_mul(rate_denominator))
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let floor_term = (floor_range_bps as u128)
        .checked_mul(range)
        .and_then(|value| value.checked_mul(rate_denominator))
        .and_then(|value| value.checked_mul(scale_squared))
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let base_numerator = dispersion_term
        .checked_add(floor_term)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    if prior_carry >= base_denominator {
        return Err(ErrorV1::FeeMismatch);
    }
    let exact_numerator = base_numerator
        .checked_add(prior_carry)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let floor_atoms = exact_numerator / base_denominator;
    let carry = exact_numerator % base_denominator;
    let terminal_ceil_atoms = if carry == 0 {
        floor_atoms
    } else {
        floor_atoms
            .checked_add(1)
            .ok_or(ErrorV1::ArithmeticOverflow)?
    };
    Ok(FeeQuoteV1 {
        base_numerator,
        base_denominator,
        exact_numerator,
        exact_denominator: base_denominator,
        floor_atoms,
        terminal_ceil_atoms,
        carry,
    })
}

/// Every variant selection named explicitly.
///
/// There is deliberately **no** `Default` and no builder: the struct has only
/// public fields, so a construction site must name every policy family.  No
/// code path may canonize a policy by omission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenPolicyV1 {
    /// Canonical fill allocation (A / B).
    pub allocation: AllocationPolicyV1,
    /// Self-cross normalization (N-a / N-b / N-c).
    pub self_cross: SelfCrossPolicyV1,
    /// All-or-none handling (2a / 2b / 2c).
    pub aon: AonPolicyV1,
    /// The one named rounding boundary (R-a / R-b / R-c).
    pub rounding: RoundingBoundaryV1,
    /// Residual-pair settlement (1a / 1b / 1c), recorded for the settlement layer.
    pub residual_settlement: ResidualSettlementV1,
    /// Kernel transfer phase gate (T-a / T-b), recorded for the kernel layer.
    pub transfer_phase: TransferPhaseV1,
    /// Portfolio lot rationing (P-a / P-b).
    pub portfolio_lots: PortfolioLotPolicyV1,
    /// Who carries the pairing proof.
    pub pairing_witness: PairingWitnessPolicyV1,
    /// Leftover-atom handling in the canonical allocator.
    pub dust: DustPolicy,
    /// Score family.
    pub score: ScorePolicyV1,
    /// Fee base.
    pub fee_base: FeeBaseV1,
}

impl FrozenPolicyV1 {
    /// Refuse policy combinations this implementation does not implement.
    pub fn validate(&self) -> Result<(), ErrorV1> {
        if self.portfolio_lots == PortfolioLotPolicyV1::MarginalProRataLots {
            return Err(ErrorV1::PolicyVariantUnimplemented);
        }
        match self.fee_base {
            FeeBaseV1::None => {}
            FeeBaseV1::FlatNotional { bps } => {
                if bps as u64 > FEE_BPS_DENOMINATOR {
                    return Err(ErrorV1::PolicyVariantUnimplemented);
                }
            }
            // The composite arithmetic is implemented, so a rate pair is
            // refused only for being unrepresentable, never for being nonzero.
            // Each rate is exact basis points over `FEE_BPS_DENOMINATOR`, the
            // same admissible band `FlatNotional` uses and the same band both
            // policy codecs pack into one `u32` wire slot.
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } => {
                if dispersion_bps as u64 > FEE_BPS_DENOMINATOR
                    || floor_range_bps as u64 > FEE_BPS_DENOMINATOR
                {
                    return Err(ErrorV1::PolicyVariantUnimplemented);
                }
            }
        }
        Ok(())
    }

    /// A deterministic, non-cryptographic policy tag folded into the digest.
    pub fn code(&self) -> u64 {
        let mut code = 0u64;
        code = code * 8 + self.allocation as u64;
        code = code * 8 + self.self_cross as u64;
        code = code * 8 + self.aon as u64;
        code = code * 8 + self.rounding as u64;
        code = code * 8 + self.residual_settlement as u64;
        code = code * 8 + self.transfer_phase as u64;
        code = code * 8 + self.portfolio_lots as u64;
        code = code * 8 + self.pairing_witness as u64;
        code = code * 8 + self.dust as u64;
        code = code * 8 + self.score as u64;
        match self.fee_base {
            FeeBaseV1::None => code * 65_536,
            FeeBaseV1::FlatNotional { bps } => code * 65_536 + 1 + bps as u64,
            // Offset past the whole admissible FlatNotional band
            // (`1..=1 + FEE_BPS_DENOMINATOR`).
            //
            // This legacy fold is **not injective over composite rate pairs**:
            // the band is 65,536 wide and the admissible pairs number
            // `10_001^2`, so `(1, 0)` and `(0, 1)` share a tag.  That is stated
            // rather than fixed because widening the band would move every
            // policy's tag and therefore every frozen candidate digest in the
            // tree.  Nothing rests on it: the cryptographic policy identity is
            // the canonical-bytes SHA-256
            // (`clutch-batch-policy-identity::batch_policy_digest`), which
            // carries both rates exactly, and the candidate digest separately
            // folds the domain's own `policy_id`.
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            } => {
                code * 65_536
                    + 2
                    + FEE_BPS_DENOMINATOR
                    + dispersion_bps as u64
                    + floor_range_bps as u64
            }
        }
    }
}

/// The frozen epoch domain: identity, shape, and the whole policy selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationDomainV1 {
    /// Must equal [`RELATION_VERSION_V1`].
    pub relation_version: u32,
    /// Market identity.
    pub market_id: u64,
    /// Book identity.
    pub book_id: u64,
    /// Clearing epoch; orders expiring before it are refused at admission.
    pub epoch: u64,
    /// Policy identity (semantic, not a commitment).
    pub policy_id: u64,
    /// Order-set identity.  Still caller supplied in the host model.
    pub order_set_id: u64,
    /// `2 ..= MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Admitted owner tags are `< owner_count`.
    pub owner_count: u16,
    /// Exact integer price scale; a complete set values at exactly this much.
    pub price_scale: u64,
    /// Seed of the frozen largest-remainder permutation.
    pub remainder_seed: u64,
    /// Every variant selection, named.
    pub policy: FrozenPolicyV1,
}

impl RelationDomainV1 {
    /// Validate the frozen domain against every bound this relation assumes.
    pub fn validate(&self) -> Result<(), ErrorV1> {
        if self.relation_version != RELATION_VERSION_V1 {
            return Err(ErrorV1::UnknownRelationVersion);
        }
        if self.outcome_count < 2 || self.outcome_count as usize > MAX_OUTCOMES {
            return Err(ErrorV1::InvalidOutcome);
        }
        if self.owner_count == 0 {
            return Err(ErrorV1::InvalidOwner);
        }
        if self.price_scale == 0 || self.price_scale > MAX_PRICE_SCALE {
            return Err(ErrorV1::InvalidPriceScale);
        }
        // The composite's common denominator is `kappa_den * S^2 * kappa'_den`,
        // so a fee-bearing composite domain carries a tighter price-scale bound
        // than the relation at large.  At zero rates the charge is identically
        // zero and no denominator is ever formed, so the bound does not apply —
        // keeping the zero-rate shape admissible at exactly the price scales
        // `FeeBaseV1::None` is.
        if let FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } = self.policy.fee_base
        {
            if (dispersion_bps != 0 || floor_range_bps != 0)
                && self.price_scale > MAX_COMPOSITE_PRICE_SCALE
            {
                return Err(ErrorV1::InvalidPriceScale);
            }
        }
        self.policy.validate()
    }

    fn outcomes(&self) -> usize {
        self.outcome_count as usize
    }
}

/// One single-Egg bound order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SingleEggOrderV1 {
    /// Nonzero and strictly increasing across the frozen book.
    pub canonical_order_id: u64,
    /// Bound owner tag, `< domain.owner_count`.
    pub owner: u16,
    /// Bound outcome, `< domain.outcome_count`.
    pub outcome: u8,
    /// Bound side.
    pub side: Side,
    /// Egg atoms, `> 0`.
    pub quantity: u64,
    /// Scaled integer limit price, `0 ..= domain.price_scale`.
    pub limit_price: u64,
    /// Minimum acceptable fill, `<= quantity`.
    pub minimum_fill: u64,
    /// Partial-fill policy.
    pub partial_policy: PartialPolicy,
    /// The order is admitted while `expiry_epoch >= domain.epoch`.
    pub expiry_epoch: u64,
}

/// One portfolio bound order: `lots` copies of a nonnegative coefficient vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioOrderV1 {
    /// Nonzero and strictly increasing across the frozen book.
    pub canonical_order_id: u64,
    /// Bound owner tag, `< domain.owner_count`.
    pub owner: u16,
    /// Bound side.
    pub side: Side,
    /// Exact nonnegative Egg atoms per lot; canonically zero beyond `active_len`.
    pub coefficients: [u64; MAX_OUTCOMES],
    /// `1 ..= domain.outcome_count`.
    pub active_len: u8,
    /// Lots, `> 0`.
    pub lots: u64,
    /// Scaled integer collateral limit per lot.
    pub limit_collateral_per_lot: u64,
    /// Minimum acceptable lot fill, `<= lots`.
    pub minimum_fill_lots: u64,
    /// Partial-fill policy.
    pub partial_policy: PartialPolicy,
    /// The order is admitted while `expiry_epoch >= domain.epoch`.
    pub expiry_epoch: u64,
}

/// The two admitted order families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderV1 {
    /// One outcome, one side.
    SingleEgg(SingleEggOrderV1),
    /// A coefficient vector over outcomes, one side.
    Portfolio(PortfolioOrderV1),
}

/// The canonical padding order.  Every array slot at or beyond `len` must equal
/// this value exactly, so noncanonical padding can never influence a digest.
pub const fn empty_order_v1() -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: 0,
        owner: 0,
        outcome: 0,
        side: Side::Buy,
        quantity: 0,
        limit_price: 0,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 0,
    })
}

impl OrderV1 {
    /// Canonical tie-break identity.
    pub fn id(&self) -> u64 {
        match self {
            OrderV1::SingleEgg(o) => o.canonical_order_id,
            OrderV1::Portfolio(o) => o.canonical_order_id,
        }
    }

    /// Bound owner tag.
    pub fn owner(&self) -> u16 {
        match self {
            OrderV1::SingleEgg(o) => o.owner,
            OrderV1::Portfolio(o) => o.owner,
        }
    }

    /// Bound side.
    pub fn side(&self) -> Side {
        match self {
            OrderV1::SingleEgg(o) => o.side,
            OrderV1::Portfolio(o) => o.side,
        }
    }

    /// Partial-fill policy.
    pub fn partial_policy(&self) -> PartialPolicy {
        match self {
            OrderV1::SingleEgg(o) => o.partial_policy,
            OrderV1::Portfolio(o) => o.partial_policy,
        }
    }

    /// Order units: Egg atoms for single-Egg orders, lots for portfolios.
    pub fn quantity(&self) -> u64 {
        match self {
            OrderV1::SingleEgg(o) => o.quantity,
            OrderV1::Portfolio(o) => o.lots,
        }
    }

    /// Minimum fill in order units.
    pub fn minimum_fill(&self) -> u64 {
        match self {
            OrderV1::SingleEgg(o) => o.minimum_fill,
            OrderV1::Portfolio(o) => o.minimum_fill_lots,
        }
    }

    /// Expiry epoch.
    pub fn expiry_epoch(&self) -> u64 {
        match self {
            OrderV1::SingleEgg(o) => o.expiry_epoch,
            OrderV1::Portfolio(o) => o.expiry_epoch,
        }
    }

    /// Egg atoms this order contributes to `outcome` when filled `units`.
    pub fn leg_quantity(&self, outcome: u8, units: u64) -> Result<u64, ErrorV1> {
        match self {
            OrderV1::SingleEgg(o) => Ok(if o.outcome == outcome { units } else { 0 }),
            OrderV1::Portfolio(o) => {
                let coefficient = o.coefficients[outcome as usize];
                units
                    .checked_mul(coefficient)
                    .ok_or(ErrorV1::ArithmeticOverflow)
            }
        }
    }

    /// Whether this order can ever touch `outcome`.
    pub fn touches(&self, outcome: u8) -> bool {
        match self {
            OrderV1::SingleEgg(o) => o.outcome == outcome,
            OrderV1::Portfolio(o) => o.coefficients[outcome as usize] != 0,
        }
    }

    /// An order carries a minimum-fill obligation when it is all-or-none or its
    /// minimum exceeds one unit.  Only such orders may appear in an AON mask.
    pub fn carries_minimum_obligation(&self) -> bool {
        self.partial_policy() == PartialPolicy::AllOrNone || self.minimum_fill() > 1
    }

    /// Reservation in price units: what the owner locked when placing.
    pub(crate) fn reservation_price_units(&self, price_scale: u64) -> Result<u128, ErrorV1> {
        match self {
            OrderV1::SingleEgg(o) => match o.side {
                Side::Buy => Ok((o.quantity as u128) * (o.limit_price as u128)),
                Side::Sell => Ok(0),
            },
            OrderV1::Portfolio(o) => match o.side {
                Side::Buy => Ok((o.lots as u128)
                    * (o.limit_collateral_per_lot as u128)
                    * (price_scale as u128)),
                Side::Sell => Ok(0),
            },
        }
    }
}

/// A frozen book: at most [`MAX_ORDERS`] orders, at most
/// [`MAX_PORTFOLIO_ORDERS`] of which are portfolios.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BookV1 {
    /// Orders in canonical identifier order.
    pub orders: [OrderV1; MAX_ORDERS],
    /// Number of admitted orders.
    pub len: u8,
}

impl BookV1 {
    /// An empty frozen book.
    pub fn empty() -> Self {
        Self {
            orders: [empty_order_v1(); MAX_ORDERS],
            len: 0,
        }
    }

    /// V0 admission: every check that must refuse before any fee or liveness
    /// charge can be assessed.
    pub fn validate(&self, domain: &RelationDomainV1) -> Result<(), ErrorV1> {
        domain.validate()?;
        if self.len as usize > MAX_ORDERS {
            return Err(ErrorV1::TooManyOrders);
        }
        let outcomes = domain.outcomes();
        let mut previous_id = 0u64;
        let mut portfolios = 0usize;
        let mut i = 0usize;
        while i < self.len as usize {
            let order = self.orders[i];
            if order.id() == 0 || order.id() <= previous_id {
                return Err(ErrorV1::NonCanonicalOrderOrder);
            }
            previous_id = order.id();
            if order.owner() >= domain.owner_count {
                return Err(ErrorV1::InvalidOwner);
            }
            if order.expiry_epoch() < domain.epoch {
                return Err(ErrorV1::ExpiredOrder);
            }
            match order {
                OrderV1::SingleEgg(o) => {
                    if o.outcome as usize >= outcomes {
                        return Err(ErrorV1::InvalidOutcome);
                    }
                    if o.quantity == 0 {
                        return Err(ErrorV1::InvalidQuantity);
                    }
                    if o.minimum_fill > o.quantity {
                        return Err(ErrorV1::InvalidMinimumFill);
                    }
                    if o.limit_price > domain.price_scale {
                        return Err(ErrorV1::PriceOutOfRange);
                    }
                }
                OrderV1::Portfolio(o) => {
                    portfolios += 1;
                    if portfolios > MAX_PORTFOLIO_ORDERS {
                        return Err(ErrorV1::TooManyPortfolios);
                    }
                    if o.active_len == 0 || o.active_len as usize > outcomes {
                        return Err(ErrorV1::InvalidOutcome);
                    }
                    if o.lots == 0 {
                        return Err(ErrorV1::InvalidQuantity);
                    }
                    if o.minimum_fill_lots > o.lots {
                        return Err(ErrorV1::InvalidMinimumFill);
                    }
                    let mut j = 0usize;
                    let mut nonzero = false;
                    while j < o.active_len as usize {
                        if o.coefficients[j] != 0 {
                            nonzero = true;
                        }
                        j += 1;
                    }
                    if !nonzero {
                        return Err(ErrorV1::InvalidQuantity);
                    }
                    while j < MAX_OUTCOMES {
                        if o.coefficients[j] != 0 {
                            return Err(ErrorV1::NonCanonicalPadding);
                        }
                        j += 1;
                    }
                    // A per-lot value must not overflow the price-unit ledger.
                    let mut value = 0u128;
                    let mut k = 0usize;
                    while k < o.active_len as usize {
                        value = value
                            .checked_add(
                                (o.coefficients[k] as u128)
                                    .checked_mul(domain.price_scale as u128)
                                    .ok_or(ErrorV1::ArithmeticOverflow)?,
                            )
                            .ok_or(ErrorV1::ArithmeticOverflow)?;
                        k += 1;
                    }
                    (o.lots as u128)
                        .checked_mul(value)
                        .ok_or(ErrorV1::ArithmeticOverflow)?;
                }
            }
            if order.partial_policy() == PartialPolicy::AllOrNone
                && order.minimum_fill() != order.quantity()
            {
                return Err(ErrorV1::InvalidMinimumFill);
            }
            if domain.policy.aon == AonPolicyV1::RefuseAdmission {
                if order.partial_policy() == PartialPolicy::AllOrNone {
                    return Err(ErrorV1::AonNotAdmitted);
                }
                if order.minimum_fill() > 1 {
                    return Err(ErrorV1::MinimumFillNotAdmitted);
                }
            }
            order.reservation_price_units(domain.price_scale)?;
            i += 1;
        }
        while i < MAX_ORDERS {
            if self.orders[i] != empty_order_v1() {
                return Err(ErrorV1::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok(())
    }
}

/// The admitted book after V0 normalization: owner tags interned into slots and
/// (under `N-b`) same-owner overlap cancelled price-independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedBookV1 {
    /// The admitted orders, unchanged.
    pub orders: [OrderV1; MAX_ORDERS],
    /// Number of admitted orders.
    pub len: u8,
    /// Quantity cancelled by `N-b` netting, in order units.
    pub cancelled: [u64; MAX_ORDERS],
    /// Interned owner slot per order.
    pub owner_slot: [u16; MAX_ORDERS],
    /// Number of distinct owners in the book.
    pub owner_slot_count: u16,
}

impl NormalizedBookV1 {
    /// The empty normalized book: the placeholder every [`normalize_into`]
    /// call overwrites completely, so a caller can initialize its out-slot
    /// without a second book-sized temporary.  It is also exactly the valid
    /// normalization of [`BookV1::empty`].
    pub const EMPTY: Self = Self {
        orders: [empty_order_v1(); MAX_ORDERS],
        len: 0,
        cancelled: [0u64; MAX_ORDERS],
        owner_slot: [0u16; MAX_ORDERS],
        owner_slot_count: 0,
    };

    /// Quantity that can still clear, in order units.
    pub fn effective_quantity(&self, index: usize) -> u64 {
        self.orders[index]
            .quantity()
            .saturating_sub(self.cancelled[index])
    }

    /// Minimum fill clamped to the effective quantity.
    pub fn effective_minimum_fill(&self, index: usize) -> u64 {
        let effective = self.effective_quantity(index);
        let minimum = self.orders[index].minimum_fill();
        if minimum > effective {
            effective
        } else {
            minimum
        }
    }
}

/// V0: admit, intern owners, and apply the frozen self-cross normalization.
///
/// A thin by-value convenience over [`normalize_into`].  The
/// [`NormalizedBookV1`] is 11,912 bytes, so this form costs a book-sized
/// callee frame plus the return copy; a caller on a bounded stack should hold
/// its own slot and call [`normalize_into`] directly.
pub fn normalize(domain: &RelationDomainV1, book: &BookV1) -> Result<NormalizedBookV1, ErrorV1> {
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    Ok(normalized)
}

/// [`normalize`] into a caller-owned slot.
///
/// The normalization writes fields directly into `out`, so this entry point
/// adds no book-sized temporary to any frame.  On `Err`, `out` holds an
/// unspecified partial normalization and must not be read.
pub fn normalize_into(
    domain: &RelationDomainV1,
    book: &BookV1,
    out: &mut NormalizedBookV1,
) -> Result<(), ErrorV1> {
    book.validate(domain)?;
    out.orders = book.orders;
    out.len = book.len;
    out.cancelled = [0u64; MAX_ORDERS];
    out.owner_slot = [0u16; MAX_ORDERS];
    out.owner_slot_count = 0;
    let mut owners = [0u16; MAX_OWNER_SLOTS];
    let mut owner_count = 0usize;
    let mut i = 0usize;
    while i < out.len as usize {
        let owner = out.orders[i].owner();
        let mut slot = usize::MAX;
        let mut j = 0usize;
        while j < owner_count {
            if owners[j] == owner {
                slot = j;
                break;
            }
            j += 1;
        }
        if slot == usize::MAX {
            if owner_count >= MAX_OWNER_SLOTS {
                return Err(ErrorV1::TooManyOrders);
            }
            owners[owner_count] = owner;
            slot = owner_count;
            owner_count += 1;
        }
        out.owner_slot[i] = slot as u16;
        i += 1;
    }
    out.owner_slot_count = owner_count as u16;

    match domain.policy.self_cross {
        SelfCrossPolicyV1::AllowGateAtPairing => Ok(()),
        SelfCrossPolicyV1::RefuseOverlap => refuse_self_cross(domain, out),
        SelfCrossPolicyV1::NetAtAdmission => net_self_cross(domain, out),
    }
}

fn refuse_self_cross(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
) -> Result<(), ErrorV1> {
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let mut slot = 0usize;
        while slot < normalized.owner_slot_count as usize {
            let mut has_buy = false;
            let mut has_sell = false;
            let mut i = 0usize;
            while i < normalized.len as usize {
                if normalized.owner_slot[i] as usize == slot
                    && normalized.orders[i].touches(outcome as u8)
                {
                    match normalized.orders[i].side() {
                        Side::Buy => has_buy = true,
                        Side::Sell => has_sell = true,
                    }
                }
                i += 1;
            }
            if has_buy && has_sell {
                return Err(ErrorV1::SelfCrossRefused);
            }
            slot += 1;
        }
        outcome += 1;
    }
    Ok(())
}

fn net_self_cross(
    domain: &RelationDomainV1,
    normalized: &mut NormalizedBookV1,
) -> Result<(), ErrorV1> {
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let mut slot = 0usize;
        while slot < normalized.owner_slot_count as usize {
            let mut buy_total = 0u64;
            let mut sell_total = 0u64;
            let mut i = 0usize;
            while i < normalized.len as usize {
                if normalized.owner_slot[i] as usize == slot
                    && normalized.orders[i].touches(outcome as u8)
                {
                    let remaining = normalized.effective_quantity(i);
                    match normalized.orders[i].side() {
                        Side::Buy => {
                            buy_total = buy_total
                                .checked_add(remaining)
                                .ok_or(ErrorV1::ArithmeticOverflow)?
                        }
                        Side::Sell => {
                            sell_total = sell_total
                                .checked_add(remaining)
                                .ok_or(ErrorV1::ArithmeticOverflow)?
                        }
                    }
                }
                i += 1;
            }
            if buy_total != 0 && sell_total != 0 {
                // Lot-coupled netting of a portfolio order is unresolved (design
                // §16 open question 3), so an overlap that involves a portfolio
                // refuses instead of netting partially.
                let mut k = 0usize;
                while k < normalized.len as usize {
                    if normalized.owner_slot[k] as usize == slot
                        && normalized.orders[k].touches(outcome as u8)
                        && matches!(normalized.orders[k], OrderV1::Portfolio(_))
                    {
                        return Err(ErrorV1::SelfCrossRefused);
                    }
                    k += 1;
                }
                let netted = if buy_total < sell_total {
                    buy_total
                } else {
                    sell_total
                };
                cancel_side(normalized, slot, outcome as u8, Side::Buy, netted)?;
                cancel_side(normalized, slot, outcome as u8, Side::Sell, netted)?;
            }
            slot += 1;
        }
        outcome += 1;
    }
    Ok(())
}

fn cancel_side(
    normalized: &mut NormalizedBookV1,
    slot: usize,
    outcome: u8,
    side: Side,
    mut remaining: u64,
) -> Result<(), ErrorV1> {
    let mut i = 0usize;
    while i < normalized.len as usize && remaining != 0 {
        if normalized.owner_slot[i] as usize == slot
            && normalized.orders[i].side() == side
            && normalized.orders[i].touches(outcome)
        {
            let available = normalized.effective_quantity(i);
            let take = if available < remaining {
                available
            } else {
                remaining
            };
            if take != 0 {
                // Netting an all-or-none order to a nonzero remainder would
                // destroy its own semantics; refuse instead.
                if normalized.orders[i].partial_policy() == PartialPolicy::AllOrNone
                    && take != available
                {
                    return Err(ErrorV1::SelfCrossRefused);
                }
                normalized.cancelled[i] = normalized.cancelled[i]
                    .checked_add(take)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                remaining -= take;
            }
        }
        i += 1;
    }
    if remaining != 0 {
        return Err(ErrorV1::ArithmeticOverflow);
    }
    Ok(())
}

/// V2 eligibility class at a candidate price vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityV1 {
    /// Strictly inside its limit; must fill fully under allocation A.
    Strict,
    /// Exactly at its limit; absorbs the residual pro-rata.
    Marginal,
    /// Outside its limit; its fill must be zero.
    Ineligible,
}

/// V1: exact simplex validation.
pub fn validate_prices(
    domain: &RelationDomainV1,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<(), ErrorV1> {
    let mut sum = 0u64;
    let mut i = 0usize;
    while i < domain.outcomes() {
        if prices[i] > domain.price_scale {
            return Err(ErrorV1::PriceOutOfRange);
        }
        sum = sum
            .checked_add(prices[i])
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        i += 1;
    }
    while i < MAX_OUTCOMES {
        if prices[i] != 0 {
            return Err(ErrorV1::NonCanonicalPadding);
        }
        i += 1;
    }
    if sum != domain.price_scale {
        return Err(ErrorV1::SimplexSumMismatch);
    }
    Ok(())
}

/// V2: classify one order exactly, with no division and no rounding.
pub fn classify_order(
    domain: &RelationDomainV1,
    order: &OrderV1,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<EligibilityV1, ErrorV1> {
    if order.expiry_epoch() < domain.epoch {
        return Err(ErrorV1::ExpiredOrder);
    }
    let (limit, value) = match order {
        OrderV1::SingleEgg(o) => (o.limit_price as u128, prices[o.outcome as usize] as u128),
        OrderV1::Portfolio(o) => {
            let mut value = 0u128;
            let mut i = 0usize;
            while i < domain.outcomes() {
                value = value
                    .checked_add(
                        (o.coefficients[i] as u128)
                            .checked_mul(prices[i] as u128)
                            .ok_or(ErrorV1::ArithmeticOverflow)?,
                    )
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                i += 1;
            }
            let limit = (o.limit_collateral_per_lot as u128)
                .checked_mul(domain.price_scale as u128)
                .ok_or(ErrorV1::ArithmeticOverflow)?;
            (limit, value)
        }
    };
    let class = match order.side() {
        Side::Buy => match limit.cmp(&value) {
            Ordering::Greater => EligibilityV1::Strict,
            Ordering::Equal => EligibilityV1::Marginal,
            Ordering::Less => EligibilityV1::Ineligible,
        },
        Side::Sell => match limit.cmp(&value) {
            Ordering::Less => EligibilityV1::Strict,
            Ordering::Equal => EligibilityV1::Marginal,
            Ordering::Greater => EligibilityV1::Ineligible,
        },
    };
    Ok(class)
}

fn classify_all(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<[EligibilityV1; MAX_ORDERS], ErrorV1> {
    let mut classes = [EligibilityV1::Ineligible; MAX_ORDERS];
    let mut i = 0usize;
    while i < normalized.len as usize {
        classes[i] = if normalized.effective_quantity(i) == 0 {
            EligibilityV1::Ineligible
        } else {
            classify_order(domain, &normalized.orders[i], prices)?
        };
        i += 1;
    }
    Ok(classes)
}

/// One end of a settlement slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegRefV1 {
    /// A filled leg of the order at this index, on the slice's outcome.
    Order(u8),
    /// The global virtual split, serving buy legs on every outcome.
    Split,
    /// The global virtual merge, absorbing sell legs on every outcome.
    Merge,
}

/// One executable transfer of the frozen decomposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingSliceV1 {
    /// A buy leg, or [`LegRefV1::Merge`].
    pub buy_ref: LegRefV1,
    /// A sell leg, or [`LegRefV1::Split`].
    pub sell_ref: LegRefV1,
    /// The bound outcome of both ends.
    pub outcome: u8,
    /// Egg atoms moved by this slice.
    pub quantity: u64,
}

/// The explicit pairing witness of the `ExplicitSlices` fallback, and the frozen
/// output of the canonical constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingWitnessV1 {
    /// The slices, in constructor emission order.
    pub slices: [PairingSliceV1; MAX_SLICES],
    /// Number of live slices.
    pub len: u16,
}

impl PairingWitnessV1 {
    /// An empty decomposition.
    pub fn empty() -> Self {
        Self {
            slices: [PairingSliceV1 {
                buy_ref: LegRefV1::Split,
                sell_ref: LegRefV1::Split,
                outcome: 0,
                quantity: 0,
            }; MAX_SLICES],
            len: 0,
        }
    }
}

/// The exact lexicographic score of a valid candidate (§11).
///
/// The score never turns an invalid candidate valid; it only orders valid ones
/// inside one proposal window, and the accepted candidate is the best **valid
/// submitted** candidate, never a claimed optimum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreV1 {
    /// Component 1 (maximize), net of the component 2 self-overlap term.
    pub weighted_direct_volume: i128,
    /// Component 3 (maximize): exact limit surplus in price units.
    pub limit_surplus_price_units: u128,
    /// Component 4 (maximize): distinct participating owners, not orders.
    pub distinct_owners: u16,
    /// Component 5 (minimize): `sigma + mu`.
    pub churn: u64,
    /// Component 6 (ascending): the canonical candidate digest.
    pub digest: u128,
}

impl ScoreV1 {
    /// The zero score of the canonical empty candidate, before digest binding.
    pub const ZERO: Self = Self {
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        distinct_owners: 0,
        churn: 0,
        digest: 0,
    };

    /// The frozen total order: every component's direction is explicit and the
    /// digest makes the order total.
    pub fn total_order(&self, other: &Self) -> Ordering {
        match self
            .weighted_direct_volume
            .cmp(&other.weighted_direct_volume)
        {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match self
            .limit_surplus_price_units
            .cmp(&other.limit_surplus_price_units)
        {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match self.distinct_owners.cmp(&other.distinct_owners) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match other.churn.cmp(&self.churn) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        other.digest.cmp(&self.digest)
    }

    /// True when `self` would be selected over `other`.
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.total_order(other) == Ordering::Greater
    }
}

/// The candidate witness.  Its only free economic coordinates are the price
/// vector, the net imbalance carried by `virtual_split`/`virtual_merge`, and —
/// under AON variant 2b — the honored mask.  Everything else is derived and
/// checked for exact equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateV1 {
    /// Number of orders this candidate binds; must equal the book length.
    pub order_len: u8,
    /// Exact scaled prices on the simplex.
    pub prices: [u64; MAX_OUTCOMES],
    /// `sigma`: complete sets created by the single global virtual split.
    pub virtual_split: u64,
    /// `mu`: complete sets destroyed by the single global virtual merge.
    pub virtual_merge: u64,
    /// Egg atoms for single-Egg orders, lots for portfolio orders.
    pub fills: [u64; MAX_ORDERS],
    /// Honored minimum-fill subset; must be zero unless AON variant 2b is frozen.
    pub honored_aon_mask: u64,
    /// Claimed score, recomputed at V9.
    pub claimed_score: ScoreV1,
    /// Claimed digest, recomputed at V9.
    pub canonical_candidate_digest: u128,
}

impl CandidateV1 {
    /// The canonical empty candidate at a price vector: no fills, no churn.
    pub fn empty(order_len: u8, prices: [u64; MAX_OUTCOMES]) -> Self {
        Self {
            order_len,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            fills: [0u64; MAX_ORDERS],
            honored_aon_mask: 0,
            claimed_score: ScoreV1::ZERO,
            canonical_candidate_digest: 0,
        }
    }

    /// The net imbalance `c = sigma - mu` every outcome must carry.
    pub fn imbalance(&self) -> i128 {
        self.virtual_split as i128 - self.virtual_merge as i128
    }
}

/// The recomputed result of a valid candidate.  Every field is derived from the
/// frozen domain, the frozen book, and the candidate witness; no claimed
/// aggregate is ever accepted because it matches another claimed aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummaryV1 {
    /// Active outcomes.
    pub outcome_count: u8,
    /// `B_i`: executed buy quantity per outcome.
    pub buy_flow: [u64; MAX_OUTCOMES],
    /// `E_i`: executed sell quantity per outcome.
    pub sell_flow: [u64; MAX_OUTCOMES],
    /// `F_i = B_i + mu = E_i + sigma`.
    pub total_flow: [u64; MAX_OUTCOMES],
    /// `F_i - sigma - mu`: flow that pairs two real owners.
    pub direct_flow: [u64; MAX_OUTCOMES],
    /// `sigma`.
    pub virtual_split: u64,
    /// `mu`.
    pub virtual_merge: u64,
    /// Egg atoms reserved by sellers at admission, per outcome.
    pub opening_reserved_egg: [u64; MAX_OUTCOMES],
    /// Egg atoms returned because the order did not fill, per outcome.
    pub unfilled_refund_egg: [u64; MAX_OUTCOMES],
    /// Egg atoms cancelled by `N-b` netting, per outcome.
    pub netting_cancelled_egg: [u64; MAX_OUTCOMES],
    /// Collateral reserved by buyers at admission, in price units.
    pub opening_reserved_cash_price_units: u128,
    /// Consideration owed by filled buy legs, in price units.
    pub buyer_consideration_price_units: u128,
    /// Consideration owed to filled sell legs, in price units.
    pub seller_credit_price_units: u128,
    /// `sigma * price_scale`.
    pub split_cost_price_units: u128,
    /// `mu * price_scale`.
    pub merge_proceeds_price_units: u128,
    /// Fee collected from payers, in price units.
    pub fee_price_units: u128,
    /// Sub-basis-point fee remainder carried per owner identity.
    pub fee_carry_bps_units: u128,
    /// Collateral returned to buyers, in price units.
    pub cash_refund_price_units: u128,
    /// Every remainder atom of the named rounding boundary.
    pub rounding_pot_price_units: u128,
    /// Collateral atoms debited from payers.
    pub debit_atoms: u128,
    /// Collateral atoms credited to payees.
    pub credit_atoms: u128,
    /// Owners with any nonzero fill.
    pub distinct_participating_owners: u16,
    /// `sum_O sum_i min(buyfill_i(O), sellfill_i(O))`, zero under `N-a`/`N-b`.
    pub self_overlap_volume: u64,
    /// The recomputed score.
    pub score: ScoreV1,
    /// The recomputed candidate digest.
    pub candidate_digest: u128,
}

/// Every refusal this relation can produce, tagged by the stage that owns it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV1 {
    // V0
    /// The domain names a relation version this code does not implement.
    UnknownRelationVersion,
    /// The domain's price scale is zero or wider than the accumulator bound.
    InvalidPriceScale,
    /// The frozen policy names a variant this implementation does not implement.
    PolicyVariantUnimplemented,
    /// Owner tag outside the admitted owner set.
    InvalidOwner,
    /// Outcome index or portfolio `active_len` out of range.
    InvalidOutcome,
    /// Zero quantity, zero lots, or an all-zero coefficient vector.
    InvalidQuantity,
    /// `minimum_fill > quantity`, or all-or-none with a partial minimum.
    InvalidMinimumFill,
    /// Order identifiers are not nonzero and strictly increasing.
    NonCanonicalOrderOrder,
    /// A padding slot, inactive price, or inactive coefficient was nonzero.
    NonCanonicalPadding,
    /// All-or-none is not admitted under AON variant 2a.
    AonNotAdmitted,
    /// `minimum_fill > 1` is not admitted under AON variant 2a.
    MinimumFillNotAdmitted,
    /// One owner stands on both sides of one outcome under `N-a`, or the
    /// overlap cannot be netted under `N-b`.
    SelfCrossRefused,
    /// The order expired before this epoch.
    ExpiredOrder,
    /// More than [`MAX_ORDERS`] orders, or more distinct owners than slots.
    TooManyOrders,
    /// More than [`MAX_PORTFOLIO_ORDERS`] portfolio orders.
    TooManyPortfolios,
    // V1
    /// The active prices do not sum to the price scale.
    SimplexSumMismatch,
    /// A price or limit is above the price scale.
    PriceOutOfRange,
    // V2
    /// An ineligible order carries a nonzero fill.
    IneligibleFill,
    // V3
    /// The fill vector is not the canonical one for this `(p, c, mask)`.
    CandidateMismatch,
    /// A strict order cannot fill fully at this `(p, c)` under allocation A.
    StrictUnderfill,
    /// A fill exceeds the order's effective quantity.
    FillExceedsQuantity,
    /// A nonzero fill is below the order's minimum.
    MinimumFillViolation,
    /// An all-or-none order is filled partially.
    AllOrNoneViolation,
    /// A masked order is ineligible or not filled to its full size.
    AonMaskDishonored,
    /// An unhonored minimum-fill order carries a nonzero fill.
    AonMaskLeak,
    /// A mask bit names an order that carries no minimum-fill obligation, or the
    /// mask is nonzero under a policy that has no mask.
    AonMaskNotApplicable,
    /// The canonical allocation needs a leftover atom under `DustPolicy::Reject`.
    DustRejected,
    // V4
    /// The fills do not carry one constant net imbalance on every outcome.
    OutcomeConservationMismatch,
    /// `min(sigma, mu) != 0`.
    ChurnNotCanonical,
    /// The candidate proposed more conversion than the book supports.
    InfeasibleVirtualLeg,
    // V5
    /// The pairing-feasibility inequality fails for this owner and outcome, so
    /// no complete executable pairing exists.
    PairingInfeasible {
        /// The outcome whose flow cannot absorb the owner's participation.
        outcome: u8,
        /// The owner tag whose participation exceeds the flow.
        owner: u16,
    },
    /// A submitted slice is not an executable transfer.
    SliceNotExecutable,
    /// Submitted slices do not sum to the fills or to `sigma`/`mu`.
    SliceSumMismatch,
    /// An explicit pairing witness was submitted under the recomputed-constructor
    /// policy.
    PairingWitnessNotAdmitted,
    /// No explicit pairing witness was submitted under the fallback policy.
    PairingWitnessMissing,
    /// The canonical constructor could not complete a feasible decomposition.
    /// Reaching this from a candidate that passed V5 is a falsified claim, not a
    /// refusal the design predicts.
    ConstructorStalled,
    /// The decomposition needs more than [`MAX_SLICES`] slices.
    SliceCapacityExceeded,
    // V6
    /// A recomputed consideration disagrees with the exact ledger.
    ConsiderationMismatch,
    /// A remainder atom exists under the exact-or-refuse rounding variant.
    RemainderRequired,
    // V7
    /// A recomputed fee disagrees with the exact ledger.
    FeeMismatch,
    /// The payer's reservation cannot fund consideration plus fee.
    FeePayerUnfunded,
    // V8
    /// A per-asset conservation equation does not close.
    ConservationFailure,
    // V9
    /// The claimed score is not the recomputed score.
    ScoreMismatch,
    /// The claimed digest is not the recomputed digest.
    DigestMismatch,
    // any
    /// An exact integer accumulator would have overflowed.
    ArithmeticOverflow,
    /// A bounded constructor search found no valid candidate (epoch lapse).
    NoValidCandidate,
    /// A bounded constructor search exceeded its explicit budget.
    SearchBudgetExceeded,
}

/// Per-outcome executed flow recomputed from a fill vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowsV1 {
    /// `B_i`.
    pub buy: [u64; MAX_OUTCOMES],
    /// `E_i`.
    pub sell: [u64; MAX_OUTCOMES],
}

/// Per-(owner slot, outcome) filled participation, the V5 accumulation table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticipationV1 {
    /// Filled buy quantity per owner slot and outcome.
    pub buy: [[u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
    /// Filled sell quantity per owner slot and outcome.
    pub sell: [[u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
}

impl ParticipationV1 {
    fn zeroed() -> Self {
        Self {
            buy: [[0u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
            sell: [[0u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
        }
    }

    /// Zero every cell in place, through the reference.
    ///
    /// `*table = ParticipationV1::zeroed()` materializes the whole 16 KiB
    /// table on the callee's frame before copying it out (measured 24,704
    /// bytes on the SBF probe — six times the 4,096-byte frame).  Writing
    /// the zeros element by element keeps the frame flat; the loop lowers
    /// to a plain in-place fill.
    fn zero_in_place(&mut self) {
        let mut slot = 0usize;
        while slot < MAX_OWNER_SLOTS {
            let mut outcome = 0usize;
            while outcome < MAX_OUTCOMES {
                self.buy[slot][outcome] = 0;
                self.sell[slot][outcome] = 0;
                outcome += 1;
            }
            slot += 1;
        }
    }

    /// `part_i(O) = buyfill_i(O) + sellfill_i(O)`.
    pub fn participation(&self, slot: usize, outcome: usize) -> Result<u64, ErrorV1> {
        self.buy[slot][outcome]
            .checked_add(self.sell[slot][outcome])
            .ok_or(ErrorV1::ArithmeticOverflow)
    }
}

/// Recompute `B_i` and `E_i` from a fill vector, expanding portfolio legs.
pub fn flows_from_fills(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    fills: &[u64; MAX_ORDERS],
) -> Result<FlowsV1, ErrorV1> {
    let mut flows = FlowsV1 {
        buy: [0u64; MAX_OUTCOMES],
        sell: [0u64; MAX_OUTCOMES],
    };
    let mut i = 0usize;
    while i < normalized.len as usize {
        let order = normalized.orders[i];
        let fill = fills[i];
        if fill != 0 {
            let mut outcome = 0usize;
            while outcome < domain.outcomes() {
                let leg = order.leg_quantity(outcome as u8, fill)?;
                if leg != 0 {
                    let target = match order.side() {
                        Side::Buy => &mut flows.buy[outcome],
                        Side::Sell => &mut flows.sell[outcome],
                    };
                    *target = target.checked_add(leg).ok_or(ErrorV1::ArithmeticOverflow)?;
                }
                outcome += 1;
            }
        }
        i += 1;
    }
    Ok(flows)
}

/// Accumulate the per-(owner, outcome) participation table from a fill vector.
pub fn participation_from_fills(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    fills: &[u64; MAX_ORDERS],
    table: &mut ParticipationV1,
) -> Result<(), ErrorV1> {
    table.zero_in_place();
    let mut i = 0usize;
    while i < normalized.len as usize {
        let order = normalized.orders[i];
        let fill = fills[i];
        let slot = normalized.owner_slot[i] as usize;
        if fill != 0 {
            let mut outcome = 0usize;
            while outcome < domain.outcomes() {
                let leg = order.leg_quantity(outcome as u8, fill)?;
                if leg != 0 {
                    let cell = match order.side() {
                        Side::Buy => &mut table.buy[slot][outcome],
                        Side::Sell => &mut table.sell[slot][outcome],
                    };
                    *cell = cell.checked_add(leg).ok_or(ErrorV1::ArithmeticOverflow)?;
                }
                outcome += 1;
            }
        }
        i += 1;
    }
    Ok(())
}

/// V5: the exact integer feasibility gate.
///
/// By the design's feasibility theorem this inequality is necessary **and**
/// sufficient for the existence of a complete executable pairing under
/// per-outcome conservation.  That statement is a design argument backed by the
/// exhaustive oracle in this crate's tests; it is not a machine-checked theorem.
pub fn check_pairing_feasibility(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    table: &ParticipationV1,
    flows: &FlowsV1,
    virtual_merge: u64,
) -> Result<(), ErrorV1> {
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let total_flow = flows.buy[outcome]
            .checked_add(virtual_merge)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        let mut slot = 0usize;
        while slot < normalized.owner_slot_count as usize {
            if table.participation(slot, outcome)? > total_flow {
                return Err(ErrorV1::PairingInfeasible {
                    outcome: outcome as u8,
                    owner: owner_tag(normalized, slot),
                });
            }
            slot += 1;
        }
        outcome += 1;
    }
    Ok(())
}

fn owner_tag(normalized: &NormalizedBookV1, slot: usize) -> u16 {
    let mut i = 0usize;
    while i < normalized.len as usize {
        if normalized.owner_slot[i] as usize == slot {
            return normalized.orders[i].owner();
        }
        i += 1;
    }
    u16::MAX
}

/// The canonical fill vector derived from the free coordinates `(p, c, mask)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalFillsV1 {
    /// The derived fills, in order units.
    pub fills: [u64; MAX_ORDERS],
    /// The derived per-outcome flow.
    pub flows: FlowsV1,
}

struct DerivationState {
    active: [bool; MAX_ORDERS],
    forced: [bool; MAX_ORDERS],
    honored: [bool; MAX_ORDERS],
}

pub(crate) fn mask_bit(mask: u64, index: usize) -> bool {
    index < 64 && (mask >> index) & 1 == 1
}

fn derivation_state(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    classes: &[EligibilityV1; MAX_ORDERS],
    mask: u64,
) -> Result<DerivationState, ErrorV1> {
    let mut state = DerivationState {
        active: [false; MAX_ORDERS],
        forced: [false; MAX_ORDERS],
        honored: [false; MAX_ORDERS],
    };
    let witnessed_mask = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
    let mut i = 0usize;
    while i < MAX_ORDERS {
        let bit = mask_bit(mask, i);
        if bit && (i >= normalized.len as usize || !witnessed_mask) {
            return Err(ErrorV1::AonMaskNotApplicable);
        }
        if i < normalized.len as usize {
            let order = normalized.orders[i];
            if bit && !order.carries_minimum_obligation() {
                return Err(ErrorV1::AonMaskNotApplicable);
            }
            state.honored[i] = bit;
            if bit
                && (classes[i] == EligibilityV1::Ineligible
                    || normalized.effective_quantity(i) == 0)
            {
                return Err(ErrorV1::AonMaskDishonored);
            }
            let obligated = witnessed_mask && order.carries_minimum_obligation();
            let portfolio = matches!(order, OrderV1::Portfolio(_));
            state.active[i] = classes[i] != EligibilityV1::Ineligible
                && normalized.effective_quantity(i) != 0
                && (!obligated || bit)
                && (!portfolio || bit || classes[i] == EligibilityV1::Strict);
            // A portfolio order fills whole, in whole lots, only when strict
            // (P-a); a honored minimum-fill order is firm at full size (2b).
            state.forced[i] = state.active[i] && (bit || portfolio);
        }
        i += 1;
    }
    Ok(state)
}

/// V3: derive the canonical fill vector for one `(p, c, mask)` triple.
///
/// This is the single owner of the allocation policy: both `verify` and the
/// untrusted constructors call it, so no code path can allocate differently.
pub fn derive_canonical(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    classes: &[EligibilityV1; MAX_ORDERS],
    imbalance: i128,
    mask: u64,
) -> Result<CanonicalFillsV1, ErrorV1> {
    let state = derivation_state(domain, normalized, classes, mask)?;
    let mut fills = [0u64; MAX_ORDERS];
    let mut i = 0usize;
    while i < normalized.len as usize {
        if state.forced[i] {
            fills[i] = normalized.effective_quantity(i);
        }
        i += 1;
    }

    let mut flows = FlowsV1 {
        buy: [0u64; MAX_OUTCOMES],
        sell: [0u64; MAX_OUTCOMES],
    };
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let mut demand = 0u128;
        let mut supply = 0u128;
        let mut forced_buy = 0u128;
        let mut forced_sell = 0u128;
        let mut forced_aon_buy = 0u128;
        let mut forced_aon_sell = 0u128;
        let mut strict_buy = 0u128;
        let mut strict_sell = 0u128;
        let mut j = 0usize;
        while j < normalized.len as usize {
            if state.active[j] {
                let leg = normalized.orders[j]
                    .leg_quantity(outcome as u8, normalized.effective_quantity(j))?
                    as u128;
                if leg != 0 {
                    let buy = normalized.orders[j].side() == Side::Buy;
                    if buy {
                        demand += leg;
                    } else {
                        supply += leg;
                    }
                    if state.forced[j] {
                        if buy {
                            forced_buy += leg;
                        } else {
                            forced_sell += leg;
                        }
                        if state.honored[j] {
                            if buy {
                                forced_aon_buy += leg;
                            } else {
                                forced_aon_sell += leg;
                            }
                        }
                    } else if classes[j] == EligibilityV1::Strict {
                        if buy {
                            strict_buy += leg;
                        } else {
                            strict_sell += leg;
                        }
                    }
                }
            }
            j += 1;
        }

        let supply_plus = supply as i128 + imbalance;
        let executed_buy = if (demand as i128) < supply_plus {
            demand as i128
        } else {
            supply_plus
        };
        let executed_sell = executed_buy - imbalance;
        if executed_buy < 0 || executed_sell < 0 {
            return Err(ErrorV1::InfeasibleVirtualLeg);
        }
        let executed_buy = executed_buy as u128;
        let executed_sell = executed_sell as u128;
        if executed_buy < forced_aon_buy || executed_sell < forced_aon_sell {
            return Err(ErrorV1::AonMaskDishonored);
        }
        if executed_buy < forced_buy || executed_sell < forced_sell {
            return Err(ErrorV1::StrictUnderfill);
        }
        if domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata
            && (executed_buy < forced_buy + strict_buy || executed_sell < forced_sell + strict_sell)
        {
            return Err(ErrorV1::StrictUnderfill);
        }

        allocate_single_side(
            domain,
            normalized,
            classes,
            &state,
            SideTarget {
                outcome: outcome as u8,
                side: Side::Buy,
                target: u64::try_from(executed_buy - forced_buy)
                    .map_err(|_| ErrorV1::ArithmeticOverflow)?,
            },
            &mut fills,
        )?;
        allocate_single_side(
            domain,
            normalized,
            classes,
            &state,
            SideTarget {
                outcome: outcome as u8,
                side: Side::Sell,
                target: u64::try_from(executed_sell - forced_sell)
                    .map_err(|_| ErrorV1::ArithmeticOverflow)?,
            },
            &mut fills,
        )?;

        flows.buy[outcome] =
            u64::try_from(executed_buy).map_err(|_| ErrorV1::ArithmeticOverflow)?;
        flows.sell[outcome] =
            u64::try_from(executed_sell).map_err(|_| ErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }

    // Minimum-fill and all-or-none obligations are enforced on the derived
    // vector too, so construction and verification cannot disagree.
    let mut k = 0usize;
    while k < normalized.len as usize {
        let fill = fills[k];
        if fill != 0 {
            // The all-or-none refusal is reported first: it is the more
            // specific statement about the same shortfall.
            if normalized.orders[k].partial_policy() == PartialPolicy::AllOrNone
                && fill != normalized.effective_quantity(k)
            {
                return Err(ErrorV1::AllOrNoneViolation);
            }
            if fill < normalized.effective_minimum_fill(k) {
                return Err(ErrorV1::MinimumFillViolation);
            }
        }
        k += 1;
    }
    Ok(CanonicalFillsV1 { fills, flows })
}

struct SideTarget {
    outcome: u8,
    side: Side,
    target: u64,
}

fn allocate_single_side(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    classes: &[EligibilityV1; MAX_ORDERS],
    state: &DerivationState,
    request: SideTarget,
    fills: &mut [u64; MAX_ORDERS],
) -> Result<(), ErrorV1> {
    let mut participants = [0usize; MAX_ORDERS];
    let mut count = 0usize;
    let mut i = 0usize;
    while i < normalized.len as usize {
        if state.active[i]
            && !state.forced[i]
            && normalized.orders[i].side() == request.side
            && normalized.orders[i].touches(request.outcome)
        {
            participants[count] = i;
            count += 1;
        }
        i += 1;
    }
    if request.target == 0 {
        return Ok(());
    }
    if count == 0 {
        return Err(ErrorV1::ConservationFailure);
    }
    let mut remaining = request.target;
    if domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata {
        let mut j = 0usize;
        while j < count {
            let index = participants[j];
            if classes[index] == EligibilityV1::Strict {
                let quantity = normalized.effective_quantity(index);
                if quantity > remaining {
                    return Err(ErrorV1::StrictUnderfill);
                }
                fills[index] = quantity;
                remaining -= quantity;
            }
            j += 1;
        }
        let mut marginal = [0usize; MAX_ORDERS];
        let mut marginal_count = 0usize;
        let mut k = 0usize;
        while k < count {
            if classes[participants[k]] == EligibilityV1::Marginal {
                marginal[marginal_count] = participants[k];
                marginal_count += 1;
            }
            k += 1;
        }
        pro_rata(
            domain,
            normalized,
            &marginal,
            marginal_count,
            remaining,
            fills,
        )
    } else {
        pro_rata(domain, normalized, &participants, count, remaining, fills)
    }
}

fn pro_rata(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    participants: &[usize; MAX_ORDERS],
    count: usize,
    target: u64,
    fills: &mut [u64; MAX_ORDERS],
) -> Result<(), ErrorV1> {
    if target == 0 {
        return Ok(());
    }
    let mut total = 0u128;
    let mut i = 0usize;
    while i < count {
        total += normalized.effective_quantity(participants[i]) as u128;
        i += 1;
    }
    if total < target as u128 {
        return Err(ErrorV1::ConservationFailure);
    }
    let mut remainders = [0u128; MAX_ORDERS];
    let mut assigned = [false; MAX_ORDERS];
    let mut floor_sum = 0u64;
    let mut j = 0usize;
    while j < count {
        let index = participants[j];
        let product = (normalized.effective_quantity(index) as u128) * (target as u128);
        let quotient = u64::try_from(product / total).map_err(|_| ErrorV1::ArithmeticOverflow)?;
        fills[index] = quotient;
        remainders[index] = product % total;
        floor_sum = floor_sum
            .checked_add(quotient)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        j += 1;
    }
    let mut dust = target
        .checked_sub(floor_sum)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    if dust != 0 && domain.policy.dust == DustPolicy::Reject {
        return Err(ErrorV1::DustRejected);
    }
    while dust != 0 {
        let mut selected: Option<usize> = None;
        let mut k = 0usize;
        while k < count {
            let index = participants[k];
            if !assigned[index] {
                selected = Some(match selected {
                    None => index,
                    Some(best) => {
                        if better_remainder(domain, normalized, index, best, &remainders) {
                            index
                        } else {
                            best
                        }
                    }
                });
            }
            k += 1;
        }
        let index = selected.ok_or(ErrorV1::ArithmeticOverflow)?;
        fills[index] = fills[index]
            .checked_add(1)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        assigned[index] = true;
        dust -= 1;
    }
    Ok(())
}

fn better_remainder(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    candidate: usize,
    best: usize,
    remainders: &[u128; MAX_ORDERS],
) -> bool {
    if remainders[candidate] != remainders[best] {
        return remainders[candidate] > remainders[best];
    }
    let candidate_rank = seeded_rank(normalized.orders[candidate].id(), domain.remainder_seed);
    let best_rank = seeded_rank(normalized.orders[best].id(), domain.remainder_seed);
    if candidate_rank != best_rank {
        return candidate_rank < best_rank;
    }
    normalized.orders[candidate].id() < normalized.orders[best].id()
}

/// The recomputed cash and Egg ledger of V6–V8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CashLedgerV1 {
    opening_reserved_cash: u128,
    netting_cancelled_cash: u128,
    consideration: u128,
    seller_credit: u128,
    fee_total: u128,
    fee_carry: u128,
    cash_refund: u128,
    rounding_pot: u128,
    debit_atoms: u128,
    credit_atoms: u128,
    limit_surplus: u128,
    opening_reserved_egg: [u64; MAX_OUTCOMES],
    netting_cancelled_egg: [u64; MAX_OUTCOMES],
    unfilled_refund_egg: [u64; MAX_OUTCOMES],
}

fn add(accumulator: &mut u128, value: u128) -> Result<(), ErrorV1> {
    *accumulator = accumulator
        .checked_add(value)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    Ok(())
}

fn leg_value_price_units(
    domain: &RelationDomainV1,
    order: &OrderV1,
    outcome: usize,
    units: u64,
    prices: &[u64; MAX_OUTCOMES],
) -> Result<u128, ErrorV1> {
    let leg = order.leg_quantity(outcome as u8, units)? as u128;
    let _ = domain;
    leg.checked_mul(prices[outcome] as u128)
        .ok_or(ErrorV1::ArithmeticOverflow)
}

/// V6/V7/V8: recompute every cash and Egg term, each with exactly one owner and
/// one sign convention, and close every conservation equation.
fn settle_cash(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    candidate: &CandidateV1,
    flows: &FlowsV1,
    table: &ParticipationV1,
) -> Result<CashLedgerV1, ErrorV1> {
    let scale = domain.price_scale as u128;
    let mut ledger = CashLedgerV1 {
        opening_reserved_cash: 0,
        netting_cancelled_cash: 0,
        consideration: 0,
        seller_credit: 0,
        fee_total: 0,
        fee_carry: 0,
        cash_refund: 0,
        rounding_pot: 0,
        debit_atoms: 0,
        credit_atoms: 0,
        limit_surplus: 0,
        opening_reserved_egg: [0u64; MAX_OUTCOMES],
        netting_cancelled_egg: [0u64; MAX_OUTCOMES],
        unfilled_refund_egg: [0u64; MAX_OUTCOMES],
    };
    let mut debit_units = [0u128; MAX_OWNER_SLOTS];
    let mut credit_units = [0u128; MAX_OWNER_SLOTS];
    let mut reserved_units = [0u128; MAX_OWNER_SLOTS];
    let mut fee_bps_units = [0u128; MAX_OWNER_SLOTS];
    let mut seller_filled_egg = [0u64; MAX_OUTCOMES];

    let mut i = 0usize;
    while i < normalized.len as usize {
        let order = normalized.orders[i];
        let slot = normalized.owner_slot[i] as usize;
        let fill = candidate.fills[i];
        let effective = normalized.effective_quantity(i);
        let cancelled = normalized.cancelled[i];
        let full_reservation = order.reservation_price_units(domain.price_scale)?;
        let effective_reservation = match order.side() {
            Side::Buy => scaled_reservation(&order, effective, domain.price_scale)?,
            Side::Sell => 0,
        };
        add(&mut ledger.opening_reserved_cash, full_reservation)?;
        add(
            &mut ledger.netting_cancelled_cash,
            full_reservation - effective_reservation,
        )?;
        add(&mut reserved_units[slot], effective_reservation)?;

        let mut order_value = 0u128;
        let mut outcome = 0usize;
        while outcome < domain.outcomes() {
            let reserved_leg = order.leg_quantity(outcome as u8, order.quantity())?;
            let cancelled_leg = order.leg_quantity(outcome as u8, cancelled)?;
            let filled_leg = order.leg_quantity(outcome as u8, fill)?;
            if order.side() == Side::Sell {
                ledger.opening_reserved_egg[outcome] = ledger.opening_reserved_egg[outcome]
                    .checked_add(reserved_leg)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                ledger.netting_cancelled_egg[outcome] = ledger.netting_cancelled_egg[outcome]
                    .checked_add(cancelled_leg)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                seller_filled_egg[outcome] = seller_filled_egg[outcome]
                    .checked_add(filled_leg)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
            }
            if fill != 0 {
                let value =
                    leg_value_price_units(domain, &order, outcome, fill, &candidate.prices)?;
                add(&mut order_value, value)?;
                if domain.policy.rounding == RoundingBoundaryV1::ReceiptFloor && value != 0 {
                    // R-c: one conversion per filled leg, the finest receipt
                    // granularity that exists at verification time.
                    match order.side() {
                        Side::Buy => {
                            let atoms = value.div_ceil(scale);
                            add(&mut ledger.debit_atoms, atoms)?;
                            add(&mut ledger.rounding_pot, atoms * scale - value)?;
                        }
                        Side::Sell => {
                            let atoms = value / scale;
                            add(&mut ledger.credit_atoms, atoms)?;
                            add(&mut ledger.rounding_pot, value - atoms * scale)?;
                        }
                    }
                }
            }
            outcome += 1;
        }

        if fill != 0 {
            match order.side() {
                Side::Buy => {
                    add(&mut ledger.consideration, order_value)?;
                    add(&mut debit_units[slot], order_value)?;
                    let limit = scaled_reservation(&order, fill, domain.price_scale)?;
                    if limit < order_value {
                        return Err(ErrorV1::ConsiderationMismatch);
                    }
                    add(&mut ledger.limit_surplus, limit - order_value)?;
                    match domain.policy.fee_base {
                        FeeBaseV1::None => {}
                        FeeBaseV1::FlatNotional { bps } => {
                            add(
                                &mut fee_bps_units[slot],
                                order_value
                                    .checked_mul(bps as u128)
                                    .ok_or(ErrorV1::ArithmeticOverflow)?,
                            )?;
                        }
                        // `G` is subadditive and quoted owner-level over the
                        // whole filled payoff vector, so the composite has no
                        // per-order term to accrue here: its numerator is
                        // formed once per owner at the V7 join below, from the
                        // participation table the same candidate produced.
                        FeeBaseV1::CompositeDispersionFloor { .. } => {}
                    }
                }
                Side::Sell => {
                    add(&mut ledger.seller_credit, order_value)?;
                    add(&mut credit_units[slot], order_value)?;
                    let limit = scaled_reservation(&order, fill, domain.price_scale)?;
                    if order_value < limit {
                        return Err(ErrorV1::ConsiderationMismatch);
                    }
                    add(&mut ledger.limit_surplus, order_value - limit)?;
                }
            }
        }
        i += 1;
    }

    // The per-order consideration ledger and the per-outcome flow ledger are
    // independent recomputations of the same quantity.
    let mut flow_consideration = 0u128;
    let mut flow_credit = 0u128;
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        add(
            &mut flow_consideration,
            (flows.buy[outcome] as u128) * (candidate.prices[outcome] as u128),
        )?;
        add(
            &mut flow_credit,
            (flows.sell[outcome] as u128) * (candidate.prices[outcome] as u128),
        )?;
        ledger.unfilled_refund_egg[outcome] = ledger.opening_reserved_egg[outcome]
            .checked_sub(ledger.netting_cancelled_egg[outcome])
            .and_then(|value| value.checked_sub(seller_filled_egg[outcome]))
            .ok_or(ErrorV1::ConservationFailure)?;
        if ledger.opening_reserved_egg[outcome]
            != seller_filled_egg[outcome]
                + ledger.netting_cancelled_egg[outcome]
                + ledger.unfilled_refund_egg[outcome]
        {
            return Err(ErrorV1::ConservationFailure);
        }
        if flows.sell[outcome] != seller_filled_egg[outcome] {
            return Err(ErrorV1::ConservationFailure);
        }
        let egg_out = (flows.sell[outcome] as u128) + (candidate.virtual_split as u128);
        let egg_in = (flows.buy[outcome] as u128) + (candidate.virtual_merge as u128);
        if egg_out != egg_in {
            return Err(ErrorV1::ConservationFailure);
        }
        outcome += 1;
    }
    if flow_consideration != ledger.consideration || flow_credit != ledger.seller_credit {
        return Err(ErrorV1::ConsiderationMismatch);
    }

    // V7 composite: one numerator per owner over the common denominator
    // `kappa_den * S^2 * kappa'_den`, formed from that owner's whole filled buy
    // vector.  Quoting per order instead would overcharge a netted portfolio
    // (`G` is subadditive) and hand fragmentation a discount.
    if let FeeBaseV1::CompositeDispersionFloor {
        dispersion_bps,
        floor_range_bps,
    } = domain.policy.fee_base
    {
        if dispersion_bps != 0 || floor_range_bps != 0 {
            let mut slot = 0usize;
            while slot < normalized.owner_slot_count as usize {
                fee_bps_units[slot] = composite_fee_quote(
                    &table.buy[slot],
                    &candidate.prices,
                    domain.outcomes(),
                    domain.price_scale,
                    dispersion_bps,
                    floor_range_bps,
                    0,
                )?
                .exact_numerator;
                slot += 1;
            }
        }
    }

    // V7: the payer is debited; the fee is never created from nothing, and the
    // sub-denominator remainder is carried per canonical owner identity, so
    // order fragmentation cannot reset it.
    let fee_denominator = fee_denominator_of(domain)?;
    let fee_quotient_is_atoms = fee_quotient_is_atoms(domain);
    let mut fee_quotient_total = 0u128;
    let mut slot = 0usize;
    while slot < normalized.owner_slot_count as usize {
        let quotient = fee_bps_units[slot] / fee_denominator;
        let owed = fee_owed_price_units(quotient, fee_quotient_is_atoms, scale)?;
        add(&mut ledger.fee_carry, fee_bps_units[slot] % fee_denominator)?;
        add(&mut fee_quotient_total, quotient)?;
        add(&mut ledger.fee_total, owed)?;
        add(&mut debit_units[slot], owed)?;
        if debit_units[slot] > reserved_units[slot] {
            return Err(ErrorV1::FeePayerUnfunded);
        }
        add(
            &mut ledger.cash_refund,
            reserved_units[slot] - debit_units[slot],
        )?;
        slot += 1;
    }
    if fee_quotient_total
        .checked_mul(fee_denominator)
        .and_then(|value| value.checked_add(ledger.fee_carry))
        .ok_or(ErrorV1::ArithmeticOverflow)?
        != fee_total_bps_units(&fee_bps_units, normalized.owner_slot_count as usize)?
    {
        return Err(ErrorV1::FeeMismatch);
    }

    // V6: the one named rounding boundary.  Debits round up and credits round
    // down, so every remainder atom lands in one non-negative rounding pot and
    // no term can draw from anywhere else.
    match domain.policy.rounding {
        RoundingBoundaryV1::ReceiptFloor => {
            // Leg conversions already happened; the fee still converts per owner.
            let mut slot = 0usize;
            while slot < normalized.owner_slot_count as usize {
                let fee_units = fee_owed_price_units(
                    fee_bps_units[slot] / fee_denominator,
                    fee_quotient_is_atoms,
                    scale,
                )?;
                if fee_units != 0 {
                    let atoms = fee_units.div_ceil(scale);
                    add(&mut ledger.debit_atoms, atoms)?;
                    add(&mut ledger.rounding_pot, atoms * scale - fee_units)?;
                }
                slot += 1;
            }
        }
        RoundingBoundaryV1::TerminalOwnerFloor | RoundingBoundaryV1::None => {
            let mut slot = 0usize;
            while slot < normalized.owner_slot_count as usize {
                if debit_units[slot] != 0 {
                    let atoms = debit_units[slot].div_ceil(scale);
                    add(&mut ledger.debit_atoms, atoms)?;
                    add(&mut ledger.rounding_pot, atoms * scale - debit_units[slot])?;
                }
                if credit_units[slot] != 0 {
                    let atoms = credit_units[slot] / scale;
                    add(&mut ledger.credit_atoms, atoms)?;
                    add(&mut ledger.rounding_pot, credit_units[slot] - atoms * scale)?;
                }
                slot += 1;
            }
        }
    }
    if domain.policy.rounding == RoundingBoundaryV1::None && ledger.rounding_pot != 0 {
        return Err(ErrorV1::RemainderRequired);
    }

    // V8: closure.  Every atom is named on exactly one side of one equation.
    let split_cost = (candidate.virtual_split as u128)
        .checked_mul(scale)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    let merge_proceeds = (candidate.virtual_merge as u128)
        .checked_mul(scale)
        .ok_or(ErrorV1::ArithmeticOverflow)?;
    if ledger.consideration + merge_proceeds != ledger.seller_credit + split_cost {
        return Err(ErrorV1::ConservationFailure);
    }
    if ledger.opening_reserved_cash
        != ledger.consideration
            + ledger.fee_total
            + ledger.cash_refund
            + ledger.netting_cancelled_cash
    {
        return Err(ErrorV1::ConservationFailure);
    }
    Ok(ledger)
}

/// The denominator every accumulated fee numerator is read against.
///
/// `FlatNotional` accrues price units scaled by [`FEE_BPS_DENOMINATOR`]; the
/// composite accrues collateral atoms scaled by `kappa_den * S^2 * kappa'_den`.
/// Both bases therefore share one join, one carry, and one identity check —
/// only this divisor and [`fee_quotient_is_atoms`] differ.
pub(crate) fn fee_denominator_of(domain: &RelationDomainV1) -> Result<u128, ErrorV1> {
    match domain.policy.fee_base {
        FeeBaseV1::None | FeeBaseV1::FlatNotional { .. } => Ok(FEE_BPS_DENOMINATOR as u128),
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } => {
            if dispersion_bps == 0 && floor_range_bps == 0 {
                // No numerator is ever accrued at zero rates, so the divisor is
                // only ever applied to zero.  Keeping the basis-point value
                // holds the zero-rate composite bit-identical to `None`.
                return Ok(FEE_BPS_DENOMINATOR as u128);
            }
            let scale = domain.price_scale as u128;
            (FEE_BPS_DENOMINATOR as u128)
                .checked_mul(scale)
                .and_then(|value| value.checked_mul(scale))
                .and_then(|value| value.checked_mul(FEE_BPS_DENOMINATOR as u128))
                .ok_or(ErrorV1::ArithmeticOverflow)
        }
    }
}

/// Whether the fee quotient is collateral atoms (composite) rather than price
/// units (flat notional).
pub(crate) fn fee_quotient_is_atoms(domain: &RelationDomainV1) -> bool {
    match domain.policy.fee_base {
        FeeBaseV1::None | FeeBaseV1::FlatNotional { .. } => false,
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } => dispersion_bps != 0 || floor_range_bps != 0,
    }
}

/// One owner's fee in price units.
///
/// The composite quotient is atoms — `G_num` already carries the `S^2` its
/// denominator divides out — so it converts back to the ledger's price units by
/// one exact multiplication.  Nothing rounds here: the only rounding the
/// composite does is the single floor that produced `quotient`, and its
/// remainder is already in the carry.
pub(crate) fn fee_owed_price_units(
    quotient: u128,
    quotient_is_atoms: bool,
    scale: u128,
) -> Result<u128, ErrorV1> {
    if quotient_is_atoms {
        quotient
            .checked_mul(scale)
            .ok_or(ErrorV1::ArithmeticOverflow)
    } else {
        Ok(quotient)
    }
}

fn fee_total_bps_units(
    fee_bps_units: &[u128; MAX_OWNER_SLOTS],
    owners: usize,
) -> Result<u128, ErrorV1> {
    let mut total = 0u128;
    let mut slot = 0usize;
    while slot < owners {
        add(&mut total, fee_bps_units[slot])?;
        slot += 1;
    }
    Ok(total)
}

pub(crate) fn scaled_reservation(
    order: &OrderV1,
    units: u64,
    price_scale: u64,
) -> Result<u128, ErrorV1> {
    match order {
        OrderV1::SingleEgg(o) => (units as u128)
            .checked_mul(o.limit_price as u128)
            .ok_or(ErrorV1::ArithmeticOverflow),
        OrderV1::Portfolio(o) => (units as u128)
            .checked_mul(o.limit_collateral_per_lot as u128)
            .and_then(|value| value.checked_mul(price_scale as u128))
            .ok_or(ErrorV1::ArithmeticOverflow),
    }
}

fn mix(state: &mut u64, value: u64) {
    // A fixed SplitMix-style permutation.  This is a deterministic host-model
    // identity, never a cryptographic commitment.
    let mut x = *state ^ value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    *state = (x ^ (x >> 31)).rotate_left(29).wrapping_add(*state);
}

/// The streaming accumulator behind [`candidate_digest`].
///
/// The digest has exactly one owner: both the batch verifier and the streaming
/// verifier drive this fold, in the same feed sequence, so no code path can
/// fold a candidate identity differently.  It is a deterministic host-model
/// identity, never a cryptographic commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DigestFoldV1 {
    high: u64,
    low: u64,
}

impl DigestFoldV1 {
    /// The frozen initialization vector.
    pub(crate) const NEW: Self = Self {
        high: 0x243F_6A88_85A3_08D3,
        low: 0x1319_8A2E_0370_7344,
    };

    /// Fold one value.
    pub(crate) fn feed(&mut self, value: u64) {
        mix(&mut self.high, value);
        mix(&mut self.low, value.rotate_left(32) ^ self.high);
    }

    /// Fold the frozen domain and the candidate head (everything the digest
    /// binds before the fill vector): domain identity, policy code, order
    /// length, prices, and the virtual pair.
    pub(crate) fn feed_head(
        &mut self,
        domain: &RelationDomainV1,
        order_len: u8,
        prices: &[u64; MAX_OUTCOMES],
        virtual_split: u64,
        virtual_merge: u64,
    ) {
        self.feed(domain.relation_version as u64);
        self.feed(domain.market_id);
        self.feed(domain.book_id);
        self.feed(domain.epoch);
        self.feed(domain.policy_id);
        self.feed(domain.order_set_id);
        self.feed(domain.outcome_count as u64);
        self.feed(domain.owner_count as u64);
        self.feed(domain.price_scale);
        self.feed(domain.remainder_seed);
        self.feed(domain.policy.code());
        self.feed(order_len as u64);
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            self.feed(prices[i]);
            i += 1;
        }
        self.feed(virtual_split);
        self.feed(virtual_merge);
    }

    /// Fold one pairing slice.
    pub(crate) fn feed_slice(&mut self, slice: &PairingSliceV1) {
        self.feed(leg_ref_code(slice.buy_ref));
        self.feed(leg_ref_code(slice.sell_ref));
        self.feed(slice.outcome as u64);
        self.feed(slice.quantity);
    }

    /// The folded identity.
    pub(crate) fn digest(&self) -> u128 {
        ((self.high as u128) << 64) | (self.low as u128)
    }

    /// The two fold words, `(high, low)`, for the checkpoint codec's encode
    /// side (`relation_v1_stream`, design §7).
    pub(crate) const fn words(self) -> (u64, u64) {
        (self.high, self.low)
    }

    /// Rebuild a fold from its two words, for the codec's decode side.  Every
    /// `(high, low)` pair is a reachable mid-stream fold state, so there is
    /// deliberately nothing to validate here.
    pub(crate) const fn from_words(high: u64, low: u64) -> Self {
        Self { high, low }
    }
}

/// The canonical candidate digest: a deterministic, non-cryptographic identity
/// over the frozen domain, the free coordinates, the fills, and — under the
/// explicit-slice variant — the submitted decomposition.
pub fn candidate_digest(
    domain: &RelationDomainV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> u128 {
    let mut fold = DigestFoldV1::NEW;
    fold.feed_head(
        domain,
        candidate.order_len,
        &candidate.prices,
        candidate.virtual_split,
        candidate.virtual_merge,
    );
    let mut j = 0usize;
    while j < MAX_ORDERS {
        fold.feed(candidate.fills[j]);
        j += 1;
    }
    fold.feed(candidate.honored_aon_mask);
    if let Some(witness) = pairing {
        fold.feed(witness.len as u64);
        let mut k = 0usize;
        while k < witness.len as usize {
            fold.feed_slice(&witness.slices[k]);
            k += 1;
        }
    }
    fold.digest()
}

fn leg_ref_code(leg: LegRefV1) -> u64 {
    match leg {
        LegRefV1::Order(index) => index as u64,
        LegRefV1::Split => 1 << 32,
        LegRefV1::Merge => 1 << 33,
    }
}

fn score_of(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    candidate: &CandidateV1,
    flows: &FlowsV1,
    table: &ParticipationV1,
) -> Result<(ScoreV1, u16, u64), ErrorV1> {
    let scale = domain.price_scale as i128;
    let mut weighted = 0i128;
    let mut overlap_total = 0u64;
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let total_flow = flows.buy[outcome]
            .checked_add(candidate.virtual_merge)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        let direct = total_flow
            .checked_sub(candidate.virtual_split)
            .and_then(|value| value.checked_sub(candidate.virtual_merge))
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        let mut overlap = 0u64;
        let mut slot = 0usize;
        while slot < normalized.owner_slot_count as usize {
            let buy = table.buy[slot][outcome];
            let sell = table.sell[slot][outcome];
            overlap = overlap
                .checked_add(if buy < sell { buy } else { sell })
                .ok_or(ErrorV1::ArithmeticOverflow)?;
            slot += 1;
        }
        overlap_total = overlap_total
            .checked_add(overlap)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        let price = candidate.prices[outcome] as i128;
        let weight = price * (scale - price);
        weighted += weight * (direct as i128 - overlap as i128);
        outcome += 1;
    }
    let mut owners = 0u16;
    let mut slot = 0usize;
    while slot < normalized.owner_slot_count as usize {
        let mut participates = false;
        let mut outcome = 0usize;
        while outcome < domain.outcomes() {
            if table.buy[slot][outcome] != 0 || table.sell[slot][outcome] != 0 {
                participates = true;
            }
            outcome += 1;
        }
        if participates {
            owners += 1;
        }
        slot += 1;
    }
    Ok((
        ScoreV1 {
            weighted_direct_volume: weighted,
            limit_surplus_price_units: 0,
            distinct_owners: owners,
            churn: candidate
                .virtual_split
                .checked_add(candidate.virtual_merge)
                .ok_or(ErrorV1::ArithmeticOverflow)?,
            digest: 0,
        },
        owners,
        overlap_total,
    ))
}

fn validate_witness_fills(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    classes: &[EligibilityV1; MAX_ORDERS],
    candidate: &CandidateV1,
) -> Result<(), ErrorV1> {
    let witnessed_mask = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
    if !witnessed_mask && candidate.honored_aon_mask != 0 {
        return Err(ErrorV1::AonMaskNotApplicable);
    }
    let mut i = normalized.len as usize;
    while i < MAX_ORDERS {
        if candidate.fills[i] != 0 {
            return Err(ErrorV1::NonCanonicalPadding);
        }
        if mask_bit(candidate.honored_aon_mask, i) {
            return Err(ErrorV1::AonMaskNotApplicable);
        }
        i += 1;
    }
    let mut j = 0usize;
    while j < normalized.len as usize {
        let order = normalized.orders[j];
        let fill = candidate.fills[j];
        let effective = normalized.effective_quantity(j);
        if fill > effective {
            return Err(ErrorV1::FillExceedsQuantity);
        }
        if fill != 0 && classes[j] == EligibilityV1::Ineligible {
            return Err(ErrorV1::IneligibleFill);
        }
        if witnessed_mask {
            let honored = mask_bit(candidate.honored_aon_mask, j);
            if honored && !order.carries_minimum_obligation() {
                return Err(ErrorV1::AonMaskNotApplicable);
            }
            if honored && (classes[j] == EligibilityV1::Ineligible || fill != effective) {
                return Err(ErrorV1::AonMaskDishonored);
            }
            if !honored && order.carries_minimum_obligation() && fill != 0 {
                return Err(ErrorV1::AonMaskLeak);
            }
        }
        if order.partial_policy() == PartialPolicy::AllOrNone && fill != 0 && fill != effective {
            return Err(ErrorV1::AllOrNoneViolation);
        }
        if fill != 0 && fill < normalized.effective_minimum_fill(j) {
            return Err(ErrorV1::MinimumFillViolation);
        }
        j += 1;
    }
    Ok(())
}

fn check_conservation_identity(
    domain: &RelationDomainV1,
    flows: &FlowsV1,
    candidate: &CandidateV1,
) -> Result<(), ErrorV1> {
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let left = (flows.buy[outcome] as u128)
            .checked_add(candidate.virtual_merge as u128)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        let right = (flows.sell[outcome] as u128)
            .checked_add(candidate.virtual_split as u128)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        if left != right {
            return Err(ErrorV1::OutcomeConservationMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

/// Verify a candidate against the frozen domain and the frozen book.
///
/// Success means exactly this: the candidate is a **valid submitted candidate**
/// whose fills admit a complete executable pairing under the bound
/// owner/outcome/side policy, whose every claimed aggregate was recomputed from
/// the whole frozen book, and whose every asset equation closes.  It does not
/// mean the candidate is optimal, and it is not a verified claim in any
/// proof-assistant sense.
///
/// `pairing` must be `Some` exactly when the frozen policy names the
/// explicit-slice fallback.
pub fn verify(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<SummaryV1, ErrorV1> {
    verify_inner(domain, book, candidate, pairing, true)
}

/// Run every stage of [`verify`] except the V9 comparison of the candidate's
/// *claimed* score and digest against the recomputed ones.
///
/// This exists for constructors, which must derive a candidate before they can
/// know its score, and for falsifiers that must observe the structural refusal
/// a stale digest would otherwise mask.  It is **not** an acceptance entry
/// point: an authoritative verifier must call [`verify`], because a candidate
/// that never binds its own aggregates is not a candidate.
pub fn verify_ignoring_claimed_aggregates(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<SummaryV1, ErrorV1> {
    verify_inner(domain, book, candidate, pairing, false)
}

/// Check that a submitted decomposition is a complete executable pairing of the
/// candidate's fills: every slice moves one outcome's Egg between two distinct
/// bound owners (or a virtual node), and the slices sum exactly to the fills and
/// to `sigma`/`mu`.
pub fn verify_pairing_witness(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    witness: &PairingWitnessV1,
) -> Result<(), ErrorV1> {
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    if candidate.order_len != normalized.len {
        return Err(ErrorV1::CandidateMismatch);
    }
    check_explicit_slices(domain, &normalized, candidate, witness)
}

fn verify_inner(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
    check_claims: bool,
) -> Result<SummaryV1, ErrorV1> {
    // V0
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    if candidate.order_len != normalized.len {
        return Err(ErrorV1::CandidateMismatch);
    }
    // V1
    validate_prices(domain, &candidate.prices)?;
    // V2
    let classes = classify_all(domain, &normalized, &candidate.prices)?;
    validate_witness_fills(domain, &normalized, &classes, candidate)?;
    // V4 (see the module's documented refusal-precedence deviation)
    if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
        return Err(ErrorV1::ChurnNotCanonical);
    }
    let flows = flows_from_fills(domain, &normalized, &candidate.fills)?;
    check_conservation_identity(domain, &flows, candidate)?;
    // V3
    let canonical = derive_canonical(
        domain,
        &normalized,
        &classes,
        candidate.imbalance(),
        candidate.honored_aon_mask,
    )?;
    if canonical.fills != candidate.fills {
        return Err(ErrorV1::CandidateMismatch);
    }
    if canonical.flows != flows {
        return Err(ErrorV1::OutcomeConservationMismatch);
    }
    // V5
    let mut table = ParticipationV1::zeroed();
    participation_from_fills(domain, &normalized, &candidate.fills, &mut table)?;
    check_pairing_feasibility(domain, &normalized, &table, &flows, candidate.virtual_merge)?;
    match (domain.policy.pairing_witness, pairing) {
        (PairingWitnessPolicyV1::RecomputedConstructor, None) => {}
        (PairingWitnessPolicyV1::RecomputedConstructor, Some(_)) => {
            return Err(ErrorV1::PairingWitnessNotAdmitted)
        }
        (PairingWitnessPolicyV1::ExplicitSlices, None) => {
            return Err(ErrorV1::PairingWitnessMissing)
        }
        (PairingWitnessPolicyV1::ExplicitSlices, Some(witness)) => {
            check_explicit_slices(domain, &normalized, candidate, witness)?;
        }
    }
    // V6, V7, V8
    let ledger = settle_cash(domain, &normalized, candidate, &flows, &table)?;
    // V9
    let (mut score, owners, overlap) = score_of(domain, &normalized, candidate, &flows, &table)?;
    score.limit_surplus_price_units = ledger.limit_surplus;
    let digest = candidate_digest(domain, candidate, pairing);
    score.digest = digest;
    if check_claims {
        if candidate.claimed_score != score {
            return Err(ErrorV1::ScoreMismatch);
        }
        if candidate.canonical_candidate_digest != digest {
            return Err(ErrorV1::DigestMismatch);
        }
    }
    let mut summary = SummaryV1 {
        outcome_count: domain.outcome_count,
        buy_flow: flows.buy,
        sell_flow: flows.sell,
        total_flow: [0u64; MAX_OUTCOMES],
        direct_flow: [0u64; MAX_OUTCOMES],
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        opening_reserved_egg: ledger.opening_reserved_egg,
        unfilled_refund_egg: ledger.unfilled_refund_egg,
        netting_cancelled_egg: ledger.netting_cancelled_egg,
        opening_reserved_cash_price_units: ledger.opening_reserved_cash,
        buyer_consideration_price_units: ledger.consideration,
        seller_credit_price_units: ledger.seller_credit,
        split_cost_price_units: (candidate.virtual_split as u128) * (domain.price_scale as u128),
        merge_proceeds_price_units: (candidate.virtual_merge as u128)
            * (domain.price_scale as u128),
        fee_price_units: ledger.fee_total,
        fee_carry_bps_units: ledger.fee_carry,
        cash_refund_price_units: ledger.cash_refund,
        rounding_pot_price_units: ledger.rounding_pot,
        debit_atoms: ledger.debit_atoms,
        credit_atoms: ledger.credit_atoms,
        distinct_participating_owners: owners,
        self_overlap_volume: overlap,
        score,
        candidate_digest: digest,
    };
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        summary.total_flow[outcome] = flows.buy[outcome]
            .checked_add(candidate.virtual_merge)
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        summary.direct_flow[outcome] = summary.total_flow[outcome]
            .checked_sub(candidate.virtual_split)
            .and_then(|value| value.checked_sub(candidate.virtual_merge))
            .ok_or(ErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    Ok(summary)
}

/// Build the canonical candidate at `(p, c, mask)` and round-trip it through
/// [`verify`].  This is a constructor, never an optimality oracle: it answers
/// "what is the canonical candidate at these coordinates", not "which
/// coordinates are best".
pub fn canonical_candidate(
    domain: &RelationDomainV1,
    book: &BookV1,
    prices: &[u64; MAX_OUTCOMES],
    imbalance: i64,
    mask: u64,
) -> Result<CandidateV1, ErrorV1> {
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    validate_prices(domain, prices)?;
    let classes = classify_all(domain, &normalized, prices)?;
    let canonical = derive_canonical(domain, &normalized, &classes, imbalance as i128, mask)?;
    let mut candidate = CandidateV1 {
        order_len: normalized.len,
        prices: *prices,
        virtual_split: if imbalance > 0 { imbalance as u64 } else { 0 },
        virtual_merge: if imbalance < 0 {
            imbalance.unsigned_abs()
        } else {
            0
        },
        fills: canonical.fills,
        honored_aon_mask: mask,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
    };
    let pairing = match domain.policy.pairing_witness {
        PairingWitnessPolicyV1::RecomputedConstructor => None,
        PairingWitnessPolicyV1::ExplicitSlices => {
            // The feasibility gate owns the refusal for books with no complete
            // executable pairing; the constructor only ever runs on fills that
            // already passed it, so both variants report the same refusal.
            let flows = flows_from_fills(domain, &normalized, &canonical.fills)?;
            let mut table = ParticipationV1::zeroed();
            participation_from_fills(domain, &normalized, &canonical.fills, &mut table)?;
            check_pairing_feasibility(
                domain,
                &normalized,
                &table,
                &flows,
                candidate.virtual_merge,
            )?;
            Some(canonical_pairing(domain, book, &candidate)?)
        }
    };
    let summary = verify_inner(domain, book, &candidate, pairing.as_ref(), false)?;
    candidate.claimed_score = summary.score;
    candidate.canonical_candidate_digest = summary.candidate_digest;
    verify(domain, book, &candidate, pairing.as_ref())?;
    Ok(candidate)
}

struct OutcomeStateV1 {
    order_index: [u8; MAX_ORDERS],
    slot: [u16; MAX_ORDERS],
    side: [Side; MAX_ORDERS],
    remaining: [u64; MAX_ORDERS],
    rank: [u64; MAX_ORDERS],
    id: [u64; MAX_ORDERS],
    count: usize,
    buy_remaining: [u64; MAX_OWNER_SLOTS],
    sell_remaining: [u64; MAX_OWNER_SLOTS],
    slots: usize,
}

impl OutcomeStateV1 {
    fn participation(&self, slot: usize) -> u64 {
        self.buy_remaining[slot] + self.sell_remaining[slot]
    }

    fn side_remaining(&self, slot: usize, side: Side) -> u64 {
        match side {
            Side::Buy => self.buy_remaining[slot],
            Side::Sell => self.sell_remaining[slot],
        }
    }

    /// The remaining leg of `slot` on `side` with the lowest seeded rank, then
    /// the lowest canonical identifier.
    fn pick_leg(&self, slot: usize, side: Side) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut i = 0usize;
        while i < self.count {
            if self.remaining[i] != 0 && self.side[i] == side && self.slot[i] as usize == slot {
                best = Some(match best {
                    None => i,
                    Some(current) => {
                        if (self.rank[i], self.id[i]) < (self.rank[current], self.id[current]) {
                            i
                        } else {
                            current
                        }
                    }
                });
            }
            i += 1;
        }
        best
    }

    /// The owner with the largest remaining participation that still has a leg
    /// on `side`, excluding `forbidden`.
    fn pick_owner(&self, side: Side, forbidden: Option<usize>) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut slot = 0usize;
        while slot < self.slots {
            if Some(slot) != forbidden && self.side_remaining(slot, side) != 0 {
                best = Some(match best {
                    None => slot,
                    Some(current) => {
                        let challenger = self.participation(slot);
                        let incumbent = self.participation(current);
                        if challenger > incumbent {
                            slot
                        } else if challenger < incumbent {
                            current
                        } else {
                            let a = self.pick_leg(slot, side);
                            let b = self.pick_leg(current, side);
                            match (a, b) {
                                (Some(a), Some(b)) => {
                                    if (self.rank[a], self.id[a]) < (self.rank[b], self.id[b]) {
                                        slot
                                    } else {
                                        current
                                    }
                                }
                                _ => current,
                            }
                        }
                    }
                });
            }
            slot += 1;
        }
        best
    }

    fn max_participation_excluding(&self, first: usize, second: Option<usize>) -> u64 {
        let mut maximum = 0u64;
        let mut slot = 0usize;
        while slot < self.slots {
            if slot != first && Some(slot) != second {
                let value = self.participation(slot);
                if value > maximum {
                    maximum = value;
                }
            }
            slot += 1;
        }
        maximum
    }
}

/// The canonical pairing constructor (§8.4).
///
/// Deterministic, price independent, and run once at candidate finalization —
/// never per submitted candidate.  It refuses rather than emitting a slice that
/// would strand residue: the design's "floored at 1" slack is replaced by
/// [`ErrorV1::ConstructorStalled`], which the bounded exhaustive oracle in this
/// crate's tests never reaches when the feasibility inequality holds.
pub fn canonical_pairing(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
) -> Result<PairingWitnessV1, ErrorV1> {
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    if candidate.order_len != normalized.len {
        return Err(ErrorV1::CandidateMismatch);
    }
    if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
        return Err(ErrorV1::ChurnNotCanonical);
    }
    let mut i = normalized.len as usize;
    while i < MAX_ORDERS {
        if candidate.fills[i] != 0 {
            return Err(ErrorV1::NonCanonicalPadding);
        }
        i += 1;
    }
    let mut j = 0usize;
    while j < normalized.len as usize {
        if candidate.fills[j] > normalized.effective_quantity(j) {
            return Err(ErrorV1::FillExceedsQuantity);
        }
        j += 1;
    }
    let flows = flows_from_fills(domain, &normalized, &candidate.fills)?;
    check_conservation_identity(domain, &flows, candidate)?;

    let mut witness = PairingWitnessV1::empty();
    let mut emitted = 0usize;
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        let mut state = OutcomeStateV1 {
            order_index: [0u8; MAX_ORDERS],
            slot: [0u16; MAX_ORDERS],
            side: [Side::Buy; MAX_ORDERS],
            remaining: [0u64; MAX_ORDERS],
            rank: [0u64; MAX_ORDERS],
            id: [0u64; MAX_ORDERS],
            count: 0,
            buy_remaining: [0u64; MAX_OWNER_SLOTS],
            sell_remaining: [0u64; MAX_OWNER_SLOTS],
            slots: normalized.owner_slot_count as usize,
        };
        let mut k = 0usize;
        while k < normalized.len as usize {
            let leg = normalized.orders[k].leg_quantity(outcome as u8, candidate.fills[k])?;
            if leg != 0 {
                let slot = normalized.owner_slot[k] as usize;
                state.order_index[state.count] = k as u8;
                state.slot[state.count] = slot as u16;
                state.side[state.count] = normalized.orders[k].side();
                state.remaining[state.count] = leg;
                state.id[state.count] = normalized.orders[k].id();
                state.rank[state.count] =
                    seeded_rank(normalized.orders[k].id(), domain.remainder_seed);
                state.count += 1;
                match normalized.orders[k].side() {
                    Side::Buy => {
                        state.buy_remaining[slot] = state.buy_remaining[slot]
                            .checked_add(leg)
                            .ok_or(ErrorV1::ArithmeticOverflow)?
                    }
                    Side::Sell => {
                        state.sell_remaining[slot] = state.sell_remaining[slot]
                            .checked_add(leg)
                            .ok_or(ErrorV1::ArithmeticOverflow)?
                    }
                }
            }
            k += 1;
        }
        let mut split_remaining = candidate.virtual_split;
        let mut merge_remaining = candidate.virtual_merge;
        let mut flow_remaining = flows.buy[outcome]
            .checked_add(candidate.virtual_merge)
            .ok_or(ErrorV1::ArithmeticOverflow)?;

        while flow_remaining != 0 {
            let buy_owner = state.pick_owner(Side::Buy, None);
            let sell_owner = state.pick_owner(Side::Sell, None);
            let side = match (buy_owner, sell_owner) {
                (None, None) => return Err(ErrorV1::ConstructorStalled),
                (Some(_), None) => Side::Buy,
                (None, Some(_)) => Side::Sell,
                (Some(buy), Some(sell)) => {
                    if state.participation(buy) >= state.participation(sell) {
                        Side::Buy
                    } else {
                        Side::Sell
                    }
                }
            };
            let chosen_slot = match side {
                Side::Buy => buy_owner,
                Side::Sell => sell_owner,
            }
            .ok_or(ErrorV1::ConstructorStalled)?;
            let chosen_leg = state
                .pick_leg(chosen_slot, side)
                .ok_or(ErrorV1::ConstructorStalled)?;
            let opposite = match side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            };
            let virtual_capacity = match side {
                Side::Buy => split_remaining,
                Side::Sell => merge_remaining,
            };
            let counterparty_slot = state.pick_owner(opposite, Some(chosen_slot));
            let use_virtual = match counterparty_slot {
                None => virtual_capacity != 0,
                Some(slot) => state.participation(slot) < virtual_capacity,
            };
            let (counterparty_leg, counterparty_remaining, counterparty_owner) = if use_virtual {
                (None, virtual_capacity, None)
            } else {
                let slot = counterparty_slot.ok_or(ErrorV1::ConstructorStalled)?;
                let leg = state
                    .pick_leg(slot, opposite)
                    .ok_or(ErrorV1::ConstructorStalled)?;
                (Some(leg), state.remaining[leg], Some(slot))
            };
            if counterparty_remaining == 0 {
                return Err(ErrorV1::ConstructorStalled);
            }
            let blocking = state.max_participation_excluding(chosen_slot, counterparty_owner);
            let slack = flow_remaining
                .checked_sub(blocking)
                .ok_or(ErrorV1::ConstructorStalled)?;
            if slack == 0 {
                return Err(ErrorV1::ConstructorStalled);
            }
            let mut quantity = state.remaining[chosen_leg];
            if counterparty_remaining < quantity {
                quantity = counterparty_remaining;
            }
            if slack < quantity {
                quantity = slack;
            }
            if quantity == 0 {
                return Err(ErrorV1::ConstructorStalled);
            }

            let chosen_ref = LegRefV1::Order(state.order_index[chosen_leg]);
            let counter_ref = match counterparty_leg {
                Some(leg) => LegRefV1::Order(state.order_index[leg]),
                None => match side {
                    Side::Buy => LegRefV1::Split,
                    Side::Sell => LegRefV1::Merge,
                },
            };
            let slice = match side {
                Side::Buy => PairingSliceV1 {
                    buy_ref: chosen_ref,
                    sell_ref: counter_ref,
                    outcome: outcome as u8,
                    quantity,
                },
                Side::Sell => PairingSliceV1 {
                    buy_ref: counter_ref,
                    sell_ref: chosen_ref,
                    outcome: outcome as u8,
                    quantity,
                },
            };
            if emitted >= MAX_SLICES {
                return Err(ErrorV1::SliceCapacityExceeded);
            }
            witness.slices[emitted] = slice;
            emitted += 1;

            state.remaining[chosen_leg] -= quantity;
            match side {
                Side::Buy => state.buy_remaining[chosen_slot] -= quantity,
                Side::Sell => state.sell_remaining[chosen_slot] -= quantity,
            }
            match (counterparty_leg, counterparty_owner) {
                (Some(leg), Some(slot)) => {
                    state.remaining[leg] -= quantity;
                    match opposite {
                        Side::Buy => state.buy_remaining[slot] -= quantity,
                        Side::Sell => state.sell_remaining[slot] -= quantity,
                    }
                }
                _ => match side {
                    Side::Buy => split_remaining -= quantity,
                    Side::Sell => merge_remaining -= quantity,
                },
            }
            flow_remaining -= quantity;
        }
        if split_remaining != 0 || merge_remaining != 0 {
            return Err(ErrorV1::ConstructorStalled);
        }
        let mut leg = 0usize;
        while leg < state.count {
            if state.remaining[leg] != 0 {
                return Err(ErrorV1::ConstructorStalled);
            }
            leg += 1;
        }
        outcome += 1;
    }
    witness.len = emitted as u16;
    Ok(witness)
}

/// The explicit-slice fallback check (§8.5): every slice must be an executable
/// transfer, and the slices must sum exactly to the fills and to `sigma`/`mu`.
fn check_explicit_slices(
    domain: &RelationDomainV1,
    normalized: &NormalizedBookV1,
    candidate: &CandidateV1,
    witness: &PairingWitnessV1,
) -> Result<(), ErrorV1> {
    if witness.len as usize > MAX_SLICES {
        return Err(ErrorV1::SliceSumMismatch);
    }
    let empty = PairingWitnessV1::empty().slices[0];
    let mut i = witness.len as usize;
    while i < MAX_SLICES {
        if witness.slices[i] != empty {
            return Err(ErrorV1::NonCanonicalPadding);
        }
        i += 1;
    }
    let mut covered = [[0u64; MAX_OUTCOMES]; MAX_ORDERS];
    let mut split_used = [0u64; MAX_OUTCOMES];
    let mut merge_used = [0u64; MAX_OUTCOMES];
    let mut k = 0usize;
    while k < witness.len as usize {
        let slice = witness.slices[k];
        if slice.quantity == 0 || slice.outcome as usize >= domain.outcomes() {
            return Err(ErrorV1::SliceNotExecutable);
        }
        let outcome = slice.outcome as usize;
        let buy_owner = match slice.buy_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= normalized.len as usize
                    || normalized.orders[index].side() != Side::Buy
                    || !normalized.orders[index].touches(slice.outcome)
                {
                    return Err(ErrorV1::SliceNotExecutable);
                }
                covered[index][outcome] = covered[index][outcome]
                    .checked_add(slice.quantity)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                Some(normalized.owner_slot[index])
            }
            LegRefV1::Merge => {
                merge_used[outcome] = merge_used[outcome]
                    .checked_add(slice.quantity)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                None
            }
            LegRefV1::Split => return Err(ErrorV1::SliceNotExecutable),
        };
        let sell_owner = match slice.sell_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= normalized.len as usize
                    || normalized.orders[index].side() != Side::Sell
                    || !normalized.orders[index].touches(slice.outcome)
                {
                    return Err(ErrorV1::SliceNotExecutable);
                }
                covered[index][outcome] = covered[index][outcome]
                    .checked_add(slice.quantity)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                Some(normalized.owner_slot[index])
            }
            LegRefV1::Split => {
                split_used[outcome] = split_used[outcome]
                    .checked_add(slice.quantity)
                    .ok_or(ErrorV1::ArithmeticOverflow)?;
                None
            }
            LegRefV1::Merge => return Err(ErrorV1::SliceNotExecutable),
        };
        match (buy_owner, sell_owner) {
            (None, None) => return Err(ErrorV1::SliceNotExecutable),
            (Some(buy), Some(sell)) if buy == sell => return Err(ErrorV1::SliceNotExecutable),
            _ => {}
        }
        k += 1;
    }
    let mut index = 0usize;
    while index < normalized.len as usize {
        let mut outcome = 0usize;
        while outcome < domain.outcomes() {
            let leg =
                normalized.orders[index].leg_quantity(outcome as u8, candidate.fills[index])?;
            if covered[index][outcome] != leg {
                return Err(ErrorV1::SliceSumMismatch);
            }
            outcome += 1;
        }
        index += 1;
    }
    let mut outcome = 0usize;
    while outcome < domain.outcomes() {
        if split_used[outcome] != candidate.virtual_split
            || merge_used[outcome] != candidate.virtual_merge
        {
            return Err(ErrorV1::SliceSumMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

/// Explicit bounds for the untrusted constructor search.  The search is not
/// part of the relation: it proposes coordinates, and every proposal is
/// round-tripped through [`verify`] before it can be compared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchBoundsV1 {
    /// Prices are enumerated on multiples of this step; it must divide the
    /// domain's price scale.
    pub price_step: u64,
    /// The search visits `-max_imbalance ..= max_imbalance` complete sets.
    pub max_imbalance: i64,
    /// Hard budget on visited coordinate triples.
    pub max_visits: u32,
}

/// Search the bounded coordinate box for the **best valid submitted candidate**.
///
/// This is a non-authoritative constructor.  It makes no optimality claim: it
/// returns the best candidate *it submitted and verified* inside the bounds it
/// was given, and a book with no valid candidate in that box returns
/// [`ErrorV1::NoValidCandidate`] (the epoch-lapse case).
pub fn propose_best_valid(
    domain: &RelationDomainV1,
    book: &BookV1,
    bounds: &SearchBoundsV1,
) -> Result<CandidateV1, ErrorV1> {
    domain.validate()?;
    book.validate(domain)?;
    if bounds.price_step == 0
        || !domain.price_scale.is_multiple_of(bounds.price_step)
        || bounds.max_imbalance < 0
    {
        return Err(ErrorV1::SearchBudgetExceeded);
    }
    let mut normalized = NormalizedBookV1::EMPTY;
    normalize_into(domain, book, &mut normalized)?;
    let mut mask_indices = [0usize; 16];
    let mut mask_count = 0usize;
    if domain.policy.aon == AonPolicyV1::WitnessedHonoredMask {
        let mut i = 0usize;
        while i < normalized.len as usize {
            if normalized.orders[i].carries_minimum_obligation() {
                if mask_count == mask_indices.len() {
                    return Err(ErrorV1::SearchBudgetExceeded);
                }
                mask_indices[mask_count] = i;
                mask_count += 1;
            }
            i += 1;
        }
    }
    let steps = domain.price_scale / bounds.price_step;
    let outcomes = domain.outcomes();
    let mut digits = [0u64; MAX_OUTCOMES];
    let mut best: Option<CandidateV1> = None;
    let mut visits = 0u32;
    loop {
        let mut used = 0u64;
        let mut i = 0usize;
        while i + 1 < outcomes {
            used += digits[i];
            i += 1;
        }
        if used <= steps {
            let mut prices = [0u64; MAX_OUTCOMES];
            let mut j = 0usize;
            while j + 1 < outcomes {
                prices[j] = digits[j] * bounds.price_step;
                j += 1;
            }
            prices[outcomes - 1] = (steps - used) * bounds.price_step;
            let mut imbalance = -bounds.max_imbalance;
            while imbalance <= bounds.max_imbalance {
                let mut mask_bits = 0u64;
                let total_masks = 1u64 << mask_count;
                while mask_bits < total_masks {
                    let mut mask = 0u64;
                    let mut bit = 0usize;
                    while bit < mask_count {
                        if (mask_bits >> bit) & 1 == 1 {
                            mask |= 1u64 << mask_indices[bit];
                        }
                        bit += 1;
                    }
                    visits += 1;
                    if visits > bounds.max_visits {
                        return Err(ErrorV1::SearchBudgetExceeded);
                    }
                    if let Ok(candidate) =
                        canonical_candidate(domain, book, &prices, imbalance, mask)
                    {
                        let better = match best {
                            None => true,
                            Some(current) => candidate
                                .claimed_score
                                .is_better_than(&current.claimed_score),
                        };
                        if better {
                            best = Some(candidate);
                        }
                    }
                    mask_bits += 1;
                }
                imbalance += 1;
            }
        }
        // Odometer over the first `outcomes - 1` price coordinates.
        if outcomes < 2 {
            break;
        }
        let mut position = 0usize;
        loop {
            if position + 1 >= outcomes {
                break;
            }
            digits[position] += 1;
            if digits[position] <= steps {
                break;
            }
            digits[position] = 0;
            position += 1;
        }
        if position + 1 >= outcomes {
            break;
        }
    }
    best.ok_or(ErrorV1::NoValidCandidate)
}

#[cfg(test)]
#[path = "relation_v1_tests.rs"]
mod tests;
