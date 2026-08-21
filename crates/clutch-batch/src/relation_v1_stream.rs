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
//! # Checkpoint bytes
//!
//! The struct is `repr(Rust)`; its in-memory bytes are one build's accident.
//! The wire form is [`ClearWorkV1::encode_into`] / [`ClearWorkV1::decode_into`]
//! — an explicit little-endian field walk of exactly
//! [`ClearWorkV1::ENCODED_BYTES`] bytes, total over hostile input, by
//! reference on both sides so no checkpoint value ever crosses a call frame.
//! `save = encode / resume = decode` is the on-chain shape of the P-BATCH-03
//! obligation, and the resumption gate drives it at every push boundary.
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
    mask_bit, scaled_reservation, AllocationPolicyV1, AonPolicyV1, BasisDescriptorV1, DigestFoldV1,
    EligibilityV1, ErrorV1, FeeBaseV1, FrozenPolicyV1, LegRefV1, OrderV1, PairingSliceV1,
    PairingWitnessPolicyV1, PortfolioLotPolicyV1, RelationDomainV1, ResidualSettlementV1,
    RoundingBoundaryV1, ScorePolicyV1, ScoreV1, SelfCrossPolicyV1, SummaryV1, TransferPhaseV1,
    MAX_OUTCOMES, MAX_OWNER_SLOTS, MAX_SLICES,
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
        self.begin_with_basis(domain, candidate, strict_claims, BasisDescriptorV1::UNGATED)
    }

    /// [`Self::begin`], with the market's payout basis bound so that V1b
    /// (moment-cone admission) can run.
    ///
    /// Mirrors [`crate::relation_v1::verify_with_basis`].  The stage is a pure
    /// function of `(domain, candidate.prices, basis)`, so it is decided here
    /// and latched; nothing about the basis enters the checkpoint state, and
    /// `ClearWorkV1::ENCODED_BYTES` is unchanged.
    pub fn begin_with_basis(
        &mut self,
        domain: &RelationDomainV1,
        candidate: &StreamCandidateV1,
        strict_claims: bool,
        basis: BasisDescriptorV1,
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
        // M05 minor 1: moment-cone admission, after the simplex checks in the
        // batch's own order (a candidate failing both reports the simplex
        // refusal in both verifiers).
        if let Err(error) =
            crate::relation_v1::validate_price_moment_cone(domain, basis, &candidate.prices)
        {
            self.latch(pos(M05_PRICES, 1, 0, 0, 0), error);
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

    /// Whether this checkpoint is the idle object — `begin` never ran.
    ///
    /// [`ClearWorkV1::status`] deliberately reports the idle phase as
    /// `Complete` (there is nothing to feed), so a resumable driver that must
    /// distinguish "never begun" from "verdict reached" asks this instead.
    pub fn is_idle(&self) -> bool {
        self.phase == PHASE_IDLE
    }

    /// Whether a mismatched resumption poisoned this checkpoint.
    ///
    /// A poisoned checkpoint yields no verdict and accepts no push; the only
    /// way forward is a fresh `begin`.
    pub fn is_poisoned(&self) -> bool {
        self.phase == PHASE_POISONED
    }

    /// Orders consumed by the pass in progress.
    ///
    /// The resumable driver's cursor: on a checkpoint decoded mid-pass this is
    /// exactly how many `(order, fill)` pairs the current pass has already
    /// folded, i.e. the index of the next order to push.
    pub fn orders_consumed(&self) -> u16 {
        self.cursor
    }

    /// Slices consumed by the slice pass so far.
    ///
    /// The witness-side cursor: the index of the next slice to push.
    pub fn slices_consumed(&self) -> u16 {
        self.slice_cursor
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
                    match domain.policy.fee_base {
                        FeeBaseV1::None => {}
                        FeeBaseV1::FlatNotional { bps } => {
                            match order_value.checked_mul(bps as u128) {
                                Some(term) => {
                                    settle_add_u128!(self, self.fee_bps_units[slot], term);
                                }
                                None => settle_latch!(self, ErrorV1::ArithmeticOverflow),
                            }
                        }
                        // Owner-level, not per order: the composite numerator
                        // is formed once per owner in `finalize_settle`, from
                        // the completed participation table.
                        FeeBaseV1::CompositeDispersionFloor { .. } => {}
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
    /// One composite fee numerator per owner slot, over the common denominator
    /// `kappa_den * S^2 * kappa'_den`, formed from that owner's whole filled buy
    /// vector — `settle_cash`'s composite block, site for site.
    ///
    /// `part_buy` is complete here: the accumulate pass closed before this
    /// finalize ran.  Deliberately **not inlined**: the pairwise loop's locals
    /// would otherwise join `end_pass`'s frame, and the SBF frame budget of
    /// STREAMING_RELATION_DESIGN §9 is measured on `end_pass` with every
    /// finalize inlined into it.
    #[inline(never)]
    fn finalize_composite_numerators(&mut self, outcomes: usize) {
        let FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } = self.domain.policy.fee_base
        else {
            return;
        };
        if dispersion_bps == 0 && floor_range_bps == 0 {
            return;
        }
        let mut slot = 0usize;
        while slot < self.owner_slots as usize {
            match crate::relation_v1::composite_fee_quote(
                &self.part_buy[slot],
                &self.cand.prices,
                outcomes,
                self.domain.price_scale,
                dispersion_bps,
                floor_range_bps,
                0,
            ) {
                Ok(quote) => self.fee_bps_units[slot] = quote.exact_numerator,
                Err(error) => self.latch(pos(M13_SETTLE, 3, slot as u16, 0, 4), error),
            }
            slot += 1;
        }
    }

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

        // V7 composite: one numerator per owner, formed before the join.
        self.finalize_composite_numerators(outcomes);

        // V7: the fee join, per owner slot ascending.
        let denominator = match crate::relation_v1::fee_denominator_of(&self.domain) {
            Ok(value) => value,
            Err(error) => {
                self.latch(pos(M13_SETTLE, 3, u16::MAX, 0, 1), error);
                crate::relation_v1::FEE_BPS_DENOMINATOR as u128
            }
        };
        let quotient_is_atoms = crate::relation_v1::fee_quotient_is_atoms(&self.domain);
        let mut fee_quotient_total = 0u128;
        let mut fee_bps_total = 0u128;
        let mut slot = 0usize;
        while slot < self.owner_slots as usize {
            let quotient = self.fee_bps_units[slot] / denominator;
            let owed = match crate::relation_v1::fee_owed_price_units(
                quotient,
                quotient_is_atoms,
                scale,
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.latch(pos(M13_SETTLE, 3, slot as u16, 0, 5), error);
                    0
                }
            };
            fee_carry += self.fee_bps_units[slot] % denominator;
            fee_quotient_total += quotient;
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
        if fee_quotient_total * denominator + fee_carry != fee_bps_total {
            self.latch(pos(M13_SETTLE, 3, u16::MAX, 0, 0), ErrorV1::FeeMismatch);
        }

        // V6: the one named rounding boundary.
        match self.domain.policy.rounding {
            RoundingBoundaryV1::ReceiptFloor => {
                let mut slot = 0usize;
                while slot < self.owner_slots as usize {
                    let fee_units = match crate::relation_v1::fee_owed_price_units(
                        self.fee_bps_units[slot] / denominator,
                        quotient_is_atoms,
                        scale,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            self.latch(pos(M13_SETTLE, 4, slot as u16, 0, 0), error);
                            0
                        }
                    };
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

// ---------------------------------------------------------------------------
// The checkpoint codec (Tier 2 join 5).
//
// `ClearWorkV1` is `repr(Rust)`: its in-memory bytes are a fact about one
// build, never a wire format.  The codec below is the wire format — an
// explicit field-by-field little-endian serialization in declaration order,
// with every enum, bool, and `Option` given one canonical byte encoding.  The
// layout plane (`clutch-solana-layout`) pins `CLEAR_WORK_BODY_BYTES` to
// [`ClearWorkV1::ENCODED_BYTES`] and treats the region as this codec's.
//
// Everything is by reference: no 48 KB value ever crosses a call frame, the
// same discipline as the feed entry points.  `decode_into` is total over
// arbitrary bytes — it never panics, and it either refuses with a typed
// [`CodecFaultV1`] (resetting the checkpoint to idle) or yields a checkpoint
// whose every field is within its representable range.  The codec claims
// *validity*, not tamper-proofing: refusal of a tampered resumption belongs to
// the fold seal (`ResumeFoldMismatch`), the layout header
// (`ClearWorkHeader::validate` / `require_continuation`), and the
// `(order_set, consumed_fold)` anchor stamped by `bind_order_set` — the codec
// must not weaken those layers, and the codec test battery pins exactly which
// mutations each layer catches.
// ---------------------------------------------------------------------------

/// Checkpoint-codec refusals.  These are deliberately neither [`ErrorV1`] nor
/// [`FeedErrorV1`]: a codec fault means the *bytes* are not a checkpoint, so
/// there is no feed to have a protocol fault in and no relation verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecFaultV1 {
    /// The buffer is not exactly [`ClearWorkV1::ENCODED_BYTES`] long.
    WrongLength,
    /// A phase byte outside the feed lifecycle.
    InvalidPhase,
    /// A boolean byte that is not exactly 0 or 1.
    InvalidBool,
    /// An eligibility class byte outside the registered classes.
    InvalidClass,
    /// An order-flag byte with unregistered bits set.
    InvalidFlags,
    /// A latch-error code outside the registered refusals, or a nonzero
    /// payload on a refusal that carries none.
    InvalidErrorCode,
    /// A policy selector or fee discriminant outside the registered families,
    /// or a fee payload behind the no-fee discriminant.
    InvalidPolicy,
    /// A cursor, count, or bound field outside its reachable range.
    InvalidCount,
    /// An owner-slot or pool index outside its table.
    InvalidSlot,
    /// A declared-slices flag that is not 0/1, or a count behind a clear flag.
    InvalidSliceDeclaration,
}

/// Exact serialized length of one [`FrozenPolicyV1`] inside the checkpoint
/// encoding: the ten family selector bytes in
/// `clutch-batch-policy-identity`'s canonical wire order, then the fee
/// discriminant and its exact-basis-points parameter.  The byte values are
/// restated from that crate (this crate cannot depend on it — the dependency
/// points the other way), and a cross-crate equality test there compares the
/// two encoders byte for byte over every registered selector product.
pub const POLICY_ENCODED_BYTES: usize = 10 + 1 + 4;

/// Encode one policy selection into exactly [`POLICY_ENCODED_BYTES`] bytes.
pub fn encode_policy_v1(policy: &FrozenPolicyV1, out: &mut [u8]) -> Result<(), CodecFaultV1> {
    if out.len() != POLICY_ENCODED_BYTES {
        return Err(CodecFaultV1::WrongLength);
    }
    let mut at = 0usize;
    put_policy(out, &mut at, policy);
    Ok(())
}

/// Decode exactly [`POLICY_ENCODED_BYTES`] policy bytes.
///
/// The accepted set is the *representable* set, not the relation-executable
/// set: registered-but-refused selections (`MarginalProRataLots`, an
/// over-denominator fee) decode, because a checkpoint that latched their
/// refusal at `begin` still carries them and must round-trip.  The relation's
/// own `FrozenPolicyV1::validate` stays the execution gate.
pub fn decode_policy_v1(input: &[u8]) -> Result<FrozenPolicyV1, CodecFaultV1> {
    if input.len() != POLICY_ENCODED_BYTES {
        return Err(CodecFaultV1::WrongLength);
    }
    let mut at = 0usize;
    get_policy(input, &mut at)
}

impl ClearWorkV1 {
    /// Exact canonical serialized length of one checkpoint: every field of
    /// the §7 object in declaration order, little-endian, enums and bools as
    /// single canonical bytes, `declared_slices` as flag byte plus `u16`.
    ///
    /// This is the number `clutch-solana-layout` pins as
    /// `CLEAR_WORK_BODY_BYTES`; the two crates assert the same literal in
    /// their own suites (`clear_work_encoded_bytes_are_pinned` here,
    /// `clearing_account_golden_lengths` there).
    pub const ENCODED_BYTES: usize = {
        // --- control ---
        let control = 1 // phase
            + 1 // pass
            + 1 // order_passes
            + 1 // slices_after_pass
            + 1 // slices_expected
            + 1 // check_claims
            + 2 // cursor
            + 2 // slice_cursor
            + 2 // order_count
            + 1 // latch_set
            + 8 // latch_position
            + 4 // latch_error: code, outcome, owner
            + 16 // fold
            + 16 // sealed_fold
            + 16 // digest
            + 8 // previous_id
            + 1; // portfolio_count
                 // --- frozen coordinates ---
        let domain = 4 + (5 * 8) + 1 + 2 + 8 + 8 + POLICY_ENCODED_BYTES;
        let score = 16 + 16 + 2 + 8 + 16;
        let cand = 1 + (MAX_OUTCOMES * 8) + 8 + 8 + 8 + score + 16 + 3;
        // --- owner interning ---
        let owners = (MAX_OWNER_SLOTS * 2) + 2;
        // --- per-order bytes ---
        let pool_row = 16 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 1;
        let per_order = (MAX_ORDERS * 2) // owner_slot
            + 8 // side_buy_bits
            + (MAX_ORDERS * 2) // touch
            + MAX_ORDERS // classes
            + MAX_ORDERS // flags
            + (MAX_ORDERS * 8) // cancelled
            + (MAX_ORDERS * pool_row); // keys
                                       // --- self-cross scratch ---
        let scratch = (2 * MAX_ORDERS * MAX_OUTCOMES * 8) + (MAX_OWNER_SLOTS * 2);
        // --- flows and participation ---
        let flows = (2 * MAX_OUTCOMES * 16) + (2 * MAX_OWNER_SLOTS * MAX_OUTCOMES * 8);
        // --- V3 aggregates and pools ---
        let pool = 16 + 2 + 8 + 8 + 1 + 1;
        let v3 = (MAX_OUTCOMES * 8 * 16) + (2 * MAX_OUTCOMES * pool);
        // --- V6-V8 ledger ---
        let ledger = (4 * MAX_OWNER_SLOTS * 16) + (3 * MAX_OUTCOMES * 8) + (8 * 16);
        // --- explicit slices ---
        let slices = 2 * MAX_OUTCOMES * 8;
        // --- output ---
        let summary = 1
            + (4 * MAX_OUTCOMES * 8)
            + 16
            + (3 * MAX_OUTCOMES * 8)
            + (11 * 16)
            + 2
            + 8
            + score
            + 16;
        control
            + domain
            + cand
            + owners
            + per_order
            + scratch
            + flows
            + v3
            + ledger
            + slices
            + summary
            + 1 // summary_valid
    };

    /// Serialize this checkpoint into exactly [`Self::ENCODED_BYTES`] bytes.
    ///
    /// Total over every in-memory checkpoint value; the only refusal is a
    /// buffer of the wrong length.  By reference throughout — the largest
    /// value on the frame is one scalar.
    pub fn encode_into(&self, out: &mut [u8]) -> Result<(), CodecFaultV1> {
        if out.len() != Self::ENCODED_BYTES {
            return Err(CodecFaultV1::WrongLength);
        }
        let mut at = 0usize;
        // --- control ---
        put_u8(out, &mut at, self.phase);
        put_u8(out, &mut at, self.pass);
        put_u8(out, &mut at, self.order_passes);
        put_u8(out, &mut at, self.slices_after_pass);
        put_bool(out, &mut at, self.slices_expected);
        put_bool(out, &mut at, self.check_claims);
        put_u16(out, &mut at, self.cursor);
        put_u16(out, &mut at, self.slice_cursor);
        put_u16(out, &mut at, self.order_count);
        put_bool(out, &mut at, self.latch_set);
        put_u64(out, &mut at, self.latch_position);
        put_error(out, &mut at, self.latch_error);
        put_fold(out, &mut at, self.fold);
        put_fold(out, &mut at, self.sealed_fold);
        put_fold(out, &mut at, self.digest);
        put_u64(out, &mut at, self.previous_id);
        put_u8(out, &mut at, self.portfolio_count);
        // --- frozen coordinates ---
        put_u32(out, &mut at, self.domain.relation_version);
        put_u64(out, &mut at, self.domain.market_id);
        put_u64(out, &mut at, self.domain.book_id);
        put_u64(out, &mut at, self.domain.epoch);
        put_u64(out, &mut at, self.domain.policy_id);
        put_u64(out, &mut at, self.domain.order_set_id);
        put_u8(out, &mut at, self.domain.outcome_count);
        put_u16(out, &mut at, self.domain.owner_count);
        put_u64(out, &mut at, self.domain.price_scale);
        put_u64(out, &mut at, self.domain.remainder_seed);
        put_policy(out, &mut at, &self.domain.policy);
        put_u8(out, &mut at, self.cand.order_len);
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            put_u64(out, &mut at, self.cand.prices[i]);
            i += 1;
        }
        put_u64(out, &mut at, self.cand.virtual_split);
        put_u64(out, &mut at, self.cand.virtual_merge);
        put_u64(out, &mut at, self.cand.honored_aon_mask);
        put_score(out, &mut at, &self.cand.claimed_score);
        put_u128(out, &mut at, self.cand.canonical_candidate_digest);
        match self.cand.declared_slices {
            None => {
                put_u8(out, &mut at, 0);
                put_u16(out, &mut at, 0);
            }
            Some(declared) => {
                put_u8(out, &mut at, 1);
                put_u16(out, &mut at, declared);
            }
        }
        // --- owner interning ---
        let mut i = 0usize;
        while i < MAX_OWNER_SLOTS {
            put_u16(out, &mut at, self.owners[i]);
            i += 1;
        }
        put_u16(out, &mut at, self.owner_slots);
        // --- per-order bytes ---
        let mut i = 0usize;
        while i < MAX_ORDERS {
            put_u16(out, &mut at, self.owner_slot[i]);
            i += 1;
        }
        put_u64(out, &mut at, self.side_buy_bits);
        let mut i = 0usize;
        while i < MAX_ORDERS {
            put_u16(out, &mut at, self.touch[i]);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            put_u8(out, &mut at, self.classes[i]);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            put_u8(out, &mut at, self.flags[i]);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            put_u64(out, &mut at, self.cancelled[i]);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            let row = &self.keys[i];
            put_u128(out, &mut at, row.remainder);
            put_u64(out, &mut at, row.rank);
            put_u64(out, &mut at, row.id);
            put_u64(out, &mut at, row.floor);
            put_u64(out, &mut at, row.effective);
            put_u64(out, &mut at, row.minimum);
            put_u8(out, &mut at, row.pool);
            put_bool(out, &mut at, row.extra);
            put_bool(out, &mut at, row.aon);
            i += 1;
        }
        // --- self-cross scratch ---
        put_cells_64(out, &mut at, &self.scratch_buy);
        put_cells_64(out, &mut at, &self.scratch_sell);
        let mut i = 0usize;
        while i < MAX_OWNER_SLOTS {
            put_u16(out, &mut at, self.cell_portfolio[i]);
            i += 1;
        }
        // --- flows and participation ---
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            put_u128(out, &mut at, self.flow_buy[i]);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            put_u128(out, &mut at, self.flow_sell[i]);
            i += 1;
        }
        put_cells_64(out, &mut at, &self.part_buy);
        put_cells_64(out, &mut at, &self.part_sell);
        // --- V3 aggregates and pools ---
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            let agg = &self.agg[i];
            put_u128(out, &mut at, agg.demand);
            put_u128(out, &mut at, agg.supply);
            put_u128(out, &mut at, agg.forced_buy);
            put_u128(out, &mut at, agg.forced_sell);
            put_u128(out, &mut at, agg.forced_aon_buy);
            put_u128(out, &mut at, agg.forced_aon_sell);
            put_u128(out, &mut at, agg.strict_buy);
            put_u128(out, &mut at, agg.strict_sell);
            i += 1;
        }
        let mut i = 0usize;
        while i < 2 * MAX_OUTCOMES {
            let pool = &self.pools[i];
            put_u128(out, &mut at, pool.total);
            put_u16(out, &mut at, pool.count);
            put_u64(out, &mut at, pool.target);
            put_u64(out, &mut at, pool.floor_sum);
            put_bool(out, &mut at, pool.ready);
            put_bool(out, &mut at, pool.dust_rejected);
            i += 1;
        }
        // --- V6-V8 ledger ---
        put_units_64(out, &mut at, &self.reserved_units);
        put_units_64(out, &mut at, &self.debit_units);
        put_units_64(out, &mut at, &self.credit_units);
        put_units_64(out, &mut at, &self.fee_bps_units);
        put_egg_16(out, &mut at, &self.opening_reserved_egg);
        put_egg_16(out, &mut at, &self.netting_cancelled_egg);
        put_egg_16(out, &mut at, &self.seller_filled_egg);
        put_u128(out, &mut at, self.opening_reserved_cash);
        put_u128(out, &mut at, self.netting_cancelled_cash);
        put_u128(out, &mut at, self.consideration);
        put_u128(out, &mut at, self.seller_credit);
        put_u128(out, &mut at, self.limit_surplus);
        put_u128(out, &mut at, self.debit_atoms);
        put_u128(out, &mut at, self.credit_atoms);
        put_u128(out, &mut at, self.rounding_pot);
        // --- explicit slices ---
        put_egg_16(out, &mut at, &self.split_used);
        put_egg_16(out, &mut at, &self.merge_used);
        // --- output ---
        put_u8(out, &mut at, self.summary.outcome_count);
        put_egg_16(out, &mut at, &self.summary.buy_flow);
        put_egg_16(out, &mut at, &self.summary.sell_flow);
        put_egg_16(out, &mut at, &self.summary.total_flow);
        put_egg_16(out, &mut at, &self.summary.direct_flow);
        put_u64(out, &mut at, self.summary.virtual_split);
        put_u64(out, &mut at, self.summary.virtual_merge);
        put_egg_16(out, &mut at, &self.summary.opening_reserved_egg);
        put_egg_16(out, &mut at, &self.summary.unfilled_refund_egg);
        put_egg_16(out, &mut at, &self.summary.netting_cancelled_egg);
        put_u128(out, &mut at, self.summary.opening_reserved_cash_price_units);
        put_u128(out, &mut at, self.summary.buyer_consideration_price_units);
        put_u128(out, &mut at, self.summary.seller_credit_price_units);
        put_u128(out, &mut at, self.summary.split_cost_price_units);
        put_u128(out, &mut at, self.summary.merge_proceeds_price_units);
        put_u128(out, &mut at, self.summary.fee_price_units);
        put_u128(out, &mut at, self.summary.fee_carry_bps_units);
        put_u128(out, &mut at, self.summary.cash_refund_price_units);
        put_u128(out, &mut at, self.summary.rounding_pot_price_units);
        put_u128(out, &mut at, self.summary.debit_atoms);
        put_u128(out, &mut at, self.summary.credit_atoms);
        put_u16(out, &mut at, self.summary.distinct_participating_owners);
        put_u64(out, &mut at, self.summary.self_overlap_volume);
        put_score(out, &mut at, &self.summary.score);
        put_u128(out, &mut at, self.summary.candidate_digest);
        put_bool(out, &mut at, self.summary_valid);
        if at != Self::ENCODED_BYTES {
            return Err(CodecFaultV1::WrongLength);
        }
        Ok(())
    }

    /// Deserialize exactly [`Self::ENCODED_BYTES`] bytes into this checkpoint.
    ///
    /// Total over arbitrary bytes: no input can panic or overflow it.  On any
    /// refusal the checkpoint is reset to the idle value, so a failed decode
    /// never leaves a half-written state behind.  The accepted set is closed
    /// under re-encoding: whatever decodes also re-encodes to the identical
    /// bytes.
    pub fn decode_into(&mut self, input: &[u8]) -> Result<(), CodecFaultV1> {
        if input.len() != Self::ENCODED_BYTES {
            return Err(CodecFaultV1::WrongLength);
        }
        match self.decode_fields(input) {
            Ok(()) => Ok(()),
            Err(fault) => {
                self.reset();
                Err(fault)
            }
        }
    }

    /// Serialize the idle checkpoint — [`Self::NEW`] — without ever holding a
    /// checkpoint value on a call frame: the source is static storage.  This
    /// is the body writer the staged account creation uses at its final grow.
    pub fn encode_idle_into(out: &mut [u8]) -> Result<(), CodecFaultV1> {
        static IDLE: ClearWorkV1 = ClearWorkV1::NEW;
        IDLE.encode_into(out)
    }

    /// The field walk of [`Self::decode_into`]; on `Err` the caller resets.
    fn decode_fields(&mut self, input: &[u8]) -> Result<(), CodecFaultV1> {
        let mut at = 0usize;
        // --- control ---
        let phase = get_u8(input, &mut at);
        if phase > PHASE_POISONED {
            return Err(CodecFaultV1::InvalidPhase);
        }
        self.phase = phase;
        /* Bounds below are phase-sensitive where reachability is: a feed that
         * `begin` refused at the domain gate is COMPLETE and legitimately
         * carries the refused domain, while an ACTIVE phase implies `begin`
         * validated the domain, so out-of-range active coordinates are exactly
         * the states whose next push would index out of a table. */
        let active = phase == PHASE_ORDERS || phase == PHASE_SLICES;
        self.pass = get_u8(input, &mut at);
        self.order_passes = get_u8(input, &mut at);
        self.slices_after_pass = get_u8(input, &mut at);
        self.slices_expected = get_bool(input, &mut at)?;
        self.check_claims = get_bool(input, &mut at)?;
        let cursor = get_u16(input, &mut at);
        if cursor as usize > MAX_ORDERS {
            return Err(CodecFaultV1::InvalidCount);
        }
        self.cursor = cursor;
        let slice_cursor = get_u16(input, &mut at);
        if slice_cursor as usize > MAX_SLICES {
            return Err(CodecFaultV1::InvalidCount);
        }
        self.slice_cursor = slice_cursor;
        let order_count = get_u16(input, &mut at);
        if order_count as usize > MAX_ORDERS {
            return Err(CodecFaultV1::InvalidCount);
        }
        self.order_count = order_count;
        self.latch_set = get_bool(input, &mut at)?;
        self.latch_position = get_u64(input, &mut at);
        self.latch_error = get_error(input, &mut at)?;
        self.fold = get_fold(input, &mut at);
        self.sealed_fold = get_fold(input, &mut at);
        self.digest = get_fold(input, &mut at);
        self.previous_id = get_u64(input, &mut at);
        self.portfolio_count = get_u8(input, &mut at);
        // --- frozen coordinates ---
        self.domain.relation_version = get_u32(input, &mut at);
        self.domain.market_id = get_u64(input, &mut at);
        self.domain.book_id = get_u64(input, &mut at);
        self.domain.epoch = get_u64(input, &mut at);
        self.domain.policy_id = get_u64(input, &mut at);
        self.domain.order_set_id = get_u64(input, &mut at);
        let outcome_count = get_u8(input, &mut at);
        if active && outcome_count as usize > MAX_OUTCOMES {
            return Err(CodecFaultV1::InvalidCount);
        }
        self.domain.outcome_count = outcome_count;
        self.domain.owner_count = get_u16(input, &mut at);
        let price_scale = get_u64(input, &mut at);
        if active && price_scale == 0 {
            // An active feed divides by the scale; `begin` completes a
            // zero-scale domain before any push, so zero here is unreachable.
            return Err(CodecFaultV1::InvalidCount);
        }
        self.domain.price_scale = price_scale;
        self.domain.remainder_seed = get_u64(input, &mut at);
        self.domain.policy = get_policy(input, &mut at)?;
        self.cand.order_len = get_u8(input, &mut at);
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            self.cand.prices[i] = get_u64(input, &mut at);
            i += 1;
        }
        self.cand.virtual_split = get_u64(input, &mut at);
        self.cand.virtual_merge = get_u64(input, &mut at);
        self.cand.honored_aon_mask = get_u64(input, &mut at);
        self.cand.claimed_score = get_score(input, &mut at);
        self.cand.canonical_candidate_digest = get_u128(input, &mut at);
        let declared_flag = get_u8(input, &mut at);
        let declared = get_u16(input, &mut at);
        self.cand.declared_slices = match declared_flag {
            0 if declared == 0 => None,
            0 => return Err(CodecFaultV1::InvalidSliceDeclaration),
            1 => Some(declared),
            _ => return Err(CodecFaultV1::InvalidSliceDeclaration),
        };
        // --- owner interning ---
        let mut i = 0usize;
        while i < MAX_OWNER_SLOTS {
            self.owners[i] = get_u16(input, &mut at);
            i += 1;
        }
        let owner_slots = get_u16(input, &mut at);
        if owner_slots as usize > MAX_OWNER_SLOTS {
            return Err(CodecFaultV1::InvalidCount);
        }
        if active {
            /* Interning appends at `owner_slots` during pass-1 pushes, so an
             * active checkpoint claiming more interned owners than consumed
             * orders is both unreachable and one push from an out-of-bounds
             * append. */
            let consumed = if phase == PHASE_ORDERS && self.pass <= 1 {
                self.cursor
            } else {
                self.order_count
            };
            if owner_slots > consumed {
                return Err(CodecFaultV1::InvalidCount);
            }
        }
        self.owner_slots = owner_slots;
        // --- per-order bytes ---
        let mut i = 0usize;
        while i < MAX_ORDERS {
            let slot = get_u16(input, &mut at);
            if slot as usize >= MAX_OWNER_SLOTS {
                return Err(CodecFaultV1::InvalidSlot);
            }
            self.owner_slot[i] = slot;
            i += 1;
        }
        self.side_buy_bits = get_u64(input, &mut at);
        let mut i = 0usize;
        while i < MAX_ORDERS {
            self.touch[i] = get_u16(input, &mut at);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            let class = get_u8(input, &mut at);
            if class > CLASS_INELIGIBLE {
                return Err(CodecFaultV1::InvalidClass);
            }
            self.classes[i] = class;
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            let flags = get_u8(input, &mut at);
            if flags & !FLAG_ALL != 0 {
                return Err(CodecFaultV1::InvalidFlags);
            }
            self.flags[i] = flags;
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            self.cancelled[i] = get_u64(input, &mut at);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_ORDERS {
            let row = &mut self.keys[i];
            row.remainder = get_u128(input, &mut at);
            row.rank = get_u64(input, &mut at);
            row.id = get_u64(input, &mut at);
            row.floor = get_u64(input, &mut at);
            row.effective = get_u64(input, &mut at);
            row.minimum = get_u64(input, &mut at);
            let pool = get_u8(input, &mut at);
            if pool != POOL_NONE && pool as usize >= 2 * MAX_OUTCOMES {
                return Err(CodecFaultV1::InvalidSlot);
            }
            row.pool = pool;
            row.extra = get_bool(input, &mut at)?;
            row.aon = get_bool(input, &mut at)?;
            i += 1;
        }
        // --- self-cross scratch ---
        get_cells_64(input, &mut at, &mut self.scratch_buy);
        get_cells_64(input, &mut at, &mut self.scratch_sell);
        let mut i = 0usize;
        while i < MAX_OWNER_SLOTS {
            self.cell_portfolio[i] = get_u16(input, &mut at);
            i += 1;
        }
        // --- flows and participation ---
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            self.flow_buy[i] = get_u128(input, &mut at);
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            self.flow_sell[i] = get_u128(input, &mut at);
            i += 1;
        }
        get_cells_64(input, &mut at, &mut self.part_buy);
        get_cells_64(input, &mut at, &mut self.part_sell);
        // --- V3 aggregates and pools ---
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            let agg = &mut self.agg[i];
            agg.demand = get_u128(input, &mut at);
            agg.supply = get_u128(input, &mut at);
            agg.forced_buy = get_u128(input, &mut at);
            agg.forced_sell = get_u128(input, &mut at);
            agg.forced_aon_buy = get_u128(input, &mut at);
            agg.forced_aon_sell = get_u128(input, &mut at);
            agg.strict_buy = get_u128(input, &mut at);
            agg.strict_sell = get_u128(input, &mut at);
            i += 1;
        }
        let mut i = 0usize;
        while i < 2 * MAX_OUTCOMES {
            let total = get_u128(input, &mut at);
            let count = get_u16(input, &mut at);
            let target = get_u64(input, &mut at);
            let floor_sum = get_u64(input, &mut at);
            let ready = get_bool(input, &mut at)?;
            let dust_rejected = get_bool(input, &mut at)?;
            if ready && target != 0 && total == 0 {
                /* The floor step divides members by `total` whenever a ready
                 * pool has a target; `fix_pool_target` only readies a nonzero
                 * target when `total >= target`, so this shape is unreachable
                 * and would be a division by zero. */
                return Err(CodecFaultV1::InvalidCount);
            }
            self.pools[i] = PoolV1 {
                total,
                count,
                target,
                floor_sum,
                ready,
                dust_rejected,
            };
            i += 1;
        }
        // --- V6-V8 ledger ---
        get_units_64(input, &mut at, &mut self.reserved_units);
        get_units_64(input, &mut at, &mut self.debit_units);
        get_units_64(input, &mut at, &mut self.credit_units);
        get_units_64(input, &mut at, &mut self.fee_bps_units);
        get_egg_16(input, &mut at, &mut self.opening_reserved_egg);
        get_egg_16(input, &mut at, &mut self.netting_cancelled_egg);
        get_egg_16(input, &mut at, &mut self.seller_filled_egg);
        self.opening_reserved_cash = get_u128(input, &mut at);
        self.netting_cancelled_cash = get_u128(input, &mut at);
        self.consideration = get_u128(input, &mut at);
        self.seller_credit = get_u128(input, &mut at);
        self.limit_surplus = get_u128(input, &mut at);
        self.debit_atoms = get_u128(input, &mut at);
        self.credit_atoms = get_u128(input, &mut at);
        self.rounding_pot = get_u128(input, &mut at);
        // --- explicit slices ---
        get_egg_16(input, &mut at, &mut self.split_used);
        get_egg_16(input, &mut at, &mut self.merge_used);
        // --- output ---
        self.summary.outcome_count = get_u8(input, &mut at);
        get_egg_16(input, &mut at, &mut self.summary.buy_flow);
        get_egg_16(input, &mut at, &mut self.summary.sell_flow);
        get_egg_16(input, &mut at, &mut self.summary.total_flow);
        get_egg_16(input, &mut at, &mut self.summary.direct_flow);
        self.summary.virtual_split = get_u64(input, &mut at);
        self.summary.virtual_merge = get_u64(input, &mut at);
        get_egg_16(input, &mut at, &mut self.summary.opening_reserved_egg);
        get_egg_16(input, &mut at, &mut self.summary.unfilled_refund_egg);
        get_egg_16(input, &mut at, &mut self.summary.netting_cancelled_egg);
        self.summary.opening_reserved_cash_price_units = get_u128(input, &mut at);
        self.summary.buyer_consideration_price_units = get_u128(input, &mut at);
        self.summary.seller_credit_price_units = get_u128(input, &mut at);
        self.summary.split_cost_price_units = get_u128(input, &mut at);
        self.summary.merge_proceeds_price_units = get_u128(input, &mut at);
        self.summary.fee_price_units = get_u128(input, &mut at);
        self.summary.fee_carry_bps_units = get_u128(input, &mut at);
        self.summary.cash_refund_price_units = get_u128(input, &mut at);
        self.summary.rounding_pot_price_units = get_u128(input, &mut at);
        self.summary.debit_atoms = get_u128(input, &mut at);
        self.summary.credit_atoms = get_u128(input, &mut at);
        self.summary.distinct_participating_owners = get_u16(input, &mut at);
        self.summary.self_overlap_volume = get_u64(input, &mut at);
        self.summary.score = get_score(input, &mut at);
        self.summary.candidate_digest = get_u128(input, &mut at);
        self.summary_valid = get_bool(input, &mut at)?;
        if at != Self::ENCODED_BYTES {
            return Err(CodecFaultV1::WrongLength);
        }
        Ok(())
    }
}

