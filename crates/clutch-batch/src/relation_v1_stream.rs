//! Streaming, resumable verification of `BatchRelationV1`.
//!
//! This module is IMPLEMENTED host-model code following the design in
//! `docs/implementation/STREAMING_RELATION_DESIGN.md`.  It exists because the
//! batch verifier cannot run on-chain: `relation_v1::verify_inner` is measured
//! at a 39,104-byte SBF call frame against a 4,096-byte maximum, because it
//! holds a `BookV1`, a `NormalizedBookV1`, and a `ParticipationV1` as single
//! locals.  Here every large table lives in one caller-owned checkpoint object,
//! [`ClearWorkV1`], which is only ever touched by reference; the working set of
//! every entry point is **one order (176 bytes), not one book (11 KB)**.
//!
//! # Contract
//!
//! For every frozen domain, canonically padded book, canonically padded
//! candidate, and (under the explicit-slice policy) canonically padded pairing
//! witness, feeding the orders (with their fills) and slices through this
//! verifier produces **the same verdict** as [`crate::relation_v1::verify`]:
//! the same `SummaryV1` on acceptance, and the same [`ErrorV1`] — including
//! the `PairingInfeasible { outcome, owner }` payload — on refusal.  That
//! equivalence is asserted by the exhaustive gate in
//! `relation_v1_stream_tests.rs`; a divergence there is a finding, never a
//! tune.  Success still means only that the candidate is a **best valid
//! submitted candidate** in the batch relation's sense; nothing here is
//! verified in a proof-assistant sense and nothing here is an SVM relation.
//!
//! Three batch refusals are facts about the fixed-array representation and
//! have no streamed counterpart, because the feed has no padding: non-canonical
//! book-slot padding, non-canonical candidate-fill padding, and non-canonical
//! slice padding at or beyond the witness length.  Equivalence is stated over
//! canonically padded batch inputs.
//!
//! # Feed protocol
//!
//! ```text
//! begin(domain, candidate)          freeze coordinates, start order pass 1
//! push_order(order, fill) ...       every order, in canonical id order
//! end_pass()                        -> NeedOrders / NeedSlices / Complete
//! push_slice(slice) ...             only when the policy carries a witness
//! verdict()                         Some(Ok(&SummaryV1) | Err(ErrorV1))
//! ```
//!
//! The number of order passes depends on the frozen self-cross policy: two
//! under `N-a`/`N-c`, three under `N-b` (an order's cancelled quantity depends
//! on whole-book totals, and everything downstream depends on cancellation).
//! Every pass must feed the same `(order, fill)` sequence: pass 1 seals a fold
//! digest over the consumed pairs and every later pass is refused with
//! [`FeedErrorV1::ResumeFoldMismatch`] unless its fold matches — a resumed
//! verification that is not provably the continuation of the same sequence
//! never yields a verdict at all.  The fold is a deterministic consistency
//! device, not a cryptographic commitment; on-chain anchoring belongs to the
//! layout's SHA-256 page digests (design §10).
//!
//! # Refusal identity
//!
//! `verify_inner` reports the refusal of the first stage that fails, at the
//! first program point inside that stage.  The streaming verifier discovers
//! the same facts at different moments, so every refusal source carries a
//! **position** on a ladder that mirrors the batch verifier's program order,
//! and the checkpoint keeps only the least-position refusal.  Resolution
//! happens once, when the feed completes; the only immediate refusals are V0
//! admission faults, which the batch verifier also reports before anything
//! else, in the same per-order sequence.

use crate::relation_v1::{
    mask_bit, scaled_reservation, AllocationPolicyV1, AonPolicyV1, DigestFoldV1, EligibilityV1,
    ErrorV1, FeeBaseV1, LegRefV1, OrderV1, PairingSliceV1, PairingWitnessPolicyV1,
    RelationDomainV1, RoundingBoundaryV1, ScoreV1, SelfCrossPolicyV1, SummaryV1, MAX_OUTCOMES,
    MAX_OWNER_SLOTS, MAX_SLICES,
};
use crate::{seeded_rank, DustPolicy, PartialPolicy, Side, MAX_ORDERS};

/// The candidate header: everything in `CandidateV1` except the fill vector
/// (fills travel with their orders) and the representation padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCandidateV1 {
    /// Number of orders this candidate binds; must equal the fed order count.
    pub order_len: u8,
    /// Exact scaled prices on the simplex.
    pub prices: [u64; MAX_OUTCOMES],
    /// `sigma`: complete sets created by the single global virtual split.
    pub virtual_split: u64,
    /// `mu`: complete sets destroyed by the single global virtual merge.
    pub virtual_merge: u64,
    /// Honored minimum-fill subset; zero unless AON variant 2b is frozen.
    pub honored_aon_mask: u64,
    /// Claimed score, recomputed at V9.
    pub claimed_score: ScoreV1,
    /// Claimed digest, recomputed at V9.
    pub canonical_candidate_digest: u128,
    /// `Some(len)` exactly when the caller will feed an explicit pairing
    /// witness of `len` slices; mirrors `verify`'s `pairing` argument.
    pub declared_slices: Option<u16>,
}

/// Feed-protocol faults.  These are deliberately not [`ErrorV1`]: a protocol
/// fault means the feed itself is broken and verification must restart with
/// [`ClearWorkV1::begin`]; a relation refusal is the verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedErrorV1 {
    /// No verification is in progress (`begin` was never called, or the state
    /// was poisoned by a mismatched resumption).
    NotInProgress,
    /// The feed is complete; only `verdict` is meaningful now.
    FeedComplete,
    /// An order was pushed while the verifier expected slices, or vice versa.
    WrongPhase,
    /// More pushes than the current pass expects.
    TooManyPushes,
    /// A resumed pass is not the continuation of the pass-1 sequence: its
    /// order count or its fold digest differs.  Refusal-on-tamper: the state
    /// is poisoned and yields no verdict.
    ResumeFoldMismatch,
}

/// What the feed expects next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedStatusV1 {
    /// Feed every order of the book again (or for the first time), in
    /// canonical order, through `push_order`.
    NeedOrders {
        /// One-based pass number, for the caller's own bookkeeping.
        pass: u8,
    },
    /// Feed the declared pairing slices through `push_slice`.
    NeedSlices,
    /// The verdict is available.
    Complete,
}

// ---------------------------------------------------------------------------
// The refusal-position ladder (design §6).  Major values mirror the stage
// order of `verify_inner`; minors mirror each stage's internal visit order.
// ---------------------------------------------------------------------------

const M00_DOMAIN: u8 = 0;
const M01_ADMIT: u8 = 1;
const M03_SELF_CROSS: u8 = 3;
const M04_LEN: u8 = 4;
const M05_PRICES: u8 = 5;
const M06_CLASSIFY: u8 = 6;
const M07_WITNESS_FILLS: u8 = 7;
const M08_CHURN: u8 = 8;
const M09_FLOWS: u8 = 9;
const M10_CONSERVATION: u8 = 10;
const M11_CANONICAL: u8 = 11;
const M12_PAIRING: u8 = 12;
const M13_SETTLE: u8 = 13;
const M14_SCORE: u8 = 14;

/// Majors at or below this value are fully decided once V0 completes; a latch
/// there ends the feed early.
const V0_COMPLETE_MAJOR: u8 = M05_PRICES;

const fn pos(major: u8, a: u16, b: u16, c: u16, site: u8) -> u64 {
    ((major as u64) << 56)
        | ((a as u64) << 40)
        | ((b as u64) << 24)
        | ((c as u64) << 8)
        | (site as u64)
}

// Sub-blocks of the V3 canonical ladder: per-outcome step codes, in the exact
// order `derive_canonical` visits them.
const V3_STEP_AGGREGATE: u16 = 0;
const V3_STEP_VIRTUAL: u16 = 1;
const V3_STEP_AON_AGG: u16 = 2;
const V3_STEP_FORCED: u16 = 3;
const V3_STEP_STRICT: u16 = 4;
const V3_STEP_BUY_CAST: u16 = 5;
const V3_STEP_BUY_POOL: u16 = 6;
const V3_STEP_BUY_DUST: u16 = 7;
const V3_STEP_SELL_CAST: u16 = 8;
const V3_STEP_SELL_POOL: u16 = 9;
const V3_STEP_SELL_DUST: u16 = 10;
const V3_STEP_FLOW_CAST: u16 = 11;
/// The post-derivation obligation walk sits after every outcome block.
const V3_BLOCK_OBLIGATION: u16 = MAX_OUTCOMES as u16 + 1;
/// The exact-equality comparison sits last.
const V3_BLOCK_EQUALITY: u16 = MAX_OUTCOMES as u16 + 2;

// Order flags.
const FLAG_ACTIVE: u8 = 1;
const FLAG_FORCED: u8 = 1 << 1;
const FLAG_HONORED: u8 = 1 << 2;
const FLAG_POOL: u8 = 1 << 3;
const FLAG_STRICT_FULL: u8 = 1 << 4;

// Class codes (stored per order; `EligibilityV1` is not zeroable).
const CLASS_STRICT: u8 = 0;
const CLASS_MARGINAL: u8 = 1;
const CLASS_INELIGIBLE: u8 = 2;

// Phase codes.
const PHASE_IDLE: u8 = 0;
const PHASE_ORDERS: u8 = 1;
const PHASE_SLICES: u8 = 2;
const PHASE_COMPLETE: u8 = 3;
const PHASE_POISONED: u8 = 4;

/// One pro-rata pool, keyed `(outcome, side)`; buy pools sit at even indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PoolV1 {
    /// Sum of member effective quantities.
    total: u128,
    /// Member count.
    count: u16,
    /// The pro-rata target, valid only when `ready`.
    target: u64,
    /// Sum of member floors, accumulated during the floor pass.
    floor_sum: u64,
    /// The pool passed its aggregate checks and its floors are meaningful.
    ready: bool,
    /// Dust exists and the frozen policy refuses it; membership is skipped.
    dust_rejected: bool,
}

impl PoolV1 {
    const ZERO: Self = Self {
        total: 0,
        count: 0,
        target: 0,
        floor_sum: 0,
        ready: false,
        dust_rejected: false,
    };
}

