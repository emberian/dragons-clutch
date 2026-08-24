// SPDX-License-Identifier: AGPL-3.0-or-later

//! Ordered runtime commit contracts for already-planned retirement bundles.
//!
//! The pure retirement crate computes every post-state and checked balance
//! before this module is entered. This module then fixes the only permitted
//! mutation order for an SBF backend. The backend remains a separately named
//! trust boundary: it maps authenticated identities to `AccountInfo` values,
//! pre-borrows every writable data/lamport cell, and propagates every error so
//! Solana transaction rollback remains effective.

use clutch_retirement::{
    CoalescedPayerDebitsV1, CoalescedRecipientCreditsV1, EpochRootRecipientCreditsV2,
    EpochRootRetirementPlanV2, GeneralEpochLifecycleProjectionV2, Identity32V1,
    PositionLifecycleStateV2, PositionReplayReopenPlanV2, PositionReplayRetirementPlanV2,
    ReplayLifecycleStateV1, RetirementErrorV2, GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
    POSITION_TOMBSTONE_V2_BYTES, POSITION_TOMBSTONE_V3_BYTES, POSITION_V2_BYTES,
    PROJECTED_REPLAY_SUCCESSOR_BYTES,
};

use crate::position_v3_bridge::PreparedPositionReplayCloseV3;
use crate::{PositionAccountV2, ReplaySuccessorAccountV1, RetirementAdapterErrorV2};

fn total_credits_v1(credits: CoalescedRecipientCreditsV1) -> Result<u64, RetirementAdapterErrorV2> {
    let mut total = 0u64;
    for credit in credits.entries.into_iter().flatten() {
        total = total
            .checked_add(credit.credit_lamports)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    }
    Ok(total)
}

fn total_epoch_credits_v2(
    credits: EpochRootRecipientCreditsV2,
) -> Result<u64, RetirementAdapterErrorV2> {
    let mut total = 0u64;
    for credit in credits.entries.into_iter().flatten() {
        total = total
            .checked_add(credit.credit_lamports)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    }
    Ok(total)
}

/// Fully prepared Position-tombstone plus Replay-deletion commit.
///
/// Construction encodes the complete tombstone before the first runtime
/// borrow or mutation. The plan contains absolute recipient post-balances, so
/// the backend must compare its preflight observations with the balances that
/// were used to build the pure plan before calling [`execute_position_replay_close_v2`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedPositionReplayCloseV2 {
    position: Identity32V1,
    replay: Identity32V1,
    position_balance_before: u64,
    replay_balance_before: u64,
    tombstone: [u8; POSITION_TOMBSTONE_V2_BYTES],
    credits: CoalescedRecipientCreditsV1,
    position_balance_after: u64,
}

impl PreparedPositionReplayCloseV2 {
    /// Authenticated Position source account.
    pub const fn position(self) -> Identity32V1 {
        self.position
    }

    /// Authenticated Replay source account.
    pub const fn replay(self) -> Identity32V1 {
        self.replay
    }

    /// Position balance authenticated before pure planning.
    pub const fn position_balance_before(self) -> u64 {
        self.position_balance_before
    }

    /// Replay balance authenticated before pure planning.
    pub const fn replay_balance_before(self) -> u64 {
        self.replay_balance_before
    }

    /// Exact encoded permanent Position tombstone.
    pub const fn tombstone(self) -> [u8; POSITION_TOMBSTONE_V2_BYTES] {
        self.tombstone
    }

    /// Alias-coalesced absolute recipient post-balances.
    pub const fn credits(self) -> CoalescedRecipientCreditsV1 {
        self.credits
    }

    /// Exact Position lamports retained for its permanent tombstone.
    pub const fn position_balance_after(self) -> u64 {
        self.position_balance_after
    }
}