/// Every registered order-flag bit; a decoded flag byte may carry no others.
const FLAG_ALL: u8 = FLAG_ACTIVE | FLAG_FORCED | FLAG_HONORED | FLAG_POOL | FLAG_STRICT_FULL;

/// The registered code of the payload-bearing pairing refusal.
const ERROR_CODE_PAIRING_INFEASIBLE: u8 = 30;

/// The registered code of the moment-cone refusal.  Codes are append-only, so
/// this sits at the end of the table rather than beside the other V1 codes;
/// renumbering would move every checkpoint encoding.  It carries the offending
/// claim in the `outcome` lane and leaves `owner` zero.
const ERROR_CODE_PRICE_OUTSIDE_MOMENT_CONE: u8 = 47;

// --- primitive writers -------------------------------------------------------

fn put_u8(out: &mut [u8], at: &mut usize, value: u8) {
    out[*at] = value;
    *at += 1;
}

fn put_bool(out: &mut [u8], at: &mut usize, value: bool) {
    put_u8(out, at, value as u8);
}

fn put_u16(out: &mut [u8], at: &mut usize, value: u16) {
    out[*at..*at + 2].copy_from_slice(&value.to_le_bytes());
    *at += 2;
}

fn put_u32(out: &mut [u8], at: &mut usize, value: u32) {
    out[*at..*at + 4].copy_from_slice(&value.to_le_bytes());
    *at += 4;
}

