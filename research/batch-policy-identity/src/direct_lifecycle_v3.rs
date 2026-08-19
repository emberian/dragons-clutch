//! Executable authority and conservation model for bounded direct selection V3.
//!
//! V3 starts atomically with a successful frozen two-order epoch. Full
//! candidate verification is staged, live Candidate accounts are bounded to
//! three, every transient account records its rent payer and exact payer-funded
//! principal, unsolicited lamports go to an immutable neutral sink, and every
//! frozen phase has a permissionless terminal path.

use clutch_batch::relation_v1::MAX_OUTCOMES;
use clutch_liveness::{DonationLedger, Id as LivenessId};
use sha2::{Digest, Sha256};

use crate::{
    direct_window_v1::{
        DirectCandidateEntryV1, DirectCandidateV2, DirectTwoOrderInputV1, DirectWindowErrorV1,
        ValidatedDirectDomainV1, DIRECT_CANDIDATE_STATUS_VERIFIED, MAX_DIRECT_CANDIDATES,
    },
    FullScoreV1, Identity32V1,
};

/// Maximum frozen ticks admitted by the replay bitmap.
pub const MAX_DIRECT_TICKS_V3: u8 = 64;
/// Minimum half-open submission span in slots.
pub const MIN_SUBMISSION_SPAN_V3: u64 = 2;
/// Minimum selection span: Begin + three Verify + Finalize slot opportunities.
pub const MIN_SELECTION_SPAN_V3: u64 = 5;
/// Minimum post-selection settlement span in slots.
pub const MIN_SETTLEMENT_SPAN_V3: u64 = 2;
/// Maximum research-policy submission span (roughly one day at 400 ms/slot).
pub const MAX_SUBMISSION_SPAN_V3: u64 = 216_000;
/// Maximum research-policy staged-selection span.
pub const MAX_SELECTION_SPAN_V3: u64 = 21_600;
/// Maximum research-policy post-selection span.
pub const MAX_SETTLEMENT_SPAN_V3: u64 = 216_000;

const TRANSCRIPT_DOMAIN_V3: &[u8] = b"dragons-clutch/direct-lifecycle-v3/admission\0";

/// Lifecycle phase of one successfully frozen direct epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLifecyclePhaseV3 {
    /// Frozen reservations exist, but no competitive Candidate exists.
    FrozenEmpty,
    /// A bounded Window accepts competitive submissions.
    WindowOpen,
    /// The exact retained prefix is being re-executed across transactions.
    Verifying,
    /// Candidate zero is selected and reservations are ENTITLED.
    Selected,
    /// Transient authority is closed; the Epoch receipt remains.
    Terminal,
}

/// Candidate account status owned by the staged V3 lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCandidateStageV3 {
    /// Submit performed the complete relation verification.
    Verified,
    /// A later staged Verify transaction independently re-executed it.
    Reverified,
    /// The exact winner.
    Selected,
}

/// Durable terminal reason stored in the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectTerminalReasonV3 {
    /// No competitive Candidate existed at the selection deadline.
    EmptyLapse,
    /// A non-empty Window was not selected before its deadline.
    PreSelectionLapse,
    /// Selected authority was not settled before its deadline.
    PostSelectionLapse,
    /// The selected pair settled atomically.
    Settled,
}

/// Reservation ownership transition authorized by one lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReservationTransitionV3 {
    /// No reservation mutation.
    None,
    /// Selection freezes both ACTIVE envelopes as settlement entitlements.
    ActiveToEntitled,
    /// Settlement consumes both ENTITLED envelopes.
    EntitledToConsumed,
    /// Empty or pre-selection lapse returns both ACTIVE envelopes.
    ActiveToReleased,
    /// Post-selection lapse returns both ENTITLED envelopes.
    EntitledToReleased,
}

/// Reservation state tracked by the executable lifecycle model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReservationStateV3 {
    /// Both exact reservations are ACTIVE.
    Active,
    /// Both exact reservations are ENTITLED to the selected slice.
    Entitled,
    /// Both exact reservations reached CONSUMED or RELEASED terminality.
    Terminal,
}

/// Explicit outcome of a valid submission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAdmissionDispositionV3 {
    /// First competitive Candidate created Candidate and Window authority.
    First,
    /// Candidate joined a not-yet-full retained prefix.
    Retained,
    /// Candidate displaced and closed the former worst.
    Replaced,
    /// Valid Candidate could not beat a full top three; no authority was made.
    RejectedNonCompetitive,
}

/// Refusal from the bounded lifecycle model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLifecycleErrorV3 {
    /// A zero identity, zero amount, malformed padding, or invalid phase shape.
    NonCanonical,
    /// The requested transition is unavailable in the current phase.
    WrongPhase,
    /// Submission lies outside the immutable half-open window.
    SubmissionClosed,
    /// Begin, Verify, or Finalize lies outside the selection window.
    SelectionWindow,
    /// Settlement lies before selection or at/after its immutable deadline.
    SettlementClosed,
    /// The canonical price tick was already competitively admitted.
    Replay,
    /// A verification bit was replayed or the exact mask is incomplete.
    VerificationIncomplete,
    /// An identity, candidate, balance, status, or receipt does not bind.
    MismatchedBinding,
    /// The prepaid work budget cannot cover its frozen obligation.
    WorkBudgetInsufficient,
    /// A bounded counter or lamport sum overflowed.
    ArithmeticOverflow,
    /// Full direct relation or frozen-grid verification refused.
    RelationRefused,
    /// Canonical liveness donation accounting refused.
    LivenessRefused,
}

impl From<DirectWindowErrorV1> for DirectLifecycleErrorV3 {
    fn from(_: DirectWindowErrorV1) -> Self {
        Self::RelationRefused
    }
}

impl From<clutch_liveness::Error> for DirectLifecycleErrorV3 {
    fn from(_: clutch_liveness::Error) -> Self {
        Self::LivenessRefused
    }
}

/// Immutable lifecycle deadlines supplied by the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectScheduleV3 {
    /// First accepted submission slot.
    pub submission_opens_slot: u64,
    /// Exclusive submission close and first selection slot.
    pub submission_closes_slot: u64,
    /// Exclusive selection deadline and first unselected lapse slot.
    pub selection_deadline_slot: u64,
    /// Exclusive settlement deadline and first selected lapse slot.
    pub settlement_deadline_slot: u64,
}

impl DirectScheduleV3 {
    /// Validate ordering plus versioned minimum and maximum horizons.
    pub fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        let submission = self
            .submission_closes_slot
            .checked_sub(self.submission_opens_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        let selection = self
            .selection_deadline_slot
            .checked_sub(self.submission_closes_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        let settlement = self
            .settlement_deadline_slot
            .checked_sub(self.selection_deadline_slot)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        if !(MIN_SUBMISSION_SPAN_V3..=MAX_SUBMISSION_SPAN_V3).contains(&submission)
            || !(MIN_SELECTION_SPAN_V3..=MAX_SELECTION_SPAN_V3).contains(&selection)
            || !(MIN_SETTLEMENT_SPAN_V3..=MAX_SETTLEMENT_SPAN_V3).contains(&settlement)
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Frozen keeper rewards paid from WorkBudget, never from rent principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectKeeperRewardsV3 {
    /// Reward for closing submissions.
    pub begin_verification: u64,
    /// Reward for each successful retained-candidate re-execution.
    pub verify_candidate: u64,
    /// Reward for final selection and entitlement creation.
    pub finalize_selection: u64,
    /// Reward for completed atomic settlement.
    pub settle: u64,
    /// Reward for any permissionless lapse transition.
    pub lapse: u64,
}

impl DirectKeeperRewardsV3 {
    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.begin_verification == 0
            || self.verify_candidate == 0
            || self.finalize_selection == 0
            || self.settle == 0
            || self.lapse == 0
        {
            return Err(DirectLifecycleErrorV3::WorkBudgetInsufficient);
        }
        self.worst_case().map(|_| ())
    }

    fn worst_case(self) -> Result<u64, DirectLifecycleErrorV3> {
        let verification = self
            .verify_candidate
            .checked_mul(MAX_DIRECT_CANDIDATES as u64)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let selected_path = self
            .begin_verification
            .checked_add(verification)
            .and_then(|value| value.checked_add(self.finalize_selection))
            .and_then(|value| value.checked_add(self.settle.max(self.lapse)))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(selected_path.max(self.lapse))
    }
}

/// Exact fixed grid view authenticated by the live PriceGrid account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGridV3 {
    /// Exact simplex scale.
    pub price_scale: u64,
    /// Active prefix in `ticks`.
    pub tick_count: u8,
    /// Strictly increasing active ticks and canonical zero padding.
    pub ticks: [u64; MAX_DIRECT_TICKS_V3 as usize],
}

impl DirectGridV3 {
    /// Validate the bounded exact-membership grid.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        if self.price_scale == 0 || self.tick_count < 2 || self.tick_count > MAX_DIRECT_TICKS_V3 {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < self.ticks.len() {
            if index < usize::from(self.tick_count) {
                if self.ticks[index] == 0
                    || self.ticks[index] >= self.price_scale
                    || (index > 0 && self.ticks[index] <= self.ticks[index - 1])
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
            } else if self.ticks[index] != 0 {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        Ok(())
    }

    fn tick_of(&self, price: u64) -> Result<u8, DirectLifecycleErrorV3> {
        self.validate()?;
        let mut index = 0usize;
        while index < usize::from(self.tick_count) {
            if self.ticks[index] == price {
                return u8::try_from(index).map_err(|_| DirectLifecycleErrorV3::ArithmeticOverflow);
            }
            index += 1;
        }
        Err(DirectLifecycleErrorV3::RelationRefused)
    }
}

/// Exact payer-funded rent principal for one transient account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRentPrincipalV3 {
    /// Authenticated payer receiving only its exact contribution.
    pub payer: Identity32V1,
    /// Exact lamports debited from that payer after prefund normalization.
    pub lamports: u64,
}

