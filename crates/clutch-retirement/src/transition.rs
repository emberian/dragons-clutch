// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    canonical_epoch_generation, AuthenticatedEpochBudgetDispositionV1, CandidateStatusWitnessV1,
    DeletableRentOwnerV1, EpochChildKindV1, EpochRetirementTailV1, GeneralEpochPhaseV2,
    GeneralEpochTombstoneV1, Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1,
    PositionTerminalProjectionV3, PositionTombstoneV1, PositionTombstoneV2, PositionTombstoneV3,
    RentSplitV2, ReplayV3HashBackend, ReplayV3TerminalProjection, ReservationCountTailV1,
    ReservationStateV1, RetirementErrorV1, RetirementErrorV2, MAX_OUTCOMES,
};
use clutch_candidate_lifecycle::CandidateWindowV4;

/// Maximum number of distinct recipients in one modeled retirement bundle.
pub const MAX_RETIREMENT_RECIPIENTS: usize = 4;
/// Maximum distinct recipients when an Epoch root also pays its closer.
pub const MAX_EPOCH_ROOT_RECIPIENTS_V2: usize = 5;

/// One forgeable adapter-projected recipient balance supplied before planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientBalanceV1 {
    /// Recipient account identity.
    pub recipient: Identity32V1,
    /// Claimed balance before any bundle mutation.
    pub balance_before: u64,
}

/// Fixed-capacity unique recipient balance book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientBalanceBookV1 {
    /// Up to four unique recipients. Unused slots are `None`.
    pub entries: [Option<RecipientBalanceV1>; MAX_RETIREMENT_RECIPIENTS],
}

impl RecipientBalanceBookV1 {
    /// Validate that present recipient identities are unique.
    pub fn validate(self) -> Result<(), RetirementErrorV2> {
        let mut left = 0usize;
        while left < self.entries.len() {
            if let Some(entry) = self.entries[left] {
                let mut right = left + 1;
                while right < self.entries.len() {
                    if self.entries[right].map(|other| other.recipient) == Some(entry.recipient) {
                        return Err(RetirementErrorV2::AccountAlias);
                    }
                    right += 1;
                }
            }
            left += 1;
        }
        Ok(())
    }

    fn locate(self, recipient: Identity32V1) -> Result<usize, RetirementErrorV2> {
        self.validate()?;
        let mut index = 0usize;
        while index < self.entries.len() {
            if self.entries[index].map(|entry| entry.recipient) == Some(recipient) {
                return Ok(index);
            }
            index += 1;
        }
        Err(RetirementErrorV2::MissingRecipient)
    }
}

/// One coalesced recipient credit and its checked final balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientCreditV1 {
    /// Recipient account identity.
    pub recipient: Identity32V1,
    /// Sum of every funding compartment credited to this recipient.
    pub credit_lamports: u64,
    /// Claimed starting balance plus the coalesced credit.
    pub balance_after: u64,
}

/// Complete fixed-capacity credit plan for a retirement bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoalescedRecipientCreditsV1 {
    /// Present entries preserve the caller's validated balance-book order.
    pub entries: [Option<RecipientCreditV1>; MAX_RETIREMENT_RECIPIENTS],
}

impl CoalescedRecipientCreditsV1 {
    fn begin(book: RecipientBalanceBookV1) -> Result<Self, RetirementErrorV2> {
        book.validate()?;
        let mut entries = [None; MAX_RETIREMENT_RECIPIENTS];
        let mut index = 0usize;
        while index < entries.len() {
            entries[index] = book.entries[index].map(|entry| RecipientCreditV1 {
                recipient: entry.recipient,
                credit_lamports: 0,
                balance_after: entry.balance_before,
            });
            index += 1;
        }
        Ok(Self { entries })
    }

    fn credit(
        &mut self,
        book: RecipientBalanceBookV1,
        recipient: Identity32V1,
        amount: u64,
    ) -> Result<(), RetirementErrorV2> {
        let index = book.locate(recipient)?;
        let mut entry = self.entries[index].ok_or(RetirementErrorV2::MissingRecipient)?;
        entry.credit_lamports = entry
            .credit_lamports
            .checked_add(amount)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        let balance_before = book.entries[index]
            .ok_or(RetirementErrorV2::MissingRecipient)?
            .balance_before;
        entry.balance_after = balance_before
            .checked_add(entry.credit_lamports)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        self.entries[index] = Some(entry);
        Ok(())
    }

    /// Look up one recipient's coalesced checked plan.
    pub fn get(self, recipient: Identity32V1) -> Option<RecipientCreditV1> {
        let mut index = 0usize;
        while index < self.entries.len() {
            if self.entries[index].map(|entry| entry.recipient) == Some(recipient) {
                return self.entries[index];
            }
            index += 1;
        }
        None
    }
}

/// Five-recipient balance book for Epoch+Window+Budget close with a distinct closer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRecipientBalanceBookV2 {
    /// Epoch payer, Window payer, Budget payer, closer, and neutral sink may all differ.
    pub entries: [Option<RecipientBalanceV1>; MAX_EPOCH_ROOT_RECIPIENTS_V2],
}

impl EpochRootRecipientBalanceBookV2 {
    fn locate(self, recipient: Identity32V1) -> Result<usize, RetirementErrorV2> {
        let mut left = 0usize;
        while left < self.entries.len() {
            if let Some(entry) = self.entries[left] {
                let mut right = left + 1;
                while right < self.entries.len() {
                    if self.entries[right].map(|other| other.recipient) == Some(entry.recipient) {
                        return Err(RetirementErrorV2::AccountAlias);
                    }
                    right += 1;
                }
                if entry.recipient == recipient {
                    return Ok(left);
                }
            }
            left += 1;
        }
        Err(RetirementErrorV2::MissingRecipient)
    }
}

/// Coalesced five-recipient credits for an atomic Epoch-root retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRecipientCreditsV2 {
    /// Present entries preserve the authenticated balance-book order.
    pub entries: [Option<RecipientCreditV1>; MAX_EPOCH_ROOT_RECIPIENTS_V2],
}

impl EpochRootRecipientCreditsV2 {
    fn begin(book: EpochRootRecipientBalanceBookV2) -> Result<Self, RetirementErrorV2> {
        let mut entries = [None; MAX_EPOCH_ROOT_RECIPIENTS_V2];
        let mut index = 0usize;
        while index < entries.len() {
            if let Some(entry) = book.entries[index] {
                book.locate(entry.recipient)?;
                entries[index] = Some(RecipientCreditV1 {
                    recipient: entry.recipient,
                    credit_lamports: 0,
                    balance_after: entry.balance_before,
                });
            }
            index += 1;
        }
        Ok(Self { entries })
    }

    fn credit(
        &mut self,
        book: EpochRootRecipientBalanceBookV2,
        recipient: Identity32V1,
        amount: u64,
    ) -> Result<(), RetirementErrorV2> {
        let index = book.locate(recipient)?;
        let balance_before = book.entries[index]
            .ok_or(RetirementErrorV2::MissingRecipient)?
            .balance_before;
        let mut entry = self.entries[index].ok_or(RetirementErrorV2::MissingRecipient)?;
        entry.credit_lamports = entry
            .credit_lamports
            .checked_add(amount)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        entry.balance_after = balance_before
            .checked_add(entry.credit_lamports)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        self.entries[index] = Some(entry);
        Ok(())
    }

    /// Look up one recipient's coalesced checked plan.
    pub fn get(self, recipient: Identity32V1) -> Option<RecipientCreditV1> {
        let mut index = 0usize;
        while index < self.entries.len() {
            if self.entries[index].map(|entry| entry.recipient) == Some(recipient) {
                return self.entries[index];
            }
            index += 1;
        }
        None
    }
}

/// Complete hostile-prefund admission plan for a deleted-at-close account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableRentAdmissionPlanV1 {
    target: Identity32V1,
    /// Persisted payer/principal/donation owner.
    rent: DeletableRentOwnerV1,
    /// Payer balance after paying the full principal.
    payer_balance_after: u64,
    /// Target balance after adding full principal atop hostile prefund.
    account_balance_after: u64,
    neutral_sink: Identity32V1,
}

impl DeletableRentAdmissionPlanV1 {
    /// Exact target account whose hostile prefund balance was admitted.
    pub const fn target(self) -> Identity32V1 {
        self.target
    }

    /// Persisted payer/principal/donation owner produced by exact admission.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Payer balance after paying full principal.
    pub const fn payer_balance_after(self) -> u64 {
        self.payer_balance_after
    }

    /// Target balance after full principal is added atop hostile prefund.
    pub const fn account_balance_after(self) -> u64 {
        self.account_balance_after
    }

    /// Frozen neutral sink checked against the payer at admission.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    const fn payer_debit_lamports(self) -> u64 {
        self.rent.refundable_principal()
    }
}

/// Admit exact funding without allowing an existing PDA prefund to discount
/// the payer's full principal obligation.
pub fn admit_deletable_rent(
    target: Identity32V1,
    payer: Identity32V1,
    refundable_principal: u64,
    hostile_prefund_balance: u64,
    payer_balance_before: u64,
    neutral_sink: Identity32V1,
) -> Result<DeletableRentAdmissionPlanV1, RetirementErrorV2> {
    if payer == neutral_sink {
        return Err(RetirementErrorV2::PayerIsNeutralSink);
    }
    let rent =
        DeletableRentOwnerV1::from_persisted(payer, refundable_principal, hostile_prefund_balance)?;
    let payer_balance_after = payer_balance_before
        .checked_sub(refundable_principal)
        .ok_or(RetirementErrorV2::PayerBalanceShortfall)?;
    let account_balance_after = hostile_prefund_balance
        .checked_add(refundable_principal)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    Ok(DeletableRentAdmissionPlanV1 {
        target,
        rent,
        payer_balance_after,
        account_balance_after,
        neutral_sink,
    })
}

/// Exact hostile-prefund admission plan for a live account retaining a
/// permanent tombstone compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentSplitAdmissionPlanV2 {
    target: Identity32V1,
    rent: RentSplitV2,
    payer_debit_lamports: u64,
    payer_balance_after: u64,
    account_balance_after: u64,
    neutral_sink: Identity32V1,
}

impl RentSplitAdmissionPlanV2 {
    /// Exact target account whose existing balance was admitted.
    pub const fn target(self) -> Identity32V1 {
        self.target
    }

    /// Persisted live-refund/tombstone/donation split produced by admission.
    pub const fn rent(self) -> RentSplitV2 {
        self.rent
    }

    /// Payer balance after its exact required transfer.
    pub const fn payer_balance_after(self) -> u64 {
        self.payer_balance_after
    }

    /// Live account balance after its exact required transfer.
    pub const fn account_balance_after(self) -> u64 {
        self.account_balance_after
    }

    /// Frozen neutral sink checked against the payer at admission.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    const fn payer_debit_lamports(self) -> u64 {
        self.payer_debit_lamports
    }
}

/// Admit a founding live account by charging full live plus tombstone
/// principal even when its canonical PDA was hostilely prefunded.
pub fn admit_initial_rent_split(
    target: Identity32V1,
    payer: Identity32V1,
    refundable_live_principal: u64,
    permanent_tombstone_principal: u64,
    hostile_prefund_balance: u64,
    payer_balance_before: u64,
    neutral_sink: Identity32V1,
) -> Result<RentSplitAdmissionPlanV2, RetirementErrorV2> {
    if payer == neutral_sink {
        return Err(RetirementErrorV2::PayerIsNeutralSink);
    }
    let rent = RentSplitV2 {
        payer,
        refundable_live_principal,
        permanent_tombstone_principal,
        donation_floor: hostile_prefund_balance,
    };
    rent.validate()?;
    let principal = refundable_live_principal
        .checked_add(permanent_tombstone_principal)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let payer_balance_after = payer_balance_before
        .checked_sub(principal)
        .ok_or(RetirementErrorV2::PayerBalanceShortfall)?;
    let account_balance_after = hostile_prefund_balance
        .checked_add(principal)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    Ok(RentSplitAdmissionPlanV2 {
        target,
        rent,
        payer_debit_lamports: principal,
        payer_balance_after,
        account_balance_after,
        neutral_sink,
    })
}

/// Admit Position reopen atop its already retained tombstone principal.
pub fn admit_reopen_rent_split(
    target: Identity32V1,
    payer: Identity32V1,
    refundable_live_principal: u64,
    permanent_tombstone_principal: u64,
    tombstone_balance_before: u64,
    payer_balance_before: u64,
    neutral_sink: Identity32V1,
) -> Result<RentSplitAdmissionPlanV2, RetirementErrorV2> {
    if payer == neutral_sink {
        return Err(RetirementErrorV2::PayerIsNeutralSink);
    }
    let donation_floor = tombstone_balance_before
        .checked_sub(permanent_tombstone_principal)
        .ok_or(RetirementErrorV2::AccountBalanceShortfall)?;
    let rent = RentSplitV2 {
        payer,
        refundable_live_principal,
        permanent_tombstone_principal,
        donation_floor,
    };
    rent.validate()?;
    let payer_balance_after = payer_balance_before
        .checked_sub(refundable_live_principal)
        .ok_or(RetirementErrorV2::PayerBalanceShortfall)?;
    let account_balance_after = tombstone_balance_before
        .checked_add(refundable_live_principal)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    Ok(RentSplitAdmissionPlanV2 {
        target,
        rent,
        payer_debit_lamports: refundable_live_principal,
        payer_balance_after,
        account_balance_after,
        neutral_sink,
    })
}

/// One payer's coalesced debit across an atomic funding bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerDebitV1 {
    /// Adapter-projected payer identity.
    pub payer: Identity32V1,
    /// Sum of every full-principal debit assigned to this payer.
    pub debit_lamports: u64,
    /// Claimed starting balance less the complete coalesced debit.
    pub balance_after: u64,
}

/// Fixed-capacity coalesced debit plan for an atomic funding bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoalescedPayerDebitsV1 {
    /// Up to four distinct payers; unused slots are `None`.
    pub entries: [Option<PayerDebitV1>; MAX_RETIREMENT_RECIPIENTS],
}

impl CoalescedPayerDebitsV1 {
    const fn empty() -> Self {
        Self {
            entries: [None; MAX_RETIREMENT_RECIPIENTS],
        }
    }

    fn debit(
        &mut self,
        payer: Identity32V1,
        debit_lamports: u64,
        individual_balance_after: u64,
    ) -> Result<(), RetirementErrorV2> {
        let balance_before = individual_balance_after
            .checked_add(debit_lamports)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        let mut index = 0usize;
        while index < self.entries.len() {
            match self.entries[index] {
                Some(mut entry) if entry.payer == payer => {
                    let prior_balance_before = entry
                        .balance_after
                        .checked_add(entry.debit_lamports)
                        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
                    if prior_balance_before != balance_before {
                        return Err(RetirementErrorV2::InconsistentPayerBalance);
                    }
                    entry.debit_lamports = entry
                        .debit_lamports
                        .checked_add(debit_lamports)
                        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
                    entry.balance_after = balance_before
                        .checked_sub(entry.debit_lamports)
                        .ok_or(RetirementErrorV2::PayerBalanceShortfall)?;
                    self.entries[index] = Some(entry);
                    return Ok(());
                }
                None => {
                    self.entries[index] = Some(PayerDebitV1 {
                        payer,
                        debit_lamports,
                        balance_after: individual_balance_after,
                    });
                    return Ok(());
                }
                Some(_) => {}
            }
            index += 1;
        }
        Err(RetirementErrorV2::ArithmeticOverflow)
    }

