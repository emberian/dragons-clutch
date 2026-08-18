//! Deterministic, offline composition of the three landed Clutch semantic crates.
//!
//! This is a host reference model, not an adapter, client, deployment, or
//! financial execution path.  It owns only orchestration and accounting
//! boundaries: the kernel remains the owner of claim transitions, the
//! accumulator remains the owner of interval-summary semantics, and the batch
//! crate remains the owner of candidate construction and verification.

use clutch_accumulator::{CoverageState, Grid, Observation, StatisticError, Summary, SummaryError};
use clutch_batch::{
    Candidate, DustPolicy, FixedBook, FrozenPolicy, Order, PartialPolicy, PriceGrid, Side, TieRule,
    MAX_GRID_TICKS, MAX_ORDERS,
};
use clutch_kernel::{
    Amount, MarketState, PayoutSet, PayoutVector, Phase, Position, MAX_OUTCOMES, MAX_PAYOUTS,
};

/// The reference model has two categorical outcomes and two independent owners.
pub const OUTCOMES: u8 = 2;
pub const OWNERS: usize = 2;
pub const RESOLUTION_THRESHOLD: u128 = 50;
pub const FEE_BPS_DENOMINATOR: u64 = 10_000;
/// Frozen observation horizon for the reference market.
pub const MATURITY_BUCKETS: u64 = 3;

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

/// Batch order plus the owner and outcome that the host settlement boundary
/// must authenticate before it can move any claim or cash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundOrder {
    pub order: Order,
    pub owner: usize,
    pub outcome: u8,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    Kernel(clutch_kernel::Error),
    Summary(SummaryError),
    Batch(clutch_batch::Error),
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

impl From<AccountingError> for ModelError {
    fn from(error: AccountingError) -> Self {
        Self::Accounting(error)
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
        let index = usize::from(outcome);
        if index >= usize::from(self.market.outcomes)
            || self.positions[seller].internal[index] < quantity
        {
            return Err(ModelError::TransferInsufficient);
        }
        if self.cash[buyer] < consideration {
            return Err(ModelError::InsufficientCash);
        }
        let seller_cash = self.cash[seller]
            .checked_add(consideration)
            .ok_or(ModelError::InsufficientCash)?;
        self.positions[seller].internal[index] -= quantity;
        self.positions[buyer].internal[index] = self.positions[buyer].internal[index]
            .checked_add(quantity)
            .ok_or(ModelError::TransferInsufficient)?;
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
        let mut ticks = [0; MAX_GRID_TICKS];
        ticks[0] = 10;
        ticks[1] = 20;
        ticks[2] = 30;
        let grid = PriceGrid::new(ticks, 3)?;
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
            clearing_price: ticks[usize::from(candidate.clearing_tick)],
            settled_by_order: [0; MAX_ORDERS],
            settled_pairs: Vec::new(),
        });
        self.trace.push(format!(
            "batch.clear tick={} matched={} fee={} liveness_paid={liveness_cost}",
            candidate.clearing_tick, candidate.matched, fee
        ));
        Ok(candidate)
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
    let orders = [
        Order {
            canonical_order_id: 1,
            side: Side::Buy,
            limit_tick: 2,
            quantity: 5,
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
        },
        Order {
            canonical_order_id: 2,
            side: Side::Sell,
            limit_tick: 2,
            quantity: 3,
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
        },
    ];
    let bound_orders = [
        BoundOrder {
            order: orders[0],
            owner: 0,
            outcome: 1,
        },
        BoundOrder {
            order: orders[1],
            owner: 1,
            outcome: 1,
        },
    ];
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
            },
            BoundOrder {
                order: orders[1],
                owner: 1,
                outcome: 0,
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
            },
            BoundOrder {
                order: orders[1],
                owner: 1,
                outcome: 0,
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
}
