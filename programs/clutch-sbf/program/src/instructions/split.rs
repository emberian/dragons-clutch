//! `Intent::Split`: add a complete internal set at the Hoard/Position seam.
//!
//! This module owns the only transition the program implements.  It contains no
//! economic logic: the complete-set split itself is
//! [`clutch_kernel::MarketState::split`], byte ownership is
//! [`clutch_solana_layout`] and the reference-only codecs of
//! [`clutch_solana_reference`], and metadata authentication is
//! [`crate::accounts`].  What lives here is the account list, the order of the
//! checks, and the write-back.
//!
//! The checks and their order are unchanged from the single-instruction
//! bring-up program, and `docs/implementation/SBF_BRINGUP.md` records the SVM
//! differential that pins them.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_kernel::{MarketState, Phase, Position, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES};
use clutch_solana_layout::{account_len, Hash32, HoardAccount, PositionAccount, MAX_OUTCOMES};
use clutch_solana_reference::{
    ExternalAccount, KernelAccount, ReplayAccount, EXTERNAL_ACCOUNT_LEN, KERNEL_ACCOUNT_LEN,
    REPLAY_ACCOUNT_LEN,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Exact number of accounts this instruction accepts.
///
/// A fixed count is itself a check: a remaining-account shuffle cannot append
/// an extra writable account for a later instruction to reuse.
pub const ACCOUNT_COUNT: usize = 9;

/// Authenticated actor; must be the position owner.
pub const IX_ACTOR: usize = 0;
/// Realm configuration account (read-only).
pub const IX_REALM: usize = 1;
/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 2;
/// Market account.
pub const IX_MARKET: usize = 3;
/// Hoard collateral account.
pub const IX_HOARD: usize = 4;
/// Owner position account.
pub const IX_POSITION: usize = 5;
/// Reference-only kernel-aggregate account.
pub const IX_KERNEL: usize = 6;
/// Reference-only external-shadow account.
pub const IX_EXTERNAL: usize = 7;
/// Reference-only replay-sequence account.
pub const IX_REPLAY: usize = 8;

/// The program-owned state roles of this instruction, in account-list order.
const STATE_ROLES: [StateRole; 8] = [
    StateRole::read_only(IX_REALM, account_len::REALM),
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
    StateRole::writable(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_EXTERNAL, EXTERNAL_ACCOUNT_LEN),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
];

/// One already-routed `Split` request.
///
/// [`crate::dispatch`] destructures the envelope so that this module never has
/// to re-match an action it already knows, and so there is no unreachable
/// fallback arm pretending another intent could arrive here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitRequest {
    /// Exact replay sequence the request claims.
    pub sequence: u64,
    /// Market identity the intent binds.
    pub market: Hash32,
    /// Owner identity the intent binds.
    pub owner: Hash32,
    /// Complete sets to create.
    pub quantity: u64,
}

/// Run the complete-set split on the pure kernel and re-encode the aggregate.
///
/// The whole `KernelAccount`/`MarketState`/`Position` working set lives in this
/// frame and nowhere else.  `internal` is updated in place only after every
/// kernel check has passed.
#[inline(never)]
fn kernel_split(
    kernel_data: &mut [u8],
    outcome_count: u8,
    collateral_before: u64,
    internal: &mut [u64; MAX_OUTCOMES],
    external: &[u64; MAX_OUTCOMES],
    quantity: u64,
) -> Outcome<u64> {
    let mut account = KernelAccount::decode(kernel_data)?;
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES || account.payouts.outcomes != outcome_count
    {
        return Err(ClutchError::MismatchedState.into());
    }
    let phase = match account.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ClutchError::NonCanonical.into()),
    };
    let mut market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: account.resolved_payout,
        collateral: collateral_before,
        total_supply: account.total_supply,
        payouts: account.payouts,
    };
    market.check_invariants()?;
    let mut position = Position {
        internal: *internal,
        external: *external,
    };
    market.split(&mut position, quantity)?;
    let mut index = 0_usize;
    while index < usize::from(outcome_count) {
        let local = position.internal[index]
            .checked_add(position.external[index])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        if local != market.total_supply[index] {
            return Err(ClutchError::AggregateClosureMismatch.into());
        }
        index += 1;
    }
    account.phase = match market.phase {
        Phase::Active => 0,
        Phase::Resolved => 1,
    };
    account.resolved_payout = market.resolved_payout;
    account.total_supply = market.total_supply;
    account.encode(kernel_data)?;
    *internal = position.internal;
    Ok(market.collateral)
}

