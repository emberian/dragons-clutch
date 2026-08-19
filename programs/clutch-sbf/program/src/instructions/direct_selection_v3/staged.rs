//! Competitive submission and staged reverification for Direct V3.
//!
//! Submit runs the complete cached direct verifier before deciding among an
//! explicit no-state rejection, a retained admission, and a bounded
//! replacement; at most three Candidate accounts stay live and a
//! competitively admitted tick can never replay. Begin and Verify stage the
//! full reexecution across transactions so no single instruction repeats the
//! V2 triple-verification that measured over the transaction compute cap.
//! Long-lived account values are heap-boxed and every large decode happens in
//! its own `#[inline(never)]` frame so each handler stays inside the fixed
//! SBF stack frame.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::direct_selection::{full_domain, DirectOrders};
use crate::instructions::genesis::{
    read_rent, require_creatable, require_system_program, RentParameters,
};
use crate::seeds;
use clutch_batch_policy_identity::{
    direct_lifecycle_v3::admission_transcript_v3,
    direct_window_v1::{
        canonical_account_candidate_id, verify_direct_two_order_candidate, DirectCandidateEntryV1,
        DirectCandidateV2, DirectCandidateWindowV1, DirectTwoOrderInputV1,
        ValidatedDirectDomainV1, DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_POLICY_V1,
        DIRECT_WINDOW_PHASE_OPEN, MAX_DIRECT_CANDIDATES,
    },
    FullRelationDomainV1, FullScoreV1, Identity32V1,
};
use clutch_solana_layout::{
    account_len,
    direct_selection_v3::{
        DirectCandidateV3Account, DirectEpochV4Account, DirectFundingLedgerV3,
        DirectWindowV3Account, DIRECT_BATCH_POLICY_V3_BYTES, DIRECT_CANDIDATE_STATUS_REVERIFIED,
        DIRECT_CANDIDATE_V3_BYTES, DIRECT_EPOCH_V4_BYTES, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        DIRECT_LIFECYCLE_PHASE_VERIFYING, DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
        DIRECT_RESERVATION_V2_BYTES, DIRECT_WINDOW_PHASE_VERIFYING, DIRECT_WINDOW_V3_BYTES,
        DIRECT_WORK_BUDGET_BYTES,
    },
    reservation::RESERVATION_STATE_ACTIVE,
    Hash32, MAX_OUTCOMES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::common::{
    frozen_pair, grid_tick, observe_funding, pay_reward, read_epoch_v4_boxed,
    read_reservation_v2_boxed, read_window_v3_boxed, read_work_budget, require_neutral_sink,
    sink_hash, write_epoch_v4, write_window_v3,
};
use super::{
    create_pda_account_full_principal, direct_creation_funding, require_exact_direct_policy,
    DIRECT_NEUTRAL_SINK_V3,
};

macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

const SUBMIT_BASE_ACCOUNT_COUNT: usize = 12;
const IX_SUBMIT_PAYER: usize = 0;
const IX_SUBMIT_EPOCH: usize = 1;
const IX_SUBMIT_POLICY: usize = 2;
const IX_SUBMIT_GRID: usize = 3;
const IX_SUBMIT_PAGE: usize = 4;
const IX_SUBMIT_RES_ZERO: usize = 5;
const IX_SUBMIT_RES_ONE: usize = 6;
const IX_SUBMIT_WINDOW: usize = 7;
const IX_SUBMIT_CANDIDATE: usize = 8;
const IX_SUBMIT_TOP_START: usize = 9;

const BEGIN_ACCOUNT_COUNT: usize = 5;
const IX_BEGIN_KEEPER: usize = 0;
const IX_BEGIN_EPOCH: usize = 1;
const IX_BEGIN_WINDOW: usize = 2;
const IX_BEGIN_WORK: usize = 3;
const IX_BEGIN_CLOCK: usize = 4;

const VERIFY_ACCOUNT_COUNT: usize = 8;
const IX_VERIFY_KEEPER: usize = 0;
const IX_VERIFY_EPOCH: usize = 1;
const IX_VERIFY_GRID: usize = 2;
const IX_VERIFY_PAGE: usize = 3;
const IX_VERIFY_WINDOW: usize = 4;
const IX_VERIFY_CANDIDATE: usize = 5;
const IX_VERIFY_WORK: usize = 6;
const IX_VERIFY_CLOCK: usize = 7;

/// One retained candidate's authenticated compact projection.
#[derive(Clone, Copy)]
struct RetainedV3 {
    entry: DirectCandidateEntryV1,
    score: FullScoreV1,
    funding: DirectFundingLedgerV3,
}

/// `SubmitDirectCandidateV3`.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn submit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    outcome_price: u64,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() >= SUBMIT_BASE_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_SUBMIT_PAYER])?;
    require(
        accounts[IX_SUBMIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    let (existing_window, top_count) =
        inspect_submission_window(program_id, &accounts[IX_SUBMIT_WINDOW])?;
    let replacement_shape = top_count == MAX_DIRECT_CANDIDATES;
    let extra = if replacement_shape { 2 } else { 0 };
    require_count(accounts, SUBMIT_BASE_ACCOUNT_COUNT + top_count + extra)?;
    require_distinct(&accounts[..IX_SUBMIT_TOP_START + top_count])?;
    validate_submit_fixed_roles(program_id, accounts)?;
    require_creatable(&accounts[IX_SUBMIT_CANDIDATE])?;
    let mut index = 0usize;
    while index < top_count {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[IX_SUBMIT_TOP_START + index],
            true,
            &[DIRECT_CANDIDATE_V3_BYTES],
        )?;
        index += 1;
    }
    let sink_index = IX_SUBMIT_TOP_START + top_count;
    let (system, displaced_payer_index) = if replacement_shape {
        require_neutral_sink(&accounts[sink_index])?;
        (sink_index + 2, Some(sink_index + 1))
    } else {
        (sink_index, None)
    };
    require_system_program(&accounts[system])?;
    let rent = read_rent(&accounts[system + 1])?;
    let now = read_clock_slot(&accounts[system + 2])?;

    let mut epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_SUBMIT_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    let expected_lifecycle = if existing_window {
        DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN
    } else {
        DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY
    };
    require(
        epoch.lifecycle_phase == expected_lifecycle,
        ClutchError::NotActive,
    )?;
    require(
        now >= epoch.direct.submission_opens_slot && now < epoch.direct.submission_closes_slot,
        ClutchError::NotActive,
    )?;
    authenticate_policy_artifact(program_id, &accounts[IX_SUBMIT_POLICY], &epoch)?;

    let orders = frozen_pair(program_id, &accounts[IX_SUBMIT_PAGE], &epoch.direct.common)?;
    let reservation_fundings = [
        read_reservation_v2_boxed(
            program_id,
            &accounts[IX_SUBMIT_RES_ZERO],
            &epoch.direct.common,
            orders.zero,
            RESERVATION_STATE_ACTIVE,
        )?
        .funding,
        read_reservation_v2_boxed(
            program_id,
            &accounts[IX_SUBMIT_RES_ONE],
            &epoch.direct.common,
            orders.one,
            RESERVATION_STATE_ACTIVE,
        )?
        .funding,
    ];
    let complement = epoch
        .direct
        .common
        .price_scale
        .checked_sub(outcome_price)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    grid_tick(
        program_id,
        &accounts[IX_SUBMIT_GRID],
        &epoch.direct.common,
        complement,
    )?;
    let tick = grid_tick(
        program_id,
        &accounts[IX_SUBMIT_GRID],
        &epoch.direct.common,
        outcome_price,
    )?;

    // The complete relation verifier issues the exact Candidate body.
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[usize::from(orders.outcome)] = outcome_price;
    prices[1usize - usize::from(orders.outcome)] = complement;
    let candidate_id = canonical_account_candidate_id(
        Identity32V1(epoch.direct.common.epoch.bytes()),
        Identity32V1(epoch.direct.common.market.bytes()),
        &prices,
    );
    let (candidate_address, candidate_bump) = seeds::direct_candidate_v3_pda(
        program_id,
        &epoch.direct.common.epoch.bytes(),
        &candidate_id.0,
    );
    expect_pda(
        accounts[IX_SUBMIT_CANDIDATE].key,
        (candidate_address, candidate_bump),
        None,
    )?;
    let candidate = issue_candidate_boxed(&epoch, orders, prices, now, candidate_bump)?;

    // Window prestate, retained registry, and the competitive tick bitmap.
    let mut window = if existing_window {
        let window = read_window_v3_boxed(program_id, &accounts[IX_SUBMIT_WINDOW], &epoch)?;
        require(
            window.window.phase == DIRECT_WINDOW_PHASE_OPEN
                && usize::from(window.window.top_count) == top_count
                && window.window.relation_domain_digest.0 == candidate.relation_domain_digest.0,
            ClutchError::MismatchedState,
        )?;
        window
    } else {
        let (window_address, window_bump) =
            seeds::direct_window_v3_pda(program_id, &epoch.direct.common.epoch.bytes());
        expect_pda(
            accounts[IX_SUBMIT_WINDOW].key,
            (window_address, window_bump),
            None,
        )?;
        first_window_state(&epoch, &candidate, window_bump)?
    };
    require(
        window.seen_competitive_ticks & (1u64 << tick) == 0,
        ClutchError::Replay,
    )?;

    let mut retained = [RetainedV3 {
        entry: DirectCandidateEntryV1::ZERO,
        score: candidate.score(),
        funding: DirectFundingLedgerV3::ZERO,
    }; MAX_DIRECT_CANDIDATES];
    load_retained_registry_v3(
        program_id,
        accounts,
        IX_SUBMIT_TOP_START,
        &window,
        &mut retained[..top_count],
    )?;

    // Explicit no-state outcome: a full top three the candidate cannot beat.
    if replacement_shape
        && !candidate
            .score()
            .is_better_than(&retained[MAX_DIRECT_CANDIDATES - 1].score)
    {
        return require_submit_observations_unchanged(
            &epoch,
            &window,
            &reservation_fundings,
            accounts,
            top_count,
        );
    }

    // Persist the monotone donation observation for everything that survives.
    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_SUBMIT_EPOCH])?;
    epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_SUBMIT_PAGE])?;
    if existing_window {
        window.funding = observe_funding(window.funding, &accounts[IX_SUBMIT_WINDOW])?;
    }
    index = 0;
    while index < top_count {
        retained[index].funding = observe_funding(
            retained[index].funding,
            &accounts[IX_SUBMIT_TOP_START + index],
        )?;
        index += 1;
    }

    // Insertion exactly as the model admits: append or displace the worst,
    // then bubble strictly better scores upward.
    let mut entries = [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES];
    let mut scores = [candidate.score(); MAX_DIRECT_CANDIDATES];
    index = 0;
    while index < top_count {
        entries[index] = retained[index].entry;
        scores[index] = retained[index].score;
        index += 1;
    }
    let mut displaced: Option<usize> = None;
    let slot_index;
    if replacement_shape {
        displaced = Some(MAX_DIRECT_CANDIDATES - 1);
        slot_index = MAX_DIRECT_CANDIDATES - 1;
    } else {
        slot_index = top_count;
    }
    entries[slot_index] = candidate.entry();
    scores[slot_index] = candidate.score();
    let post_count = if replacement_shape {
        MAX_DIRECT_CANDIDATES
    } else {
        top_count + 1
    };
    let mut at = slot_index;
    while at > 0 && scores[at].is_better_than(&scores[at - 1]) {
        entries.swap(at, at - 1);
        scores.swap(at, at - 1);
        at -= 1;
    }

    // Close the displaced worst before the new account exists: its recorded
    // payer principal returns to its payer, everything else to the sink.
    if let Some(worst) = displaced {
        let payer_index =
            displaced_payer_index.ok_or(Refusal::Adapter(ClutchError::AccountCount))?;
        super::common::close_funded_account(
            &accounts[IX_SUBMIT_TOP_START + worst],
            &accounts[payer_index],
            &accounts[sink_index],
            retained[worst].funding,
            0,
        )?;
    }

    let post_admissions = window
        .window
        .admitted_count
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let post_admissions_u8 =
        u8::try_from(post_admissions).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    window.window.top = entries;
    window.window.top_count = post_count as u8;
    window.window.admitted_count = post_admissions;
    window.window.admission_transcript = admission_transcript_v3(
        window.window.admission_transcript,
        post_admissions_u8,
        tick,
        candidate.entry(),
    );
    window.seen_competitive_ticks |= 1u64 << tick;
    window.live_candidate_mask = prefix_mask(post_count as u8)?;
    window.verification_mask = 0;

    if !existing_window {
        epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN;
    }

    /* Existing-state writes precede the create CPIs; no borrow survives into
     * a CPI, and a failed create rolls the whole transaction back. */
    write_epoch_v4(&accounts[IX_SUBMIT_EPOCH], &epoch)?;
    index = 0;
    while index < top_count {
        if displaced == Some(index) {
            index += 1;
            continue;
        }
        persist_candidate_observation(
            &accounts[IX_SUBMIT_TOP_START + index],
            retained[index].funding,
        )?;
        index += 1;
    }

    let candidate_funding = direct_creation_funding(
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_CANDIDATE],
        &rent,
        DIRECT_CANDIDATE_V3_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    create_pda_account_full_principal(
        program_id,
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_CANDIDATE],
        &accounts[system],
        &rent,
        DIRECT_CANDIDATE_V3_BYTES,
        candidate_funding,
        0,
        &[
            seeds::SEED_DIRECT_CANDIDATE_V3,
            &epoch.direct.common.epoch.bytes(),
            &candidate_id.0,
            &[candidate_bump],
        ],
    )?;
    write_new_candidate(&accounts[IX_SUBMIT_CANDIDATE], &candidate, candidate_funding)?;

    if !existing_window {
        window.funding = create_first_window(
            program_id,
            accounts,
            &rent,
            system,
            &epoch,
            window.window.stored_bump,
        )?;
    }
    write_window_v3(&accounts[IX_SUBMIT_WINDOW], &window)?;
    Ok(())
}