/// One pool member's row in the dust-selection key table (design §5).
/// Every pool member is a single-Egg order in exactly one pool, so one row per
/// order bounds the table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PoolRowV1 {
    /// Largest-remainder numerator: `(quantity * target) % total`.
    remainder: u128,
    /// Frozen seeded rank.
    rank: u64,
    /// Canonical order id (unique, makes the key order total).
    id: u64,
    /// `(quantity * target) / total`.
    floor: u64,
    /// Effective quantity, for the derived-vector obligation walk.
    effective: u64,
    /// Effective minimum fill, for the same walk.
    minimum: u64,
    /// Pool index (`outcome * 2 + side`), or `POOL_NONE`.
    pool: u8,
    /// The candidate filled `floor + 1` (versus exactly `floor`).
    extra: bool,
    /// The order is all-or-none.
    aon: bool,
}

impl PoolRowV1 {
    const ZERO: Self = Self {
        remainder: 0,
        rank: 0,
        id: 0,
        floor: 0,
        effective: 0,
        minimum: 0,
        pool: POOL_NONE,
        extra: false,
        aon: false,
    };
}

const POOL_NONE: u8 = u8::MAX;

/// Per-outcome V3 aggregates, in `derive_canonical`'s own units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutcomeAggV1 {
    demand: u128,
    supply: u128,
    forced_buy: u128,
    forced_sell: u128,
    forced_aon_buy: u128,
    forced_aon_sell: u128,
    strict_buy: u128,
    strict_sell: u128,
}

impl OutcomeAggV1 {
    const ZERO: Self = Self {
        demand: 0,
        supply: 0,
        forced_buy: 0,
        forced_sell: 0,
        forced_aon_buy: 0,
        forced_aon_sell: 0,
        strict_buy: 0,
        strict_sell: 0,
    };
}

const SUMMARY_ZERO: SummaryV1 = SummaryV1 {
    outcome_count: 0,
    buy_flow: [0; MAX_OUTCOMES],
    sell_flow: [0; MAX_OUTCOMES],
    total_flow: [0; MAX_OUTCOMES],
    direct_flow: [0; MAX_OUTCOMES],
    virtual_split: 0,
    virtual_merge: 0,
    opening_reserved_egg: [0; MAX_OUTCOMES],
    unfilled_refund_egg: [0; MAX_OUTCOMES],
    netting_cancelled_egg: [0; MAX_OUTCOMES],
    opening_reserved_cash_price_units: 0,
    buyer_consideration_price_units: 0,
    seller_credit_price_units: 0,
    split_cost_price_units: 0,
    merge_proceeds_price_units: 0,
    fee_price_units: 0,
    fee_carry_bps_units: 0,
    cash_refund_price_units: 0,
    rounding_pot_price_units: 0,
    debit_atoms: 0,
    credit_atoms: 0,
    distinct_participating_owners: 0,
    self_overlap_volume: 0,
    score: ScoreV1::ZERO,
    candidate_digest: 0,
};

const DOMAIN_ZERO: RelationDomainV1 = RelationDomainV1 {
    relation_version: 0,
    market_id: 0,
    book_id: 0,
    epoch: 0,
    policy_id: 0,
    order_set_id: 0,
    outcome_count: 0,
    owner_count: 0,
    price_scale: 0,
    remainder_seed: 0,
    policy: crate::relation_v1::FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::RefuseOverlap,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::None,
        residual_settlement: crate::relation_v1::ResidualSettlementV1::FullPairOnly,
        transfer_phase: crate::relation_v1::TransferPhaseV1::ActiveOnly,
        portfolio_lots: crate::relation_v1::PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::Reject,
        score: crate::relation_v1::ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    },
};

const CANDIDATE_ZERO: StreamCandidateV1 = StreamCandidateV1 {
    order_len: 0,
    prices: [0; MAX_OUTCOMES],
    virtual_split: 0,
    virtual_merge: 0,
    honored_aon_mask: 0,
    claimed_score: ScoreV1::ZERO,
    canonical_candidate_digest: 0,
    declared_slices: None,
};