fn put_u64(out: &mut [u8], at: &mut usize, value: u64) {
    out[*at..*at + 8].copy_from_slice(&value.to_le_bytes());
    *at += 8;
}

fn put_u128(out: &mut [u8], at: &mut usize, value: u128) {
    out[*at..*at + 16].copy_from_slice(&value.to_le_bytes());
    *at += 16;
}

fn put_i128(out: &mut [u8], at: &mut usize, value: i128) {
    out[*at..*at + 16].copy_from_slice(&value.to_le_bytes());
    *at += 16;
}

fn put_fold(out: &mut [u8], at: &mut usize, fold: DigestFoldV1) {
    let (high, low) = fold.words();
    put_u64(out, at, high);
    put_u64(out, at, low);
}

fn put_score(out: &mut [u8], at: &mut usize, score: &ScoreV1) {
    put_i128(out, at, score.weighted_direct_volume);
    put_u128(out, at, score.limit_surplus_price_units);
    put_u16(out, at, score.distinct_owners);
    put_u64(out, at, score.churn);
    put_u128(out, at, score.digest);
}

fn put_cells_64(out: &mut [u8], at: &mut usize, cells: &[[u64; MAX_OUTCOMES]; MAX_ORDERS]) {
    let mut i = 0usize;
    while i < MAX_ORDERS {
        let mut j = 0usize;
        while j < MAX_OUTCOMES {
            put_u64(out, at, cells[i][j]);
            j += 1;
        }
        i += 1;
    }
}