/// Validate hostile accounts and apply exactly one `Split`.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &SplitRequest,
) -> Outcome<()> {
    require_count(accounts, ACCOUNT_COUNT)?;

    let actor = &accounts[IX_ACTOR];
    require_signer(actor)?;

    /* Role uniqueness.  A writable alias would let one logical debit or credit
     * land twice, which is obligation 3 of the reference adapter doc. */
    require_distinct(accounts)?;

    /* Program ownership, executable bit, declared mutability by role, and exact
     * data length per role. */
    accounts::validate_state_roles(program_id, accounts, &STATE_ROLES)?;

    let realm = accounts::read_realm(&accounts[IX_REALM].data.borrow())?;
    let profile = accounts::read_profile(&accounts[IX_PROFILE].data.borrow())?;
    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    let mut hoard = HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?;
    let mut position = PositionAccount::decode(&accounts[IX_POSITION].data.borrow())?;
    let kernel = accounts::read_kernel(&accounts[IX_KERNEL].data.borrow())?;
    let external = ExternalAccount::decode(&accounts[IX_EXTERNAL].data.borrow())?;
    let mut replay = ReplayAccount::decode(&accounts[IX_REPLAY].data.borrow())?;

    /* Derived addresses.  Caller-supplied expected keys are never accepted:
     * every address is recomputed from the frozen seed schema and compared,
     * and every stored bump is compared against the canonical bump. */
    let market_bytes = market.market.bytes();
    let realm_bytes = market.realm.bytes();
    let profile_bytes = market.profile.bytes();
    let owner_bytes = position.owner.bytes();
    expect_pda(
        accounts[IX_REALM].key,
        seeds::realm_pda(program_id, &realm_bytes),
        Some(realm.stored_bump),
    )?;
    expect_pda(
        accounts[IX_PROFILE].key,
        seeds::profile_pda(program_id, &realm_bytes, &profile_bytes),
        None,
    )?;
    expect_pda(
        accounts[IX_MARKET].key,
        seeds::market_pda(program_id, &realm_bytes, &market_bytes),
        Some(market.stored_bump),
    )?;
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(
        accounts[IX_HOARD].key,
        hoard_derived,
        Some(hoard.stored_bump),
    )?;
    require(market.hoard_bump == hoard_derived.1, ClutchError::WrongBump)?;
    expect_pda(
        accounts[IX_POSITION].key,
        seeds::position_pda(program_id, &market_bytes, &owner_bytes),
        Some(position.stored_bump),
    )?;
    expect_pda(
        accounts[IX_KERNEL].key,
        seeds::kernel_pda(program_id, &market_bytes),
        None,
    )?;
    expect_pda(
        accounts[IX_EXTERNAL].key,
        seeds::external_pda(program_id, &market_bytes, &owner_bytes, position.generation),
        Some(external.stored_bump),
    )?;
    expect_pda(
        accounts[IX_REPLAY].key,
        seeds::replay_pda(program_id, &market_bytes, &owner_bytes, position.generation),
        Some(replay.stored_bump),
    )?;

    /* Cross-account linkage, mirroring `validate_links` in the offline
     * reference adapter plus the Realm/Profile edges the reference only
     * checks at market initialization. */
    require(
        realm.realm == market.realm
            && realm.profile == market.profile
            && profile.profile == market.profile
            && profile.realm == market.realm
            && realm.profile_version == profile.version
            && usize::from(realm.max_outcomes) == MAX_OUTCOMES
            && market.outcome_count <= realm.max_outcomes,
        ClutchError::MismatchedState,
    )?;
    require(
        market.market == hoard.market
            && market.realm == hoard.realm
            && market.market == position.market
            && market.market == kernel.market
            && market.market == external.market
            && market.market == replay.market
            && position.owner == external.owner
            && position.owner == replay.owner
            && position.generation == external.position_generation
            && position.generation == replay.position_generation
            && (market.lifecycle != 0 || kernel.phase == 0)
            && (market.lifecycle != 1 || kernel.phase == 1)
            && market.lifecycle <= 1
            && kernel.payout_outcomes == market.outcome_count
            && usize::from(market.outcome_count) <= KERNEL_MAX_OUTCOMES,
        ClutchError::MismatchedState,
    )?;

    /* Padding beyond the active outcome count must be canonically zero in every
     * balance vector. */
    let count = usize::from(market.outcome_count);
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        require(
            position.internal[padding] == 0
                && kernel.total_supply[padding] == 0
                && external.balances[padding] == 0,
            ClutchError::NonCanonical,
        )?;
        padding += 1;
    }

    /* Closed single-position aggregate closure, before any write.
     *
     * NAMED DIVERGENCE from the offline reference adapter.  Commit 9c43863
     * replaced the reference's single-position equality with CLO-DELTA-V1
     * (`docs/implementation/MULTI_POSITION_CLOSURE.md`): a per-triple *bound*
     * against the market-wide supply ledger (C2), plus a delta write into that
     * ledger (C3).  This instruction still carries the old equality, because
     * porting it means taking a tenth account -- the supply ledger -- which is
     * an account-list change, not a check swap.
     *
     * The divergence is fail-closed rather than fail-open: `internal + external
     * == total_supply` is strictly stronger than C2's `<=`, so this program
     * refuses states the reference now accepts and accepts nothing the
     * reference refuses.  Concretely, it refuses every market holding a second
     * position.  The bring-up fixture is single-position, so the differential is
     * unaffected; a multi-position fixture would show the two adapters
     * disagreeing, with this one refusing.
     *
     * The C1/C2/C3 primitives are already written and tested in
     * `crate::accounts`; the port is listed as a follow-on in
     * `docs/implementation/SBF_BRINGUP.md`. */
    let mut outcome = 0_usize;
    while outcome < count {
        let local = position.internal[outcome]
            .checked_add(external.balances[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            local == kernel.total_supply[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }

    require(request.sequence == replay.sequence, ClutchError::Replay)?;
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Replay))?;

    require(
        actor.key.to_bytes() == position.owner.bytes(),
        ClutchError::UnauthorizedActor,
    )?;
    require(
        request.market == market.market && request.owner == position.owner,
        ClutchError::MismatchedState,
    )?;
    require(
        market.lifecycle == 0 && position.close_state == 0,
        ClutchError::NotActive,
    )?;
    let quantity = request.quantity;

    /* Collateral cap and position cash are checked before the kernel runs, in
     * the same order as the offline reference adapter. */
    let next_collateral = hoard
        .collateral_atoms
        .checked_add(quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        next_collateral <= market.collateral_cap,
        ClutchError::CollateralCap,
    )?;
    let next_cash = position
        .cash_atoms
        .checked_sub(quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    /* Everything below this line writes.  A refusal after this point aborts the
     * instruction, and SVM transaction semantics -- not this program -- are
     * what discard the partial write. */
    position.cash_atoms = next_cash;
    let collateral_after = {
        let mut kernel_data = accounts[IX_KERNEL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        kernel_split(
            &mut kernel_data,
            market.outcome_count,
            hoard.collateral_atoms,
            &mut position.internal,
            &external.balances,
            quantity,
        )?
    };
    hoard.collateral_atoms = collateral_after;
    replay.sequence = next_sequence;

    hoard.encode(
        &mut accounts[IX_HOARD]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    position.encode(
        &mut accounts[IX_POSITION]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    replay.encode(
        &mut accounts[IX_REPLAY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;

    /* Market and external-shadow bytes are deliberately untouched: `Split`
     * changes neither.  The differential harness still compares them against
     * the reference adapter's re-encoded post-state, so a codec that did not
     * round-trip would fail the comparison rather than hide inside a rewrite. */
    Ok(())
}
