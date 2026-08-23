// SPDX-License-Identifier: AGPL-3.0-or-later
//! Checked, fixed-memory admission arithmetic for prepaid protocol work.
//!
//! This crate deliberately has no collateral/Hoard input, fee-revenue input,
//! price input, clock, account memory, CPI, allocator, or Solana dependency.
//! It can prove that *named finite payments* are covered by atoms present at
//! admission; it cannot promise transaction inclusion or justify a maximum.

#![no_std]
#![forbid(unsafe_code)]

/// A stable identity supplied and authenticated by an adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Id([u8; 32]);

impl Id {
    /// The padding value. It is not admissible as a live identity.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct an identity from its fixed bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the fixed identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Whether this is the reserved padding identity.
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

/// A fail-closed refusal from admission or a liveness transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ZeroIdentity,
    SameOwnerAndNeutralSink,
    NeutralSinkMismatch,
    ZeroMandatoryMaximum,
    ArithmeticOverflow,
    UnderfundedMarketWork,
    UnderfundedMarketStorage,
    UnderfundedResolution,
    UnderfundedOrder,
    AlreadyTerminal,
    InvalidTransition,
    PaymentExceedsReservation,
    StorageExceedsReservation,
    StorageReturnMismatch,
    FeedCapacityReached,
    DuplicateSubscriber,
    ZeroShare,
    NoncanonicalDeposit,
    EmptyFeed,
    KeeperPaymentExceedsReserve,
    WrongFeeOwner,
    WrongIntent,
    ZeroFeeDenominator,
    NoncanonicalFeeCarry,
    FundingDeltaMismatch,
    AccountBalanceShortfall,
    FeeEnvelopeExceeded,
    TreasuryServiceOutstanding,
    NoServingEpoch,
}

fn add(left: u64, right: u64) -> Result<u64, Error> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

fn sum3(a: u64, b: u64, c: u64) -> Result<u64, Error> {
    add(add(a, b)?, c)
}

fn live_id(id: Id) -> Result<(), Error> {
    if id.is_zero() {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

/// Monotone accounting for unsolicited lamports on one program account.
///
/// This value deliberately owns no principal or work budget. The adapter must
/// first prove that the admission payer transferred the exact named principal
/// and work deposit; any balance that existed before that transfer, or arrives
/// later above the accounted compartments, remains a donation assigned only to
/// the immutable neutral sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DonationLedger {
    neutral_sink: Id,
    donation_lamports: u64,
}

impl DonationLedger {
    /// Admit an account whose exact payer deposit occurs inside the transition.
    ///
    /// `balance_before` cannot be credited to `payer`: it may have been sent by
    /// anyone. `balance_after` must equal that prior balance plus the exact
    /// independently accounted deposit. A caller cannot use prior donations to
    /// reduce the payer's required transfer.
    pub fn admit_prefunded(
        payer: Id,
        neutral_sink: Id,
        balance_before: u64,
        exact_payer_deposit: u64,
        balance_after: u64,
    ) -> Result<Self, Error> {
        live_id(payer)?;
        live_id(neutral_sink)?;
        if payer == neutral_sink {
            return Err(Error::SameOwnerAndNeutralSink);
        }
        let expected_after = add(balance_before, exact_payer_deposit)?;
        if balance_after != expected_after {
            return Err(Error::FundingDeltaMismatch);
        }
        Ok(Self {
            neutral_sink,
            donation_lamports: balance_before,
        })
    }

    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    pub const fn donation_lamports(self) -> u64 {
        self.donation_lamports
    }

    /// Observe the account after a transition without letting donations cover
    /// an accounted principal/work shortfall.
    ///
    /// Previously observed donations are monotone. Any new surplus becomes a
    /// donation as well; it can never be reclassified into another compartment.
    pub fn observe(self, actual_balance: u64, accounted_lamports: u64) -> Result<Self, Error> {
        let minimum = add(accounted_lamports, self.donation_lamports)?;
        if actual_balance < minimum {
            return Err(Error::AccountBalanceShortfall);
        }
        Ok(Self {
            neutral_sink: self.neutral_sink,
            donation_lamports: actual_balance - accounted_lamports,
        })
    }

    /// Quote the donation amount that must be sent to the neutral sink in the
    /// same terminal transaction as all independently accounted dispositions.
    pub fn terminal_split(
        self,
        actual_balance: u64,
        accounted_lamports: u64,
    ) -> Result<DonationDisposition, Error> {
        let observed = self.observe(actual_balance, accounted_lamports)?;
        Ok(DonationDisposition {
            neutral_sink: observed.neutral_sink,
            neutral_lamports: observed.donation_lamports,
        })
    }
}

/// Terminal owner and amount for unsolicited balance only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DonationDisposition {
    pub neutral_sink: Id,
    pub neutral_lamports: u64,
}

/// Unmeasured maxima selected by a Realm policy.
///
/// The kernel treats these numbers as inputs. Their presence here is not SBF
/// cost evidence. Every mandatory compartment is strictly positive so a zero-
/// fee order cannot create uncapitalized keeper work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivenessPolicy {
    pub market_work_max_lamports: u64,
    pub market_storage_max_lamports: u64,
    pub resolution_max_lamports: u64,
    pub per_order_clear_max_lamports: u64,
    pub per_order_settle_max_lamports: u64,
    pub neutral_sink: Id,
}

impl LivenessPolicy {
    /// Validate policy shape and the largest single-admission sums.
    pub fn validate(self) -> Result<(), Error> {
        live_id(self.neutral_sink)?;
        if self.market_work_max_lamports == 0
            || self.market_storage_max_lamports == 0
            || self.resolution_max_lamports == 0
            || self.per_order_clear_max_lamports == 0
            || self.per_order_settle_max_lamports == 0
        {
            return Err(Error::ZeroMandatoryMaximum);
        }
        sum3(
            self.market_work_max_lamports,
            self.market_storage_max_lamports,
            self.resolution_max_lamports,
        )?;
        add(
            self.per_order_clear_max_lamports,
            self.per_order_settle_max_lamports,
        )?;
        Ok(())
    }

    /// Exact market obligations, before any optional excess deposit.
    pub fn market_quote(self) -> Result<MarketQuote, Error> {
        self.validate()?;
        Ok(MarketQuote {
            work_lamports: self.market_work_max_lamports,
            storage_lamports: self.market_storage_max_lamports,
            resolution_lamports: self.resolution_max_lamports,
            total_lamports: sum3(
                self.market_work_max_lamports,
                self.market_storage_max_lamports,
                self.resolution_max_lamports,
            )?,
        })
    }

    /// Exact per-order obligations. Fees are intentionally not an input.
    pub fn order_quote(self) -> Result<OrderQuote, Error> {
        self.validate()?;
        Ok(OrderQuote {
            clear_lamports: self.per_order_clear_max_lamports,
            settle_lamports: self.per_order_settle_max_lamports,
            total_lamports: add(
                self.per_order_clear_max_lamports,
                self.per_order_settle_max_lamports,
            )?,
        })
    }
}

/// Frozen market-level admission requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketQuote {
    pub work_lamports: u64,
    pub storage_lamports: u64,
    pub resolution_lamports: u64,
    pub total_lamports: u64,
}

/// Frozen per-order admission requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderQuote {
    pub clear_lamports: u64,
    pub settle_lamports: u64,
    pub total_lamports: u64,
}

/// Component-wise market funding supplied now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFunding {
    pub work_lamports: u64,
    pub storage_lamports: u64,
    pub resolution_lamports: u64,
}

/// Terminal disposition of a one-shot work reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalDisposition {
    Pending,
    Success,
    Released,
    NeutralFailure,
}

/// One prepaid one-shot keeper job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkReservation {
    maximum_lamports: u64,
    reserved_lamports: u64,
    keeper_paid_lamports: u64,
    refundable_lamports: u64,
    neutral_lamports: u64,
    disposition: TerminalDisposition,
}

impl WorkReservation {
    fn new(maximum_lamports: u64) -> Self {
        Self {
            maximum_lamports,
            reserved_lamports: maximum_lamports,
            keeper_paid_lamports: 0,
            refundable_lamports: 0,
            neutral_lamports: 0,
            disposition: TerminalDisposition::Pending,
        }
    }

    /// Frozen maximum.
    pub const fn maximum_lamports(self) -> u64 {
        self.maximum_lamports
    }

    /// Still reserved for the job.
    pub const fn reserved_lamports(self) -> u64 {
        self.reserved_lamports
    }

