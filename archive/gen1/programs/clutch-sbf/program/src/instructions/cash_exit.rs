//! Owner-authorized exit of unreserved pooled collateral cash.
//!
//! `WithdrawCash` is deliberately distinct from `Endow`: one intent admits an
//! inbound owner-to-Hoard transfer, while this one admits a Hoard-to-owner
//! transfer signed only by the canonical Hoard authority PDA. The transition
//! debits `PositionAccount::cash_atoms` but never its reserved subset, never
//! changes locked claim backing, and never assigns unsolicited Hoard surplus.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::split::validate_token_program;
use crate::{seeds, token};
use clutch_kernel::Error as KernelError;
use clutch_solana_layout::{
    account_len, collateral, Hash32, HoardAccount, Intent, PositionAccount, ProfileAccount,
};
use clutch_solana_reference::{Action, ReplayAccount, Request, REPLAY_ACCOUNT_LEN};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Exact account count for `WithdrawCash`.
pub const ACCOUNT_COUNT: usize = 12;
/// Position owner and transaction signer.
pub const IX_ACTOR: usize = 0;
/// Market the cash belongs to.
pub const IX_MARKET: usize = 1;
/// Locked-backing accounting record; read-only because withdrawal cannot alter it.
pub const IX_HOARD: usize = 2;
/// Owner Position whose unreserved cash is debited.
pub const IX_POSITION: usize = 3;
/// Owner replay sequence.
pub const IX_REPLAY: usize = 4;
/// Frozen collateral Profile.
pub const IX_PROFILE: usize = 5;
/// Content-authenticated collateral policy bytes.
pub const IX_POLICY: usize = 6;
/// Pinned executable Token-2022 program.
pub const IX_TOKEN_PROGRAM: usize = 7;
/// Realm collateral mint.
pub const IX_COLLATERAL_MINT: usize = 8;
/// Owner-controlled collateral destination.
pub const IX_DESTINATION: usize = 9;
/// Canonical Hoard signing authority PDA.
pub const IX_HOARD_AUTHORITY: usize = 10;
/// Canonical pooled Hoard collateral token account.
pub const IX_HOARD_TOKEN: usize = 11;

