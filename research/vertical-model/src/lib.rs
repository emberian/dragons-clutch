//! Deterministic, offline composition of the three landed Clutch semantic crates.
//!
//! This is a host reference model, not an adapter, client, deployment, or
//! financial execution path.  It owns only orchestration and accounting
//! boundaries: the kernel remains the owner of claim transitions, the
//! accumulator remains the owner of interval-summary semantics, and the batch
//! crate remains the owner of candidate construction and verification.
//!
//! # Two clearing paths, one settlement discipline
//!
//! The model carries two parallel entry points, and they are not the same
//! object:
//!
//! * the **scalar** path ([`VerticalModel::clear_batch`],
//!   [`VerticalModel::clear_batch_with_bindings`],
//!   [`VerticalModel::settle_batch_fill_with_consideration`]) drives
//!   `clutch_batch::FixedBook`, whose relation clears one grid tick over side
//!   totals with owner and outcome erased.  Its pairing bookkeeping is the
//!   model's own: the model, not the relation, decides which buy faces which
//!   sell.  It is retained unchanged as a permanent regression lab and its
//!   golden trace (`golden/basic.trace`) is never rewritten;
//! * the **coupled** path ([`VerticalModel::clear_relation_v1`],
//!   [`VerticalModel::settle_relation_receipt`]) drives
//!   `clutch_batch::relation_v1`, whose relation binds every fill to
//!   `(owner, outcome, side)` and emits a frozen [`PairingWitnessV1`].
//!   Settlement consumes *that* decomposition; the model keeps no pairing
//!   opinion of its own.  Its golden trace is `golden/coupled.trace`.
//!
//! Both paths move claims through `clutch_kernel::MarketState::transfer_internal`
//! with the phase policy named at the call site, and both stage every mutating
//! transition on a clone that is committed only after the conservation check
//! passes.
//!
//! # Nothing here is canonized
//!
//! Every policy selection this model makes is PROPOSED and named at its
//! construction site: the frozen relation policy
//! ([`proposed_relation_policy`]), the price scale and tick table, the bounded
//! search box ([`PROPOSED_SEARCH_BOUNDS`]), the residual-pair settlement
//! variant, and the kernel transfer-phase gate.  An accepted candidate is the
//! **best valid submitted candidate** of its bounded proposal window, never an
//! optimum.

use clutch_accumulator::{CoverageState, Grid, Observation, StatisticError, Summary, SummaryError};
use clutch_batch::relation_v1::{
    self as relation, AllocationPolicyV1, AonPolicyV1, FeeBaseV1, OrderV1, PairingWitnessPolicyV1,
    PairingWitnessV1, PortfolioLotPolicyV1, RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1,
    SingleEggOrderV1, MAX_SLICES, RELATION_VERSION_V1,
};
/// The coupled relation's own vocabulary, re-exported because it appears in
/// this model's public signatures.  These types have exactly one owner and it
/// is `clutch_batch::relation_v1`; nothing here redefines or shadows them.
pub use clutch_batch::relation_v1::{
    BookV1, CandidateV1, ErrorV1, FrozenPolicyV1, LegRefV1, PairingSliceV1, RelationDomainV1,
    ResidualSettlementV1, SearchBoundsV1, SummaryV1, TransferPhaseV1,
};
use clutch_batch::{
    Candidate, DustPolicy, FixedBook, FrozenPolicy, Order, PartialPolicy, PriceGrid, Side, TieRule,
    MAX_GRID_TICKS, MAX_ORDERS,
};
use clutch_kernel::{
    Amount, MarketState, PayoutSet, PayoutVector, Phase, Position, TransferPhasePolicy,
    MAX_OUTCOMES, MAX_PAYOUTS,
};

/// The reference model has two categorical outcomes and two independent owners.
pub const OUTCOMES: u8 = 2;
pub const OWNERS: usize = 2;
pub const RESOLUTION_THRESHOLD: u128 = 50;
pub const FEE_BPS_DENOMINATOR: u64 = 10_000;
/// Frozen observation horizon for the reference market.
pub const MATURITY_BUCKETS: u64 = 3;

/// The model's frozen price-tick table, in the same exact integer units both
/// clearing paths use.  The scalar path reads a tick *index* off each order;
/// the coupled path reads the same table as scaled simplex limit prices.  One
/// table, one semantic owner, no parallel truth.
pub const MODEL_PRICE_TICKS: [u64; 3] = [10, 20, 30];
/// PROPOSED exact integer price scale for the coupled relation: one complete
/// set values at exactly this many price units.  It is a domain parameter of
/// this fixture, never a canonized protocol constant.
pub const RELATION_PRICE_SCALE: u64 = 100;
/// PROPOSED bounded coordinate box for the coupled constructor search.
///
/// `max_imbalance: 0` is load bearing, not incidental: this host model does not
/// yet host the virtual split/merge pot of
/// `docs/implementation/BATCH_RELATION_V1_DESIGN.md` §14.3, so it refuses to
/// clear any candidate that would create or destroy complete sets.  Widening
/// the box without landing the pot produces
/// [`ModelError::VirtualLegNotHosted`], never a silent strand.
pub const PROPOSED_SEARCH_BOUNDS: SearchBoundsV1 = SearchBoundsV1 {
    price_step: 10,
    max_imbalance: 0,
    max_visits: 64,
};

// The coupled relation and the kernel must agree on the outcome bound, or a
// price vector could not be handed from one to the other unchanged.
const _: () = assert!(relation::MAX_OUTCOMES == MAX_OUTCOMES);
const _: () = assert!(RELATION_PRICE_SCALE.is_multiple_of(PROPOSED_SEARCH_BOUNDS.price_step));

/// The scalar path's frozen grid, built from the one model tick table.
fn model_price_grid() -> Result<PriceGrid, clutch_batch::Error> {
    let mut ticks = [0; MAX_GRID_TICKS];
    let mut i = 0usize;
    while i < MODEL_PRICE_TICKS.len() {
        ticks[i] = MODEL_PRICE_TICKS[i];
        i += 1;
    }
    PriceGrid::new(ticks, MODEL_PRICE_TICKS.len() as u8)
}

const fn empty_order() -> Order {
    Order {
        canonical_order_id: 0,
        side: Side::Buy,
        limit_tick: 0,
        quantity: 0,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
    }
}

const fn empty_bound_order() -> BoundOrder {
    BoundOrder {
        order: empty_order(),
        owner: 0,
        outcome: 0,
        expiry_epoch: 0,
    }
}

/// Explicit domain tuple for one frozen batch. These fields are semantic
/// identity, not a cryptographic commitment or deployment authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchDomain {
    pub market_id: u64,
    pub book_id: u64,
    pub epoch: u64,
    pub policy_id: u64,
    pub order_set_id: u64,
}

impl BatchDomain {
    pub const fn new(
        market_id: u64,
        book_id: u64,
        epoch: u64,
        policy_id: u64,
        order_set_id: u64,
    ) -> Self {
        Self {
            market_id,
            book_id,
            epoch,
            policy_id,
            order_set_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateIdentity {
    pub domain: BatchDomain,
    pub candidate: Candidate,
}

/// Batch order plus the owner, outcome, and expiry that the host settlement
/// boundary must authenticate before it can move any claim or cash.
///
/// The scalar path reads only `order`, `owner`, and `outcome`; the coupled
/// path additionally needs `expiry_epoch`, because the coupled relation admits
/// an order only while `expiry_epoch >= domain.epoch`.  Carrying it here keeps
/// one binding record rather than a second parallel order table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundOrder {
    pub order: Order,
    pub owner: usize,
    pub outcome: u8,
    /// The order is admitted by the coupled relation while this is at least the
    /// clearing epoch.  The scalar relation has no expiry concept and ignores
    /// it.
    pub expiry_epoch: u64,
}

/// A settlement receipt binds candidate identity, order identity, parties,
/// claim leg, and exact cash consideration in one value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceipt {
    pub identity: CandidateIdentity,
    pub buy_order_index: u8,
    pub buy_order_id: u64,
    pub sell_order_index: u8,
    pub sell_order_id: u64,
    pub outcome: u8,
    pub quantity: Amount,
    pub consideration: Amount,
}

impl SettlementReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identity: CandidateIdentity,
        buy_order_index: u8,
        buy_order_id: u64,
        sell_order_index: u8,
        sell_order_id: u64,
        outcome: u8,
        quantity: Amount,
        consideration: Amount,
    ) -> Self {
        Self {
            identity,
            buy_order_index,
            buy_order_id,
            sell_order_index,
            sell_order_id,
            outcome,
            quantity,
            consideration,
        }
    }
}

/// Identity of one cleared coupled-relation candidate.
///
/// `domain` is the model's own keying tuple, retained verbatim from the scalar
/// path.  `candidate_digest` is the coupled relation's recomputed candidate
/// digest, which folds the whole frozen relation domain (version, market, book,
/// epoch, policy code, price scale, seed, owner and outcome counts), the free
/// coordinates, the fill vector, **and** the frozen pairing decomposition,
/// because this model freezes [`PairingWitnessPolicyV1::ExplicitSlices`].  A
/// receipt bound to this identity is therefore bound to the slice universe it
/// claims to consume.
///
/// The digest is a deterministic host-model identity, not a cryptographic
/// commitment.  The model never relies on collision resistance: a second
/// candidate arriving under an equal identity but unequal witness is refused
/// with [`ModelError::RelationIdentityCollision`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationCandidateIdentity {
    pub domain: BatchDomain,
    pub candidate_digest: u128,
}

/// What a coupled-relation receipt draws on.
///
/// The kind is not interchangeable: the frozen
/// [`ResidualSettlementV1`] variant decides which kind is admitted, and a
/// receipt of the other kind refuses with
/// [`ModelError::SettlementTargetNotAdmitted`].  There is deliberately no
/// default and no inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementTarget {
    /// Variants **1a** and **1c**: the frozen slice index this receipt
    /// consumes.  1a consumes it whole exactly once; 1c consumes it
    /// cumulatively until it is exhausted.
    Slice(u16),
    /// Variant **1b-canonical**: the receipt draws on the executable pair named
    /// by its own bound order indices and outcome, aggregated over every frozen
    /// slice that carries that pair.  The pair universe is exactly the frozen
    /// slices, so 1b never admits a pair the relation did not emit.
    Pair,
}

/// A coupled-relation settlement receipt.
///
/// It binds the full candidate identity, both canonical order identifiers and
/// their book indices, the outcome, the exact quantity, and the exact cash
/// consideration in price units — the same binding discipline the scalar path
/// already required, extended with the residual target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationReceipt {
    pub identity: RelationCandidateIdentity,
    pub target: SettlementTarget,
    pub buy_order_index: u8,
    pub buy_order_id: u64,
    pub sell_order_index: u8,
    pub sell_order_id: u64,
    pub outcome: u8,
    pub quantity: Amount,
    /// Exact consideration in price units: `quantity * prices[outcome]`.
    pub consideration: Amount,
}

/// What one accepted coupled clearing produced.
///
/// `accepted_volume` is the only quantity the model is allowed to charge fees
/// or liveness against: it is the relation's own recomputed direct flow, every
/// atom of which the frozen decomposition pairs to two distinct bound owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationClearing {
    pub identity: RelationCandidateIdentity,
    pub summary: SummaryV1,
    /// `sum_i direct_flow[i]`, in Egg atoms.
    pub accepted_volume: Amount,
    pub fee: Amount,
    pub slice_count: u16,
    /// Whether this call cleared a new candidate or replayed a cleared one.
    pub replayed: bool,
}

/// A read-only projection of one coupled ledger, for tests and host callers
/// that must compare settlement state without depending on the trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationLedgerView {
    pub identity: RelationCandidateIdentity,
    pub residual_settlement: ResidualSettlementV1,
    pub accepted_volume: Amount,
    pub settled_volume: Amount,
    pub slice_count: u16,
    /// Cumulative settled quantity per order index, ceilinged by the candidate
    /// fill vector.
    pub settled_by_order: [Amount; MAX_ORDERS],
    /// Cumulative settled quantity per frozen slice.
    pub settled_by_slice: [Amount; MAX_SLICES],
    /// The relation's own price-unit rounding pot.  This model settles in exact
    /// price units and never draws on it; it is recorded so the collateral-atom
    /// boundary stays visible rather than implicit.
    pub rounding_pot_price_units: u128,
}

/// Explicit accounting for funds that are not claim principal.
///
/// `principal` mirrors the kernel market collateral.  Fee revenue and
/// liveness funding never enter that value.  Liveness is prepaid in its own
/// bucket and can only be paid, reserved, or returned from that bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accounting {
    pub principal: Amount,
    pub fee_revenue: Amount,
    pub liveness_funding: Amount,
    pub liveness_reserved: Amount,
    pub liveness_paid: Amount,
    pub liveness_returned: Amount,
}

impl Accounting {
    pub fn new(liveness_funding: Amount) -> Self {
        Self {
            principal: 0,
            fee_revenue: 0,
            liveness_funding,
            liveness_reserved: liveness_funding,
            liveness_paid: 0,
            liveness_returned: 0,
        }
    }

    pub fn reserve_liveness(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let mut staged = *self;
        staged.reserve_liveness_inner(amount)?;
        staged.check()?;
        *self = staged;
        Ok(())
    }

    fn reserve_liveness_inner(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let reserved = self
            .liveness_reserved
            .checked_add(amount)
            .ok_or(AccountingError::Overflow)?;
        let funding = self
            .liveness_funding
            .checked_add(amount)
            .ok_or(AccountingError::Overflow)?;
        self.liveness_reserved = reserved;
        self.liveness_funding = funding;
        Ok(())
    }

    pub fn pay_liveness(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let mut staged = *self;
        staged.pay_liveness_inner(amount)?;
        staged.check()?;
        *self = staged;
        Ok(())
    }

    fn pay_liveness_inner(&mut self, amount: Amount) -> Result<(), AccountingError> {
        if amount > self.liveness_reserved {
            return Err(AccountingError::InsufficientLiveness);
        }
        let paid = self
            .liveness_paid
            .checked_add(amount)
            .ok_or(AccountingError::Overflow)?;
        self.liveness_reserved -= amount;
        self.liveness_paid = paid;
        Ok(())
    }

    pub fn return_liveness(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let mut staged = *self;
        staged.return_liveness_inner(amount)?;
        staged.check()?;
        *self = staged;
        Ok(())
    }

    fn return_liveness_inner(&mut self, amount: Amount) -> Result<(), AccountingError> {
        if amount > self.liveness_reserved {
            return Err(AccountingError::InsufficientLiveness);
        }
        let returned = self
            .liveness_returned
            .checked_add(amount)
            .ok_or(AccountingError::Overflow)?;
        self.liveness_reserved -= amount;
        self.liveness_returned = returned;
        Ok(())
    }

    fn add_fee(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let mut staged = *self;
        staged.add_fee_inner(amount)?;
        staged.check()?;
        *self = staged;
        Ok(())
    }

    fn add_fee_inner(&mut self, amount: Amount) -> Result<(), AccountingError> {
        let revenue = self
            .fee_revenue
            .checked_add(amount)
            .ok_or(AccountingError::Overflow)?;
        self.fee_revenue = revenue;
        Ok(())
    }