    /// Accepted keeper payout.
    pub const fn keeper_paid_lamports(self) -> u64 {
        self.keeper_paid_lamports
    }

    /// Unused maximum refundable to the admission payer.
    pub const fn refundable_lamports(self) -> u64 {
        self.refundable_lamports
    }

    /// Unused maximum irrevocably assigned to the neutral failure sink.
    pub const fn neutral_lamports(self) -> u64 {
        self.neutral_lamports
    }

    pub const fn disposition(self) -> TerminalDisposition {
        self.disposition
    }

    /// Successful execution pays at most the maximum and refunds its residue.
    pub fn succeed(self, keeper_payment: u64) -> Result<Self, Error> {
        self.finish(keeper_payment, TerminalDisposition::Success)
    }

    /// Authorized abandonment before execution refunds the whole reservation.
    pub fn release(self) -> Result<Self, Error> {
        self.finish(0, TerminalDisposition::Released)
    }

    /// Terminal failure pays accepted work and sends the residue to a neutral
    /// sink. It never rewards the market creator, claimant, or treasury here.
    pub fn fail_neutral(self, keeper_payment: u64) -> Result<Self, Error> {
        self.finish(keeper_payment, TerminalDisposition::NeutralFailure)
    }

    fn finish(self, keeper_payment: u64, disposition: TerminalDisposition) -> Result<Self, Error> {
        if self.disposition != TerminalDisposition::Pending {
            return Err(Error::AlreadyTerminal);
        }
        if keeper_payment > self.reserved_lamports {
            return Err(Error::PaymentExceedsReservation);
        }
        let residue = self.reserved_lamports - keeper_payment;
        let (refundable_lamports, neutral_lamports) = match disposition {
            TerminalDisposition::Success => (residue, 0),
            TerminalDisposition::Released => (self.reserved_lamports, 0),
            TerminalDisposition::NeutralFailure => (0, residue),
            TerminalDisposition::Pending => return Err(Error::InvalidTransition),
        };
        Ok(Self {
            maximum_lamports: self.maximum_lamports,
            reserved_lamports: 0,
            keeper_paid_lamports: keeper_payment,
            refundable_lamports,
            neutral_lamports,
            disposition,
        })
    }

    fn accounted(self) -> Result<u64, Error> {
        sum3(
            self.reserved_lamports,
            self.keeper_paid_lamports,
            add(self.refundable_lamports, self.neutral_lamports)?,
        )
    }
}

/// One storage/rent-principal reservation. It cannot become a keeper reward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageReservation {
    maximum_lamports: u64,
    reserved_lamports: u64,
    locked_lamports: u64,
    refundable_lamports: u64,
    disposition: TerminalDisposition,
}

impl StorageReservation {
    fn new(maximum_lamports: u64) -> Self {
        Self {
            maximum_lamports,
            reserved_lamports: maximum_lamports,
            locked_lamports: 0,
            refundable_lamports: 0,
            disposition: TerminalDisposition::Pending,
        }
    }

    pub const fn maximum_lamports(self) -> u64 {
        self.maximum_lamports
    }

    pub const fn reserved_lamports(self) -> u64 {
        self.reserved_lamports
    }

    pub const fn locked_lamports(self) -> u64 {
        self.locked_lamports
    }

    pub const fn refundable_lamports(self) -> u64 {
        self.refundable_lamports
    }

    pub const fn disposition(self) -> TerminalDisposition {
        self.disposition
    }

    /// Lock the actual rent principal and immediately refund unused headroom.
    pub fn lock(self, actual_lamports: u64) -> Result<Self, Error> {
        if self.disposition != TerminalDisposition::Pending {
            return Err(Error::AlreadyTerminal);
        }
        if actual_lamports > self.reserved_lamports {
            return Err(Error::StorageExceedsReservation);
        }
        Ok(Self {
            maximum_lamports: self.maximum_lamports,
            reserved_lamports: 0,
            locked_lamports: actual_lamports,
            refundable_lamports: self.reserved_lamports - actual_lamports,
            disposition: TerminalDisposition::Success,
        })
    }

    /// Release unused storage headroom before an account is created.
    pub fn release(self) -> Result<Self, Error> {
        if self.disposition != TerminalDisposition::Pending {
            return Err(Error::AlreadyTerminal);
        }
        Ok(Self {
            maximum_lamports: self.maximum_lamports,
            reserved_lamports: 0,
            locked_lamports: 0,
            refundable_lamports: self.reserved_lamports,
            disposition: TerminalDisposition::Released,
        })
    }

    /// Return exactly the originally locked principal after valid closure.
    pub fn close(self, returned_principal: u64) -> Result<Self, Error> {
        if self.disposition != TerminalDisposition::Success || self.locked_lamports == 0 {
            return Err(Error::InvalidTransition);
        }
        if returned_principal != self.locked_lamports {
            return Err(Error::StorageReturnMismatch);
        }
        Ok(Self {
            maximum_lamports: self.maximum_lamports,
            reserved_lamports: 0,
            locked_lamports: 0,
            refundable_lamports: add(self.refundable_lamports, returned_principal)?,
            disposition: TerminalDisposition::Released,
        })
    }

    fn accounted(self) -> Result<u64, Error> {
        sum3(
            self.reserved_lamports,
            self.locked_lamports,
            self.refundable_lamports,
        )
    }
}

/// Prepaid market-level reserve, partitioned by semantic purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketEndowment {
    owner: Id,
    neutral_sink: Id,
    funded_lamports: u64,
    admission_residue_lamports: u64,
    pub work: WorkReservation,
    pub storage: StorageReservation,
    pub resolution: WorkReservation,
}

impl MarketEndowment {
    /// Admit from present component-wise deposits only.
    pub fn admit(owner: Id, policy: LivenessPolicy, funding: MarketFunding) -> Result<Self, Error> {
        live_id(owner)?;
        policy.validate()?;
        if owner == policy.neutral_sink {
            return Err(Error::SameOwnerAndNeutralSink);
        }
        if funding.work_lamports < policy.market_work_max_lamports {
            return Err(Error::UnderfundedMarketWork);
        }
        if funding.storage_lamports < policy.market_storage_max_lamports {
            return Err(Error::UnderfundedMarketStorage);
        }
        if funding.resolution_lamports < policy.resolution_max_lamports {
            return Err(Error::UnderfundedResolution);
        }
        let funded_lamports = sum3(
            funding.work_lamports,
            funding.storage_lamports,
            funding.resolution_lamports,
        )?;
        let admission_residue_lamports = sum3(
            funding.work_lamports - policy.market_work_max_lamports,
            funding.storage_lamports - policy.market_storage_max_lamports,
            funding.resolution_lamports - policy.resolution_max_lamports,
        )?;
        Ok(Self {
            owner,
            neutral_sink: policy.neutral_sink,
            funded_lamports,
            admission_residue_lamports,
            work: WorkReservation::new(policy.market_work_max_lamports),
            storage: StorageReservation::new(policy.market_storage_max_lamports),
            resolution: WorkReservation::new(policy.resolution_max_lamports),
        })
    }

    pub const fn owner(self) -> Id {
        self.owner
    }

    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    pub const fn funded_lamports(self) -> u64 {
        self.funded_lamports
    }

    /// Every refundable residue owned by the admission payer.
    pub fn refundable_lamports(self) -> Result<u64, Error> {
        add(
            add(
                self.admission_residue_lamports,
                self.work.refundable_lamports(),
            )?,
            add(
                self.storage.refundable_lamports(),
                self.resolution.refundable_lamports(),
            )?,
        )
    }

    /// The conserved partition of all admitted lamports.
    pub fn accounted_lamports(self) -> Result<u64, Error> {
        add(
            self.admission_residue_lamports,
            add(
                add(self.work.accounted()?, self.storage.accounted()?)?,
                self.resolution.accounted()?,
            )?,
        )
    }

    pub fn succeed_work(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.work = self.work.succeed(keeper_payment)?;
        Ok(self)
    }

    pub fn fail_work_neutral(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.work = self.work.fail_neutral(keeper_payment)?;
        Ok(self)
    }

    pub fn succeed_resolution(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.resolution = self.resolution.succeed(keeper_payment)?;
        Ok(self)
    }

    pub fn fail_resolution_neutral(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.resolution = self.resolution.fail_neutral(keeper_payment)?;
        Ok(self)
    }

    pub fn lock_storage(mut self, actual_principal: u64) -> Result<Self, Error> {
        self.storage = self.storage.lock(actual_principal)?;
        Ok(self)
    }