fn put_units_64(out: &mut [u8], at: &mut usize, units: &[u128; MAX_OWNER_SLOTS]) {
    let mut i = 0usize;
    while i < MAX_OWNER_SLOTS {
        put_u128(out, at, units[i]);
        i += 1;
    }
}

fn put_egg_16(out: &mut [u8], at: &mut usize, egg: &[u64; MAX_OUTCOMES]) {
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        put_u64(out, at, egg[i]);
        i += 1;
    }
}

fn put_error(out: &mut [u8], at: &mut usize, error: ErrorV1) {
    put_u8(out, at, error_code(error));
    let (outcome, owner) = match error {
        ErrorV1::PairingInfeasible { outcome, owner } => (outcome, owner),
        ErrorV1::PriceOutsideMomentCone { outcome } => (outcome, 0),
        _ => (0, 0),
    };
    put_u8(out, at, outcome);
    put_u16(out, at, owner);
}

fn put_policy(out: &mut [u8], at: &mut usize, policy: &FrozenPolicyV1) {
    put_u8(
        out,
        at,
        match policy.allocation {
            AllocationPolicyV1::PricePriorityMarginalProRata => 0,
            AllocationPolicyV1::FullProRata => 1,
        },
    );
    put_u8(
        out,
        at,
        match policy.self_cross {
            SelfCrossPolicyV1::RefuseOverlap => 0,
            SelfCrossPolicyV1::NetAtAdmission => 1,
            SelfCrossPolicyV1::AllowGateAtPairing => 2,
        },
    );
    put_u8(
        out,
        at,
        match policy.aon {
            AonPolicyV1::RefuseAdmission => 0,
            AonPolicyV1::WitnessedHonoredMask => 1,
            AonPolicyV1::FullSizeCounting => 2,
        },
    );
    put_u8(
        out,
        at,
        match policy.rounding {
            RoundingBoundaryV1::None => 0,
            RoundingBoundaryV1::TerminalOwnerFloor => 1,
            RoundingBoundaryV1::ReceiptFloor => 2,
        },
    );
    put_u8(
        out,
        at,
        match policy.residual_settlement {
            ResidualSettlementV1::FullPairOnly => 0,
            ResidualSettlementV1::CumulativePairCanonical => 1,
            ResidualSettlementV1::CumulativePairFree => 2,
            ResidualSettlementV1::UniqueSliceReceipts => 3,
        },
    );
    put_u8(
        out,
        at,
        match policy.transfer_phase {
            TransferPhaseV1::ActiveOnly => 0,
            TransferPhaseV1::ActiveOrResolved => 1,
        },
    );
    put_u8(
        out,
        at,
        match policy.portfolio_lots {
            PortfolioLotPolicyV1::StrictWholeOrder => 0,
            PortfolioLotPolicyV1::MarginalProRataLots => 1,
        },
    );
    put_u8(
        out,
        at,
        match policy.pairing_witness {
            PairingWitnessPolicyV1::RecomputedConstructor => 0,
            PairingWitnessPolicyV1::ExplicitSlices => 1,
        },
    );
    put_u8(
        out,
        at,
        match policy.dust {
            DustPolicy::AssignCanonical => 0,
            DustPolicy::Reject => 1,
        },
    );
    put_u8(
        out,
        at,
        match policy.score {
            ScorePolicyV1::LexicographicDispersionV1 => 0,
        },
    );
    let (fee_tag, fee_bps) = match policy.fee_base {
        FeeBaseV1::None => (0u8, 0u32),
        FeeBaseV1::FlatNotional { bps } => (1, bps),
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } => (2, pack_composite_rates(dispersion_bps, floor_range_bps)),
    };
    put_u8(out, at, fee_tag);
    put_u32(out, at, fee_bps);
}