    /// Look up one payer's complete coalesced debit plan.
    pub fn get(self, payer: Identity32V1) -> Option<PayerDebitV1> {
        let mut index = 0usize;
        while index < self.entries.len() {
            if self.entries[index].map(|entry| entry.payer) == Some(payer) {
                return self.entries[index];
            }
            index += 1;
        }
        None
    }
}

/// Exact close disposition for an account that leaves no tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableRentDispositionV1 {
    /// Stored payer receiving exact principal.
    pub payer: Identity32V1,
    /// Exact principal returned to the payer.
    pub payer_refund_lamports: u64,
    /// Frozen neutral sink receiving prefund plus later unsolicited balance.
    pub neutral_sink: Identity32V1,
    /// Exact amount routed to the neutral sink.
    pub neutral_lamports: u64,
}

fn deletable_rent_disposition(
    rent: DeletableRentOwnerV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<DeletableRentDispositionV1, RetirementErrorV2> {
    rent.validate()?;
    if rent.payer() == neutral_sink {
        return Err(RetirementErrorV2::PayerIsNeutralSink);
    }
    let floor = rent
        .refundable_principal()
        .checked_add(rent.donation_floor())
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if actual_balance < floor {
        return Err(RetirementErrorV2::AccountBalanceShortfall);
    }
    Ok(DeletableRentDispositionV1 {
        payer: rent.payer(),
        payer_refund_lamports: rent.refundable_principal(),
        neutral_sink,
        neutral_lamports: actual_balance
            .checked_sub(rent.refundable_principal())
            .ok_or(RetirementErrorV2::AccountBalanceShortfall)?,
    })
}

/// Exact terminal distribution derived from a persisted rent split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentDispositionV2 {
    /// Stored payer receiving refundable live principal.
    pub payer: Identity32V1,
    /// Exact live principal returned to `payer`.
    pub payer_refund_lamports: u64,
    /// Principal left in the permanent tombstone.
    pub tombstone_lamports: u64,
    /// Frozen neutral sink receiving every unsolicited lamport.
    pub neutral_sink: Identity32V1,
    /// Exact surplus transferred to `neutral_sink`.
    pub neutral_lamports: u64,
}

/// Complete precomputed output of one root account's shrink and lamport split.
///
/// The adapter performs no write, realloc, or transfer for this root until this
/// value exists. Other accounts in a required bundle are deliberately outside
/// this type and require their own precomputed plans before the transaction's
/// first mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementCommitPlanV2<State> {
    /// Complete post-transition live-or-tombstone state.
    pub post_state: State,
    /// Exact stored payer receiving the refundable principal.
    pub payer: Identity32V1,
    /// Payer balance after its checked credit.
    pub payer_balance_after: u64,
    /// Exact frozen neutral sink receiving all surplus.
    pub neutral_sink: Identity32V1,
    /// Neutral-sink balance after its checked credit.
    pub neutral_balance_after: u64,
    /// Exact lamports retained by the resized tombstone account.
    pub tombstone_balance_after: u64,
}

fn commit_plan<State>(
    post_state: State,
    disposition: RentDispositionV2,
    payer_balance_before: u64,
    neutral_balance_before: u64,
) -> Result<RetirementCommitPlanV2<State>, RetirementErrorV1> {
    let payer_balance_after = payer_balance_before
        .checked_add(disposition.payer_refund_lamports)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let neutral_balance_after = neutral_balance_before
        .checked_add(disposition.neutral_lamports)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    Ok(RetirementCommitPlanV2 {
        post_state,
        payer: disposition.payer,
        payer_balance_after,
        neutral_sink: disposition.neutral_sink,
        neutral_balance_after,
        tombstone_balance_after: disposition.tombstone_lamports,
    })
}

fn rent_disposition(
    rent: RentSplitV2,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<RentDispositionV2, RetirementErrorV1> {
    rent.validate()?;
    if rent.payer == neutral_sink {
        return Err(RetirementErrorV1::PayerIsNeutralSink);
    }
    let principal = rent
        .refundable_live_principal
        .checked_add(rent.permanent_tombstone_principal)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let floor = principal
        .checked_add(rent.donation_floor)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    if actual_balance < floor {
        return Err(RetirementErrorV1::AccountBalanceShortfall);
    }
    let neutral_lamports = actual_balance
        .checked_sub(principal)
        .ok_or(RetirementErrorV1::AccountBalanceShortfall)?;
    Ok(RentDispositionV2 {
        payer: rent.payer,
        payer_refund_lamports: rent.refundable_live_principal,
        tombstone_lamports: rent.permanent_tombstone_principal,
        neutral_sink,
        neutral_lamports,
    })
}

/// Local economic compartments that must be zero before Position retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionEconomicStateV1 {
    /// Venue-held trading cash.
    pub cash_atoms: u64,
    /// Encumbered trading cash.
    pub reserved_cash_atoms: u64,
    /// Internal Egg balances at the fixed maximum width.
    pub internal_atoms: [u64; MAX_OUTCOMES],
}

impl PositionEconomicStateV1 {
    /// Canonical zero economic state.
    pub const ZERO: Self = Self {
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        internal_atoms: [0; MAX_OUTCOMES],
    };

    /// Whether all local economic compartments are exactly zero.
    pub fn is_zero(self) -> bool {
        self.cash_atoms == 0
            && self.reserved_cash_atoms == 0
            && self.internal_atoms.iter().all(|amount| *amount == 0)
    }
}

/// Forgeable adapter projection of a live Position V2 for pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePositionV2 {
    /// Claimed Market identity from the base Position body.
    pub market: Identity32V1,
    /// Claimed owner identity from the base Position body.
    pub owner: Identity32V1,
    /// Current Position generation; zero is the canonical founding generation.
    pub generation: u64,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Count and funding state owned by this crate.
    pub retirement: PositionRetirementTailV1,
}

impl LivePositionV2 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        self.retirement.validate()
    }
}

/// Live-or-tombstone Position state; absence is deliberately unrepresentable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionLifecycleStateV2 {
    /// Full live Position V2.
    Live(LivePositionV2),
    /// Compact permanent Position tombstone.
    Tombstone(PositionTombstoneV1),
}

/// Close a locally and aggregately empty Position into its permanent tombstone.
///
/// The function constructs the complete post-state and disposition before
/// returning. Passing the returned tombstone again refuses as a replay.
fn close_position_root(
    state: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(PositionLifecycleStateV2, RentDispositionV2), RetirementErrorV1> {
    let live = match state {
        PositionLifecycleStateV2::Live(live) => live,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV1::AlreadyTerminal),
    };
    live.validate()?;
    if !economic.is_zero() {
        return Err(RetirementErrorV1::EconomicBalanceOutstanding);
    }
    if live.retirement.outstanding_reservations != 0 {
        return Err(RetirementErrorV1::ReservationOutstanding);
    }
    let disposition = rent_disposition(live.retirement.rent, actual_balance, neutral_sink)?;
    let tombstone = PositionTombstoneV1 {
        market: live.market,
        owner: live.owner,
        generation: live.generation,
        stored_bump: live.stored_bump,
    };
    tombstone.validate()?;
    Ok((PositionLifecycleStateV2::Tombstone(tombstone), disposition))
}

/// Precompute the Position root's tombstone, payer credit, sink credit, and
/// retained balance before an adapter mutates any account.
fn plan_position_root_retirement(
    state: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
    payer_balance_before: u64,
    neutral_balance_before: u64,
) -> Result<RetirementCommitPlanV2<PositionLifecycleStateV2>, RetirementErrorV1> {
    let (post_state, disposition) =
        close_position_root(state, economic, actual_balance, neutral_sink)?;
    commit_plan(
        post_state,
        disposition,
        payer_balance_before,
        neutral_balance_before,
    )
}

/// Reopen the next Position generation at the same permanent identity.
///
/// The adapter independently proves the retained tombstone minimum and exact
/// payer transfer before supplying `new_rent`. A live Position cannot reopen.
fn reopen_position_root(
    state: PositionLifecycleStateV2,
    new_rent: RentSplitV2,
    neutral_sink: Identity32V1,
) -> Result<PositionLifecycleStateV2, RetirementErrorV1> {
    let tombstone = match state {
        PositionLifecycleStateV2::Tombstone(tombstone) => tombstone,
        PositionLifecycleStateV2::Live(_) => return Err(RetirementErrorV1::WrongPhase),
    };
    tombstone.validate()?;
    new_rent.validate()?;
    if new_rent.payer == neutral_sink {
        return Err(RetirementErrorV1::PayerIsNeutralSink);
    }
    let generation = tombstone
        .generation
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    Ok(PositionLifecycleStateV2::Live(LivePositionV2 {
        market: tombstone.market,
        owner: tombstone.owner,
        generation,
        stored_bump: tombstone.stored_bump,
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: new_rent,
        },
    }))
}

/// Close a locally and aggregately empty Position into its permanent
/// tombstone using the committed root-only V2 API.
///
/// This preserves the frozen pure transition exactly. Live runtime routing
/// remains disabled; successor activation requires the atomic Replay sibling.
pub fn close_position(
    state: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(PositionLifecycleStateV2, RentDispositionV2), RetirementErrorV1> {
    close_position_root(state, economic, actual_balance, neutral_sink)
}

/// Precompute the committed Position root-only retirement plan.
///
/// This preserves the frozen pure API; it is not live authorization and does
/// not satisfy the successor requirement to delete Replay atomically.
pub fn plan_position_retirement(
    state: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
    payer_balance_before: u64,
    neutral_balance_before: u64,
) -> Result<RetirementCommitPlanV2<PositionLifecycleStateV2>, RetirementErrorV1> {
    plan_position_root_retirement(
        state,
        economic,
        actual_balance,
        neutral_sink,
        payer_balance_before,
        neutral_balance_before,
    )
}

/// Reopen the next Position generation through the committed root-only API.
///
/// This preserves the frozen pure transition exactly. Live runtime routing
/// remains disabled; successor activation requires the atomic Replay sibling.
pub fn reopen_position(
    state: PositionLifecycleStateV2,
    new_rent: RentSplitV2,
    neutral_sink: Identity32V1,
) -> Result<PositionLifecycleStateV2, RetirementErrorV1> {
    reopen_position_root(state, new_rent, neutral_sink)
}

/// Live generation-scoped Replay successor funded independently from Position.
///
/// The exact legacy Replay body remains owned by `clutch-solana-reference`.
/// This projection adds only the retirement funding fact required to close the
/// sibling atomically with its Position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveReplaySuccessorV1 {
    /// Claimed Market identity from the Replay body.
    pub market: Identity32V1,
    /// Claimed owner identity from the Replay body.
    pub owner: Identity32V1,
    /// Exact Position generation named by this Replay sibling.
    pub position_generation: u64,
    /// Exact next request sequence.
    pub sequence: u64,
    /// Canonical stored Replay PDA bump.
    pub stored_bump: u8,
    /// Separately paid closeable Replay principal and donation floor.
    pub rent: DeletableRentOwnerV1,
}

impl LiveReplaySuccessorV1 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        self.rent.validate()
    }
}

/// Present-or-absent state of one generation-scoped Replay sibling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayLifecycleStateV1 {
    /// Live Replay for exactly one Position generation.
    Live(LiveReplaySuccessorV1),
    /// Canonical absence after deletion. A later generation uses a new PDA.
    Absent,
}

/// Forgeable adapter projection claiming absence of one Replay PDA.
///
/// This pure DTO carries no runtime authority. A live adapter must prove that
/// the canonical account is system-owned and empty before supplying it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterReplayAbsenceProjectionV1 {
    /// Claimed canonical prior-generation Replay PDA.
    pub account: Identity32V1,
    /// Market identity used by the canonical Replay derivation.
    pub market: Identity32V1,
    /// Position owner used by the canonical Replay derivation.
    pub owner: Identity32V1,
    /// Closed Position generation whose Replay PDA is absent.
    pub position_generation: u64,
}

/// Forgeable adapter projection of a Position PDA and semantic body.
///
/// Public fields are hostile pure inputs, not an authentication capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPositionAccountProjectionV1 {
    /// Canonical Position PDA identity.
    pub account: Identity32V1,
    /// Claimed Market identity from the Position body and PDA seeds.
    pub market: Identity32V1,
    /// Claimed owner identity from the Position body and PDA seeds.
    pub owner: Identity32V1,
}

/// Forgeable adapter projection of a Replay PDA and semantic body/seed.
///
/// Public fields are hostile pure inputs, not an authentication capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterReplayAccountProjectionV1 {
    /// Canonical generation-scoped Replay PDA identity.
    pub account: Identity32V1,
    /// Claimed Market identity from the Replay body and PDA seeds.
    pub market: Identity32V1,
    /// Claimed Position owner from the Replay body and PDA seeds.
    pub owner: Identity32V1,
    /// Claimed Position generation from the Replay body and PDA seeds.
    pub position_generation: u64,
}

/// Forgeable adapter projection of one immutable Market/Realm neutral sink.
///
/// Public fields are cross-checked by pure plans but carry no proof that a
/// Realm or Market account selected this sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterNeutralSinkBindingProjectionV1 {
    /// Market whose immutable Realm/policy selected the sink.
    pub market: Identity32V1,
    /// Claimed neutral sink from that immutable owner.
    pub neutral_sink: Identity32V1,
}

impl AdapterNeutralSinkBindingProjectionV1 {
    fn require(self, market: Identity32V1, sink: Identity32V1) -> Result<(), RetirementErrorV2> {
        if self.market != market || self.neutral_sink != sink {
            Err(RetirementErrorV2::WrongNeutralSink)
        } else {
            Ok(())
        }
    }
}

/// Projected source account identities for atomic Position/Replay retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayAccountsV1 {
    /// Permanent Position PDA and claimed semantic binding.
    pub position: AdapterPositionAccountProjectionV1,
    /// Generation-scoped Replay PDA and claimed semantic binding.
    pub replay: AdapterReplayAccountProjectionV1,
}

/// Projected identities supplied for Position/Replay reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenAccountsV1 {
    /// Permanent Position PDA and claimed semantic binding.
    pub position: AdapterPositionAccountProjectionV1,
    /// Next-generation Replay target and claimed seed binding.
    pub next_replay: AdapterReplayAccountProjectionV1,
}

