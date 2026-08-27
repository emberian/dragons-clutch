// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure, error-atomic candidate lifecycle transitions.
//!
//! Every public transition takes account values by copy and returns replacement
//! values. An error therefore exposes no partially mutated state. The SBF
//! adapter remains responsible for authenticating identities and evidence,
//! moving lamports, and committing all returned accounts atomically.

use core::cmp::Ordering;

use crate::state::{
    add, live, mul, CandidateEscrowV2, CandidateIndexPageV1, CandidateLifecyclePolicyV2,
    CandidateLivenessPolicyV2, CandidateRecordV2, CandidateStatus, CandidateVerdictV1,
    CandidateWindowV3, EpochCandidateBudgetV2, Error, EscrowFundingState, Id, Interval, RankKey,
    ScorePolicyBindingV1, VerdictKind, CANDIDATES_PER_INDEX_PAGE, MAX_CANDIDATE_INDEX_PAGES,
    TOP_CANDIDATE_CAPACITY,
};

/// Security properties that cannot be established from fixed account bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterObligation {
    AuthenticateClockSysvar,
    DeriveAndAuthenticateIdentityAndPda,
    ProveFreshCanonicalAdmissionAccounts,
    AuthorizeCopyResistantAdmissionAndRewardDestination,
    DeriveCanonicalFeedGeometryWorkUnitsAndRent,
    AuthenticateCompleteFeedAndDigest,
    ExecuteRelationPolicyAndDeriveOutcome,
    ExecuteScorePolicyAndDeriveRankKey,
    AuthenticateWorkCheckpointAndClosure,
    AuthenticateSelectedSettlementTerminal,
    RouteUnsolicitedSurplusToNeutralSink,
    MoveLamportsExactly,
    CommitReturnedAccountsAtomically,
    MirrorEpochTerminalState,
    CloseOwnedAccountsAndRefundRent,
}

/// Known blockers intentionally preserved in the public interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionBlocker {
    CopyFrontRunningAdmissionDesign,
    QualityCapacityDenialOfService,
    SbfAdapterNotConnected,
    EpochRootRetirementNotImplemented,
    GlobalAccountTagMappingNotReserved,
}

pub const ADAPTER_OBLIGATIONS: [AdapterObligation; 15] = [
    AdapterObligation::AuthenticateClockSysvar,
    AdapterObligation::DeriveAndAuthenticateIdentityAndPda,
    AdapterObligation::ProveFreshCanonicalAdmissionAccounts,
    AdapterObligation::AuthorizeCopyResistantAdmissionAndRewardDestination,
    AdapterObligation::DeriveCanonicalFeedGeometryWorkUnitsAndRent,
    AdapterObligation::AuthenticateCompleteFeedAndDigest,
    AdapterObligation::ExecuteRelationPolicyAndDeriveOutcome,
    AdapterObligation::ExecuteScorePolicyAndDeriveRankKey,
    AdapterObligation::AuthenticateWorkCheckpointAndClosure,
    AdapterObligation::AuthenticateSelectedSettlementTerminal,
    AdapterObligation::RouteUnsolicitedSurplusToNeutralSink,
    AdapterObligation::MoveLamportsExactly,
    AdapterObligation::CommitReturnedAccountsAtomically,
    AdapterObligation::MirrorEpochTerminalState,
    AdapterObligation::CloseOwnedAccountsAndRefundRent,
];

pub const PROMOTION_BLOCKERS: [PromotionBlocker; 5] = [
    PromotionBlocker::CopyFrontRunningAdmissionDesign,
    PromotionBlocker::QualityCapacityDenialOfService,
    PromotionBlocker::SbfAdapterNotConnected,
    PromotionBlocker::EpochRootRetirementNotImplemented,
    PromotionBlocker::GlobalAccountTagMappingNotReserved,
];