/// Pack the composite's two rates into the one `u32` rate word: dispersion low,
/// floor high.
///
/// Both rates are bounded by `FEE_BPS_DENOMINATOR` (10,000, a 14-bit value), so
/// the pair fits one word with room to spare and the checkpoint gains no bytes.
/// The zero-rate composite still encodes as the word `0`, so the shape's
/// artifact bytes — and every digest over them — are exactly what they were
/// before rates existed.  `clutch-batch-policy-identity` packs identically; the
/// cross-crate byte-equality test compares the two encoders.
pub(crate) fn pack_composite_rates(dispersion_bps: u32, floor_range_bps: u32) -> u32 {
    (dispersion_bps & 0xFFFF) | ((floor_range_bps & 0xFFFF) << 16)
}

/// Unpack a composite rate word, refusing anything outside the admissible band.
pub(crate) fn unpack_composite_rates(word: u32) -> Option<(u32, u32)> {
    let dispersion_bps = word & 0xFFFF;
    let floor_range_bps = word >> 16;
    let bound = crate::relation_v1::FEE_BPS_DENOMINATOR as u32;
    if dispersion_bps > bound || floor_range_bps > bound {
        return None;
    }
    Some((dispersion_bps, floor_range_bps))
}

// --- primitive readers -------------------------------------------------------