/// Complete atomic Position-tombstone plus Replay-deletion plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRetirementPlanV1 {
    /// Position tombstone post-state.
    pub position_post_state: PositionLifecycleStateV2,
    /// Replay absence post-state.
    pub replay_post_state: ReplayLifecycleStateV1,
    /// Coalesced and overflow-checked payer/sink credits.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// Exact retained Position tombstone balance.
    pub position_balance_after: u64,
    /// Exact deleted Replay balance.
    pub replay_balance_after: u64,
}

/// Exact source accounts for one purpose-owned Position V3/Replay V3 close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3ReplayV3AccountsV1 {
    /// Permanent Position V3 account rewritten to its V3 tombstone.
    pub position: Identity32V1,
    /// Exact purpose-owned Replay V3 account deleted atomically.
    pub replay: Identity32V1,
}

/// Complete pure inputs for one terminal Position V3/Replay V3 retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3ReplayV3RetirementRequestV1<'a> {
    /// Economically empty, close-requested Position V3 projection.
    pub position: PositionTerminalProjectionV3,
    /// Exact hash-authenticated terminal purpose-owned Replay V3 projection.
    pub replay: ReplayV3TerminalProjection<'a>,
    /// Actual Position lamports before shrinking to its permanent tombstone.
    pub position_balance: u64,
    /// Actual Replay lamports before deletion.
    pub replay_balance: u64,
    /// Canonical immutable Realm neutral lamport sink.
    pub neutral_sink: Identity32V1,
    /// Actual source account identities authenticated by the runtime adapter.
    pub accounts: PositionV3ReplayV3AccountsV1,
    /// Authenticated unique balances for both persisted payers and neutral sink.
    pub recipient_balances: RecipientBalanceBookV1,
    /// Sequence authenticated from the signed close instruction.
    pub signed_sequence: u64,
}

/// Atomic Position V3 tombstone plus purpose-owned Replay V3 deletion plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3ReplayV3RetirementPlanV1 {
    /// Full-identity permanent Position V3 tombstone.
    pub position_tombstone: PositionTombstoneV3,
    /// Semantic identity of the exact terminal Replay prefix and extension.
    pub terminal_replay_semantic_id: Identity32V1,
    /// Alias-coalesced, overflow-checked payer and neutral-sink credits.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// Exact permanent tombstone principal retained by Position.
    pub position_balance_after: u64,
    /// Replay is deleted to zero lamports.
    pub replay_balance_after: u64,
}

/// Plan the canonical Position V3 plus purpose-owned Replay V3 retirement.
///
/// Purpose handlers alone may terminalize their Replay extension. This common
/// planner deliberately does not interpret those bytes: it authenticates the
/// shared Position/Replay/purpose/generation/sequence join and moves only the
/// two independently persisted lamport-rent compartments. Position cash,
/// native Eggs, collateral principal, fees, and liveness funding cannot enter
/// this transition.
pub fn plan_position_v3_replay_v3_retirement_v1<B: ReplayV3HashBackend>(
    request: PositionV3ReplayV3RetirementRequestV1<'_>,
    backend: &B,
) -> Result<PositionV3ReplayV3RetirementPlanV1, RetirementErrorV2> {
    let position = request.position.position();
    let replay = request.replay.header();
    if request.accounts.position == request.accounts.replay
        || position.replay_account() != request.accounts.replay
        || replay.position_account() != request.accounts.position
        || replay.replay_account() != request.accounts.replay
        || replay.purpose() != position.purpose()
        || replay.purpose_binding_id() != position.purpose_binding_id()
        || replay.position_generation() != position.generation()
        || replay.next_sequence() != request.signed_sequence
    {
        return Err(RetirementErrorV2::ReplayMismatch);
    }
    require_no_source_recipient_alias(
        &[request.accounts.position, request.accounts.replay],
        request.recipient_balances,
    )?;

    let position_disposition = rent_disposition(
        position.rent(),
        request.position_balance,
        request.neutral_sink,
    )
    .map_err(RetirementErrorV2::from)?;
    let replay_disposition =
        deletable_rent_disposition(replay.rent(), request.replay_balance, request.neutral_sink)?;
    let mut credits = CoalescedRecipientCreditsV1::begin(request.recipient_balances)?;
    credits.credit(
        request.recipient_balances,
        position_disposition.payer,
        position_disposition.payer_refund_lamports,
    )?;
    credits.credit(
        request.recipient_balances,
        replay_disposition.payer,
        replay_disposition.payer_refund_lamports,
    )?;
    credits.credit(
        request.recipient_balances,
        request.neutral_sink,
        position_disposition.neutral_lamports,
    )?;
    credits.credit(
        request.recipient_balances,
        request.neutral_sink,
        replay_disposition.neutral_lamports,
    )?;

    let position_tombstone = request.position.tombstone()?;
    if position_disposition.tombstone_lamports
        != position_tombstone.fields().permanent_tombstone_principal
    {
        return Err(RetirementErrorV2::NonCanonicalState);
    }
    Ok(PositionV3ReplayV3RetirementPlanV1 {
        position_tombstone,
        terminal_replay_semantic_id: request.replay.semantic_id(backend)?,
        recipient_credits: credits,
        position_balance_after: position_disposition.tombstone_lamports,
        replay_balance_after: 0,
    })
}

/// Complete pure inputs for one Position/Replay retirement plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRetirementRequestV1 {
    /// Live Position or already-terminal tombstone.
    pub position: PositionLifecycleStateV2,
    /// Required matching generation-scoped Replay sibling.
    pub replay: ReplayLifecycleStateV1,
    /// Adapter-projected local Position economic compartments.
    pub economic: PositionEconomicStateV1,
    /// Actual Position lamport balance.
    pub position_balance: u64,
    /// Actual Replay lamport balance.
    pub replay_balance: u64,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical source account identities.
    pub accounts: PositionReplayAccountsV1,
    /// Claimed unique recipient balances that the runtime must authenticate.
    pub recipient_balances: RecipientBalanceBookV1,
}

fn require_no_source_recipient_alias(
    sources: &[Identity32V1],
    recipients: RecipientBalanceBookV1,
) -> Result<(), RetirementErrorV2> {
    recipients.validate()?;
    let mut left = 0usize;
    while left < sources.len() {
        let mut right = left + 1;
        while right < sources.len() {
            if sources[left] == sources[right] {
                return Err(RetirementErrorV2::AccountAlias);
            }
            right += 1;
        }
        let mut recipient = 0usize;
        while recipient < recipients.entries.len() {
            if recipients.entries[recipient].map(|entry| entry.recipient) == Some(sources[left]) {
                return Err(RetirementErrorV2::AccountAlias);
            }
            recipient += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_no_target_payer_alias(
    targets: &[Identity32V1],
    payers: CoalescedPayerDebitsV1,
    neutral_sink: Identity32V1,
) -> Result<(), RetirementErrorV2> {
    let mut left = 0usize;
    while left < targets.len() {
        if targets[left] == neutral_sink {
            return Err(RetirementErrorV2::AccountAlias);
        }
        let mut right = left + 1;
        while right < targets.len() {
            if targets[left] == targets[right] {
                return Err(RetirementErrorV2::AccountAlias);
            }
            right += 1;
        }
        let mut payer = 0usize;
        while payer < payers.entries.len() {
            if payers.entries[payer].map(|entry| entry.payer) == Some(targets[left]) {
                return Err(RetirementErrorV2::AccountAlias);
            }
            payer += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Exact alias-safe deletion balance plan for one source account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableAccountClosePlanV1 {
    /// Coalesced, overflow-checked payer and neutral-sink credits.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// Deleted source account's exact final balance.
    pub account_balance_after: u64,
}

fn plan_deletable_account_close(
    rent: DeletableRentOwnerV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
    source_accounts: &[Identity32V1],
    recipient_balances: RecipientBalanceBookV1,
) -> Result<DeletableAccountClosePlanV1, RetirementErrorV2> {
    require_no_source_recipient_alias(source_accounts, recipient_balances)?;
    let disposition = deletable_rent_disposition(rent, actual_balance, neutral_sink)?;
    let mut credits = CoalescedRecipientCreditsV1::begin(recipient_balances)?;
    credits.credit(
        recipient_balances,
        disposition.payer,
        disposition.payer_refund_lamports,
    )?;
    credits.credit(
        recipient_balances,
        disposition.neutral_sink,
        disposition.neutral_lamports,
    )?;
    Ok(DeletableAccountClosePlanV1 {
        recipient_credits: credits,
        account_balance_after: 0,
    })
}

/// Atomically plan Position tombstoning and deletion of the exact matching
/// generation-scoped Replay sibling.
pub fn plan_position_replay_retirement(
    request: PositionReplayRetirementRequestV1,
) -> Result<PositionReplayRetirementPlanV1, RetirementErrorV2> {
    let PositionReplayRetirementRequestV1 {
        position,
        replay,
        economic,
        position_balance,
        replay_balance,
        neutral_sink,
        neutral_sink_binding,
        accounts,
        recipient_balances,
    } = request;
    let live_position = match position {
        PositionLifecycleStateV2::Live(live) => live,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV2::AlreadyTerminal),
    };
    neutral_sink_binding.require(live_position.market, neutral_sink)?;
    let live_replay = match replay {
        ReplayLifecycleStateV1::Live(live) => live,
        ReplayLifecycleStateV1::Absent => return Err(RetirementErrorV2::ReplayMismatch),
    };
    live_replay.validate()?;
    if live_replay.market != live_position.market
        || live_replay.owner != live_position.owner
        || live_replay.position_generation != live_position.generation
    {
        return Err(RetirementErrorV2::ReplayMismatch);
    }
    if accounts.position.market != live_position.market
        || accounts.position.owner != live_position.owner
        || accounts.replay.market != live_replay.market
        || accounts.replay.owner != live_replay.owner
        || accounts.replay.position_generation != live_replay.position_generation
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    require_no_source_recipient_alias(
        &[accounts.position.account, accounts.replay.account],
        recipient_balances,
    )?;
    let position_plan =
        plan_position_root_retirement(position, economic, position_balance, neutral_sink, 0, 0)?;
    let replay_disposition =
        deletable_rent_disposition(live_replay.rent, replay_balance, neutral_sink)?;
    let mut credits = CoalescedRecipientCreditsV1::begin(recipient_balances)?;
    credits.credit(
        recipient_balances,
        position_plan.payer,
        live_position.retirement.rent.refundable_live_principal,
    )?;
    credits.credit(
        recipient_balances,
        replay_disposition.payer,
        replay_disposition.payer_refund_lamports,
    )?;
    credits.credit(
        recipient_balances,
        neutral_sink,
        position_plan.neutral_balance_after,
    )?;
    credits.credit(
        recipient_balances,
        neutral_sink,
        replay_disposition.neutral_lamports,
    )?;
    Ok(PositionReplayRetirementPlanV1 {
        position_post_state: position_plan.post_state,
        replay_post_state: ReplayLifecycleStateV1::Absent,
        recipient_credits: credits,
        position_balance_after: position_plan.tombstone_balance_after,
        replay_balance_after: 0,
    })
}

/// Signed-sequence-bound inputs for a production Position/Replay close path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRetirementRequestV2 {
    /// Complete V1 economic and account graph.
    pub retirement: PositionReplayRetirementRequestV1,
    /// Sequence authenticated from the signed instruction envelope.
    pub signed_sequence: u64,
}

/// Plan Position/Replay retirement only after binding the signed sequence to
/// the exact authenticated generation-scoped Replay account.
///
/// The V1 planner remains available as a pure compatibility surface, but a
/// live mutation route must use this entry point so changing Replay sequence
/// bytes cannot preserve close success.
pub fn plan_position_replay_retirement_v2(
    request: PositionReplayRetirementRequestV2,
) -> Result<PositionReplayRetirementPlanV1, RetirementErrorV2> {
    match request.retirement.replay {
        ReplayLifecycleStateV1::Live(replay) if replay.sequence == request.signed_sequence => {}
        ReplayLifecycleStateV1::Live(_) => return Err(RetirementErrorV2::ReplayMismatch),
        ReplayLifecycleStateV1::Absent => return Err(RetirementErrorV2::ReplayMismatch),
    }
    plan_position_replay_retirement(request.retirement)
}

/// Production successor Position/Replay close request.
///
/// This wraps the signed-sequence V2 request but upgrades the retained Position
/// image to the rent-owner-complete tombstone V2 codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRetirementRequestV3 {
    /// Complete signed-sequence-bound retirement inputs.
    pub retirement: PositionReplayRetirementRequestV2,
}

/// Production successor Position/Replay close plan with tombstone V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRetirementPlanV2 {
    /// Exact permanent Position tombstone with retained principal.
    pub position_tombstone: PositionTombstoneV2,
    /// Replay successor is deleted atomically.
    pub replay_post_state: ReplayLifecycleStateV1,
    /// Alias-coalesced recipient credits.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// Exact retained Position balance.
    pub position_balance_after: u64,
    /// Exact deleted Replay balance.
    pub replay_balance_after: u64,
}

/// Plan production Position/Replay retirement using the successor tombstone.
pub fn plan_position_replay_retirement_v3(
    request: PositionReplayRetirementRequestV3,
) -> Result<PositionReplayRetirementPlanV2, RetirementErrorV2> {
    let live = match request.retirement.retirement.position {
        PositionLifecycleStateV2::Live(value) => value,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV2::AlreadyTerminal),
    };
    let plan = plan_position_replay_retirement_v2(request.retirement)?;
    let identity = match plan.position_post_state {
        PositionLifecycleStateV2::Tombstone(value) => value,
        PositionLifecycleStateV2::Live(_) => return Err(RetirementErrorV2::WrongPhase),
    };
    let position_tombstone = PositionTombstoneV2 {
        market: identity.market,
        owner: identity.owner,
        generation: identity.generation,
        stored_bump: identity.stored_bump,
        permanent_tombstone_principal: live.retirement.rent.permanent_tombstone_principal,
    };
    position_tombstone.validate()?;
    if plan.position_balance_after != position_tombstone.permanent_tombstone_principal {
        return Err(RetirementErrorV2::NonCanonicalState);
    }
    Ok(PositionReplayRetirementPlanV2 {
        position_tombstone,
        replay_post_state: plan.replay_post_state,
        recipient_credits: plan.recipient_credits,
        position_balance_after: plan.position_balance_after,
        replay_balance_after: plan.replay_balance_after,
    })
}

/// Complete atomic reopen plan for the next Position and Replay generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenPlanV1 {
    /// Reopened Position next generation.
    pub position_post_state: PositionLifecycleStateV2,
    /// Fresh Replay sibling with sequence zero and the same next generation.
    pub replay_post_state: ReplayLifecycleStateV1,
    /// Full-principal admission for the reopened Position PDA.
    pub position_funding: RentSplitAdmissionPlanV2,
    /// Full-principal hostile-prefund admission for the Replay PDA.
    pub replay_funding: DeletableRentAdmissionPlanV1,
    /// Coalesced payer debits across Position and Replay.
    pub payer_debits: CoalescedPayerDebitsV1,
    /// Exact Position balance after the checked funding transfer.
    pub position_balance_after: u64,
    /// Exact Replay balance after the checked funding transfer.
    pub replay_balance_after: u64,
}

