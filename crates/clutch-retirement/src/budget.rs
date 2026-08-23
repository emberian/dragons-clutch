// SPDX-License-Identifier: AGPL-3.0-or-later

//! Owner-qualified terminal disposition for the General V2 Epoch Budget.
//!
//! The authoritative Budget codec and reward-state machine live outside this
//! crate.  This module deliberately does not reproduce either one.  Its only
//! input capability is minted after that semantic owner has authenticated the
//! exact account and established that every earlier reward and selected-rent
//! obligation is terminal.  Retirement then owns the final, atomic split:
//! the root-close reward goes to the successful closer, rent principal goes
//! to its recorded payer, and donation plus later surplus goes to the frozen
//! neutral sink.

use crate::{
    CoalescedRecipientCreditsV1, DeletableRentOwnerV1, Identity32V1, RecipientBalanceBookV1,
    RecipientCreditV1, RetirementErrorV2, MAX_RETIREMENT_RECIPIENTS,
};

/// Private-field capability produced from the authoritative Budget owner.
///
/// This type does not authenticate Solana bytes, their owner, or their PDA.
/// A live adapter may call [`Self::after_semantic_owner_validation`] only with
/// fields returned by the exact Budget semantic owner after it has checked:
///
/// - freeze, finalize, solver, and selected-artifact-rent obligations are
///   terminal and have no remaining spendable balance;
/// - the root-close reward remains present and equals `root_close_reward`;
/// - `funding_payer` owns root-reward/selected-rent capitalization while
///   `rent.payer()` independently owns the Budget account rent principal; and
/// - Market, Epoch, generation, sink, bump, account bytes, program owner, and
///   PDA are authenticated rather than caller-asserted.
///
/// Private fields prevent later code from manufacturing the capability by
/// struct literal and make the semantic-owner handoff one reviewable seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEpochBudgetDispositionV1 {
    budget_account: Identity32V1,
    market: Identity32V1,
    epoch: Identity32V1,
    epoch_generation: u64,
    neutral_sink: Identity32V1,
    funding_payer: Identity32V1,
    rent: DeletableRentOwnerV1,
    root_close_reward: u64,
}

impl AuthenticatedEpochBudgetDispositionV1 {
    /// Mint the retirement capability after authoritative semantic validation.
    ///
    /// This constructor rechecks all cross-owner facts needed by retirement;
    /// it intentionally does not accept Budget lifecycle markers or a second
    /// persisted Budget DTO.  The semantic owner remains solely responsible
    /// for interpreting those markers.
    #[allow(clippy::too_many_arguments)]
    pub fn after_semantic_owner_validation(
        budget_account: Identity32V1,
        market: Identity32V1,
        epoch: Identity32V1,
        epoch_generation: u64,
        neutral_sink: Identity32V1,
        funding_payer: Identity32V1,
        rent: DeletableRentOwnerV1,
        root_close_reward: u64,
    ) -> Result<Self, RetirementErrorV2> {
        rent.validate()?;
        if epoch_generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        if root_close_reward == 0 {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        if funding_payer == neutral_sink || rent.payer() == neutral_sink {
            return Err(RetirementErrorV2::PayerIsNeutralSink);
        }
        if budget_account == market
            || budget_account == epoch
            || budget_account == funding_payer
            || budget_account == rent.payer()
            || budget_account == neutral_sink
        {
            return Err(RetirementErrorV2::AccountAlias);
        }
        rent.refundable_principal()
            .checked_add(rent.donation_floor())
            .and_then(|value| value.checked_add(root_close_reward))
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        Ok(Self {
            budget_account,
            market,
            epoch,
            epoch_generation,
            neutral_sink,
            funding_payer,
            rent,
            root_close_reward,
        })
    }

    /// Exact authenticated Budget account slated for deletion.
    pub const fn budget_account(self) -> Identity32V1 {
        self.budget_account
    }

    /// Parent Market authenticated by the Budget semantic owner and PDA.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Parent Epoch authenticated by the Budget semantic owner and PDA.
    pub const fn epoch(self) -> Identity32V1 {
        self.epoch
    }

    /// Exact counted parent generation.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }

    /// Immutable neutral sink selected by the Market/Realm binding.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    /// Payer that capitalized root rewards and selected-artifact rent.
    pub const fn funding_payer(self) -> Identity32V1 {
        self.funding_payer
    }

    /// Exact persisted rent-principal and donation-floor compartment.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Still-present reward earned by the successful atomic root closer.
    pub const fn root_close_reward(self) -> u64 {
        self.root_close_reward
    }
}

/// Complete no-write input for deleting one authenticated terminal Budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetRetirementRequestV1 {
    /// Owner-qualified terminal Budget disposition.
    pub disposition: AuthenticatedEpochBudgetDispositionV1,
    /// Signer that earns the still-present root-close reward.
    pub reward_recipient: Identity32V1,
    /// Actual live Budget balance before any mutation.
    pub budget_balance: u64,
    /// Authenticated starting balances for every distinct recipient.
    pub recipient_balances: RecipientBalanceBookV1,
}