fn get_u8(input: &[u8], at: &mut usize) -> u8 {
    let value = input[*at];
    *at += 1;
    value
}

fn get_bool(input: &[u8], at: &mut usize) -> Result<bool, CodecFaultV1> {
    match get_u8(input, at) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CodecFaultV1::InvalidBool),
    }
}

fn get_u16(input: &[u8], at: &mut usize) -> u16 {
    let value = u16::from_le_bytes([input[*at], input[*at + 1]]);
    *at += 2;
    value
}

fn get_u32(input: &[u8], at: &mut usize) -> u32 {
    let value = u32::from_le_bytes([input[*at], input[*at + 1], input[*at + 2], input[*at + 3]]);
    *at += 4;
    value
}

fn get_u64(input: &[u8], at: &mut usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[*at..*at + 8]);
    *at += 8;
    u64::from_le_bytes(bytes)
}

fn get_u128(input: &[u8], at: &mut usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&input[*at..*at + 16]);
    *at += 16;
    u128::from_le_bytes(bytes)
}

fn get_i128(input: &[u8], at: &mut usize) -> i128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&input[*at..*at + 16]);
    *at += 16;
    i128::from_le_bytes(bytes)
}

fn get_fold(input: &[u8], at: &mut usize) -> DigestFoldV1 {
    let high = get_u64(input, at);
    let low = get_u64(input, at);
    DigestFoldV1::from_words(high, low)
}