/// Complete pure inputs for atomic Position/Replay reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenRequestV1 {
    /// Permanent Position tombstone.
    pub position: PositionLifecycleStateV2,
    /// Forgeable adapter projection of prior Replay absence.
    pub prior_replay: AdapterReplayAbsenceProjectionV1,
    /// Independently admitted live/tombstone Position funding split.
    pub position_funding: RentSplitAdmissionPlanV2,
    /// Canonical bump for the next generation's Replay PDA.
    pub replay_stored_bump: u8,
    /// Independently admitted full Replay principal and hostile prefund.
    pub replay_funding: DeletableRentAdmissionPlanV1,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical Position, prior Replay, and next Replay identities.
    pub accounts: PositionReplayReopenAccountsV1,
}

/// Forgeable adapter projection of a fresh General V2 Position namespace.
///
/// A live adapter may construct this only from the exact program-owned
/// General V2 MarketBinding account created atomically with a never-legacy
/// Market. System-account absence alone cannot distinguish a never-used
/// Position PDA from a legacy Position deleted without a tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterFreshPositionNamespaceProjectionV1 {
    /// Fresh Market identity that had no legacy Position family.
    pub market: Identity32V1,
    /// Immutable neutral sink selected by that Market's Realm binding.
    pub neutral_sink: Identity32V1,
}

/// Forgeable adapter projection claiming canonical Position-PDA absence.
///
/// Runtime code must prove the exact PDA is System-owned, zero-data,
/// non-executable, and writable while separately preserving its observed
/// hostile prefund in the funding admission plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPositionAbsenceProjectionV1 {
    /// Claimed absent canonical Position PDA.
    pub account: Identity32V1,
    /// Market seed used by the Position derivation.
    pub market: Identity32V1,
    /// Owner seed used by the Position derivation.
    pub owner: Identity32V1,
}

/// Complete pure inputs for founding Position V2 and Replay successor
/// generation zero in a fresh, never-legacy Market namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayFoundingRequestV1 {
    /// Authenticated fresh General V2 Market namespace projection.
    pub namespace: AdapterFreshPositionNamespaceProjectionV1,
    /// Canonical absent Position target.
    pub position_absence: AdapterPositionAbsenceProjectionV1,
    /// Canonical absent generation-zero Replay target.
    pub replay_absence: AdapterReplayAbsenceProjectionV1,
    /// Full live plus permanent-tombstone Position funding admission.
    pub position_funding: RentSplitAdmissionPlanV2,
    /// Full Replay principal funding admission.
    pub replay_funding: DeletableRentAdmissionPlanV1,
    /// Canonical stored Position bump.
    pub position_stored_bump: u8,
    /// Canonical stored Replay bump.
    pub replay_stored_bump: u8,
    /// Immutable Market/Realm neutral-sink binding.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical Position and generation-zero Replay account identities.
    pub accounts: PositionReplayReopenAccountsV1,
}

/// Found a generation-zero Position and Replay only in an authenticated fresh
/// namespace and only after full hostile-prefund funding admission.
///
/// There is intentionally no Position V1-to-V2 migration function. Legacy
/// Position V1 owns no exhaustive reservation count, so account-local bytes
/// cannot prove that every economically live Reservation has been retired.
pub fn found_position_with_replay(
    request: PositionReplayFoundingRequestV1,
) -> Result<PositionReplayReopenPlanV1, RetirementErrorV2> {
    let market = request.namespace.market;
    let owner = request.position_absence.owner;
    let neutral_sink = request.namespace.neutral_sink;
    request.neutral_sink_binding.require(market, neutral_sink)?;
    if request.position_absence.market != market
        || request.replay_absence.market != market
        || request.replay_absence.owner != owner
        || request.replay_absence.position_generation != 0
        || request.accounts.position.market != market
        || request.accounts.position.owner != owner
        || request.accounts.next_replay.market != market
        || request.accounts.next_replay.owner != owner
        || request.accounts.next_replay.position_generation != 0
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    if request.position_absence.account != request.accounts.position.account
        || request.replay_absence.account != request.accounts.next_replay.account
        || request.position_funding.target() != request.accounts.position.account
        || request.replay_funding.target() != request.accounts.next_replay.account
    {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    if request.position_funding.neutral_sink() != neutral_sink
        || request.replay_funding.neutral_sink() != neutral_sink
    {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    let expected_position_debit = request
        .position_funding
        .rent()
        .refundable_live_principal
        .checked_add(
            request
                .position_funding
                .rent()
                .permanent_tombstone_principal,
        )
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if request.position_funding.payer_debit_lamports() != expected_position_debit {
        return Err(RetirementErrorV2::NonCanonicalState);
    }

    let mut payer_debits = CoalescedPayerDebitsV1::empty();
    payer_debits.debit(
        request.position_funding.rent().payer,
        request.position_funding.payer_debit_lamports(),
        request.position_funding.payer_balance_after(),
    )?;
    payer_debits.debit(
        request.replay_funding.rent().payer(),
        request.replay_funding.payer_debit_lamports(),
        request.replay_funding.payer_balance_after(),
    )?;
    require_no_target_payer_alias(
        &[
            request.accounts.position.account,
            request.accounts.next_replay.account,
        ],
        payer_debits,
        neutral_sink,
    )?;

    let position = LivePositionV2 {
        market,
        owner,
        generation: 0,
        stored_bump: request.position_stored_bump,
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: request.position_funding.rent(),
        },
    };
    position.validate()?;
    let replay = LiveReplaySuccessorV1 {
        market,
        owner,
        position_generation: 0,
        sequence: 0,
        stored_bump: request.replay_stored_bump,
        rent: request.replay_funding.rent(),
    };
    replay.validate()?;
    Ok(PositionReplayReopenPlanV1 {
        position_post_state: PositionLifecycleStateV2::Live(position),
        replay_post_state: ReplayLifecycleStateV1::Live(replay),
        position_funding: request.position_funding,
        replay_funding: request.replay_funding,
        payer_debits,
        position_balance_after: request.position_funding.account_balance_after(),
        replay_balance_after: request.replay_funding.account_balance_after(),
    })
}

/// Atomically construct a reopened Position and its independently funded,
/// generation-scoped Replay sibling.
pub fn reopen_position_with_replay(
    request: PositionReplayReopenRequestV1,
) -> Result<PositionReplayReopenPlanV1, RetirementErrorV2> {
    let PositionReplayReopenRequestV1 {
        position,
        prior_replay,
        position_funding,
        replay_stored_bump,
        replay_funding,
        neutral_sink,
        neutral_sink_binding,
        accounts,
    } = request;
    let tombstone = match position {
        PositionLifecycleStateV2::Tombstone(tombstone) => tombstone,
        PositionLifecycleStateV2::Live(_) => return Err(RetirementErrorV2::WrongPhase),
    };
    neutral_sink_binding.require(tombstone.market, neutral_sink)?;
    if prior_replay.market != tombstone.market
        || prior_replay.owner != tombstone.owner
        || prior_replay.position_generation != tombstone.generation
    {
        return Err(RetirementErrorV2::ReplayMismatch);
    }
    if accounts.position.market != tombstone.market || accounts.position.owner != tombstone.owner {
        return Err(RetirementErrorV2::WrongParent);
    }
    if position_funding.neutral_sink() != neutral_sink
        || replay_funding.neutral_sink() != neutral_sink
    {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    if position_funding.target() != accounts.position.account
        || replay_funding.target() != accounts.next_replay.account
    {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    if position_funding.payer_debit_lamports() != position_funding.rent().refundable_live_principal
    {
        return Err(RetirementErrorV2::NonCanonicalState);
    }
    let mut payer_debits = CoalescedPayerDebitsV1::empty();
    payer_debits.debit(
        position_funding.rent().payer,
        position_funding.payer_debit_lamports(),
        position_funding.payer_balance_after(),
    )?;
    payer_debits.debit(
        replay_funding.rent().payer(),
        replay_funding.payer_debit_lamports(),
        replay_funding.payer_balance_after(),
    )?;
    let next_generation = tombstone
        .generation
        .checked_add(1)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if accounts.next_replay.market != tombstone.market
        || accounts.next_replay.owner != tombstone.owner
        || accounts.next_replay.position_generation != next_generation
    {
        return Err(RetirementErrorV2::ReplayMismatch);
    }
    require_no_target_payer_alias(
        &[
            accounts.position.account,
            prior_replay.account,
            accounts.next_replay.account,
        ],
        payer_debits,
        neutral_sink,
    )?;
    let position_post_state =
        reopen_position_root(position, position_funding.rent(), neutral_sink)?;
    let live_position = match position_post_state {
        PositionLifecycleStateV2::Live(live) => live,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV2::WrongPhase),
    };
    let replay = LiveReplaySuccessorV1 {
        market: live_position.market,
        owner: live_position.owner,
        position_generation: live_position.generation,
        sequence: 0,
        stored_bump: replay_stored_bump,
        rent: replay_funding.rent,
    };
    replay.validate()?;
    Ok(PositionReplayReopenPlanV1 {
        position_post_state,
        replay_post_state: ReplayLifecycleStateV1::Live(replay),
        position_funding,
        replay_funding,
        payer_debits,
        position_balance_after: position_funding.account_balance_after(),
        replay_balance_after: replay_funding.account_balance_after(),
    })
}

/// Successor reopen request that accounts for a hostile prefund sent to the
/// already-deleted predecessor Replay PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenRequestV2 {
    /// Complete generation-safe V1 reopen request.
    pub reopen: PositionReplayReopenRequestV1,
    /// Actual lamports on the System-owned, zero-data predecessor Replay PDA.
    pub prior_replay_prefund_lamports: u64,
    /// Authenticated neutral-sink balance before sweeping that prefund.
    pub neutral_sink_balance_before: u64,
}

/// Complete next-generation reopen plan including predecessor-prefund sweep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenPlanV2 {
    /// Position/next-Replay funding and post-state plan.
    pub reopen: PositionReplayReopenPlanV1,
    /// Exact System transfer from the predecessor Replay PDA to the sink.
    pub prior_replay_prefund_lamports: u64,
    /// Predecessor Replay balance after the sweep.
    pub prior_replay_balance_after: u64,
    /// Neutral-sink balance after receiving the checked prefund.
    pub neutral_sink_balance_after: u64,
}

/// Plan reopen without stranding an attacker's prefund at an obsolete Replay
/// generation.
///
/// The live adapter must authenticate the predecessor as a writable,
/// System-owned, zero-data, non-executable canonical PDA and execute its
/// program-signed System transfer in the same transaction as both funding
/// transfers and state writes. A zero prefund remains canonical and emits no
/// transfer.
pub fn reopen_position_with_replay_v2(
    request: PositionReplayReopenRequestV2,
) -> Result<PositionReplayReopenPlanV2, RetirementErrorV2> {
    let neutral_sink_balance_after = request
        .neutral_sink_balance_before
        .checked_add(request.prior_replay_prefund_lamports)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let reopen = reopen_position_with_replay(request.reopen)?;
    Ok(PositionReplayReopenPlanV2 {
        reopen,
        prior_replay_prefund_lamports: request.prior_replay_prefund_lamports,
        prior_replay_balance_after: 0,
        neutral_sink_balance_after,
    })
}

/// Frozen forgeable projection used by the committed general Epoch V5 API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveEpochV5 {
    /// Claimed Market identity from the base Epoch body.
    pub market: Identity32V1,
    /// Claimed canonical Epoch identity from the base Epoch body.
    pub epoch: Identity32V1,
    /// Monotone Market-owned index.
    pub epoch_index: u64,
    /// Frozen three-state pure lifecycle projection.
    pub phase: crate::GeneralEpochPhaseV1,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Generation, counts, and rent state owned by this crate.
    pub retirement: EpochRetirementTailV1,
}

impl LiveEpochV5 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        self.retirement.validate()
    }
}

/// Frozen live-or-tombstone general Epoch V5 state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochLifecycleStateV5 {
    /// Full live general Epoch V5 projection.
    Live(LiveEpochV5),
    /// Compact permanent general Epoch tombstone.
    Tombstone(GeneralEpochTombstoneV1),
}

/// Consume the committed Market cursor and construct a fresh OPEN V5 Epoch.
pub fn open_general_epoch(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
    market: Identity32V1,
    epoch: Identity32V1,
    stored_bump: u8,
    rent: RentSplitV2,
) -> Result<(MarketEpochCursorV1, LiveEpochV5), RetirementErrorV1> {
    if requested_index != cursor.next_general_epoch_index {
        return Err(RetirementErrorV1::NonmonotoneEpoch);
    }
    if requested_index == u64::MAX {
        return Err(RetirementErrorV1::EpochIndexExhausted);
    }
    rent.validate()?;
    let next_index = requested_index
        .checked_add(1)
        .ok_or(RetirementErrorV1::EpochIndexExhausted)?;
    let live = LiveEpochV5 {
        market,
        epoch,
        epoch_index: requested_index,
        phase: crate::GeneralEpochPhaseV1::Open,
        stored_bump,
        retirement: EpochRetirementTailV1 {
            epoch_generation: next_index,
            children: Default::default(),
            rent,
        },
    };
    live.validate()?;
    Ok((
        MarketEpochCursorV1 {
            next_general_epoch_index: next_index,
        },
        live,
    ))
}

/// Close through the frozen root-only V5 API.
///
/// This compatibility surface is not live authorization and does not satisfy
/// the successor Window/Budget retirement requirements.
pub fn close_epoch(
    state: EpochLifecycleStateV5,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(EpochLifecycleStateV5, RentDispositionV2), RetirementErrorV1> {
    let live = match state {
        EpochLifecycleStateV5::Live(live) => live,
        EpochLifecycleStateV5::Tombstone(_) => return Err(RetirementErrorV1::AlreadyTerminal),
    };
    live.validate()?;
    if live.phase == crate::GeneralEpochPhaseV1::Open {
        return Err(RetirementErrorV1::WrongPhase);
    }
    if !live.retirement.children.is_zero() {
        return Err(RetirementErrorV1::ChildOutstanding);
    }
    let disposition = rent_disposition(live.retirement.rent, actual_balance, neutral_sink)?;
    let tombstone = GeneralEpochTombstoneV1 {
        epoch: live.epoch,
        market: live.market,
        epoch_index: live.epoch_index,
        epoch_generation: live.retirement.epoch_generation,
        stored_bump: live.stored_bump,
    };
    tombstone.validate()?;
    Ok((EpochLifecycleStateV5::Tombstone(tombstone), disposition))
}

/// Precompute the frozen root-only V5 retirement plan.
///
/// Live runtime integration remains fail-closed on the successor Budget
/// disposition blocker.
pub fn plan_epoch_retirement(
    state: EpochLifecycleStateV5,
    actual_balance: u64,
    neutral_sink: Identity32V1,
    payer_balance_before: u64,
    neutral_balance_before: u64,
) -> Result<RetirementCommitPlanV2<EpochLifecycleStateV5>, RetirementErrorV1> {
    let (post_state, disposition) = close_epoch(state, actual_balance, neutral_sink)?;
    commit_plan(
        post_state,
        disposition,
        payer_balance_before,
        neutral_balance_before,
    )
}

/// Forgeable successor projection of a live general Epoch V5 body with the
/// complete five-state retirement lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGeneralEpochProjectionV2 {
    /// Claimed Market identity from the base Epoch body.
    pub market: Identity32V1,
    /// Claimed canonical Epoch identity from the base Epoch body.
    pub epoch: Identity32V1,
    /// Monotone Market-owned index.
    pub epoch_index: u64,
    /// Current Epoch lifecycle phase.
    pub phase: GeneralEpochPhaseV2,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Generation, counts, and rent state owned by this crate.
    pub retirement: EpochRetirementTailV1,
}