    pub fn close_storage(mut self, returned_principal: u64) -> Result<Self, Error> {
        self.storage = self.storage.close(returned_principal)?;
        Ok(self)
    }

    /// Adapter-authorized terminal release. The kernel does not decide whether
    /// market liabilities make cancellation legal.
    pub fn release_unstarted(mut self) -> Result<Self, Error> {
        self.work = self.work.release()?;
        self.storage = self.storage.release()?;
        self.resolution = self.resolution.release()?;
        Ok(self)
    }

    /// Adapter-authorized release of a still-pending market-work job.
    pub fn release_work(mut self) -> Result<Self, Error> {
        self.work = self.work.release()?;
        Ok(self)
    }

    /// Adapter-authorized release of still-pending storage headroom.
    pub fn release_storage(mut self) -> Result<Self, Error> {
        self.storage = self.storage.release()?;
        Ok(self)
    }

    /// Adapter-authorized release of a still-pending resolution job.
    pub fn release_resolution(mut self) -> Result<Self, Error> {
        self.resolution = self.resolution.release()?;
        Ok(self)
    }
}

/// Per-order prepaid clear and settlement work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderEndowment {
    owner: Id,
    order_id: Id,
    neutral_sink: Id,
    funded_lamports: u64,
    admission_residue_lamports: u64,
    pub clear: WorkReservation,
    pub settle: WorkReservation,
}

impl OrderEndowment {
    /// `funding_lamports` is required even when the venue fee is exactly zero.
    pub fn admit(
        owner: Id,
        order_id: Id,
        policy: LivenessPolicy,
        funding_lamports: u64,
    ) -> Result<Self, Error> {
        live_id(owner)?;
        live_id(order_id)?;
        policy.validate()?;
        if owner == policy.neutral_sink {
            return Err(Error::SameOwnerAndNeutralSink);
        }
        let quote = policy.order_quote()?;
        if funding_lamports < quote.total_lamports {
            return Err(Error::UnderfundedOrder);
        }
        Ok(Self {
            owner,
            order_id,
            neutral_sink: policy.neutral_sink,
            funded_lamports: funding_lamports,
            admission_residue_lamports: funding_lamports - quote.total_lamports,
            clear: WorkReservation::new(quote.clear_lamports),
            settle: WorkReservation::new(quote.settle_lamports),
        })
    }

    pub const fn owner(self) -> Id {
        self.owner
    }

    pub const fn order_id(self) -> Id {
        self.order_id
    }

    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    pub const fn funded_lamports(self) -> u64 {
        self.funded_lamports
    }

    pub fn refundable_lamports(self) -> Result<u64, Error> {
        add(
            self.admission_residue_lamports,
            add(
                self.clear.refundable_lamports(),
                self.settle.refundable_lamports(),
            )?,
        )
    }

    pub fn accounted_lamports(self) -> Result<u64, Error> {
        add(
            self.admission_residue_lamports,
            add(self.clear.accounted()?, self.settle.accounted()?)?,
        )
    }

    pub fn succeed_clear(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.clear = self.clear.succeed(keeper_payment)?;
        Ok(self)
    }

    pub fn fail_clear_neutral(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.clear = self.clear.fail_neutral(keeper_payment)?;
        Ok(self)
    }

    pub fn succeed_settle(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.settle = self.settle.succeed(keeper_payment)?;
        Ok(self)
    }

    pub fn fail_settle_neutral(mut self, keeper_payment: u64) -> Result<Self, Error> {
        self.settle = self.settle.fail_neutral(keeper_payment)?;
        Ok(self)
    }

    /// Release both jobs only while neither has executed.
    pub fn release_unstarted(mut self) -> Result<Self, Error> {
        self.clear = self.clear.release()?;
        self.settle = self.settle.release()?;
        Ok(self)
    }

    /// After clear terminates, release a still-pending settle reservation.
    pub fn release_unsettled(mut self) -> Result<Self, Error> {
        if self.clear.disposition() == TerminalDisposition::Pending {
            return Err(Error::InvalidTransition);
        }
        self.settle = self.settle.release()?;
        Ok(self)
    }
}

/// One canonical subscriber/share entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriberShare {
    pub subscriber: Id,
    pub lamports: u64,
}

impl SubscriberShare {
    const EMPTY: Self = Self {
        subscriber: Id::ZERO,
        lamports: 0,
    };
}

/// Reimbursement paid to an incumbent during a join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reimbursement {
    pub subscriber: Id,
    pub lamports: u64,
}

impl Reimbursement {
    const EMPTY: Self = Self {
        subscriber: Id::ZERO,
        lamports: 0,
    };
}

/// Atomic evidence for a shared-feed join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedJoinReceipt<const N: usize> {
    pub subscriber: Id,
    pub deposit_lamports: u64,
    reimbursements: [Reimbursement; N],
    reimbursement_count: usize,
}

impl<const N: usize> FeedJoinReceipt<N> {
    pub fn reimbursements(&self) -> &[Reimbursement] {
        &self.reimbursements[..self.reimbursement_count]
    }
}

/// Per-subscriber terminal cost/refund evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedAllocation {
    pub subscriber: Id,
    pub cost_lamports: u64,
    pub refund_lamports: u64,
}

impl FeedAllocation {
    const EMPTY: Self = Self {
        subscriber: Id::ZERO,
        cost_lamports: 0,
        refund_lamports: 0,
    };
}

/// Terminal shared-feed settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedSettlement<const N: usize> {
    pub success: bool,
    pub keeper_paid_lamports: u64,
    pub neutral_sink: Id,
    pub neutral_lamports: u64,
    allocations: [FeedAllocation; N],
    allocation_count: usize,
}

impl<const N: usize> FeedSettlement<N> {
    pub fn allocations(&self) -> &[FeedAllocation] {
        &self.allocations[..self.allocation_count]
    }
}

/// A fully capitalized shared source or archive reserve.
///
/// Arrival order is the canonical share order. A join that would receive zero
/// atoms is refused, closing the free-subscriber spam surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedFeedReserve<const N: usize> {
    cap_lamports: u64,
    reserve_lamports: u64,
    neutral_sink: Id,
    shares: [SubscriberShare; N],
    subscriber_count: usize,
}

impl<const N: usize> SharedFeedReserve<N> {
    pub fn empty(cap_lamports: u64, neutral_sink: Id) -> Result<Self, Error> {
        live_id(neutral_sink)?;
        Ok(Self {
            cap_lamports,
            reserve_lamports: 0,
            neutral_sink,
            shares: [SubscriberShare::EMPTY; N],
            subscriber_count: 0,
        })
    }

    pub const fn cap_lamports(self) -> u64 {
        self.cap_lamports
    }

    pub const fn reserve_lamports(self) -> u64 {
        self.reserve_lamports
    }

    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    pub const fn subscriber_count(self) -> usize {
        self.subscriber_count
    }

    pub fn shares(&self) -> &[SubscriberShare] {
        &self.shares[..self.subscriber_count]
    }

    /// Exact deposit required from the next arrival-order subscriber.
    pub fn required_join_deposit(&self) -> Result<u64, Error> {
        if self.subscriber_count == N {
            return Err(Error::FeedCapacityReached);
        }
        let next_count = self
            .subscriber_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = u64::try_from(next_count).map_err(|_| Error::ArithmeticOverflow)?;
        let required = if self.subscriber_count == 0 {
            self.cap_lamports
        } else {
            self.cap_lamports / denominator
        };
        if required == 0 {
            return Err(Error::ZeroShare);
        }
        Ok(required)
    }