    fn check(&self) -> Result<(), AccountingError> {
        let accounted = self
            .liveness_reserved
            .checked_add(self.liveness_paid)
            .and_then(|value| value.checked_add(self.liveness_returned))
            .ok_or(AccountingError::Overflow)?;
        if accounted != self.liveness_funding {
            return Err(AccountingError::LivenessConservation);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountingError {
    Overflow,
    InsufficientLiveness,
    LivenessConservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    NotMature,
    NotSealed,
    GappedCoverage,
    NoAcceptedCoverage,
    AmbiguousTerminalInterval,
    UnsupportedStatistic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveDecision {
    Resolved(u8),
    Refused(Refusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CandidateLedger {
    identity: CandidateIdentity,
    orders: [BoundOrder; MAX_ORDERS],
    has_bindings: bool,
    clearing_price: Amount,
    settled_by_order: [Amount; MAX_ORDERS],
    settled_pairs: Vec<(u8, u8)>,
}

/// One cleared coupled candidate and its settlement residue.
///
/// The frozen decomposition is the *only* pairing authority: `settled_by_slice`
/// is the single cumulative ledger all three §13 variants draw on, so a pair's
/// remaining quantity under 1b is derived from the same numbers 1a and 1c
/// consume rather than tracked twice.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationLedger {
    identity: RelationCandidateIdentity,
    domain: RelationDomainV1,
    book: BookV1,
    candidate: CandidateV1,
    pairing: PairingWitnessV1,
    accepted_volume: Amount,
    settled_volume: Amount,
    settled_by_order: [Amount; MAX_ORDERS],
    settled_by_slice: [Amount; MAX_SLICES],
    rounding_pot_price_units: u128,
}

impl RelationLedger {
    fn view(&self) -> RelationLedgerView {
        RelationLedgerView {
            identity: self.identity,
            residual_settlement: self.domain.policy.residual_settlement,
            accepted_volume: self.accepted_volume,
            settled_volume: self.settled_volume,
            slice_count: self.pairing.len,
            settled_by_order: self.settled_by_order,
            settled_by_slice: self.settled_by_slice,
            rounding_pot_price_units: self.rounding_pot_price_units,
        }
    }

    fn slice(&self, index: u16) -> Result<PairingSliceV1, ModelError> {
        if index >= self.pairing.len {
            return Err(ModelError::UnknownSlice);
        }
        Ok(self.pairing.slices[index as usize])
    }

    fn slice_remaining(&self, index: u16) -> Result<Amount, ModelError> {
        let slice = self.slice(index)?;
        slice
            .quantity
            .checked_sub(self.settled_by_slice[index as usize])
            .ok_or(ModelError::SliceExceeded)
    }

    /// The bound order indices and outcome of one frozen slice, refusing any
    /// slice that names a virtual split or merge node.  This model does not
    /// host the §14.3 pot, and it refuses such a slice rather than inventing a
    /// counterparty for it.
    fn slice_pair(&self, index: u16) -> Result<(u8, u8, u8), ModelError> {
        let slice = self.slice(index)?;
        match (slice.buy_ref, slice.sell_ref) {
            (LegRefV1::Order(buy), LegRefV1::Order(sell)) => Ok((buy, sell, slice.outcome)),
            _ => Err(ModelError::VirtualLegNotHosted),
        }
    }

    /// Every frozen slice carrying one executable pair, ascending, with the
    /// pair's remaining quantity derived from the same cumulative ledger the
    /// slice-addressed variants consume.  There is no second pair table.
    fn pair_plan(&self, bound: (u8, u8, u8)) -> Result<DrawPlan, ModelError> {
        let mut plan = DrawPlan {
            slices: [0u16; MAX_SLICES],
            count: 0,
            capacity: 0,
        };
        let mut index = 0u16;
        while index < self.pairing.len {
            let slice = self.pairing.slices[usize::from(index)];
            if let (LegRefV1::Order(buy), LegRefV1::Order(sell)) = (slice.buy_ref, slice.sell_ref) {
                if (buy, sell, slice.outcome) == bound {
                    plan.slices[plan.count] = index;
                    plan.count += 1;
                    plan.capacity = plan
                        .capacity
                        .checked_add(self.slice_remaining(index)?)
                        .ok_or(ModelError::ExceedsPairRemaining)?;
                }
            }
            index += 1;
        }
        if plan.count == 0 {
            return Err(ModelError::UnknownPair);
        }
        Ok(plan)
    }

    /// The single-Egg order at one book index.  This host model's coupled path
    /// admits only single-Egg bindings; a portfolio order has no one-outcome
    /// leg for a settlement receipt to name.
    fn single_egg(&self, index: u8) -> Result<SingleEggOrderV1, ModelError> {
        if usize::from(index) >= usize::from(self.book.len) {
            return Err(ModelError::InvalidFill);
        }
        match self.book.orders[usize::from(index)] {
            OrderV1::SingleEgg(order) => Ok(order),
            OrderV1::Portfolio(_) => Err(ModelError::InvalidFill),
        }
    }
}

/// The frozen slices one receipt may draw on, in canonical (ascending index)
/// order, together with the total quantity still available across them.
struct DrawPlan {
    slices: [u16; MAX_SLICES],
    count: usize,
    capacity: Amount,
}

/// Everything one accepted coupled receipt will write, computed before the
/// first mutation so a refusal cannot leave a partial write behind.
struct SettlementPlan {
    buyer: usize,
    seller: usize,
    phase_policy: TransferPhasePolicy,
    next_buy: Amount,
    next_sell: Amount,
    settled_volume: Amount,
    draws: Vec<(u16, Amount)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    Kernel(clutch_kernel::Error),
    Summary(SummaryError),
    Batch(clutch_batch::Error),
    /// A refusal owned by the coupled relation, reported verbatim.
    Relation(ErrorV1),
    Accounting(AccountingError),
    InvalidOwner,
    InvalidBps,
    InvalidObservationOrder,
    ObservationAfterSeal,
    MaturityExceeded,
    NotMature,
    AlreadySealed,
    MissingConsideration,
    MissingPairBindings,
    PairAlreadySettled,
    InvalidConsideration,
    InsufficientCash,
    TransferInsufficient,
    InvalidFill,
    // Coupled-path refusals.
    /// The frozen relation domain disagrees with this model's shape: outcome
    /// count, owner count, price scale, relation version, or tick table.
    RelationDomainMismatch,
    /// A binding names an order the coupled relation cannot represent: an
    /// out-of-range tick index or a limit above the price scale.
    UnrepresentableBinding,
    /// The best valid submitted candidate creates or destroys complete sets.
    /// This model does not host the §14.3 virtual split/merge pot, so it
    /// refuses before charging anything rather than clearing volume it could
    /// not settle.
    VirtualLegNotHosted,
    /// Two unequal candidates arrived under one host identity.  The digest is
    /// a deterministic host identity, not a commitment, so the collision is
    /// refused rather than resolved.
    RelationIdentityCollision,
    /// No cleared coupled candidate matches this receipt's identity.
    MissingRelationLedger,
    /// The receipt's target kind is not the one the frozen residual variant
    /// admits.
    SettlementTargetNotAdmitted,
    /// The receipt names a slice index the frozen decomposition does not have.
    UnknownSlice,
    /// The receipt would consume more than the named slice still holds.
    SliceExceeded,
    /// The receipt names an executable pair the frozen decomposition never
    /// emitted.
    UnknownPair,
    /// The receipt would consume more than the named pair still holds.
    ExceedsPairRemaining,
    /// Variant 1a admits only whole-slice receipts.
    PartialPairRefused,
    /// The frozen residual variant is recorded but not implemented here.
    ResidualVariantUnimplemented,
}

impl From<clutch_kernel::Error> for ModelError {
    fn from(error: clutch_kernel::Error) -> Self {
        Self::Kernel(error)
    }
}

impl From<SummaryError> for ModelError {
    fn from(error: SummaryError) -> Self {
        Self::Summary(error)
    }
}

impl From<clutch_batch::Error> for ModelError {
    fn from(error: clutch_batch::Error) -> Self {
        Self::Batch(error)
    }
}

impl From<ErrorV1> for ModelError {
    fn from(error: ErrorV1) -> Self {
        Self::Relation(error)
    }
}

impl From<AccountingError> for ModelError {
    fn from(error: AccountingError) -> Self {
        Self::Accounting(error)
    }
}

/// The PROPOSED frozen policy of this model's coupled clearing path, with every
/// one of the eleven variant families named at this single construction site.
///
/// Nothing below is canonized.  Each selection is a reviewable host-fixture
/// choice, and the reason it was made is stated beside it:
///
/// * **A** `PricePriorityMarginalProRata` — strict orders fill whole and the
///   marginal set absorbs the residual, the behaviour the scalar lab already
///   modelled;
/// * **N-a** `RefuseOverlap` — this model's settlement boundary can only move
///   claims between *distinct* owners, so a book in which one owner stands on
///   both sides of one outcome is refused at admission rather than netted;
/// * **2c** `FullSizeCounting` — all-or-none orders stay admissible and are
///   counted at full size, matching the scalar lab's landed behaviour; 2a would
///   make them unrepresentable in this fixture and 2b needs a mask-carrying
///   receipt format this model does not have;
/// * **R-b** `TerminalOwnerFloor` — the price-unit to collateral-atom
///   conversion is recorded per owner.  This model settles cash in exact price
///   units and never draws on the rounding pot; the pot is carried in the
///   ledger so the conversion boundary stays visible;
/// * **T-a/T-b** — supplied by the caller, and the settlement call site derives
///   its `clutch_kernel::TransferPhasePolicy` from it, so the phase gate has
///   exactly one semantic owner;
/// * **P-a** `StrictWholeOrder` — the only implemented portfolio rationing;
/// * `ExplicitSlices` — the frozen decomposition is folded into the candidate
///   digest, which is what makes a receipt's identity bind the slice universe
///   it claims to consume (design §13, variant 1c);
/// * `FeeBaseV1::None` — the model keeps its own landed fee shape (basis points
///   on accepted volume, into `fee_revenue`).  Selecting a relation fee base
///   here would create a second fee owner; the fee-policy redesign is a
///   separate PROPOSED arm.
pub fn proposed_relation_policy(
    residual_settlement: ResidualSettlementV1,
    transfer_phase: TransferPhaseV1,
) -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::RefuseOverlap,
        aon: AonPolicyV1::FullSizeCounting,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement,
        transfer_phase,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

/// Lift one model [`BatchDomain`] into the coupled relation's frozen domain.
///
/// The five identity fields have exactly one source, so the model's ledger key
/// and the relation's domain can never disagree.  Everything else is a PROPOSED
/// fixture parameter of this host model.
pub fn proposed_relation_domain(
    domain: BatchDomain,
    residual_settlement: ResidualSettlementV1,
    transfer_phase: TransferPhaseV1,
) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: domain.market_id,
        book_id: domain.book_id,
        epoch: domain.epoch,
        policy_id: domain.policy_id,
        order_set_id: domain.order_set_id,
        outcome_count: OUTCOMES,
        owner_count: OWNERS as u16,
        price_scale: RELATION_PRICE_SCALE,
        remainder_seed: 7,
        policy: proposed_relation_policy(residual_settlement, transfer_phase),
    }
}

/// The model keying tuple a frozen relation domain projects onto.
pub fn batch_domain_of(domain: &RelationDomainV1) -> BatchDomain {
    BatchDomain::new(
        domain.market_id,
        domain.book_id,
        domain.epoch,
        domain.policy_id,
        domain.order_set_id,
    )
}

/// The kernel phase gate named by a frozen relation phase variant.
///
/// Total and explicit: the frozen policy is the single owner of the choice, and
/// this function is the only place the two vocabularies meet.
pub const fn kernel_phase_policy(phase: TransferPhaseV1) -> TransferPhasePolicy {
    match phase {
        TransferPhaseV1::ActiveOnly => TransferPhasePolicy::ActiveOnly,
        TransferPhaseV1::ActiveOrResolved => TransferPhasePolicy::ActiveOrResolved,
    }
}

/// A complete deterministic host-only composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerticalModel {
    pub market: MarketState,
    pub positions: [Position; OWNERS],
    pub summary: Summary,
    pub accounting: Accounting,
    pub cash: [Amount; OWNERS],
    pub protocol_cash: Amount,
    pub cash_funded: Amount,
    pub fee_bps: u64,
    pub trace: Vec<String>,
    candidate_ledgers: Vec<CandidateLedger>,
    relation_ledgers: Vec<RelationLedger>,
    sealed: bool,
    next_observation_bucket: u64,
}

impl VerticalModel {
    pub const DEFAULT_BATCH_DOMAIN: BatchDomain = BatchDomain::new(1, 1, 1, 1, 1);

    fn transact<T, F>(&mut self, operation: F) -> Result<T, ModelError>
    where
        F: FnOnce(&mut Self) -> Result<T, ModelError>,
    {
        let mut staged = self.clone();
        let value = operation(&mut staged)?;
        staged.check_conservation()?;
        *self = staged;
        Ok(value)
    }

    /// Create a two-outcome market with one-hot payout candidates.
    pub fn create_market(liveness_funding: Amount, fee_bps: u64) -> Result<Self, ModelError> {
        if fee_bps > FEE_BPS_DENOMINATOR {
            return Err(ModelError::InvalidBps);
        }
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut first = [0; MAX_OUTCOMES];
        let mut second = [0; MAX_OUTCOMES];
        first[0] = 1;
        second[1] = 1;
        vectors[0] = PayoutVector::new(1, first);
        vectors[1] = PayoutVector::new(1, second);
        let payouts = PayoutSet::new(2, OUTCOMES, vectors);
        let market = MarketState::new(OUTCOMES, payouts, 0)?;
        let grid = Grid::new(7, 1, 60).map_err(ModelError::Summary)?;
        let mut trace = Vec::new();
        trace.push("market.create outcomes=2 payouts=2 principal=0".to_owned());
        trace.push(format!("market.maturity buckets={MATURITY_BUCKETS}"));
        trace.push(format!("liveness.book reserved={liveness_funding}"));
        Ok(Self {
            market,
            positions: [Position::EMPTY; OWNERS],
            summary: Summary::empty(grid),
            accounting: Accounting::new(liveness_funding),
            cash: [0; OWNERS],
            protocol_cash: 0,
            cash_funded: 0,
            fee_bps,
            trace,
            candidate_ledgers: Vec::new(),
            relation_ledgers: Vec::new(),
            sealed: false,
            next_observation_bucket: 0,
        })
    }

    fn owner(&self, owner: usize) -> Result<(), ModelError> {
        if owner >= OWNERS {
            Err(ModelError::InvalidOwner)
        } else {
            Ok(())
        }
    }

    /// Move one outcome's internal claims between two model owners through the
    /// kernel.
    ///
    /// The kernel carries no owner identity and cannot check that `from` and
    /// `to` are distinct *semantic* parties; that obligation is this model's,
    /// and it is discharged here before the borrow is even taken.  Rust's
    /// borrow rules then make a self-transfer inexpressible.
    fn transfer_claim(
        &mut self,
        from: usize,
        to: usize,
        outcome: u8,
        quantity: Amount,
        phase_policy: TransferPhasePolicy,
    ) -> Result<(), ModelError> {
        self.owner(from)?;
        self.owner(to)?;
        if from == to {
            return Err(ModelError::InvalidFill);
        }
        let split = if from > to { from } else { to };
        let (low, high) = self.positions.split_at_mut(split);
        let (source, destination) = if from < to {
            (&mut low[from], &mut high[0])
        } else {
            (&mut high[0], &mut low[to])
        };
        self.market
            .transfer_internal(source, destination, outcome, quantity, phase_policy)?;
        Ok(())
    }

    pub fn split(&mut self, owner: usize, quantity: Amount) -> Result<(), ModelError> {
        self.transact(|next| next.split_inner(owner, quantity))
    }

    fn split_inner(&mut self, owner: usize, quantity: Amount) -> Result<(), ModelError> {
        self.owner(owner)?;
        self.market.split(&mut self.positions[owner], quantity)?;
        self.accounting.principal = self.market.collateral;
        self.trace
            .push(format!("kernel.split owner={owner} quantity={quantity}"));
        self.check_conservation()?;
        Ok(())
    }

    pub fn materialize(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<(), ModelError> {
        self.transact(|next| next.materialize_inner(owner, outcome, quantity))
    }

    fn materialize_inner(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<(), ModelError> {
        self.owner(owner)?;
        self.market
            .materialize(&mut self.positions[owner], outcome, quantity)?;
        self.trace.push(format!(
            "kernel.materialize owner={owner} outcome={outcome} quantity={quantity}"
        ));
        self.check_conservation()?;
        Ok(())
    }

    pub fn dematerialize(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<(), ModelError> {
        self.transact(|next| next.dematerialize_inner(owner, outcome, quantity))
    }

    fn dematerialize_inner(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<(), ModelError> {
        self.owner(owner)?;
        self.market
            .dematerialize(&mut self.positions[owner], outcome, quantity)?;
        self.trace.push(format!(
            "kernel.dematerialize owner={owner} outcome={outcome} quantity={quantity}"
        ));
        self.check_conservation()?;
        Ok(())
    }

    pub fn merge(&mut self, owner: usize, quantity: Amount) -> Result<(), ModelError> {
        self.transact(|next| next.merge_inner(owner, quantity))
    }

    fn merge_inner(&mut self, owner: usize, quantity: Amount) -> Result<(), ModelError> {
        self.owner(owner)?;
        self.market.merge(&mut self.positions[owner], quantity)?;
        self.accounting.principal = self.market.collateral;
        self.trace
            .push(format!("kernel.merge owner={owner} quantity={quantity}"));
        self.check_conservation()?;
        Ok(())
    }

    /// Fund a host cash balance for exact batch consideration tests.
    pub fn fund_cash(&mut self, owner: usize, amount: Amount) -> Result<(), ModelError> {
        self.transact(|next| next.fund_cash_inner(owner, amount))
    }

    fn fund_cash_inner(&mut self, owner: usize, amount: Amount) -> Result<(), ModelError> {
        self.owner(owner)?;
        let balance = self.cash[owner]
            .checked_add(amount)
            .ok_or(ModelError::InsufficientCash)?;
        let funded = self
            .cash_funded
            .checked_add(amount)
            .ok_or(ModelError::InsufficientCash)?;
        self.cash[owner] = balance;
        self.cash_funded = funded;
        self.trace
            .push(format!("cash.fund owner={owner} amount={amount}"));
        self.check_conservation()?;
        Ok(())
    }

    /// Append exactly one canonical bucket. Missing buckets are explicit.
    pub fn observe(&mut self, observation: Observation) -> Result<(), ModelError> {
        self.transact(|next| next.observe_inner(observation))
    }

    fn observe_inner(&mut self, observation: Observation) -> Result<(), ModelError> {
        if self.sealed {
            return Err(ModelError::ObservationAfterSeal);
        }
        if self.next_observation_bucket >= MATURITY_BUCKETS {
            return Err(ModelError::MaturityExceeded);
        }
        if observation.bucket() != self.next_observation_bucket {
            return Err(ModelError::InvalidObservationOrder);
        }
        self.summary = self.summary.append(observation)?;
        self.next_observation_bucket = self
            .next_observation_bucket
            .checked_add(1)
            .ok_or(ModelError::InvalidObservationOrder)?;
        self.trace.push(match observation {
            Observation::Accepted { bucket, value } => format!(
                "accumulator.accept bucket={bucket} low={} high={}",
                value.low(),
                value.high()
            ),
            Observation::Missing { bucket } => format!("accumulator.missing bucket={bucket}"),
        });
        Ok(())
    }

    /// Seal the frozen observation window once every maturity bucket has been
    /// explicitly represented (accepted or missing).
    pub fn seal_observations(&mut self) -> Result<(), ModelError> {
        self.transact(|next| next.seal_observations_inner())
    }

    fn seal_observations_inner(&mut self) -> Result<(), ModelError> {
        if self.sealed {
            return Err(ModelError::AlreadySealed);
        }
        if self.next_observation_bucket < MATURITY_BUCKETS {
            return Err(ModelError::NotMature);
        }
        self.sealed = true;
        self.trace
            .push(format!("accumulator.seal end_bucket={MATURITY_BUCKETS}"));
        Ok(())
    }

    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Resolve only if the sealed summary has complete coverage and its
    /// terminal interval lies wholly inside one frozen binary partition cell.
    pub fn resolve_from_summary(&mut self) -> Result<ResolveDecision, ModelError> {
        self.transact(|next| next.resolve_from_summary_inner())
    }

    fn resolve_from_summary_inner(&mut self) -> Result<ResolveDecision, ModelError> {
        if self.next_observation_bucket < MATURITY_BUCKETS {
            self.trace
                .push("resolve.refuse reason=not_mature".to_owned());
            return Ok(ResolveDecision::Refused(Refusal::NotMature));
        }
        if !self.sealed {
            self.trace
                .push("resolve.refuse reason=not_sealed".to_owned());
            return Ok(ResolveDecision::Refused(Refusal::NotSealed));
        }
        let coverage = self.summary.coverage();
        if coverage.state() == CoverageState::Gapped {
            self.trace
                .push("resolve.refuse reason=gapped_coverage".to_owned());
            return Ok(ResolveDecision::Refused(Refusal::GappedCoverage));
        }
        let terminal = match self.summary.terminal() {
            Ok(value) => value,
            Err(StatisticError::NoAcceptedCoverage) => {
                self.trace
                    .push("resolve.refuse reason=no_coverage".to_owned());
                return Ok(ResolveDecision::Refused(Refusal::NoAcceptedCoverage));
            }
            Err(_) => return Ok(ResolveDecision::Refused(Refusal::UnsupportedStatistic)),
        };
        let index = if terminal.high() < RESOLUTION_THRESHOLD {
            0
        } else if terminal.low() >= RESOLUTION_THRESHOLD {
            1
        } else {
            self.trace
                .push("resolve.refuse reason=ambiguous_terminal_interval".to_owned());
            return Ok(ResolveDecision::Refused(Refusal::AmbiguousTerminalInterval));
        };
        self.market.resolve(index)?;
        self.trace
            .push(format!("kernel.resolve payout_index={index}"));
        self.check_conservation()?;
        Ok(ResolveDecision::Resolved(index))
    }

    pub fn redeem_internal(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.transact(|next| next.redeem_internal_inner(owner, outcome, quantity))
    }

    fn redeem_internal_inner(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.owner(owner)?;
        let payout = self
            .market
            .redeem_internal(&mut self.positions[owner], outcome, quantity)?;
        self.accounting.principal = self.market.collateral;
        self.trace.push(format!(
            "kernel.redeem_internal owner={owner} outcome={outcome} quantity={quantity} payout={payout}"
        ));
        self.check_conservation()?;
        Ok(payout)
    }

    pub fn redeem_external(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.transact(|next| next.redeem_external_inner(owner, outcome, quantity))
    }

    fn redeem_external_inner(
        &mut self,
        owner: usize,
        outcome: u8,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.owner(owner)?;
        let payout = self
            .market
            .redeem_external(&mut self.positions[owner], outcome, quantity)?;
        self.accounting.principal = self.market.collateral;
        self.trace.push(format!(
            "kernel.redeem_external owner={owner} outcome={outcome} quantity={quantity} payout={payout}"
        ));
        self.check_conservation()?;
        Ok(payout)
    }

    /// The old claim-only settlement entry point is intentionally refused:
    /// this component cannot settle a batch without an explicit cash
    /// consideration leg.
    pub fn settle_batch_fill(
        &mut self,
        _candidate: &Candidate,
        _order_index: usize,
        _seller: usize,
        _buyer: usize,
        _outcome: u8,
        _quantity: Amount,
    ) -> Result<(), ModelError> {
        Err(ModelError::MissingConsideration)
    }

    /// Apply one atomic matched buy/sell transfer plus exact cash
    /// consideration. Unbound generic books refuse here because they do not
    /// carry authenticated side, owner, and outcome semantics.
    pub fn settle_batch_fill_with_consideration(
        &mut self,
        receipt: &SettlementReceipt,
    ) -> Result<(), ModelError> {
        let mut staged = self.clone();
        staged.settle_batch_fill_with_consideration_inner(receipt)?;
        staged.check_conservation()?;
        *self = staged;
        Ok(())
    }

    fn settle_batch_fill_with_consideration_inner(
        &mut self,
        receipt: &SettlementReceipt,
    ) -> Result<(), ModelError> {
        let domain = receipt.identity.domain;
        let candidate = receipt.identity.candidate;
        let buy_index = usize::from(receipt.buy_order_index);
        let sell_index = usize::from(receipt.sell_order_index);
        let quantity = receipt.quantity;
        let outcome = receipt.outcome;
        let consideration = receipt.consideration;
        if buy_index == sell_index
            || buy_index >= usize::from(candidate.len)
            || sell_index >= usize::from(candidate.len)
            || quantity == 0
        {
            return Err(ModelError::InvalidFill);
        }
        if quantity > candidate.fills[buy_index]
            || quantity > candidate.fills[sell_index]
            || usize::from(candidate.len) > MAX_ORDERS
        {
            return Err(ModelError::InvalidFill);
        }
        let ledger_index = self
            .candidate_ledgers
            .iter()
            .position(|entry| entry.identity == (CandidateIdentity { domain, candidate }))
            .ok_or(ModelError::InvalidFill)?;
        let ledger = &self.candidate_ledgers[ledger_index];
        if !ledger.has_bindings {
            return Err(ModelError::MissingPairBindings);
        }
        let buy = ledger.orders[buy_index];
        let sell = ledger.orders[sell_index];
        if buy.order.canonical_order_id != receipt.buy_order_id
            || sell.order.canonical_order_id != receipt.sell_order_id
            || buy.order.side != Side::Buy
            || sell.order.side != Side::Sell
            || buy.owner == sell.owner
            || buy.outcome != sell.outcome
            || buy.outcome != outcome
        {
            return Err(ModelError::InvalidFill);
        }
        let buy_settled = ledger.settled_by_order[buy_index];
        let sell_settled = ledger.settled_by_order[sell_index];
        let expected_consideration = ledger
            .clearing_price
            .checked_mul(quantity)
            .ok_or(ModelError::InvalidConsideration)?;
        if consideration != expected_consideration {
            return Err(ModelError::InvalidConsideration);
        }
        if ledger
            .settled_pairs
            .contains(&(receipt.buy_order_index, receipt.sell_order_index))
        {
            return Err(ModelError::PairAlreadySettled);
        }
        let next_buy_settled = buy_settled
            .checked_add(quantity)
            .ok_or(ModelError::InvalidFill)?;
        let next_sell_settled = sell_settled
            .checked_add(quantity)
            .ok_or(ModelError::InvalidFill)?;
        if next_buy_settled > candidate.fills[buy_index]
            || next_sell_settled > candidate.fills[sell_index]
        {
            return Err(ModelError::InvalidFill);
        }
        let buyer = buy.owner;
        let seller = sell.owner;
        self.owner(buyer)?;
        self.owner(seller)?;
        // The kernel owns claim movement.  `transfer_internal` takes `&self` on
        // the market, so supply and collateral neutrality is structural here
        // rather than a property this model has to re-argue.
        //
        // PROPOSED phase selection, pending policy freeze: `ActiveOnly` (design
        // §14.2 variant T-a).  The scalar path settles strictly before
        // resolution, so refusing a transfer that races resolution is the
        // conservative reading; T-b is the liveness-favouring alternative and
        // is not selected here.  The choice is named at the call site precisely
        // because the kernel holds no default opinion.
        self.transfer_claim(
            seller,
            buyer,
            outcome,
            quantity,
            TransferPhasePolicy::ActiveOnly,
        )?;
        if self.cash[buyer] < consideration {
            return Err(ModelError::InsufficientCash);
        }
        let seller_cash = self.cash[seller]
            .checked_add(consideration)
            .ok_or(ModelError::InsufficientCash)?;
        self.cash[buyer] -= consideration;
        self.cash[seller] = seller_cash;
        self.trace.push(format!(
            "batch.fill buy_index={} buy_id={} sell_index={} sell_id={} seller={seller} buyer={buyer} outcome={outcome} quantity={quantity} consideration={consideration}",
            receipt.buy_order_index,
            receipt.buy_order_id,
            receipt.sell_order_index,
            receipt.sell_order_id
        ));
        let ledger = self
            .candidate_ledgers
            .iter_mut()
            .find(|entry| entry.identity == (CandidateIdentity { domain, candidate }))
            .ok_or(ModelError::InvalidFill)?;
        ledger.settled_by_order[buy_index] = next_buy_settled;
        ledger.settled_by_order[sell_index] = next_sell_settled;
        ledger
            .settled_pairs
            .push((receipt.buy_order_index, receipt.sell_order_index));
        self.check_conservation()?;
        Ok(())
    }

    /// Evaluate the exact interval TWAP exposed by the accumulator summary.
    pub fn twap(&self) -> Result<clutch_accumulator::RatioInterval, StatisticError> {
        self.summary.twap()
    }

    /// Construct and verify a generic fixed-grid batch, then account its fee.
    /// The batch relation does not mutate claim balances; settlement remains an
    /// explicit adapter operation outside these three semantic crates.
    pub fn clear_batch(
        &mut self,
        orders: &[Order],
        liveness_cost: Amount,
    ) -> Result<Candidate, ModelError> {
        self.clear_batch_in_domain(Self::DEFAULT_BATCH_DOMAIN, orders, liveness_cost)
    }

    /// Construct a candidate whose order sides, owners, and outcomes are
    /// bound for atomic matched-pair settlement.
    pub fn clear_batch_with_bindings(
        &mut self,
        domain: BatchDomain,
        bindings: &[BoundOrder],
        liveness_cost: Amount,
    ) -> Result<Candidate, ModelError> {
        if bindings.len() > MAX_ORDERS {
            return Err(ModelError::Batch(clutch_batch::Error::TooManyOrders));
        }
        let mut orders = [empty_order(); MAX_ORDERS];
        let mut bound = [empty_bound_order(); MAX_ORDERS];
        for i in 0..bindings.len() {
            orders[i] = bindings[i].order;
            bound[i] = bindings[i];
        }
        let used = bindings.len();
        self.transact(|next| {
            next.clear_batch_inner(domain, &orders[..used], Some(bound), liveness_cost)
        })
    }

    /// Construct one candidate under an explicit market/book/epoch/policy
    /// and canonical order-set domain.
    pub fn clear_batch_in_domain(
        &mut self,
        domain: BatchDomain,
        orders: &[Order],
        liveness_cost: Amount,
    ) -> Result<Candidate, ModelError> {
        self.transact(|next| next.clear_batch_inner(domain, orders, None, liveness_cost))
    }

    fn clear_batch_inner(
        &mut self,
        domain: BatchDomain,
        orders: &[Order],
        bindings: Option<[BoundOrder; MAX_ORDERS]>,
        liveness_cost: Amount,
    ) -> Result<Candidate, ModelError> {
        if orders.len() > MAX_ORDERS {
            return Err(ModelError::Batch(clutch_batch::Error::TooManyOrders));
        }
        let grid = model_price_grid()?;
        let policy = FrozenPolicy::new(
            grid,
            TieRule::MaxVolumeMinImbalanceHighestTick,
            DustPolicy::AssignCanonical,
            7,
        )?;
        let mut fixed = [Order {
            canonical_order_id: 0,
            side: Side::Buy,
            limit_tick: 0,
            quantity: 0,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
        }; MAX_ORDERS];
        fixed[..orders.len()].copy_from_slice(orders);
        let book = FixedBook::new(policy, fixed, orders.len() as u8)?;
        let candidate = book.propose()?;
        book.verify(&candidate)?;
        let identity = CandidateIdentity { domain, candidate };
        // Replaying the same verified candidate is an idempotent retry.  Do
        // not charge its fee or liveness work twice and keep one settlement
        // ledger for cumulative per-order ceilings.
        if self
            .candidate_ledgers
            .iter()
            .any(|entry| entry.identity == identity)
        {
            return Ok(candidate);
        }
        let fee = candidate
            .matched
            .checked_mul(self.fee_bps)
            .ok_or(ModelError::Accounting(AccountingError::Overflow))?
            / FEE_BPS_DENOMINATOR;
        self.accounting
            .fee_revenue
            .checked_add(fee)
            .ok_or(ModelError::Accounting(AccountingError::Overflow))?;
        self.accounting.pay_liveness(liveness_cost)?;
        self.accounting.add_fee(fee)?;
        let has_bindings = bindings.is_some();
        let stored_orders = match bindings {
            Some(value) => value,
            None => {
                let mut result = [empty_bound_order(); MAX_ORDERS];
                for i in 0..fixed.len() {
                    result[i].order = fixed[i];
                }
                result
            }
        };
        self.candidate_ledgers.push(CandidateLedger {
            identity,
            orders: stored_orders,
            has_bindings,
            clearing_price: MODEL_PRICE_TICKS[usize::from(candidate.clearing_tick)],
            settled_by_order: [0; MAX_ORDERS],
            settled_pairs: Vec::new(),
        });
        self.trace.push(format!(
            "batch.clear tick={} matched={} fee={} liveness_paid={liveness_cost}",
            candidate.clearing_tick, candidate.matched, fee
        ));
        Ok(candidate)
    }

    /// Lower one bound book onto the coupled relation's frozen [`BookV1`].
    ///
    /// Every `BoundOrder` field has exactly one destination: the tick index
    /// becomes a scaled limit price off the one model tick table, and the
    /// owner, outcome, and expiry the host boundary authenticates become the
    /// relation's own bound coordinates.  Nothing is invented and nothing is
    /// dropped.
    pub fn relation_book(
        domain: &RelationDomainV1,
        bindings: &[BoundOrder],
    ) -> Result<BookV1, ModelError> {
        if domain.relation_version != RELATION_VERSION_V1
            || domain.outcome_count != OUTCOMES
            || domain.owner_count as usize != OWNERS
            || domain.price_scale != RELATION_PRICE_SCALE
        {
            return Err(ModelError::RelationDomainMismatch);
        }
        if bindings.len() > MAX_ORDERS {
            return Err(ModelError::Relation(ErrorV1::TooManyOrders));
        }
        let mut book = BookV1::empty();
        for (index, binding) in bindings.iter().enumerate() {
            if binding.owner >= OWNERS {
                return Err(ModelError::InvalidOwner);
            }
            let tick = usize::from(binding.order.limit_tick);
            if tick >= MODEL_PRICE_TICKS.len() {
                return Err(ModelError::UnrepresentableBinding);
            }
            let limit_price = MODEL_PRICE_TICKS[tick];
            if limit_price > domain.price_scale {
                return Err(ModelError::UnrepresentableBinding);
            }
            book.orders[index] = OrderV1::SingleEgg(SingleEggOrderV1 {
                canonical_order_id: binding.order.canonical_order_id,
                owner: binding.owner as u16,
                outcome: binding.outcome,
                side: binding.order.side,
                quantity: binding.order.quantity,
                limit_price,
                minimum_fill: binding.order.minimum_fill,
                partial_policy: binding.order.partial_policy,
                expiry_epoch: binding.expiry_epoch,
            });
        }
        book.len = bindings.len() as u8;
        book.validate(domain)?;
        Ok(book)
    }

    /// Clear one batch through the **coupled** relation and freeze its
    /// settlement decomposition.
    ///
    /// This is a parallel entry point, not a replacement: the scalar
    /// [`VerticalModel::clear_batch_with_bindings`] path and its permanent
    /// regressions are untouched.  What changes is who owns the pairing.  The
    /// scalar path hands the model a matched *volume* and lets the model choose
    /// counterparties; here the relation returns a
    /// [`PairingWitnessV1`] whose slices are the only settlement universe that
    /// exists, and the model keeps no pairing opinion of its own.
    ///
    /// The returned candidate is the **best valid submitted candidate** of the
    /// bounded box `bounds`, never an optimum.
    ///
    /// Every admission, feasibility, conservation, and pairing refusal happens
    /// before the first fee or liveness charge, and the accepted volume the fee
    /// is charged on is the relation's own recomputed direct flow — so the
    /// scalar relation's charge-then-refuse defect (adversarial review §P1-B)
    /// has no expressible form on this path.
    pub fn clear_relation_v1(
        &mut self,
        domain: RelationDomainV1,
        bindings: &[BoundOrder],
        bounds: SearchBoundsV1,
        liveness_cost: Amount,
    ) -> Result<RelationClearing, ModelError> {
        self.transact(|next| next.clear_relation_v1_inner(domain, bindings, bounds, liveness_cost))
    }

    fn clear_relation_v1_inner(
        &mut self,
        domain: RelationDomainV1,
        bindings: &[BoundOrder],
        bounds: SearchBoundsV1,
        liveness_cost: Amount,
    ) -> Result<RelationClearing, ModelError> {
        if domain.outcome_count != self.market.outcomes {
            return Err(ModelError::RelationDomainMismatch);
        }
        // Refused here, before anything is charged, and not at settlement:
        // clearing a batch whose frozen residual variant this model cannot
        // settle would be exactly the charge-then-refuse shape the coupled
        // relation exists to make impossible.  1b-free needs a terminal sweep
        // authority for its documented strand hazard, and none exists here.
        if domain.policy.residual_settlement == ResidualSettlementV1::CumulativePairFree {
            return Err(ModelError::ResidualVariantUnimplemented);
        }
        let book = Self::relation_book(&domain, bindings)?;
        // The constructor searches; the relation accepts.  `propose_best_valid`
        // round-trips every coordinate it visits through `verify` before it can
        // compare it, so nothing reaches this line that the relation has not
        // already accepted once.
        let candidate = relation::propose_best_valid(&domain, &book, &bounds)?;
        if candidate.virtual_split != 0 || candidate.virtual_merge != 0 {
            // No §14.3 pot exists in this host model, so complete-set churn has
            // no position to move through.  Refuse before any charge rather
            // than clear volume that could never settle.
            return Err(ModelError::VirtualLegNotHosted);
        }
        let pairing = relation::canonical_pairing(&domain, &book, &candidate)?;
        // Independent recomputation, not a re-read of a claimed aggregate: the
        // frozen policy names `ExplicitSlices`, so `verify` recomputes the
        // digest over the decomposition and refuses if the witness this model
        // reconstructed is not the one the candidate bound.
        let summary = relation::verify(&domain, &book, &candidate, Some(&pairing))?;
        relation::verify_pairing_witness(&domain, &book, &candidate, &pairing)?;
        let mut slice = 0u16;
        while slice < pairing.len {
            let entry = pairing.slices[slice as usize];
            if !matches!(entry.buy_ref, LegRefV1::Order(_))
                || !matches!(entry.sell_ref, LegRefV1::Order(_))
            {
                return Err(ModelError::VirtualLegNotHosted);
            }
            slice += 1;
        }
        let mut accepted_volume: Amount = 0;
        let mut outcome = 0usize;
        while outcome < usize::from(domain.outcome_count) {
            accepted_volume = accepted_volume
                .checked_add(summary.direct_flow[outcome])
                .ok_or(ModelError::Accounting(AccountingError::Overflow))?;
            outcome += 1;
        }
        let identity = RelationCandidateIdentity {
            domain: batch_domain_of(&domain),
            candidate_digest: summary.candidate_digest,
        };
        // Replaying the same cleared candidate is an idempotent retry: no
        // second fee, no second liveness charge, no second trace event, and one
        // settlement ledger.
        if let Some(existing) = self
            .relation_ledgers
            .iter()
            .find(|entry| entry.identity == identity)
        {
            if existing.candidate != candidate
                || existing.pairing != pairing
                || existing.domain != domain
                || existing.book != book
            {
                return Err(ModelError::RelationIdentityCollision);
            }
            return Ok(RelationClearing {
                identity,
                summary,
                accepted_volume: existing.accepted_volume,
                fee: 0,
                slice_count: existing.pairing.len,
                replayed: true,
            });
        }
        // The landed fee shape, charged on volume the relation accepted and the
        // frozen decomposition pairs.  Fee revenue is never collateral and
        // never liveness capitalization.
        let fee = accepted_volume
            .checked_mul(self.fee_bps)
            .ok_or(ModelError::Accounting(AccountingError::Overflow))?
            / FEE_BPS_DENOMINATOR;
        self.accounting.pay_liveness(liveness_cost)?;
        self.accounting.add_fee(fee)?;
        self.relation_ledgers.push(RelationLedger {
            identity,
            domain,
            book,
            candidate,
            pairing,
            accepted_volume,
            settled_volume: 0,
            settled_by_order: [0; MAX_ORDERS],
            settled_by_slice: [0; MAX_SLICES],
            rounding_pot_price_units: summary.rounding_pot_price_units,
        });
        self.trace.push(format!(
            "relation.clear digest={:#034x} volume={accepted_volume} fee={fee} liveness_paid={liveness_cost} slices={} residual={:?} phase={:?} rounding_pot={}",
            summary.candidate_digest,
            pairing.len,
            domain.policy.residual_settlement,
            domain.policy.transfer_phase,
            summary.rounding_pot_price_units,
        ));
        Ok(RelationClearing {
            identity,
            summary,
            accepted_volume,
            fee,
            slice_count: pairing.len,
            replayed: false,
        })
    }

    /// Settle one coupled-relation receipt against the frozen decomposition.
    ///
    /// The residual-pair variant is read from the frozen policy the candidate's
    /// digest already binds, so there is exactly one place the choice lives and
    /// no call site can select a different one after the fact:
    ///
    /// * **1a** [`ResidualSettlementV1::FullPairOnly`] — a receipt names a
    ///   slice and must consume it whole, exactly once;
    /// * **1b** [`ResidualSettlementV1::CumulativePairCanonical`] — a receipt
    ///   names an executable *pair* and draws any quantity up to what the
    ///   frozen slices carrying that pair still hold;
    /// * **1c** [`ResidualSettlementV1::UniqueSliceReceipts`] — a receipt names
    ///   a slice and draws any quantity up to that slice's residue.
    ///
    /// [`ResidualSettlementV1::CumulativePairFree`] is recorded by the relation
    /// but refused here: its documented strand hazard needs a terminal sweep
    /// authority that this model does not have, and implementing it without one
    /// would be a silent acceptance of strandable residue.
    pub fn settle_relation_receipt(&mut self, receipt: &RelationReceipt) -> Result<(), ModelError> {
        self.transact(|next| next.settle_relation_receipt_inner(receipt))
    }

    fn settle_relation_receipt_inner(
        &mut self,
        receipt: &RelationReceipt,
    ) -> Result<(), ModelError> {
        let ledger_index = self
            .relation_ledgers
            .iter()
            .position(|entry| entry.identity == receipt.identity)
            .ok_or(ModelError::MissingRelationLedger)?;
        let plan = self.relation_settlement_plan(ledger_index, receipt)?;
        self.transfer_claim(
            plan.seller,
            plan.buyer,
            receipt.outcome,
            receipt.quantity,
            plan.phase_policy,
        )?;
        if self.cash[plan.buyer] < receipt.consideration {
            return Err(ModelError::InsufficientCash);
        }
        let seller_cash = self.cash[plan.seller]
            .checked_add(receipt.consideration)
            .ok_or(ModelError::InsufficientCash)?;
        self.cash[plan.buyer] -= receipt.consideration;
        self.cash[plan.seller] = seller_cash;
        let target = match receipt.target {
            SettlementTarget::Slice(index) => format!("slice:{index}"),
            SettlementTarget::Pair => "pair".to_owned(),
        };
        self.trace.push(format!(
            "relation.settle target={target} buy_index={} buy_id={} sell_index={} sell_id={} seller={} buyer={} outcome={} quantity={} consideration={}",
            receipt.buy_order_index,
            receipt.buy_order_id,
            receipt.sell_order_index,
            receipt.sell_order_id,
            plan.seller,
            plan.buyer,
            receipt.outcome,
            receipt.quantity,
            receipt.consideration,
        ));
        let ledger = &mut self.relation_ledgers[ledger_index];
        ledger.settled_by_order[usize::from(receipt.buy_order_index)] = plan.next_buy;
        ledger.settled_by_order[usize::from(receipt.sell_order_index)] = plan.next_sell;
        for (index, quantity) in &plan.draws {
            let slot = &mut ledger.settled_by_slice[usize::from(*index)];
            *slot = slot
                .checked_add(*quantity)
                .ok_or(ModelError::SliceExceeded)?;
        }
        ledger.settled_volume = plan.settled_volume;
        self.check_conservation()?;
        Ok(())
    }

    /// Every check a coupled receipt must pass, computed against an immutable
    /// ledger before the first write.
    fn relation_settlement_plan(
        &self,
        ledger_index: usize,
        receipt: &RelationReceipt,
    ) -> Result<SettlementPlan, ModelError> {
        let ledger = &self.relation_ledgers[ledger_index];
        if receipt.quantity == 0 {
            return Err(ModelError::InvalidFill);
        }
        let variant = ledger.domain.policy.residual_settlement;
        let (bound, plan, exceeded) = match (variant, receipt.target) {
            (ResidualSettlementV1::CumulativePairFree, _) => {
                return Err(ModelError::ResidualVariantUnimplemented)
            }
            (
                ResidualSettlementV1::FullPairOnly | ResidualSettlementV1::UniqueSliceReceipts,
                SettlementTarget::Slice(index),
            ) => {
                let slice = ledger.slice(index)?;
                let bound = ledger.slice_pair(index)?;
                if variant == ResidualSettlementV1::FullPairOnly {
                    if ledger.settled_by_slice[usize::from(index)] != 0 {
                        return Err(ModelError::PairAlreadySettled);
                    }
                    if receipt.quantity != slice.quantity {
                        return Err(ModelError::PartialPairRefused);
                    }
                }
                let mut slices = [0u16; MAX_SLICES];
                slices[0] = index;
                (
                    bound,
                    DrawPlan {
                        slices,
                        count: 1,
                        capacity: ledger.slice_remaining(index)?,
                    },
                    ModelError::SliceExceeded,
                )
            }
            (ResidualSettlementV1::CumulativePairCanonical, SettlementTarget::Pair) => {
                let bound = (
                    receipt.buy_order_index,
                    receipt.sell_order_index,
                    receipt.outcome,
                );
                (
                    bound,
                    ledger.pair_plan(bound)?,
                    ModelError::ExceedsPairRemaining,
                )
            }
            _ => return Err(ModelError::SettlementTargetNotAdmitted),
        };
        // The receipt's own bound coordinates must be the ones the frozen
        // decomposition names.  For a pair target these agree by construction;
        // for a slice target this is the check that stops a receipt from
        // relabelling somebody else's slice.
        if bound
            != (
                receipt.buy_order_index,
                receipt.sell_order_index,
                receipt.outcome,
            )
        {
            return Err(ModelError::InvalidFill);
        }
        let buy = ledger.single_egg(bound.0)?;
        let sell = ledger.single_egg(bound.1)?;
        if buy.canonical_order_id != receipt.buy_order_id
            || sell.canonical_order_id != receipt.sell_order_id
            || buy.side != Side::Buy
            || sell.side != Side::Sell
            || buy.outcome != receipt.outcome
            || sell.outcome != receipt.outcome
            || buy.owner == sell.owner
        {
            return Err(ModelError::InvalidFill);
        }
        let price = ledger.candidate.prices[usize::from(receipt.outcome)];
        let expected = receipt
            .quantity
            .checked_mul(price)
            .ok_or(ModelError::InvalidConsideration)?;
        if receipt.consideration != expected {
            return Err(ModelError::InvalidConsideration);
        }
        // Residue first: exhausting the named slice or pair is the specific
        // statement about this refusal, and the §13 falsifiers name it.
        if receipt.quantity > plan.capacity {
            return Err(exceeded);
        }
        let mut draws = Vec::new();
        let mut left = receipt.quantity;
        let mut k = 0usize;
        while k < plan.count && left != 0 {
            let index = plan.slices[k];
            let available = ledger.slice_remaining(index)?;
            let take = if available < left { available } else { left };
            if take != 0 {
                draws.push((index, take));
                left -= take;
            }
            k += 1;
        }
        if left != 0 {
            return Err(exceeded);
        }
        // The cumulative per-order ceiling is retained verbatim from the scalar
        // path: no sequence of receipts can consume more of an order than the
        // verified candidate filled.  The frozen slices already sum exactly to
        // the fills, so the slice ledger above makes this ceiling unreachable
        // rather than redundant-but-live; it is kept as the backstop that would
        // catch a decomposition that did not.
        let next_buy = ledger.settled_by_order[usize::from(bound.0)]
            .checked_add(receipt.quantity)
            .ok_or(ModelError::InvalidFill)?;
        let next_sell = ledger.settled_by_order[usize::from(bound.1)]
            .checked_add(receipt.quantity)
            .ok_or(ModelError::InvalidFill)?;
        if next_buy > ledger.candidate.fills[usize::from(bound.0)]
            || next_sell > ledger.candidate.fills[usize::from(bound.1)]
        {
            return Err(ModelError::InvalidFill);
        }
        let settled_volume = ledger
            .settled_volume
            .checked_add(receipt.quantity)
            .ok_or(ModelError::InvalidFill)?;
        if settled_volume > ledger.accepted_volume {
            return Err(exceeded);
        }
        Ok(SettlementPlan {
            buyer: usize::from(buy.owner),
            seller: usize::from(sell.owner),
            // One semantic owner for the phase gate: the frozen relation policy
            // names T-a or T-b and this call site names the matching kernel
            // policy.  Both remain PROPOSED pending policy freeze.
            phase_policy: kernel_phase_policy(ledger.domain.policy.transfer_phase),
            next_buy,
            next_sell,
            settled_volume,
            draws,
        })
    }

    /// A read-only projection of one cleared coupled ledger.
    pub fn relation_ledger(
        &self,
        identity: &RelationCandidateIdentity,
    ) -> Option<RelationLedgerView> {
        self.relation_ledgers
            .iter()
            .find(|entry| &entry.identity == identity)
            .map(RelationLedger::view)
    }

    /// The frozen slice universe of one cleared coupled candidate.
    pub fn relation_slices(
        &self,
        identity: &RelationCandidateIdentity,
    ) -> Option<Vec<PairingSliceV1>> {
        self.relation_ledgers
            .iter()
            .find(|entry| &entry.identity == identity)
            .map(|entry| entry.pairing.slices[..usize::from(entry.pairing.len)].to_vec())
    }

    /// The verified candidate of one cleared coupled ledger.
    pub fn relation_candidate(&self, identity: &RelationCandidateIdentity) -> Option<CandidateV1> {
        self.relation_ledgers
            .iter()
            .find(|entry| &entry.identity == identity)
            .map(|entry| entry.candidate)
    }

    /// Redeem `quantity` complete sets in the Resolved phase.
    ///
    /// The kernel's unconditional terminal exit: a holder of one unit of every
    /// outcome never remainders, even where a single-outcome redemption refuses
    /// forever under a fractional payout vector.
    pub fn redeem_complete_set(
        &mut self,
        owner: usize,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.transact(|next| next.redeem_complete_set_inner(owner, quantity))
    }

    fn redeem_complete_set_inner(
        &mut self,
        owner: usize,
        quantity: Amount,
    ) -> Result<Amount, ModelError> {
        self.owner(owner)?;
        let payout = self
            .market
            .redeem_complete_set(&mut self.positions[owner], quantity)?;
        self.accounting.principal = self.market.collateral;
        self.trace.push(format!(
            "kernel.redeem_complete_set owner={owner} quantity={quantity} payout={payout}"
        ));
        self.check_conservation()?;
        Ok(payout)
    }

    pub fn return_liveness(&mut self, amount: Amount) -> Result<(), ModelError> {
        self.transact(|next| next.return_liveness_inner(amount))
    }

    fn return_liveness_inner(&mut self, amount: Amount) -> Result<(), ModelError> {
        self.accounting.return_liveness(amount)?;
        self.trace.push(format!("liveness.return amount={amount}"));
        Ok(())
    }

    /// Check kernel invariants, aggregate claim supply, and accounting
    /// separation after every successful transition.
    pub fn check_conservation(&self) -> Result<(), ModelError> {
        self.market.check_invariants()?;
        self.accounting.check()?;
        for outcome in 0..usize::from(self.market.outcomes) {
            let mut sum: Amount = 0;
            for position in self.positions {
                sum = sum
                    .checked_add(position.internal[outcome])
                    .and_then(|value| value.checked_add(position.external[outcome]))
                    .ok_or(ModelError::Accounting(AccountingError::Overflow))?;
            }
            if sum != self.market.total_supply[outcome] {
                return Err(ModelError::TransferInsufficient);
            }
        }
        if self.accounting.principal != self.market.collateral {
            return Err(ModelError::Accounting(
                AccountingError::LivenessConservation,
            ));
        }
        let mut cash_total = self.protocol_cash;
        for balance in self.cash {
            cash_total = cash_total
                .checked_add(balance)
                .ok_or(ModelError::InsufficientCash)?;
        }
        if cash_total != self.cash_funded {
            return Err(ModelError::InsufficientCash);
        }
        Ok(())
    }

    pub fn phase(&self) -> Phase {
        self.market.phase
    }
}

/// Deterministic fixture used by the golden trace and integration tests.
pub fn golden_scenario() -> Result<VerticalModel, ModelError> {
    let mut model = VerticalModel::create_market(9, 5_000)?;
    model.split(0, 8)?;
    model.split(1, 4)?;
    model.materialize(0, 1, 2)?;
    model.dematerialize(0, 1, 1)?;
    let bound_orders = golden_bindings();
    let candidate =
        model.clear_batch_with_bindings(VerticalModel::DEFAULT_BATCH_DOMAIN, &bound_orders, 2)?;
    model.fund_cash(0, 30)?;
    let receipt = SettlementReceipt::new(
        CandidateIdentity {
            domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
            candidate,
        },
        0,
        1,
        1,
        2,
        1,
        1,
        30,
    );
    model.settle_batch_fill_with_consideration(&receipt)?;
    model.observe(Observation::accepted(0, 20, 20))?;
    model.observe(Observation::accepted(1, 30, 30))?;
    model.observe(Observation::accepted(2, 70, 70))?;
    model.seal_observations()?;
    model.merge(1, 1)?;
    model.resolve_from_summary()?;
    model.redeem_internal(0, 1, 3)?;
    model.redeem_external(0, 1, 1)?;
    model.return_liveness(7)?;
    model.check_conservation()?;
    Ok(model)
}

/// The two bound orders both golden fixtures clear, so the scalar and coupled
/// traces differ only where the two relations genuinely differ.
fn golden_bindings() -> [BoundOrder; 2] {
    [
        BoundOrder {
            order: Order {
                canonical_order_id: 1,
                side: Side::Buy,
                limit_tick: 2,
                quantity: 5,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            owner: 0,
            outcome: 1,
            expiry_epoch: u64::MAX,
        },
        BoundOrder {
            order: Order {
                canonical_order_id: 2,
                side: Side::Sell,
                limit_tick: 2,
                quantity: 3,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            owner: 1,
            outcome: 1,
            expiry_epoch: u64::MAX,
        },
    ]
}

/// The coupled-relation twin of [`golden_scenario`], pinned by
/// `golden/coupled.trace`.
///
/// It clears the same book, at the same volume, for the same fee, and settles
/// through the relation's own frozen decomposition instead of a model-chosen
/// pair.  `golden/basic.trace` is never rewritten; this is a second trace
/// beside it.
///
/// The residual variant is a required argument with no default: the trace is
/// pinned for [`ResidualSettlementV1::UniqueSliceReceipts`] (1c), and the other
/// variants are exercised by the settlement tests.
pub fn coupled_golden_scenario(
    residual_settlement: ResidualSettlementV1,
) -> Result<VerticalModel, ModelError> {
    let mut model = VerticalModel::create_market(9, 5_000)?;
    model.split(0, 8)?;
    model.split(1, 4)?;
    model.materialize(0, 1, 2)?;
    model.dematerialize(0, 1, 1)?;
    let domain = proposed_relation_domain(
        VerticalModel::DEFAULT_BATCH_DOMAIN,
        residual_settlement,
        // PROPOSED phase selection, pending policy freeze: T-a, matching the
        // scalar path's pre-resolution settlement discipline.
        TransferPhaseV1::ActiveOnly,
    );
    let bindings = golden_bindings();
    let clearing = model.clear_relation_v1(domain, &bindings, PROPOSED_SEARCH_BOUNDS, 2)?;
    model.fund_cash(0, 90)?;
    let slices = model
        .relation_slices(&clearing.identity)
        .ok_or(ModelError::MissingRelationLedger)?;
    let candidate = model
        .relation_candidate(&clearing.identity)
        .ok_or(ModelError::MissingRelationLedger)?;
    for (index, slice) in slices.iter().enumerate() {
        let (buy, sell) = match (slice.buy_ref, slice.sell_ref) {
            (LegRefV1::Order(buy), LegRefV1::Order(sell)) => (buy, sell),
            _ => return Err(ModelError::VirtualLegNotHosted),
        };
        let consideration = slice
            .quantity
            .checked_mul(candidate.prices[usize::from(slice.outcome)])
            .ok_or(ModelError::InvalidConsideration)?;
        model.settle_relation_receipt(&RelationReceipt {
            identity: clearing.identity,
            target: SettlementTarget::Slice(index as u16),
            buy_order_index: buy,
            buy_order_id: bindings[usize::from(buy)].order.canonical_order_id,
            sell_order_index: sell,
            sell_order_id: bindings[usize::from(sell)].order.canonical_order_id,
            outcome: slice.outcome,
            quantity: slice.quantity,
            consideration,
        })?;
    }
    model.observe(Observation::accepted(0, 20, 20))?;
    model.observe(Observation::accepted(1, 30, 30))?;
    model.observe(Observation::accepted(2, 70, 70))?;
    model.seal_observations()?;
    model.merge(1, 1)?;
    model.resolve_from_summary()?;
    model.redeem_internal(0, 1, 3)?;
    model.redeem_external(0, 1, 1)?;
    model.return_liveness(7)?;
    model.check_conservation()?;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn receipt(
        domain: BatchDomain,
        candidate: Candidate,
        buy_order_id: u64,
        sell_order_id: u64,
        outcome: u8,
        quantity: Amount,
        consideration: Amount,
    ) -> SettlementReceipt {
        SettlementReceipt::new(
            CandidateIdentity { domain, candidate },
            0,
            buy_order_id,
            1,
            sell_order_id,
            outcome,
            quantity,
            consideration,
        )
    }

    #[test]
    fn golden_trace_is_stable() {
        let model = golden_scenario().unwrap();
        let twap = model.twap().unwrap();
        assert_eq!(twap.numerator_low(), 7_200);
        assert_eq!(twap.numerator_high(), 7_200);
        assert_eq!(twap.denominator(), 180);
        let actual = format!("{}\n", model.trace.join("\n"));
        let expected = include_str!("../golden/basic.trace");
        assert_eq!(actual, expected);
    }

    #[test]
    fn partial_candidate_is_rejected_and_valid_candidate_conserves() {
        let mut model = VerticalModel::create_market(3, 0).unwrap();
        let orders = [
            Order {
                canonical_order_id: 1,
                side: Side::Buy,
                limit_tick: 1,
                quantity: 9,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            Order {
                canonical_order_id: 2,
                side: Side::Sell,
                limit_tick: 1,
                quantity: 4,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
        ];
        let candidate = model.clear_batch(&orders, 1).unwrap();
        assert_eq!(candidate.matched, 4);
        let mut tampered = candidate;
        tampered.fills[0] += 1;
        let mut ticks = [0; MAX_GRID_TICKS];
        ticks[0] = 10;
        ticks[1] = 20;
        ticks[2] = 30;
        let grid = PriceGrid::new(ticks, 3).unwrap();
        let policy = FrozenPolicy::new(
            grid,
            TieRule::MaxVolumeMinImbalanceHighestTick,
            DustPolicy::AssignCanonical,
            7,
        )
        .unwrap();
        let mut fixed = [Order {
            canonical_order_id: 0,
            side: Side::Buy,
            limit_tick: 0,
            quantity: 0,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
        }; MAX_ORDERS];
        fixed[0] = orders[0];
        fixed[1] = orders[1];
        let book = FixedBook::new(policy, fixed, 2).unwrap();
        assert_eq!(
            book.verify(&tampered),
            Err(clutch_batch::Error::ConservationFailure)
        );
        model.check_conservation().unwrap();
    }

    #[test]
    fn crash_retry_and_reorder_are_refused() {
        let mut model = VerticalModel::create_market(4, 0).unwrap();
        model.observe(Observation::accepted(0, 10, 10)).unwrap();
        assert_eq!(
            model.observe(Observation::accepted(0, 10, 10)),
            Err(ModelError::InvalidObservationOrder)
        );
        assert_eq!(
            model.observe(Observation::accepted(2, 70, 70)),
            Err(ModelError::InvalidObservationOrder)
        );
        let grid = Grid::new(7, 1, 60).unwrap();
        let left = Summary::singleton(grid, Observation::accepted(1, 20, 20)).unwrap();
        let right = Summary::singleton(grid, Observation::accepted(2, 70, 70)).unwrap();
        assert_eq!(right.combine(left), Err(SummaryError::NonAdjacent));
        assert_eq!(
            left.combine(right).unwrap(),
            left.append(Observation::accepted(2, 70, 70)).unwrap()
        );

        let mut settlement = VerticalModel::create_market(1, 0).unwrap();
        settlement.split(1, 2).unwrap();
        let orders = [
            Order {
                canonical_order_id: 1,
                side: Side::Buy,
                limit_tick: 1,
                quantity: 2,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            Order {
                canonical_order_id: 2,
                side: Side::Sell,
                limit_tick: 1,
                quantity: 1,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
        ];
        let bindings = [
            BoundOrder {
                order: orders[0],
                owner: 0,
                outcome: 0,
                expiry_epoch: u64::MAX,
            },
            BoundOrder {
                order: orders[1],
                owner: 1,
                outcome: 0,
                expiry_epoch: u64::MAX,
            },
        ];
        let candidate = settlement
            .clear_batch_with_bindings(VerticalModel::DEFAULT_BATCH_DOMAIN, &bindings, 1)
            .unwrap();
        assert_eq!(
            settlement.settle_batch_fill(&candidate, 1, 1, 0, 0, 1),
            Err(ModelError::MissingConsideration)
        );
        let no_cash = receipt(
            VerticalModel::DEFAULT_BATCH_DOMAIN,
            candidate,
            1,
            2,
            0,
            1,
            20,
        );
        assert_eq!(
            settlement.settle_batch_fill_with_consideration(&no_cash),
            Err(ModelError::InsufficientCash)
        );
        settlement.fund_cash(0, 20).unwrap();
        let first_receipt = receipt(
            VerticalModel::DEFAULT_BATCH_DOMAIN,
            candidate,
            1,
            2,
            0,
            1,
            20,
        );
        let mut corrupted = settlement.clone();
        corrupted.cash_funded = 21;
        let corrupted_before = corrupted.clone();
        assert_eq!(
            corrupted.settle_batch_fill_with_consideration(&first_receipt),
            Err(ModelError::InsufficientCash)
        );
        assert_eq!(corrupted, corrupted_before);
        settlement
            .settle_batch_fill_with_consideration(&first_receipt)
            .unwrap();
        let cash_before_reverse = settlement.cash;
        let claims_before_reverse = settlement.positions;
        let reversed_pair = SettlementReceipt::new(
            CandidateIdentity {
                domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
                candidate,
            },
            1,
            2,
            0,
            1,
            0,
            1,
            20,
        );
        assert_eq!(
            settlement.settle_batch_fill_with_consideration(&reversed_pair),
            Err(ModelError::InvalidFill)
        );
        assert_eq!(settlement.cash, cash_before_reverse);
        assert_eq!(settlement.positions, claims_before_reverse);
        assert_eq!(
            settlement.settle_batch_fill_with_consideration(&first_receipt),
            Err(ModelError::PairAlreadySettled)
        );
        let wrong_price = receipt(
            VerticalModel::DEFAULT_BATCH_DOMAIN,
            candidate,
            1,
            2,
            0,
            1,
            19,
        );
        assert_eq!(
            settlement.settle_batch_fill_with_consideration(&wrong_price),
            Err(ModelError::InvalidConsideration)
        );

        let wrong_outcome = SettlementReceipt::new(
            CandidateIdentity {
                domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
                candidate,
            },
            0,
            1,
            1,
            2,
            1,
            1,
            20,
        );
        assert_eq!(
            settlement.settle_batch_fill_with_consideration(&wrong_outcome),
            Err(ModelError::InvalidFill)
        );

        let mut cumulative = VerticalModel::create_market(5, 0).unwrap();
        cumulative.split(1, 8).unwrap();
        let orders = [
            Order {
                canonical_order_id: 1,
                side: Side::Buy,
                limit_tick: 1,
                quantity: 5,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            Order {
                canonical_order_id: 2,
                side: Side::Sell,
                limit_tick: 1,
                quantity: 5,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
        ];
        let bindings = [
            BoundOrder {
                order: orders[0],
                owner: 0,
                outcome: 0,
                expiry_epoch: u64::MAX,
            },
            BoundOrder {
                order: orders[1],
                owner: 1,
                outcome: 0,
                expiry_epoch: u64::MAX,
            },
        ];
        let candidate = cumulative
            .clear_batch_with_bindings(VerticalModel::DEFAULT_BATCH_DOMAIN, &bindings, 1)
            .unwrap();
        assert_eq!(candidate.fills[0], 5);
        cumulative.fund_cash(0, 100).unwrap();
        let receipt_3 = SettlementReceipt::new(
            CandidateIdentity {
                domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
                candidate,
            },
            0,
            1,
            1,
            2,
            0,
            3,
            60,
        );
        cumulative
            .settle_batch_fill_with_consideration(&receipt_3)
            .unwrap();
        let receipt_2 = SettlementReceipt::new(
            CandidateIdentity {
                domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
                candidate,
            },
            0,
            1,
            1,
            2,
            0,
            2,
            40,
        );
        assert_eq!(
            cumulative.settle_batch_fill_with_consideration(&receipt_2),
            Err(ModelError::PairAlreadySettled)
        );
        cumulative.check_conservation().unwrap();

        let domain_a = BatchDomain::new(1, 7, 11, 13, 17);
        let domain_b = BatchDomain::new(1, 8, 11, 13, 17);
        let domain_c = BatchDomain::new(1, 7, 12, 13, 17);
        assert_ne!(
            CandidateIdentity {
                domain: domain_a,
                candidate,
            },
            CandidateIdentity {
                domain: domain_b,
                candidate,
            }
        );
        assert_ne!(
            CandidateIdentity {
                domain: domain_a,
                candidate,
            },
            CandidateIdentity {
                domain: domain_c,
                candidate,
            }
        );
        let mut cross_domain = VerticalModel::create_market(4, 0).unwrap();
        cross_domain.split(1, 2).unwrap();
        let candidate_a = cross_domain
            .clear_batch_with_bindings(domain_a, &bindings, 1)
            .unwrap();
        cross_domain.fund_cash(0, 20).unwrap();
        let wrong_book = receipt(domain_b, candidate_a, 1, 2, 0, 1, 20);
        assert_eq!(
            cross_domain.settle_batch_fill_with_consideration(&wrong_book),
            Err(ModelError::InvalidFill)
        );
        let wrong_epoch = receipt(domain_c, candidate_a, 1, 2, 0, 1, 20);
        assert_eq!(
            cross_domain.settle_batch_fill_with_consideration(&wrong_epoch),
            Err(ModelError::InvalidFill)
        );
        let wrong_order = receipt(domain_a, candidate_a, 999, 2, 0, 1, 20);
        assert_eq!(
            cross_domain.settle_batch_fill_with_consideration(&wrong_order),
            Err(ModelError::InvalidFill)
        );
    }

    #[test]
    fn refusal_does_not_resolve_and_unsupported_features_refuse() {
        let mut gapped = VerticalModel::create_market(2, 0).unwrap();
        gapped.observe(Observation::accepted(0, 10, 10)).unwrap();
        gapped.observe(Observation::missing(1)).unwrap();
        gapped.observe(Observation::accepted(2, 70, 70)).unwrap();
        gapped.seal_observations().unwrap();
        assert_eq!(
            gapped.resolve_from_summary().unwrap(),
            ResolveDecision::Refused(Refusal::GappedCoverage)
        );
        assert_eq!(gapped.phase(), Phase::Active);
        assert_eq!(
            gapped.summary.threshold_crossings(50),
            Err(StatisticError::UnsupportedPredicate)
        );

        let mut ambiguous = VerticalModel::create_market(0, 0).unwrap();
        ambiguous.observe(Observation::accepted(0, 10, 10)).unwrap();
        ambiguous.observe(Observation::accepted(1, 40, 60)).unwrap();
        ambiguous.observe(Observation::accepted(2, 40, 60)).unwrap();
        ambiguous.seal_observations().unwrap();
        assert_eq!(
            ambiguous.resolve_from_summary().unwrap(),
            ResolveDecision::Refused(Refusal::AmbiguousTerminalInterval)
        );
        assert_eq!(ambiguous.phase(), Phase::Active);
    }

    #[test]
    fn maturity_and_seal_gate_resolution_and_freeze_observations() {
        let mut model = VerticalModel::create_market(0, 0).unwrap();
        model.observe(Observation::accepted(0, 10, 10)).unwrap();
        assert_eq!(
            model.resolve_from_summary().unwrap(),
            ResolveDecision::Refused(Refusal::NotMature)
        );
        assert_eq!(model.seal_observations(), Err(ModelError::NotMature));
        model.observe(Observation::accepted(1, 20, 20)).unwrap();
        model.observe(Observation::accepted(2, 30, 30)).unwrap();
        assert_eq!(
            model.resolve_from_summary().unwrap(),
            ResolveDecision::Refused(Refusal::NotSealed)
        );
        model.seal_observations().unwrap();
        assert_eq!(
            model.resolve_from_summary().unwrap(),
            ResolveDecision::Resolved(0)
        );
        assert!(model.is_sealed());
        assert_eq!(
            model.observe(Observation::accepted(3, 30, 30)),
            Err(ModelError::ObservationAfterSeal)
        );
        assert_eq!(model.seal_observations(), Err(ModelError::AlreadySealed));

        let mut overlong = VerticalModel::create_market(0, 0).unwrap();
        overlong.observe(Observation::accepted(0, 10, 10)).unwrap();
        overlong.observe(Observation::accepted(1, 20, 20)).unwrap();
        overlong.observe(Observation::accepted(2, 30, 30)).unwrap();
        assert_eq!(
            overlong.observe(Observation::accepted(3, 30, 30)),
            Err(ModelError::MaturityExceeded)
        );
    }

    #[test]
    fn fee_liveness_and_principal_boundaries_remain_disjoint() {
        let mut model = VerticalModel::create_market(10, 5_000).unwrap();
        model.split(0, 5).unwrap();
        assert_eq!(model.accounting.principal, 5);
        let orders = [
            Order {
                canonical_order_id: 1,
                side: Side::Buy,
                limit_tick: 0,
                quantity: 2,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            Order {
                canonical_order_id: 2,
                side: Side::Sell,
                limit_tick: 0,
                quantity: 2,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
        ];
        model.clear_batch(&orders, 3).unwrap();
        assert_eq!(model.accounting.principal, model.market.collateral);
        assert_eq!(model.accounting.fee_revenue, 1);
        assert_eq!(model.accounting.liveness_paid, 3);
        assert_eq!(model.accounting.liveness_reserved, 7);
        assert_eq!(model.accounting.liveness_returned, 0);
        assert!(model.accounting.pay_liveness(8).is_err());
    }

    #[test]
    fn accounting_refusal_preserves_full_state() {
        let mut accounting = Accounting::new(1);
        accounting.liveness_paid = 1;
        let before = accounting;
        assert_eq!(
            accounting.pay_liveness(1),
            Err(AccountingError::LivenessConservation)
        );
        assert_eq!(accounting, before);

        let mut returned = Accounting::new(1);
        returned.liveness_returned = 1;
        let before_return = returned;
        assert_eq!(
            returned.return_liveness(1),
            Err(AccountingError::LivenessConservation)
        );
        assert_eq!(returned, before_return);
    }

    // ---------------------------------------------------------------------
    // Coupled-relation path.
    //
    // Every §6 counterexample name of `docs/implementation/ADVERSARIAL_REVIEW_V0.md`
    // that this component owns appears below verbatim.
    // ---------------------------------------------------------------------

    fn bind(
        id: u64,
        owner: usize,
        outcome: u8,
        side: Side,
        quantity: Amount,
        limit_tick: u8,
    ) -> BoundOrder {
        BoundOrder {
            order: Order {
                canonical_order_id: id,
                side,
                limit_tick,
                quantity,
                minimum_fill: 1,
                partial_policy: PartialPolicy::Allow,
            },
            owner,
            outcome,
            expiry_epoch: u64::MAX,
        }
    }

    /// One buy and one sell of `quantity`, both bound to outcome 1 at the top
    /// tick, owned by the two distinct model owners.
    fn one_pair_book(quantity: Amount) -> [BoundOrder; 2] {
        [
            bind(1, 0, 1, Side::Buy, quantity, 2),
            bind(2, 1, 1, Side::Sell, quantity, 2),
        ]
    }

    /// Three unit buys against three unit sells: a book whose frozen
    /// decomposition has more than one slice, so settlement order is a real
    /// degree of freedom.
    fn three_pair_book() -> [BoundOrder; 6] {
        [
            bind(1, 0, 1, Side::Buy, 1, 2),
            bind(2, 1, 1, Side::Sell, 1, 2),
            bind(3, 0, 1, Side::Buy, 1, 2),
            bind(4, 1, 1, Side::Sell, 1, 2),
            bind(5, 0, 1, Side::Buy, 1, 2),
            bind(6, 1, 1, Side::Sell, 1, 2),
        ]
    }

    fn coupled_domain(residual_settlement: ResidualSettlementV1) -> RelationDomainV1 {
        proposed_relation_domain(
            VerticalModel::DEFAULT_BATCH_DOMAIN,
            residual_settlement,
            TransferPhaseV1::ActiveOnly,
        )
    }

    /// Create a market, fund the seller's claims and the buyer's cash, and
    /// clear `bindings` through the coupled relation.
    fn cleared(
        residual_settlement: ResidualSettlementV1,
        bindings: &[BoundOrder],
        seller_sets: Amount,
        buyer_cash: Amount,
    ) -> (VerticalModel, RelationClearing) {
        let mut model = VerticalModel::create_market(10, 5_000).unwrap();
        model.split(1, seller_sets).unwrap();
        let clearing = model
            .clear_relation_v1(
                coupled_domain(residual_settlement),
                bindings,
                PROPOSED_SEARCH_BOUNDS,
                1,
            )
            .unwrap();
        model.fund_cash(0, buyer_cash).unwrap();
        (model, clearing)
    }

    fn slice_pair(slice: PairingSliceV1) -> (u8, u8) {
        match (slice.buy_ref, slice.sell_ref) {
            (LegRefV1::Order(buy), LegRefV1::Order(sell)) => (buy, sell),
            _ => panic!("this model refuses to freeze a virtual slice"),
        }
    }

    fn receipt_for(
        model: &VerticalModel,
        clearing: &RelationClearing,
        bindings: &[BoundOrder],
        slice_index: u16,
        quantity: Amount,
        target: SettlementTarget,
    ) -> RelationReceipt {
        let slices = model.relation_slices(&clearing.identity).unwrap();
        let candidate = model.relation_candidate(&clearing.identity).unwrap();
        let slice = slices[usize::from(slice_index)];
        let (buy, sell) = slice_pair(slice);
        RelationReceipt {
            identity: clearing.identity,
            target,
            buy_order_index: buy,
            buy_order_id: bindings[usize::from(buy)].order.canonical_order_id,
            sell_order_index: sell,
            sell_order_id: bindings[usize::from(sell)].order.canonical_order_id,
            outcome: slice.outcome,
            quantity,
            consideration: quantity * candidate.prices[usize::from(slice.outcome)],
        }
    }

    /// The target kind the frozen variant admits.
    fn target_for(residual_settlement: ResidualSettlementV1, slice_index: u16) -> SettlementTarget {
        match residual_settlement {
            ResidualSettlementV1::CumulativePairCanonical => SettlementTarget::Pair,
            _ => SettlementTarget::Slice(slice_index),
        }
    }

    fn settle_slice(
        model: &mut VerticalModel,
        clearing: &RelationClearing,
        bindings: &[BoundOrder],
        residual_settlement: ResidualSettlementV1,
        slice_index: u16,
    ) -> Result<(), ModelError> {
        let quantity =
            model.relation_slices(&clearing.identity).unwrap()[usize::from(slice_index)].quantity;
        let receipt = receipt_for(
            model,
            clearing,
            bindings,
            slice_index,
            quantity,
            target_for(residual_settlement, slice_index),
        );
        model.settle_relation_receipt(&receipt)
    }

    /// The projection the permutation test compares: everything a settlement
    /// touches except the trace, whose order is the thing being permuted.
    #[allow(clippy::type_complexity)]
    fn settled_state(
        model: &VerticalModel,
        clearing: &RelationClearing,
    ) -> (
        MarketState,
        [Position; OWNERS],
        [Amount; OWNERS],
        Amount,
        Amount,
        Accounting,
        RelationLedgerView,
    ) {
        (
            model.market,
            model.positions,
            model.cash,
            model.protocol_cash,
            model.cash_funded,
            model.accounting,
            model.relation_ledger(&clearing.identity).unwrap(),
        )
    }

    #[test]
    fn coupled_golden_trace_is_stable() {
        let model = coupled_golden_scenario(ResidualSettlementV1::UniqueSliceReceipts).unwrap();
        let twap = model.twap().unwrap();
        assert_eq!(twap.numerator_low(), 7_200);
        assert_eq!(twap.numerator_high(), 7_200);
        assert_eq!(twap.denominator(), 180);
        let actual = format!("{}\n", model.trace.join("\n"));
        let expected = include_str!("../golden/coupled.trace");
        assert_eq!(actual, expected);
        // The scalar golden trace is a permanent regression and is never
        // rewritten by anything on the coupled path.
        let scalar = golden_scenario().unwrap();
        assert_eq!(
            format!("{}\n", scalar.trace.join("\n")),
            include_str!("../golden/basic.trace")
        );
    }

    #[test]
    fn settlement_consumes_paired_buy_and_sell_fill_exactly_once() {
        for residual_settlement in [
            ResidualSettlementV1::FullPairOnly,
            ResidualSettlementV1::CumulativePairCanonical,
            ResidualSettlementV1::UniqueSliceReceipts,
        ] {
            let bindings = one_pair_book(3);
            let (mut model, clearing) = cleared(residual_settlement, &bindings, 3, 90);
            assert_eq!(clearing.accepted_volume, 3);
            assert_eq!(clearing.slice_count, 1);
            settle_slice(&mut model, &clearing, &bindings, residual_settlement, 0).unwrap();

            let ledger = model.relation_ledger(&clearing.identity).unwrap();
            let candidate = model.relation_candidate(&clearing.identity).unwrap();
            assert_eq!(ledger.settled_volume, ledger.accepted_volume);
            assert_eq!(ledger.settled_by_order[0], candidate.fills[0]);
            assert_eq!(ledger.settled_by_order[1], candidate.fills[1]);
            assert_eq!(ledger.settled_by_slice[0], 3);
            assert_eq!(model.positions[0].internal[1], 3);
            assert_eq!(model.positions[1].internal[1], 0);
            assert_eq!(model.cash, [0, 90]);

            // A second consumption of the same paired fill is refused under
            // every variant; only the diagnostic differs.
            let before = model.clone();
            let expected = match residual_settlement {
                ResidualSettlementV1::FullPairOnly => ModelError::PairAlreadySettled,
                ResidualSettlementV1::CumulativePairCanonical => ModelError::ExceedsPairRemaining,
                _ => ModelError::SliceExceeded,
            };
            assert_eq!(
                settle_slice(&mut model, &clearing, &bindings, residual_settlement, 0),
                Err(expected)
            );
            assert_eq!(model, before);
            model.check_conservation().unwrap();
        }
    }

    #[test]
    fn settlement_partial_pair_behavior_matches_frozen_terminal_policy() {
        // 1a: full-pair-only.  A receipt below the slice quantity is refused
        // outright; the whole slice settles exactly once.
        let bindings = one_pair_book(3);
        let (mut model, clearing) = cleared(ResidualSettlementV1::FullPairOnly, &bindings, 3, 90);
        let partial = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            2,
            SettlementTarget::Slice(0),
        );
        let before = model.clone();
        assert_eq!(
            model.settle_relation_receipt(&partial),
            Err(ModelError::PartialPairRefused)
        );
        assert_eq!(model, before);
        // 1a admits only slice targets.
        let mispointed = RelationReceipt {
            target: SettlementTarget::Pair,
            ..receipt_for(
                &model,
                &clearing,
                &bindings,
                0,
                3,
                SettlementTarget::Slice(0),
            )
        };
        assert_eq!(
            model.settle_relation_receipt(&mispointed),
            Err(ModelError::SettlementTargetNotAdmitted)
        );
        assert_eq!(model, before);
        settle_slice(
            &mut model,
            &clearing,
            &bindings,
            ResidualSettlementV1::FullPairOnly,
            0,
        )
        .unwrap();
        assert_eq!(
            model
                .relation_ledger(&clearing.identity)
                .unwrap()
                .settled_volume,
            3
        );

        // 1b: cumulative per-pair remaining, addressed by the executable pair.
        let (mut model, clearing) = cleared(
            ResidualSettlementV1::CumulativePairCanonical,
            &bindings,
            3,
            90,
        );
        let first = receipt_for(&model, &clearing, &bindings, 0, 2, SettlementTarget::Pair);
        model.settle_relation_receipt(&first).unwrap();
        let second = receipt_for(&model, &clearing, &bindings, 0, 1, SettlementTarget::Pair);
        model.settle_relation_receipt(&second).unwrap();
        let sixth = receipt_for(&model, &clearing, &bindings, 0, 1, SettlementTarget::Pair);
        let before = model.clone();
        assert_eq!(
            model.settle_relation_receipt(&sixth),
            Err(ModelError::ExceedsPairRemaining)
        );
        assert_eq!(model, before);
        // 1b admits only pair targets: the slice index is not the ledger key.
        let (mut fresh, fresh_clearing) = cleared(
            ResidualSettlementV1::CumulativePairCanonical,
            &bindings,
            3,
            90,
        );
        let by_slice = receipt_for(
            &fresh,
            &fresh_clearing,
            &bindings,
            0,
            3,
            SettlementTarget::Slice(0),
        );
        assert_eq!(
            fresh.settle_relation_receipt(&by_slice),
            Err(ModelError::SettlementTargetNotAdmitted)
        );

        // 1c: unique match-slice receipts, addressed by the frozen slice id.
        let (mut model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        let first = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            2,
            SettlementTarget::Slice(0),
        );
        model.settle_relation_receipt(&first).unwrap();
        let second = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            1,
            SettlementTarget::Slice(0),
        );
        model.settle_relation_receipt(&second).unwrap();
        let third = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            1,
            SettlementTarget::Slice(0),
        );
        let before = model.clone();
        assert_eq!(
            model.settle_relation_receipt(&third),
            Err(ModelError::SliceExceeded)
        );
        assert_eq!(model, before);
        // Naming a slice the frozen decomposition does not have is refused;
        // freezing the decomposition is the point.
        let unknown = RelationReceipt {
            target: SettlementTarget::Slice(7),
            ..first
        };
        assert_eq!(
            model.settle_relation_receipt(&unknown),
            Err(ModelError::UnknownSlice)
        );

        // 1b-free is recorded by the relation and refused here, before any fee
        // or liveness charge, because its documented strand hazard has no
        // terminal sweep authority in this model.
        let mut free = VerticalModel::create_market(10, 5_000).unwrap();
        free.split(1, 3).unwrap();
        let before = free.clone();
        assert_eq!(
            free.clear_relation_v1(
                coupled_domain(ResidualSettlementV1::CumulativePairFree),
                &bindings,
                PROPOSED_SEARCH_BOUNDS,
                1,
            )
            .err(),
            Some(ModelError::ResidualVariantUnimplemented)
        );
        assert_eq!(free, before);
    }

    #[test]
    fn settlement_cumulative_consumption_cannot_exceed_receipt() {
        // The review's `3 + 2 + 1` against a verified fill of 5.
        let bindings = one_pair_book(5);
        for (residual_settlement, expected) in [
            (
                ResidualSettlementV1::UniqueSliceReceipts,
                ModelError::SliceExceeded,
            ),
            (
                ResidualSettlementV1::CumulativePairCanonical,
                ModelError::ExceedsPairRemaining,
            ),
        ] {
            let (mut model, clearing) = cleared(residual_settlement, &bindings, 5, 150);
            let candidate = model.relation_candidate(&clearing.identity).unwrap();
            assert_eq!(candidate.fills[0], 5);
            assert_eq!(candidate.fills[1], 5);
            for quantity in [3, 2] {
                let receipt = receipt_for(
                    &model,
                    &clearing,
                    &bindings,
                    0,
                    quantity,
                    target_for(residual_settlement, 0),
                );
                model.settle_relation_receipt(&receipt).unwrap();
            }
            let sixth = receipt_for(
                &model,
                &clearing,
                &bindings,
                0,
                1,
                target_for(residual_settlement, 0),
            );
            let before = model.clone();
            assert_eq!(model.settle_relation_receipt(&sixth), Err(expected));
            assert_eq!(model, before);
            let ledger = model.relation_ledger(&clearing.identity).unwrap();
            assert_eq!(ledger.settled_volume, 5);
            assert_eq!(ledger.settled_by_order[0], 5);
            assert_eq!(ledger.settled_by_order[1], 5);
            model.check_conservation().unwrap();
        }
    }

    #[test]
    fn settlement_rejects_wrong_book_epoch_candidate_owner_side_asset_pair_and_generation() {
        let bindings = three_pair_book();
        let (mut model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        let good = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            1,
            SettlementTarget::Slice(0),
        );
        let before = model.clone();

        // Wrong book, epoch, policy generation, and canonical order-set: the
        // model key is the `BatchDomain` tuple and none of them match.
        for domain in [
            BatchDomain::new(1, 9, 1, 1, 1),
            BatchDomain::new(1, 1, 9, 1, 1),
            BatchDomain::new(1, 1, 1, 9, 1),
            BatchDomain::new(1, 1, 1, 1, 9),
            BatchDomain::new(9, 1, 1, 1, 1),
        ] {
            let wrong = RelationReceipt {
                identity: RelationCandidateIdentity {
                    domain,
                    ..clearing.identity
                },
                ..good
            };
            assert_eq!(
                model.settle_relation_receipt(&wrong),
                Err(ModelError::MissingRelationLedger)
            );
        }
        // Wrong candidate: the digest folds the whole frozen domain, the fill
        // vector, and the frozen decomposition.
        let wrong_candidate = RelationReceipt {
            identity: RelationCandidateIdentity {
                candidate_digest: clearing.identity.candidate_digest ^ 1,
                ..clearing.identity
            },
            ..good
        };
        assert_eq!(
            model.settle_relation_receipt(&wrong_candidate),
            Err(ModelError::MissingRelationLedger)
        );

        // Wrong side: the frozen slice pins which leg is the buy.
        let reversed = RelationReceipt {
            buy_order_index: good.sell_order_index,
            buy_order_id: good.sell_order_id,
            sell_order_index: good.buy_order_index,
            sell_order_id: good.buy_order_id,
            ..good
        };
        assert_eq!(
            model.settle_relation_receipt(&reversed),
            Err(ModelError::InvalidFill)
        );
        // Wrong asset: outcome 0 is not the slice's outcome.
        let wrong_asset = RelationReceipt { outcome: 0, ..good };
        assert_eq!(
            model.settle_relation_receipt(&wrong_asset),
            Err(ModelError::InvalidFill)
        );
        // Wrong order identity at a right index.
        let wrong_order = RelationReceipt {
            buy_order_id: 999,
            ..good
        };
        assert_eq!(
            model.settle_relation_receipt(&wrong_order),
            Err(ModelError::InvalidFill)
        );
        // Wrong consideration, to the atom.
        let wrong_price = RelationReceipt {
            consideration: good.consideration - 1,
            ..good
        };
        assert_eq!(
            model.settle_relation_receipt(&wrong_price),
            Err(ModelError::InvalidConsideration)
        );
        // Wrong pair: a pairing of two real legs that the relation never froze.
        let slices = model.relation_slices(&clearing.identity).unwrap();
        let frozen: Vec<(u8, u8, u8)> = slices
            .iter()
            .map(|slice| {
                let (buy, sell) = slice_pair(*slice);
                (buy, sell, slice.outcome)
            })
            .collect();
        let unfrozen = [0u8, 2, 4]
            .into_iter()
            .flat_map(|buy| [1u8, 3, 5].into_iter().map(move |sell| (buy, sell, 1u8)))
            .find(|pair| !frozen.contains(pair))
            .expect("a nine-pair universe cannot be exhausted by three slices");
        let (mut pair_model, pair_clearing) = cleared(
            ResidualSettlementV1::CumulativePairCanonical,
            &bindings,
            3,
            90,
        );
        let unknown_pair = RelationReceipt {
            identity: pair_clearing.identity,
            target: SettlementTarget::Pair,
            buy_order_index: unfrozen.0,
            buy_order_id: bindings[usize::from(unfrozen.0)].order.canonical_order_id,
            sell_order_index: unfrozen.1,
            sell_order_id: bindings[usize::from(unfrozen.1)].order.canonical_order_id,
            outcome: unfrozen.2,
            quantity: 1,
            consideration: 30,
        };
        assert_eq!(
            pair_model.settle_relation_receipt(&unknown_pair),
            Err(ModelError::UnknownPair)
        );

        // Wrong owner: a book in which one owner stands on both sides of one
        // outcome never clears at all under the frozen `RefuseOverlap`
        // variant, so no receipt with equal parties can exist to refuse.
        let self_crossing = [
            bind(1, 0, 1, Side::Buy, 3, 2),
            bind(2, 0, 1, Side::Sell, 3, 2),
        ];
        let mut wash = VerticalModel::create_market(10, 5_000).unwrap();
        wash.split(0, 3).unwrap();
        let wash_before = wash.clone();
        assert_eq!(
            wash.clear_relation_v1(
                coupled_domain(ResidualSettlementV1::UniqueSliceReceipts),
                &self_crossing,
                PROPOSED_SEARCH_BOUNDS,
                1,
            )
            .err(),
            Some(ModelError::Relation(ErrorV1::SelfCrossRefused))
        );
        assert_eq!(wash, wash_before);

        // Not one of the refusals above moved a claim, an atom of cash, a
        // ledger entry, or a trace line.
        assert_eq!(model, before);
        model.check_conservation().unwrap();
    }

    #[test]
    fn settlement_retry_and_all_permutations_are_idempotent() {
        let bindings = three_pair_book();
        const PERMUTATIONS: [[u16; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for residual_settlement in [
            ResidualSettlementV1::FullPairOnly,
            ResidualSettlementV1::CumulativePairCanonical,
            ResidualSettlementV1::UniqueSliceReceipts,
        ] {
            let mut reference: Option<_> = None;
            let mut reference_trace: Option<Vec<String>> = None;
            for order in PERMUTATIONS {
                let (mut model, clearing) = cleared(residual_settlement, &bindings, 3, 90);
                assert_eq!(clearing.slice_count, 3);
                for slice_index in order {
                    settle_slice(
                        &mut model,
                        &clearing,
                        &bindings,
                        residual_settlement,
                        slice_index,
                    )
                    .unwrap();
                }
                let state = settled_state(&model, &clearing);
                match &reference {
                    None => reference = Some(state),
                    Some(expected) => assert_eq!(&state, expected),
                }
                let mut sorted = model.trace.clone();
                sorted.sort();
                match &reference_trace {
                    None => reference_trace = Some(sorted),
                    Some(expected) => assert_eq!(&sorted, expected),
                }
                model.check_conservation().unwrap();

                // Retry: replaying the whole clearing is idempotent and charges
                // no second fee or liveness, and replaying a settled receipt is
                // refused with the whole prestate preserved.
                let before = model.clone();
                let replay = model
                    .clear_relation_v1(
                        coupled_domain(residual_settlement),
                        &bindings,
                        PROPOSED_SEARCH_BOUNDS,
                        1,
                    )
                    .unwrap();
                assert!(replay.replayed);
                assert_eq!(replay.fee, 0);
                assert_eq!(replay.identity, clearing.identity);
                assert_eq!(model, before);
                assert!(settle_slice(
                    &mut model,
                    &clearing,
                    &bindings,
                    residual_settlement,
                    order[0]
                )
                .is_err());
                assert_eq!(model, before);
            }
        }
    }

    #[test]
    fn cleared_volume_always_settles_under_every_variant() {
        // The clear-time promise: the relation's accepted volume is exactly the
        // volume its frozen decomposition pairs, so a funded settlement always
        // completes it.  Fees and liveness are charged only against that
        // volume, which is what makes the scalar relation's charge-then-refuse
        // shape (§P1-B) inexpressible here.
        for residual_settlement in [
            ResidualSettlementV1::FullPairOnly,
            ResidualSettlementV1::CumulativePairCanonical,
            ResidualSettlementV1::UniqueSliceReceipts,
        ] {
            for bindings in [&one_pair_book(3)[..], &three_pair_book()[..]] {
                let (mut model, clearing) = cleared(residual_settlement, bindings, 3, 90);
                let slices = model.relation_slices(&clearing.identity).unwrap();
                let slice_total: Amount = slices.iter().map(|slice| slice.quantity).sum();
                assert_eq!(slice_total, clearing.accepted_volume);
                assert_eq!(
                    model.accounting.fee_revenue,
                    clearing.accepted_volume * 5_000 / FEE_BPS_DENOMINATOR
                );
                for slice_index in 0..clearing.slice_count {
                    settle_slice(
                        &mut model,
                        &clearing,
                        bindings,
                        residual_settlement,
                        slice_index,
                    )
                    .unwrap();
                }
                let ledger = model.relation_ledger(&clearing.identity).unwrap();
                let candidate = model.relation_candidate(&clearing.identity).unwrap();
                assert_eq!(ledger.settled_volume, clearing.accepted_volume);
                for index in 0..bindings.len() {
                    assert_eq!(ledger.settled_by_order[index], candidate.fills[index]);
                }
                for (index, slice) in slices.iter().enumerate() {
                    assert_eq!(ledger.settled_by_slice[index], slice.quantity);
                }
                assert_eq!(model.positions[0].internal[1], clearing.accepted_volume);
                assert_eq!(model.cash[1], clearing.accepted_volume * 30);
                model.check_conservation().unwrap();
            }
        }
    }

    #[test]
    fn relation_charges_only_pairable_volume_where_the_scalar_lab_charged_unpairable_volume() {
        // The executed §P1-B counterexample at the model boundary: a buy bound
        // to outcome 0 against a sell bound to outcome 1.
        let bindings = [
            bind(1, 0, 0, Side::Buy, 3, 2),
            bind(2, 1, 1, Side::Sell, 3, 2),
        ];

        // Scalar lab: outcome is erased, one unit of volume is matched, a fee
        // is charged on it, and every settlement of it is then refused.
        let mut scalar = VerticalModel::create_market(10, 5_000).unwrap();
        scalar.split(1, 3).unwrap();
        let candidate = scalar
            .clear_batch_with_bindings(VerticalModel::DEFAULT_BATCH_DOMAIN, &bindings, 1)
            .unwrap();
        assert_eq!(candidate.matched, 3);
        assert_eq!(scalar.accounting.fee_revenue, 1);
        scalar.fund_cash(0, 90).unwrap();
        let unsettleable = SettlementReceipt::new(
            CandidateIdentity {
                domain: VerticalModel::DEFAULT_BATCH_DOMAIN,
                candidate,
            },
            0,
            1,
            1,
            2,
            0,
            1,
            30,
        );
        assert_eq!(
            scalar.settle_batch_fill_with_consideration(&unsettleable),
            Err(ModelError::InvalidFill)
        );

        // Coupled relation: per-outcome conservation has no solution, so the
        // volume cannot be claimed at all and no fee is charged on it.
        let (model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        assert_eq!(clearing.accepted_volume, 0);
        assert_eq!(clearing.fee, 0);
        assert_eq!(clearing.slice_count, 0);
        assert_eq!(model.accounting.fee_revenue, 0);
        assert_eq!(
            model.relation_slices(&clearing.identity).unwrap(),
            Vec::new()
        );
        assert_eq!(model.positions[0].internal, [0; MAX_OUTCOMES]);
        model.check_conservation().unwrap();
    }

    #[test]
    fn coupled_settlement_moves_claims_through_the_kernel() {
        // The claim leg is the kernel's transition, not an inline field write:
        // an underfunded seller is refused by `transfer_internal` itself, and
        // the market's supply and collateral are untouched by a transfer.
        let bindings = one_pair_book(3);
        let (mut model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 1, 90);
        let supply_before = model.market.total_supply;
        let collateral_before = model.market.collateral;
        let before = model.clone();
        assert_eq!(
            settle_slice(
                &mut model,
                &clearing,
                &bindings,
                ResidualSettlementV1::UniqueSliceReceipts,
                0
            ),
            Err(ModelError::Kernel(
                clutch_kernel::Error::InsufficientBalance
            ))
        );
        assert_eq!(model, before);

        let (mut model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        let supply = model.market.total_supply;
        let collateral = model.market.collateral;
        settle_slice(
            &mut model,
            &clearing,
            &bindings,
            ResidualSettlementV1::UniqueSliceReceipts,
            0,
        )
        .unwrap();
        assert_eq!(model.market.total_supply, supply);
        assert_eq!(model.market.collateral, collateral);
        assert_ne!(supply_before, [0; MAX_OUTCOMES]);
        assert_eq!(collateral_before, 1);

        // The frozen relation policy is the single owner of the phase gate, and
        // the settlement call site names the kernel policy it maps to.
        assert_eq!(
            kernel_phase_policy(TransferPhaseV1::ActiveOnly),
            TransferPhasePolicy::ActiveOnly
        );
        assert_eq!(
            kernel_phase_policy(TransferPhaseV1::ActiveOrResolved),
            TransferPhasePolicy::ActiveOrResolved
        );
        // T-a refuses a claim leg that races resolution.  Nothing here selects
        // T-b for the model; the alternative is named, not taken, and the
        // strand it implies is exactly the ordering rule §14.2 leaves open.
        let (mut resolved, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        resolved.observe(Observation::accepted(0, 70, 70)).unwrap();
        resolved.observe(Observation::accepted(1, 70, 70)).unwrap();
        resolved.observe(Observation::accepted(2, 70, 70)).unwrap();
        resolved.seal_observations().unwrap();
        assert_eq!(
            resolved.resolve_from_summary().unwrap(),
            ResolveDecision::Resolved(1)
        );
        let before = resolved.clone();
        let late = receipt_for(
            &resolved,
            &clearing,
            &bindings,
            0,
            3,
            SettlementTarget::Slice(0),
        );
        assert_eq!(
            resolved.settle_relation_receipt(&late),
            Err(ModelError::Kernel(clutch_kernel::Error::AlreadyResolved))
        );
        assert_eq!(resolved, before);
    }

    #[test]
    fn redeem_complete_set_exits_a_balanced_holding() {
        let mut model = VerticalModel::create_market(0, 0).unwrap();
        model.split(0, 4).unwrap();
        assert_eq!(
            model.redeem_complete_set(0, 1),
            Err(ModelError::Kernel(clutch_kernel::Error::NotResolved))
        );
        model.observe(Observation::accepted(0, 70, 70)).unwrap();
        model.observe(Observation::accepted(1, 70, 70)).unwrap();
        model.observe(Observation::accepted(2, 70, 70)).unwrap();
        model.seal_observations().unwrap();
        assert_eq!(
            model.resolve_from_summary().unwrap(),
            ResolveDecision::Resolved(1)
        );
        assert_eq!(model.redeem_complete_set(0, 3), Ok(3));
        assert_eq!(model.positions[0].internal[0], 1);
        assert_eq!(model.positions[0].internal[1], 1);
        assert_eq!(model.market.collateral, 1);
        let before = model.clone();
        assert_eq!(
            model.redeem_complete_set(0, 2),
            Err(ModelError::Kernel(
                clutch_kernel::Error::InsufficientBalance
            ))
        );
        assert_eq!(model, before);
        model.check_conservation().unwrap();
    }

    #[test]
    fn vertical_every_error_preserves_exact_prestate() {
        // Clearing refusals: every one of them precedes the fee and liveness
        // charge, so the prestate includes an unmoved accounting bucket.
        let bindings = one_pair_book(3);
        let mut model = VerticalModel::create_market(10, 5_000).unwrap();
        model.split(1, 3).unwrap();
        let domain = coupled_domain(ResidualSettlementV1::UniqueSliceReceipts);
        let clearing_refusals: Vec<(
            RelationDomainV1,
            Vec<BoundOrder>,
            SearchBoundsV1,
            ModelError,
        )> = vec![
            (
                RelationDomainV1 {
                    outcome_count: 3,
                    ..domain
                },
                bindings.to_vec(),
                PROPOSED_SEARCH_BOUNDS,
                ModelError::RelationDomainMismatch,
            ),
            (
                RelationDomainV1 {
                    price_scale: 1_000,
                    ..domain
                },
                bindings.to_vec(),
                PROPOSED_SEARCH_BOUNDS,
                ModelError::RelationDomainMismatch,
            ),
            (
                RelationDomainV1 {
                    relation_version: 2,
                    ..domain
                },
                bindings.to_vec(),
                PROPOSED_SEARCH_BOUNDS,
                ModelError::RelationDomainMismatch,
            ),
            (
                domain,
                vec![bind(1, 9, 1, Side::Buy, 3, 2), bindings[1]],
                PROPOSED_SEARCH_BOUNDS,
                ModelError::InvalidOwner,
            ),
            (
                domain,
                vec![bind(1, 0, 1, Side::Buy, 3, 9), bindings[1]],
                PROPOSED_SEARCH_BOUNDS,
                ModelError::UnrepresentableBinding,
            ),
            (
                domain,
                vec![
                    bind(2, 0, 1, Side::Buy, 3, 2),
                    bind(1, 1, 1, Side::Sell, 3, 2),
                ],
                PROPOSED_SEARCH_BOUNDS,
                ModelError::Relation(ErrorV1::NonCanonicalOrderOrder),
            ),
            (
                domain,
                vec![
                    BoundOrder {
                        expiry_epoch: 0,
                        ..bindings[0]
                    },
                    bindings[1],
                ],
                PROPOSED_SEARCH_BOUNDS,
                ModelError::Relation(ErrorV1::ExpiredOrder),
            ),
            (
                coupled_domain(ResidualSettlementV1::CumulativePairFree),
                bindings.to_vec(),
                PROPOSED_SEARCH_BOUNDS,
                ModelError::ResidualVariantUnimplemented,
            ),
            (
                domain,
                bindings.to_vec(),
                SearchBoundsV1 {
                    price_step: 7,
                    ..PROPOSED_SEARCH_BOUNDS
                },
                ModelError::Relation(ErrorV1::SearchBudgetExceeded),
            ),
        ];
        for (domain, book, bounds, expected) in clearing_refusals {
            let before = model.clone();
            assert_eq!(
                model.clear_relation_v1(domain, &book, bounds, 1).err(),
                Some(expected)
            );
            assert_eq!(model, before);
        }

        // Settlement refusals on a cleared, funded batch.
        let (mut model, clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 60);
        let good = receipt_for(
            &model,
            &clearing,
            &bindings,
            0,
            3,
            SettlementTarget::Slice(0),
        );
        let settlement_refusals = [
            (
                RelationReceipt {
                    quantity: 0,
                    consideration: 0,
                    ..good
                },
                ModelError::InvalidFill,
            ),
            (
                RelationReceipt {
                    target: SettlementTarget::Slice(3),
                    ..good
                },
                ModelError::UnknownSlice,
            ),
            (
                RelationReceipt {
                    target: SettlementTarget::Pair,
                    ..good
                },
                ModelError::SettlementTargetNotAdmitted,
            ),
            (
                RelationReceipt {
                    quantity: 4,
                    consideration: 120,
                    ..good
                },
                ModelError::SliceExceeded,
            ),
            (
                RelationReceipt {
                    consideration: 91,
                    ..good
                },
                ModelError::InvalidConsideration,
            ),
            // Funded with 60 price units against a 90 price-unit slice.
            (good, ModelError::InsufficientCash),
        ];
        for (receipt, expected) in settlement_refusals {
            let before = model.clone();
            assert_eq!(model.settle_relation_receipt(&receipt), Err(expected));
            assert_eq!(model, before);
        }
        model.check_conservation().unwrap();

        // A refusal deep inside the transition — after the claim leg has
        // already moved on the staged clone — still preserves the prestate
        // exactly, including claims, cash, ledgers, accounting, and trace.
        let mut corrupted = model.clone();
        corrupted.cash_funded = 61;
        let corrupted_before = corrupted.clone();
        assert!(corrupted.settle_relation_receipt(&good).is_err());
        assert_eq!(corrupted, corrupted_before);
    }

    #[test]
    fn proposed_search_box_is_named_and_the_virtual_leg_guard_is_explicit() {
        // The bounded box is a PROPOSED fixture parameter, not a canonized
        // one, and `max_imbalance: 0` is what keeps every accepted candidate
        // inside the set this model can settle.  Widening it does not silently
        // change the cleared candidate on the golden book.
        assert_eq!(PROPOSED_SEARCH_BOUNDS.max_imbalance, 0);
        let bindings = golden_bindings();
        let (narrow, narrow_clearing) =
            cleared(ResidualSettlementV1::UniqueSliceReceipts, &bindings, 3, 90);
        let mut wide = VerticalModel::create_market(10, 5_000).unwrap();
        wide.split(1, 3).unwrap();
        let wide_clearing = wide
            .clear_relation_v1(
                coupled_domain(ResidualSettlementV1::UniqueSliceReceipts),
                &bindings,
                SearchBoundsV1 {
                    max_imbalance: 2,
                    max_visits: 512,
                    ..PROPOSED_SEARCH_BOUNDS
                },
                1,
            )
            .unwrap();
        assert_eq!(wide_clearing.identity, narrow_clearing.identity);
        assert_eq!(wide_clearing.accepted_volume, 3);
        let candidate = narrow
            .relation_candidate(&narrow_clearing.identity)
            .unwrap();
        assert_eq!(candidate.virtual_split, 0);
        assert_eq!(candidate.virtual_merge, 0);
        // Every frozen slice of this model names two real bound legs; a virtual
        // split or merge leg has no position to move through here and is
        // refused at clearing, never carried into a ledger.
        for slice in narrow.relation_slices(&narrow_clearing.identity).unwrap() {
            assert!(matches!(slice.buy_ref, LegRefV1::Order(_)));
            assert!(matches!(slice.sell_ref, LegRefV1::Order(_)));
        }
    }
}