/// `BeginDirectVerificationV3`.
#[inline(never)]
pub(super) fn begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, BEGIN_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_BEGIN_KEEPER])?;
    require(
        accounts[IX_BEGIN_KEEPER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(
        program_id,
        accounts,
        &[
            StateRole::writable(IX_BEGIN_EPOCH, DIRECT_EPOCH_V4_BYTES),
            StateRole::writable(IX_BEGIN_WINDOW, DIRECT_WINDOW_V3_BYTES),
            StateRole::writable(IX_BEGIN_WORK, DIRECT_WORK_BUDGET_BYTES),
        ],
    )?;
    let now = read_clock_slot(&accounts[IX_BEGIN_CLOCK])?;
    let mut epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_BEGIN_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
        ClutchError::NotActive,
    )?;
    require_selection_slot(&epoch, now)?;
    let mut window = read_window_v3_boxed(program_id, &accounts[IX_BEGIN_WINDOW], &epoch)?;
    require(
        window.window.phase == DIRECT_WINDOW_PHASE_OPEN,
        ClutchError::NotActive,
    )?;
    let mut budget = read_work_budget(program_id, &accounts[IX_BEGIN_WORK], &epoch)?;

    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_BEGIN_EPOCH])?;
    window.funding = observe_funding(window.funding, &accounts[IX_BEGIN_WINDOW])?;
    budget.funding = super::observe_direct_funding(
        budget.funding,
        accounts[IX_BEGIN_WORK]
            .lamports()
            .checked_sub(budget.reward_balance)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?,
        DIRECT_NEUTRAL_SINK_V3,
    )?;

    let reward = budget.rewards.begin_verification;
    pay_reward(
        &accounts[IX_BEGIN_WORK],
        &accounts[IX_BEGIN_KEEPER],
        &mut budget,
        reward,
    )?;
    window.window.phase = DIRECT_WINDOW_PHASE_VERIFYING;
    window.verification_mask = 0;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_VERIFYING;

    write_epoch_v4(&accounts[IX_BEGIN_EPOCH], &epoch)?;
    write_window_v3(&accounts[IX_BEGIN_WINDOW], &window)?;
    budget.encode(sink_hash(), &mut borrow_mut!(accounts[IX_BEGIN_WORK])?)?;
    Ok(())
}