    /// Join exactly at the current canonical share and reimburse incumbents.
    pub fn join(
        self,
        subscriber: Id,
        deposit_lamports: u64,
    ) -> Result<(Self, FeedJoinReceipt<N>), Error> {
        live_id(subscriber)?;
        if subscriber == self.neutral_sink {
            return Err(Error::SameOwnerAndNeutralSink);
        }
        for share in self.shares() {
            if share.subscriber == subscriber {
                return Err(Error::DuplicateSubscriber);
            }
        }
        let required = self.required_join_deposit()?;
        if deposit_lamports != required {
            return Err(Error::NoncanonicalDeposit);
        }
        let new_count = self.subscriber_count + 1;
        let denominator = u64::try_from(new_count).map_err(|_| Error::ArithmeticOverflow)?;
        let quotient = self.cap_lamports / denominator;
        let remainder = self.cap_lamports % denominator;
        let remainder_count = usize::try_from(remainder).map_err(|_| Error::ArithmeticOverflow)?;
        let mut next = self;
        let mut reimbursements = [Reimbursement::EMPTY; N];
        let mut reimbursement_total = 0u64;
        for (index, reimbursement_slot) in reimbursements
            .iter_mut()
            .enumerate()
            .take(self.subscriber_count)
        {
            let new_share = quotient + u64::from(index < remainder_count);
            let old_share = self.shares[index].lamports;
            let reimbursement = old_share
                .checked_sub(new_share)
                .ok_or(Error::ArithmeticOverflow)?;
            *reimbursement_slot = Reimbursement {
                subscriber: self.shares[index].subscriber,
                lamports: reimbursement,
            };
            reimbursement_total = add(reimbursement_total, reimbursement)?;
            next.shares[index].lamports = new_share;
        }
        next.shares[self.subscriber_count] = SubscriberShare {
            subscriber,
            lamports: quotient,
        };
        next.subscriber_count = new_count;
        if self.subscriber_count == 0 {
            next.reserve_lamports = deposit_lamports;
        } else if reimbursement_total != deposit_lamports {
            return Err(Error::NoncanonicalDeposit);
        }
        if next.reserve_lamports != next.cap_lamports {
            return Err(Error::NoncanonicalDeposit);
        }
        Ok((
            next,
            FeedJoinReceipt {
                subscriber,
                deposit_lamports,
                reimbursements,
                reimbursement_count: self.subscriber_count,
            },
        ))
    }

    /// Settle success with canonical costs/refunds, or terminal failure with no
    /// subscriber refund and a predeclared neutral residual destination.
    pub fn settle(self, keeper_payment: u64, success: bool) -> Result<FeedSettlement<N>, Error> {
        if self.subscriber_count == 0 {
            return Err(Error::EmptyFeed);
        }
        if keeper_payment > self.reserve_lamports {
            return Err(Error::KeeperPaymentExceedsReserve);
        }
        let count = u64::try_from(self.subscriber_count).map_err(|_| Error::ArithmeticOverflow)?;
        let quotient = keeper_payment / count;
        let remainder = keeper_payment % count;
        let remainder_count = usize::try_from(remainder).map_err(|_| Error::ArithmeticOverflow)?;
        let mut allocations = [FeedAllocation::EMPTY; N];
        for (index, allocation_slot) in allocations
            .iter_mut()
            .enumerate()
            .take(self.subscriber_count)
        {
            let cost = if success {
                quotient + u64::from(index < remainder_count)
            } else {
                self.shares[index].lamports
            };
            let refund = if success {
                self.shares[index]
                    .lamports
                    .checked_sub(cost)
                    .ok_or(Error::ArithmeticOverflow)?
            } else {
                0
            };
            *allocation_slot = FeedAllocation {
                subscriber: self.shares[index].subscriber,
                cost_lamports: cost,
                refund_lamports: refund,
            };
        }
        Ok(FeedSettlement {
            success,
            keeper_paid_lamports: keeper_payment,
            neutral_sink: self.neutral_sink,
            neutral_lamports: if success {
                0
            } else {
                self.reserve_lamports - keeper_payment
            },
            allocations,
            allocation_count: self.subscriber_count,
        })
    }
}

/// Source and archive capitalization are two independent reserves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedFeedPair<const N: usize> {
    pub source: SharedFeedReserve<N>,
    pub archive: SharedFeedReserve<N>,
}

/// Atomic receipts from a source-plus-archive subscription.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedPairJoinReceipt<const N: usize> {
    pub source: FeedJoinReceipt<N>,
    pub archive: FeedJoinReceipt<N>,
}

impl<const N: usize> SharedFeedPair<N> {
    pub fn empty(
        source_cap_lamports: u64,
        archive_cap_lamports: u64,
        neutral_sink: Id,
    ) -> Result<Self, Error> {
        Ok(Self {
            source: SharedFeedReserve::empty(source_cap_lamports, neutral_sink)?,
            archive: SharedFeedReserve::empty(archive_cap_lamports, neutral_sink)?,
        })
    }

    pub fn required_join_deposits(&self) -> Result<(u64, u64), Error> {
        Ok((
            self.source.required_join_deposit()?,
            self.archive.required_join_deposit()?,
        ))
    }

    /// Validate and join both reserves atomically in this value model.
    pub fn join(
        self,
        subscriber: Id,
        source_deposit_lamports: u64,
        archive_deposit_lamports: u64,
    ) -> Result<(Self, FeedPairJoinReceipt<N>), Error> {
        let (required_source, required_archive) = self.required_join_deposits()?;
        if source_deposit_lamports != required_source
            || archive_deposit_lamports != required_archive
        {
            return Err(Error::NoncanonicalDeposit);
        }
        let (source, source_receipt) = self.source.join(subscriber, source_deposit_lamports)?;
        let (archive, archive_receipt) = self.archive.join(subscriber, archive_deposit_lamports)?;
        Ok((
            Self { source, archive },
            FeedPairJoinReceipt {
                source: source_receipt,
                archive: archive_receipt,
            },
        ))
    }
}

/// All present funding for one market-level admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketAdmissionFunding {
    pub market: MarketFunding,
    pub source_join_lamports: u64,
    pub archive_join_lamports: u64,
}

/// Market plus source/archive quote at the current subscriber counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketAdmissionQuote {
    pub market: MarketQuote,
    pub source_join_lamports: u64,
    pub archive_join_lamports: u64,
    pub total_lamports: u64,
}

/// Atomic value-model result of market and shared-feed admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedMarket<const N: usize> {
    pub endowment: MarketEndowment,
    pub feeds: SharedFeedPair<N>,
    pub feed_join: FeedPairJoinReceipt<N>,
}

/// Quote all market-level mandatory obligations without a future-volume input.
pub fn quote_market_admission<const N: usize>(
    policy: LivenessPolicy,
    feeds: &SharedFeedPair<N>,
) -> Result<MarketAdmissionQuote, Error> {
    policy.validate()?;
    if feeds.source.neutral_sink() != policy.neutral_sink
        || feeds.archive.neutral_sink() != policy.neutral_sink
    {
        return Err(Error::NeutralSinkMismatch);
    }
    let market = policy.market_quote()?;
    let (source_join_lamports, archive_join_lamports) = feeds.required_join_deposits()?;
    Ok(MarketAdmissionQuote {
        market,
        source_join_lamports,
        archive_join_lamports,
        total_lamports: add(
            market.total_lamports,
            add(source_join_lamports, archive_join_lamports)?,
        )?,
    })
}

/// Admit a market and its source/archive subscriptions as one pure transition.
///
/// Component underfunding refuses independently. Feed deposits are exact
/// because any excess would have no canonical owner in the sharing relation.
pub fn admit_market<const N: usize>(
    owner: Id,
    policy: LivenessPolicy,
    feeds: SharedFeedPair<N>,
    funding: MarketAdmissionFunding,
) -> Result<AdmittedMarket<N>, Error> {
    let quote = quote_market_admission(policy, &feeds)?;
    if funding.source_join_lamports != quote.source_join_lamports
        || funding.archive_join_lamports != quote.archive_join_lamports
    {
        return Err(Error::NoncanonicalDeposit);
    }
    let endowment = MarketEndowment::admit(owner, policy, funding.market)?;
    let (feeds, feed_join) = feeds.join(
        owner,
        funding.source_join_lamports,
        funding.archive_join_lamports,
    )?;
    Ok(AdmittedMarket {
        endowment,
        feeds,
        feed_join,
    })
}

/// Persistent fractional fee carry owned by one signed intent.
///
/// This object accounts fee atoms only. No method can move them into a work,
/// storage, resolution, source, or archive reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentFeeCarry {
    owner: Id,
    intent_id: Id,
    denominator: u128,
    remainder: u128,
    paid_atoms: u64,
    closed: bool,
}

/// Atoms charged by one fee-carry transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeCharge {
    pub atoms: u64,
    pub remainder: u128,
    pub terminal: bool,
}

impl IntentFeeCarry {
    /// Open one carry under the exact denominator frozen by the fee policy.
    ///
    /// The width is intentionally `u128`: the selected composite base uses
    /// `10_000 * price_scale^2 * 10_000`, which exceeds `u64` for admitted
    /// price scales above 429,496.  Narrowing that denominator would make the
    /// kernel disagree with the verifier before an account codec even exists.
    pub fn admit(owner: Id, intent_id: Id, denominator: u128) -> Result<Self, Error> {
        live_id(owner)?;
        live_id(intent_id)?;
        if denominator == 0 {
            return Err(Error::ZeroFeeDenominator);
        }
        Ok(Self {
            owner,
            intent_id,
            denominator,
            remainder: 0,
            paid_atoms: 0,
            closed: false,
        })
    }