/// The resumable checkpoint object — the `ClearWork` account body.
///
/// One flat struct, fixed size, `no_std`, no allocation.  On-chain it is
/// account data; on the host it is wherever the caller puts it.  Every entry
/// point takes `&mut self` and never materializes more than one order, one
/// slice, or one scalar row on the call frame.  `Clone + PartialEq` is what
/// makes the resumption obligation testable: save = copy, resume = keep using
/// the copy (P-BATCH-03 in the design document).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearWorkV1 {
    // --- control ---
    phase: u8,
    pass: u8,
    order_passes: u8,
    slices_after_pass: u8,
    slices_expected: bool,
    check_claims: bool,
    cursor: u16,
    slice_cursor: u16,
    order_count: u16,
    latch_set: bool,
    latch_position: u64,
    latch_error: ErrorV1,
    fold: DigestFoldV1,
    sealed_fold: DigestFoldV1,
    digest: DigestFoldV1,
    previous_id: u64,
    portfolio_count: u8,
    // --- frozen coordinates ---
    domain: RelationDomainV1,
    cand: StreamCandidateV1,
    // --- owner interning ---
    owners: [u16; MAX_OWNER_SLOTS],
    owner_slots: u16,
    // --- per-order bytes ---
    owner_slot: [u16; MAX_ORDERS],
    side_buy_bits: u64,
    touch: [u16; MAX_ORDERS],
    classes: [u8; MAX_ORDERS],
    flags: [u8; MAX_ORDERS],
    cancelled: [u64; MAX_ORDERS],
    keys: [PoolRowV1; MAX_ORDERS],
    // --- self-cross scratch (N-a presence / N-b totals, then the explicit
    //     slice `covered` table) ---
    scratch_buy: [[u64; MAX_OUTCOMES]; MAX_ORDERS],
    scratch_sell: [[u64; MAX_OUTCOMES]; MAX_ORDERS],
    cell_portfolio: [u16; MAX_OWNER_SLOTS],
    // --- flows and participation ---
    flow_buy: [u128; MAX_OUTCOMES],
    flow_sell: [u128; MAX_OUTCOMES],
    part_buy: [[u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
    part_sell: [[u64; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
    // --- V3 aggregates and pools ---
    agg: [OutcomeAggV1; MAX_OUTCOMES],
    pools: [PoolV1; 2 * MAX_OUTCOMES],
    // --- V6-V8 ledger ---
    reserved_units: [u128; MAX_OWNER_SLOTS],
    debit_units: [u128; MAX_OWNER_SLOTS],
    credit_units: [u128; MAX_OWNER_SLOTS],
    fee_bps_units: [u128; MAX_OWNER_SLOTS],
    opening_reserved_egg: [u64; MAX_OUTCOMES],
    netting_cancelled_egg: [u64; MAX_OUTCOMES],
    seller_filled_egg: [u64; MAX_OUTCOMES],
    opening_reserved_cash: u128,
    netting_cancelled_cash: u128,
    consideration: u128,
    seller_credit: u128,
    limit_surplus: u128,
    debit_atoms: u128,
    credit_atoms: u128,
    rounding_pot: u128,
    // --- explicit slices ---
    split_used: [u64; MAX_OUTCOMES],
    merge_used: [u64; MAX_OUTCOMES],
    // --- output ---
    summary: SummaryV1,
    summary_valid: bool,
}

impl ClearWorkV1 {
    /// The idle checkpoint object.  A `const`, so a probe or a program can
    /// place it in static storage without a by-value copy through a frame.
    pub const NEW: Self = Self {
        phase: PHASE_IDLE,
        pass: 0,
        order_passes: 0,
        slices_after_pass: 0,
        slices_expected: false,
        check_claims: true,
        cursor: 0,
        slice_cursor: 0,
        order_count: 0,
        latch_set: false,
        latch_position: 0,
        latch_error: ErrorV1::UnknownRelationVersion,
        fold: DigestFoldV1::NEW,
        sealed_fold: DigestFoldV1::NEW,
        digest: DigestFoldV1::NEW,
        previous_id: 0,
        portfolio_count: 0,
        domain: DOMAIN_ZERO,
        cand: CANDIDATE_ZERO,
        owners: [0; MAX_OWNER_SLOTS],
        owner_slots: 0,
        owner_slot: [0; MAX_ORDERS],
        side_buy_bits: 0,
        touch: [0; MAX_ORDERS],
        classes: [CLASS_INELIGIBLE; MAX_ORDERS],
        flags: [0; MAX_ORDERS],
        cancelled: [0; MAX_ORDERS],
        keys: [PoolRowV1::ZERO; MAX_ORDERS],
        scratch_buy: [[0; MAX_OUTCOMES]; MAX_ORDERS],
        scratch_sell: [[0; MAX_OUTCOMES]; MAX_ORDERS],
        cell_portfolio: [0; MAX_OWNER_SLOTS],
        flow_buy: [0; MAX_OUTCOMES],
        flow_sell: [0; MAX_OUTCOMES],
        part_buy: [[0; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
        part_sell: [[0; MAX_OUTCOMES]; MAX_OWNER_SLOTS],
        agg: [OutcomeAggV1::ZERO; MAX_OUTCOMES],
        pools: [PoolV1::ZERO; 2 * MAX_OUTCOMES],
        reserved_units: [0; MAX_OWNER_SLOTS],
        debit_units: [0; MAX_OWNER_SLOTS],
        credit_units: [0; MAX_OWNER_SLOTS],
        fee_bps_units: [0; MAX_OWNER_SLOTS],
        opening_reserved_egg: [0; MAX_OUTCOMES],
        netting_cancelled_egg: [0; MAX_OUTCOMES],
        seller_filled_egg: [0; MAX_OUTCOMES],
        opening_reserved_cash: 0,
        netting_cancelled_cash: 0,
        consideration: 0,
        seller_credit: 0,
        limit_surplus: 0,
        debit_atoms: 0,
        credit_atoms: 0,
        rounding_pot: 0,
        split_used: [0; MAX_OUTCOMES],
        merge_used: [0; MAX_OUTCOMES],
        summary: SUMMARY_ZERO,
        summary_valid: false,
    };

    /// A fresh idle checkpoint object.
    pub fn new() -> Self {
        Self::NEW
    }

    /// Reset every field in place.
    fn reset(&mut self) {
        *self = Self::NEW;
    }

    fn latch(&mut self, position: u64, error: ErrorV1) {
        if !self.latch_set || position < self.latch_position {
            self.latch_set = true;
            self.latch_position = position;
            self.latch_error = error;
        }
    }

    fn outcomes(&self) -> usize {
        self.domain.outcome_count as usize
    }

    fn imbalance(&self) -> i128 {
        self.cand.virtual_split as i128 - self.cand.virtual_merge as i128
    }

    fn slices_declared(&self) -> u16 {
        self.cand.declared_slices.unwrap_or_default()
    }

    /// Whether the explicit-slice checks are live: a witness was declared,
    /// the frozen policy consumes one, and its length is representable.
    fn slice_checks_live(&self) -> bool {
        self.slices_expected
            && self.domain.policy.pairing_witness == PairingWitnessPolicyV1::ExplicitSlices
            && (self.slices_declared() as usize) <= MAX_SLICES
    }

    // -----------------------------------------------------------------------
    // begin
    // -----------------------------------------------------------------------

    /// Freeze the coordinates and start the first order pass.
    ///
    /// Mirrors [`crate::relation_v1::verify`]; `strict_claims: false` mirrors
    /// [`crate::relation_v1::verify_ignoring_claimed_aggregates`].
    pub fn begin(
        &mut self,
        domain: &RelationDomainV1,
        candidate: &StreamCandidateV1,
        strict_claims: bool,
    ) -> Result<FeedStatusV1, FeedErrorV1> {
        self.reset();
        self.check_claims = strict_claims;
        self.domain = *domain;
        self.cand = *candidate;
        self.order_passes = match domain.policy.self_cross {
            SelfCrossPolicyV1::NetAtAdmission => 3,
            SelfCrossPolicyV1::RefuseOverlap | SelfCrossPolicyV1::AllowGateAtPairing => 2,
        };
        self.slices_expected = candidate.declared_slices.is_some();
        self.slices_after_pass = self.order_passes - 1;

        // M00: the domain gate, exactly `BookV1::validate`'s first move.
        if let Err(error) = domain.validate() {
            self.latch(pos(M00_DOMAIN, 0, 0, 0, 0), error);
            self.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        // M05: simplex validation, latched (the batch verifier reports the
        // book-length mismatch first, and the length is a feed-end fact).
        if let Err(error) = crate::relation_v1::validate_prices(domain, &candidate.prices) {
            self.latch(pos(M05_PRICES, 0, 0, 0, 0), error);
        }
        // M07 head: a mask under a policy that has no mask.
        let witnessed = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
        if !witnessed && candidate.honored_aon_mask != 0 {
            self.latch(
                pos(M07_WITNESS_FILLS, 0, 0, 0, 0),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        // M08: canonical churn.
        if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
            self.latch(pos(M08_CHURN, 0, 0, 0, 0), ErrorV1::ChurnNotCanonical);
        }
        // M12 witness-policy pairing: declared versus frozen.
        match (domain.policy.pairing_witness, self.slices_expected) {
            (PairingWitnessPolicyV1::RecomputedConstructor, true) => {
                self.latch(
                    pos(M12_PAIRING, 2, 0, 0, 0),
                    ErrorV1::PairingWitnessNotAdmitted,
                );
            }
            (PairingWitnessPolicyV1::ExplicitSlices, false) => {
                self.latch(pos(M12_PAIRING, 2, 0, 0, 0), ErrorV1::PairingWitnessMissing);
            }
            _ => {}
        }
        // An over-long declared witness is the batch `len > MAX_SLICES`
        // refusal; the slice pass is skipped entirely.
        if self.slices_expected && (self.slices_declared() as usize) > MAX_SLICES {
            self.latch(pos(M12_PAIRING, 3, 0, 0, 0), ErrorV1::SliceSumMismatch);
        }
        // The candidate digest binds the head now; fills fold per push.
        self.digest.feed_head(
            domain,
            candidate.order_len,
            &candidate.prices,
            candidate.virtual_split,
            candidate.virtual_merge,
        );
        self.phase = PHASE_ORDERS;
        self.pass = 1;
        Ok(FeedStatusV1::NeedOrders { pass: 1 })
    }

    /// What the feed expects next.
    pub fn status(&self) -> FeedStatusV1 {
        match self.phase {
            PHASE_ORDERS => FeedStatusV1::NeedOrders { pass: self.pass },
            PHASE_SLICES => FeedStatusV1::NeedSlices,
            _ => FeedStatusV1::Complete,
        }
    }

    /// The verdict, once the feed is complete.
    pub fn verdict(&self) -> Option<Result<&SummaryV1, ErrorV1>> {
        if self.phase != PHASE_COMPLETE {
            return None;
        }
        if self.latch_set {
            Some(Err(self.latch_error))
        } else if self.summary_valid {
            Some(Ok(&self.summary))
        } else {
            None
        }
    }

    /// The continuation digest over the consumed `(order, fill)` sequence.
    ///
    /// A deterministic identity for binding the feed to an external order-set
    /// commitment; not a cryptographic commitment by itself.
    pub fn consumed_fold(&self) -> u128 {
        self.sealed_fold.digest()
    }

    // -----------------------------------------------------------------------
    // push_order
    // -----------------------------------------------------------------------

    /// Feed one order and its candidate fill.  Orders must arrive in canonical
    /// (strictly increasing id) sequence; every pass feeds the same sequence.
    pub fn push_order(&mut self, order: &OrderV1, fill: u64) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.phase {
            PHASE_ORDERS => {}
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            PHASE_SLICES => return Err(FeedErrorV1::WrongPhase),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.pass == 1 {
            if self.cursor as usize >= MAX_ORDERS {
                // The 65th order is the batch `TooManyOrders` bound.
                self.latch(pos(M01_ADMIT, self.cursor, 0, 0, 0), ErrorV1::TooManyOrders);
                self.phase = PHASE_COMPLETE;
                return Ok(FeedStatusV1::Complete);
            }
        } else if self.cursor >= self.order_count {
            return Err(FeedErrorV1::TooManyPushes);
        }
        let index = self.cursor as usize;
        self.cursor += 1;
        self.fold_order(order, fill);
        if self.pass == 1 {
            self.digest.feed(fill);
            if let Some(status) = self.admit_order(index, order)? {
                return Ok(status);
            }
        }
        let steps = self.pass_steps();
        if steps.net_assign {
            self.net_assign_order(index, order);
        }
        if steps.accumulate {
            self.accumulate_order(index, order, fill);
        }
        if steps.floor {
            self.floor_order(index, order, fill);
        }
        Ok(self.status())
    }

    /// Which work items the current order pass performs.
    fn pass_steps(&self) -> PassSteps {
        let netting = self.domain.policy.self_cross == SelfCrossPolicyV1::NetAtAdmission;
        if netting {
            PassSteps {
                net_assign: self.pass == 2,
                accumulate: self.pass == 2,
                floor: self.pass == 3,
            }
        } else {
            PassSteps {
                net_assign: false,
                accumulate: self.pass == 1,
                floor: self.pass == 2,
            }
        }
    }

    fn fold_order(&mut self, order: &OrderV1, fill: u64) {
        match order {
            OrderV1::SingleEgg(o) => {
                self.fold.feed(1);
                self.fold.feed(o.canonical_order_id);
                self.fold.feed(o.owner as u64);
                self.fold.feed(o.outcome as u64);
                self.fold.feed(match o.side {
                    Side::Buy => 0,
                    Side::Sell => 1,
                });
                self.fold.feed(o.quantity);
                self.fold.feed(o.limit_price);
                self.fold.feed(o.minimum_fill);
                self.fold.feed(match o.partial_policy {
                    PartialPolicy::Allow => 0,
                    PartialPolicy::AllOrNone => 1,
                });
                self.fold.feed(o.expiry_epoch);
            }
            OrderV1::Portfolio(o) => {
                self.fold.feed(2);
                self.fold.feed(o.canonical_order_id);
                self.fold.feed(o.owner as u64);
                self.fold.feed(match o.side {
                    Side::Buy => 0,
                    Side::Sell => 1,
                });
                let mut i = 0usize;
                while i < MAX_OUTCOMES {
                    self.fold.feed(o.coefficients[i]);
                    i += 1;
                }
                self.fold.feed(o.active_len as u64);
                self.fold.feed(o.lots);
                self.fold.feed(o.limit_collateral_per_lot);
                self.fold.feed(o.minimum_fill_lots);
                self.fold.feed(match o.partial_policy {
                    PartialPolicy::Allow => 0,
                    PartialPolicy::AllOrNone => 1,
                });
                self.fold.feed(o.expiry_epoch);
            }
        }
        self.fold.feed(fill);
    }

    /// V0 admission for one order, exactly `BookV1::validate`'s per-order
    /// walk, plus interning, descriptors, and the self-cross accumulators.
    /// An admission fault ends the feed at once, as the batch verifier does.
    fn admit_order(
        &mut self,
        index: usize,
        order: &OrderV1,
    ) -> Result<Option<FeedStatusV1>, FeedErrorV1> {
        let domain = self.domain;
        let outcomes = self.outcomes();
        let refuse = |work: &mut Self, error: ErrorV1| {
            work.latch(pos(M01_ADMIT, index as u16, 0, 0, 0), error);
            work.phase = PHASE_COMPLETE;
            Some(FeedStatusV1::Complete)
        };
        if order.id() == 0 || order.id() <= self.previous_id {
            return Ok(refuse(self, ErrorV1::NonCanonicalOrderOrder));
        }
        self.previous_id = order.id();
        if order.owner() >= domain.owner_count {
            return Ok(refuse(self, ErrorV1::InvalidOwner));
        }
        if order.expiry_epoch() < domain.epoch {
            return Ok(refuse(self, ErrorV1::ExpiredOrder));
        }
        match order {
            OrderV1::SingleEgg(o) => {
                if o.outcome as usize >= outcomes {
                    return Ok(refuse(self, ErrorV1::InvalidOutcome));
                }
                if o.quantity == 0 {
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                if o.minimum_fill > o.quantity {
                    return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
                }
                if o.limit_price > domain.price_scale {
                    return Ok(refuse(self, ErrorV1::PriceOutOfRange));
                }
            }
            OrderV1::Portfolio(o) => {
                self.portfolio_count += 1;
                if self.portfolio_count as usize > crate::relation_v1::MAX_PORTFOLIO_ORDERS {
                    return Ok(refuse(self, ErrorV1::TooManyPortfolios));
                }
                if o.active_len == 0 || o.active_len as usize > outcomes {
                    return Ok(refuse(self, ErrorV1::InvalidOutcome));
                }
                if o.lots == 0 {
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                if o.minimum_fill_lots > o.lots {
                    return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
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
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                while j < MAX_OUTCOMES {
                    if o.coefficients[j] != 0 {
                        return Ok(refuse(self, ErrorV1::NonCanonicalPadding));
                    }
                    j += 1;
                }
                let mut value = 0u128;
                let mut k = 0usize;
                while k < o.active_len as usize {
                    let term =
                        match (o.coefficients[k] as u128).checked_mul(domain.price_scale as u128) {
                            Some(term) => term,
                            None => return Ok(refuse(self, ErrorV1::ArithmeticOverflow)),
                        };
                    value = match value.checked_add(term) {
                        Some(value) => value,
                        None => return Ok(refuse(self, ErrorV1::ArithmeticOverflow)),
                    };
                    k += 1;
                }
                if (o.lots as u128).checked_mul(value).is_none() {
                    return Ok(refuse(self, ErrorV1::ArithmeticOverflow));
                }
            }
        }
        if order.partial_policy() == PartialPolicy::AllOrNone
            && order.minimum_fill() != order.quantity()
        {
            return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
        }
        if domain.policy.aon == AonPolicyV1::RefuseAdmission {
            if order.partial_policy() == PartialPolicy::AllOrNone {
                return Ok(refuse(self, ErrorV1::AonNotAdmitted));
            }
            if order.minimum_fill() > 1 {
                return Ok(refuse(self, ErrorV1::MinimumFillNotAdmitted));
            }
        }
        if order.reservation_price_units(domain.price_scale).is_err() {
            return Ok(refuse(self, ErrorV1::ArithmeticOverflow));
        }

        // Interning: first-appearance owner slots, as `normalize` assigns them.
        let owner = order.owner();
        let mut slot = usize::MAX;
        let mut s = 0usize;
        while s < self.owner_slots as usize {
            if self.owners[s] == owner {
                slot = s;
                break;
            }
            s += 1;
        }
        if slot == usize::MAX {
            slot = self.owner_slots as usize;
            self.owners[slot] = owner;
            self.owner_slots += 1;
        }
        self.owner_slot[index] = slot as u16;

        // Descriptors for the slice pass.
        if order.side() == Side::Buy {
            self.side_buy_bits |= 1u64 << index;
        }
        let mut touch = 0u16;
        let mut outcome = 0usize;
        while outcome < outcomes {
            if order.touches(outcome as u8) {
                touch |= 1u16 << outcome;
            }
            outcome += 1;
        }
        self.touch[index] = touch;

        // Self-cross accumulators.
        match domain.policy.self_cross {
            SelfCrossPolicyV1::AllowGateAtPairing => {}
            SelfCrossPolicyV1::RefuseOverlap => {
                // Presence only; saturating, because `N-a` computes no totals.
                let mut i = 0usize;
                while i < outcomes {
                    if touch & (1u16 << i) != 0 {
                        let cell = match order.side() {
                            Side::Buy => &mut self.scratch_buy[slot][i],
                            Side::Sell => &mut self.scratch_sell[slot][i],
                        };
                        *cell = cell.saturating_add(1);
                    }
                    i += 1;
                }
            }
            SelfCrossPolicyV1::NetAtAdmission => {
                // Exact order-unit totals, exactly as `net_self_cross` sums
                // them, with the batch's own overflow refusal per cell.
                let units = order.quantity();
                let portfolio = matches!(order, OrderV1::Portfolio(_));
                let mut i = 0usize;
                while i < outcomes {
                    if touch & (1u16 << i) != 0 {
                        if portfolio {
                            self.cell_portfolio[slot] |= 1u16 << i;
                        }
                        let buy = order.side() == Side::Buy;
                        let cell = if buy {
                            self.scratch_buy[slot][i]
                        } else {
                            self.scratch_sell[slot][i]
                        };
                        match cell.checked_add(units) {
                            Some(sum) => {
                                if buy {
                                    self.scratch_buy[slot][i] = sum;
                                } else {
                                    self.scratch_sell[slot][i] = sum;
                                }
                            }
                            None => {
                                self.latch(
                                    pos(M03_SELF_CROSS, i as u16, slot as u16, 0, 0),
                                    ErrorV1::ArithmeticOverflow,
                                );
                            }
                        }
                    }
                    i += 1;
                }
            }
        }
        Ok(None)
    }

    /// `N-b` cancellation assignment for one order: the greedy take from its
    /// cell's remaining-cancel counter, in book order, exactly `cancel_side`.
    fn net_assign_order(&mut self, index: usize, order: &OrderV1) {
        let outcomes = self.outcomes();
        let slot = self.owner_slot[index] as usize;
        let mut i = 0usize;
        while i < outcomes {
            if self.touch[index] & (1u16 << i) == 0 {
                i += 1;
                continue;
            }
            let buy = order.side() == Side::Buy;
            let cell = if buy {
                self.scratch_buy[slot][i]
            } else {
                self.scratch_sell[slot][i]
            };
            let available = order.quantity().saturating_sub(self.cancelled[index]);
            let take = if available < cell { available } else { cell };
            if take != 0 {
                if order.partial_policy() == PartialPolicy::AllOrNone && take != available {
                    // Netting an all-or-none order to a nonzero remainder.
                    self.latch(
                        pos(M03_SELF_CROSS, i as u16, slot as u16, 2, 0),
                        ErrorV1::SelfCrossRefused,
                    );
                }
                if buy {
                    self.scratch_buy[slot][i] = cell - take;
                } else {
                    self.scratch_sell[slot][i] = cell - take;
                }
                self.cancelled[index] += take;
            }
            i += 1;
        }
    }

    /// The accumulate step: V2 classification, the witness-fill walk, flows,
    /// participation, the V3 aggregates, and the V6–V8 per-order ledger.
    fn accumulate_order(&mut self, index: usize, order: &OrderV1, fill: u64) {
        let domain = self.domain;
        let outcomes = self.outcomes();
        let slot = self.owner_slot[index] as usize;
        let effective = order.quantity().saturating_sub(self.cancelled[index]);
        let minimum = order.minimum_fill();
        let effective_minimum = if minimum > effective {
            effective
        } else {
            minimum
        };

        // V2 classification (M06).
        let class = if effective == 0 {
            CLASS_INELIGIBLE
        } else {
            match crate::relation_v1::classify_order(&domain, order, &self.cand.prices) {
                Ok(EligibilityV1::Strict) => CLASS_STRICT,
                Ok(EligibilityV1::Marginal) => CLASS_MARGINAL,
                Ok(EligibilityV1::Ineligible) => CLASS_INELIGIBLE,
                Err(error) => {
                    self.latch(pos(M06_CLASSIFY, index as u16, 0, 0, 0), error);
                    CLASS_INELIGIBLE
                }
            }
        };
        self.classes[index] = class;

        // The witness-fill walk (M07 block 2), exactly `validate_witness_fills`:
        // the first failing check of each order, in the batch check order.
        let witnessed = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
        let honored_bit = mask_bit(self.cand.honored_aon_mask, index);
        let witness_fault = if fill > effective {
            Some(ErrorV1::FillExceedsQuantity)
        } else if fill != 0 && class == CLASS_INELIGIBLE {
            Some(ErrorV1::IneligibleFill)
        } else if witnessed && honored_bit && !order.carries_minimum_obligation() {
            Some(ErrorV1::AonMaskNotApplicable)
        } else if witnessed && honored_bit && (class == CLASS_INELIGIBLE || fill != effective) {
            Some(ErrorV1::AonMaskDishonored)
        } else if witnessed && !honored_bit && order.carries_minimum_obligation() && fill != 0 {
            Some(ErrorV1::AonMaskLeak)
        } else if order.partial_policy() == PartialPolicy::AllOrNone
            && fill != 0
            && fill != effective
        {
            Some(ErrorV1::AllOrNoneViolation)
        } else if fill != 0 && fill < effective_minimum {
            Some(ErrorV1::MinimumFillViolation)
        } else {
            None
        };
        if let Some(error) = witness_fault {
            self.latch(pos(M07_WITNESS_FILLS, 2, index as u16, 0, 0), error);
        }

        // Derivation state (M11 block 0), exactly `derivation_state`.
        let obligated = witnessed && order.carries_minimum_obligation();
        let portfolio = matches!(order, OrderV1::Portfolio(_));
        if honored_bit && !witnessed {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 0),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        if honored_bit && witnessed && !order.carries_minimum_obligation() {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 1),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        if honored_bit && (class == CLASS_INELIGIBLE || effective == 0) {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 2),
                ErrorV1::AonMaskDishonored,
            );
        }
        let active = class != CLASS_INELIGIBLE
            && effective != 0
            && (!obligated || honored_bit)
            && (!portfolio || honored_bit || class == CLASS_STRICT);
        let forced = active && (honored_bit || portfolio);
        let mut flags = 0u8;
        if active {
            flags |= FLAG_ACTIVE;
        }
        if forced {
            flags |= FLAG_FORCED;
        }
        if honored_bit {
            flags |= FLAG_HONORED;
        }

        // Flows (M09), participation (M12 block 0), and the V3 aggregates.
        let allocation_a =
            domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata;
        let mut outcome = 0usize;
        while outcome < outcomes {
            // Legs of the candidate fill: flows and participation.
            if fill != 0 {
                match order.leg_quantity(outcome as u8, fill) {
                    Ok(leg) => {
                        if leg != 0 {
                            let buy = order.side() == Side::Buy;
                            let flow = if buy {
                                self.flow_buy[outcome]
                            } else {
                                self.flow_sell[outcome]
                            };
                            let widened = flow + leg as u128;
                            if flow <= u64::MAX as u128 && widened > u64::MAX as u128 {
                                self.latch(pos(M09_FLOWS, 0, 0, 0, 0), ErrorV1::ArithmeticOverflow);
                            }
                            if buy {
                                self.flow_buy[outcome] = widened;
                            } else {
                                self.flow_sell[outcome] = widened;
                            }
                            let cell = if buy {
                                self.part_buy[slot][outcome]
                            } else {
                                self.part_sell[slot][outcome]
                            };
                            let cell = match cell.checked_add(leg) {
                                Some(sum) => sum,
                                None => {
                                    self.latch(
                                        pos(M12_PAIRING, 0, 0, 0, 0),
                                        ErrorV1::ArithmeticOverflow,
                                    );
                                    u64::MAX
                                }
                            };
                            if buy {
                                self.part_buy[slot][outcome] = cell;
                            } else {
                                self.part_sell[slot][outcome] = cell;
                            }
                        }
                    }
                    Err(error) => self.latch(pos(M09_FLOWS, 0, 0, 0, 0), error),
                }
            }
            // Legs of the effective quantity: the V3 aggregates.
            if active {
                match order.leg_quantity(outcome as u8, effective) {
                    Ok(leg) => {
                        if leg != 0 {
                            let leg = leg as u128;
                            let agg = &mut self.agg[outcome];
                            let buy = order.side() == Side::Buy;
                            if buy {
                                agg.demand += leg;
                            } else {
                                agg.supply += leg;
                            }
                            if forced {
                                if buy {
                                    agg.forced_buy += leg;
                                } else {
                                    agg.forced_sell += leg;
                                }
                                if honored_bit {
                                    if buy {
                                        agg.forced_aon_buy += leg;
                                    } else {
                                        agg.forced_aon_sell += leg;
                                    }
                                }
                            } else if class == CLASS_STRICT {
                                if buy {
                                    agg.strict_buy += leg;
                                } else {
                                    agg.strict_sell += leg;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        self.latch(
                            pos(
                                M11_CANONICAL,
                                1 + outcome as u16,
                                V3_STEP_AGGREGATE,
                                index as u16,
                                0,
                            ),
                            error,
                        );
                    }
                }
            }
            outcome += 1;
        }

        // Pool membership: active, not forced, touching; marginal only under
        // allocation A.  Portfolios are always forced when active, so every
        // pool member is a single-Egg order in exactly one pool.
        if let OrderV1::SingleEgg(o) = order {
            let participant = active && !forced && (o.outcome as usize) < outcomes;
            let pooled = participant && (!allocation_a || class == CLASS_MARGINAL);
            let strict_full = participant && allocation_a && class == CLASS_STRICT;
            if pooled {
                flags |= FLAG_POOL;
                let pool = pool_index(o.outcome as usize, o.side);
                self.pools[pool].total += effective as u128;
                self.pools[pool].count += 1;
            }
            if strict_full {
                flags |= FLAG_STRICT_FULL;
            }
        }
        self.flags[index] = flags;

        // V6–V8 per-order ledger (M13 block 0), exactly `settle_cash`'s walk.
        self.settle_order(index, order, fill, effective, slot);
    }

    /// One order's V6–V8 ledger terms, mirroring `settle_cash`'s per-order
    /// walk site by site.
    fn settle_order(
        &mut self,
        index: usize,
        order: &OrderV1,
        fill: u64,
        effective: u64,
        slot: usize,
    ) {
        let domain = self.domain;
        let scale = domain.price_scale as u128;
        let outcomes = self.outcomes();
        let cancelled = self.cancelled[index];
        let mut site = 0u8;
        macro_rules! settle_latch {
            ($self:ident, $error:expr) => {{
                $self.latch(pos(M13_SETTLE, 0, index as u16, 0, site), $error);
            }};
        }
        macro_rules! settle_add_u128 {
            ($self:ident, $field:expr, $value:expr) => {{
                match $field.checked_add($value) {
                    Some(sum) => $field = sum,
                    None => settle_latch!($self, ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
            }};
        }
        let full_reservation = match order.reservation_price_units(domain.price_scale) {
            Ok(value) => value,
            Err(error) => {
                settle_latch!(self, error);
                return;
            }
        };
        let effective_reservation = match order.side() {
            Side::Buy => match scaled_reservation(order, effective, domain.price_scale) {
                Ok(value) => value,
                Err(error) => {
                    settle_latch!(self, error);
                    return;
                }
            },
            Side::Sell => 0,
        };
        settle_add_u128!(self, self.opening_reserved_cash, full_reservation);
        settle_add_u128!(
            self,
            self.netting_cancelled_cash,
            full_reservation - effective_reservation
        );
        settle_add_u128!(self, self.reserved_units[slot], effective_reservation);

        let mut order_value = 0u128;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let reserved_leg = match order.leg_quantity(outcome as u8, order.quantity()) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(self, error);
                    return;
                }
            };
            let cancelled_leg = match order.leg_quantity(outcome as u8, cancelled) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(self, error);
                    return;
                }
            };
            let filled_leg = match order.leg_quantity(outcome as u8, fill) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(self, error);
                    return;
                }
            };
            if order.side() == Side::Sell {
                match self.opening_reserved_egg[outcome].checked_add(reserved_leg) {
                    Some(sum) => self.opening_reserved_egg[outcome] = sum,
                    None => settle_latch!(self, ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
                match self.netting_cancelled_egg[outcome].checked_add(cancelled_leg) {
                    Some(sum) => self.netting_cancelled_egg[outcome] = sum,
                    None => settle_latch!(self, ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
                match self.seller_filled_egg[outcome].checked_add(filled_leg) {
                    Some(sum) => self.seller_filled_egg[outcome] = sum,
                    None => settle_latch!(self, ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
            }
            if fill != 0 {
                let value =
                    match (filled_leg as u128).checked_mul(self.cand.prices[outcome] as u128) {
                        Some(value) => value,
                        None => {
                            settle_latch!(self, ErrorV1::ArithmeticOverflow);
                            return;
                        }
                    };
                settle_add_u128!(self, order_value, value);
                if domain.policy.rounding == RoundingBoundaryV1::ReceiptFloor && value != 0 {
                    match order.side() {
                        Side::Buy => {
                            let atoms = value.div_ceil(scale);
                            settle_add_u128!(self, self.debit_atoms, atoms);
                            settle_add_u128!(self, self.rounding_pot, atoms * scale - value);
                        }
                        Side::Sell => {
                            let atoms = value / scale;
                            settle_add_u128!(self, self.credit_atoms, atoms);
                            settle_add_u128!(self, self.rounding_pot, value - atoms * scale);
                        }
                    }
                }
            }
            outcome += 1;
        }

        if fill != 0 {
            match order.side() {
                Side::Buy => {
                    settle_add_u128!(self, self.consideration, order_value);
                    settle_add_u128!(self, self.debit_units[slot], order_value);
                    let limit = match scaled_reservation(order, fill, domain.price_scale) {
                        Ok(value) => value,
                        Err(error) => {
                            settle_latch!(self, error);
                            return;
                        }
                    };
                    if limit < order_value {
                        settle_latch!(self, ErrorV1::ConsiderationMismatch);
                        return;
                    }
                    settle_add_u128!(self, self.limit_surplus, limit - order_value);
                    if let FeeBaseV1::FlatNotional { bps } = domain.policy.fee_base {
                        match order_value.checked_mul(bps as u128) {
                            Some(term) => {
                                settle_add_u128!(self, self.fee_bps_units[slot], term);
                            }
                            None => settle_latch!(self, ErrorV1::ArithmeticOverflow),
                        }
                    }
                }
                Side::Sell => {
                    settle_add_u128!(self, self.seller_credit, order_value);
                    settle_add_u128!(self, self.credit_units[slot], order_value);
                    let limit = match scaled_reservation(order, fill, domain.price_scale) {
                        Ok(value) => value,
                        Err(error) => {
                            settle_latch!(self, error);
                            return;
                        }
                    };
                    if order_value < limit {
                        settle_latch!(self, ErrorV1::ConsiderationMismatch);
                        return;
                    }
                    settle_add_u128!(self, self.limit_surplus, order_value - limit);
                }
            }
        }
        let _ = site;
    }

    /// The floor step: per-order canonical-fill characterization against the
    /// pool targets, plus the explicit-slice covered comparison.
    fn floor_order(&mut self, index: usize, order: &OrderV1, fill: u64) {
        let outcomes = self.outcomes();
        let flags = self.flags[index];
        let effective = order.quantity().saturating_sub(self.cancelled[index]);
        let minimum = order.minimum_fill();
        let effective_minimum = if minimum > effective {
            effective
        } else {
            minimum
        };
        let mismatch = pos(M11_CANONICAL, V3_BLOCK_EQUALITY, 0, 0, 0);

        if flags & FLAG_POOL != 0 {
            if let OrderV1::SingleEgg(o) = order {
                let pool = pool_index(o.outcome as usize, o.side);
                let p = self.pools[pool];
                if p.ready && p.target != 0 {
                    // `total >= target > 0` whenever the pool is ready.
                    let product = (effective as u128) * (p.target as u128);
                    let floor = (product / p.total) as u64;
                    let remainder = product % p.total;
                    if fill != floor && fill != floor.saturating_add(1) {
                        self.latch(mismatch, ErrorV1::CandidateMismatch);
                    }
                    self.keys[index] = PoolRowV1 {
                        remainder,
                        rank: seeded_rank(order.id(), self.domain.remainder_seed),
                        id: order.id(),
                        floor,
                        effective,
                        minimum: effective_minimum,
                        pool: pool as u8,
                        extra: fill != floor && fill == floor.saturating_add(1),
                        aon: order.partial_policy() == PartialPolicy::AllOrNone,
                    };
                    self.pools[pool].floor_sum = self.pools[pool].floor_sum.saturating_add(floor);
                } else if p.ready && fill != 0 {
                    // A zero-target pool assigns nothing; a nonzero fill is
                    // not the canonical vector.
                    self.latch(mismatch, ErrorV1::CandidateMismatch);
                }
            }
        } else if flags & (FLAG_FORCED | FLAG_STRICT_FULL) != 0 {
            // Forced orders and (under allocation A) strict participants fill
            // fully in the canonical vector.
            if fill != effective {
                self.latch(mismatch, ErrorV1::CandidateMismatch);
            }
        } else if fill != 0 {
            // Inactive orders carry a zero canonical fill.
            self.latch(mismatch, ErrorV1::CandidateMismatch);
        }

        // Explicit-slice covered comparison, riding this walk (M12 block 4).
        if self.slice_checks_live() {
            let mut outcome = 0usize;
            while outcome < outcomes {
                let leg = match order.leg_quantity(outcome as u8, fill) {
                    Ok(leg) => leg,
                    Err(error) => {
                        self.latch(pos(M09_FLOWS, 0, 0, 0, 0), error);
                        0
                    }
                };
                if self.scratch_buy[index][outcome] != leg {
                    self.latch(pos(M12_PAIRING, 4, 0, 0, 0), ErrorV1::SliceSumMismatch);
                }
                outcome += 1;
            }
        }
    }

    // -----------------------------------------------------------------------
    // push_slice
    // -----------------------------------------------------------------------

    /// Feed one pairing slice of the declared witness, in witness order.
    pub fn push_slice(&mut self, slice: &PairingSliceV1) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.phase {
            PHASE_SLICES => {}
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            PHASE_ORDERS => return Err(FeedErrorV1::WrongPhase),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.slice_cursor >= self.slices_declared() {
            return Err(FeedErrorV1::TooManyPushes);
        }
        let k = self.slice_cursor;
        self.slice_cursor += 1;
        self.digest.feed_slice(slice);
        self.check_slice(k, slice);
        Ok(self.status())
    }

    /// Per-slice executability and coverage, exactly `check_explicit_slices`'s
    /// slice walk.
    fn check_slice(&mut self, k: u16, slice: &PairingSliceV1) {
        let outcomes = self.outcomes();
        let count = self.order_count as usize;
        let fault = pos(M12_PAIRING, 3, 1, k, 0);
        if slice.quantity == 0 || slice.outcome as usize >= outcomes {
            self.latch(fault, ErrorV1::SliceNotExecutable);
            return;
        }
        let outcome = slice.outcome as usize;
        let buy_owner = match slice.buy_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= count
                    || self.side_buy_bits & (1u64 << index) == 0
                    || self.touch[index] & (1u16 << outcome) == 0
                {
                    self.latch(fault, ErrorV1::SliceNotExecutable);
                    return;
                }
                match self.scratch_buy[index][outcome].checked_add(slice.quantity) {
                    Some(sum) => self.scratch_buy[index][outcome] = sum,
                    None => {
                        self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow);
                    }
                }
                Some(self.owner_slot[index])
            }
            LegRefV1::Merge => {
                match self.merge_used[outcome].checked_add(slice.quantity) {
                    Some(sum) => self.merge_used[outcome] = sum,
                    None => {
                        self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow);
                    }
                }
                None
            }
            LegRefV1::Split => {
                self.latch(fault, ErrorV1::SliceNotExecutable);
                return;
            }
        };
        let sell_owner = match slice.sell_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= count
                    || self.side_buy_bits & (1u64 << index) != 0
                    || self.touch[index] & (1u16 << outcome) == 0
                {
                    self.latch(fault, ErrorV1::SliceNotExecutable);
                    return;
                }
                match self.scratch_buy[index][outcome].checked_add(slice.quantity) {
                    Some(sum) => self.scratch_buy[index][outcome] = sum,
                    None => {
                        self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow);
                    }
                }
                Some(self.owner_slot[index])
            }
            LegRefV1::Split => {
                match self.split_used[outcome].checked_add(slice.quantity) {
                    Some(sum) => self.split_used[outcome] = sum,
                    None => {
                        self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow);
                    }
                }
                None
            }
            LegRefV1::Merge => {
                self.latch(fault, ErrorV1::SliceNotExecutable);
                return;
            }
        };
        match (buy_owner, sell_owner) {
            (None, None) => self.latch(fault, ErrorV1::SliceNotExecutable),
            (Some(buy), Some(sell)) if buy == sell => {
                self.latch(fault, ErrorV1::SliceNotExecutable)
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // end_pass
    // -----------------------------------------------------------------------

    /// Close the current pass.  Verifies that a resumed pass consumed the
    /// pass-1 sequence, runs the pass's finalize work, and reports what the
    /// feed expects next.
    pub fn end_pass(&mut self) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.phase {
            PHASE_ORDERS => {}
            PHASE_SLICES => {
                if self.slice_cursor != self.slices_declared() {
                    self.phase = PHASE_POISONED;
                    return Err(FeedErrorV1::ResumeFoldMismatch);
                }
                self.phase = PHASE_ORDERS;
                self.pass += 1;
                return Ok(self.status());
            }
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.pass == 1 {
            self.order_count = self.cursor;
            self.sealed_fold = self.fold;
        } else if self.cursor != self.order_count || self.fold != self.sealed_fold {
            self.phase = PHASE_POISONED;
            return Err(FeedErrorV1::ResumeFoldMismatch);
        }
        self.fold = DigestFoldV1::NEW;
        self.cursor = 0;

        if self.pass == 1 {
            self.finalize_pass_one();
        }
        let steps = self.pass_steps();
        if steps.accumulate {
            self.finalize_accumulate();
        }
        let netting = self.domain.policy.self_cross == SelfCrossPolicyV1::NetAtAdmission;
        let v0_complete = if netting {
            self.pass >= 2
        } else {
            self.pass >= 1
        };
        if v0_complete && self.latch_set && (self.latch_position >> 56) as u8 <= V0_COMPLETE_MAJOR {
            self.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        if steps.floor {
            self.finalize_floor();
            self.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        if self.pass == self.slices_after_pass && self.slice_checks_live() {
            // The covered table reuses the netting scratch; zero it first.
            let mut i = 0usize;
            while i < MAX_ORDERS {
                self.scratch_buy[i] = [0; MAX_OUTCOMES];
                i += 1;
            }
            self.digest.feed(self.slices_declared() as u64);
            self.phase = PHASE_SLICES;
            return Ok(FeedStatusV1::NeedSlices);
        }
        self.pass += 1;
        Ok(self.status())
    }

    /// Pass-1 finalize: the self-cross scan, the length gate, and the mask
    /// bits beyond the book.
    fn finalize_pass_one(&mut self) {
        let outcomes = self.outcomes();
        // Candidate-digest padding: the batch digest folds all 64 fill slots.
        let mut j = self.order_count as usize;
        while j < MAX_ORDERS {
            self.digest.feed(0);
            j += 1;
        }
        self.digest.feed(self.cand.honored_aon_mask);

        match self.domain.policy.self_cross {
            SelfCrossPolicyV1::AllowGateAtPairing => {}
            SelfCrossPolicyV1::RefuseOverlap => {
                let mut outcome = 0usize;
                while outcome < outcomes {
                    let mut slot = 0usize;
                    while slot < self.owner_slots as usize {
                        if self.scratch_buy[slot][outcome] != 0
                            && self.scratch_sell[slot][outcome] != 0
                        {
                            self.latch(
                                pos(M03_SELF_CROSS, outcome as u16, slot as u16, 1, 0),
                                ErrorV1::SelfCrossRefused,
                            );
                        }
                        slot += 1;
                    }
                    outcome += 1;
                }
            }
            SelfCrossPolicyV1::NetAtAdmission => {
                // Turn gross totals into remaining-cancel counters: overlap
                // cells net `min(buy, sell)` on each side, everything else
                // cancels nothing.  A portfolio in an overlap cell refuses.
                let mut outcome = 0usize;
                while outcome < outcomes {
                    let mut slot = 0usize;
                    while slot < self.owner_slots as usize {
                        let buy_total = self.scratch_buy[slot][outcome];
                        let sell_total = self.scratch_sell[slot][outcome];
                        if buy_total != 0 && sell_total != 0 {
                            if self.cell_portfolio[slot] & (1u16 << outcome) != 0 {
                                self.latch(
                                    pos(M03_SELF_CROSS, outcome as u16, slot as u16, 1, 0),
                                    ErrorV1::SelfCrossRefused,
                                );
                            }
                            let netted = if buy_total < sell_total {
                                buy_total
                            } else {
                                sell_total
                            };
                            self.scratch_buy[slot][outcome] = netted;
                            self.scratch_sell[slot][outcome] = netted;
                        } else {
                            self.scratch_buy[slot][outcome] = 0;
                            self.scratch_sell[slot][outcome] = 0;
                        }
                        slot += 1;
                    }
                    outcome += 1;
                }
            }
        }

        // Length gate (M04), then mask bits beyond the book (M07 block 1).
        if self.order_count != self.cand.order_len as u16 {
            self.latch(pos(M04_LEN, 0, 0, 0, 0), ErrorV1::CandidateMismatch);
        }
        let mut i = self.order_count as usize;
        while i < MAX_ORDERS {
            if mask_bit(self.cand.honored_aon_mask, i) {
                self.latch(
                    pos(M07_WITNESS_FILLS, 1, i as u16, 0, 0),
                    ErrorV1::AonMaskNotApplicable,
                );
            }
            i += 1;
        }
    }

    /// Accumulate-pass finalize: per-outcome conservation, the V4 identity,
    /// and the V3 aggregate ladder that fixes every pool target.
    fn finalize_accumulate(&mut self) {
        let outcomes = self.outcomes();
        let imbalance = self.imbalance();
        let allocation_a =
            self.domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata;

        // V4 conservation identity (M10), per outcome ascending.
        let mut outcome = 0usize;
        while outcome < outcomes {
            let left = self.flow_buy[outcome] + self.cand.virtual_merge as u128;
            let right = self.flow_sell[outcome] + self.cand.virtual_split as u128;
            if left != right {
                self.latch(
                    pos(M10_CONSERVATION, outcome as u16, 0, 0, 0),
                    ErrorV1::OutcomeConservationMismatch,
                );
            }
            outcome += 1;
        }

        // The V3 aggregate ladder (M11 blocks 1..=outcomes), per outcome in
        // `derive_canonical`'s exact step order.
        let mut i = 0usize;
        while i < outcomes {
            let block = 1 + i as u16;
            let agg = self.agg[i];
            let supply_plus = agg.supply as i128 + imbalance;
            let executed_buy_signed = if (agg.demand as i128) < supply_plus {
                agg.demand as i128
            } else {
                supply_plus
            };
            let executed_sell_signed = executed_buy_signed - imbalance;
            if executed_buy_signed < 0 || executed_sell_signed < 0 {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_VIRTUAL, 0, 0),
                    ErrorV1::InfeasibleVirtualLeg,
                );
                i += 1;
                continue;
            }
            let executed_buy = executed_buy_signed as u128;
            let executed_sell = executed_sell_signed as u128;
            if executed_buy < agg.forced_aon_buy || executed_sell < agg.forced_aon_sell {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_AON_AGG, 0, 0),
                    ErrorV1::AonMaskDishonored,
                );
            }
            if executed_buy < agg.forced_buy || executed_sell < agg.forced_sell {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_FORCED, 0, 0),
                    ErrorV1::StrictUnderfill,
                );
                i += 1;
                continue;
            }
            if allocation_a
                && (executed_buy < agg.forced_buy + agg.strict_buy
                    || executed_sell < agg.forced_sell + agg.strict_sell)
            {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_STRICT, 0, 0),
                    ErrorV1::StrictUnderfill,
                );
                i += 1;
                continue;
            }
            self.fix_pool_target(
                i,
                Side::Buy,
                executed_buy - agg.forced_buy,
                if allocation_a { agg.strict_buy } else { 0 },
                block,
                V3_STEP_BUY_CAST,
                V3_STEP_BUY_POOL,
            );
            self.fix_pool_target(
                i,
                Side::Sell,
                executed_sell - agg.forced_sell,
                if allocation_a { agg.strict_sell } else { 0 },
                block,
                V3_STEP_SELL_CAST,
                V3_STEP_SELL_POOL,
            );
            if u64::try_from(executed_buy).is_err() || u64::try_from(executed_sell).is_err() {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_FLOW_CAST, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                );
            }
            i += 1;
        }
    }

    /// Fix one pool's pro-rata target, with `allocate_single_side`'s own
    /// refusals: the u64 cast, the empty pool, and the short pool.
    #[allow(clippy::too_many_arguments)]
    fn fix_pool_target(
        &mut self,
        outcome: usize,
        side: Side,
        target_less_forced: u128,
        strict: u128,
        block: u16,
        cast_step: u16,
        pool_step: u16,
    ) {
        let pool = pool_index(outcome, side);
        if u64::try_from(target_less_forced).is_err() {
            self.latch(
                pos(M11_CANONICAL, block, cast_step, 0, 0),
                ErrorV1::ArithmeticOverflow,
            );
            return;
        }
        // Under allocation A the strict walk consumed `strict` exactly (the
        // step-4 aggregate check guarantees it fits).
        let target = target_less_forced - strict;
        let p = &mut self.pools[pool];
        if target == 0 {
            p.target = 0;
            p.ready = true;
            return;
        }
        if p.count == 0 || p.total < target {
            self.latch(
                pos(M11_CANONICAL, block, pool_step, 0, 0),
                ErrorV1::ConservationFailure,
            );
            return;
        }
        // `target <= total <= 2^70`, and it survived the u64 cast above.
        p.target = target as u64;
        p.ready = true;
    }

    /// Floor-pass finalize: dust, top-D membership, the derived-vector
    /// obligation walk, the H-i-O scan, the slice sums, the V6–V8 closures,
    /// and the V9 score, digest, and claims.
    fn finalize_floor(&mut self) {
        self.finalize_dust();
        self.finalize_feasibility();
        self.finalize_slice_sums();
        self.finalize_settle();
        self.finalize_score();
    }

    /// Dust per pool (in `derive_canonical`'s outcome-major, buy-then-sell
    /// order), then exact top-D membership and the obligation walk.
    fn finalize_dust(&mut self) {
        let outcomes = self.outcomes();
        let reject = self.domain.policy.dust == DustPolicy::Reject;
        let mut i = 0usize;
        while i < outcomes {
            let mut s = 0usize;
            while s < 2 {
                let side = if s == 0 { Side::Buy } else { Side::Sell };
                let pool = pool_index(i, side);
                let p = self.pools[pool];
                if p.ready && p.target != 0 {
                    let dust = p.target.saturating_sub(p.floor_sum);
                    if dust != 0 && reject {
                        let step = if s == 0 {
                            V3_STEP_BUY_DUST
                        } else {
                            V3_STEP_SELL_DUST
                        };
                        self.latch(
                            pos(M11_CANONICAL, 1 + i as u16, step, 0, 0),
                            ErrorV1::DustRejected,
                        );
                        self.pools[pool].dust_rejected = true;
                    }
                }
                s += 1;
            }
            i += 1;
        }

        // Top-D membership per pool member: rank-by-comparison over the key
        // table (design §5), then the extras must be exactly the top-D set.
        let count = self.order_count as usize;
        let mismatch = pos(M11_CANONICAL, V3_BLOCK_EQUALITY, 0, 0, 0);
        let mut j = 0usize;
        while j < count {
            let row = self.keys[j];
            if row.pool == POOL_NONE {
                j += 1;
                continue;
            }
            let p = self.pools[row.pool as usize];
            if !p.ready || p.dust_rejected {
                j += 1;
                continue;
            }
            let dust = p.target.saturating_sub(p.floor_sum) as usize;
            let mut better = 0usize;
            let mut k = 0usize;
            while k < count {
                if k != j && self.keys[k].pool == row.pool && key_beats(&self.keys[k], &row) {
                    better += 1;
                }
                k += 1;
            }
            let member = better < dust;
            if member != row.extra {
                self.latch(mismatch, ErrorV1::CandidateMismatch);
            }
            // The derived-vector obligation walk (M11 obligation block): only
            // pool members can violate, because forced and strict-full orders
            // derive their own effective quantity.
            let derived = if member {
                row.floor.saturating_add(1)
            } else {
                row.floor
            };
            if derived != 0 {
                if row.aon && derived != row.effective {
                    self.latch(
                        pos(M11_CANONICAL, V3_BLOCK_OBLIGATION, j as u16, 0, 0),
                        ErrorV1::AllOrNoneViolation,
                    );
                }
                if derived < row.minimum {
                    self.latch(
                        pos(M11_CANONICAL, V3_BLOCK_OBLIGATION, j as u16, 0, 1),
                        ErrorV1::MinimumFillViolation,
                    );
                }
            }
            j += 1;
        }
    }

    /// The H-i-O scan (M12 block 1), in `check_pairing_feasibility`'s exact
    /// `(outcome, slot)` order, with its payload.
    fn finalize_feasibility(&mut self) {
        let outcomes = self.outcomes();
        let merge = self.cand.virtual_merge;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let flow = if self.flow_buy[outcome] > u64::MAX as u128 {
                u64::MAX
            } else {
                self.flow_buy[outcome] as u64
            };
            let total_flow = match flow.checked_add(merge) {
                Some(sum) => sum,
                None => {
                    self.latch(
                        pos(M12_PAIRING, 1, outcome as u16, 0, 0),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let mut slot = 0usize;
            while slot < self.owner_slots as usize {
                match self.part_buy[slot][outcome].checked_add(self.part_sell[slot][outcome]) {
                    Some(part) => {
                        if part > total_flow {
                            self.latch(
                                pos(M12_PAIRING, 1, outcome as u16, slot as u16, 1),
                                ErrorV1::PairingInfeasible {
                                    outcome: outcome as u8,
                                    owner: self.owners[slot],
                                },
                            );
                        }
                    }
                    None => {
                        self.latch(
                            pos(M12_PAIRING, 1, outcome as u16, slot as u16, 0),
                            ErrorV1::ArithmeticOverflow,
                        );
                    }
                }
                slot += 1;
            }
            outcome += 1;
        }
    }

    /// The per-outcome split/merge slice sums (M12 block 5).
    fn finalize_slice_sums(&mut self) {
        if !self.slice_checks_live() {
            return;
        }
        let outcomes = self.outcomes();
        let mut outcome = 0usize;
        while outcome < outcomes {
            if self.split_used[outcome] != self.cand.virtual_split
                || self.merge_used[outcome] != self.cand.virtual_merge
            {
                self.latch(pos(M12_PAIRING, 5, 0, 0, 0), ErrorV1::SliceSumMismatch);
            }
            outcome += 1;
        }
    }

    /// `settle_cash`'s post-walk blocks (M13 blocks 1..=6): the per-outcome
    /// Egg conservation, the fee join, the rounding boundary, and the
    /// closures.  Also fills the summary's ledger fields.
    fn finalize_settle(&mut self) {
        let outcomes = self.outcomes();
        let scale = self.domain.price_scale as u128;
        let mut flow_consideration = 0u128;
        let mut flow_credit = 0u128;
        let mut fee_total = 0u128;
        let mut fee_carry = 0u128;
        let mut cash_refund = 0u128;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let price = self.cand.prices[outcome] as u128;
            match self.flow_buy[outcome]
                .checked_mul(price)
                .and_then(|term| flow_consideration.checked_add(term))
            {
                Some(sum) => flow_consideration = sum,
                None => self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            match self.flow_sell[outcome]
                .checked_mul(price)
                .and_then(|term| flow_credit.checked_add(term))
            {
                Some(sum) => flow_credit = sum,
                None => self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 1),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let refund = self.opening_reserved_egg[outcome]
                .checked_sub(self.netting_cancelled_egg[outcome])
                .and_then(|value| value.checked_sub(self.seller_filled_egg[outcome]));
            let refund = match refund {
                Some(refund) => refund,
                None => {
                    self.latch(
                        pos(M13_SETTLE, 1, outcome as u16, 0, 2),
                        ErrorV1::ConservationFailure,
                    );
                    0
                }
            };
            self.summary.unfilled_refund_egg[outcome] = refund;
            if (self.opening_reserved_egg[outcome] as u128)
                != self.seller_filled_egg[outcome] as u128
                    + self.netting_cancelled_egg[outcome] as u128
                    + refund as u128
            {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 3),
                    ErrorV1::ConservationFailure,
                );
            }
            if self.flow_sell[outcome] != self.seller_filled_egg[outcome] as u128 {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 4),
                    ErrorV1::ConservationFailure,
                );
            }
            let egg_out = self.flow_sell[outcome] + self.cand.virtual_split as u128;
            let egg_in = self.flow_buy[outcome] + self.cand.virtual_merge as u128;
            if egg_out != egg_in {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 5),
                    ErrorV1::ConservationFailure,
                );
            }
            outcome += 1;
        }
        if flow_consideration != self.consideration || flow_credit != self.seller_credit {
            self.latch(pos(M13_SETTLE, 2, 0, 0, 0), ErrorV1::ConsiderationMismatch);
        }

        // V7: the fee join, per owner slot ascending.
        let denominator = crate::relation_v1::FEE_BPS_DENOMINATOR as u128;
        let mut fee_bps_total = 0u128;
        let mut slot = 0usize;
        while slot < self.owner_slots as usize {
            let owed = self.fee_bps_units[slot] / denominator;
            fee_carry += self.fee_bps_units[slot] % denominator;
            match fee_total.checked_add(owed) {
                Some(sum) => fee_total = sum,
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            match self.debit_units[slot].checked_add(owed) {
                Some(sum) => self.debit_units[slot] = sum,
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 1),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            if self.debit_units[slot] > self.reserved_units[slot] {
                self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 2),
                    ErrorV1::FeePayerUnfunded,
                );
            }
            cash_refund += self.reserved_units[slot].saturating_sub(self.debit_units[slot]);
            match fee_bps_total.checked_add(self.fee_bps_units[slot]) {
                Some(sum) => fee_bps_total = sum,
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 3),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            slot += 1;
        }
        if fee_total * denominator + fee_carry != fee_bps_total {
            self.latch(pos(M13_SETTLE, 3, u16::MAX, 0, 0), ErrorV1::FeeMismatch);
        }

        // V6: the one named rounding boundary.
        match self.domain.policy.rounding {
            RoundingBoundaryV1::ReceiptFloor => {
                let mut slot = 0usize;
                while slot < self.owner_slots as usize {
                    let fee_units = self.fee_bps_units[slot] / denominator;
                    if fee_units != 0 {
                        let atoms = fee_units.div_ceil(scale);
                        self.debit_atoms += atoms;
                        self.rounding_pot += atoms * scale - fee_units;
                    }
                    slot += 1;
                }
            }
            RoundingBoundaryV1::TerminalOwnerFloor | RoundingBoundaryV1::None => {
                let mut slot = 0usize;
                while slot < self.owner_slots as usize {
                    if self.debit_units[slot] != 0 {
                        let atoms = self.debit_units[slot].div_ceil(scale);
                        self.debit_atoms += atoms;
                        self.rounding_pot += atoms * scale - self.debit_units[slot];
                    }
                    if self.credit_units[slot] != 0 {
                        let atoms = self.credit_units[slot] / scale;
                        self.credit_atoms += atoms;
                        self.rounding_pot += self.credit_units[slot] - atoms * scale;
                    }
                    slot += 1;
                }
            }
        }
        if self.domain.policy.rounding == RoundingBoundaryV1::None && self.rounding_pot != 0 {
            self.latch(pos(M13_SETTLE, 5, 0, 0, 0), ErrorV1::RemainderRequired);
        }

        // V8: closure.
        let split_cost = match (self.cand.virtual_split as u128).checked_mul(scale) {
            Some(value) => value,
            None => {
                self.latch(pos(M13_SETTLE, 6, 0, 0, 0), ErrorV1::ArithmeticOverflow);
                0
            }
        };
        let merge_proceeds = match (self.cand.virtual_merge as u128).checked_mul(scale) {
            Some(value) => value,
            None => {
                self.latch(pos(M13_SETTLE, 6, 0, 0, 1), ErrorV1::ArithmeticOverflow);
                0
            }
        };
        if self.consideration + merge_proceeds != self.seller_credit + split_cost {
            self.latch(pos(M13_SETTLE, 6, 0, 0, 2), ErrorV1::ConservationFailure);
        }
        if self.opening_reserved_cash
            != self.consideration + fee_total + cash_refund + self.netting_cancelled_cash
        {
            self.latch(pos(M13_SETTLE, 6, 0, 0, 3), ErrorV1::ConservationFailure);
        }

        self.summary.fee_price_units = fee_total;
        self.summary.fee_carry_bps_units = fee_carry;
        self.summary.cash_refund_price_units = cash_refund;
        self.summary.split_cost_price_units = split_cost;
        self.summary.merge_proceeds_price_units = merge_proceeds;
    }

    /// V9: score, digest, claims, and the summary (M14).
    fn finalize_score(&mut self) {
        let outcomes = self.outcomes();
        let scale = self.domain.price_scale as i128;
        let sigma = self.cand.virtual_split;
        let mu = self.cand.virtual_merge;
        let mut weighted = 0i128;
        let mut overlap_total = 0u64;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let flow = if self.flow_buy[outcome] > u64::MAX as u128 {
                u64::MAX
            } else {
                self.flow_buy[outcome] as u64
            };
            let total_flow = match flow.checked_add(mu) {
                Some(sum) => sum,
                None => {
                    self.latch(
                        pos(M14_SCORE, 0, outcome as u16, 0, 0),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let direct = match total_flow
                .checked_sub(sigma)
                .and_then(|v| v.checked_sub(mu))
            {
                Some(direct) => direct,
                None => {
                    self.latch(
                        pos(M14_SCORE, 0, outcome as u16, 0, 1),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let mut overlap = 0u64;
            let mut slot = 0usize;
            while slot < self.owner_slots as usize {
                let buy = self.part_buy[slot][outcome];
                let sell = self.part_sell[slot][outcome];
                let cell = if buy < sell { buy } else { sell };
                match overlap.checked_add(cell) {
                    Some(sum) => overlap = sum,
                    None => self.latch(
                        pos(M14_SCORE, 0, outcome as u16, slot as u16, 2),
                        ErrorV1::ArithmeticOverflow,
                    ),
                }
                slot += 1;
            }
            match overlap_total.checked_add(overlap) {
                Some(sum) => overlap_total = sum,
                None => self.latch(
                    pos(M14_SCORE, 0, outcome as u16, 0, 3),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let price = self.cand.prices[outcome] as i128;
            let weight = price * (scale - price);
            weighted += weight * (direct as i128 - overlap as i128);
            self.summary.buy_flow[outcome] = flow;
            self.summary.sell_flow[outcome] = if self.flow_sell[outcome] > u64::MAX as u128 {
                u64::MAX
            } else {
                self.flow_sell[outcome] as u64
            };
            self.summary.total_flow[outcome] = total_flow;
            self.summary.direct_flow[outcome] = direct;
            outcome += 1;
        }
        let mut owners = 0u16;
        let mut slot = 0usize;
        while slot < self.owner_slots as usize {
            let mut participates = false;
            let mut i = 0usize;
            while i < outcomes {
                if self.part_buy[slot][i] != 0 || self.part_sell[slot][i] != 0 {
                    participates = true;
                }
                i += 1;
            }
            if participates {
                owners += 1;
            }
            slot += 1;
        }
        let churn = match sigma.checked_add(mu) {
            Some(churn) => churn,
            None => {
                self.latch(
                    pos(M14_SCORE, 0, u16::MAX, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                );
                0
            }
        };
        let digest = self.digest.digest();
        let score = ScoreV1 {
            weighted_direct_volume: weighted,
            limit_surplus_price_units: self.limit_surplus,
            distinct_owners: owners,
            churn,
            digest,
        };
        if self.check_claims {
            if self.cand.claimed_score != score {
                self.latch(pos(M14_SCORE, 1, 0, 0, 0), ErrorV1::ScoreMismatch);
            }
            if self.cand.canonical_candidate_digest != digest {
                self.latch(pos(M14_SCORE, 2, 0, 0, 0), ErrorV1::DigestMismatch);
            }
        }

        self.summary.outcome_count = self.domain.outcome_count;
        self.summary.virtual_split = sigma;
        self.summary.virtual_merge = mu;
        self.summary.opening_reserved_egg = self.opening_reserved_egg;
        self.summary.netting_cancelled_egg = self.netting_cancelled_egg;
        self.summary.opening_reserved_cash_price_units = self.opening_reserved_cash;
        self.summary.buyer_consideration_price_units = self.consideration;
        self.summary.seller_credit_price_units = self.seller_credit;
        self.summary.rounding_pot_price_units = self.rounding_pot;
        self.summary.debit_atoms = self.debit_atoms;
        self.summary.credit_atoms = self.credit_atoms;
        self.summary.distinct_participating_owners = owners;
        self.summary.self_overlap_volume = overlap_total;
        self.summary.score = score;
        self.summary.candidate_digest = digest;
        self.summary_valid = !self.latch_set;
    }
}

impl Default for ClearWorkV1 {
    fn default() -> Self {
        Self::new()
    }
}

/// The work items of one order pass.  Under `N-b`, pass 1 additionally
/// accumulates the netting totals inside the admission step itself.
struct PassSteps {
    net_assign: bool,
    accumulate: bool,
    floor: bool,
}

fn pool_index(outcome: usize, side: Side) -> usize {
    outcome * 2
        + match side {
            Side::Buy => 0,
            Side::Sell => 1,
        }
}

/// The frozen largest-remainder key order: greater remainder first, then lower
/// seeded rank, then lower canonical id — exactly `better_remainder`.
fn key_beats(a: &PoolRowV1, b: &PoolRowV1) -> bool {
    if a.remainder != b.remainder {
        return a.remainder > b.remainder;
    }
    if a.rank != b.rank {
        return a.rank < b.rank;
    }
    a.id < b.id
}

#[cfg(test)]
#[path = "relation_v1_stream_tests.rs"]
mod tests;