impl LiveGeneralEpochProjectionV2 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        Ok(self.retirement.validate()?)
    }
}

/// Successor live-or-tombstone general Epoch projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEpochLifecycleProjectionV2 {
    /// Full live general Epoch V5.
    Live(LiveGeneralEpochProjectionV2),
    /// Compact permanent general Epoch tombstone.
    Tombstone(GeneralEpochTombstoneV1),
}

/// Forgeable adapter projection of a Market PDA and semantic identity.
///
/// Public fields are hostile pure inputs, not an authentication capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterMarketAccountProjectionV1 {
    /// Canonical Market PDA identity.
    pub account: Identity32V1,
    /// Claimed Market identity from its body and PDA seeds.
    pub market: Identity32V1,
}

/// Forgeable adapter projection of an Epoch PDA and semantic identity.
///
/// Public fields are hostile pure inputs, not an authentication capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterEpochAccountProjectionV1 {
    /// Canonical Epoch PDA identity, distinct from the semantic Epoch id.
    pub account: Identity32V1,
    /// Claimed Market identity from the Epoch body and PDA seeds.
    pub market: Identity32V1,
    /// Claimed semantic Epoch identity from the body and PDA seeds.
    pub epoch: Identity32V1,
    /// Claimed Epoch index from the body and PDA seeds.
    pub epoch_index: u64,
}

/// Consume the exact Market cursor and construct a fresh OPEN general Epoch.
///
/// The adapter creates the complete root bundle and writes the returned cursor
/// and Epoch in one transaction. Index `u64::MAX` is never admitted because no
/// strictly greater cursor exists.
fn open_general_epoch_root_only(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
    market: Identity32V1,
    epoch: Identity32V1,
    stored_bump: u8,
    rent: RentSplitV2,
) -> Result<(MarketEpochCursorV1, LiveGeneralEpochProjectionV2), RetirementErrorV2> {
    if requested_index != cursor.next_general_epoch_index {
        return Err(RetirementErrorV2::NonmonotoneEpoch);
    }
    if requested_index == u64::MAX {
        return Err(RetirementErrorV2::EpochIndexExhausted);
    }
    rent.validate()?;
    let next_index = requested_index
        .checked_add(1)
        .ok_or(RetirementErrorV2::EpochIndexExhausted)?;
    let live = LiveGeneralEpochProjectionV2 {
        market,
        epoch,
        epoch_index: requested_index,
        phase: GeneralEpochPhaseV2::Open,
        stored_bump,
        retirement: EpochRetirementTailV1 {
            epoch_generation: next_index,
            children: Default::default(),
            rent,
        },
    };
    live.validate()?;
    Ok((
        MarketEpochCursorV1 {
            next_general_epoch_index: next_index,
        },
        live,
    ))
}

/// Complete pure inputs for atomic general-Epoch root creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenGeneralEpochRootRequestV1 {
    /// Current Market-owned next-index cursor.
    pub cursor: MarketEpochCursorV1,
    /// Exact index requested by the instruction.
    pub requested_index: u64,
    /// Market identity.
    pub market: Identity32V1,
    /// Canonical Market account and semantic binding.
    pub market_account: AdapterMarketAccountProjectionV1,
    /// Canonical Epoch identity derived from Market and requested index.
    pub epoch: Identity32V1,
    /// Canonical Epoch PDA bump.
    pub stored_bump: u8,
    /// Full-principal admission for Epoch live plus tombstone compartments.
    pub epoch_funding: RentSplitAdmissionPlanV2,
    /// Full-principal hostile-prefund admission for Window.
    pub window_funding: DeletableRentAdmissionPlanV1,
    /// Full-principal hostile-prefund admission for candidate-work Budget.
    pub budget_funding: DeletableRentAdmissionPlanV1,
    /// Frozen neutral sink against which every admission was checked.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical Epoch, Window, and Budget target identities.
    pub accounts: EpochRootAccountsV1,
}

/// Complete atomic general-Epoch root creation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenGeneralEpochRootPlanV1 {
    /// Advanced Market cursor.
    pub cursor_post_state: MarketEpochCursorV1,
    /// Fresh OPEN counted Epoch.
    pub epoch_post_state: LiveGeneralEpochProjectionV2,
    /// Fresh generation-matched Window sibling funding projection.
    pub window_post_state: EpochWindowRootSiblingV1,
    /// Fresh generation-matched Budget sibling funding projection.
    pub budget_post_state: EpochBudgetRootSiblingV1,
    /// Coalesced payer debits across all three root accounts.
    pub payer_debits: CoalescedPayerDebitsV1,
    /// Exact Epoch balance after funding.
    pub epoch_balance_after: u64,
    /// Exact Window balance after funding.
    pub window_balance_after: u64,
    /// Exact Budget balance after funding.
    pub budget_balance_after: u64,
}

/// Atomically construct the complete Epoch/Window/Budget root bundle.
pub fn open_general_epoch_root(
    request: OpenGeneralEpochRootRequestV1,
) -> Result<OpenGeneralEpochRootPlanV1, RetirementErrorV2> {
    let OpenGeneralEpochRootRequestV1 {
        cursor,
        requested_index,
        market,
        market_account,
        epoch,
        stored_bump,
        epoch_funding,
        window_funding,
        budget_funding,
        neutral_sink,
        neutral_sink_binding,
        accounts,
    } = request;
    if market_account.market != market
        || accounts.epoch.market != market
        || accounts.epoch.epoch != epoch
        || accounts.epoch.epoch_index != requested_index
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    neutral_sink_binding.require(market, neutral_sink)?;
    if epoch_funding.neutral_sink() != neutral_sink
        || window_funding.neutral_sink() != neutral_sink
        || budget_funding.neutral_sink() != neutral_sink
    {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    if epoch_funding.target() != accounts.epoch.account
        || window_funding.target() != accounts.window
        || budget_funding.target() != accounts.budget
    {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    let epoch_initial_principal = epoch_funding
        .rent()
        .refundable_live_principal
        .checked_add(epoch_funding.rent().permanent_tombstone_principal)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if epoch_funding.payer_debit_lamports() != epoch_initial_principal {
        return Err(RetirementErrorV2::NonCanonicalState);
    }
    let mut payer_debits = CoalescedPayerDebitsV1::empty();
    payer_debits.debit(
        epoch_funding.rent().payer,
        epoch_funding.payer_debit_lamports(),
        epoch_funding.payer_balance_after(),
    )?;
    payer_debits.debit(
        window_funding.rent().payer(),
        window_funding.payer_debit_lamports(),
        window_funding.payer_balance_after(),
    )?;
    payer_debits.debit(
        budget_funding.rent().payer(),
        budget_funding.payer_debit_lamports(),
        budget_funding.payer_balance_after(),
    )?;
    require_no_target_payer_alias(
        &[
            market_account.account,
            accounts.epoch.account,
            accounts.window,
            accounts.budget,
        ],
        payer_debits,
        neutral_sink,
    )?;
    let (cursor_post_state, epoch_post_state) = open_general_epoch_root_only(
        cursor,
        requested_index,
        market,
        epoch,
        stored_bump,
        epoch_funding.rent(),
    )?;
    let epoch_generation = epoch_post_state.retirement.epoch_generation;
    let _non_executable_rent_only_projection = OpenGeneralEpochRootPlanV1 {
        cursor_post_state,
        epoch_post_state,
        window_post_state: EpochWindowRootSiblingV1 {
            market,
            epoch,
            epoch_generation,
            rent: window_funding.rent(),
        },
        budget_post_state: EpochBudgetRootSiblingV1 {
            market,
            epoch,
            epoch_generation,
            rent: budget_funding.rent(),
        },
        payer_debits,
        epoch_balance_after: epoch_funding.account_balance_after(),
        window_balance_after: window_funding.account_balance_after(),
        budget_balance_after: budget_funding.account_balance_after(),
    };
    Err(RetirementErrorV2::BudgetFundingUnauthenticated)
}

/// Close a terminal child-free Epoch into its permanent identity tombstone.
fn close_epoch_root(
    state: GeneralEpochLifecycleProjectionV2,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(GeneralEpochLifecycleProjectionV2, RentDispositionV2), RetirementErrorV2> {
    let live = match state {
        GeneralEpochLifecycleProjectionV2::Live(live) => live,
        GeneralEpochLifecycleProjectionV2::Tombstone(_) => {
            return Err(RetirementErrorV2::AlreadyTerminal)
        }
    };
    live.validate()?;
    if !matches!(
        live.phase,
        GeneralEpochPhaseV2::Settled | GeneralEpochPhaseV2::Lapsed
    ) {
        return Err(RetirementErrorV2::WrongPhase);
    }
    if !live.retirement.children.is_zero() {
        return Err(RetirementErrorV2::ChildOutstanding);
    }
    let disposition = rent_disposition(live.retirement.rent, actual_balance, neutral_sink)?;
    let tombstone = GeneralEpochTombstoneV1 {
        epoch: live.epoch,
        market: live.market,
        epoch_index: live.epoch_index,
        epoch_generation: live.retirement.epoch_generation,
        stored_bump: live.stored_bump,
    };
    tombstone.validate()?;
    Ok((
        GeneralEpochLifecycleProjectionV2::Tombstone(tombstone),
        disposition,
    ))
}

/// Precompute the Epoch root account's tombstone, payer credit, sink credit,
/// and retained balance before an adapter mutates any account.
///
/// This root-only helper is private so no caller can bypass the mandatory
/// Window/Budget plan exported by [`plan_epoch_root_retirement`].
fn plan_epoch_root_only_retirement(
    state: GeneralEpochLifecycleProjectionV2,
    actual_balance: u64,
    neutral_sink: Identity32V1,
    payer_balance_before: u64,
    neutral_balance_before: u64,
) -> Result<RetirementCommitPlanV2<GeneralEpochLifecycleProjectionV2>, RetirementErrorV2> {
    let (post_state, disposition) = close_epoch_root(state, actual_balance, neutral_sink)?;
    Ok(commit_plan(
        post_state,
        disposition,
        payer_balance_before,
        neutral_balance_before,
    )?)
}

/// Separately funded mandatory Candidate Window root sibling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochWindowRootSiblingV1 {
    /// Claimed Market identity projected from CandidateWindowV4.
    pub market: Identity32V1,
    /// Claimed semantic Epoch identity projected from CandidateWindowV4.
    pub epoch: Identity32V1,
    /// Claimed parent generation projected from the sibling body.
    pub epoch_generation: u64,
    /// Independent payer/principal/donation owner for full deletion.
    pub rent: DeletableRentOwnerV1,
}

impl EpochWindowRootSiblingV1 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        self.rent.validate()
    }
}

/// Opaque pure witness that one CandidateWindowV4 admission ledger is
/// terminal and empty for one counted general Epoch generation.
///
/// Fields are private so callers cannot manufacture a retired bit. The only
/// constructor consumes and validates the complete semantic-owner state and
/// exact live Epoch binding. This does not prove runtime owner, codec, account,
/// or PDA authenticity; live activation requires a future exact WindowV4
/// adapter before invoking this pure seam.
///
/// The private fields make direct fabrication a compile error:
///
/// ```compile_fail
/// use clutch_retirement::{Identity32V1, ValidatedAdmissionLedgerRetiredV1};
/// let id = Identity32V1::new([1; 32]).unwrap();
/// let _ = ValidatedAdmissionLedgerRetiredV1 {
///     market: id,
///     epoch: id,
///     epoch_generation: 1,
///     finalized_slot: 1,
///     admitted_count: 0,
///     closed_node_count: 0,
///     best_candidate_node: [0; 32],
///     selected_candidate_node: [0; 32],
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedAdmissionLedgerRetiredV1 {
    market: Identity32V1,
    epoch: Identity32V1,
    epoch_generation: u64,
    finalized_slot: u64,
    admitted_count: u64,
    closed_node_count: u64,
    best_candidate_node: [u8; 32],
    selected_candidate_node: [u8; 32],
}

impl ValidatedAdmissionLedgerRetiredV1 {
    /// Check CandidateWindowV4's complete semantic invariants, bind it to the
    /// exact live Epoch, and admit only a finalized, headless, fully closed
    /// reverse-linked ledger.
    pub fn from_candidate_window(
        window: CandidateWindowV4,
        epoch: LiveGeneralEpochProjectionV2,
    ) -> Result<Self, RetirementErrorV2> {
        window
            .validate()
            .map_err(|_| RetirementErrorV2::NonCanonicalState)?;
        epoch.validate()?;
        let market = Identity32V1::new(window.market.bytes())?;
        let window_epoch = Identity32V1::new(window.epoch.bytes())?;
        if market != epoch.market || window_epoch != epoch.epoch {
            return Err(RetirementErrorV2::WrongParent);
        }
        if !window
            .admission_ledger_retired()
            .map_err(|_| RetirementErrorV2::NonCanonicalState)?
            || window.finalized_slot == 0
            || window.live_node_count != 0
            || !window.admission_head.is_zero()
            || window.closed_node_count != window.admitted_count
            || window.selected_candidate_node != window.best_candidate_node
        {
            return Err(RetirementErrorV2::AdmissionLedgerOutstanding);
        }
        Ok(Self {
            market,
            epoch: window_epoch,
            epoch_generation: epoch.retirement.epoch_generation,
            finalized_slot: window.finalized_slot,
            admitted_count: window.admitted_count,
            closed_node_count: window.closed_node_count,
            best_candidate_node: window.best_candidate_node.bytes(),
            selected_candidate_node: window.selected_candidate_node.bytes(),
        })
    }

    fn binds(self, epoch: LiveGeneralEpochProjectionV2) -> bool {
        self.market == epoch.market
            && self.epoch == epoch.epoch
            && self.epoch_generation == epoch.retirement.epoch_generation
            && self.finalized_slot != 0
            && self.closed_node_count == self.admitted_count
            && self.selected_candidate_node == self.best_candidate_node
    }
}