    pub const fn owner(self) -> Id {
        self.owner
    }

    pub const fn intent_id(self) -> Id {
        self.intent_id
    }

    pub const fn denominator(self) -> u128 {
        self.denominator
    }

    pub const fn remainder(self) -> u128 {
        self.remainder
    }

    pub const fn paid_atoms(self) -> u64 {
        self.paid_atoms
    }

    pub const fn is_closed(self) -> bool {
        self.closed
    }

    fn authenticate(self, owner: Id, intent_id: Id) -> Result<(), Error> {
        if owner != self.owner {
            return Err(Error::WrongFeeOwner);
        }
        if intent_id != self.intent_id {
            return Err(Error::WrongIntent);
        }
        if self.closed {
            return Err(Error::AlreadyTerminal);
        }
        if self.remainder >= self.denominator {
            return Err(Error::NoncanonicalFeeCarry);
        }
        Ok(())
    }

    /// Add an exact fragment numerator under the carry's frozen denominator.
    pub fn charge_fragment(
        mut self,
        owner: Id,
        intent_id: Id,
        exact_numerator: u128,
    ) -> Result<(Self, FeeCharge), Error> {
        self.authenticate(owner, intent_id)?;
        let accumulated = self
            .remainder
            .checked_add(exact_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        let atoms =
            u64::try_from(accumulated / self.denominator).map_err(|_| Error::ArithmeticOverflow)?;
        self.remainder = accumulated % self.denominator;
        self.paid_atoms = add(self.paid_atoms, atoms)?;
        Ok((
            self,
            FeeCharge {
                atoms,
                remainder: self.remainder,
                terminal: false,
            },
        ))
    }

    /// Close exactly once with a single ceiling atom when carry remains.
    pub fn close(mut self, owner: Id, intent_id: Id) -> Result<(Self, FeeCharge), Error> {
        self.authenticate(owner, intent_id)?;
        let atoms = u64::from(self.remainder != 0);
        self.paid_atoms = add(self.paid_atoms, atoms)?;
        self.remainder = 0;
        self.closed = true;
        Ok((
            self,
            FeeCharge {
                atoms,
                remainder: 0,
                terminal: true,
            },
        ))
    }
}

/// The (D8) owner-signed fee envelope enforced over one [`IntentFeeCarry`]:
/// the carry arithmetic of `docs/design/REVENUE_POLICY_V1.md` §6, host-side,
/// ahead of its reservation-vNext byte home (the reservation format bump is
/// deliberately NOT taken here — it lands once, later, carrying this and the
/// partial-fill design's answer together).
///
/// Invariant, maintained at every transition: `paid_atoms` **plus the
/// terminal-ceil atom the open remainder already commits to** never exceeds
/// the owner-signed `max_fee_atoms`.  Because the commitment is checked at
/// each fragment, the close can always land inside the envelope — an intent
/// can never be marooned with a remainder its envelope cannot pay.  A zero
/// envelope therefore refuses the very first nonzero numerator: under the
/// zero-fee policy (and under the fee shape's zero rates) no charge path can
/// touch the carry with anything but exact zeros.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopedIntentFeeCarry {
    carry: IntentFeeCarry,
    max_fee_atoms: u64,
}

impl EnvelopedIntentFeeCarry {
    /// Admit one intent's carry under its owner-signed envelope.
    ///
    /// `worst_case_fee_atoms` is the admission-time quote of the largest
    /// total the policy can charge this intent, **terminal-ceil atom
    /// included** (design §8.3 requires every admissible base to supply it);
    /// admission refuses an envelope below it, so a signer's bound is
    /// checked before any fill exists.  At zero rates the worst case is
    /// exactly zero and the signed zero envelope admits.
    pub fn admit(
        owner: Id,
        intent_id: Id,
        denominator: u128,
        worst_case_fee_atoms: u64,
        max_fee_atoms: u64,
    ) -> Result<Self, Error> {
        if worst_case_fee_atoms > max_fee_atoms {
            return Err(Error::FeeEnvelopeExceeded);
        }
        Ok(Self {
            carry: IntentFeeCarry::admit(owner, intent_id, denominator)?,
            max_fee_atoms,
        })
    }

    /// The wrapped carry state.
    pub const fn carry(self) -> IntentFeeCarry {
        self.carry
    }

    /// The owner-signed envelope.
    pub const fn max_fee_atoms(self) -> u64 {
        self.max_fee_atoms
    }

    /// Atoms already paid plus the ceil atom the open remainder commits to.
    pub fn committed_atoms(self) -> Result<u64, Error> {
        add(
            self.carry.paid_atoms(),
            u64::from(!self.carry.is_closed() && self.carry.remainder() != 0),
        )
    }

    /// Charge one exact fragment numerator, refusing any charge whose
    /// committed total (terminal ceil included) would exceed the envelope.
    /// A refused charge leaves the caller's copy exactly as it was.
    pub fn charge_fragment(
        self,
        owner: Id,
        intent_id: Id,
        exact_numerator: u128,
    ) -> Result<(Self, FeeCharge), Error> {
        let (carry, charge) = self
            .carry
            .charge_fragment(owner, intent_id, exact_numerator)?;
        let next = Self {
            carry,
            max_fee_atoms: self.max_fee_atoms,
        };
        if next.committed_atoms()? > self.max_fee_atoms {
            return Err(Error::FeeEnvelopeExceeded);
        }
        Ok((next, charge))
    }

    /// Close once; the ceiling atom is inside the envelope by the fragment
    /// invariant, and that is asserted rather than assumed.
    pub fn close(self, owner: Id, intent_id: Id) -> Result<(Self, FeeCharge), Error> {
        let (carry, charge) = self.carry.close(owner, intent_id)?;
        if carry.paid_atoms() > self.max_fee_atoms {
            return Err(Error::FeeEnvelopeExceeded);
        }
        Ok((
            Self {
                carry,
                max_fee_atoms: self.max_fee_atoms,
            },
            charge,
        ))
    }
}

/// The B4b mid-epoch-close grief rider, as fixed-memory bookkeeping: **a
/// treasury Position serving an unsettled fee-bearing epoch refuses close.**
///
/// Adopted 2026-08-20 (item 8 on `REPORT_revenue-policy-v1` §2, stress point
/// 1): the treasury Position is owner-closable like any Position, and a
/// mid-epoch close would let the fee recipient halt *other parties'*
/// settlement.  No Position close route exists in the program today
/// (`close_state` is written by no handler), so this kernel is the rider's
/// executable form ahead of its byte home — exactly as [`IntentFeeCarry`]
/// preceded its reservation host.  Any future Position close route MUST
/// consult a ledger with these semantics:
///
/// * fee-bearing epoch admission calls [`Self::begin_service`] for the
///   Market's treasury Position;
/// * the epoch's terminal settlement (its root close) calls
///   [`Self::settle_service`] — never more times than services began;
/// * [`Self::close`] refuses while any service is outstanding, and closes
///   exactly once.
///
/// The ledger is a counter, deliberately: this crate holds no account
/// memory, and *which* epoch settles is authenticated by the adapter against
/// the epoch account itself.  The counter enforces the arithmetic invariant
/// the refusal set needs — close only after every begun service settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreasuryServiceLedger {
    treasury_position: Id,
    outstanding: u64,
    closed: bool,
}

impl TreasuryServiceLedger {
    /// Open the ledger for one treasury Position identity.
    pub fn admit(treasury_position: Id) -> Result<Self, Error> {
        live_id(treasury_position)?;
        Ok(Self {
            treasury_position,
            outstanding: 0,
            closed: false,
        })
    }

    /// The treasury Position this ledger guards.
    pub const fn treasury_position(self) -> Id {
        self.treasury_position
    }

    /// Fee-bearing epochs admitted and not yet settled.
    pub const fn outstanding(self) -> u64 {
        self.outstanding
    }

    /// Whether the guarded Position has closed.
    pub const fn is_closed(self) -> bool {
        self.closed
    }

    fn live(self) -> Result<(), Error> {
        if self.closed {
            return Err(Error::AlreadyTerminal);
        }
        Ok(())
    }