impl DirectRentPrincipalV3 {
    /// Canonical absent account.
    pub const ZERO: Self = Self {
        payer: Identity32V1::ZERO,
        lamports: 0,
    };

    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.payer.is_zero() || self.lamports == 0 {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Canonical liveness ledger plus the separately owned payer rent principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAccountLedgerV3 {
    /// Exact payer-funded principal. Donations never reduce this amount.
    pub rent: DirectRentPrincipalV3,
    /// Canonical semantic owner for prebalance and later unsolicited lamports.
    pub donation: Option<DonationLedger>,
}

impl DirectAccountLedgerV3 {
    /// Canonical absent account.
    pub const ZERO: Self = Self {
        rent: DirectRentPrincipalV3::ZERO,
        donation: None,
    };

    /// Reconstruct the canonical semantic owner from hostile persisted fields.
    ///
    /// No new payer transfer occurs while decoding, so the exact delta is zero;
    /// the stored lower bound remains entirely neutral donation.
    pub fn restore(
        rent: DirectRentPrincipalV3,
        sink: Identity32V1,
        donation_lamports: u64,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        rent.validate()?;
        let donation = DonationLedger::admit_prefunded(
            LivenessId::from_bytes(rent.payer.0),
            LivenessId::from_bytes(sink.0),
            donation_lamports,
            0,
            donation_lamports,
        )?;
        Ok(Self {
            rent,
            donation: Some(donation),
        })
    }

    fn validate(self, sink: Identity32V1) -> Result<(), DirectLifecycleErrorV3> {
        self.rent.validate()?;
        let donation = self.donation.ok_or(DirectLifecycleErrorV3::NonCanonical)?;
        if sink.is_zero() || self.rent.payer == sink || donation.neutral_sink().bytes() != sink.0 {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    /// Donation amount last authenticated by the canonical liveness kernel.
    pub fn donation_lamports(self) -> Result<u64, DirectLifecycleErrorV3> {
        self.donation
            .map(DonationLedger::donation_lamports)
            .ok_or(DirectLifecycleErrorV3::NonCanonical)
    }
}

/// One exact create/allocate/assign request over a predictable PDA target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCreationFundingV3 {
    /// Exact payer principal deposited without crediting the prior balance.
    pub rent: DirectRentPrincipalV3,
    /// Target balance before the exact payer deposit.
    pub balance_before: u64,
    /// Target balance after the exact payer deposit.
    pub balance_after: u64,
}

impl DirectCreationFundingV3 {
    /// Canonical absent creation.
    pub const ZERO: Self = Self {
        rent: DirectRentPrincipalV3::ZERO,
        balance_before: 0,
        balance_after: 0,
    };

    fn account(
        self,
        sink: Identity32V1,
        extra_accounted_lamports: u64,
    ) -> Result<DirectAccountLedgerV3, DirectLifecycleErrorV3> {
        self.rent.validate()?;
        let exact_deposit = self
            .rent
            .lamports
            .checked_add(extra_accounted_lamports)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let donation = DonationLedger::admit_prefunded(
            LivenessId::from_bytes(self.rent.payer.0),
            LivenessId::from_bytes(sink.0),
            self.balance_before,
            exact_deposit,
            self.balance_after,
        )?;
        Ok(DirectAccountLedgerV3 {
            rent: self.rent,
            donation: Some(donation),
        })
    }
}

/// One Candidate created only from the complete direct verifier and exact grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateLeaseV3 {
    /// Byte-exact V2 Candidate body; V3 adds rent fields around this body.
    pub candidate: DirectCandidateV2,
    /// Canonical grid tick of the selected outcome price.
    pub tick: u8,
    /// Staged status stored in the V3 Candidate status byte.
    pub stage: DirectCandidateStageV3,
    /// Exact rent principal plus canonical monotone donation ownership.
    pub account: DirectAccountLedgerV3,
}

impl DirectCandidateLeaseV3 {
    /// Canonical inactive slot.
    pub const ZERO: Self = Self {
        candidate: DirectCandidateV2 {
            candidate_id: Identity32V1::ZERO,
            epoch_id: Identity32V1::ZERO,
            market_id: Identity32V1::ZERO,
            order_set_id: Identity32V1::ZERO,
            policy_id: Identity32V1::ZERO,
            relation_domain_digest: Identity32V1::ZERO,
            relation_candidate_digest: Identity32V1::ZERO,
            prices: [0; MAX_OUTCOMES],
            fills: [0; 2],
            weighted_direct_volume: 0,
            limit_surplus_price_units: 0,
            submitted_slot: 0,
            quantity: 0,
            buy_index: 0,
            sell_index: 0,
            outcome: 0,
            distinct_owners: 0,
            order_len: 0,
            outcome_count: 0,
            status: 0,
            stored_bump: 0,
            flags: 0,
            reserved: [0; 12],
        },
        tick: 0,
        stage: DirectCandidateStageV3::Verified,
        account: DirectAccountLedgerV3::ZERO,
    };

    /// Stage an admission lease only after byte-exact relation reexecution.
    ///
    /// This pure constructor proves the proposed create delta and DonationLedger;
    /// it does not debit the payer or create an account. `admit` decides whether
    /// the staged create executes. A `RejectedNonCompetitive` result executes
    /// neither the create nor the payer deposit.
    pub fn issue(
        domain: &ValidatedDirectDomainV1,
        grid: &DirectGridV3,
        candidate: DirectCandidateV2,
        input: DirectTwoOrderInputV1,
        creation: DirectCreationFundingV3,
        neutral_sink: Identity32V1,
    ) -> Result<Self, DirectLifecycleErrorV3> {
        candidate.validate()?;
        domain.reverify_decoded_candidate(&candidate, input)?;
        if grid.price_scale
            != candidate.prices[0]
                .checked_add(candidate.prices[1])
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        grid.tick_of(candidate.prices[0])?;
        grid.tick_of(candidate.prices[1])?;
        let tick = grid.tick_of(candidate.prices[usize::from(candidate.outcome)])?;
        let account = creation.account(neutral_sink, 0)?;
        Ok(Self {
            candidate,
            tick,
            stage: DirectCandidateStageV3::Verified,
            account,
        })
    }

    fn validate_body(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.candidate.validate()?;
        if self.candidate.status != DIRECT_CANDIDATE_STATUS_VERIFIED
            || self.tick >= MAX_DIRECT_TICKS_V3
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }

    fn entry(&self) -> DirectCandidateEntryV1 {
        self.candidate.entry()
    }

    fn score(&self) -> FullScoreV1 {
        self.candidate.score()
    }
}

/// Immutable identities every staged transaction must authenticate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleAuthorityV3 {
    /// Frozen verifier semantics/build identity from BatchPolicy/manifest.
    pub verifier_code_identity: Identity32V1,
    /// Realm-authenticated destination for every unsolicited lamport.
    pub neutral_lamport_sink: Identity32V1,
}

impl DirectLifecycleAuthorityV3 {
    fn validate(self) -> Result<(), DirectLifecycleErrorV3> {
        if self.verifier_code_identity.is_zero() || self.neutral_lamport_sink.is_zero() {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        Ok(())
    }
}

/// Slot and verifier identity supplied to a semantics-bearing transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTransitionContextV3 {
    /// Authenticated Clock slot.
    pub now: u64,
    /// Exact frozen verifier identity checked across all staged transactions.
    pub verifier_code_identity: Identity32V1,
}

/// WorkBudget account funding, kept separate from spendable rewards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectWorkBudgetFundingV3 {
    /// Reward sponsor receiving unused spendable rewards.
    pub reward_sponsor: Identity32V1,
    /// Exact create transition over the target's prior donation balance.
    pub creation: DirectCreationFundingV3,
    /// Exact reward-only lamports deposited by the sponsor.
    pub reward_lamports: u64,
}

/// Funding supplied atomically with successful FreezeDirectEpochV4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFrozenFundingV3 {
    /// WorkBudget rent and spendable rewards.
    pub work_budget: DirectWorkBudgetFundingV3,
    /// Exact liveness ledgers already persisted by two Reservation V2 accounts.
    pub reservation_accounts: [DirectAccountLedgerV3; 2],
}

/// Funding for receipt and pot creation at final selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionFundingV3 {
    /// Exact receipt creation funding.
    pub receipt: DirectCreationFundingV3,
    /// Exact pot creation funding.
    pub pot: DirectCreationFundingV3,
}

/// Live balances observed for accounts a transition closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectObservedBalancesV3 {
    /// Candidate balances in retained slot order.
    pub candidates: [u64; MAX_DIRECT_CANDIDATES],
    /// Window live balance.
    pub window: u64,
    /// Receipt live balance.
    pub receipt: u64,
    /// Pot live balance.
    pub pot: u64,
    /// WorkBudget balance before this transition's keeper reward.
    pub work_budget: u64,
    /// Exact two Reservation live balances.
    pub reservations: [u64; 2],
}

impl DirectObservedBalancesV3 {
    /// Canonical no-close observation.
    pub const ZERO: Self = Self {
        candidates: [0; MAX_DIRECT_CANDIDATES],
        window: 0,
        receipt: 0,
        pot: 0,
        work_budget: 0,
        reservations: [0; 2],
    };
}

/// Closed transient account kind, retained for conservation diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClosedAuthorityV3 {
    /// Candidate at retained index.
    Candidate(u8),
    /// Candidate Window.
    Window,
    /// Settlement receipt.
    Receipt,
    /// Settlement pot.
    Pot,
    /// Reward WorkBudget account.
    WorkBudget,
    /// Reservation at book index.
    Reservation(u8),
}

/// Exact split of one closed account balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseLegV3 {
    /// Closed authority kind.
    pub authority: DirectClosedAuthorityV3,
    /// Original payer.
    pub payer: Identity32V1,
    /// Exact payer-funded principal returned to `payer`.
    pub payer_principal_lamports: u64,
    /// Entire observed balance after any same-transition reward debit.
    pub observed_live_lamports: u64,
    /// Previously authenticated monotone donation lower bound.
    pub prior_donation_lamports: u64,
    /// Excess routed to the immutable neutral sink.
    pub neutral_surplus_lamports: u64,
}