fn bind_budget(
    budget: EpochCandidateBudgetV2,
    epoch: Id,
    liveness: CandidateLivenessPolicyV2,
) -> Result<(), Error> {
    budget.validate()?;
    liveness.validate()?;
    if budget.epoch != epoch
        || budget.liveness_policy_id != liveness.policy_id
        || budget.neutral_sink != liveness.neutral_sink
        || budget.freeze_initial != liveness.freeze_reward
        || budget.finalizer_initial != liveness.finalizer_reward
        || budget.index_cleanup_initial != liveness.index_cleanup_reserve()?
        || budget.solver_initial != liveness.solver_prize
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn bind_escrow(
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<(), Error> {
    candidate.validate()?;
    escrow.validate()?;
    liveness.validate()?;
    if escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != liveness.policy_id
        || escrow.neutral_sink != liveness.neutral_sink
        || candidate.liveness_policy_id != liveness.policy_id
        || escrow.bond_initial != liveness.bond_lamports
        || escrow.cleanup_initial != liveness.candidate_cleanup_reserve()?
        || (escrow.solver_credited != 0 && escrow.solver_credited != liveness.solver_prize)
    {
        return Err(Error::MismatchedBinding);
    }
    let sealed_funding = match candidate.status {
        CandidateStatus::Staging | CandidateStatus::ExpiredStaging => false,
        CandidateStatus::Sealed
        | CandidateStatus::Verdicted
        | CandidateStatus::ExpiredUnverified => true,
    };
    if sealed_funding != (escrow.funding_state == EscrowFundingState::Sealed) {
        return Err(Error::MismatchedBinding);
    }
    if sealed_funding
        && (escrow.work_initial != liveness.work_reserve(candidate.verification_units)?
            || escrow.total_units != candidate.verification_units)
    {
        return Err(Error::MismatchedBinding);
    }
    let expiry_paid = match candidate.status {
        CandidateStatus::ExpiredStaging | CandidateStatus::ExpiredUnverified => {
            liveness.expiry_reward
        }
        CandidateStatus::Staging | CandidateStatus::Sealed | CandidateStatus::Verdicted => 0,
    };
    let expected_cleanup_paid = if escrow.candidate_closed == 1 {
        add(expiry_paid, liveness.candidate_close_reward)?
    } else {
        expiry_paid
    };
    if escrow.cleanup_paid != expected_cleanup_paid {
        return Err(Error::MismatchedBinding);
    }
    match candidate.status {
        CandidateStatus::Staging | CandidateStatus::Sealed => {
            if escrow.bond_slashed != 0
                || escrow.bond_refund_claimed != 0
                || escrow.work_refund_claimed != 0
                || escrow.cleanup_finalized != 0
                || escrow.solver_credited != 0
                || escrow.solver_credit_claimed != 0
                || escrow.work_closed != 0
                || escrow.candidate_closed != 0
            {
                return Err(Error::MismatchedBinding);
            }
        }
        CandidateStatus::ExpiredUnverified => {
            if escrow.bond_slashed != 0 {
                return Err(Error::MismatchedBinding);
            }
            if escrow.solver_credited != 0 || escrow.solver_credit_claimed != 0 {
                return Err(Error::MismatchedBinding);
            }
        }
        CandidateStatus::ExpiredStaging => {
            if escrow.bond_slashed != liveness.abandonment_penalty
                || escrow.work_closed != 0
                || escrow.work_refund_claimed != 0
                || escrow.solver_credited != 0
                || escrow.solver_credit_claimed != 0
            {
                return Err(Error::MismatchedBinding);
            }
        }
        CandidateStatus::Verdicted => {
            if escrow.paid_units != escrow.total_units || escrow.work_remaining != 0 {
                return Err(Error::MismatchedBinding);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LamportDispositionV1 {
    pub keeper_reward: u64,
    pub neutral_sink: u64,
    pub refund_destination_credit: u64,
    /// Epoch budget transfer into the selected candidate escrow.
    pub solver_escrow_credit: u64,
    /// Candidate escrow payout to the immutable solver reward destination.
    pub solver_payout: u64,
    pub rent_principal_refund: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBudgetAdmissionV2 {
    pub epoch: Id,
    pub sponsor: Id,
    pub refund_destination: Id,
    pub account_rent_principal: u64,
    /// Exact total rent principal for all four pre-created index pages.
    pub index_page_rent_principal: u64,
    pub budget_bump: u8,
}

pub fn admit_epoch_budget(
    input: EpochBudgetAdmissionV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<EpochCandidateBudgetV2, Error> {
    liveness.validate()?;
    for id in [input.epoch, input.sponsor, input.refund_destination] {
        live(id)?;
    }
    let page_count =
        u64::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?;
    if input.account_rent_principal == 0
        || input.index_page_rent_principal == 0
        || !input.index_page_rent_principal.is_multiple_of(page_count)
    {
        return Err(Error::InvalidState);
    }
    let value = EpochCandidateBudgetV2 {
        epoch: input.epoch,
        sponsor: input.sponsor,
        refund_destination: input.refund_destination,
        neutral_sink: liveness.neutral_sink,
        liveness_policy_id: liveness.policy_id,
        account_rent_principal: input.account_rent_principal,
        index_page_rent_principal: input.index_page_rent_principal,
        freeze_initial: liveness.freeze_reward,
        freeze_remaining: liveness.freeze_reward,
        freeze_paid: 0,
        finalizer_initial: liveness.finalizer_reward,
        finalizer_remaining: liveness.finalizer_reward,
        finalizer_paid: 0,
        index_cleanup_initial: liveness.index_cleanup_reserve()?,
        index_cleanup_remaining: liveness.index_cleanup_reserve()?,
        index_cleanup_paid: 0,
        index_cleanup_refunded: 0,
        solver_initial: liveness.solver_prize,
        solver_remaining: liveness.solver_prize,
        solver_credited: 0,
        solver_refunded: 0,
        surplus_routed: 0,
        index_pages_owed: 0,
        terminalized: 0,
        refund_claimed: 0,
        stored_bump: input.budget_bump,
        flags: 0,
    };
    value.validate()?;
    Ok(value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeTransitionV2 {
    pub window: CandidateWindowV3,
    pub budget: EpochCandidateBudgetV2,
    pub disposition: LamportDispositionV1,
}

pub fn freeze_window(
    window: CandidateWindowV3,
    budget: EpochCandidateBudgetV2,
    lifecycle: CandidateLifecyclePolicyV2,
    score: ScorePolicyBindingV1,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<FreezeTransitionV2, Error> {
    window.bind_policies(lifecycle, score, liveness)?;
    bind_budget(budget, window.epoch, liveness)?;
    if budget.liveness_policy_id != window.liveness_policy_id {
        return Err(Error::MismatchedBinding);
    }
    if window.frozen_slot != 0 {
        return Err(Error::Replay);
    }
    if now_slot < window.freeze_deadline_slot {
        return Err(Error::NotActive);
    }
    if budget.freeze_remaining != liveness.freeze_reward {
        return Err(Error::Underfunded);
    }
    let schedule = crate::state::Schedule::stamp(
        now_slot,
        lifecycle.submission_span_slots,
        lifecycle.verification_span_slots,
    )?;
    let mut next_window = window;
    let mut next_budget = budget;
    next_window.frozen_slot = schedule.frozen_slot;
    next_window.submission_closes_slot = schedule.submission_closes_slot;
    next_window.verification_closes_slot = schedule.verification_closes_slot;
    next_budget.freeze_remaining = 0;
    next_budget.freeze_paid = add(next_budget.freeze_paid, liveness.freeze_reward)?;
    next_window.validate()?;
    bind_budget(next_budget, next_window.epoch, liveness)?;
    Ok(FreezeTransitionV2 {
        window: next_window,
        budget: next_budget,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.freeze_reward,
            ..LamportDispositionV1::default()
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginCandidateInputV2 {
    pub candidate: Id,
    pub solver: Id,
    pub solver_reward_destination: Id,
    pub feed: Id,
    pub payer: Id,
    pub refund_destination: Id,
    pub expected_feed_bytes: u32,
    pub verification_units: u16,
    pub staging_rent_principal: u64,
    pub candidate_bump: u8,
    pub escrow_bump: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginCandidateTransitionV2 {
    pub window: CandidateWindowV3,
    pub index_page: CandidateIndexPageV1,
    pub candidate: CandidateRecordV2,
    pub escrow: CandidateEscrowV2,
}

pub fn begin_candidate(
    window: CandidateWindowV3,
    index_page: CandidateIndexPageV1,
    input: BeginCandidateInputV2,
    lifecycle: CandidateLifecyclePolicyV2,
    score: ScorePolicyBindingV1,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<BeginCandidateTransitionV2, Error> {
    window.bind_policies(lifecycle, score, liveness)?;
    index_page.validate()?;
    if window.is_finalized() || window.schedule()?.interval(now_slot)? != Interval::Submission {
        return Err(Error::NotActive);
    }
    if window.begun_candidate_count >= lifecycle.max_begun_candidates {
        return Err(Error::CapacityReached);
    }
    if input.verification_units == 0
        || input.verification_units > lifecycle.max_verification_units
        || input.expected_feed_bytes == 0
        || input.expected_feed_bytes > lifecycle.max_feed_bytes
        || input.staging_rent_principal == 0
    {
        return Err(Error::InvalidCount);
    }
    let ordinal = usize::from(window.begun_candidate_count);
    let expected_page = ordinal / CANDIDATES_PER_INDEX_PAGE;
    let expected_offset = ordinal % CANDIDATES_PER_INDEX_PAGE;
    if index_page.epoch != window.epoch
        || usize::from(index_page.page_index) != expected_page
        || usize::from(index_page.count) != expected_offset
        || (expected_offset == 0 && usize::from(window.candidate_page_count) != expected_page)
        || (expected_offset != 0 && usize::from(window.candidate_page_count) != expected_page + 1)
    {
        return Err(Error::MismatchedBinding);
    }
    let cleanup = liveness.candidate_cleanup_reserve()?;
    let record = CandidateRecordV2 {
        candidate: input.candidate,
        epoch: window.epoch,
        market: window.market,
        relation_policy_id: window.relation_policy_id,
        lifecycle_policy_id: window.lifecycle_policy_id,
        score_policy_id: window.score_policy_id,
        liveness_policy_id: window.liveness_policy_id,
        solver: input.solver,
        solver_reward_destination: input.solver_reward_destination,
        feed: input.feed,
        feed_content_digest: Id::ZERO,
        verdict: Id::ZERO,
        begun_slot: now_slot,
        sealed_slot: 0,
        terminal_slot: 0,
        expected_feed_bytes: input.expected_feed_bytes,
        verification_units: input.verification_units,
        index_ordinal: window.begun_candidate_count,
        status: CandidateStatus::Staging,
        stored_bump: input.candidate_bump,
        flags: 0,
    };
    let escrow = CandidateEscrowV2 {
        candidate: input.candidate,
        payer: input.payer,
        refund_destination: input.refund_destination,
        neutral_sink: liveness.neutral_sink,
        liveness_policy_id: liveness.policy_id,
        staging_rent_principal: input.staging_rent_principal,
        verification_rent_principal: 0,
        work_initial: 0,
        work_remaining: 0,
        work_paid: 0,
        work_refunded: 0,
        bond_initial: liveness.bond_lamports,
        bond_remaining: liveness.bond_lamports,
        bond_slashed: 0,
        bond_refunded: 0,
        cleanup_initial: cleanup,
        cleanup_remaining: cleanup,
        cleanup_paid: 0,
        cleanup_refunded: 0,
        solver_credited: 0,
        solver_remaining: 0,
        solver_paid: 0,
        surplus_routed: 0,
        paid_units: 0,
        total_units: 0,
        funding_state: EscrowFundingState::Staging,
        bond_refund_claimed: 0,
        work_refund_claimed: 0,
        cleanup_finalized: 0,
        solver_credit_claimed: 0,
        work_closed: 0,
        candidate_closed: 0,
        stored_bump: input.escrow_bump,
        flags: 0,
    };
    record.validate()?;
    bind_escrow(record, escrow, liveness)?;

    let mut next_window = window;
    let mut next_page = index_page;
    next_page.candidates[expected_offset] = input.candidate;
    next_page.count = next_page
        .count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next_window.begun_candidate_count = next_window
        .begun_candidate_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if expected_offset == 0 {
        next_window.candidate_page_count = next_window
            .candidate_page_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    next_page.validate()?;
    next_window.validate()?;
    Ok(BeginCandidateTransitionV2 {
        window: next_window,
        index_page: next_page,
        candidate: record,
        escrow,
    })
}

/// Adapter attestation for an exact, complete, canonically padded feed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedSealV1 {
    pub candidate: Id,
    pub epoch: Id,
    pub feed: Id,
    pub content_digest: Id,
    pub exact_bytes: u32,
    pub written_bytes: u32,
    pub canonical_padding: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealFundingV1 {
    pub verification_rent_principal: u64,
    pub work_reward_deposit: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealCandidateTransitionV2 {
    pub window: CandidateWindowV3,
    pub candidate: CandidateRecordV2,
    pub escrow: CandidateEscrowV2,
}

#[allow(clippy::too_many_arguments)]
pub fn seal_candidate(
    window: CandidateWindowV3,
    index_page: CandidateIndexPageV1,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    feed: FeedSealV1,
    funding: SealFundingV1,
    lifecycle: CandidateLifecyclePolicyV2,
    score: ScorePolicyBindingV1,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<SealCandidateTransitionV2, Error> {
    window.bind_policies(lifecycle, score, liveness)?;
    candidate.bind_window(window)?;
    index_page.bind_candidate(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    if window.is_finalized() || window.schedule()?.interval(now_slot)? != Interval::Submission {
        return Err(Error::NotActive);
    }
    if candidate.expected_feed_bytes > lifecycle.max_feed_bytes
        || candidate.verification_units > lifecycle.max_verification_units
    {
        return Err(Error::InvalidCount);
    }
    if candidate.status != CandidateStatus::Staging
        || escrow.funding_state != EscrowFundingState::Staging
        || escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != candidate.liveness_policy_id
        || feed.candidate != candidate.candidate
        || feed.epoch != candidate.epoch
        || feed.feed != candidate.feed
        || feed.content_digest.is_zero()
        || feed.exact_bytes != candidate.expected_feed_bytes
        || feed.written_bytes != feed.exact_bytes
        || feed.canonical_padding != 1
    {
        return Err(Error::MismatchedBinding);
    }
    let expected_work = liveness.work_reserve(candidate.verification_units)?;
    if funding.verification_rent_principal == 0 || funding.work_reward_deposit != expected_work {
        return Err(Error::Underfunded);
    }
    let mut next_window = window;
    let mut next_candidate = candidate;
    let mut next_escrow = escrow;
    next_candidate.feed_content_digest = feed.content_digest;
    next_candidate.sealed_slot = now_slot;
    next_candidate.status = CandidateStatus::Sealed;
    next_escrow.verification_rent_principal = funding.verification_rent_principal;
    next_escrow.work_initial = expected_work;
    next_escrow.work_remaining = expected_work;
    next_escrow.total_units = candidate.verification_units;
    next_escrow.funding_state = EscrowFundingState::Sealed;
    next_window.sealed_candidate_count = next_window
        .sealed_candidate_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next_candidate.validate()?;
    bind_escrow(next_candidate, next_escrow, liveness)?;
    next_window.validate()?;
    Ok(SealCandidateTransitionV2 {
        window: next_window,
        candidate: next_candidate,
        escrow: next_escrow,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressTransitionV2 {
    pub escrow: CandidateEscrowV2,
    pub disposition: LamportDispositionV1,
}

pub fn pay_verification_progress(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
    prior_units: u16,
    new_units: u16,
    now_slot: u64,
) -> Result<ProgressTransitionV2, Error> {
    candidate.bind_window(window)?;
    bind_escrow(candidate, escrow, liveness)?;
    if window.liveness_policy_id != liveness.policy_id
        || escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != liveness.policy_id
    {
        return Err(Error::MismatchedBinding);
    }
    if window.is_finalized()
        || window.schedule()?.interval(now_slot)? != Interval::Verification
        || candidate.status != CandidateStatus::Sealed
    {
        return Err(Error::NotActive);
    }
    if prior_units != escrow.paid_units
        || new_units <= prior_units
        || new_units > escrow.total_units
        || escrow.total_units != candidate.verification_units
    {
        return Err(Error::Replay);
    }
    let delta = new_units
        .checked_sub(prior_units)
        .ok_or(Error::ArithmeticOverflow)?;
    let reward = mul(liveness.progress_reward_per_unit, u64::from(delta))?;
    if escrow.work_remaining < reward {
        return Err(Error::Underfunded);
    }
    let mut next = escrow;
    next.work_remaining = next
        .work_remaining
        .checked_sub(reward)
        .ok_or(Error::ArithmeticOverflow)?;
    next.work_paid = add(next.work_paid, reward)?;
    next.paid_units = new_units;
    bind_escrow(candidate, next, liveness)?;
    Ok(ProgressTransitionV2 {
        escrow: next,
        disposition: LamportDispositionV1 {
            keeper_reward: reward,
            ..LamportDispositionV1::default()
        },
    })
}

/// Outcome already checked by the relation and score adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterVerifiedOutcomeV1 {
    pub verdict: Id,
    pub relation_digest: Id,
    pub kind: VerdictKind,
    pub refusal_code: u16,
    pub rank_key: RankKey,
    pub verdict_bump: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteVerificationTransitionV2 {
    pub window: CandidateWindowV3,
    pub candidate: CandidateRecordV2,
    pub escrow: CandidateEscrowV2,
    pub verdict: CandidateVerdictV1,
    pub disposition: LamportDispositionV1,
}

fn validate_top(
    window: CandidateWindowV3,
    top: [CandidateVerdictV1; TOP_CANDIDATE_CAPACITY],
) -> Result<(), Error> {
    let mut index = 0usize;
    while index < TOP_CANDIDATE_CAPACITY {
        if index < usize::from(window.top_count) {
            let verdict = top[index];
            verdict.validate()?;
            if verdict.kind != VerdictKind::Valid
                || verdict.candidate != window.top_candidates[index]
                || verdict.epoch != window.epoch
                || verdict.score_policy_id != window.score_policy_id
                || verdict.rank_key.len() != window.rank_key_len
            {
                return Err(Error::MismatchedBinding);
            }
            if index > 0 {
                match top[index - 1].rank_key.compare(verdict.rank_key)? {
                    Ordering::Greater => {}
                    Ordering::Equal => return Err(Error::RankCollision),
                    Ordering::Less => return Err(Error::MismatchedBinding),
                }
            }
        } else if !top[index].is_empty() {
            return Err(Error::InvalidState);
        }
        index += 1;
    }
    Ok(())
}

fn insert_top(
    window: CandidateWindowV3,
    prior: [CandidateVerdictV1; TOP_CANDIDATE_CAPACITY],
    verdict: CandidateVerdictV1,
) -> Result<[Id; TOP_CANDIDATE_CAPACITY], Error> {
    validate_top(window, prior)?;
    let prior_count = usize::from(window.top_count);
    let mut values = [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY + 1];
    let mut index = 0usize;
    while index < prior_count {
        if prior[index].candidate == verdict.candidate || prior[index].verdict == verdict.verdict {
            return Err(Error::DuplicateIdentity);
        }
        values[index] = prior[index];
        index += 1;
    }
    values[prior_count] = verdict;
    let active = prior_count + 1;
    let mut cursor = active - 1;
    while cursor > 0 {
        match values[cursor - 1]
            .rank_key
            .compare(values[cursor].rank_key)?
        {
            Ordering::Greater => break,
            Ordering::Equal => return Err(Error::RankCollision),
            Ordering::Less => values.swap(cursor - 1, cursor),
        }
        cursor -= 1;
    }
    let mut output = [Id::ZERO; TOP_CANDIDATE_CAPACITY];
    let retained = core::cmp::min(active, TOP_CANDIDATE_CAPACITY);
    index = 0;
    while index < retained {
        output[index] = values[index].candidate;
        index += 1;
    }
    Ok(output)
}

pub fn complete_verification(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    prior_top: [CandidateVerdictV1; TOP_CANDIDATE_CAPACITY],
    outcome: AdapterVerifiedOutcomeV1,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<CompleteVerificationTransitionV2, Error> {
    candidate.bind_window(window)?;
    bind_escrow(candidate, escrow, liveness)?;
    if window.liveness_policy_id != liveness.policy_id
        || escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != liveness.policy_id
    {
        return Err(Error::MismatchedBinding);
    }
    if window.is_finalized()
        || window.schedule()?.interval(now_slot)? != Interval::Verification
        || candidate.status != CandidateStatus::Sealed
    {
        return Err(Error::NotActive);
    }
    if escrow.paid_units != candidate.verification_units
        || escrow.total_units != candidate.verification_units
        || escrow.work_remaining != liveness.completion_reward
    {
        return Err(Error::UnresolvedCandidates);
    }
    let verdict = CandidateVerdictV1 {
        verdict: outcome.verdict,
        candidate: candidate.candidate,
        epoch: candidate.epoch,
        relation_digest: outcome.relation_digest,
        score_policy_id: candidate.score_policy_id,
        rank_key: outcome.rank_key,
        verified_slot: now_slot,
        refusal_code: outcome.refusal_code,
        kind: outcome.kind,
        stored_bump: outcome.verdict_bump,
        flags: 0,
    };
    verdict.validate()?;
    if verdict.kind == VerdictKind::Valid && verdict.rank_key.len() != window.rank_key_len {
        return Err(Error::MismatchedBinding);
    }

    let mut next_window = window;
    let mut next_candidate = candidate;
    let mut next_escrow = escrow;
    next_escrow.work_remaining = 0;
    next_escrow.work_paid = add(next_escrow.work_paid, liveness.completion_reward)?;
    next_candidate.status = CandidateStatus::Verdicted;
    next_candidate.terminal_slot = now_slot;
    next_candidate.verdict = verdict.verdict;
    next_window.verdict_count = next_window
        .verdict_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;

    let mut penalty = 0u64;
    if verdict.kind == VerdictKind::Valid {
        next_window.valid_verdict_count = next_window
            .valid_verdict_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next_window.top_candidates = insert_top(window, prior_top, verdict)?;
        next_window.top_count = u8::try_from(core::cmp::min(
            usize::from(next_window.valid_verdict_count),
            TOP_CANDIDATE_CAPACITY,
        ))
        .map_err(|_| Error::ArithmeticOverflow)?;
    } else {
        if next_escrow.bond_remaining < liveness.invalidity_penalty {
            return Err(Error::Underfunded);
        }
        next_escrow.bond_remaining = next_escrow
            .bond_remaining
            .checked_sub(liveness.invalidity_penalty)
            .ok_or(Error::ArithmeticOverflow)?;
        next_escrow.bond_slashed = add(next_escrow.bond_slashed, liveness.invalidity_penalty)?;
        penalty = liveness.invalidity_penalty;
    }
    next_window.validate()?;
    next_candidate.validate()?;
    bind_escrow(next_candidate, next_escrow, liveness)?;
    verdict.bind_candidate(next_candidate, next_window)?;
    Ok(CompleteVerificationTransitionV2 {
        window: next_window,
        candidate: next_candidate,
        escrow: next_escrow,
        verdict,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.completion_reward,
            neutral_sink: penalty,
            ..LamportDispositionV1::default()
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeSelectionTransitionV2 {
    pub window: CandidateWindowV3,
    pub budget: EpochCandidateBudgetV2,
    pub winner_escrow: Option<CandidateEscrowV2>,
    pub disposition: LamportDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinnerFundingV2 {
    pub candidate: CandidateRecordV2,
    pub escrow: CandidateEscrowV2,
}

pub fn finalize_selection(
    window: CandidateWindowV3,
    budget: EpochCandidateBudgetV2,
    top: [CandidateVerdictV1; TOP_CANDIDATE_CAPACITY],
    winner_funding: Option<WinnerFundingV2>,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<FinalizeSelectionTransitionV2, Error> {
    window.validate()?;
    bind_budget(budget, window.epoch, liveness)?;
    validate_top(window, top)?;
    if window.is_finalized() {
        return Err(Error::Replay);
    }
    let schedule = window.schedule()?;
    if now_slot < schedule.submission_closes_slot {
        return Err(Error::NotActive);
    }
    if now_slot < schedule.verification_closes_slot
        && window.verdict_count != window.sealed_candidate_count
    {
        return Err(Error::UnresolvedCandidates);
    }
    if budget.epoch != window.epoch
        || budget.liveness_policy_id != liveness.policy_id
        || budget.terminalized != 0
        || budget.freeze_remaining != 0
        || budget.finalizer_remaining != liveness.finalizer_reward
    {
        return Err(Error::MismatchedBinding);
    }
    let selected = if window.top_count == 0 {
        Id::ZERO
    } else {
        top[0].candidate
    };
    let mut next_winner = None;
    if selected.is_zero() {
        if winner_funding.is_some() {
            return Err(Error::MismatchedBinding);
        }
    } else {
        let funding = winner_funding.ok_or(Error::MismatchedBinding)?;
        funding.candidate.bind_window(window)?;
        top[0].bind_candidate(funding.candidate, window)?;
        bind_escrow(funding.candidate, funding.escrow, liveness)?;
        let mut escrow = funding.escrow;
        if funding.candidate.candidate != selected
            || escrow.solver_credited != 0
            || escrow.solver_remaining != 0
            || escrow.solver_paid != 0
            || escrow.solver_credit_claimed != 0
            || escrow.candidate_closed != 0
        {
            return Err(Error::MismatchedBinding);
        }
        if budget.solver_remaining < liveness.solver_prize {
            return Err(Error::Underfunded);
        }
        escrow.solver_credited = liveness.solver_prize;
        escrow.solver_remaining = liveness.solver_prize;
        bind_escrow(funding.candidate, escrow, liveness)?;
        next_winner = Some(escrow);
    }

    let mut next_window = window;
    let mut next_budget = budget;
    next_window.finalized_slot = now_slot;
    next_window.selected_candidate = selected;
    next_budget.finalizer_remaining = 0;
    next_budget.finalizer_paid = add(next_budget.finalizer_paid, liveness.finalizer_reward)?;
    if !selected.is_zero() {
        next_budget.solver_remaining = next_budget
            .solver_remaining
            .checked_sub(liveness.solver_prize)
            .ok_or(Error::ArithmeticOverflow)?;
        next_budget.solver_credited = add(next_budget.solver_credited, liveness.solver_prize)?;
    }
    next_budget.terminalized = 1;
    next_budget.index_pages_owed =
        u8::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?;
    next_window.validate()?;
    bind_budget(next_budget, next_window.epoch, liveness)?;
    Ok(FinalizeSelectionTransitionV2 {
        window: next_window,
        budget: next_budget,
        winner_escrow: next_winner,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.finalizer_reward,
            solver_escrow_credit: if selected.is_zero() {
                0
            } else {
                liveness.solver_prize
            },
            ..LamportDispositionV1::default()
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpireCandidateTransitionV2 {
    pub window: CandidateWindowV3,
    pub candidate: CandidateRecordV2,
    pub escrow: CandidateEscrowV2,
    pub disposition: LamportDispositionV1,
}

pub fn expire_candidate(
    window: CandidateWindowV3,
    index_page: CandidateIndexPageV1,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
    now_slot: u64,
) -> Result<ExpireCandidateTransitionV2, Error> {
    candidate.bind_window(window)?;
    index_page.bind_candidate(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    if escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != liveness.policy_id
        || window.liveness_policy_id != liveness.policy_id
    {
        return Err(Error::MismatchedBinding);
    }
    let schedule = window.schedule()?;
    let (next_status, penalty) = match candidate.status {
        CandidateStatus::Staging if now_slot >= schedule.submission_closes_slot => (
            CandidateStatus::ExpiredStaging,
            liveness.abandonment_penalty,
        ),
        CandidateStatus::Sealed if now_slot >= schedule.verification_closes_slot => {
            (CandidateStatus::ExpiredUnverified, 0)
        }
        CandidateStatus::ExpiredStaging
        | CandidateStatus::ExpiredUnverified
        | CandidateStatus::Verdicted => return Err(Error::Replay),
        CandidateStatus::Staging | CandidateStatus::Sealed => return Err(Error::NotActive),
    };
    if escrow.cleanup_remaining < liveness.expiry_reward || escrow.bond_remaining < penalty {
        return Err(Error::Underfunded);
    }
    let mut next_window = window;
    let mut next_candidate = candidate;
    let mut next_escrow = escrow;
    next_candidate.status = next_status;
    next_candidate.terminal_slot = now_slot;
    next_escrow.cleanup_remaining = next_escrow
        .cleanup_remaining
        .checked_sub(liveness.expiry_reward)
        .ok_or(Error::ArithmeticOverflow)?;
    next_escrow.cleanup_paid = add(next_escrow.cleanup_paid, liveness.expiry_reward)?;
    if penalty != 0 {
        next_escrow.bond_remaining = next_escrow
            .bond_remaining
            .checked_sub(penalty)
            .ok_or(Error::ArithmeticOverflow)?;
        next_escrow.bond_slashed = add(next_escrow.bond_slashed, penalty)?;
    }
    match next_status {
        CandidateStatus::ExpiredStaging => {
            next_window.expired_staging_count = next_window
                .expired_staging_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        CandidateStatus::ExpiredUnverified => {
            next_window.expired_unverified_count = next_window
                .expired_unverified_count
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        CandidateStatus::Staging | CandidateStatus::Sealed | CandidateStatus::Verdicted => {
            return Err(Error::InvalidState);
        }
    }
    next_window.validate()?;
    next_candidate.validate()?;
    bind_escrow(next_candidate, next_escrow, liveness)?;
    Ok(ExpireCandidateTransitionV2 {
        window: next_window,
        candidate: next_candidate,
        escrow: next_escrow,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.expiry_reward,
            neutral_sink: penalty,
            ..LamportDispositionV1::default()
        },
    })
}

fn terminal(candidate: CandidateRecordV2) -> Result<(), Error> {
    candidate.validate()?;
    match candidate.status {
        CandidateStatus::Verdicted
        | CandidateStatus::ExpiredStaging
        | CandidateStatus::ExpiredUnverified => Ok(()),
        CandidateStatus::Staging | CandidateStatus::Sealed => Err(Error::NotActive),
    }
}

fn validate_terminal_slash(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    verdict: Option<CandidateVerdictV1>,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<(), Error> {
    let expected_slash = match candidate.status {
        CandidateStatus::Verdicted => {
            let checked = verdict.ok_or(Error::MismatchedBinding)?;
            checked.bind_candidate(candidate, window)?;
            match checked.kind {
                VerdictKind::Valid => 0,
                VerdictKind::Refused => liveness.invalidity_penalty,
            }
        }
        CandidateStatus::ExpiredStaging => {
            if verdict.is_some() {
                return Err(Error::MismatchedBinding);
            }
            liveness.abandonment_penalty
        }
        CandidateStatus::ExpiredUnverified => {
            if verdict.is_some() {
                return Err(Error::MismatchedBinding);
            }
            0
        }
        CandidateStatus::Staging | CandidateStatus::Sealed => return Err(Error::NotActive),
    };
    if escrow.bond_slashed != expected_slash {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EscrowClaimTransitionV2 {
    pub escrow: CandidateEscrowV2,
    pub disposition: LamportDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCleanupTransitionV2 {
    pub index_page: CandidateIndexPageV1,
    pub escrow: CandidateEscrowV2,
    pub disposition: LamportDispositionV1,
}

/// Adapter-authenticated evidence that the selected witness is no longer
/// required by settlement. Unselected candidates must not supply it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterVerifiedSettlementTerminalV1 {
    pub epoch: Id,
    pub candidate: Id,
    pub terminal_slot: u64,
    pub flags: u8,
}

pub fn claim_bond_refund(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    verdict: Option<CandidateVerdictV1>,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<EscrowClaimTransitionV2, Error> {
    candidate.bind_window(window)?;
    terminal(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    validate_terminal_slash(window, candidate, verdict, escrow, liveness)?;
    if escrow.bond_refund_claimed != 0 {
        return Err(Error::Replay);
    }
    let refund = escrow.bond_remaining;
    let mut next = escrow;
    next.bond_remaining = 0;
    next.bond_refunded = add(next.bond_refunded, refund)?;
    next.bond_refund_claimed = 1;
    bind_escrow(candidate, next, liveness)?;
    Ok(EscrowClaimTransitionV2 {
        escrow: next,
        disposition: LamportDispositionV1 {
            refund_destination_credit: refund,
            ..LamportDispositionV1::default()
        },
    })
}

pub fn mark_work_closed(
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
    observed_paid_units: u16,
) -> Result<CandidateEscrowV2, Error> {
    terminal(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    if escrow.candidate != candidate.candidate
        || escrow.funding_state != EscrowFundingState::Sealed
        || candidate.status == CandidateStatus::ExpiredStaging
    {
        return Err(Error::MismatchedBinding);
    }
    if escrow.work_closed != 0 {
        return Err(Error::Replay);
    }
    if observed_paid_units != escrow.paid_units {
        return Err(Error::MismatchedBinding);
    }
    let mut next = escrow;
    next.work_closed = 1;
    bind_escrow(candidate, next, liveness)?;
    Ok(next)
}

pub fn claim_work_refund(
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<EscrowClaimTransitionV2, Error> {
    terminal(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    if escrow.candidate != candidate.candidate || escrow.work_closed != 1 {
        return Err(Error::NotActive);
    }
    if escrow.work_refund_claimed != 0 {
        return Err(Error::Replay);
    }
    let refund = escrow.work_remaining;
    let mut next = escrow;
    next.work_remaining = 0;
    next.work_refunded = add(next.work_refunded, refund)?;
    next.work_refund_claimed = 1;
    bind_escrow(candidate, next, liveness)?;
    Ok(EscrowClaimTransitionV2 {
        escrow: next,
        disposition: LamportDispositionV1 {
            refund_destination_credit: refund,
            ..LamportDispositionV1::default()
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub fn finish_candidate_cleanup(
    window: CandidateWindowV3,
    index_page: CandidateIndexPageV1,
    candidate: CandidateRecordV2,
    verdict: Option<CandidateVerdictV1>,
    settlement_terminal: Option<AdapterVerifiedSettlementTerminalV1>,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
    observed_owned_lamports: u64,
) -> Result<CandidateCleanupTransitionV2, Error> {
    candidate.bind_window(window)?;
    index_page.bind_candidate(candidate)?;
    terminal(candidate)?;
    bind_escrow(candidate, escrow, liveness)?;
    validate_terminal_slash(window, candidate, verdict, escrow, liveness)?;
    if !window.is_finalized() {
        return Err(Error::NotActive);
    }
    if window.selected_candidate == candidate.candidate {
        let selected_verdict = verdict.ok_or(Error::MismatchedBinding)?;
        if selected_verdict.kind != VerdictKind::Valid {
            return Err(Error::MismatchedBinding);
        }
        let solver_claim_is_exact = if liveness.solver_prize == 0 {
            escrow.solver_credited == 0
                && escrow.solver_remaining == 0
                && escrow.solver_paid == 0
                && escrow.solver_credit_claimed == 0
        } else {
            escrow.solver_credited == liveness.solver_prize
                && escrow.solver_remaining == 0
                && escrow.solver_paid == liveness.solver_prize
                && escrow.solver_credit_claimed == 1
        };
        if !solver_claim_is_exact {
            return Err(Error::MismatchedBinding);
        }
        let settlement = settlement_terminal.ok_or(Error::UnresolvedCandidates)?;
        if settlement.epoch != window.epoch
            || settlement.candidate != candidate.candidate
            || settlement.terminal_slot < window.finalized_slot
            || settlement.flags != 0
        {
            return Err(Error::MismatchedBinding);
        }
    } else if settlement_terminal.is_some()
        || escrow.solver_credited != 0
        || escrow.solver_remaining != 0
        || escrow.solver_paid != 0
        || escrow.solver_credit_claimed != 0
    {
        return Err(Error::MismatchedBinding);
    }
    if escrow.candidate != candidate.candidate
        || escrow.liveness_policy_id != liveness.policy_id
        || (escrow.funding_state == EscrowFundingState::Sealed && escrow.work_closed != 1)
        || escrow.bond_refund_claimed != 1
        || (escrow.funding_state == EscrowFundingState::Sealed && escrow.work_refund_claimed != 1)
        || (escrow.solver_credited != 0 && escrow.solver_credit_claimed != 1)
    {
        return Err(Error::MismatchedBinding);
    }
    if escrow.cleanup_finalized != 0 || escrow.candidate_closed != 0 {
        return Err(Error::Replay);
    }
    if escrow.cleanup_remaining < liveness.candidate_close_reward {
        return Err(Error::Underfunded);
    }
    let expected_owned_lamports = escrow.accounted_lamports()?;
    let surplus = observed_owned_lamports
        .checked_sub(expected_owned_lamports)
        .ok_or(Error::Underfunded)?;
    let after_reward = escrow
        .cleanup_remaining
        .checked_sub(liveness.candidate_close_reward)
        .ok_or(Error::ArithmeticOverflow)?;
    let rent = if escrow.funding_state == EscrowFundingState::Sealed {
        add(
            escrow.staging_rent_principal,
            escrow.verification_rent_principal,
        )?
    } else {
        escrow.staging_rent_principal
    };
    let mut next = escrow;
    next.cleanup_remaining = 0;
    next.cleanup_paid = add(next.cleanup_paid, liveness.candidate_close_reward)?;
    next.cleanup_refunded = add(next.cleanup_refunded, after_reward)?;
    next.cleanup_finalized = 1;
    next.candidate_closed = 1;
    next.surplus_routed = next
        .surplus_routed
        .checked_add(u128::from(surplus))
        .ok_or(Error::ArithmeticOverflow)?;
    bind_escrow(candidate, next, liveness)?;
    let next_page = index_page.mark_candidate_closed(candidate)?;
    Ok(CandidateCleanupTransitionV2 {
        index_page: next_page,
        escrow: next,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.candidate_close_reward,
            neutral_sink: surplus,
            refund_destination_credit: after_reward,
            rent_principal_refund: rent,
            ..LamportDispositionV1::default()
        },
    })
}

pub fn claim_solver_credit(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    verdict: CandidateVerdictV1,
    escrow: CandidateEscrowV2,
    liveness: CandidateLivenessPolicyV2,
) -> Result<EscrowClaimTransitionV2, Error> {
    candidate.bind_window(window)?;
    verdict.bind_candidate(candidate, window)?;
    bind_escrow(candidate, escrow, liveness)?;
    if !window.is_finalized()
        || window.selected_candidate != candidate.candidate
        || escrow.candidate != candidate.candidate
        || candidate.status != CandidateStatus::Verdicted
        || verdict.kind != VerdictKind::Valid
    {
        return Err(Error::MismatchedBinding);
    }
    if escrow.solver_credit_claimed != 0 {
        return Err(Error::Replay);
    }
    if escrow.solver_credited == 0 || escrow.solver_remaining != escrow.solver_credited {
        return Err(Error::NotActive);
    }
    let amount = escrow.solver_remaining;
    let mut next = escrow;
    next.solver_remaining = 0;
    next.solver_paid = add(next.solver_paid, amount)?;
    next.solver_credit_claimed = 1;
    bind_escrow(candidate, next, liveness)?;
    Ok(EscrowClaimTransitionV2 {
        escrow: next,
        disposition: LamportDispositionV1 {
            solver_payout: amount,
            ..LamportDispositionV1::default()
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseIndexPageTransitionV2 {
    pub budget: EpochCandidateBudgetV2,
    pub disposition: LamportDispositionV1,
}

pub fn close_index_page(
    window: CandidateWindowV3,
    budget: EpochCandidateBudgetV2,
    page: CandidateIndexPageV1,
    liveness: CandidateLivenessPolicyV2,
    observed_page_lamports: u64,
) -> Result<CloseIndexPageTransitionV2, Error> {
    window.validate()?;
    bind_budget(budget, window.epoch, liveness)?;
    page.validate()?;
    let page_start = usize::from(page.page_index)
        .checked_mul(CANDIDATES_PER_INDEX_PAGE)
        .ok_or(Error::ArithmeticOverflow)?;
    let expected_count = core::cmp::min(
        usize::from(window.begun_candidate_count).saturating_sub(page_start),
        CANDIDATES_PER_INDEX_PAGE,
    );
    if !window.is_finalized()
        || budget.terminalized != 1
        || budget.epoch != window.epoch
        || page.epoch != window.epoch
        || budget.liveness_policy_id != liveness.policy_id
        || budget.index_pages_owed == 0
        || page.page_index != budget.index_pages_owed - 1
        || usize::from(page.count) != expected_count
        || !page.all_candidates_closed()?
    {
        return Err(Error::MismatchedBinding);
    }
    if budget.index_cleanup_remaining < liveness.index_page_close_reward {
        return Err(Error::Underfunded);
    }
    let page_count =
        u64::try_from(MAX_CANDIDATE_INDEX_PAGES).map_err(|_| Error::ArithmeticOverflow)?;
    if !budget.index_page_rent_principal.is_multiple_of(page_count) {
        return Err(Error::InvalidState);
    }
    let rent_refund = budget.index_page_rent_principal / page_count;
    let surplus = observed_page_lamports
        .checked_sub(rent_refund)
        .ok_or(Error::Underfunded)?;
    let mut next = budget;
    next.index_cleanup_remaining = next
        .index_cleanup_remaining
        .checked_sub(liveness.index_page_close_reward)
        .ok_or(Error::ArithmeticOverflow)?;
    next.index_cleanup_paid = add(next.index_cleanup_paid, liveness.index_page_close_reward)?;
    next.index_pages_owed = next
        .index_pages_owed
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next.surplus_routed = next
        .surplus_routed
        .checked_add(u128::from(surplus))
        .ok_or(Error::ArithmeticOverflow)?;
    bind_budget(next, window.epoch, liveness)?;
    Ok(CloseIndexPageTransitionV2 {
        budget: next,
        disposition: LamportDispositionV1 {
            keeper_reward: liveness.index_page_close_reward,
            neutral_sink: surplus,
            rent_principal_refund: rent_refund,
            ..LamportDispositionV1::default()
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRefundTransitionV2 {
    pub budget: EpochCandidateBudgetV2,
    pub disposition: LamportDispositionV1,
}

pub fn claim_epoch_unused(
    window: CandidateWindowV3,
    budget: EpochCandidateBudgetV2,
    liveness: CandidateLivenessPolicyV2,
    observed_budget_lamports: u64,
) -> Result<EpochRefundTransitionV2, Error> {
    window.validate()?;
    bind_budget(budget, window.epoch, liveness)?;
    if !window.is_finalized()
        || budget.epoch != window.epoch
        || budget.terminalized != 1
        || budget.index_pages_owed != 0
    {
        return Err(Error::NotActive);
    }
    if budget.refund_claimed != 0 {
        return Err(Error::Replay);
    }
    let solver_phase_matches_selection = if window.selected_candidate.is_zero() {
        budget.solver_remaining == budget.solver_initial
            && budget.solver_credited == 0
            && budget.solver_refunded == 0
    } else {
        budget.solver_remaining == 0
            && budget.solver_credited == budget.solver_initial
            && budget.solver_refunded == 0
    };
    if !solver_phase_matches_selection {
        return Err(Error::MismatchedBinding);
    }
    let expected_budget_lamports = budget.accounted_lamports()?;
    let surplus = observed_budget_lamports
        .checked_sub(expected_budget_lamports)
        .ok_or(Error::Underfunded)?;
    let refund = add(budget.index_cleanup_remaining, budget.solver_remaining)?;
    let mut next = budget;
    next.index_cleanup_refunded = add(next.index_cleanup_refunded, next.index_cleanup_remaining)?;
    next.index_cleanup_remaining = 0;
    next.solver_refunded = add(next.solver_refunded, next.solver_remaining)?;
    next.solver_remaining = 0;
    next.refund_claimed = 1;
    next.surplus_routed = next
        .surplus_routed
        .checked_add(u128::from(surplus))
        .ok_or(Error::ArithmeticOverflow)?;
    bind_budget(next, window.epoch, liveness)?;
    Ok(EpochRefundTransitionV2 {
        budget: next,
        disposition: LamportDispositionV1 {
            neutral_sink: surplus,
            refund_destination_credit: refund,
            ..LamportDispositionV1::default()
        },
    })
}