const STATE_ROLES: [StateRole; 5] = [
    StateRole::read_only(IX_MARKET, account_len::MARKET),
    StateRole::read_only(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
];

/// Decoded wire intent plus its exact owner replay sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawCashRequest {
    /// Exact replay sequence the caller consumes.
    pub sequence: u64,
    /// Market the pooled cash belongs to.
    pub market: Hash32,
    /// Position owner and signer.
    pub owner: Hash32,
    /// Exact owner-controlled collateral token destination.
    pub destination: Hash32,
    /// Unreserved cash atoms to withdraw.
    pub amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenSnapshot {
    destination: u64,
    hoard: u64,
    decimals: u8,
    authority_bump: u8,
}

#[inline(never)]
fn decode_request(request: &Request) -> Outcome<WithdrawCashRequest> {
    match request.action {
        Action::Layout(Intent::WithdrawCash {
            market,
            owner,
            destination,
            amount,
        }) => Ok(WithdrawCashRequest {
            sequence: request.sequence,
            market,
            owner,
            destination,
            amount,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Compute the complete ledger post-state without touching account bytes.
///
/// `cash_atoms` includes the reserved subset, so only
/// `cash_atoms - reserved_cash_atoms` is spendable. Both state values are
/// returned fully formed before the token CPI; they are encoded only after the
/// observed token balances prove the exact transfer.
#[inline(never)]
fn validated_withdrawal(
    market_bytes: &[u8],
    position_bytes: &[u8],
    replay_bytes: &[u8],
    actor: Hash32,
    request: &WithdrawCashRequest,
) -> Outcome<(PositionAccount, ReplayAccount)> {
    let market = accounts::read_market(market_bytes)?;
    let mut position = PositionAccount::decode(position_bytes)?;
    let mut replay = ReplayAccount::decode(replay_bytes)?;

    require(actor == position.owner, ClutchError::UnauthorizedActor)?;
    require(
        request.market == market.market
            && request.owner == position.owner
            && market.market == position.market
            && market.market == replay.market
            && position.owner == replay.owner
            && position.generation == replay.position_generation,
        ClutchError::MismatchedState,
    )?;
    /* Cash must remain withdrawable after resolution and while an owner is
     * completing a requested close. A fully closed Position may not reopen
     * its value plane. */
    require(position.close_state != 2, ClutchError::NotActive)?;
    require(request.sequence == replay.sequence, ClutchError::Replay)?;
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Replay))?;
    let free_cash = position.free_cash_atoms()?;
    if request.amount > free_cash {
        return Err(Refusal::Kernel(KernelError::InsufficientBalance));
    }
    position.cash_atoms = position
        .cash_atoms
        .checked_sub(request.amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    replay.sequence = next_sequence;
    Ok((position, replay))
}

#[inline(never)]
fn admit_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market: &accounts::MarketFacts,
    hoard: &HoardAccount,
) -> Outcome<TokenSnapshot> {
    validate_token_program(&accounts[IX_TOKEN_PROGRAM])?;
    let profile = ProfileAccount::decode(&accounts[IX_PROFILE].data.borrow())?;
    let policy_account = &accounts[IX_POLICY];
    require(
        !policy_account.is_writable
            && !policy_account.executable
            && policy_account.data_len() == collateral::COLLATERAL_POLICY_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let policy = collateral::verify_profile_identity(&policy_account.data.borrow(), &profile)?;
    token::require_drivable_collateral(&policy)?;

    let mint = &accounts[IX_COLLATERAL_MINT];
    require(!mint.is_writable, ClutchError::UnexpectedWritable)?;
    require(!mint.executable, ClutchError::ExecutableAccount)?;
    let mint_observation = token::admit_mint(mint, &token::MintPolicy::collateral(&policy))?;

    let authority = &accounts[IX_HOARD_AUTHORITY];
    require(
        !authority.is_writable && !authority.executable && authority.data_is_empty(),
        ClutchError::UnexpectedWritable,
    )?;
    let authority_derived = seeds::hoard_authority_pda(program_id, &market.market.bytes());
    expect_pda(authority.key, authority_derived, None)?;
    require(
        hoard.authority.bytes() == authority.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let destination = &accounts[IX_DESTINATION];
    let hoard_token = &accounts[IX_HOARD_TOKEN];
    require(
        destination.is_writable && hoard_token.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !destination.executable && !hoard_token.executable,
        ClutchError::ExecutableAccount,
    )?;
    expect_pda(
        hoard_token.key,
        seeds::hoard_token_pda(program_id, &market.market.bytes()),
        None,
    )?;
    let destination_observation = token::admit_token_account(
        destination,
        &token::TokenAccountPolicy::collateral_holder(&policy, *accounts[IX_ACTOR].key),
    )?;
    let hoard_observation = token::admit_token_account(
        hoard_token,
        &token::TokenAccountPolicy::hoard(&policy, *authority.key),
    )?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, hoard_observation.amount)?;

    Ok(TokenSnapshot {
        destination: destination_observation.amount,
        hoard: hoard_observation.amount,
        decimals: mint_observation.decimals,
        authority_bump: authority_derived.1,
    })
}

/// Withdraw unreserved owner cash through an exact Hoard-to-owner token move.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let request = decode_request(request)?;
    require_count(accounts, ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &STATE_ROLES)?;

    let actor = Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes());
    require(actor == request.owner, ClutchError::UnauthorizedActor)?;
    require(
        request.destination.bytes() == accounts[IX_DESTINATION].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    let hoard = HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?;
    let position = PositionAccount::decode(&accounts[IX_POSITION].data.borrow())?;
    let replay = ReplayAccount::decode(&accounts[IX_REPLAY].data.borrow())?;
    let profile = ProfileAccount::decode(&accounts[IX_PROFILE].data.borrow())?;
    let market_bytes = market.market.bytes();
    let owner_bytes = position.owner.bytes();

    expect_pda(
        accounts[IX_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes),
        Some(market.stored_bump),
    )?;
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(
        accounts[IX_HOARD].key,
        hoard_derived,
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        accounts[IX_POSITION].key,
        seeds::position_pda(program_id, &market_bytes, &owner_bytes),
        Some(position.stored_bump),
    )?;
    expect_pda(
        accounts[IX_REPLAY].key,
        seeds::replay_pda(program_id, &market_bytes, &owner_bytes, position.generation),
        Some(replay.stored_bump),
    )?;
    expect_pda(
        accounts[IX_PROFILE].key,
        seeds::profile_pda(program_id, &market.realm.bytes(), &market.profile.bytes()),
        None,
    )?;
    require(
        hoard.market == market.market
            && hoard.realm == market.realm
            && market.hoard_bump == hoard_derived.1
            && position.market == market.market
            && replay.market == market.market
            && replay.owner == position.owner
            && replay.position_generation == position.generation
            && profile.profile == market.profile
            && profile.realm == market.realm,
        ClutchError::MismatchedState,
    )?;

    let (position_post, replay_post) = validated_withdrawal(
        &accounts[IX_MARKET].data.borrow(),
        &accounts[IX_POSITION].data.borrow(),
        &accounts[IX_REPLAY].data.borrow(),
        actor,
        &request,
    )?;
    let snapshot = admit_tokens(program_id, accounts, &market, &hoard)?;
    let expected_destination = snapshot
        .destination
        .checked_add(request.amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_hoard = snapshot
        .hoard
        .checked_sub(request.amount)
        .ok_or(Refusal::Kernel(KernelError::InsufficientCollateral))?;
    require(
        expected_hoard >= hoard.collateral_atoms,
        ClutchError::HoardMirrorMismatch,
    )?;

    let bump = [snapshot.authority_bump];
    let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY, &market_bytes, &bump];
    token::transfer_checked_signed(
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_HOARD_TOKEN],
        &accounts[IX_COLLATERAL_MINT],
        &accounts[IX_DESTINATION],
        &accounts[IX_HOARD_AUTHORITY],
        request.amount,
        snapshot.decimals,
        &signer,
    )?;

    let post_destination = token::token_amount(&accounts[IX_DESTINATION])?;
    let post_hoard = token::token_amount(&accounts[IX_HOARD_TOKEN])?;
    token::require_exact_credit(snapshot.destination, post_destination, request.amount)?;
    token::require_exact_debit(snapshot.hoard, post_hoard, request.amount)?;
    require(
        post_destination == expected_destination && post_hoard == expected_hoard,
        ClutchError::TokenDeltaMismatch,
    )?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, post_hoard)?;

    position_post.encode(
        &mut accounts[IX_POSITION]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    replay_post
        .encode(
            &mut accounts[IX_REPLAY]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{canonical_outcome_id, MarketAccount, MAX_OUTCOMES};

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn fixture(
        lifecycle: u8,
        cash: u64,
        reserved: u64,
        sequence: u64,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut market = vec![0; account_len::MARKET];
        MarketAccount {
            market: h(3),
            realm: h(1),
            profile: h(2),
            terms: h(4),
            outcome_count: 2,
            lifecycle,
            stored_bump: 1,
            hoard_bump: 2,
            outcomes: {
                let mut values = [Hash32::ZERO; MAX_OUTCOMES];
                values[0] = canonical_outcome_id(h(3), 0);
                values[1] = canonical_outcome_id(h(3), 1);
                values
            },
            feed: h(5),
            collateral_cap: 1_000,
            created_slot: 1,
            reserved: Hash32::ZERO,
        }
        .encode(&mut market)
        .unwrap();
        let mut position = vec![0; account_len::POSITION];
        PositionAccount {
            market: h(3),
            owner: h(8),
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: cash,
            reserved_cash_atoms: reserved,
            stored_bump: 4,
            close_state: 0,
        }
        .encode(&mut position)
        .unwrap();
        let mut replay = vec![0; REPLAY_ACCOUNT_LEN];
        ReplayAccount {
            market: h(3),
            owner: h(8),
            position_generation: 0,
            sequence,
            stored_bump: 5,
            flags: 0,
        }
        .encode(&mut replay)
        .unwrap();
        (market, position, replay)
    }

    fn request(sequence: u64, amount: u64) -> WithdrawCashRequest {
        WithdrawCashRequest {
            sequence,
            market: h(3),
            owner: h(8),
            destination: h(9),
            amount,
        }
    }

    #[test]
    fn withdrawal_debits_only_total_cash_and_preserves_reservation() {
        let (market, position, replay) = fixture(0, 100, 40, 7);
        let (position, replay) =
            validated_withdrawal(&market, &position, &replay, h(8), &request(7, 60)).unwrap();
        assert_eq!(position.cash_atoms, 40);
        assert_eq!(position.reserved_cash_atoms, 40);
        assert_eq!(replay.sequence, 8);
    }

    #[test]
    fn withdrawal_cannot_spend_one_reserved_atom() {
        let (market, position, replay) = fixture(0, 100, 40, 7);
        assert_eq!(
            validated_withdrawal(&market, &position, &replay, h(8), &request(7, 61)),
            Err(Refusal::Kernel(KernelError::InsufficientBalance))
        );
    }

    #[test]
    fn withdrawal_remains_open_after_resolution() {
        let (market, position, replay) = fixture(1, 10, 0, 7);
        assert!(validated_withdrawal(&market, &position, &replay, h(8), &request(7, 10)).is_ok());
    }

    #[test]
    fn wrong_actor_replay_or_binding_refuses_without_mutation() {
        let (market, position, replay) = fixture(0, 100, 40, 7);
        assert_eq!(
            validated_withdrawal(&market, &position, &replay, h(10), &request(7, 1)),
            Err(ClutchError::UnauthorizedActor.into())
        );
        assert_eq!(
            validated_withdrawal(&market, &position, &replay, h(8), &request(8, 1)),
            Err(ClutchError::Replay.into())
        );
        let mut wrong = request(7, 1);
        wrong.market = h(11);
        assert_eq!(
            validated_withdrawal(&market, &position, &replay, h(8), &wrong),
            Err(ClutchError::MismatchedState.into())
        );
        assert_eq!(PositionAccount::decode(&position).unwrap().cash_atoms, 100);
        assert_eq!(ReplayAccount::decode(&replay).unwrap().sequence, 7);
    }
}