/// Convert a complete pure Position/Replay close plan into an ordered runtime
/// commit. A live adapter must pass the same two authenticated writable account
/// identities used to construct the pure plan.
pub fn prepare_position_replay_close_v2(
    position: Identity32V1,
    replay: Identity32V1,
    position_balance_before: u64,
    replay_balance_before: u64,
    plan: PositionReplayRetirementPlanV2,
) -> Result<PreparedPositionReplayCloseV2, RetirementAdapterErrorV2> {
    if position == replay {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    if !matches!(
        plan.replay_post_state,
        clutch_retirement::ReplayLifecycleStateV1::Absent
    ) || plan.replay_balance_after != 0
    {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    let total_before = position_balance_before
        .checked_add(replay_balance_before)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let total_after = plan
        .position_balance_after
        .checked_add(total_credits_v1(plan.recipient_credits)?)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if total_before != total_after {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    Ok(PreparedPositionReplayCloseV2 {
        position,
        replay,
        position_balance_before,
        replay_balance_before,
        tombstone: plan.position_tombstone.encode()?,
        credits: plan.recipient_credits,
        position_balance_after: plan.position_balance_after,
    })
}

/// Fully prepared Epoch-tombstone plus Window/Budget deletion commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedEpochRootCloseV1 {
    epoch: Identity32V1,
    window: Identity32V1,
    budget: Identity32V1,
    epoch_balance_before: u64,
    window_balance_before: u64,
    budget_balance_before: u64,
    tombstone: [u8; GENERAL_EPOCH_TOMBSTONE_V1_BYTES],
    credits: EpochRootRecipientCreditsV2,
    epoch_balance_after: u64,
}

impl PreparedEpochRootCloseV1 {
    /// Authenticated Epoch source account.
    pub const fn epoch(self) -> Identity32V1 {
        self.epoch
    }

    /// Authenticated Window source account.
    pub const fn window(self) -> Identity32V1 {
        self.window
    }

    /// Authenticated Budget source account.
    pub const fn budget(self) -> Identity32V1 {
        self.budget
    }

    /// Epoch balance authenticated before pure planning.
    pub const fn epoch_balance_before(self) -> u64 {
        self.epoch_balance_before
    }

    /// Window balance authenticated before pure planning.
    pub const fn window_balance_before(self) -> u64 {
        self.window_balance_before
    }

    /// Budget balance authenticated before pure planning.
    pub const fn budget_balance_before(self) -> u64 {
        self.budget_balance_before
    }

    /// Exact encoded permanent Epoch tombstone.
    pub const fn tombstone(self) -> [u8; GENERAL_EPOCH_TOMBSTONE_V1_BYTES] {
        self.tombstone
    }

    /// Alias-coalesced absolute recipient post-balances.
    pub const fn credits(self) -> EpochRootRecipientCreditsV2 {
        self.credits
    }

    /// Exact Epoch lamports retained for its permanent tombstone.
    pub const fn epoch_balance_after(self) -> u64 {
        self.epoch_balance_after
    }
}

/// Convert a complete pure Epoch-root close plan into an ordered runtime
/// commit. Window and Budget deletion are inseparable from Epoch tombstoning.
pub fn prepare_epoch_root_close_v1(
    epoch: Identity32V1,
    window: Identity32V1,
    budget: Identity32V1,
    epoch_balance_before: u64,
    window_balance_before: u64,
    budget_balance_before: u64,
    plan: EpochRootRetirementPlanV2,
) -> Result<PreparedEpochRootCloseV1, RetirementAdapterErrorV2> {
    if epoch == window || epoch == budget || window == budget {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    let tombstone = match plan.epoch_post_state {
        GeneralEpochLifecycleProjectionV2::Tombstone(value) => value,
        GeneralEpochLifecycleProjectionV2::Live(_) => {
            return Err(RetirementErrorV2::WrongPhase.into())
        }
    };
    if plan.window_balance_after != 0 || plan.budget_balance_after != 0 {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    let total_before = epoch_balance_before
        .checked_add(window_balance_before)
        .and_then(|value| value.checked_add(budget_balance_before))
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let total_after = plan
        .epoch_balance_after
        .checked_add(total_epoch_credits_v2(plan.recipient_credits)?)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if total_before != total_after {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    Ok(PreparedEpochRootCloseV1 {
        epoch,
        window,
        budget,
        epoch_balance_before,
        window_balance_before,
        budget_balance_before,
        tombstone: tombstone.encode()?,
        credits: plan.recipient_credits,
        epoch_balance_after: plan.epoch_balance_after,
    })
}

/// SBF-side mutation boundary for terminal retirement.
///
/// Implementations must map each identity to the already-authenticated
/// writable account and must not invoke CPI from `resize_*` onward. `preflight`
/// must borrow every writable data/lamport cell and compare current recipient
/// balances to the snapshots used by the pure planner. Returning any error is
/// mandatory; swallowing it would invalidate rollback reasoning.
pub trait RetirementCloseRuntimeV1 {
    /// Backend error propagated unchanged to the instruction entry point.
    type Error;

    /// Pre-borrow and recheck every account in a Position/Replay bundle.
    fn preflight_position_replay(
        &mut self,
        commit: PreparedPositionReplayCloseV2,
    ) -> Result<(), Self::Error>;

    /// Pre-borrow and recheck every account in an Epoch/Window/Budget bundle.
    fn preflight_epoch_root(&mut self, commit: PreparedEpochRootCloseV1)
        -> Result<(), Self::Error>;

    /// Reallocate one still-program-owned source to an exact smaller length.
    fn resize_program_owned(
        &mut self,
        account: Identity32V1,
        new_len: usize,
    ) -> Result<(), Self::Error>;

    /// Write an exact prepared byte image after resizing.
    fn write_exact(&mut self, account: Identity32V1, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Set one authenticated recipient to its precomputed absolute balance.
    fn set_recipient_balance(
        &mut self,
        recipient: Identity32V1,
        balance_after: u64,
    ) -> Result<(), Self::Error>;

    /// Set a still-program-owned source to its precomputed absolute balance.
    fn set_source_balance(
        &mut self,
        source: Identity32V1,
        balance_after: u64,
    ) -> Result<(), Self::Error>;

    /// Release a zero-data, zero-lamport source to the System program.
    fn assign_system(&mut self, source: Identity32V1) -> Result<(), Self::Error>;
}

/// Execute the fixed Position/Replay mutation order after all pure planning.
///
/// There is no CPI in this close path. All fallible preparation and all
/// writable borrows occur before the first resize. Replay ownership is
/// released only after its bytes and lamports are both zero.
pub fn execute_position_replay_close_v2<R: RetirementCloseRuntimeV1>(
    runtime: &mut R,
    commit: PreparedPositionReplayCloseV2,
) -> Result<(), R::Error> {
    runtime.preflight_position_replay(commit)?;
    runtime.resize_program_owned(commit.position, POSITION_TOMBSTONE_V2_BYTES)?;
    runtime.write_exact(commit.position, &commit.tombstone)?;
    runtime.resize_program_owned(commit.replay, 0)?;
    for credit in commit.credits.entries.into_iter().flatten() {
        runtime.set_recipient_balance(credit.recipient, credit.balance_after)?;
    }
    runtime.set_source_balance(commit.position, commit.position_balance_after)?;
    runtime.set_source_balance(commit.replay, 0)?;
    runtime.assign_system(commit.replay)
}

/// SBF-side mutation and postwrite-authentication boundary for a Position V3
/// plus current-generation Replay close.
///
/// Implementations must bind every identity to the same authenticated account
/// used to prepare the commit, recheck the two source balances and every
/// alias-coalesced recipient pre-balance, and borrow every writable cell in
/// [`Self::preflight_position_replay_close_v3`] before the first resize. The
/// Replay's exact terminal semantic ID and signed sequence are part of that
/// preflight; a merely terminal-looking replacement is not sufficient.
pub trait PositionReplayCloseRuntimeV3 {
    /// Backend error propagated unchanged to the instruction entry point.
    type Error;

    /// Reauthenticate the complete prospective commit without mutating it.
    fn preflight_position_replay_close_v3(
        &mut self,
        commit: &PreparedPositionReplayCloseV3,
    ) -> Result<(), Self::Error>;

    /// Reallocate one still-program-owned source to an exact smaller length.
    fn resize_program_owned_v3(
        &mut self,
        account: Identity32V1,
        new_len: usize,
    ) -> Result<(), Self::Error>;

    /// Write one exact prepared byte image after resizing.
    fn write_exact_v3(
        &mut self,
        account: Identity32V1,
        bytes: &[u8],
    ) -> Result<(), Self::Error>;

    /// Set one authenticated, alias-coalesced recipient to its absolute balance.
    fn set_recipient_balance_v3(
        &mut self,
        recipient: Identity32V1,
        balance_after: u64,
    ) -> Result<(), Self::Error>;

    /// Set one still-program-owned source to its absolute post-balance.
    fn set_source_balance_v3(
        &mut self,
        source: Identity32V1,
        balance_after: u64,
    ) -> Result<(), Self::Error>;

    /// Release the zero-data, zero-lamport Replay to the System program.
    fn assign_replay_system_v3(&mut self, replay: Identity32V1) -> Result<(), Self::Error>;

    /// Reauthenticate exact bytes, owners, lengths, and balances after writes.
    fn authenticate_position_replay_close_v3_postwrite(
        &mut self,
        commit: &PreparedPositionReplayCloseV3,
    ) -> Result<(), Self::Error>;
}

/// Non-copy evidence that the fixed V3 close order and hostile postwrite
/// authentication both completed.
///
/// The runtime-facing instruction composer must consume this value when it
/// mints its own terminal receipt. It cannot duplicate success evidence by
/// copying a boolean or caller-supplied identifier.
#[derive(Debug, Eq, PartialEq)]
pub struct ExecutedPositionReplayCloseV3 {
    committed: PreparedPositionReplayCloseV3,
}

impl ExecutedPositionReplayCloseV3 {
    /// Position PDA now holding the exact permanent V3 tombstone.
    pub const fn position_account(&self) -> Identity32V1 {
        self.committed.position_account()
    }

    /// Replay PDA now System-owned, empty, and zero-lamport.
    pub const fn replay_account(&self) -> Identity32V1 {
        self.committed.replay_account()
    }

    /// Semantic ID of the terminal Replay bytes observed before deletion.
    pub const fn replay_terminal_semantic_id(&self) -> Identity32V1 {
        self.committed.replay_terminal_semantic_id()
    }

    /// Exact signed terminal sequence authenticated before deletion.
    pub const fn signed_sequence(&self) -> u64 {
        self.committed.signed_sequence()
    }

    /// Alias-coalesced absolute recipient post-balances that were committed.
    pub const fn recipient_credits(&self) -> CoalescedRecipientCreditsV1 {
        self.committed.recipient_credits()
    }

    /// Exact permanent tombstone balance observed after the commit.
    pub const fn position_lamports_after(&self) -> u64 {
        self.committed.position_lamports_after()
    }
}

/// Execute the sole Position V3 plus Replay V3 retirement mutation order.
///
/// No CPI is permitted after preflight. Position is first reduced to its exact
/// tombstone image and Replay data is erased while both remain program-owned;
/// recipient credits are then written once per coalesced identity, source
/// balances are set absolutely, and Replay ownership is released last. A
/// non-copy receipt is returned only after an exact hostile postwrite check.
pub fn execute_position_replay_close_v3<R: PositionReplayCloseRuntimeV3>(
    runtime: &mut R,
    commit: PreparedPositionReplayCloseV3,
) -> Result<ExecutedPositionReplayCloseV3, R::Error> {
    runtime.preflight_position_replay_close_v3(&commit)?;
    runtime.resize_program_owned_v3(commit.position_account(), POSITION_TOMBSTONE_V3_BYTES)?;
    runtime.write_exact_v3(
        commit.position_account(),
        &commit.position_tombstone_bytes(),
    )?;
    runtime.resize_program_owned_v3(commit.replay_account(), 0)?;
    for credit in commit.recipient_credits().entries.into_iter().flatten() {
        runtime.set_recipient_balance_v3(credit.recipient, credit.balance_after)?;
    }
    runtime.set_source_balance_v3(
        commit.position_account(),
        commit.position_lamports_after(),
    )?;
    runtime.set_source_balance_v3(commit.replay_account(), commit.replay_lamports_after())?;
    runtime.assign_replay_system_v3(commit.replay_account())?;
    runtime.authenticate_position_replay_close_v3_postwrite(&commit)?;
    Ok(ExecutedPositionReplayCloseV3 { committed: commit })
}

/// Execute the fixed Epoch/Window/Budget mutation order after all pure joins.
///
/// The two deletable siblings remain program-owned until their data and
/// lamports are zero; owner release is last. There is no CPI after preflight.
pub fn execute_epoch_root_close_v1<R: RetirementCloseRuntimeV1>(
    runtime: &mut R,
    commit: PreparedEpochRootCloseV1,
) -> Result<(), R::Error> {
    runtime.preflight_epoch_root(commit)?;
    runtime.resize_program_owned(commit.epoch, GENERAL_EPOCH_TOMBSTONE_V1_BYTES)?;
    runtime.write_exact(commit.epoch, &commit.tombstone)?;
    runtime.resize_program_owned(commit.window, 0)?;
    runtime.resize_program_owned(commit.budget, 0)?;
    for credit in commit.credits.entries.into_iter().flatten() {
        runtime.set_recipient_balance(credit.recipient, credit.balance_after)?;
    }
    runtime.set_source_balance(commit.epoch, commit.epoch_balance_after)?;
    runtime.set_source_balance(commit.window, 0)?;
    runtime.set_source_balance(commit.budget, 0)?;
    runtime.assign_system(commit.window)?;
    runtime.assign_system(commit.budget)
}

/// Fully prepared next-generation Position/Replay reopen commit.
///
/// Both byte images are encoded and cross-checked against the pure reopen plan
/// before any System transfer. The prior-generation Replay absence remains a
/// mandatory read-only preflight role, preventing a stale tombstone from
/// skipping the exact predecessor slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedPositionReplayReopenV2 {
    prior_replay: Identity32V1,
    position: Identity32V1,
    replay: Identity32V1,
    position_payer: Identity32V1,
    replay_payer: Identity32V1,
    neutral_sink: Identity32V1,
    prior_replay_prefund_lamports: u64,
    neutral_sink_balance_after: u64,
    position_transfer_lamports: u64,
    replay_transfer_lamports: u64,
    payer_debits: CoalescedPayerDebitsV1,
    position_bytes: [u8; POSITION_V2_BYTES],
    replay_bytes: [u8; PROJECTED_REPLAY_SUCCESSOR_BYTES],
}

impl PreparedPositionReplayReopenV2 {
    /// Canonical absent Replay PDA for the closed predecessor generation.
    pub const fn prior_replay(self) -> Identity32V1 {
        self.prior_replay
    }

    /// Permanent Position PDA resized back to its live V2 shape.
    pub const fn position(self) -> Identity32V1 {
        self.position
    }

    /// Fresh successor Replay PDA for the new generation.
    pub const fn replay(self) -> Identity32V1 {
        self.replay
    }

    /// Alias-coalesced payer post-balances authenticated during preflight.
    pub const fn payer_debits(self) -> CoalescedPayerDebitsV1 {
        self.payer_debits
    }

    /// Position funding payer.
    pub const fn position_payer(self) -> Identity32V1 {
        self.position_payer
    }

    /// Replay funding payer.
    pub const fn replay_payer(self) -> Identity32V1 {
        self.replay_payer
    }

    /// Immutable neutral sink receiving predecessor hostile prefund.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    /// Full Position live-rent delta transferred on reopen.
    pub const fn position_transfer_lamports(self) -> u64 {
        self.position_transfer_lamports
    }

    /// Full Replay rent principal transferred without prefund discount.
    pub const fn replay_transfer_lamports(self) -> u64 {
        self.replay_transfer_lamports
    }

    /// Hostile prefund swept from the predecessor Replay PDA.
    pub const fn prior_replay_prefund_lamports(self) -> u64 {
        self.prior_replay_prefund_lamports
    }

    /// Sink balance after the checked predecessor-prefund sweep.
    pub const fn neutral_sink_balance_after(self) -> u64 {
        self.neutral_sink_balance_after
    }
}

/// Prepare a concrete Position V2 and Replay-successor byte image for the
/// already-planned next generation.
///
/// `position_value.base` is owned by the authoritative layout codec; the
/// function checks every retirement-owned identity, generation, bump, and
/// funding field against the pure plan. The base codec remains responsible for
/// its other economic fields and must construct the canonical zero/open body.
pub fn prepare_position_replay_reopen_v2(
    prior_replay: Identity32V1,
    position_value: PositionAccountV2,
    replay_value: ReplaySuccessorAccountV1,
    plan: PositionReplayReopenPlanV2,
) -> Result<PreparedPositionReplayReopenV2, RetirementAdapterErrorV2> {
    let reopen = plan.reopen;
    let live_position = match reopen.position_post_state {
        PositionLifecycleStateV2::Live(value) => value,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV2::WrongPhase.into()),
    };
    let live_replay = match reopen.replay_post_state {
        ReplayLifecycleStateV1::Live(value) => value,
        ReplayLifecycleStateV1::Absent => return Err(RetirementErrorV2::ReplayMismatch.into()),
    };
    let position_market = Identity32V1::new(position_value.base.market.bytes())?;
    let position_owner = Identity32V1::new(position_value.base.owner.bytes())?;
    let replay_market = Identity32V1::new(replay_value.base.market.bytes())?;
    let replay_owner = Identity32V1::new(replay_value.base.owner.bytes())?;
    if position_market != live_position.market
        || position_owner != live_position.owner
        || position_value.base.generation != live_position.generation
        || position_value.base.stored_bump != live_position.stored_bump
        || position_value.base.close_state != 0
        || position_value.base.cash_atoms != 0
        || position_value.base.reserved_cash_atoms != 0
        || position_value
            .base
            .internal
            .iter()
            .any(|amount| *amount != 0)
        || position_value.retirement != live_position.retirement
        || replay_market != live_replay.market
        || replay_owner != live_replay.owner
        || replay_value.base.position_generation != live_replay.position_generation
        || replay_value.base.sequence != 0
        || replay_value.base.stored_bump != live_replay.stored_bump
        || replay_value.rent != live_replay.rent
    {
        return Err(RetirementErrorV2::ReplayMismatch.into());
    }

    let position = reopen.position_funding.target();
    let replay = reopen.replay_funding.target();
    if prior_replay == position || prior_replay == replay || position == replay {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    if reopen.position_balance_after != reopen.position_funding.account_balance_after()
        || reopen.replay_balance_after != reopen.replay_funding.account_balance_after()
        || plan.prior_replay_balance_after != 0
    {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }

    Ok(PreparedPositionReplayReopenV2 {
        prior_replay,
        position,
        replay,
        position_payer: reopen.position_funding.rent().payer,
        replay_payer: reopen.replay_funding.rent().payer(),
        neutral_sink: reopen.position_funding.neutral_sink(),
        prior_replay_prefund_lamports: plan.prior_replay_prefund_lamports,
        neutral_sink_balance_after: plan.neutral_sink_balance_after,
        position_transfer_lamports: reopen.position_funding.rent().refundable_live_principal,
        replay_transfer_lamports: reopen.replay_funding.rent().refundable_principal(),
        payer_debits: reopen.payer_debits,
        position_bytes: position_value.encode()?,
        replay_bytes: replay_value.encode()?,
    })
}

/// SBF-side mutation boundary for Position/Replay reopen.
///
/// System transfers and the new Replay allocation/assignment are the only CPI
/// operations. They all precede state writes. Any error must escape the
/// instruction so transfers, allocation, resize, and writes roll back as one
/// Solana transaction.
pub trait PositionReplayReopenRuntimeV2 {
    /// Backend error propagated unchanged to the instruction entry point.
    type Error;

    /// Recheck tombstone bytes, prior-Replay absence, fresh-Replay absence,
    /// payer signers/balances, exact prefunds, and every writable borrow.
    fn preflight_reopen(
        &mut self,
        commit: PreparedPositionReplayReopenV2,
    ) -> Result<(), Self::Error>;

    /// Transfer full persisted principal without discounting hostile prefunds.
    fn system_transfer(
        &mut self,
        payer: Identity32V1,
        target: Identity32V1,
        lamports: u64,
    ) -> Result<(), Self::Error>;

    /// Sweep hostile prefund from the obsolete System-owned Replay PDA using
    /// its canonical program-signed seeds.
    fn system_transfer_from_prior_replay(
        &mut self,
        prior_replay: Identity32V1,
        neutral_sink: Identity32V1,
        lamports: u64,
        neutral_sink_balance_after: u64,
    ) -> Result<(), Self::Error>;

    /// Resize the still-program-owned Position tombstone back to live width.
    fn resize_position(
        &mut self,
        position: Identity32V1,
        new_len: usize,
    ) -> Result<(), Self::Error>;

    /// Allocate and assign the System-owned Replay PDA using its canonical
    /// program-signed seeds. The backend must not use `create_account`, because
    /// a positive hostile prefund is explicitly admitted.
    fn allocate_assign_replay(
        &mut self,
        replay: Identity32V1,
        new_len: usize,
    ) -> Result<(), Self::Error>;

    /// Write one exact pre-encoded state image after every CPI succeeds.
    fn write_exact(&mut self, account: Identity32V1, bytes: &[u8]) -> Result<(), Self::Error>;
}

/// Execute the fixed next-generation reopen order.
pub fn execute_position_replay_reopen_v2<R: PositionReplayReopenRuntimeV2>(
    runtime: &mut R,
    commit: PreparedPositionReplayReopenV2,
) -> Result<(), R::Error> {
    runtime.preflight_reopen(commit)?;
    if commit.prior_replay_prefund_lamports != 0 {
        runtime.system_transfer_from_prior_replay(
            commit.prior_replay,
            commit.neutral_sink,
            commit.prior_replay_prefund_lamports,
            commit.neutral_sink_balance_after,
        )?;
    }
    runtime.system_transfer(
        commit.position_payer,
        commit.position,
        commit.position_transfer_lamports,
    )?;
    runtime.system_transfer(
        commit.replay_payer,
        commit.replay,
        commit.replay_transfer_lamports,
    )?;
    runtime.resize_position(commit.position, POSITION_V2_BYTES)?;
    runtime.allocate_assign_replay(commit.replay, PROJECTED_REPLAY_SUCCESSOR_BYTES)?;
    runtime.write_exact(commit.position, &commit.position_bytes)?;
    runtime.write_exact(commit.replay, &commit.replay_bytes)
}
