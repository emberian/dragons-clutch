//! Hostile-account validation and the single bring-up transition.
//!
//! This module contains no semantic or economic logic.  Balances, supplies,
//! collateral and invariants belong to [`clutch_kernel`]; byte ownership
//! belongs to [`clutch_solana_layout`] and to the reference-only codecs in
//! [`clutch_solana_reference`].  What lives here is exactly the part that the
//! offline reference adapter cannot have: authentication of runtime-supplied
//! [`AccountInfo`] metadata, derivation of program addresses, and write-back
//! into runtime account data.
//!
//! Only `Split` is implemented.  Every other action refuses, and the refusals
//! are deliberate rather than incidental.

use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_kernel::{MarketState, Phase, Position, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES};
use clutch_solana_layout::{
    account_len, Hash32, HoardAccount, Intent, MarketAccount, PositionAccount, ProfileAccount,
    RealmAccount, MAX_OUTCOMES,
};
use clutch_solana_reference::{
    Action, ExternalAccount, KernelAccount, ReplayAccount, Request, EXTERNAL_ACCOUNT_LEN,
    KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN,
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

type Outcome<T> = core::result::Result<T, Refusal>;

fn require(condition: bool, error: ClutchError) -> Outcome<()> {
    if condition {
        Ok(())
    } else {
        Err(error.into())
    }
}

struct RealmFacts {
    realm: Hash32,
    profile: Hash32,
    max_outcomes: u8,
    profile_version: u8,
    stored_bump: u8,
}

struct ProfileFacts {
    profile: Hash32,
    realm: Hash32,
    version: u8,
}

struct MarketFacts {
    market: Hash32,
    realm: Hash32,
    profile: Hash32,
    outcome_count: u8,
    lifecycle: u8,
    stored_bump: u8,
    hoard_bump: u8,
    collateral_cap: u64,
}

struct KernelFacts {
    market: Hash32,
    phase: u8,
    payout_outcomes: u8,
    total_supply: [u64; MAX_OUTCOMES],
}

/* The three readers below are `inline(never)` on purpose.  The decoded
 * `MarketAccount` and `KernelAccount` values are large fixed-size structures,
 * and SBF gives each call frame a hard 4 KiB budget.  Keeping the large value
 * inside its own frame and returning only the small facts the transition needs
 * is what keeps the entrypoint frame inside that budget. */

#[inline(never)]
fn read_realm(data: &[u8]) -> Outcome<RealmFacts> {
    let value = RealmAccount::decode(data)?;
    Ok(RealmFacts {
        realm: value.realm,
        profile: value.profile,
        max_outcomes: value.max_outcomes,
        profile_version: value.profile_version,
        stored_bump: value.stored_bump,
    })
}

#[inline(never)]
fn read_profile(data: &[u8]) -> Outcome<ProfileFacts> {
    let value = ProfileAccount::decode(data)?;
    Ok(ProfileFacts {
        profile: value.profile,
        realm: value.realm,
        version: value.version,
    })
}

#[inline(never)]
fn read_market(data: &[u8]) -> Outcome<MarketFacts> {
    let value = MarketAccount::decode(data)?;
    Ok(MarketFacts {
        market: value.market,
        realm: value.realm,
        profile: value.profile,
        outcome_count: value.outcome_count,
        lifecycle: value.lifecycle,
        stored_bump: value.stored_bump,
        hoard_bump: value.hoard_bump,
        collateral_cap: value.collateral_cap,
    })
}

#[inline(never)]
fn read_kernel(data: &[u8]) -> Outcome<KernelFacts> {
    let value = KernelAccount::decode(data)?;
    Ok(KernelFacts {
        market: value.market,
        phase: value.phase,
        payout_outcomes: value.payouts.outcomes,
        total_supply: value.total_supply,
    })
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
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES
        || account.payouts.outcomes != outcome_count
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

fn expect_pda(
    actual: &Pubkey,
    derived: (Pubkey, u8),
    stored_bump: Option<u8>,
) -> Outcome<()> {
    require(*actual == derived.0, ClutchError::WrongPda)?;
    match stored_bump {
        Some(bump) => require(bump == derived.1, ClutchError::WrongBump),
        None => Ok(()),
    }
}

/// Validate hostile accounts and apply exactly one `Split`.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    require(accounts.len() == ACCOUNT_COUNT, ClutchError::AccountCount)?;

    let actor = &accounts[IX_ACTOR];
    require(actor.is_signer, ClutchError::MissingSignature)?;

    /* Role uniqueness.  A writable alias would let one logical debit or credit
     * land twice, which is obligation 3 of the reference adapter doc. */
    let mut left = 0_usize;
    while left < ACCOUNT_COUNT {
        let mut right = left + 1;
        while right < ACCOUNT_COUNT {
            require(
                accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }

    /* Program ownership, executable bit, and declared mutability by role. */
    let mut index = IX_REALM;
    while index < ACCOUNT_COUNT {
        let account = &accounts[index];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.executable, ClutchError::ExecutableAccount)?;
        if index >= IX_MARKET {
            require(account.is_writable, ClutchError::NotWritable)?;
        } else {
            require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        }
        index += 1;
    }

    /* Exact data lengths.  The codecs re-check length, discriminator and
     * version, but checking here first means a wrong-length account never
     * reaches a codec at all. */
    let lengths = [
        (IX_REALM, account_len::REALM),
        (IX_PROFILE, account_len::PROFILE),
        (IX_MARKET, account_len::MARKET),
        (IX_HOARD, account_len::HOARD),
        (IX_POSITION, account_len::POSITION),
        (IX_KERNEL, KERNEL_ACCOUNT_LEN),
        (IX_EXTERNAL, EXTERNAL_ACCOUNT_LEN),
        (IX_REPLAY, REPLAY_ACCOUNT_LEN),
    ];
    for (role, expected) in lengths {
        require(
            accounts[role].data_len() == expected,
            ClutchError::WrongDataLength,
        )?;
    }

    let realm = read_realm(&accounts[IX_REALM].data.borrow())?;
    let profile = read_profile(&accounts[IX_PROFILE].data.borrow())?;
    let market = read_market(&accounts[IX_MARKET].data.borrow())?;
    let mut hoard = HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?;
    let mut position = PositionAccount::decode(&accounts[IX_POSITION].data.borrow())?;
    let kernel = read_kernel(&accounts[IX_KERNEL].data.borrow())?;
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

    /* Closed single-position aggregate closure, before any write. */
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

    let request = Request::decode(instruction_data)?;
    require(request.sequence == replay.sequence, ClutchError::Replay)?;
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Replay))?;

    let quantity = match request.action {
        Action::Layout(Intent::Split {
            market: intent_market,
            owner: intent_owner,
            quantity,
        }) => {
            require(
                actor.key.to_bytes() == position.owner.bytes(),
                ClutchError::UnauthorizedActor,
            )?;
            require(
                intent_market == market.market && intent_owner == position.owner,
                ClutchError::MismatchedState,
            )?;
            require(
                market.lifecycle == 0 && position.close_state == 0,
                ClutchError::NotActive,
            )?;
            quantity
        }
        Action::Layout(Intent::CreateMarket { .. }) => {
            return Err(ClutchError::AuthorizationUnavailable.into())
        }
        Action::Resolve { .. } | Action::RedeemInternal { .. } => {
            return Err(ClutchError::ResolutionEvidenceUnavailable.into())
        }
        Action::Layout(_) => return Err(ClutchError::UnsupportedInstruction.into()),
    };

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