    /// One fee-bearing epoch admitted against this treasury Position.
    pub fn begin_service(mut self) -> Result<Self, Error> {
        self.live()?;
        self.outstanding = add(self.outstanding, 1)?;
        Ok(self)
    }

    /// One served fee-bearing epoch fully settled.  Refuses a settlement
    /// nobody began: the counter can never go negative, so a hostile
    /// pre-settle cannot bank free closes.
    pub fn settle_service(mut self) -> Result<Self, Error> {
        self.live()?;
        if self.outstanding == 0 {
            return Err(Error::NoServingEpoch);
        }
        self.outstanding -= 1;
        Ok(self)
    }

    /// The rider itself: close refuses while any served epoch is unsettled,
    /// and lands exactly once.
    pub fn close(mut self) -> Result<Self, Error> {
        self.live()?;
        if self.outstanding != 0 {
            return Err(Error::TreasuryServiceOutstanding);
        }
        self.closed = true;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn policy() -> LivenessPolicy {
        LivenessPolicy {
            market_work_max_lamports: 11,
            market_storage_max_lamports: 7,
            resolution_max_lamports: 13,
            per_order_clear_max_lamports: 5,
            per_order_settle_max_lamports: 9,
            neutral_sink: id(250),
        }
    }

    #[test]
    fn prefund_is_neutral_donation_not_payer_principal() {
        let ledger = DonationLedger::admit_prefunded(id(1), id(250), 1, 17, 18).unwrap();
        assert_eq!(ledger.neutral_sink(), id(250));
        assert_eq!(ledger.donation_lamports(), 1);

        let terminal = ledger.terminal_split(18, 17).unwrap();
        assert_eq!(terminal.neutral_sink, id(250));
        assert_eq!(terminal.neutral_lamports, 1);
    }

    #[test]
    fn payer_must_supply_the_exact_accounted_deposit() {
        assert_eq!(
            DonationLedger::admit_prefunded(id(1), id(250), 9, 17, 17),
            Err(Error::FundingDeltaMismatch)
        );
        assert_eq!(
            DonationLedger::admit_prefunded(id(1), id(250), 9, 17, 25),
            Err(Error::FundingDeltaMismatch)
        );
        assert_eq!(
            DonationLedger::admit_prefunded(id(1), id(1), 0, 17, 17),
            Err(Error::SameOwnerAndNeutralSink)
        );
        assert_eq!(
            DonationLedger::admit_prefunded(id(1), id(250), u64::MAX, 1, 0),
            Err(Error::ArithmeticOverflow)
        );
    }

    #[test]
    fn later_donations_are_monotone_and_cannot_cover_a_shortfall() {
        let ledger = DonationLedger::admit_prefunded(id(1), id(250), 3, 20, 23).unwrap();
        assert_eq!(ledger.observe(22, 20), Err(Error::AccountBalanceShortfall));

        let grown = ledger.observe(28, 20).unwrap();
        assert_eq!(grown.donation_lamports(), 8);
        assert_eq!(grown.observe(27, 20), Err(Error::AccountBalanceShortfall));
        assert_eq!(
            grown.terminal_split(28, 20),
            Ok(DonationDisposition {
                neutral_sink: id(250),
                neutral_lamports: 8,
            })
        );
    }

    #[test]
    fn market_admission_is_component_wise_and_conserving() {
        let exact = MarketFunding {
            work_lamports: 11,
            storage_lamports: 7,
            resolution_lamports: 13,
        };
        for underfunded in [
            MarketFunding {
                work_lamports: 10,
                ..exact
            },
            MarketFunding {
                storage_lamports: 6,
                ..exact
            },
            MarketFunding {
                resolution_lamports: 12,
                ..exact
            },
        ] {
            assert!(MarketEndowment::admit(id(1), policy(), underfunded).is_err());
        }
        let state = MarketEndowment::admit(
            id(1),
            policy(),
            MarketFunding {
                work_lamports: 14,
                storage_lamports: 9,
                resolution_lamports: 17,
            },
        )
        .unwrap();
        assert_eq!(state.funded_lamports(), 40);
        assert_eq!(state.accounted_lamports(), Ok(40));
        assert_eq!(state.refundable_lamports(), Ok(9));
    }

    #[test]
    fn success_release_and_neutral_failure_freeze_residue_ownership() {
        let success = WorkReservation::new(11).succeed(4).unwrap();
        assert_eq!(success.keeper_paid_lamports(), 4);
        assert_eq!(success.refundable_lamports(), 7);
        assert_eq!(success.neutral_lamports(), 0);
        assert_eq!(success.accounted(), Ok(11));

        let released = WorkReservation::new(11).release().unwrap();
        assert_eq!(released.refundable_lamports(), 11);
        assert_eq!(released.accounted(), Ok(11));

        let failed = WorkReservation::new(11).fail_neutral(4).unwrap();
        assert_eq!(failed.keeper_paid_lamports(), 4);
        assert_eq!(failed.refundable_lamports(), 0);
        assert_eq!(failed.neutral_lamports(), 7);
        assert_eq!(failed.accounted(), Ok(11));
        assert_eq!(failed.succeed(0), Err(Error::AlreadyTerminal));
        assert_eq!(
            WorkReservation::new(11).succeed(12),
            Err(Error::PaymentExceedsReservation)
        );
    }

    #[test]
    fn storage_principal_never_becomes_keeper_pay() {
        let state = MarketEndowment::admit(
            id(1),
            policy(),
            MarketFunding {
                work_lamports: 11,
                storage_lamports: 7,
                resolution_lamports: 13,
            },
        )
        .unwrap()
        .lock_storage(5)
        .unwrap();
        assert_eq!(state.storage.locked_lamports(), 5);
        assert_eq!(state.storage.refundable_lamports(), 2);
        assert_eq!(state.accounted_lamports(), Ok(31));
        assert_eq!(state.close_storage(4), Err(Error::StorageReturnMismatch));
        let closed = state.close_storage(5).unwrap();
        assert_eq!(closed.storage.refundable_lamports(), 7);
        assert_eq!(closed.accounted_lamports(), Ok(31));
    }

    #[test]
    fn zero_fee_order_still_prepays_clear_and_settle() {
        let quote = policy().order_quote().unwrap();
        assert_eq!(quote.total_lamports, 14);
        assert_eq!(
            OrderEndowment::admit(id(1), id(2), policy(), 0),
            Err(Error::UnderfundedOrder)
        );
        let state = OrderEndowment::admit(id(1), id(2), policy(), 14).unwrap();
        assert_eq!(state.accounted_lamports(), Ok(14));
        let state = state.succeed_clear(3).unwrap().succeed_settle(7).unwrap();
        assert_eq!(state.refundable_lamports(), Ok(4));
        assert_eq!(state.accounted_lamports(), Ok(14));
    }

    #[test]
    fn policy_rejects_free_mandatory_work_and_width_overflow() {
        let mut bad = policy();
        bad.per_order_clear_max_lamports = 0;
        assert_eq!(bad.validate(), Err(Error::ZeroMandatoryMaximum));
        let mut huge = policy();
        huge.per_order_clear_max_lamports = u64::MAX;
        assert_eq!(huge.validate(), Err(Error::ArithmeticOverflow));
    }

    #[test]
    fn shared_feed_join_and_success_refund_identities_are_exhaustive() {
        for cap in 1..=32u64 {
            let mut feed = SharedFeedReserve::<16>::empty(cap, id(250)).unwrap();
            let mut net_paid = [0u64; 16];
            for index in 0..16usize {
                let required = match feed.required_join_deposit() {
                    Ok(value) => value,
                    Err(Error::ZeroShare) => break,
                    other => panic!("unexpected quote: {other:?}"),
                };
                let (next, receipt) = feed.join(id((index + 1) as u8), required).unwrap();
                net_paid[index] = required;
                for reimbursement in receipt.reimbursements() {
                    let incumbent = usize::from(reimbursement.subscriber.bytes()[0]) - 1;
                    net_paid[incumbent] -= reimbursement.lamports;
                }
                feed = next;
                assert_eq!(feed.reserve_lamports(), cap);
                let share_sum: u64 = feed.shares().iter().map(|share| share.lamports).sum();
                assert_eq!(share_sum, cap);
                for (share, paid) in feed.shares().iter().zip(net_paid) {
                    assert_eq!(share.lamports, paid);
                }
                for keeper in 0..=cap {
                    let settlement = feed.settle(keeper, true).unwrap();
                    let cost_sum: u64 = settlement
                        .allocations()
                        .iter()
                        .map(|a| a.cost_lamports)
                        .sum();
                    let refund_sum: u64 = settlement
                        .allocations()
                        .iter()
                        .map(|a| a.refund_lamports)
                        .sum();
                    assert_eq!(cost_sum, keeper);
                    assert_eq!(refund_sum, cap - keeper);
                    for allocation in settlement.allocations() {
                        let participant = usize::from(allocation.subscriber.bytes()[0]) - 1;
                        assert_eq!(
                            allocation.cost_lamports + allocation.refund_lamports,
                            net_paid[participant]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn zero_share_duplicate_and_capacity_spam_are_refused() {
        let feed = SharedFeedReserve::<8>::empty(3, id(250)).unwrap();
        let (feed, _) = feed.join(id(1), 3).unwrap();
        assert_eq!(feed.join(id(1), 1), Err(Error::DuplicateSubscriber));
        let (feed, _) = feed.join(id(2), 1).unwrap();
        let (feed, _) = feed.join(id(3), 1).unwrap();
        assert_eq!(feed.required_join_deposit(), Err(Error::ZeroShare));
        assert_eq!(feed.join(id(4), 0), Err(Error::ZeroShare));

        let feed = SharedFeedReserve::<1>::empty(9, id(250)).unwrap();
        let (feed, _) = feed.join(id(1), 9).unwrap();
        assert_eq!(
            feed.required_join_deposit(),
            Err(Error::FeedCapacityReached)
        );
    }

    #[test]
    fn terminal_feed_failure_has_no_subscriber_refund_or_interested_recipient() {
        let feed = SharedFeedReserve::<4>::empty(19, id(250)).unwrap();
        let (feed, _) = feed.join(id(1), 19).unwrap();
        let (feed, _) = feed.join(id(2), 9).unwrap();
        let (feed, _) = feed.join(id(3), 6).unwrap();
        let settlement = feed.settle(7, false).unwrap();
        assert_eq!(settlement.keeper_paid_lamports, 7);
        assert_eq!(settlement.neutral_lamports, 12);
        assert_eq!(settlement.neutral_sink, id(250));
        assert!(settlement
            .allocations()
            .iter()
            .all(|a| a.refund_lamports == 0));
        let costs: u64 = settlement
            .allocations()
            .iter()
            .map(|a| a.cost_lamports)
            .sum();
        assert_eq!(costs, 19);
    }

    #[test]
    fn source_archive_pair_joins_atomically_at_independent_shares() {
        let pair = SharedFeedPair::<8>::empty(17, 11, id(250)).unwrap();
        assert_eq!(pair.required_join_deposits(), Ok((17, 11)));
        assert_eq!(pair.join(id(1), 17, 10), Err(Error::NoncanonicalDeposit));
        let (pair, _) = pair.join(id(1), 17, 11).unwrap();
        assert_eq!(pair.required_join_deposits(), Ok((8, 5)));
        let (pair, _) = pair.join(id(2), 8, 5).unwrap();
        assert_eq!(pair.source.reserve_lamports(), 17);
        assert_eq!(pair.archive.reserve_lamports(), 11);
    }

    #[test]
    fn combined_market_admission_has_no_future_subscriber_or_partial_feed_credit() {
        let feeds = SharedFeedPair::<8>::empty(17, 11, id(250)).unwrap();
        let quote = quote_market_admission(policy(), &feeds).unwrap();
        assert_eq!(quote.total_lamports, 31 + 17 + 11);
        let funding = MarketAdmissionFunding {
            market: MarketFunding {
                work_lamports: 11,
                storage_lamports: 7,
                resolution_lamports: 13,
            },
            source_join_lamports: 17,
            archive_join_lamports: 11,
        };
        let mut short = funding;
        short.archive_join_lamports -= 1;
        assert_eq!(
            admit_market(id(1), policy(), feeds, short),
            Err(Error::NoncanonicalDeposit)
        );
        let admitted = admit_market(id(1), policy(), feeds, funding).unwrap();
        assert_eq!(admitted.endowment.accounted_lamports(), Ok(31));
        assert_eq!(admitted.feeds.source.reserve_lamports(), 17);
        assert_eq!(admitted.feeds.archive.reserve_lamports(), 11);
    }

    #[test]
    fn a_neutral_sink_cannot_subscribe_or_own_the_market() {
        let feed = SharedFeedReserve::<4>::empty(9, id(250)).unwrap();
        assert_eq!(feed.join(id(250), 9), Err(Error::SameOwnerAndNeutralSink));
        assert_eq!(
            MarketEndowment::admit(
                id(250),
                policy(),
                MarketFunding {
                    work_lamports: 11,
                    storage_lamports: 7,
                    resolution_lamports: 13,
                }
            ),
            Err(Error::SameOwnerAndNeutralSink)
        );
    }

    #[test]
    fn persistent_fee_carry_is_owned_and_fragmentation_invariant() {
        for denominator in 1..=31u128 {
            for total in 0..=127u128 {
                let whole = IntentFeeCarry::admit(id(1), id(2), denominator).unwrap();
                let (whole, _) = whole.charge_fragment(id(1), id(2), total).unwrap();
                let (whole, terminal) = whole.close(id(1), id(2)).unwrap();
                assert_eq!(u128::from(whole.paid_atoms()), total.div_ceil(denominator));
                assert!(terminal.terminal);
                for first in 0..=total {
                    let carry = IntentFeeCarry::admit(id(1), id(2), denominator).unwrap();
                    let (carry, _) = carry.charge_fragment(id(1), id(2), first).unwrap();
                    let (carry, _) = carry.charge_fragment(id(1), id(2), total - first).unwrap();
                    let (carry, _) = carry.close(id(1), id(2)).unwrap();
                    assert_eq!(carry.paid_atoms(), whole.paid_atoms());
                }
            }
        }
    }

    #[test]
    fn fee_carry_cannot_be_stolen_reset_or_reopened() {
        let carry = IntentFeeCarry::admit(id(1), id(2), 10).unwrap();
        assert_eq!(
            carry.charge_fragment(id(3), id(2), 7),
            Err(Error::WrongFeeOwner)
        );
        assert_eq!(
            carry.charge_fragment(id(1), id(3), 7),
            Err(Error::WrongIntent)
        );
        let (carry, _) = carry.charge_fragment(id(1), id(2), 7).unwrap();
        let (closed, charge) = carry.close(id(1), id(2)).unwrap();
        assert_eq!(charge.atoms, 1);
        assert_eq!(
            closed.charge_fragment(id(1), id(2), 1),
            Err(Error::AlreadyTerminal)
        );
        assert_eq!(closed.close(id(1), id(2)), Err(Error::AlreadyTerminal));
    }

    /// A zero denominator is a refusal, not a divide fault (previously
    /// unexercised).
    #[test]
    fn zero_fee_denominator_refuses_admission() {
        assert_eq!(
            IntentFeeCarry::admit(id(1), id(2), 0),
            Err(Error::ZeroFeeDenominator)
        );
        assert_eq!(
            EnvelopedIntentFeeCarry::admit(id(1), id(2), 0, 0, 0),
            Err(Error::ZeroFeeDenominator)
        );
    }

    /// REVENUE_POLICY_V1 §10.4 at zero rates: **carry of zero is zero,
    /// exactly, through every path.**  For every denominator and every
    /// fragmentation (one to five zero fragments), the enveloped carry under
    /// the signed ZERO envelope admits, accumulates nothing, closes with a
    /// terminal charge of exactly zero atoms, and refuses reopen — so the
    /// terminal ceil never mints an atom from a zero-rate intent, and the
    /// zero envelope's meaning ("no fee, ever") is bit-exact.
    #[test]
    fn zero_rate_carry_is_exactly_zero_through_every_path() {
        for denominator in 1..=17u128 {
            for fragments in 1..=5usize {
                let mut carry =
                    EnvelopedIntentFeeCarry::admit(id(1), id(2), denominator, 0, 0).unwrap();
                for _ in 0..fragments {
                    let (next, charge) = carry.charge_fragment(id(1), id(2), 0).unwrap();
                    assert_eq!(charge.atoms, 0);
                    assert_eq!(charge.remainder, 0);
                    assert!(!charge.terminal);
                    carry = next;
                }
                assert_eq!(carry.committed_atoms(), Ok(0));
                let (closed, terminal) = carry.close(id(1), id(2)).unwrap();
                assert_eq!(terminal.atoms, 0, "the ceil atom must not exist at zero");
                assert_eq!(terminal.remainder, 0);
                assert!(terminal.terminal);
                assert_eq!(closed.carry().paid_atoms(), 0);
                assert_eq!(closed.carry().remainder(), 0);
                assert!(closed.carry().is_closed());
                assert_eq!(
                    closed.charge_fragment(id(1), id(2), 0),
                    Err(Error::AlreadyTerminal),
                    "reopen refused even for a zero charge"
                );
                assert_eq!(closed.close(id(1), id(2)), Err(Error::AlreadyTerminal));
            }
        }
    }

    /// §10.2's host half: a zero envelope provably never pays an atom or
    /// accrues a remainder — the FIRST nonzero numerator refuses (its ceil
    /// commitment already exceeds zero) and the refused copy is unchanged —
    /// and a nonzero envelope is a hard bound with the terminal ceil atom
    /// counted, so a charge that would maroon the close outside the envelope
    /// refuses at the fragment, never at the close.
    #[test]
    fn the_envelope_bounds_every_charge_including_the_terminal_ceil() {
        // Admission quotes above the signed envelope refuse outright.
        assert_eq!(
            EnvelopedIntentFeeCarry::admit(id(1), id(2), 10, 1, 0),
            Err(Error::FeeEnvelopeExceeded)
        );

        // The zero envelope: sub-atom numerators refuse too, because the
        // remainder they would leave commits a ceil atom the envelope
        // cannot pay.
        let zero = EnvelopedIntentFeeCarry::admit(id(1), id(2), 10, 0, 0).unwrap();
        for numerator in [1u128, 5, 9, 10, 11, 1_000] {
            assert_eq!(
                zero.charge_fragment(id(1), id(2), numerator),
                Err(Error::FeeEnvelopeExceeded),
                "a zero envelope must never be touched by numerator {numerator}"
            );
        }
        // The refused value is untouched and still closes at exactly zero.
        let (closed, terminal) = zero.close(id(1), id(2)).unwrap();
        assert_eq!(terminal.atoms, 0);
        assert_eq!(closed.carry().paid_atoms(), 0);

        // A one-atom envelope admits a sub-atom remainder (ceil commitment
        // exactly 1) and the close pays exactly that committed atom.
        let one = EnvelopedIntentFeeCarry::admit(id(1), id(2), 10, 1, 1).unwrap();
        let (one, charge) = one.charge_fragment(id(1), id(2), 5).unwrap();
        assert_eq!(charge.atoms, 0);
        assert_eq!(charge.remainder, 5);
        assert_eq!(one.committed_atoms(), Ok(1));
        // A second fragment that would commit a second atom refuses here,
        // not at the close.
        assert_eq!(
            one.charge_fragment(id(1), id(2), 10),
            Err(Error::FeeEnvelopeExceeded)
        );
        let (closed, terminal) = one.close(id(1), id(2)).unwrap();
        assert_eq!(terminal.atoms, 1);
        assert_eq!(closed.carry().paid_atoms(), 1);
        assert_eq!(closed.max_fee_atoms(), 1);
    }

    /// The selected composite relation and this persistent carry must operate
    /// on one identical rational.  The high-scale row deliberately has a
    /// denominator above `u64::MAX`; it is the regression that the former
    /// narrow carry could not represent even though the relation admitted it.
    #[test]
    fn composite_quote_and_persistent_carry_agree_at_full_width() {
        use clutch_batch::relation_v1::{composite_fee_quote, MAX_OUTCOMES, TEST_COMPOSITE_LAB};

        let clutch_batch::relation_v1::FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } = TEST_COMPOSITE_LAB
        else {
            unreachable!()
        };

        for scale in [10_000u64, 1_000_000, 1_000_000_000] {
            let mut payoffs = [0u64; MAX_OUTCOMES];
            payoffs[1] = 100;
            let mut prices = [0u64; MAX_OUTCOMES];
            prices[0] = scale / 2;
            prices[1] = scale - prices[0];
            let quote = composite_fee_quote(
                &payoffs,
                &prices,
                2,
                scale,
                dispersion_bps,
                floor_range_bps,
                0,
            )
            .unwrap();
            if scale == 1_000_000_000 {
                assert!(quote.base_denominator > u128::from(u64::MAX));
            }
            let ceiling = u64::try_from(quote.terminal_ceil_atoms).unwrap();

            let carry = EnvelopedIntentFeeCarry::admit(
                id(1),
                id(2),
                quote.base_denominator,
                ceiling,
                ceiling,
            )
            .unwrap();
            let (carry, fragment) = carry
                .charge_fragment(id(1), id(2), quote.base_numerator)
                .unwrap();
            assert_eq!(u128::from(fragment.atoms), quote.floor_atoms);
            assert_eq!(fragment.remainder, quote.carry);
            assert_eq!(carry.carry().remainder(), quote.carry);
            assert_eq!(carry.committed_atoms(), Ok(ceiling));
            let (closed, terminal) = carry.close(id(1), id(2)).unwrap();
            assert_eq!(
                u128::from(closed.carry().paid_atoms()),
                quote.terminal_ceil_atoms
            );
            assert_eq!(terminal.atoms, u64::from(quote.carry != 0));

            // Three fragments of the same exact numerator produce the same
            // paid total and terminal carry; no fragment gets its own round.
            let first = quote.base_numerator / 3;
            let second = quote.base_numerator / 3;
            let third = quote.base_numerator - first - second;
            let mut fragmented = EnvelopedIntentFeeCarry::admit(
                id(1),
                id(2),
                quote.base_denominator,
                ceiling,
                ceiling,
            )
            .unwrap();
            for numerator in [first, second, third] {
                fragmented = fragmented
                    .charge_fragment(id(1), id(2), numerator)
                    .unwrap()
                    .0;
            }
            let (fragmented, _) = fragmented.close(id(1), id(2)).unwrap();
            assert_eq!(fragmented.carry().paid_atoms(), closed.carry().paid_atoms());
        }
    }

    /// Hostile widths fail closed: adding a fragment cannot wrap the exact
    /// numerator, a quotient that cannot become a collateral `u64` atom count
    /// refuses, and the monotone paid counter cannot roll over.
    #[test]
    fn full_width_fee_carry_refuses_every_overflow_boundary() {
        let carry = IntentFeeCarry::admit(id(1), id(2), u128::MAX).unwrap();
        let (near, _) = carry.charge_fragment(id(1), id(2), u128::MAX - 1).unwrap();
        assert_eq!(
            near.charge_fragment(id(1), id(2), 2),
            Err(Error::ArithmeticOverflow)
        );

        let unit = IntentFeeCarry::admit(id(1), id(2), 1).unwrap();
        assert_eq!(
            unit.charge_fragment(id(1), id(2), u128::from(u64::MAX) + 1),
            Err(Error::ArithmeticOverflow)
        );
        let (full, _) = unit
            .charge_fragment(id(1), id(2), u128::from(u64::MAX))
            .unwrap();
        assert_eq!(
            full.charge_fragment(id(1), id(2), 1),
            Err(Error::ArithmeticOverflow)
        );
    }

    /// The B4b grief rider's arithmetic: close refuses while any served
    /// fee-bearing epoch is unsettled, settles never go negative, and close
    /// lands exactly once.
    #[test]
    fn treasury_service_ledger_refuses_mid_epoch_close() {
        assert_eq!(
            TreasuryServiceLedger::admit(Id::ZERO),
            Err(Error::ZeroIdentity)
        );
        let ledger = TreasuryServiceLedger::admit(id(9)).unwrap();
        assert_eq!(ledger.outstanding(), 0);
        // Settling a service nobody began refuses: no banked free closes.
        assert_eq!(ledger.settle_service(), Err(Error::NoServingEpoch));

        // Two epochs admitted against the treasury; close refuses until the
        // exact number settled.
        let ledger = ledger.begin_service().unwrap().begin_service().unwrap();
        assert_eq!(ledger.outstanding(), 2);
        assert_eq!(ledger.close(), Err(Error::TreasuryServiceOutstanding));
        let ledger = ledger.settle_service().unwrap();
        assert_eq!(ledger.close(), Err(Error::TreasuryServiceOutstanding));
        let ledger = ledger.settle_service().unwrap();
        let closed = ledger.close().unwrap();
        assert!(closed.is_closed());

        // Close happens once, ever; a closed ledger serves nothing.
        assert_eq!(closed.close(), Err(Error::AlreadyTerminal));
        assert_eq!(closed.begin_service(), Err(Error::AlreadyTerminal));
        assert_eq!(closed.settle_service(), Err(Error::AlreadyTerminal));
    }
}