impl DirectCloseLegV3 {
    /// Canonical inactive padding.
    pub const ZERO: Self = Self {
        authority: DirectClosedAuthorityV3::Window,
        payer: Identity32V1::ZERO,
        payer_principal_lamports: 0,
        observed_live_lamports: 0,
        prior_donation_lamports: 0,
        neutral_surplus_lamports: 0,
    };
}

/// Side effects authorized by one modeled transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleEffectsV3 {
    /// Exact closed-account splits; maximum terminal shape is seven accounts.
    pub close_legs: [DirectCloseLegV3; 7],
    /// Active prefix length of `close_legs`.
    pub close_count: u8,
    /// Keeper reward debited only from spendable WorkBudget rewards.
    pub keeper_reward: u64,
    /// Unused spendable rewards returned to their authenticated sponsor.
    pub work_budget_refund: u64,
    /// WorkBudget reward sponsor.
    pub work_budget_refund_recipient: Identity32V1,
    /// Immutable sink for prefunds and later close-time surplus.
    pub neutral_lamport_sink: Identity32V1,
    /// Exact two-reservation state transition.
    pub reservation_transition: DirectReservationTransitionV3,
}

impl DirectLifecycleEffectsV3 {
    /// No external effect.
    pub const NONE: Self = Self {
        close_legs: [DirectCloseLegV3::ZERO; 7],
        close_count: 0,
        keeper_reward: 0,
        work_budget_refund: 0,
        work_budget_refund_recipient: Identity32V1::ZERO,
        neutral_lamport_sink: Identity32V1::ZERO,
        reservation_transition: DirectReservationTransitionV3::None,
    };

    /// Sum exact payer principal, excluding rewards and neutral surplus.
    pub fn payer_principal_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].payer_principal_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    /// Sum observed closed balances after any same-transition reward debit.
    pub fn observed_close_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].observed_live_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    /// Sum unsolicited close-time surplus sent to the neutral sink.
    pub fn neutral_surplus_total(self) -> Result<u64, DirectLifecycleErrorV3> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.close_count) {
            total = total
                .checked_add(self.close_legs[index].neutral_surplus_lamports)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    fn push_close(
        &mut self,
        authority: DirectClosedAuthorityV3,
        account: DirectAccountLedgerV3,
        observed_live_lamports: u64,
        protected_lamports: u64,
        sink: Identity32V1,
    ) -> Result<(), DirectLifecycleErrorV3> {
        account.validate(sink)?;
        let protected_total = account
            .rent
            .lamports
            .checked_add(protected_lamports)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let disposition = account
            .donation
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?
            .terminal_split(observed_live_lamports, protected_total)?;
        let neutral_surplus_lamports = disposition.neutral_lamports;
        let index = usize::from(self.close_count);
        if index >= self.close_legs.len()
            || sink.is_zero()
            || (!self.neutral_lamport_sink.is_zero() && self.neutral_lamport_sink != sink)
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        self.neutral_lamport_sink = sink;
        self.close_legs[index] = DirectCloseLegV3 {
            authority,
            payer: account.rent.payer,
            payer_principal_lamports: account.rent.lamports,
            observed_live_lamports,
            prior_donation_lamports: account.donation_lamports()?,
            neutral_surplus_lamports,
        };
        self.close_count = self
            .close_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Compact durable terminal receipt embedded in the versioned Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTerminalReceiptV3 {
    /// Why authority terminated.
    pub reason: DirectTerminalReasonV3,
    /// Selected Candidate, or zero before selection.
    pub candidate_id: Identity32V1,
    /// Full relation-candidate digest, or zero before selection.
    pub relation_candidate_digest: Identity32V1,
    /// Settled outcome; zero for lapse.
    pub outcome: u8,
    /// Settled quantity; zero for lapse.
    pub quantity: u64,
    /// Settled price units; zero for lapse.
    pub price: u64,
    /// Exact `quantity * price`; zero for lapse.
    pub consideration_price_units: u128,
    /// Slot of terminal transition.
    pub terminal_slot: u64,
}

impl DirectTerminalReceiptV3 {
    /// Canonical placeholder before terminality; phase owns its interpretation.
    pub const EMPTY: Self = Self {
        reason: DirectTerminalReasonV3::EmptyLapse,
        candidate_id: Identity32V1::ZERO,
        relation_candidate_digest: Identity32V1::ZERO,
        outcome: 0,
        quantity: 0,
        price: 0,
        consideration_price_units: 0,
        terminal_slot: 0,
    };
}

/// Complete bounded authority/economic state across Epoch, Window, Candidate,
/// WorkBudget, receipt/pot, and two Reservation accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLifecycleV3 {
    /// Current lifecycle phase.
    pub phase: DirectLifecyclePhaseV3,
    /// Immutable deadlines.
    pub schedule: DirectScheduleV3,
    /// Immutable semantics/sink authority.
    pub authority: DirectLifecycleAuthorityV3,
    /// Immutable reward schedule.
    pub rewards: DirectKeeperRewardsV3,
    /// WorkBudget reward sponsor.
    pub work_budget_sponsor: Identity32V1,
    /// WorkBudget rent and canonical donation ledger.
    pub work_budget_account: DirectAccountLedgerV3,
    /// Remaining spendable reward balance.
    pub work_budget_balance: u64,
    /// Original spendable reward balance.
    pub work_budget_initial_balance: u64,
    /// Rewards already paid before terminality.
    pub work_rewards_paid: u64,
    /// Exact Reservation V2 rent and donation ledgers.
    pub reservation_accounts: [DirectAccountLedgerV3; 2],
    /// Current paired reservation state.
    pub reservation_state: DirectReservationStateV3,
    /// Window top count. It remains unchanged after selection until Window close.
    pub top_count: u8,
    /// Window top commitments. Losers remain here after their accounts close.
    pub top: [DirectCandidateEntryV1; MAX_DIRECT_CANDIDATES],
    /// Live Candidate facts. Closed losers become canonical zero after Finalize.
    pub retained: [DirectCandidateLeaseV3; MAX_DIRECT_CANDIDATES],
    /// Exact physical Candidate-account presence bits.
    pub live_candidate_mask: u8,
    /// Ticks ever competitively admitted, including displaced entries.
    pub seen_competitive_ticks: u64,
    /// Competitive admissions only; bounded by 64.
    pub competitive_admission_count: u8,
    /// Ordered full-width commitment over competitive admissions.
    pub competitive_admission_transcript: Identity32V1,
    /// Window rent and donation ledger.
    pub window_account: DirectAccountLedgerV3,
    /// Bits whose exact Candidate has been fully re-executed in V3.
    pub verification_mask: u8,
    /// Receipt rent/donation ledger, set once by Finalize.
    pub receipt_account: DirectAccountLedgerV3,
    /// Pot rent/donation ledger, set once by Finalize.
    pub pot_account: DirectAccountLedgerV3,
    /// Exact Finalize slot, zero before selection.
    pub selected_slot: u64,
    /// Durable terminal audit evidence retained in Epoch.
    pub terminal_receipt: DirectTerminalReceiptV3,
}

/// Successful initialization at the same atomic boundary as Freeze.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInitializationPlanV3 {
    /// Frozen-empty poststate with two ACTIVE reservations.
    pub post: DirectLifecycleV3,
    /// WorkBudget target prefund normalization.
    pub effects: DirectLifecycleEffectsV3,
}

/// One accepted or explicitly rejected valid submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectAdmissionPlanV3 {
    /// Complete poststate; byte-identical for a noncompetitive attempt.
    pub post: DirectLifecycleV3,
    /// Explicit outcome.
    pub disposition: DirectAdmissionDispositionV3,
    /// Prefund sweep and optional displaced-account close.
    pub effects: DirectLifecycleEffectsV3,
}

/// One successful staged lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTransitionPlanV3 {
    /// Complete poststate.
    pub post: DirectLifecycleV3,
    /// Exact account splits, rewards, prefunds, and reservation transition.
    pub effects: DirectLifecycleEffectsV3,
}