/// Separately funded mandatory candidate-work Budget root sibling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetRootSiblingV1 {
    /// Claimed Market identity projected from the Budget body.
    pub market: Identity32V1,
    /// Claimed semantic Epoch identity projected from the Budget body.
    pub epoch: Identity32V1,
    /// Claimed parent generation projected from the sibling body.
    pub epoch_generation: u64,
    /// Independent payer/principal/donation owner for full deletion.
    pub rent: DeletableRentOwnerV1,
}

impl EpochBudgetRootSiblingV1 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        self.rent.validate()
    }
}

/// Canonical source identities for the disjoint Epoch/Window/Budget bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootAccountsV1 {
    /// Epoch account and semantic binding for the root being mutated.
    pub epoch: AdapterEpochAccountProjectionV1,
    /// Mandatory Window account deleted at root close.
    pub window: Identity32V1,
    /// Mandatory candidate-work Budget account deleted at root close.
    pub budget: Identity32V1,
}

/// Complete atomic root-bundle plan with disjoint funding compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRetirementPlanV1 {
    /// Epoch tombstone post-state.
    pub epoch_post_state: GeneralEpochLifecycleProjectionV2,
    /// Coalesced payer/sink credits across all three accounts.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// Exact retained Epoch tombstone balance.
    pub epoch_balance_after: u64,
    /// Exact deleted Window balance.
    pub window_balance_after: u64,
    /// Exact deleted Budget balance.
    pub budget_balance_after: u64,
}

/// Complete pure inputs for one Epoch/Window/Budget root retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRetirementRequestV1 {
    /// Live Epoch or already-terminal tombstone.
    pub epoch: GeneralEpochLifecycleProjectionV2,
    /// Mandatory generation-matched Window sibling.
    pub window: EpochWindowRootSiblingV1,
    /// Opaque checked CandidateWindowV4 terminal-ledger capability.
    pub admission_ledger: ValidatedAdmissionLedgerRetiredV1,
    /// Mandatory generation-matched candidate-work Budget sibling.
    pub budget: EpochBudgetRootSiblingV1,
    /// Actual Epoch lamport balance.
    pub epoch_balance: u64,
    /// Actual Window lamport balance.
    pub window_balance: u64,
    /// Actual Budget lamport balance.
    pub budget_balance: u64,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical source account identities.
    pub accounts: EpochRootAccountsV1,
    /// Claimed unique recipient balances that the runtime must authenticate.
    pub recipient_balances: RecipientBalanceBookV1,
}

/// Plan the inseparable terminal Epoch root bundle before any mutation.
///
/// Epoch's live/tombstone split, Window's deletion principal, and Budget's
/// deletion principal remain three independently persisted compartments even
/// when two or more payer identities coincide. Credits are coalesced before
/// checked addition to the supplied recipient-balance projections.
pub fn plan_epoch_root_retirement(
    request: EpochRootRetirementRequestV1,
) -> Result<EpochRootRetirementPlanV1, RetirementErrorV2> {
    let EpochRootRetirementRequestV1 {
        epoch,
        window,
        admission_ledger,
        budget,
        epoch_balance,
        window_balance,
        budget_balance,
        neutral_sink,
        neutral_sink_binding,
        accounts,
        recipient_balances,
    } = request;
    let live_epoch = match epoch {
        GeneralEpochLifecycleProjectionV2::Live(live) => live,
        GeneralEpochLifecycleProjectionV2::Tombstone(_) => {
            return Err(RetirementErrorV2::AlreadyTerminal)
        }
    };
    neutral_sink_binding.require(live_epoch.market, neutral_sink)?;
    if accounts.epoch.market != live_epoch.market
        || accounts.epoch.epoch != live_epoch.epoch
        || accounts.epoch.epoch_index != live_epoch.epoch_index
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    window.validate()?;
    budget.validate()?;
    if window.epoch_generation != live_epoch.retirement.epoch_generation
        || budget.epoch_generation != live_epoch.retirement.epoch_generation
    {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    if window.market != live_epoch.market
        || window.epoch != live_epoch.epoch
        || budget.market != live_epoch.market
        || budget.epoch != live_epoch.epoch
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    if !admission_ledger.binds(live_epoch)
        || admission_ledger.market != window.market
        || admission_ledger.epoch != window.epoch
        || admission_ledger.epoch_generation != window.epoch_generation
    {
        return Err(RetirementErrorV2::AdmissionLedgerOutstanding);
    }
    require_no_source_recipient_alias(
        &[accounts.epoch.account, accounts.window, accounts.budget],
        recipient_balances,
    )?;
    let epoch_plan = plan_epoch_root_only_retirement(epoch, epoch_balance, neutral_sink, 0, 0)?;
    let window_disposition = deletable_rent_disposition(window.rent, window_balance, neutral_sink)?;
    let budget_disposition = deletable_rent_disposition(budget.rent, budget_balance, neutral_sink)?;
    let mut credits = CoalescedRecipientCreditsV1::begin(recipient_balances)?;
    credits.credit(
        recipient_balances,
        epoch_plan.payer,
        live_epoch.retirement.rent.refundable_live_principal,
    )?;
    credits.credit(
        recipient_balances,
        window_disposition.payer,
        window_disposition.payer_refund_lamports,
    )?;
    credits.credit(
        recipient_balances,
        budget_disposition.payer,
        budget_disposition.payer_refund_lamports,
    )?;
    credits.credit(
        recipient_balances,
        neutral_sink,
        epoch_plan.neutral_balance_after,
    )?;
    credits.credit(
        recipient_balances,
        neutral_sink,
        window_disposition.neutral_lamports,
    )?;
    credits.credit(
        recipient_balances,
        neutral_sink,
        budget_disposition.neutral_lamports,
    )?;
    let _non_executable_rent_only_projection = EpochRootRetirementPlanV1 {
        epoch_post_state: epoch_plan.post_state,
        recipient_credits: credits,
        epoch_balance_after: epoch_plan.tombstone_balance_after,
        window_balance_after: 0,
        budget_balance_after: 0,
    };
    Err(RetirementErrorV2::BudgetRetirementUnauthenticated)
}

/// Complete atomic root-retirement inputs after the authoritative Budget
/// semantic owner has supplied its terminal disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRetirementRequestV2 {
    /// Live Epoch; tombstones replay-refuse.
    pub epoch: GeneralEpochLifecycleProjectionV2,
    /// Mandatory generation-matched Window sibling.
    pub window: EpochWindowRootSiblingV1,
    /// Opaque checked CandidateWindow terminal-ledger capability.
    pub admission_ledger: ValidatedAdmissionLedgerRetiredV1,
    /// Owner-qualified terminal Budget disposition.
    pub budget: AuthenticatedEpochBudgetDispositionV1,
    /// Successful signer receiving the still-present root-close reward.
    pub reward_recipient: Identity32V1,
    /// Actual source balances before any mutation.
    pub epoch_balance: u64,
    /// Actual Window balance before any mutation.
    pub window_balance: u64,
    /// Actual Budget balance before any mutation.
    pub budget_balance: u64,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm neutral-sink binding.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical source account identities.
    pub accounts: EpochRootAccountsV1,
    /// Authenticated starting balances for up to five distinct recipients.
    pub recipient_balances: EpochRootRecipientBalanceBookV2,
}

/// Success-capable pure plan for Epoch tombstoning plus Window/Budget deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRootRetirementPlanV2 {
    /// Epoch tombstone post-state.
    pub epoch_post_state: GeneralEpochLifecycleProjectionV2,
    /// Alias-coalesced payer, closer, and sink credits.
    pub recipient_credits: EpochRootRecipientCreditsV2,
    /// Root-close reward kept distinct from rent and surplus.
    pub root_close_reward_lamports: u64,
    /// Exact retained Epoch tombstone balance.
    pub epoch_balance_after: u64,
    /// Deleted Window post-balance.
    pub window_balance_after: u64,
    /// Deleted Budget post-balance.
    pub budget_balance_after: u64,
}

/// Plan the complete terminal root transition without mutating any state.
///
/// This succeeds only with an owner-qualified Budget disposition. The frozen
/// V1 entry point remains an unconditional Budget refusal, preventing old
/// callers from inheriting this capability accidentally.
pub fn plan_epoch_root_retirement_v2(
    request: EpochRootRetirementRequestV2,
) -> Result<EpochRootRetirementPlanV2, RetirementErrorV2> {
    let live_epoch = match request.epoch {
        GeneralEpochLifecycleProjectionV2::Live(live) => live,
        GeneralEpochLifecycleProjectionV2::Tombstone(_) => {
            return Err(RetirementErrorV2::AlreadyTerminal)
        }
    };
    request
        .neutral_sink_binding
        .require(live_epoch.market, request.neutral_sink)?;
    if request.accounts.epoch.market != live_epoch.market
        || request.accounts.epoch.epoch != live_epoch.epoch
        || request.accounts.epoch.epoch_index != live_epoch.epoch_index
        || request.window.market != live_epoch.market
        || request.window.epoch != live_epoch.epoch
        || request.budget.market() != live_epoch.market
        || request.budget.epoch() != live_epoch.epoch
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    request.window.validate()?;
    let generation = live_epoch.retirement.epoch_generation;
    if request.window.epoch_generation != generation
        || request.budget.epoch_generation() != generation
    {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    if request.budget.budget_account() != request.accounts.budget {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    if request.budget.neutral_sink() != request.neutral_sink {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    if !request.admission_ledger.binds(live_epoch)
        || request.admission_ledger.market != request.window.market
        || request.admission_ledger.epoch != request.window.epoch
        || request.admission_ledger.epoch_generation != request.window.epoch_generation
    {
        return Err(RetirementErrorV2::AdmissionLedgerOutstanding);
    }
    if request.reward_recipient == request.neutral_sink {
        return Err(RetirementErrorV2::AccountAlias);
    }
    let sources = [
        request.accounts.epoch.account,
        request.accounts.window,
        request.accounts.budget,
    ];
    let mut left = 0usize;
    while left < sources.len() {
        let mut right = left + 1;
        while right < sources.len() {
            if sources[left] == sources[right] {
                return Err(RetirementErrorV2::AccountAlias);
            }
            right += 1;
        }
        let mut recipient = 0usize;
        while recipient < request.recipient_balances.entries.len() {
            if request.recipient_balances.entries[recipient].map(|entry| entry.recipient)
                == Some(sources[left])
            {
                return Err(RetirementErrorV2::AccountAlias);
            }
            recipient += 1;
        }
        left += 1;
    }
    if request.budget.funding_payer() == request.neutral_sink
        || request.budget.funding_payer() == request.accounts.epoch.account
        || request.budget.funding_payer() == request.accounts.window
        || request.budget.funding_payer() == request.accounts.budget
    {
        return Err(RetirementErrorV2::AccountAlias);
    }

    let epoch_plan = plan_epoch_root_only_retirement(
        request.epoch,
        request.epoch_balance,
        request.neutral_sink,
        0,
        0,
    )?;
    let window = deletable_rent_disposition(
        request.window.rent,
        request.window_balance,
        request.neutral_sink,
    )?;
    let budget_rent = request.budget.rent();
    let budget_principal = budget_rent.refundable_principal();
    let reward = request.budget.root_close_reward();
    let budget_required = budget_principal
        .checked_add(reward)
        .and_then(|value| value.checked_add(budget_rent.donation_floor()))
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if request.budget_balance < budget_required {
        return Err(RetirementErrorV2::AccountBalanceShortfall);
    }
    let budget_neutral = request
        .budget_balance
        .checked_sub(budget_principal)
        .and_then(|value| value.checked_sub(reward))
        .ok_or(RetirementErrorV2::AccountBalanceShortfall)?;

    let book = request.recipient_balances;
    let mut credits = EpochRootRecipientCreditsV2::begin(book)?;
    credits.credit(
        book,
        epoch_plan.payer,
        live_epoch.retirement.rent.refundable_live_principal,
    )?;
    credits.credit(book, window.payer, window.payer_refund_lamports)?;
    credits.credit(book, budget_rent.payer(), budget_principal)?;
    credits.credit(book, request.reward_recipient, reward)?;
    credits.credit(book, request.neutral_sink, epoch_plan.neutral_balance_after)?;
    credits.credit(book, request.neutral_sink, window.neutral_lamports)?;
    credits.credit(book, request.neutral_sink, budget_neutral)?;

    Ok(EpochRootRetirementPlanV2 {
        epoch_post_state: epoch_plan.post_state,
        recipient_credits: credits,
        root_close_reward_lamports: reward,
        epoch_balance_after: epoch_plan.tombstone_balance_after,
        window_balance_after: 0,
        budget_balance_after: 0,
    })
}

/// Frozen count-only Reservation projection used by V5/V6 envelopes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedReservationV1 {
    /// Position generation already persisted by the legacy Reservation body.
    pub position_generation: u64,
    /// Semantic ownership state already persisted by the legacy body.
    pub state: ReservationStateV1,
    /// Epoch generation and once-only Position counter marker.
    pub count: ReservationCountTailV1,
}

impl CountedReservationV1 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        self.count.validate()?;
        if self.state.is_position_counted() != self.count.position_counted {
            return Err(RetirementErrorV1::NonCanonicalState);
        }
        Ok(())
    }
}

/// Register through the frozen count-only direct Reservation V6 API.
///
/// The caller-supplied generation remains part of this compatibility surface;
/// live routing remains disabled. Successor V8 creation must instead use
/// [`register_direct_reservation_v2`] and a Direct Epoch projection.
pub fn register_direct_reservation(
    mut position: LivePositionV2,
    direct_epoch_generation: u64,
) -> Result<(LivePositionV2, CountedReservationV1), RetirementErrorV1> {
    position.validate()?;
    if direct_epoch_generation == 0 {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    position.retirement.outstanding_reservations = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let reservation = CountedReservationV1 {
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: direct_epoch_generation,
            position_counted: true,
        },
    };
    reservation.validate()?;
    Ok((position, reservation))
}

/// Forgeable frozen projection of one counted Epoch child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochChildProjectionV1 {
    /// Parent Epoch generation copied from the child's versioned bytes.
    pub epoch_generation: u64,
    /// Account family whose authoritative count owns this child.
    pub kind: EpochChildKindV1,
    /// Version-qualified candidate state, present only for CandidateBundle.
    pub candidate_status: Option<CandidateStatusWitnessV1>,
}

impl EpochChildProjectionV1 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV1::WrongGeneration);
        }
        let candidate = self.kind == EpochChildKindV1::CandidateBundle;
        if candidate != self.candidate_status.is_some() {
            return Err(RetirementErrorV1::WrongChildKind);
        }
        Ok(())
    }
}

/// Frozen historical name for the forgeable child projection.
///
/// The name is preserved for source compatibility only. Public fields confer
/// no runtime owner, PDA, codec, or account-byte authority.
pub type AuthenticatedEpochChildV1 = EpochChildProjectionV1;

/// Frozen projected presence of one canonical child account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildSlotV1 {
    /// Claimed system-owned zero-data canonical PDA target.
    Absent,
    /// Claimed present, versioned, program-owned child.
    Present(AuthenticatedEpochChildV1),
}