/// Atomic close plan for the Budget member of an Epoch root bundle.
///
/// The containing root planner must combine this with the Epoch tombstone and
/// Window deletion plans before the adapter performs its first mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetRetirementPlanV1 {
    /// Exact source account deleted by the plan.
    pub budget_account: Identity32V1,
    /// Closer that earns the root-close reward.
    pub reward_recipient: Identity32V1,
    /// Exact earned reward, kept distinct from rent principal.
    pub root_close_reward_lamports: u64,
    /// Exact recorded rent payer.
    pub rent_payer: Identity32V1,
    /// Exact principal refund, never reward or surplus.
    pub rent_principal_refund_lamports: u64,
    /// Exact frozen neutral sink.
    pub neutral_sink: Identity32V1,
    /// Donation floor plus every later unsolicited lamport.
    pub neutral_lamports: u64,
    /// Alias-coalesced, overflow-checked recipient balances.
    pub recipient_credits: CoalescedRecipientCreditsV1,
    /// A successfully deleted Budget retains no lamports.
    pub budget_balance_after: u64,
}

fn begin_credits(
    book: RecipientBalanceBookV1,
) -> Result<CoalescedRecipientCreditsV1, RetirementErrorV2> {
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
    Ok(CoalescedRecipientCreditsV1 { entries })
}

fn credit(
    credits: &mut CoalescedRecipientCreditsV1,
    book: RecipientBalanceBookV1,
    recipient: Identity32V1,
    amount: u64,
) -> Result<(), RetirementErrorV2> {
    let mut index = 0usize;
    while index < book.entries.len() {
        if book.entries[index].map(|entry| entry.recipient) == Some(recipient) {
            let balance_before = book.entries[index]
                .ok_or(RetirementErrorV2::MissingRecipient)?
                .balance_before;
            let mut entry = credits.entries[index].ok_or(RetirementErrorV2::MissingRecipient)?;
            entry.credit_lamports = entry
                .credit_lamports
                .checked_add(amount)
                .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
            entry.balance_after = balance_before
                .checked_add(entry.credit_lamports)
                .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
            credits.entries[index] = Some(entry);
            return Ok(());
        }
        index += 1;
    }
    Err(RetirementErrorV2::MissingRecipient)
}

/// Precompute the terminal Budget deletion and all three economic credits.
///
/// The root-close reward may coalesce with the rent payer when the successful
/// closer is also the funding payer.  It may not alias the neutral sink or the
/// source Budget.  Every checked balance and the zero source post-balance are
/// returned before any runtime write is authorized.
pub fn plan_epoch_budget_retirement(
    request: EpochBudgetRetirementRequestV1,
) -> Result<EpochBudgetRetirementPlanV1, RetirementErrorV2> {
    let disposition = request.disposition;
    if request.reward_recipient == disposition.budget_account
        || request.reward_recipient == disposition.neutral_sink
    {
        return Err(RetirementErrorV2::AccountAlias);
    }
    request.recipient_balances.validate()?;
    let mut index = 0usize;
    while index < request.recipient_balances.entries.len() {
        if request.recipient_balances.entries[index].map(|entry| entry.recipient)
            == Some(disposition.budget_account)
        {
            return Err(RetirementErrorV2::AccountAlias);
        }
        index += 1;
    }

    let principal = disposition.rent.refundable_principal();
    let required = principal
        .checked_add(disposition.root_close_reward)
        .and_then(|value| value.checked_add(disposition.rent.donation_floor()))
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if request.budget_balance < required {
        return Err(RetirementErrorV2::AccountBalanceShortfall);
    }
    let neutral_lamports = request
        .budget_balance
        .checked_sub(principal)
        .and_then(|value| value.checked_sub(disposition.root_close_reward))
        .ok_or(RetirementErrorV2::AccountBalanceShortfall)?;

    let mut recipient_credits = begin_credits(request.recipient_balances)?;
    credit(
        &mut recipient_credits,
        request.recipient_balances,
        disposition.rent.payer(),
        principal,
    )?;
    credit(
        &mut recipient_credits,
        request.recipient_balances,
        request.reward_recipient,
        disposition.root_close_reward,
    )?;
    credit(
        &mut recipient_credits,
        request.recipient_balances,
        disposition.neutral_sink,
        neutral_lamports,
    )?;

    Ok(EpochBudgetRetirementPlanV1 {
        budget_account: disposition.budget_account,
        reward_recipient: request.reward_recipient,
        root_close_reward_lamports: disposition.root_close_reward,
        rent_payer: disposition.rent.payer(),
        rent_principal_refund_lamports: principal,
        neutral_sink: disposition.neutral_sink,
        neutral_lamports,
        recipient_credits,
        budget_balance_after: 0,
    })
}