impl DirectLifecycleV3 {
    /// Create V3 authority only at successful Freeze with two ACTIVE reservations.
    pub fn initialize_frozen(
        schedule: DirectScheduleV3,
        authority: DirectLifecycleAuthorityV3,
        rewards: DirectKeeperRewardsV3,
        funding: DirectFrozenFundingV3,
    ) -> Result<DirectInitializationPlanV3, DirectLifecycleErrorV3> {
        schedule.validate()?;
        authority.validate()?;
        rewards.validate()?;
        let work_budget_account = funding.work_budget.creation.account(
            authority.neutral_lamport_sink,
            funding.work_budget.reward_lamports,
        )?;
        funding.reservation_accounts[0].validate(authority.neutral_lamport_sink)?;
        funding.reservation_accounts[1].validate(authority.neutral_lamport_sink)?;
        if funding.work_budget.reward_sponsor.is_zero()
            || funding.work_budget.reward_sponsor != work_budget_account.rent.payer
            || funding.work_budget.reward_lamports < rewards.worst_case()?
        {
            return Err(DirectLifecycleErrorV3::WorkBudgetInsufficient);
        }
        let post = Self {
            phase: DirectLifecyclePhaseV3::FrozenEmpty,
            schedule,
            authority,
            rewards,
            work_budget_sponsor: funding.work_budget.reward_sponsor,
            work_budget_account,
            work_budget_balance: funding.work_budget.reward_lamports,
            work_budget_initial_balance: funding.work_budget.reward_lamports,
            work_rewards_paid: 0,
            reservation_accounts: funding.reservation_accounts,
            reservation_state: DirectReservationStateV3::Active,
            top_count: 0,
            top: [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES],
            retained: [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES],
            live_candidate_mask: 0,
            seen_competitive_ticks: 0,
            competitive_admission_count: 0,
            competitive_admission_transcript: Identity32V1::ZERO,
            window_account: DirectAccountLedgerV3::ZERO,
            verification_mask: 0,
            receipt_account: DirectAccountLedgerV3::ZERO,
            pot_account: DirectAccountLedgerV3::ZERO,
            selected_slot: 0,
            terminal_receipt: DirectTerminalReceiptV3::EMPTY,
        };
        post.validate()?;
        Ok(DirectInitializationPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3::NONE,
        })
    }

    /// Validate every hostile persisted field before any public transition.
    pub fn validate(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.schedule.validate()?;
        self.authority.validate()?;
        self.rewards.validate()?;
        if self.top_count as usize > MAX_DIRECT_CANDIDATES
            || self.competitive_admission_count > MAX_DIRECT_TICKS_V3
            || self.seen_competitive_ticks.count_ones()
                != u32::from(self.competitive_admission_count)
            || self.live_candidate_mask & !prefix_mask(self.top_count)? != 0
            || self.verification_mask & !prefix_mask(self.top_count)? != 0
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let mut index = 0usize;
        while index < MAX_DIRECT_CANDIDATES {
            if index < usize::from(self.top_count) {
                let entry = self.top[index];
                if entry.candidate_id.is_zero() || entry.relation_candidate_digest.is_zero() {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                let mut prior = 0usize;
                while prior < index {
                    if self.top[prior].candidate_id == entry.candidate_id
                        || self.top[prior].relation_candidate_digest
                            == entry.relation_candidate_digest
                    {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    prior += 1;
                }
                if self.live_candidate_mask & (1u8 << index) != 0 {
                    let lease = self.retained[index];
                    lease.validate_body()?;
                    lease
                        .account
                        .validate(self.authority.neutral_lamport_sink)?;
                    if entry != lease.entry()
                        || self.seen_competitive_ticks & (1u64 << lease.tick) == 0
                    {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    let mut live_prior = 0usize;
                    while live_prior < index {
                        if self.live_candidate_mask & (1u8 << live_prior) != 0
                            && self.retained[live_prior].tick == lease.tick
                        {
                            return Err(DirectLifecycleErrorV3::MismatchedBinding);
                        }
                        live_prior += 1;
                    }
                    if index > 0
                        && self.live_candidate_mask & (1u8 << (index - 1)) != 0
                        && !self.retained[index - 1]
                            .score()
                            .is_better_than(&lease.score())
                    {
                        return Err(DirectLifecycleErrorV3::NonCanonical);
                    }
                } else if self.phase != DirectLifecyclePhaseV3::Selected
                    || index == 0
                    || self.retained[index] != DirectCandidateLeaseV3::ZERO
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
            } else if self.top[index] != DirectCandidateEntryV1::ZERO
                || self.retained[index] != DirectCandidateLeaseV3::ZERO
            {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
            index += 1;
        }
        if self.competitive_admission_count == 0 {
            if !self.competitive_admission_transcript.is_zero() {
                return Err(DirectLifecycleErrorV3::NonCanonical);
            }
        } else if self.competitive_admission_transcript.is_zero() {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        match self.phase {
            DirectLifecyclePhaseV3::FrozenEmpty => {
                if self.top_count != 0
                    || self.live_candidate_mask != 0
                    || self.window_account != DirectAccountLedgerV3::ZERO
                    || self.verification_mask != 0
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::WindowOpen => {
                if self.top_count == 0
                    || self.live_candidate_mask != prefix_mask(self.top_count)?
                    || self.verification_mask != 0
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.require_stages(false)?;
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Verifying => {
                if self.top_count == 0
                    || self.live_candidate_mask != prefix_mask(self.top_count)?
                    || self.selected_slot != 0
                    || self.receipt_account != DirectAccountLedgerV3::ZERO
                    || self.pot_account != DirectAccountLedgerV3::ZERO
                    || self.reservation_state != DirectReservationStateV3::Active
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                let mut candidate_index = 0usize;
                while candidate_index < usize::from(self.top_count) {
                    let bit = 1u8 << candidate_index;
                    let expected = if self.verification_mask & bit == 0 {
                        DirectCandidateStageV3::Verified
                    } else {
                        DirectCandidateStageV3::Reverified
                    };
                    if self.retained[candidate_index].stage != expected {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    candidate_index += 1;
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Selected => {
                if self.top_count == 0
                    || self.live_candidate_mask != 1
                    || self.verification_mask != prefix_mask(self.top_count)?
                    || self.selected_slot < self.schedule.submission_closes_slot
                    || self.selected_slot >= self.schedule.selection_deadline_slot
                    || self.reservation_state != DirectReservationStateV3::Entitled
                    || self.terminal_receipt != DirectTerminalReceiptV3::EMPTY
                {
                    return Err(DirectLifecycleErrorV3::NonCanonical);
                }
                self.window_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.receipt_account
                    .validate(self.authority.neutral_lamport_sink)?;
                self.pot_account
                    .validate(self.authority.neutral_lamport_sink)?;
                if self.retained[0].stage != DirectCandidateStageV3::Selected {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
                let mut loser = 1usize;
                while loser < usize::from(self.top_count) {
                    if self.retained[loser] != DirectCandidateLeaseV3::ZERO {
                        return Err(DirectLifecycleErrorV3::MismatchedBinding);
                    }
                    loser += 1;
                }
                self.validate_open_work_budget()?;
            }
            DirectLifecyclePhaseV3::Terminal => self.validate_terminal()?,
        }
        Ok(())
    }

    /// Admit one verified Candidate iff it can enter the monotone top three.
    pub fn admit(
        self,
        context: DirectTransitionContextV3,
        candidate: DirectCandidateLeaseV3,
        first_window: DirectCreationFundingV3,
        displaced_live_lamports: u64,
    ) -> Result<DirectAdmissionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_code(context)?;
        if !matches!(
            self.phase,
            DirectLifecyclePhaseV3::FrozenEmpty | DirectLifecyclePhaseV3::WindowOpen
        ) {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        if context.now < self.schedule.submission_opens_slot
            || context.now >= self.schedule.submission_closes_slot
        {
            return Err(DirectLifecycleErrorV3::SubmissionClosed);
        }
        candidate.validate_body()?;
        candidate
            .account
            .validate(self.authority.neutral_lamport_sink)?;
        if candidate.stage != DirectCandidateStageV3::Verified
            || candidate.candidate.submitted_slot != context.now
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let first_window_account = if self.phase == DirectLifecyclePhaseV3::FrozenEmpty {
            first_window.account(self.authority.neutral_lamport_sink, 0)?
        } else if first_window != DirectCreationFundingV3::ZERO {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        } else {
            DirectAccountLedgerV3::ZERO
        };
        let bit = 1u64 << candidate.tick;
        if self.seen_competitive_ticks & bit != 0 {
            return Err(DirectLifecycleErrorV3::Replay);
        }
        let count = usize::from(self.top_count);
        let mut effects = DirectLifecycleEffectsV3::NONE;
        if count == MAX_DIRECT_CANDIDATES
            && !candidate
                .score()
                .is_better_than(&self.retained[MAX_DIRECT_CANDIDATES - 1].score())
        {
            return Ok(DirectAdmissionPlanV3 {
                post: self,
                disposition: DirectAdmissionDispositionV3::RejectedNonCompetitive,
                effects,
            });
        }
        let mut post = self;
        let disposition;
        if post.phase == DirectLifecyclePhaseV3::FrozenEmpty {
            post.phase = DirectLifecyclePhaseV3::WindowOpen;
            post.window_account = first_window_account;
            disposition = DirectAdmissionDispositionV3::First;
        } else if count == MAX_DIRECT_CANDIDATES {
            effects.push_close(
                DirectClosedAuthorityV3::Candidate((MAX_DIRECT_CANDIDATES - 1) as u8),
                post.retained[MAX_DIRECT_CANDIDATES - 1].account,
                displaced_live_lamports,
                0,
                post.authority.neutral_lamport_sink,
            )?;
            post.retained[MAX_DIRECT_CANDIDATES - 1] = candidate;
            post.top[MAX_DIRECT_CANDIDATES - 1] = candidate.entry();
            disposition = DirectAdmissionDispositionV3::Replaced;
        } else {
            post.retained[count] = candidate;
            post.top[count] = candidate.entry();
            post.top_count = post
                .top_count
                .checked_add(1)
                .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
            disposition = DirectAdmissionDispositionV3::Retained;
        }
        if disposition == DirectAdmissionDispositionV3::First {
            post.retained[0] = candidate;
            post.top[0] = candidate.entry();
            post.top_count = 1;
        }
        let mut index = usize::from(post.top_count) - 1;
        while index > 0
            && post.retained[index]
                .score()
                .is_better_than(&post.retained[index - 1].score())
        {
            post.retained.swap(index, index - 1);
            post.top.swap(index, index - 1);
            index -= 1;
        }
        post.live_candidate_mask = prefix_mask(post.top_count)?;
        post.seen_competitive_ticks |= bit;
        post.competitive_admission_count = post
            .competitive_admission_count
            .checked_add(1)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        post.competitive_admission_transcript = next_transcript(
            post.competitive_admission_transcript,
            post.competitive_admission_count,
            &candidate,
        );
        post.validate()?;
        Ok(DirectAdmissionPlanV3 {
            post,
            disposition,
            effects,
        })
    }

    /// Close submissions and begin staged retained-candidate verification.
    pub fn begin_verification(
        self,
        context: DirectTransitionContextV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_code(context)?;
        if self.phase != DirectLifecyclePhaseV3::WindowOpen {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        let mut post = self;
        let reward = post.debit_reward(post.rewards.begin_verification)?;
        post.phase = DirectLifecyclePhaseV3::Verifying;
        post.verification_mask = 0;
        post.validate()?;
        Ok(DirectTransitionPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3 {
                keeper_reward: reward,
                ..DirectLifecycleEffectsV3::NONE
            },
        })
    }

    /// Re-execute one exact retained Candidate under the frozen verifier/grid.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_candidate(
        self,
        context: DirectTransitionContextV3,
        index: u8,
        domain: &ValidatedDirectDomainV1,
        grid: &DirectGridV3,
        buy_limit: u64,
        sell_limit: u64,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_code(context)?;
        if self.phase != DirectLifecyclePhaseV3::Verifying {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        if index >= self.top_count {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        let bit = 1u8 << index;
        if self.verification_mask & bit != 0 {
            return Err(DirectLifecycleErrorV3::Replay);
        }
        let retained = self.retained[usize::from(index)];
        verify_lease(domain, grid, retained, buy_limit, sell_limit)?;
        let mut post = self;
        let reward = post.debit_reward(post.rewards.verify_candidate)?;
        post.retained[usize::from(index)].stage = DirectCandidateStageV3::Reverified;
        post.verification_mask |= bit;
        post.validate()?;
        Ok(DirectTransitionPlanV3 {
            post,
            effects: DirectLifecycleEffectsV3 {
                keeper_reward: reward,
                ..DirectLifecycleEffectsV3::NONE
            },
        })
    }

    /// Select top zero only after exact staged reexecution of the full prefix.
    pub fn finalize_selection(
        self,
        context: DirectTransitionContextV3,
        funding: DirectSelectionFundingV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_code(context)?;
        if self.phase != DirectLifecyclePhaseV3::Verifying {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        self.require_selection_slot(context.now)?;
        let required_mask = prefix_mask(self.top_count)?;
        if self.verification_mask != required_mask {
            return Err(DirectLifecycleErrorV3::VerificationIncomplete);
        }
        let receipt_account = funding
            .receipt
            .account(self.authority.neutral_lamport_sink, 0)?;
        let pot_account = funding
            .pot
            .account(self.authority.neutral_lamport_sink, 0)?;
        let mut candidate_index = 0usize;
        while candidate_index < usize::from(self.top_count) {
            if self.retained[candidate_index].stage != DirectCandidateStageV3::Reverified {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            candidate_index += 1;
        }
        let mut post = self;
        let reward = post.debit_reward(post.rewards.finalize_selection)?;
        let mut effects = DirectLifecycleEffectsV3 {
            keeper_reward: reward,
            reservation_transition: DirectReservationTransitionV3::ActiveToEntitled,
            ..DirectLifecycleEffectsV3::NONE
        };
        let mut loser = 1usize;
        while loser < usize::from(post.top_count) {
            effects.push_close(
                DirectClosedAuthorityV3::Candidate(loser as u8),
                post.retained[loser].account,
                observed.candidates[loser],
                0,
                post.authority.neutral_lamport_sink,
            )?;
            post.retained[loser] = DirectCandidateLeaseV3::ZERO;
            loser += 1;
        }
        post.retained[0].stage = DirectCandidateStageV3::Selected;
        post.live_candidate_mask = 1;
        post.phase = DirectLifecyclePhaseV3::Selected;
        post.reservation_state = DirectReservationStateV3::Entitled;
        post.receipt_account = receipt_account;
        post.pot_account = pot_account;
        post.selected_slot = context.now;
        post.validate()?;
        Ok(DirectTransitionPlanV3 { post, effects })
    }

    /// Complete exact settlement from the selected Candidate facts.
    pub fn settle(
        self,
        context: DirectTransitionContextV3,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        self.require_code(context)?;
        if self.phase != DirectLifecyclePhaseV3::Selected {
            return Err(DirectLifecycleErrorV3::WrongPhase);
        }
        if context.now < self.selected_slot || context.now >= self.schedule.settlement_deadline_slot
        {
            return Err(DirectLifecycleErrorV3::SettlementClosed);
        }
        let selected = self.retained[0];
        let price = selected.candidate.prices[usize::from(selected.candidate.outcome)];
        let consideration = u128::from(selected.candidate.quantity)
            .checked_mul(u128::from(price))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        let receipt = DirectTerminalReceiptV3 {
            reason: DirectTerminalReasonV3::Settled,
            candidate_id: selected.candidate.candidate_id,
            relation_candidate_digest: selected.candidate.relation_candidate_digest,
            outcome: selected.candidate.outcome,
            quantity: selected.candidate.quantity,
            price,
            consideration_price_units: consideration,
            terminal_slot: context.now,
        };
        self.finish_terminal(
            receipt,
            DirectReservationTransitionV3::EntitledToConsumed,
            self.rewards.settle,
            observed,
        )
    }

    /// Permissionlessly terminate every frozen lifecycle phase after deadline.
    pub fn lapse(
        self,
        now: u64,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        self.validate()?;
        let (reason, reservation_transition) = match self.phase {
            DirectLifecyclePhaseV3::FrozenEmpty if now >= self.schedule.selection_deadline_slot => {
                (
                    DirectTerminalReasonV3::EmptyLapse,
                    DirectReservationTransitionV3::ActiveToReleased,
                )
            }
            DirectLifecyclePhaseV3::WindowOpen | DirectLifecyclePhaseV3::Verifying
                if now >= self.schedule.selection_deadline_slot =>
            {
                (
                    DirectTerminalReasonV3::PreSelectionLapse,
                    DirectReservationTransitionV3::ActiveToReleased,
                )
            }
            DirectLifecyclePhaseV3::Selected if now >= self.schedule.settlement_deadline_slot => (
                DirectTerminalReasonV3::PostSelectionLapse,
                DirectReservationTransitionV3::EntitledToReleased,
            ),
            DirectLifecyclePhaseV3::Terminal => return Err(DirectLifecycleErrorV3::Replay),
            _ => return Err(DirectLifecycleErrorV3::SelectionWindow),
        };
        let selected = if self.top_count == 0 {
            DirectCandidateLeaseV3::ZERO
        } else {
            self.retained[0]
        };
        let receipt = DirectTerminalReceiptV3 {
            reason,
            candidate_id: if reason == DirectTerminalReasonV3::PostSelectionLapse {
                selected.candidate.candidate_id
            } else {
                Identity32V1::ZERO
            },
            relation_candidate_digest: if reason == DirectTerminalReasonV3::PostSelectionLapse {
                selected.candidate.relation_candidate_digest
            } else {
                Identity32V1::ZERO
            },
            outcome: 0,
            quantity: 0,
            price: 0,
            consideration_price_units: 0,
            terminal_slot: now,
        };
        self.finish_terminal(
            receipt,
            reservation_transition,
            self.rewards.lapse,
            observed,
        )
    }

    fn validate_open_work_budget(&self) -> Result<(), DirectLifecycleErrorV3> {
        self.work_budget_account
            .validate(self.authority.neutral_lamport_sink)?;
        self.reservation_accounts[0].validate(self.authority.neutral_lamport_sink)?;
        self.reservation_accounts[1].validate(self.authority.neutral_lamport_sink)?;
        if self.work_budget_sponsor.is_zero()
            || self.work_budget_initial_balance
                != self
                    .work_budget_balance
                    .checked_add(self.work_rewards_paid)
                    .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?
        {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_terminal(&self) -> Result<(), DirectLifecycleErrorV3> {
        if self.top_count != 0
            || self.top != [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES]
            || self.retained != [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES]
            || self.live_candidate_mask != 0
            || self.window_account != DirectAccountLedgerV3::ZERO
            || self.verification_mask != 0
            || self.receipt_account != DirectAccountLedgerV3::ZERO
            || self.pot_account != DirectAccountLedgerV3::ZERO
            || self.work_budget_sponsor != Identity32V1::ZERO
            || self.work_budget_account != DirectAccountLedgerV3::ZERO
            || self.work_budget_balance != 0
            || self.work_budget_initial_balance != 0
            || self.work_rewards_paid != 0
            || self.reservation_accounts != [DirectAccountLedgerV3::ZERO; 2]
            || self.reservation_state != DirectReservationStateV3::Terminal
            || self.selected_slot != 0
            || self.terminal_receipt.terminal_slot == 0
        {
            return Err(DirectLifecycleErrorV3::NonCanonical);
        }
        let receipt = self.terminal_receipt;
        match receipt.reason {
            DirectTerminalReasonV3::EmptyLapse | DirectTerminalReasonV3::PreSelectionLapse => {
                if !receipt.candidate_id.is_zero()
                    || !receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome != 0
                    || receipt.quantity != 0
                    || receipt.price != 0
                    || receipt.consideration_price_units != 0
                    || receipt.terminal_slot < self.schedule.selection_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
            DirectTerminalReasonV3::PostSelectionLapse => {
                if receipt.candidate_id.is_zero()
                    || receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome != 0
                    || receipt.quantity != 0
                    || receipt.price != 0
                    || receipt.consideration_price_units != 0
                    || receipt.terminal_slot < self.schedule.settlement_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
            DirectTerminalReasonV3::Settled => {
                let consideration = u128::from(receipt.quantity)
                    .checked_mul(u128::from(receipt.price))
                    .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
                if receipt.candidate_id.is_zero()
                    || receipt.relation_candidate_digest.is_zero()
                    || receipt.outcome >= 2
                    || receipt.quantity == 0
                    || receipt.price == 0
                    || receipt.consideration_price_units != consideration
                    || receipt.terminal_slot >= self.schedule.settlement_deadline_slot
                {
                    return Err(DirectLifecycleErrorV3::MismatchedBinding);
                }
            }
        }
        Ok(())
    }

    fn require_stages(&self, reverified: bool) -> Result<(), DirectLifecycleErrorV3> {
        let expected = if reverified {
            DirectCandidateStageV3::Reverified
        } else {
            DirectCandidateStageV3::Verified
        };
        let mut index = 0usize;
        while index < usize::from(self.top_count) {
            if self.retained[index].stage != expected {
                return Err(DirectLifecycleErrorV3::MismatchedBinding);
            }
            index += 1;
        }
        Ok(())
    }

    fn require_code(
        &self,
        context: DirectTransitionContextV3,
    ) -> Result<(), DirectLifecycleErrorV3> {
        if context.verifier_code_identity != self.authority.verifier_code_identity {
            return Err(DirectLifecycleErrorV3::MismatchedBinding);
        }
        Ok(())
    }

    fn require_selection_slot(&self, now: u64) -> Result<(), DirectLifecycleErrorV3> {
        if now < self.schedule.submission_closes_slot
            || now >= self.schedule.selection_deadline_slot
        {
            return Err(DirectLifecycleErrorV3::SelectionWindow);
        }
        Ok(())
    }

    fn debit_reward(&mut self, reward: u64) -> Result<u64, DirectLifecycleErrorV3> {
        self.work_budget_balance = self
            .work_budget_balance
            .checked_sub(reward)
            .ok_or(DirectLifecycleErrorV3::WorkBudgetInsufficient)?;
        self.work_rewards_paid = self
            .work_rewards_paid
            .checked_add(reward)
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)?;
        Ok(reward)
    }

    fn finish_terminal(
        self,
        receipt: DirectTerminalReceiptV3,
        reservation_transition: DirectReservationTransitionV3,
        keeper_reward: u64,
        observed: DirectObservedBalancesV3,
    ) -> Result<DirectTransitionPlanV3, DirectLifecycleErrorV3> {
        let mut post = self;
        let reward = post.debit_reward(keeper_reward)?;
        let mut effects = DirectLifecycleEffectsV3 {
            keeper_reward: reward,
            reservation_transition,
            ..DirectLifecycleEffectsV3::NONE
        };
        let sink = post.authority.neutral_lamport_sink;
        let mut index = 0usize;
        while index < usize::from(post.top_count) {
            if post.live_candidate_mask & (1u8 << index) != 0 {
                effects.push_close(
                    DirectClosedAuthorityV3::Candidate(index as u8),
                    post.retained[index].account,
                    observed.candidates[index],
                    0,
                    sink,
                )?;
            }
            index += 1;
        }
        if post.window_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Window,
                post.window_account,
                observed.window,
                0,
                sink,
            )?;
        }
        if post.receipt_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Receipt,
                post.receipt_account,
                observed.receipt,
                0,
                sink,
            )?;
        }
        if post.pot_account != DirectAccountLedgerV3::ZERO {
            effects.push_close(
                DirectClosedAuthorityV3::Pot,
                post.pot_account,
                observed.pot,
                0,
                sink,
            )?;
        }
        let work_live_after_reward = observed
            .work_budget
            .checked_sub(reward)
            .ok_or(DirectLifecycleErrorV3::MismatchedBinding)?;
        effects.push_close(
            DirectClosedAuthorityV3::WorkBudget,
            post.work_budget_account,
            work_live_after_reward,
            post.work_budget_balance,
            sink,
        )?;
        effects.work_budget_refund = post.work_budget_balance;
        effects.work_budget_refund_recipient = post.work_budget_sponsor;
        let mut reservation_index = 0usize;
        while reservation_index < 2 {
            effects.push_close(
                DirectClosedAuthorityV3::Reservation(reservation_index as u8),
                post.reservation_accounts[reservation_index],
                observed.reservations[reservation_index],
                0,
                sink,
            )?;
            reservation_index += 1;
        }
        post.phase = DirectLifecyclePhaseV3::Terminal;
        post.top_count = 0;
        post.top = [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES];
        post.retained = [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_CANDIDATES];
        post.live_candidate_mask = 0;
        post.window_account = DirectAccountLedgerV3::ZERO;
        post.verification_mask = 0;
        post.receipt_account = DirectAccountLedgerV3::ZERO;
        post.pot_account = DirectAccountLedgerV3::ZERO;
        post.work_budget_sponsor = Identity32V1::ZERO;
        post.work_budget_account = DirectAccountLedgerV3::ZERO;
        post.work_budget_balance = 0;
        post.work_budget_initial_balance = 0;
        post.work_rewards_paid = 0;
        post.reservation_accounts = [DirectAccountLedgerV3::ZERO; 2];
        post.reservation_state = DirectReservationStateV3::Terminal;
        post.selected_slot = 0;
        post.terminal_receipt = receipt;
        post.validate()?;
        Ok(DirectTransitionPlanV3 { post, effects })
    }
}

fn prefix_mask(count: u8) -> Result<u8, DirectLifecycleErrorV3> {
    if usize::from(count) > MAX_DIRECT_CANDIDATES {
        return Err(DirectLifecycleErrorV3::NonCanonical);
    }
    if count == 0 {
        Ok(0)
    } else {
        1u8.checked_shl(u32::from(count))
            .and_then(|value| value.checked_sub(1))
            .ok_or(DirectLifecycleErrorV3::ArithmeticOverflow)
    }
}

fn verify_lease(
    domain: &ValidatedDirectDomainV1,
    grid: &DirectGridV3,
    lease: DirectCandidateLeaseV3,
    buy_limit: u64,
    sell_limit: u64,
) -> Result<(), DirectLifecycleErrorV3> {
    lease.validate_body()?;
    lease.account.validate(Identity32V1(
        lease
            .account
            .donation
            .ok_or(DirectLifecycleErrorV3::NonCanonical)?
            .neutral_sink()
            .bytes(),
    ))?;
    let candidate = lease.candidate;
    let input = DirectTwoOrderInputV1 {
        prices: candidate.prices,
        buy_limit,
        sell_limit,
        quantity: candidate.quantity,
        submitted_slot: candidate.submitted_slot,
        buy_index: candidate.buy_index,
        sell_index: candidate.sell_index,
        outcome: candidate.outcome,
        stored_bump: candidate.stored_bump,
    };
    domain.reverify_decoded_candidate(&candidate, input)?;
    grid.tick_of(candidate.prices[0])?;
    grid.tick_of(candidate.prices[1])?;
    if grid.tick_of(candidate.prices[usize::from(candidate.outcome)])? != lease.tick {
        return Err(DirectLifecycleErrorV3::MismatchedBinding);
    }
    Ok(())
}

fn next_transcript(
    previous: Identity32V1,
    count: u8,
    candidate: &DirectCandidateLeaseV3,
) -> Identity32V1 {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_DOMAIN_V3);
    hasher.update(previous.0);
    hasher.update([count]);
    hasher.update([candidate.tick]);
    hasher.update(candidate.candidate.candidate_id.0);
    hasher.update(candidate.candidate.relation_candidate_digest.0);
    Identity32V1(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{batch_policy_digest, FullRelationDomainV1};
    use clutch_batch::relation_v1::{PRICE_SCALE, RELATION_VERSION_V1};

    const BUY_LIMIT: u64 = PRICE_SCALE;
    const SELL_LIMIT: u64 = 0;

    fn id(byte: u8) -> Identity32V1 {
        let mut value = [byte; 32];
        value[31] = byte.wrapping_add(1);
        Identity32V1(value)
    }

    fn schedule() -> DirectScheduleV3 {
        DirectScheduleV3 {
            submission_opens_slot: 10,
            submission_closes_slot: 20,
            selection_deadline_slot: 30,
            settlement_deadline_slot: 40,
        }
    }

    fn rewards() -> DirectKeeperRewardsV3 {
        DirectKeeperRewardsV3 {
            begin_verification: 2,
            verify_candidate: 3,
            finalize_selection: 5,
            settle: 7,
            lapse: 11,
        }
    }

    fn authority() -> DirectLifecycleAuthorityV3 {
        DirectLifecycleAuthorityV3 {
            verifier_code_identity: id(80),
            neutral_lamport_sink: id(81),
        }
    }

    fn context(now: u64) -> DirectTransitionContextV3 {
        DirectTransitionContextV3 {
            now,
            verifier_code_identity: authority().verifier_code_identity,
        }
    }

    fn rent(payer: u8, lamports: u64) -> DirectRentPrincipalV3 {
        DirectRentPrincipalV3 {
            payer: id(payer),
            lamports,
        }
    }

    fn create(payer: u8, lamports: u64, prefund: u64) -> DirectCreationFundingV3 {
        create_with_extra(payer, lamports, prefund, 0)
    }

    fn create_with_extra(
        payer: u8,
        lamports: u64,
        prefund: u64,
        extra: u64,
    ) -> DirectCreationFundingV3 {
        DirectCreationFundingV3 {
            rent: rent(payer, lamports),
            balance_before: prefund,
            balance_after: prefund + lamports + extra,
        }
    }

    fn account(payer: u8, lamports: u64, prefund: u64) -> DirectAccountLedgerV3 {
        create(payer, lamports, prefund)
            .account(authority().neutral_lamport_sink, 0)
            .unwrap()
    }

    fn funding() -> DirectFrozenFundingV3 {
        DirectFrozenFundingV3 {
            work_budget: DirectWorkBudgetFundingV3 {
                reward_sponsor: id(90),
                creation: create_with_extra(90, 500, 1, 100),
                reward_lamports: 100,
            },
            reservation_accounts: [account(92, 600, 2), account(93, 700, 3)],
        }
    }

    fn empty() -> DirectLifecycleV3 {
        DirectLifecycleV3::initialize_frozen(schedule(), authority(), rewards(), funding())
            .unwrap()
            .post
    }

    fn domain() -> ValidatedDirectDomainV1 {
        let policy = crate::direct_window_v1::DIRECT_POLICY_V1;
        let domain = FullRelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: id(1),
            book_id: id(2),
            epoch_id: id(3),
            policy_id: batch_policy_digest(&policy).unwrap(),
            order_set_id: id(5),
            epoch_index: 7,
            outcome_count: 2,
            owner_count: 2,
            price_scale: PRICE_SCALE,
            remainder_seed: 9,
            policy,
        };
        ValidatedDirectDomainV1::new(&domain).unwrap()
    }

    fn grid() -> DirectGridV3 {
        let mut ticks = [0u64; MAX_DIRECT_TICKS_V3 as usize];
        let active = [
            1_000, 2_000, 3_000, 4_000, 5_000, 6_000, 7_000, 8_000, 9_000,
        ];
        ticks[..active.len()].copy_from_slice(&active);
        DirectGridV3 {
            price_scale: PRICE_SCALE,
            tick_count: active.len() as u8,
            ticks,
        }
    }

    fn issued(price: u64, slot: u64, payer: u8) -> DirectCandidateLeaseV3 {
        issued_on_grid(&grid(), price, slot, payer)
    }

    fn issued_on_grid(
        grid: &DirectGridV3,
        price: u64,
        slot: u64,
        payer: u8,
    ) -> DirectCandidateLeaseV3 {
        let domain = domain();
        let input = DirectTwoOrderInputV1 {
            prices: {
                let mut prices = [0u64; MAX_OUTCOMES];
                prices[0] = price;
                prices[1] = PRICE_SCALE - price;
                prices
            },
            buy_limit: BUY_LIMIT,
            sell_limit: SELL_LIMIT,
            quantity: PRICE_SCALE,
            submitted_slot: slot,
            buy_index: 0,
            sell_index: 1,
            outcome: 0,
            stored_bump: 7,
        };
        let candidate = domain.verify(input).unwrap();
        DirectCandidateLeaseV3::issue(
            &domain,
            grid,
            candidate,
            input,
            create(payer, 100 + price / 1_000, 1),
            authority().neutral_lamport_sink,
        )
        .unwrap()
    }

    fn admit_three() -> DirectLifecycleV3 {
        let first = empty()
            .admit(context(10), issued(2_000, 10, 11), create(21, 200, 2), 0)
            .unwrap();
        let second = first
            .post
            .admit(
                context(11),
                issued(3_000, 11, 12),
                DirectCreationFundingV3::ZERO,
                0,
            )
            .unwrap();
        second
            .post
            .admit(
                context(12),
                issued(4_000, 12, 13),
                DirectCreationFundingV3::ZERO,
                0,
            )
            .unwrap()
            .post
    }

    fn verify_all(state: DirectLifecycleV3) -> DirectLifecycleV3 {
        let begun = state.begin_verification(context(20)).unwrap().post;
        let mut post = begun;
        let count = post.top_count;
        let mut index = 0u8;
        while index < count {
            post = post
                .verify_candidate(
                    context(21 + u64::from(index)),
                    index,
                    &domain(),
                    &grid(),
                    BUY_LIMIT,
                    SELL_LIMIT,
                )
                .unwrap()
                .post;
            index += 1;
        }
        post
    }

    fn observed(state: &DirectLifecycleV3, donation: u64) -> DirectObservedBalancesV3 {
        let mut out = DirectObservedBalancesV3::ZERO;
        let mut index = 0usize;
        while index < usize::from(state.top_count) {
            if state.live_candidate_mask & (1u8 << index) != 0 {
                out.candidates[index] = account_balance(state.retained[index].account, 0, donation);
            }
            index += 1;
        }
        if state.window_account != DirectAccountLedgerV3::ZERO {
            out.window = account_balance(state.window_account, 0, donation);
        }
        if state.receipt_account != DirectAccountLedgerV3::ZERO {
            out.receipt = account_balance(state.receipt_account, 0, donation);
        }
        if state.pot_account != DirectAccountLedgerV3::ZERO {
            out.pot = account_balance(state.pot_account, 0, donation);
        }
        if state.work_budget_account != DirectAccountLedgerV3::ZERO {
            out.work_budget = account_balance(
                state.work_budget_account,
                state.work_budget_balance,
                donation,
            );
        }
        let mut reservation = 0usize;
        while reservation < 2 {
            if state.reservation_accounts[reservation] != DirectAccountLedgerV3::ZERO {
                out.reservations[reservation] =
                    account_balance(state.reservation_accounts[reservation], 0, donation);
            }
            reservation += 1;
        }
        out
    }

    fn account_balance(account: DirectAccountLedgerV3, protected: u64, later: u64) -> u64 {
        account.rent.lamports + account.donation_lamports().unwrap() + protected + later
    }

    fn selected() -> DirectLifecycleV3 {
        let verified = verify_all(admit_three());
        let mut balances = DirectObservedBalancesV3::ZERO;
        let mut index = 1usize;
        while index < usize::from(verified.top_count) {
            balances.candidates[index] = account_balance(verified.retained[index].account, 0, 0);
            index += 1;
        }
        verified
            .finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 1),
                    pot: create(51, 400, 1),
                },
                balances,
            )
            .unwrap()
            .post
    }

    #[test]
    fn schedule_reward_and_prefund_rules_are_exact() {
        let mut too_short = schedule();
        too_short.selection_deadline_slot = 24;
        assert_eq!(
            too_short.validate(),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut too_long = schedule();
        too_long.settlement_deadline_slot =
            too_long.selection_deadline_slot + MAX_SETTLEMENT_SPAN_V3 + 1;
        assert_eq!(
            too_long.validate(),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut zero = rewards();
        zero.verify_candidate = 0;
        assert_eq!(
            zero.validate(),
            Err(DirectLifecycleErrorV3::WorkBudgetInsufficient)
        );
        let initialized =
            DirectLifecycleV3::initialize_frozen(schedule(), authority(), rewards(), funding())
                .unwrap();
        assert_eq!(initialized.effects, DirectLifecycleEffectsV3::NONE);
        assert_eq!(
            initialized.post.work_budget_account.donation_lamports(),
            Ok(1)
        );
        assert_eq!(
            initialized.post.reservation_accounts[0].donation_lamports(),
            Ok(2)
        );
    }

    #[test]
    fn canonical_donation_ledgers_refuse_shortfall_and_compose_by_account_class() {
        let underfunded = DirectCreationFundingV3 {
            rent: rent(10, 100),
            balance_before: 7,
            balance_after: 106,
        };
        assert_eq!(
            underfunded.account(authority().neutral_lamport_sink, 0),
            Err(DirectLifecycleErrorV3::LivenessRefused)
        );

        let first = create(10, 100, 7)
            .account(authority().neutral_lamport_sink, 0)
            .unwrap();
        assert_eq!(
            DirectAccountLedgerV3::restore(
                first.rent,
                authority().neutral_lamport_sink,
                first.donation_lamports().unwrap(),
            ),
            Ok(first)
        );
        let second = create(11, 200, 9)
            .account(authority().neutral_lamport_sink, 0)
            .unwrap();
        let mut effects = DirectLifecycleEffectsV3::NONE;
        effects
            .push_close(
                DirectClosedAuthorityV3::Candidate(0),
                first,
                112,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        effects
            .push_close(
                DirectClosedAuthorityV3::Window,
                second,
                212,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        assert_eq!(effects.payer_principal_total(), Ok(300));
        assert_eq!(effects.neutral_surplus_total(), Ok(24));
        assert_eq!(effects.neutral_lamport_sink, id(81));
        assert_eq!(effects.close_legs[0].prior_donation_lamports, 7);
        assert_eq!(effects.close_legs[1].prior_donation_lamports, 9);

        let maximum = DirectCreationFundingV3 {
            rent: rent(12, u64::MAX),
            balance_before: 0,
            balance_after: u64::MAX,
        }
        .account(authority().neutral_lamport_sink, 0)
        .unwrap();
        let mut overflow = DirectLifecycleEffectsV3::NONE;
        overflow
            .push_close(
                DirectClosedAuthorityV3::Candidate(0),
                maximum,
                u64::MAX,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        overflow
            .push_close(
                DirectClosedAuthorityV3::Window,
                first,
                107,
                0,
                authority().neutral_lamport_sink,
            )
            .unwrap();
        assert_eq!(
            overflow.payer_principal_total(),
            Err(DirectLifecycleErrorV3::ArithmeticOverflow)
        );
    }

    #[test]
    fn verified_input_binds_grid_tick_candidate_id_score_and_digest() {
        let valid = issued(2_000, 10, 11);
        let mut wrong_tick = valid;
        wrong_tick.tick = 2;
        assert_eq!(
            verify_lease(&domain(), &grid(), wrong_tick, BUY_LIMIT, SELL_LIMIT),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let mut wrong_score = valid;
        wrong_score.candidate.weighted_direct_volume += 1;
        assert_eq!(
            verify_lease(&domain(), &grid(), wrong_score, BUY_LIMIT, SELL_LIMIT),
            Err(DirectLifecycleErrorV3::RelationRefused)
        );
        let mut wrong_id = valid.candidate;
        wrong_id.candidate_id = id(44);
        assert_eq!(
            DirectCandidateLeaseV3::issue(
                &domain(),
                &grid(),
                wrong_id,
                DirectTwoOrderInputV1 {
                    prices: valid.candidate.prices,
                    buy_limit: BUY_LIMIT,
                    sell_limit: SELL_LIMIT,
                    quantity: valid.candidate.quantity,
                    submitted_slot: valid.candidate.submitted_slot,
                    buy_index: valid.candidate.buy_index,
                    sell_index: valid.candidate.sell_index,
                    outcome: valid.candidate.outcome,
                    stored_bump: valid.candidate.stored_bump,
                },
                create(11, valid.account.rent.lamports, 1),
                authority().neutral_lamport_sink,
            ),
            Err(DirectLifecycleErrorV3::RelationRefused)
        );
    }

    #[test]
    fn submission_edges_noncompetitive_noop_and_replacement_conserve() {
        let state = empty();
        assert_eq!(
            state.admit(context(9), issued(2_000, 9, 11), create(21, 200, 0), 0),
            Err(DirectLifecycleErrorV3::SubmissionClosed)
        );
        assert!(state
            .admit(context(10), issued(2_000, 10, 11), create(21, 200, 0), 0)
            .is_ok());
        assert_eq!(
            state.admit(context(20), issued(2_000, 20, 11), create(21, 200, 0), 0),
            Err(DirectLifecycleErrorV3::SubmissionClosed)
        );
        let full = admit_three();
        let rejected = full
            .admit(
                context(13),
                issued(1_000, 13, 14),
                DirectCreationFundingV3::ZERO,
                0,
            )
            .unwrap();
        assert_eq!(
            rejected.disposition,
            DirectAdmissionDispositionV3::RejectedNonCompetitive
        );
        assert_eq!(rejected.post, full);
        assert_eq!(rejected.effects, DirectLifecycleEffectsV3::NONE);
        let displaced = account_balance(full.retained[2].account, 0, 9);
        let replacement = full
            .admit(
                context(13),
                issued(5_000, 13, 14),
                DirectCreationFundingV3::ZERO,
                displaced,
            )
            .unwrap();
        assert_eq!(
            replacement.disposition,
            DirectAdmissionDispositionV3::Replaced
        );
        assert_eq!(replacement.post.top_count, 3);
        assert_eq!(
            replacement.effects.payer_principal_total().unwrap(),
            full.retained[2].account.rent.lamports
        );
        assert_eq!(replacement.effects.neutral_surplus_total().unwrap(), 10);
        assert_eq!(
            replacement.post.admit(
                context(14),
                issued(2_000, 14, 11),
                DirectCreationFundingV3::ZERO,
                0
            ),
            Err(DirectLifecycleErrorV3::Replay)
        );
    }

    #[test]
    fn all_sixty_four_verified_ticks_are_bounded_by_one_bitmap() {
        let mut ticks = [0u64; MAX_DIRECT_TICKS_V3 as usize];
        let mut index = 0usize;
        while index < 32 {
            ticks[index] = (index + 1) as u64;
            ticks[32 + index] = PRICE_SCALE - (32 - index) as u64;
            index += 1;
        }
        let grid = DirectGridV3 {
            price_scale: PRICE_SCALE,
            tick_count: MAX_DIRECT_TICKS_V3,
            ticks,
        };
        grid.validate().unwrap();
        let long_schedule = DirectScheduleV3 {
            submission_opens_slot: 10,
            submission_closes_slot: 100,
            selection_deadline_slot: 105,
            settlement_deadline_slot: 107,
        };
        let mut state =
            DirectLifecycleV3::initialize_frozen(long_schedule, authority(), rewards(), funding())
                .unwrap()
                .post;
        let mut candidates = [DirectCandidateLeaseV3::ZERO; MAX_DIRECT_TICKS_V3 as usize];
        let mut candidate_index = 0usize;
        while candidate_index < candidates.len() {
            candidates[candidate_index] = issued_on_grid(
                &grid,
                ticks[candidate_index],
                10,
                (candidate_index + 1) as u8,
            );
            candidate_index += 1;
        }
        let mut unsorted = 1usize;
        while unsorted < candidates.len() {
            let mut cursor = unsorted;
            while cursor > 0
                && candidates[cursor - 1]
                    .score()
                    .is_better_than(&candidates[cursor].score())
            {
                candidates.swap(cursor - 1, cursor);
                cursor -= 1;
            }
            unsorted += 1;
        }
        let mut arrival = 0usize;
        while arrival < candidates.len() {
            let candidate = candidates[arrival];
            let window = if arrival == 0 {
                create(21, 200, 0)
            } else {
                DirectCreationFundingV3::ZERO
            };
            let displaced = if state.top_count as usize == MAX_DIRECT_CANDIDATES {
                account_balance(state.retained[2].account, 0, 0)
            } else {
                0
            };
            state = state
                .admit(context(10), candidate, window, displaced)
                .unwrap()
                .post;
            arrival += 1;
        }
        assert_eq!(state.competitive_admission_count, MAX_DIRECT_TICKS_V3);
        assert_eq!(state.seen_competitive_ticks, u64::MAX);
        assert_eq!(state.top_count, MAX_DIRECT_CANDIDATES as u8);
        assert_eq!(state.live_candidate_mask, 0b111);
    }

    #[test]
    fn hostile_prestate_refuses_before_shift_or_mask() {
        let mut hostile = admit_three();
        hostile.top_count = 8;
        assert_eq!(
            hostile.begin_verification(context(20)),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut hostile = admit_three();
        hostile.verification_mask = 0x80;
        assert_eq!(
            hostile.begin_verification(context(20)),
            Err(DirectLifecycleErrorV3::NonCanonical)
        );
        let mut hostile = admit_three();
        hostile.retained[1].candidate.candidate_id = hostile.retained[0].candidate.candidate_id;
        assert!(hostile.begin_verification(context(20)).is_err());
        let mut hostile = admit_three();
        hostile.retained[2] = DirectCandidateLeaseV3::ZERO;
        assert!(hostile.validate().is_err());
    }

    #[test]
    fn every_selection_stage_checks_deadline_code_and_status_mask() {
        let open = admit_three();
        assert_eq!(
            open.begin_verification(context(19)),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
        let mut wrong_code = context(20);
        wrong_code.verifier_code_identity = id(99);
        assert_eq!(
            open.begin_verification(wrong_code),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        let begun = open.begin_verification(context(20)).unwrap().post;
        let one = begun
            .verify_candidate(context(21), 0, &domain(), &grid(), BUY_LIMIT, SELL_LIMIT)
            .unwrap()
            .post;
        assert_eq!(
            one.verify_candidate(context(22), 0, &domain(), &grid(), BUY_LIMIT, SELL_LIMIT),
            Err(DirectLifecycleErrorV3::Replay)
        );
        assert_eq!(
            one.finalize_selection(
                context(22),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::VerificationIncomplete)
        );
        let mut hostile = verify_all(admit_three());
        hostile.retained[1].stage = DirectCandidateStageV3::Verified;
        assert_eq!(
            hostile.finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::MismatchedBinding)
        );
        assert_eq!(
            verify_all(admit_three()).finalize_selection(
                context(30),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 0),
                    pot: create(51, 400, 0)
                },
                DirectObservedBalancesV3::ZERO
            ),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
    }

    #[test]
    fn finalize_preserves_window_top_and_entitles_reservations() {
        let verified = verify_all(admit_three());
        let original_top = verified.top;
        let original_count = verified.top_count;
        let mut balances = DirectObservedBalancesV3::ZERO;
        balances.candidates[1] = account_balance(verified.retained[1].account, 0, 1);
        balances.candidates[2] = account_balance(verified.retained[2].account, 0, 2);
        let plan = verified
            .finalize_selection(
                context(24),
                DirectSelectionFundingV3 {
                    receipt: create(50, 300, 4),
                    pot: create(51, 400, 5),
                },
                balances,
            )
            .unwrap();
        assert_eq!(plan.post.top_count, original_count);
        assert_eq!(plan.post.top, original_top);
        assert_eq!(plan.post.live_candidate_mask, 1);
        assert_eq!(plan.post.retained[1], DirectCandidateLeaseV3::ZERO);
        assert_eq!(plan.post.retained[2], DirectCandidateLeaseV3::ZERO);
        assert_eq!(plan.effects.close_count, 2);
        assert_eq!(plan.effects.neutral_surplus_total().unwrap(), 5);
        assert_eq!(plan.post.receipt_account.donation_lamports(), Ok(4));
        assert_eq!(plan.post.pot_account.donation_lamports(), Ok(5));
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToEntitled
        );
    }

    #[test]
    fn work_budget_equation_holds_for_partial_verification_lapse() {
        let initial = empty().work_budget_initial_balance;
        let begun = admit_three().begin_verification(context(20)).unwrap();
        let verified = begun
            .post
            .verify_candidate(context(21), 0, &domain(), &grid(), BUY_LIMIT, SELL_LIMIT)
            .unwrap();
        let before_lapse = verified.post;
        let plan = before_lapse.lapse(30, observed(&before_lapse, 0)).unwrap();
        assert_eq!(
            initial,
            begun.effects.keeper_reward
                + verified.effects.keeper_reward
                + plan.effects.keeper_reward
                + plan.effects.work_budget_refund
        );
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToReleased
        );
    }

    #[test]
    fn empty_preselected_and_selected_lapse_are_distinct_and_exact() {
        let empty_state = empty();
        let empty_plan = empty_state.lapse(30, observed(&empty_state, 1)).unwrap();
        assert_eq!(
            empty_plan.post.terminal_receipt.reason,
            DirectTerminalReasonV3::EmptyLapse
        );
        assert_eq!(empty_plan.effects.close_count, 3);
        assert_eq!(
            empty_plan.effects.reservation_transition,
            DirectReservationTransitionV3::ActiveToReleased
        );
        let open = admit_three();
        let pre = open.lapse(30, observed(&open, 1)).unwrap();
        assert_eq!(
            pre.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PreSelectionLapse
        );
        assert_eq!(pre.effects.close_count, 7);
        let selected = selected();
        assert_eq!(
            selected.lapse(39, observed(&selected, 0)),
            Err(DirectLifecycleErrorV3::SelectionWindow)
        );
        let post = selected.lapse(40, observed(&selected, 1)).unwrap();
        assert_eq!(
            post.post.terminal_receipt.reason,
            DirectTerminalReasonV3::PostSelectionLapse
        );
        assert_eq!(
            post.effects.reservation_transition,
            DirectReservationTransitionV3::EntitledToReleased
        );
        assert_eq!(post.effects.close_count, 7);
    }

    #[test]
    fn settlement_derives_exact_economics_and_composite_conservation() {
        let state = selected();
        let initial_budget = state.work_budget_initial_balance;
        assert_eq!(
            state.settle(context(23), observed(&state, 0)),
            Err(DirectLifecycleErrorV3::SettlementClosed)
        );
        assert_eq!(
            state.settle(context(40), observed(&state, 0)),
            Err(DirectLifecycleErrorV3::SettlementClosed)
        );
        let balances = observed(&state, 2);
        let plan = state.settle(context(39), balances).unwrap();
        let receipt = plan.post.terminal_receipt;
        assert_eq!(receipt.reason, DirectTerminalReasonV3::Settled);
        assert_eq!(receipt.quantity, PRICE_SCALE);
        assert_eq!(
            receipt.consideration_price_units,
            u128::from(receipt.quantity) * u128::from(receipt.price)
        );
        assert_eq!(
            plan.effects.reservation_transition,
            DirectReservationTransitionV3::EntitledToConsumed
        );
        assert_eq!(plan.effects.close_count, 7);
        assert_eq!(plan.effects.neutral_lamport_sink, id(81));
        assert_eq!(plan.effects.neutral_surplus_total(), Ok(25));
        assert_eq!(
            plan.effects.observed_close_total().unwrap(),
            plan.effects.payer_principal_total().unwrap()
                + plan.effects.work_budget_refund
                + plan.effects.neutral_surplus_total().unwrap()
        );
        assert_eq!(
            initial_budget,
            state.work_rewards_paid + plan.effects.keeper_reward + plan.effects.work_budget_refund
        );
        let work_leg = plan
            .effects
            .close_legs
            .iter()
            .find(|leg| leg.authority == DirectClosedAuthorityV3::WorkBudget)
            .unwrap();
        assert_eq!(work_leg.payer_principal_lamports, 500);
        assert_eq!(
            balances.work_budget,
            work_leg.observed_live_lamports + plan.effects.keeper_reward
        );
    }

    #[test]
    fn close_balance_shortfall_refuses_without_state() {
        let state = selected();
        let mut balances = observed(&state, 0);
        balances.candidates[0] = state.retained[0].account.rent.lamports - 1;
        assert_eq!(
            state.settle(context(39), balances),
            Err(DirectLifecycleErrorV3::LivenessRefused)
        );
        assert_eq!(state.validate(), Ok(()));
    }
}