/// `VerifyDirectCandidateV3(index)`.
#[inline(never)]
pub(super) fn verify(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    retained_index: u8,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, VERIFY_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_VERIFY_KEEPER])?;
    require(
        accounts[IX_VERIFY_KEEPER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(
        program_id,
        accounts,
        &[
            StateRole::read_only(IX_VERIFY_EPOCH, DIRECT_EPOCH_V4_BYTES),
            StateRole::read_only(IX_VERIFY_GRID, account_len::PRICE_GRID),
            StateRole::read_only(IX_VERIFY_PAGE, account_len::ORDER_PAGE),
            StateRole::writable(IX_VERIFY_WINDOW, DIRECT_WINDOW_V3_BYTES),
            StateRole::writable(IX_VERIFY_CANDIDATE, DIRECT_CANDIDATE_V3_BYTES),
            StateRole::writable(IX_VERIFY_WORK, DIRECT_WORK_BUDGET_BYTES),
        ],
    )?;
    let now = read_clock_slot(&accounts[IX_VERIFY_CLOCK])?;
    let epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_VERIFY_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_VERIFYING,
        ClutchError::NotActive,
    )?;
    require_selection_slot(&epoch, now)?;
    let mut window = read_window_v3_boxed(program_id, &accounts[IX_VERIFY_WINDOW], &epoch)?;
    require(
        window.window.phase == DIRECT_WINDOW_PHASE_VERIFYING,
        ClutchError::NotActive,
    )?;
    require(
        retained_index < window.window.top_count,
        ClutchError::MismatchedState,
    )?;
    let bit = 1u8 << retained_index;
    require(window.verification_mask & bit == 0, ClutchError::Replay)?;

    let orders = frozen_pair(program_id, &accounts[IX_VERIFY_PAGE], &epoch.direct.common)?;
    let mut retained = read_candidate_v3_boxed(
        program_id,
        &accounts[IX_VERIFY_CANDIDATE],
        &epoch.direct.common.epoch,
    )?;
    require(
        retained.candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED
            && retained.candidate.entry() == window.window.top[usize::from(retained_index)]
            && retained.candidate.relation_domain_digest == window.window.relation_domain_digest
            && retained.candidate.submitted_slot >= epoch.direct.submission_opens_slot
            && retained.candidate.submitted_slot < epoch.direct.submission_closes_slot,
        ClutchError::MismatchedState,
    )?;

    // Full staged reexecution of exactly one retained Candidate.
    reverify_retained(&epoch, orders, &retained.candidate)?;
    grid_tick(
        program_id,
        &accounts[IX_VERIFY_GRID],
        &epoch.direct.common,
        retained.candidate.prices[0],
    )?;
    grid_tick(
        program_id,
        &accounts[IX_VERIFY_GRID],
        &epoch.direct.common,
        retained.candidate.prices[1],
    )?;

    let mut budget = read_work_budget(program_id, &accounts[IX_VERIFY_WORK], &epoch)?;
    window.funding = observe_funding(window.funding, &accounts[IX_VERIFY_WINDOW])?;
    retained.funding = observe_funding(retained.funding, &accounts[IX_VERIFY_CANDIDATE])?;
    budget.funding = super::observe_direct_funding(
        budget.funding,
        accounts[IX_VERIFY_WORK]
            .lamports()
            .checked_sub(budget.reward_balance)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let reward = budget.rewards.verify_candidate;
    pay_reward(
        &accounts[IX_VERIFY_WORK],
        &accounts[IX_VERIFY_KEEPER],
        &mut budget,
        reward,
    )?;

    retained.candidate.status = DIRECT_CANDIDATE_STATUS_REVERIFIED;
    window.verification_mask |= bit;
    write_window_v3(&accounts[IX_VERIFY_WINDOW], &window)?;
    write_candidate_v3(&accounts[IX_VERIFY_CANDIDATE], &retained)?;
    budget.encode(sink_hash(), &mut borrow_mut!(accounts[IX_VERIFY_WORK])?)?;
    Ok(())
}

/// Begin/Verify/Finalize lie inside the immutable staged-selection window.
pub(super) fn require_selection_slot(epoch: &DirectEpochV4Account, now: u64) -> Outcome<()> {
    require(
        now >= epoch.direct.submission_closes_slot && now < epoch.selection_deadline_slot,
        ClutchError::NotActive,
    )
}

#[inline(never)]
fn inspect_submission_window(program_id: &Pubkey, account: &AccountInfo) -> Outcome<(bool, usize)> {
    if account.owner == program_id {
        accounts::validate_state_role_lengths(
            program_id,
            account,
            true,
            &[DIRECT_WINDOW_V3_BYTES],
        )?;
        let window = DirectWindowV3Account::decode(&account.data.borrow(), sink_hash())?;
        Ok((true, usize::from(window.window.top_count)))
    } else {
        require_creatable(account)?;
        Ok((false, 0))
    }
}

fn validate_submit_fixed_roles(program_id: &Pubkey, accounts: &[AccountInfo]) -> Outcome<()> {
    for (index, len, writable) in [
        (IX_SUBMIT_EPOCH, DIRECT_EPOCH_V4_BYTES, true),
        (IX_SUBMIT_POLICY, DIRECT_BATCH_POLICY_V3_BYTES, false),
        (IX_SUBMIT_GRID, account_len::PRICE_GRID, false),
        (IX_SUBMIT_PAGE, account_len::ORDER_PAGE, false),
        (IX_SUBMIT_RES_ZERO, DIRECT_RESERVATION_V2_BYTES, false),
        (IX_SUBMIT_RES_ONE, DIRECT_RESERVATION_V2_BYTES, false),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], writable, &[len])?;
    }
    Ok(())
}