fn require_open_epoch_v1(epoch: LiveEpochV5) -> Result<(), RetirementErrorV1> {
    epoch.validate()?;
    if epoch.phase != crate::GeneralEpochPhaseV1::Open {
        Err(RetirementErrorV1::WrongPhase)
    } else {
        Ok(())
    }
}

fn require_terminal_epoch_v1(epoch: LiveEpochV5) -> Result<(), RetirementErrorV1> {
    epoch.validate()?;
    if epoch.phase == crate::GeneralEpochPhaseV1::Open {
        Err(RetirementErrorV1::WrongPhase)
    } else {
        Ok(())
    }
}

/// Register through the frozen count-only general Reservation V5 API.
pub fn register_general_reservation(
    position: LivePositionV2,
    epoch: LiveEpochV5,
) -> Result<
    (
        LivePositionV2,
        LiveEpochV5,
        CountedReservationV1,
        ChildSlotV1,
    ),
    RetirementErrorV1,
> {
    position.validate()?;
    epoch.validate()?;
    if epoch.phase != crate::GeneralEpochPhaseV1::Open {
        return Err(RetirementErrorV1::WrongPhase);
    }
    let next_position_count = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let next_epoch_counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::ReservationArchive)?;
    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_position_count;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = next_epoch_counts;
    let reservation = CountedReservationV1 {
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: epoch.retirement.epoch_generation,
            position_counted: true,
        },
    };
    reservation.validate()?;
    let archive = ChildSlotV1::Present(EpochChildProjectionV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::ReservationArchive,
        candidate_status: None,
    });
    Ok((next_position, next_epoch, reservation, archive))
}

/// Move frozen count-only Reservation V5/V6 ACTIVE to ENTITLED.
pub fn entitle_reservation(
    reservation: CountedReservationV1,
) -> Result<CountedReservationV1, RetirementErrorV1> {
    reservation.validate()?;
    if reservation.state != ReservationStateV1::Active {
        return Err(if reservation.state.is_terminal() {
            RetirementErrorV1::AlreadyTerminal
        } else {
            RetirementErrorV1::WrongPhase
        });
    }
    let mut next = reservation;
    next.state = ReservationStateV1::Entitled;
    next.validate()?;
    Ok(next)
}

/// Apply the frozen count-only Reservation's first terminal debit.
pub fn terminate_reservation(
    position: LivePositionV2,
    reservation: CountedReservationV1,
    target: ReservationStateV1,
) -> Result<(LivePositionV2, CountedReservationV1), RetirementErrorV1> {
    position.validate()?;
    reservation.validate()?;
    if !target.is_terminal() {
        return Err(RetirementErrorV1::WrongPhase);
    }
    if reservation.state.is_terminal() {
        return Err(RetirementErrorV1::AlreadyTerminal);
    }
    if reservation.position_generation != position.generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    let next_count = position
        .retirement
        .outstanding_reservations
        .checked_sub(1)
        .ok_or(RetirementErrorV1::CounterUnderflow)?;
    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_count;
    let mut next_reservation = reservation;
    next_reservation.state = target;
    next_reservation.count.position_counted = false;
    next_reservation.validate()?;
    Ok((next_position, next_reservation))
}

/// Create one child through the frozen three-phase/count-only API.
pub fn create_epoch_child(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
    kind: EpochChildKindV1,
) -> Result<(LiveEpochV5, ChildSlotV1), RetirementErrorV1> {
    require_open_epoch_v1(epoch)?;
    if matches!(
        kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    if slot != ChildSlotV1::Absent {
        return Err(RetirementErrorV1::ChildAlreadyPresent);
    }
    let counts = epoch.retirement.children.checked_increment(kind)?;
    let child = EpochChildProjectionV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind,
        candidate_status: None,
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Present(child)))
}

/// Create one candidate through the frozen three-phase/count-only API.
pub fn create_registered_candidate_after_validation(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
    status: CandidateStatusWitnessV1,
) -> Result<(LiveEpochV5, ChildSlotV1), RetirementErrorV1> {
    require_open_epoch_v1(epoch)?;
    if slot != ChildSlotV1::Absent {
        return Err(RetirementErrorV1::ChildAlreadyPresent);
    }
    let counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::CandidateBundle)?;
    let child = EpochChildProjectionV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::CandidateBundle,
        candidate_status: Some(status),
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Present(child)))
}

/// Update one frozen candidate projection without changing its count.
pub fn update_registered_candidate_status_after_validation(
    slot: ChildSlotV1,
    status: CandidateStatusWitnessV1,
) -> Result<ChildSlotV1, RetirementErrorV1> {
    let mut child = match slot {
        ChildSlotV1::Present(child) => child,
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
    };
    child.validate()?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    let prior = child
        .candidate_status
        .ok_or(RetirementErrorV1::WrongChildKind)?;
    if status.schema_tag() != prior.schema_tag() {
        return Err(RetirementErrorV1::WrongTag);
    }
    if status.schema_version() != prior.schema_version() {
        return Err(RetirementErrorV1::WrongVersion);
    }
    child.candidate_status = Some(status);
    child.validate()?;
    Ok(ChildSlotV1::Present(child))
}

fn checked_present_v1(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
) -> Result<EpochChildProjectionV1, RetirementErrorV1> {
    let child = match slot {
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
        ChildSlotV1::Present(child) => child,
    };
    child.validate()?;
    if child.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    Ok(child)
}

/// Close one generic child through the frozen count-only API.
pub fn close_epoch_child(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
) -> Result<(LiveEpochV5, ChildSlotV1), RetirementErrorV1> {
    require_terminal_epoch_v1(epoch)?;
    let child = checked_present_v1(epoch, slot)?;
    if matches!(
        child.kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    let counts = epoch.retirement.children.checked_decrement(child.kind)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}

/// Close one candidate through the frozen count-only API.
pub fn close_registered_candidate(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
    canonical_clear_work_present: bool,
) -> Result<(LiveEpochV5, ChildSlotV1), RetirementErrorV1> {
    require_terminal_epoch_v1(epoch)?;
    let child = checked_present_v1(epoch, slot)?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    if canonical_clear_work_present {
        return Err(RetirementErrorV1::ClearWorkOutstanding);
    }
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::CandidateBundle)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}

/// Close one terminal count-only general Reservation archive.
pub fn close_general_reservation_archive(
    epoch: LiveEpochV5,
    slot: ChildSlotV1,
    reservation: CountedReservationV1,
) -> Result<(LiveEpochV5, ChildSlotV1), RetirementErrorV1> {
    reservation.validate()?;
    if !reservation.state.is_terminal() || reservation.count.position_counted {
        return Err(RetirementErrorV1::ReservationOutstanding);
    }
    if reservation.count.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    let child = match slot {
        ChildSlotV1::Present(child) if child.kind == EpochChildKindV1::ReservationArchive => child,
        ChildSlotV1::Present(_) => return Err(RetirementErrorV1::WrongChildKind),
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
    };
    if child.epoch_generation != reservation.count.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    require_terminal_epoch_v1(epoch)?;
    child.validate()?;
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::ReservationArchive)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}

/// Deletable Reservation successor projection spanning V7/V8 bodies and tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedReservationV2 {
    /// Claimed Market identity from the existing Reservation base.
    pub market: Identity32V1,
    /// Claimed parent Epoch identity from the existing Reservation base.
    pub epoch: Identity32V1,
    /// Claimed Position owner from the existing Reservation base.
    pub owner: Identity32V1,
    /// Position generation already persisted by current reservation schemas.
    pub position_generation: u64,
    /// Semantic ownership state already persisted by current reservation schemas.
    pub state: ReservationStateV1,
    /// New epoch-generation and once-only count marker.
    pub count: ReservationCountTailV1,
    /// Embedded exact funding owner for deletion after terminality.
    pub rent: DeletableRentOwnerV1,
}

impl CountedReservationV2 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        self.count.validate()?;
        self.rent.validate()?;
        if self.state.is_position_counted() != self.count.position_counted {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        Ok(())
    }
}

/// Forgeable adapter projection of the direct-Epoch parent identity.
///
/// The checked generation is derived from the projected index and admission is
/// restricted to the exact pre-freeze-open lifecycle. These public fields
/// carry no runtime authority. The adapter must authenticate the exact Direct
/// Epoch codec, owner, PDA, stored bump, and complete lifecycle shape first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterDirectEpochProjectionV1 {
    /// Canonical direct-Epoch PDA identity.
    pub account: Identity32V1,
    /// Claimed Market identity from the direct-Epoch semantic owner.
    pub market: Identity32V1,
    /// Claimed Direct Epoch identity from its exact codec and PDA seeds.
    pub epoch: Identity32V1,
    /// Claimed immutable direct Epoch index from the body.
    pub epoch_index: u64,
    /// Claimed authoritative Direct Epoch lifecycle phase.
    pub lifecycle_phase: DirectEpochLifecyclePhaseV1,
}

/// Exact semantic projection of the six authoritative Direct Epoch V4
/// lifecycle phases.
///
/// This enum carries no account authority by itself. It exists so successor
/// registration cannot discard the admission lifecycle after exact decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectEpochLifecyclePhaseV1 {
    /// Reservations may be admitted before the order pair freezes.
    PrefreezeOpen,
    /// The order pair froze without a competitive-candidate window.
    FrozenEmpty,
    /// Candidate submission is open.
    WindowOpen,
    /// Retained candidates are being verified.
    Verifying,
    /// One candidate has been selected.
    Selected,
    /// The Direct Epoch carries its terminal receipt.
    Terminal,
}

/// Forgeable account projections for general Reservation registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationRegistrationAccountsV1 {
    /// Position PDA and semantic binding.
    pub position: AdapterPositionAccountProjectionV1,
    /// General Epoch PDA and semantic binding.
    pub epoch: AdapterEpochAccountProjectionV1,
    /// Fresh canonical Reservation target PDA.
    pub reservation: Identity32V1,
}

/// Forgeable account projections for direct Reservation registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationRegistrationAccountsV1 {
    /// Position PDA and semantic binding.
    pub position: AdapterPositionAccountProjectionV1,
    /// Fresh canonical Reservation target PDA.
    pub reservation: Identity32V1,
}

impl AdapterDirectEpochProjectionV1 {
    /// Canonical nonzero child generation, refusing an exhausted index.
    pub fn reservation_generation(self) -> Result<u64, RetirementErrorV2> {
        canonical_epoch_generation(self.epoch_index)
    }

    fn require_reservation_admission(self) -> Result<(), RetirementErrorV2> {
        if self.lifecycle_phase != DirectEpochLifecyclePhaseV1::PrefreezeOpen {
            return Err(RetirementErrorV2::WrongPhase);
        }
        Ok(())
    }
}

fn increment_position(mut position: LivePositionV2) -> Result<LivePositionV2, RetirementErrorV2> {
    position.validate()?;
    position.retirement.outstanding_reservations = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    Ok(position)
}

/// Register a direct reservation against Position's aggregate count.
///
/// The direct Epoch adapter separately authenticates and supplies its nonzero
/// generation; its own root accounting remains owned by that direct family.
pub fn register_direct_reservation_v2(
    position: LivePositionV2,
    direct_epoch: AdapterDirectEpochProjectionV1,
    funding: DeletableRentAdmissionPlanV1,
    neutral_sink: Identity32V1,
    neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    accounts: DirectReservationRegistrationAccountsV1,
) -> Result<(LivePositionV2, CountedReservationV2), RetirementErrorV2> {
    direct_epoch.require_reservation_admission()?;
    let direct_epoch_generation = direct_epoch.reservation_generation()?;
    let rent = funding.rent();
    rent.validate()?;
    if direct_epoch.market != position.market {
        return Err(RetirementErrorV2::WrongParent);
    }
    neutral_sink_binding.require(position.market, neutral_sink)?;
    if funding.neutral_sink() != neutral_sink {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    if funding.target() != accounts.reservation {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    if accounts.position.market != position.market || accounts.position.owner != position.owner {
        return Err(RetirementErrorV2::WrongParent);
    }
    let mut payer_debits = CoalescedPayerDebitsV1::empty();
    payer_debits.debit(
        funding.rent().payer(),
        funding.payer_debit_lamports(),
        funding.payer_balance_after(),
    )?;
    require_no_target_payer_alias(
        &[
            accounts.position.account,
            direct_epoch.account,
            accounts.reservation,
        ],
        payer_debits,
        neutral_sink,
    )?;
    let next_position = increment_position(position)?;
    let reservation = CountedReservationV2 {
        market: direct_epoch.market,
        epoch: direct_epoch.epoch,
        owner: position.owner,
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: direct_epoch_generation,
            position_counted: true,
        },
        rent,
    };
    reservation.validate()?;
    Ok((next_position, reservation))
}

/// Register a general reservation in both authoritative aggregates atomically.
pub fn register_general_reservation_v2(
    position: LivePositionV2,
    epoch: LiveGeneralEpochProjectionV2,
    funding: DeletableRentAdmissionPlanV1,
    neutral_sink: Identity32V1,
    neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    accounts: GeneralReservationRegistrationAccountsV1,
) -> Result<
    (
        LivePositionV2,
        LiveGeneralEpochProjectionV2,
        CountedReservationV2,
        CountedEpochChildSlotV2,
    ),
    RetirementErrorV2,
> {
    position.validate()?;
    epoch.validate()?;
    let rent = funding.rent();
    rent.validate()?;
    if epoch.market != position.market {
        return Err(RetirementErrorV2::WrongParent);
    }
    neutral_sink_binding.require(position.market, neutral_sink)?;
    if funding.neutral_sink() != neutral_sink {
        return Err(RetirementErrorV2::WrongNeutralSink);
    }
    if funding.target() != accounts.reservation {
        return Err(RetirementErrorV2::WrongFundingTarget);
    }
    if accounts.position.market != position.market
        || accounts.position.owner != position.owner
        || accounts.epoch.market != epoch.market
        || accounts.epoch.epoch != epoch.epoch
        || accounts.epoch.epoch_index != epoch.epoch_index
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    let mut payer_debits = CoalescedPayerDebitsV1::empty();
    payer_debits.debit(
        funding.rent().payer(),
        funding.payer_debit_lamports(),
        funding.payer_balance_after(),
    )?;
    require_no_target_payer_alias(
        &[
            accounts.position.account,
            accounts.epoch.account,
            accounts.reservation,
        ],
        payer_debits,
        neutral_sink,
    )?;
    if epoch.phase != GeneralEpochPhaseV2::Open {
        return Err(RetirementErrorV2::WrongPhase);
    }
    let next_position_count = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let next_epoch_counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::ReservationArchive)?;

    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_position_count;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = next_epoch_counts;
    let reservation = CountedReservationV2 {
        market: epoch.market,
        epoch: epoch.epoch,
        owner: position.owner,
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: epoch.retirement.epoch_generation,
            position_counted: true,
        },
        rent,
    };
    reservation.validate()?;
    let archive = CountedEpochChildSlotV2::Present(CountedEpochChildProjectionV2 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::ReservationArchive,
        candidate_status: None,
    });
    Ok((next_position, next_epoch, reservation, archive))
}