fn get_score(input: &[u8], at: &mut usize) -> ScoreV1 {
    ScoreV1 {
        weighted_direct_volume: get_i128(input, at),
        limit_surplus_price_units: get_u128(input, at),
        distinct_owners: get_u16(input, at),
        churn: get_u64(input, at),
        digest: get_u128(input, at),
    }
}

fn get_cells_64(input: &[u8], at: &mut usize, cells: &mut [[u64; MAX_OUTCOMES]; MAX_ORDERS]) {
    let mut i = 0usize;
    while i < MAX_ORDERS {
        let mut j = 0usize;
        while j < MAX_OUTCOMES {
            cells[i][j] = get_u64(input, at);
            j += 1;
        }
        i += 1;
    }
}

fn get_units_64(input: &[u8], at: &mut usize, units: &mut [u128; MAX_OWNER_SLOTS]) {
    let mut i = 0usize;
    while i < MAX_OWNER_SLOTS {
        units[i] = get_u128(input, at);
        i += 1;
    }
}

fn get_egg_16(input: &[u8], at: &mut usize, egg: &mut [u64; MAX_OUTCOMES]) {
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        egg[i] = get_u64(input, at);
        i += 1;
    }
}

fn get_error(input: &[u8], at: &mut usize) -> Result<ErrorV1, CodecFaultV1> {
    let code = get_u8(input, at);
    let outcome = get_u8(input, at);
    let owner = get_u16(input, at);
    // Each code owns exactly the payload lanes its variant carries; every
    // other lane must be canonical zero.
    let lanes_canonical = match code {
        ERROR_CODE_PAIRING_INFEASIBLE => true,
        ERROR_CODE_PRICE_OUTSIDE_MOMENT_CONE => owner == 0,
        _ => outcome == 0 && owner == 0,
    };
    if !lanes_canonical {
        return Err(CodecFaultV1::InvalidErrorCode);
    }
    Ok(match code {
        0 => ErrorV1::UnknownRelationVersion,
        1 => ErrorV1::InvalidPriceScale,
        2 => ErrorV1::PolicyVariantUnimplemented,
        3 => ErrorV1::InvalidOwner,
        4 => ErrorV1::InvalidOutcome,
        5 => ErrorV1::InvalidQuantity,
        6 => ErrorV1::InvalidMinimumFill,
        7 => ErrorV1::NonCanonicalOrderOrder,
        8 => ErrorV1::NonCanonicalPadding,
        9 => ErrorV1::AonNotAdmitted,
        10 => ErrorV1::MinimumFillNotAdmitted,
        11 => ErrorV1::SelfCrossRefused,
        12 => ErrorV1::ExpiredOrder,
        13 => ErrorV1::TooManyOrders,
        14 => ErrorV1::TooManyPortfolios,
        15 => ErrorV1::SimplexSumMismatch,
        16 => ErrorV1::PriceOutOfRange,
        17 => ErrorV1::IneligibleFill,
        18 => ErrorV1::CandidateMismatch,
        19 => ErrorV1::StrictUnderfill,
        20 => ErrorV1::FillExceedsQuantity,
        21 => ErrorV1::MinimumFillViolation,
        22 => ErrorV1::AllOrNoneViolation,
        23 => ErrorV1::AonMaskDishonored,
        24 => ErrorV1::AonMaskLeak,
        25 => ErrorV1::AonMaskNotApplicable,
        26 => ErrorV1::DustRejected,
        27 => ErrorV1::OutcomeConservationMismatch,
        28 => ErrorV1::ChurnNotCanonical,
        29 => ErrorV1::InfeasibleVirtualLeg,
        30 => ErrorV1::PairingInfeasible { outcome, owner },
        31 => ErrorV1::SliceNotExecutable,
        32 => ErrorV1::SliceSumMismatch,
        33 => ErrorV1::PairingWitnessNotAdmitted,
        34 => ErrorV1::PairingWitnessMissing,
        35 => ErrorV1::ConstructorStalled,
        36 => ErrorV1::SliceCapacityExceeded,
        37 => ErrorV1::ConsiderationMismatch,
        38 => ErrorV1::RemainderRequired,
        39 => ErrorV1::FeeMismatch,
        40 => ErrorV1::FeePayerUnfunded,
        41 => ErrorV1::ConservationFailure,
        42 => ErrorV1::ScoreMismatch,
        43 => ErrorV1::DigestMismatch,
        44 => ErrorV1::ArithmeticOverflow,
        45 => ErrorV1::NoValidCandidate,
        46 => ErrorV1::SearchBudgetExceeded,
        ERROR_CODE_PRICE_OUTSIDE_MOMENT_CONE => ErrorV1::PriceOutsideMomentCone { outcome },
        _ => return Err(CodecFaultV1::InvalidErrorCode),
    })
}

fn error_code(error: ErrorV1) -> u8 {
    match error {
        ErrorV1::UnknownRelationVersion => 0,
        ErrorV1::InvalidPriceScale => 1,
        ErrorV1::PolicyVariantUnimplemented => 2,
        ErrorV1::InvalidOwner => 3,
        ErrorV1::InvalidOutcome => 4,
        ErrorV1::InvalidQuantity => 5,
        ErrorV1::InvalidMinimumFill => 6,
        ErrorV1::NonCanonicalOrderOrder => 7,
        ErrorV1::NonCanonicalPadding => 8,
        ErrorV1::AonNotAdmitted => 9,
        ErrorV1::MinimumFillNotAdmitted => 10,
        ErrorV1::SelfCrossRefused => 11,
        ErrorV1::ExpiredOrder => 12,
        ErrorV1::TooManyOrders => 13,
        ErrorV1::TooManyPortfolios => 14,
        ErrorV1::SimplexSumMismatch => 15,
        ErrorV1::PriceOutOfRange => 16,
        ErrorV1::IneligibleFill => 17,
        ErrorV1::CandidateMismatch => 18,
        ErrorV1::StrictUnderfill => 19,
        ErrorV1::FillExceedsQuantity => 20,
        ErrorV1::MinimumFillViolation => 21,
        ErrorV1::AllOrNoneViolation => 22,
        ErrorV1::AonMaskDishonored => 23,
        ErrorV1::AonMaskLeak => 24,
        ErrorV1::AonMaskNotApplicable => 25,
        ErrorV1::DustRejected => 26,
        ErrorV1::OutcomeConservationMismatch => 27,
        ErrorV1::ChurnNotCanonical => 28,
        ErrorV1::InfeasibleVirtualLeg => 29,
        ErrorV1::PairingInfeasible { .. } => ERROR_CODE_PAIRING_INFEASIBLE,
        ErrorV1::PriceOutsideMomentCone { .. } => ERROR_CODE_PRICE_OUTSIDE_MOMENT_CONE,
        ErrorV1::SliceNotExecutable => 31,
        ErrorV1::SliceSumMismatch => 32,
        ErrorV1::PairingWitnessNotAdmitted => 33,
        ErrorV1::PairingWitnessMissing => 34,
        ErrorV1::ConstructorStalled => 35,
        ErrorV1::SliceCapacityExceeded => 36,
        ErrorV1::ConsiderationMismatch => 37,
        ErrorV1::RemainderRequired => 38,
        ErrorV1::FeeMismatch => 39,
        ErrorV1::FeePayerUnfunded => 40,
        ErrorV1::ConservationFailure => 41,
        ErrorV1::ScoreMismatch => 42,
        ErrorV1::DigestMismatch => 43,
        ErrorV1::ArithmeticOverflow => 44,
        ErrorV1::NoValidCandidate => 45,
        ErrorV1::SearchBudgetExceeded => 46,
    }
}