/// The exact 96-byte DirectBatchPolicy artifact account, in its own frame.
#[inline(never)]
fn authenticate_policy_artifact(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
) -> Outcome<()> {
    let supplied_policy =
        clutch_solana_layout::direct_selection_v3::DirectBatchPolicyV3::decode(
            &account.data.borrow(),
        )?;
    require_exact_direct_policy(
        epoch.direct.common.epoch,
        epoch.direct_policy_v3_id,
        supplied_policy,
    )?;
    expect_pda(
        account.key,
        seeds::direct_batch_policy_v3_pda(
            program_id,
            &epoch.direct.common.epoch.bytes(),
            &epoch.direct_policy_v3_id.bytes(),
        ),
        None,
    )
}

/// Run the complete cached direct verifier; the full relation domain and its
/// verification temporaries stay in this frame and the issued Candidate body
/// returns boxed.
#[inline(never)]
fn issue_candidate_boxed(
    epoch: &DirectEpochV4Account,
    orders: DirectOrders,
    prices: [u64; MAX_OUTCOMES],
    now: u64,
    stored_bump: u8,
) -> Outcome<Box<DirectCandidateV2>> {
    let domain = full_domain(&epoch.direct, DIRECT_POLICY_V1)?;
    let candidate = verify_direct_two_order_candidate(
        &domain,
        DirectTwoOrderInputV1 {
            prices,
            buy_limit: orders.buy_limit,
            sell_limit: orders.sell_limit,
            quantity: orders.quantity,
            submitted_slot: now,
            buy_index: orders.buy_index,
            sell_index: orders.sell_index,
            outcome: orders.outcome,
            stored_bump,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(Box::new(candidate))
}

/// Reexecute one retained Candidate against the frozen source in this frame.
#[inline(never)]
fn reverify_retained(
    epoch: &DirectEpochV4Account,
    orders: DirectOrders,
    candidate: &DirectCandidateV2,
) -> Outcome<()> {
    let domain: FullRelationDomainV1 = full_domain(&epoch.direct, DIRECT_POLICY_V1)?;
    let validated = ValidatedDirectDomainV1::new(&domain)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    validated
        .reverify_decoded_candidate(
            candidate,
            DirectTwoOrderInputV1 {
                prices: candidate.prices,
                buy_limit: orders.buy_limit,
                sell_limit: orders.sell_limit,
                quantity: orders.quantity,
                submitted_slot: candidate.submitted_slot,
                buy_index: orders.buy_index,
                sell_index: orders.sell_index,
                outcome: orders.outcome,
                stored_bump: candidate.stored_bump,
            },
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

/// Boxed decode of one V3 Candidate account with its PDA authenticated.
#[inline(never)]
fn read_candidate_v3_boxed(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch_id: &Hash32,
) -> Outcome<Box<DirectCandidateV3Account>> {
    let value = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_candidate_v3_pda(
            program_id,
            &epoch_id.bytes(),
            &value.candidate.candidate_id.0,
        ),
        Some(value.candidate.stored_bump),
    )?;
    Ok(Box::new(value))
}

/// Persist one V3 Candidate value; encode temporaries stay in this frame.
#[inline(never)]
fn write_candidate_v3(account: &AccountInfo, value: &DirectCandidateV3Account) -> Outcome<()> {
    let mut data = borrow_mut!(account)?;
    value.encode(sink_hash(), &mut data)?;
    Ok(())
}

/// Persist a surviving retained Candidate's observed funding unchanged-body.
#[inline(never)]
fn persist_candidate_observation(
    account: &AccountInfo,
    funding: DirectFundingLedgerV3,
) -> Outcome<()> {
    let survivor = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    let observed = DirectCandidateV3Account {
        candidate: survivor.candidate,
        funding,
    };
    let mut data = borrow_mut!(account)?;
    observed.encode(sink_hash(), &mut data)?;
    Ok(())
}

/// Persist the newly issued winner-eligible Candidate account.
#[inline(never)]
fn write_new_candidate(
    account: &AccountInfo,
    candidate: &DirectCandidateV2,
    funding: DirectFundingLedgerV3,
) -> Outcome<()> {
    let value = DirectCandidateV3Account {
        candidate: *candidate,
        funding,
    };
    let mut data = borrow_mut!(account)?;
    value.encode(sink_hash(), &mut data)?;
    Ok(())
}

/// Load, authenticate, and compact the retained V3 candidate registry.
#[inline(never)]
fn load_retained_registry_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    start: usize,
    window: &DirectWindowV3Account,
    out: &mut [RetainedV3],
) -> Outcome<()> {
    require(
        out.len() == usize::from(window.window.top_count),
        ClutchError::MismatchedState,
    )?;
    let mut index = 0usize;
    while index < out.len() {
        let value =
            DirectCandidateV3Account::decode(&accounts[start + index].data.borrow(), sink_hash())?;
        expect_pda(
            accounts[start + index].key,
            seeds::direct_candidate_v3_pda(
                program_id,
                &value.candidate.epoch_id.0,
                &value.candidate.candidate_id.0,
            ),
            Some(value.candidate.stored_bump),
        )?;
        require(
            value.candidate.epoch_id == window.window.epoch_id
                && value.candidate.market_id == window.window.market_id
                && value.candidate.order_set_id == window.window.order_set_id
                && value.candidate.policy_id == window.window.policy_id
                && value.candidate.relation_domain_digest == window.window.relation_domain_digest
                && value.candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED
                && value.candidate.entry() == window.window.top[index],
            ClutchError::MismatchedState,
        )?;
        out[index] = RetainedV3 {
            entry: value.candidate.entry(),
            score: value.candidate.score(),
            funding: value.funding,
        };
        index += 1;
    }
    Ok(())
}

/// The first admission's Window V3 poststate skeleton, boxed.
#[inline(never)]
fn first_window_state(
    epoch: &DirectEpochV4Account,
    candidate: &DirectCandidateV2,
    window_bump: u8,
) -> Outcome<Box<DirectWindowV3Account>> {
    Ok(Box::new(DirectWindowV3Account {
        window: DirectCandidateWindowV1 {
            epoch_id: candidate.epoch_id,
            market_id: candidate.market_id,
            order_set_id: candidate.order_set_id,
            policy_id: candidate.policy_id,
            relation_domain_digest: candidate.relation_domain_digest,
            admission_transcript: Identity32V1::ZERO,
            selected_candidate: Identity32V1::ZERO,
            top: [DirectCandidateEntryV1::ZERO; MAX_DIRECT_CANDIDATES],
            opens_slot: epoch.direct.submission_opens_slot,
            closes_slot: epoch.direct.submission_closes_slot,
            selected_slot: 0,
            admitted_count: 0,
            top_count: 0,
            phase: DIRECT_WINDOW_PHASE_OPEN,
            stored_bump: window_bump,
            flags: 0,
            reserved: [0; 2],
        },
        funding: DirectFundingLedgerV3::ZERO,
        seen_competitive_ticks: 0,
        verification_mask: 0,
        live_candidate_mask: 0,
        extension_flags: 0,
        selection_deadline_slot: epoch.selection_deadline_slot,
        settlement_deadline_slot: epoch.settlement_deadline_slot,
        receipt_funding: DirectFundingLedgerV3::ZERO,
        pot_funding: DirectFundingLedgerV3::ZERO,
        reserved: [0; 4],
    }))
}

/// Create the first Window account with its exact full-principal delta.
#[inline(never)]
fn create_first_window(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rent: &RentParameters,
    system: usize,
    epoch: &DirectEpochV4Account,
    window_bump: u8,
) -> Outcome<DirectFundingLedgerV3> {
    let funding = direct_creation_funding(
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_WINDOW],
        rent,
        DIRECT_WINDOW_V3_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    create_pda_account_full_principal(
        program_id,
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_WINDOW],
        &accounts[system],
        rent,
        DIRECT_WINDOW_V3_BYTES,
        funding,
        0,
        &[
            seeds::SEED_DIRECT_WINDOW_V3,
            &epoch.direct.common.epoch.bytes(),
            &[window_bump],
        ],
    )?;
    Ok(funding)
}

/// The model's no-state rule: a rejected noncompetitive attempt must observe
/// every live funded account byte-unchanged, because nothing is persisted.
#[inline(never)]
fn require_submit_observations_unchanged(
    epoch: &DirectEpochV4Account,
    window: &DirectWindowV3Account,
    reservation_fundings: &[DirectFundingLedgerV3; 2],
    accounts: &[AccountInfo],
    top_count: usize,
) -> Outcome<()> {
    require_funding_unchanged(epoch.epoch_funding, &accounts[IX_SUBMIT_EPOCH])?;
    require_funding_unchanged(epoch.page_funding, &accounts[IX_SUBMIT_PAGE])?;
    require_funding_unchanged(window.funding, &accounts[IX_SUBMIT_WINDOW])?;
    require_funding_unchanged(reservation_fundings[0], &accounts[IX_SUBMIT_RES_ZERO])?;
    require_funding_unchanged(reservation_fundings[1], &accounts[IX_SUBMIT_RES_ONE])?;
    let mut index = 0usize;
    while index < top_count {
        let value = DirectCandidateV3Account::decode(
            &accounts[IX_SUBMIT_TOP_START + index].data.borrow(),
            sink_hash(),
        )?;
        require_funding_unchanged(value.funding, &accounts[IX_SUBMIT_TOP_START + index])?;
        index += 1;
    }
    Ok(())
}

fn require_funding_unchanged(
    funding: DirectFundingLedgerV3,
    account: &AccountInfo,
) -> Outcome<()> {
    let accounted = funding
        .payer_principal_lamports
        .checked_add(funding.prior_donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        account.lamports() == accounted,
        ClutchError::MismatchedState,
    )
}

fn prefix_mask(count: u8) -> Outcome<u8> {
    if usize::from(count) > MAX_DIRECT_CANDIDATES {
        return Err(Refusal::Adapter(ClutchError::MismatchedState));
    }
    Ok(if count == 0 { 0 } else { (1u8 << count) - 1 })
}