/// Move ACTIVE to ENTITLED without changing Position's count.
pub fn entitle_reservation_v2(
    reservation: CountedReservationV2,
) -> Result<CountedReservationV2, RetirementErrorV2> {
    reservation.validate()?;
    if reservation.state != ReservationStateV1::Active {
        return Err(if reservation.state.is_terminal() {
            RetirementErrorV2::AlreadyTerminal
        } else {
            RetirementErrorV2::WrongPhase
        });
    }
    let mut next = reservation;
    next.state = ReservationStateV1::Entitled;
    next.validate()?;
    Ok(next)
}

/// Apply the first terminal reservation transition and decrement Position once.
///
/// RELEASED asset return and CONSUMED entitlement/payment equalities remain
/// adapter inputs owned by their existing economic codecs. The adapter must
/// calculate those post-states before encoding this returned accounting state.
pub fn terminate_reservation_v2(
    position: LivePositionV2,
    reservation: CountedReservationV2,
    target: ReservationStateV1,
) -> Result<(LivePositionV2, CountedReservationV2), RetirementErrorV2> {
    position.validate()?;
    reservation.validate()?;
    if !target.is_terminal() {
        return Err(RetirementErrorV2::WrongPhase);
    }
    if reservation.state.is_terminal() {
        return Err(RetirementErrorV2::AlreadyTerminal);
    }
    if reservation.market != position.market || reservation.owner != position.owner {
        return Err(RetirementErrorV2::WrongParent);
    }
    if reservation.position_generation != position.generation {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    let next_count = position
        .retirement
        .outstanding_reservations
        .checked_sub(1)
        .ok_or(RetirementErrorV2::CounterUnderflow)?;
    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_count;
    let mut next_reservation = reservation;
    next_reservation.state = target;
    next_reservation.count.position_counted = false;
    next_reservation.validate()?;
    Ok((next_position, next_reservation))
}

/// Forgeable projection of one counted Epoch child registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedEpochChildProjectionV2 {
    /// Parent Epoch generation copied from the child's versioned bytes.
    pub epoch_generation: u64,
    /// Account family whose authoritative count owns this child.
    pub kind: EpochChildKindV1,
    /// Version-qualified candidate state, present only for CandidateBundle.
    pub candidate_status: Option<CandidateStatusWitnessV1>,
}

impl CountedEpochChildProjectionV2 {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        let candidate = self.kind == EpochChildKindV1::CandidateBundle;
        if candidate != self.candidate_status.is_some() {
            return Err(RetirementErrorV2::WrongChildKind);
        }
        Ok(())
    }
}

/// Projected presence of one canonical child account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CountedEpochChildSlotV2 {
    /// System-owned zero-data canonical PDA target.
    Absent,
    /// Claimed present, versioned, program-owned child.
    Present(CountedEpochChildProjectionV2),
}

fn require_terminal_epoch(epoch: LiveGeneralEpochProjectionV2) -> Result<(), RetirementErrorV2> {
    epoch.validate()?;
    match epoch.phase {
        GeneralEpochPhaseV2::Settled | GeneralEpochPhaseV2::Lapsed => Ok(()),
        GeneralEpochPhaseV2::Open | GeneralEpochPhaseV2::Frozen | GeneralEpochPhaseV2::Cleared => {
            Err(RetirementErrorV2::WrongPhase)
        }
    }
}

fn require_child_creation_phase(
    epoch: LiveGeneralEpochProjectionV2,
    kind: EpochChildKindV1,
) -> Result<(), RetirementErrorV2> {
    epoch.validate()?;
    let required = match kind {
        EpochChildKindV1::OrderPage => GeneralEpochPhaseV2::Open,
        EpochChildKindV1::CandidateBundle
        | EpochChildKindV1::CandidateIndexPage
        | EpochChildKindV1::CandidateVerdict
        | EpochChildKindV1::CandidateEscrow
        | EpochChildKindV1::ClearWorkBundle => GeneralEpochPhaseV2::Frozen,
        EpochChildKindV1::SettlementReceipt | EpochChildKindV1::FinalPot => {
            GeneralEpochPhaseV2::Cleared
        }
        EpochChildKindV1::ReservationArchive => return Err(RetirementErrorV2::WrongChildKind),
    };
    if epoch.phase != required {
        return Err(RetirementErrorV2::WrongPhase);
    }
    Ok(())
}

/// Create one generic Epoch child and increment its typed count once.
///
/// Candidate bundles use [`create_registered_candidate_after_validation_v2`]. Reservation
/// archives must use [`register_general_reservation_v2`] so their Position and
/// Epoch counts cannot diverge.
pub fn create_epoch_child_v2(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
    kind: EpochChildKindV1,
) -> Result<(LiveGeneralEpochProjectionV2, CountedEpochChildSlotV2), RetirementErrorV2> {
    if matches!(
        kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV2::WrongChildKind);
    }
    require_child_creation_phase(epoch, kind)?;
    if slot != CountedEpochChildSlotV2::Absent {
        return Err(RetirementErrorV2::ChildAlreadyPresent);
    }
    let counts = epoch.retirement.children.checked_increment(kind)?;
    let child = CountedEpochChildProjectionV2 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind,
        candidate_status: None,
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, CountedEpochChildSlotV2::Present(child)))
}

/// Create one candidate bundle after its lifecycle owner validates its state.
pub fn create_registered_candidate_after_validation_v2(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
    status: CandidateStatusWitnessV1,
) -> Result<(LiveGeneralEpochProjectionV2, CountedEpochChildSlotV2), RetirementErrorV2> {
    require_child_creation_phase(epoch, EpochChildKindV1::CandidateBundle)?;
    if slot != CountedEpochChildSlotV2::Absent {
        return Err(RetirementErrorV2::ChildAlreadyPresent);
    }
    let counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::CandidateBundle)?;
    let child = CountedEpochChildProjectionV2 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::CandidateBundle,
        candidate_status: Some(status),
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, CountedEpochChildSlotV2::Present(child)))
}

/// Record a lifecycle-validated candidate status without changing its count.
///
/// This crate deliberately does not duplicate the current or ADR-0006
/// candidate state graph. The owning lifecycle adapter first validates that
/// transition, then supplies its version-qualified post-state here. The child
/// must already be a registered candidate in the same occupied slot.
pub fn update_registered_candidate_status_after_validation_v2(
    slot: CountedEpochChildSlotV2,
    status: CandidateStatusWitnessV1,
) -> Result<CountedEpochChildSlotV2, RetirementErrorV2> {
    let mut child = match slot {
        CountedEpochChildSlotV2::Present(child) => child,
        CountedEpochChildSlotV2::Absent => return Err(RetirementErrorV2::ChildAbsent),
    };
    child.validate()?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV2::WrongChildKind);
    }
    let prior = child
        .candidate_status
        .ok_or(RetirementErrorV2::WrongChildKind)?;
    if status.schema_tag() != prior.schema_tag() {
        return Err(RetirementErrorV2::WrongTag);
    }
    if status.schema_version() != prior.schema_version() {
        return Err(RetirementErrorV2::WrongVersion);
    }
    child.candidate_status = Some(status);
    child.validate()?;
    Ok(CountedEpochChildSlotV2::Present(child))
}

fn checked_present_projection(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
) -> Result<CountedEpochChildProjectionV2, RetirementErrorV2> {
    let child = match slot {
        CountedEpochChildSlotV2::Absent => return Err(RetirementErrorV2::ChildAbsent),
        CountedEpochChildSlotV2::Present(child) => child,
    };
    child.validate()?;
    if child.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    Ok(child)
}

/// Close one generic child and decrement its typed count exactly once.
///
/// The adapter first validates the account family's economic close condition.
/// Candidate bundles and reservation archives are deliberately refused here;
/// their specialized functions enforce additional dependencies.
pub fn close_epoch_child_v2(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
) -> Result<(LiveGeneralEpochProjectionV2, CountedEpochChildSlotV2), RetirementErrorV2> {
    require_terminal_epoch(epoch)?;
    let child = checked_present_projection(epoch, slot)?;
    if matches!(
        child.kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV2::WrongChildKind);
    }
    let counts = epoch.retirement.children.checked_decrement(child.kind)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, CountedEpochChildSlotV2::Absent))
}

/// Close one candidate bundle in any status after its canonical ClearWork is absent.
pub fn close_registered_candidate_v2(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
    canonical_clear_work_present: bool,
) -> Result<(LiveGeneralEpochProjectionV2, CountedEpochChildSlotV2), RetirementErrorV2> {
    require_terminal_epoch(epoch)?;
    let child = checked_present_projection(epoch, slot)?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV2::WrongChildKind);
    }
    if canonical_clear_work_present {
        return Err(RetirementErrorV2::ClearWorkOutstanding);
    }
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::CandidateBundle)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, CountedEpochChildSlotV2::Absent))
}

/// Close a terminal, economically uncounted general reservation archive.
///
/// This decrements only the Epoch archive count. The Position count was
/// already decremented by [`terminate_reservation_v2`].
fn close_general_reservation_archive_root(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
    reservation: CountedReservationV2,
) -> Result<(LiveGeneralEpochProjectionV2, CountedEpochChildSlotV2), RetirementErrorV2> {
    reservation.validate()?;
    if !reservation.state.is_terminal() || reservation.count.position_counted {
        return Err(RetirementErrorV2::ReservationOutstanding);
    }
    if reservation.count.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    if reservation.market != epoch.market || reservation.epoch != epoch.epoch {
        return Err(RetirementErrorV2::WrongParent);
    }
    let child = match slot {
        CountedEpochChildSlotV2::Present(child)
            if child.kind == EpochChildKindV1::ReservationArchive =>
        {
            child
        }
        CountedEpochChildSlotV2::Present(_) => return Err(RetirementErrorV2::WrongChildKind),
        CountedEpochChildSlotV2::Absent => return Err(RetirementErrorV2::ChildAbsent),
    };
    if child.epoch_generation != reservation.count.epoch_generation {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    require_terminal_epoch(epoch)?;
    child.validate()?;
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::ReservationArchive)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, CountedEpochChildSlotV2::Absent))
}

/// Complete general Reservation deletion plus Epoch archive-count plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationClosePlanV1 {
    /// Epoch with its reservation-archive count decremented exactly once.
    pub epoch_post_state: LiveGeneralEpochProjectionV2,
    /// Canonical archive absence.
    pub reservation_post_slot: CountedEpochChildSlotV2,
    /// Exact payer/sink balances and zero Reservation balance.
    pub rent_close: DeletableAccountClosePlanV1,
}

/// Complete pure inputs for general Reservation archive deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationCloseRequestV1 {
    /// Terminal parent Epoch.
    pub epoch: LiveGeneralEpochProjectionV2,
    /// Writable Epoch PDA and forgeable semantic projection.
    pub epoch_account: AdapterEpochAccountProjectionV1,
    /// Claimed present reservation archive slot.
    pub slot: CountedEpochChildSlotV2,
    /// Terminal economically uncounted Reservation projection.
    pub reservation: CountedReservationV2,
    /// Actual Reservation lamport balance.
    pub reservation_balance: u64,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical Reservation source account identity.
    pub reservation_account: Identity32V1,
    /// Claimed unique recipient balances that the runtime must authenticate.
    pub recipient_balances: RecipientBalanceBookV1,
}

/// Plan a terminal general Reservation archive deletion before any mutation.
pub fn plan_general_reservation_close(
    request: GeneralReservationCloseRequestV1,
) -> Result<GeneralReservationClosePlanV1, RetirementErrorV2> {
    let GeneralReservationCloseRequestV1 {
        epoch,
        epoch_account,
        slot,
        reservation,
        reservation_balance,
        neutral_sink,
        neutral_sink_binding,
        reservation_account,
        recipient_balances,
    } = request;
    if epoch_account.market != epoch.market
        || epoch_account.epoch != epoch.epoch
        || epoch_account.epoch_index != epoch.epoch_index
    {
        return Err(RetirementErrorV2::WrongParent);
    }
    neutral_sink_binding.require(epoch.market, neutral_sink)?;
    let rent_close = plan_deletable_account_close(
        reservation.rent,
        reservation_balance,
        neutral_sink,
        &[epoch_account.account, reservation_account],
        recipient_balances,
    )?;
    let (epoch_post_state, reservation_post_slot) =
        close_general_reservation_archive_root(epoch, slot, reservation)?;
    Ok(GeneralReservationClosePlanV1 {
        epoch_post_state,
        reservation_post_slot,
        rent_close,
    })
}

/// Complete direct Reservation deletion plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationClosePlanV1 {
    /// Canonical absence after deletion.
    pub reservation_present_after: bool,
    /// Exact payer/sink balances and zero Reservation balance.
    pub rent_close: DeletableAccountClosePlanV1,
}

/// Complete pure inputs for direct Reservation deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationCloseRequestV1 {
    /// Forgeable direct parent projection.
    pub direct_epoch: AdapterDirectEpochProjectionV1,
    /// Terminal economically uncounted Reservation projection.
    pub reservation: CountedReservationV2,
    /// Actual Reservation lamport balance.
    pub reservation_balance: u64,
    /// Frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Immutable Market/Realm binding for the supplied neutral sink.
    pub neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1,
    /// Canonical Reservation source account identity.
    pub reservation_account: Identity32V1,
    /// Claimed unique recipient balances that the runtime must authenticate.
    pub recipient_balances: RecipientBalanceBookV1,
}

/// Plan deletion of a terminal direct Reservation cross-bound to the supplied
/// direct Epoch projection's checked generation.
pub fn plan_direct_reservation_close(
    request: DirectReservationCloseRequestV1,
) -> Result<DirectReservationClosePlanV1, RetirementErrorV2> {
    let DirectReservationCloseRequestV1 {
        direct_epoch,
        reservation,
        reservation_balance,
        neutral_sink,
        neutral_sink_binding,
        reservation_account,
        recipient_balances,
    } = request;
    reservation.validate()?;
    neutral_sink_binding.require(direct_epoch.market, neutral_sink)?;
    if !reservation.state.is_terminal() || reservation.count.position_counted {
        return Err(RetirementErrorV2::ReservationOutstanding);
    }
    if reservation.count.epoch_generation != direct_epoch.reservation_generation()? {
        return Err(RetirementErrorV2::WrongGeneration);
    }
    if reservation.market != direct_epoch.market || reservation.epoch != direct_epoch.epoch {
        return Err(RetirementErrorV2::WrongParent);
    }
    let rent_close = plan_deletable_account_close(
        reservation.rent,
        reservation_balance,
        neutral_sink,
        &[reservation_account],
        recipient_balances,
    )?;
    Ok(DirectReservationClosePlanV1 {
        reservation_present_after: false,
        rent_close,
    })
}