fn get_policy(input: &[u8], at: &mut usize) -> Result<FrozenPolicyV1, CodecFaultV1> {
    let allocation = match get_u8(input, at) {
        0 => AllocationPolicyV1::PricePriorityMarginalProRata,
        1 => AllocationPolicyV1::FullProRata,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let self_cross = match get_u8(input, at) {
        0 => SelfCrossPolicyV1::RefuseOverlap,
        1 => SelfCrossPolicyV1::NetAtAdmission,
        2 => SelfCrossPolicyV1::AllowGateAtPairing,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let aon = match get_u8(input, at) {
        0 => AonPolicyV1::RefuseAdmission,
        1 => AonPolicyV1::WitnessedHonoredMask,
        2 => AonPolicyV1::FullSizeCounting,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let rounding = match get_u8(input, at) {
        0 => RoundingBoundaryV1::None,
        1 => RoundingBoundaryV1::TerminalOwnerFloor,
        2 => RoundingBoundaryV1::ReceiptFloor,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let residual_settlement = match get_u8(input, at) {
        0 => ResidualSettlementV1::FullPairOnly,
        1 => ResidualSettlementV1::CumulativePairCanonical,
        2 => ResidualSettlementV1::CumulativePairFree,
        3 => ResidualSettlementV1::UniqueSliceReceipts,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let transfer_phase = match get_u8(input, at) {
        0 => TransferPhaseV1::ActiveOnly,
        1 => TransferPhaseV1::ActiveOrResolved,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let portfolio_lots = match get_u8(input, at) {
        0 => PortfolioLotPolicyV1::StrictWholeOrder,
        1 => PortfolioLotPolicyV1::MarginalProRataLots,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let pairing_witness = match get_u8(input, at) {
        0 => PairingWitnessPolicyV1::RecomputedConstructor,
        1 => PairingWitnessPolicyV1::ExplicitSlices,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let dust = match get_u8(input, at) {
        0 => DustPolicy::AssignCanonical,
        1 => DustPolicy::Reject,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let score = match get_u8(input, at) {
        0 => ScorePolicyV1::LexicographicDispersionV1,
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    let fee_tag = get_u8(input, at);
    let fee_bps = get_u32(input, at);
    let fee_base = match fee_tag {
        0 if fee_bps == 0 => FeeBaseV1::None,
        0 => return Err(CodecFaultV1::InvalidPolicy),
        1 => FeeBaseV1::FlatNotional { bps: fee_bps },
        // The composite's two rates are packed low/high into the one rate word
        // (`unpack_composite_rates`).  Every rate is bounded by
        // `FEE_BPS_DENOMINATOR`, so a word outside the packing is fail-closed
        // corruption, never a policy.
        2 => match unpack_composite_rates(fee_bps) {
            Some((dispersion_bps, floor_range_bps)) => FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            },
            None => return Err(CodecFaultV1::InvalidPolicy),
        },
        _ => return Err(CodecFaultV1::InvalidPolicy),
    };
    Ok(FrozenPolicyV1 {
        allocation,
        self_cross,
        aon,
        rounding,
        residual_settlement,
        transfer_phase,
        portfolio_lots,
        pairing_witness,
        dust,
        score,
        fee_base,
    })
}

/// Named byte offsets into the encoding, for the codec test battery: field
/// mutation and control-field sweeps address regions by name, never by magic
/// number.  Test-only so production code cannot grow an offset dependence.
#[cfg(test)]
pub(crate) mod codec_offsets {
    use super::{MAX_ORDERS, MAX_OUTCOMES, MAX_OWNER_SLOTS, POLICY_ENCODED_BYTES};

    pub const PHASE: usize = 0;
    pub const PASS: usize = 1;
    pub const SLICES_EXPECTED: usize = 4;
    pub const CHECK_CLAIMS: usize = 5;
    pub const CURSOR: usize = 6;
    pub const SLICE_CURSOR: usize = 8;
    pub const ORDER_COUNT: usize = 10;
    pub const LATCH_SET: usize = 12;
    pub const LATCH_ERROR: usize = 21;
    pub const FOLD: usize = 25;
    pub const SEALED_FOLD: usize = 41;
    pub const DIGEST: usize = 57;
    pub const PREVIOUS_ID: usize = 73;
    pub const DOMAIN: usize = 82;
    pub const DOMAIN_OUTCOME_COUNT: usize = DOMAIN + 44;
    pub const DOMAIN_PRICE_SCALE: usize = DOMAIN + 47;
    pub const DOMAIN_POLICY: usize = DOMAIN + 63;
    pub const CAND: usize = DOMAIN + 63 + POLICY_ENCODED_BYTES;
    pub const CAND_DECLARED: usize = CAND + 227;
    pub const OWNERS: usize = CAND + 230;
    pub const OWNER_SLOTS: usize = OWNERS + MAX_OWNER_SLOTS * 2;
    pub const OWNER_SLOT: usize = OWNER_SLOTS + 2;
    pub const SIDE_BUY_BITS: usize = OWNER_SLOT + MAX_ORDERS * 2;
    pub const TOUCH: usize = SIDE_BUY_BITS + 8;
    pub const CLASSES: usize = TOUCH + MAX_ORDERS * 2;
    pub const FLAGS: usize = CLASSES + MAX_ORDERS;
    pub const CANCELLED: usize = FLAGS + MAX_ORDERS;
    pub const KEYS: usize = CANCELLED + MAX_ORDERS * 8;
    pub const KEYS_POOL: usize = KEYS + 56;
    pub const KEYS_EXTRA: usize = KEYS + 57;
    pub const SCRATCH_BUY: usize = KEYS + MAX_ORDERS * 59;
    pub const SCRATCH_SELL: usize = SCRATCH_BUY + MAX_ORDERS * MAX_OUTCOMES * 8;
    pub const CELL_PORTFOLIO: usize = SCRATCH_SELL + MAX_ORDERS * MAX_OUTCOMES * 8;
    pub const FLOW_BUY: usize = CELL_PORTFOLIO + MAX_OWNER_SLOTS * 2;
    pub const FLOW_SELL: usize = FLOW_BUY + MAX_OUTCOMES * 16;
    pub const PART_BUY: usize = FLOW_SELL + MAX_OUTCOMES * 16;
    pub const PART_SELL: usize = PART_BUY + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
    pub const AGG: usize = PART_SELL + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
    pub const POOLS: usize = AGG + MAX_OUTCOMES * 8 * 16;
    pub const POOLS_READY: usize = POOLS + 34;
    pub const RESERVED_UNITS: usize = POOLS + 2 * MAX_OUTCOMES * 36;
    pub const LEDGER_EGG: usize = RESERVED_UNITS + 4 * MAX_OWNER_SLOTS * 16;
    pub const CASH_SCALARS: usize = LEDGER_EGG + 3 * MAX_OUTCOMES * 8;
    pub const SPLIT_USED: usize = CASH_SCALARS + 8 * 16;
    pub const SUMMARY: usize = SPLIT_USED + 2 * MAX_OUTCOMES * 8;
    pub const SUMMARY_VALID: usize = SUMMARY + 1173;
    pub const END: usize = SUMMARY_VALID + 1;
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

#[cfg(test)]
#[path = "relation_v1_stream_codec_tests.rs"]
mod codec_tests;
