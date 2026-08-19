//! Positionless redemption of bearer Token-2022 outcome claims.
//!
//! External Eggs are ordinary bearer tokens.  Their owner, transfer history,
//! and the Position that first materialized them are irrelevant at redemption:
//! the claimant proves authority over the exact source token account, burns the
//! claim, and receives the immutable resolved payout from the pooled Hoard.
//! Token-2022 mint supply is the aggregate external truth.

use crate::accounts::{
    self, expect_pda, require, require_distinct, require_signer, Outcome, StateRole,
};
use crate::claim_truth::{self, ObservedMintSupplies};
use crate::error::{ClutchError, Refusal};
use crate::instructions::observe_resolve::{bound_native_resolution, reconstruct_native_market};
use crate::instructions::split::validate_token_program;
use crate::{seeds, token};
use clutch_kernel::{BasisMode, MarketState, PayoutVector, Phase, Position};
use clutch_solana_layout::{
    account_len, collateral, native_resolution::NATIVE_RESOLUTION_LEN, Hash32, HoardAccount,
    Intent, PayoutVectorBytes, ProfileAccount, TermsAccount,
};
use clutch_solana_reference::{Action, KernelAccount, Request, KERNEL_ACCOUNT_LEN};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Bearer claimant and burn authority (signer).
pub const IX_CLAIMANT: usize = 0;
/// Frozen collateral Profile.
pub const IX_PROFILE: usize = 1;
/// Resolved Market.
pub const IX_MARKET: usize = 2;
/// Pooled collateral accounting Hoard (writable).
pub const IX_HOARD: usize = 3;
/// Kernel aggregate (writable).
pub const IX_KERNEL: usize = 4;
/// Market-wide internal plus cached-external supply ledger (writable).
pub const IX_SUPPLY: usize = 5;
/// Immutable resolution record.
pub const IX_RESOLUTION: usize = 6;
/// Immutable terms artifact.
pub const IX_TERMS: usize = 7;
/// Realm collateral-policy bytes.
pub const IX_POLICY: usize = 8;
/// Pinned Token-2022 program.
pub const IX_TOKEN_PROGRAM: usize = 9;
/// Frozen collateral mint.
pub const IX_COLLATERAL_MINT: usize = 10;
/// Claimant-owned collateral destination (writable).
pub const IX_DESTINATION: usize = 11;
/// Hoard signing authority PDA.
pub const IX_HOARD_AUTHORITY: usize = 12;
/// Hoard Token-2022 collateral account (writable).
pub const IX_HOARD_TOKEN: usize = 13;
/// Claimant-owned outcome-token source (writable).
pub const IX_SOURCE: usize = 14;
/// First mint in the canonical complete outcome-mint suffix.
pub const IX_OUTCOME_MINTS: usize = 15;

const STATE_ROLES: [StateRole; 6] = [
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
    StateRole::read_only(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_SUPPLY, account_len::SUPPLY_LEDGER),
    StateRole::read_only(IX_TERMS, account_len::TERMS),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionBinding {
    Legacy { payout_index: u8 },
    Native { vector: PayoutVectorBytes },
}

impl ResolutionBinding {
    const fn kernel_index(self) -> u8 {
        match self {
            Self::Legacy { payout_index } => payout_index,
            Self::Native { .. } => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExitRequest {
    market: Hash32,
    claimant: Hash32,
    source: Hash32,
    destination: Hash32,
    outcome: u8,
    quantity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenSnapshot {
    source: u64,
    destination: u64,
    hoard: u64,
    decimals: u8,
    authority_bump: u8,
    mint_index: usize,
    observed: ObservedMintSupplies,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeRedeemStep {
    total_supply: [u64; clutch_solana_layout::MAX_OUTCOMES],
    payout: u64,
}

#[inline(never)]
fn decode_request(request: &Request) -> Outcome<ExitRequest> {
    require(request.sequence == 0, ClutchError::Replay)?;
    match request.action {
        Action::Layout(Intent::RedeemExternal {
            market,
            claimant,
            source,
            destination,
            outcome,
            quantity,
        }) => Ok(ExitRequest {
            market,
            claimant,
            source,
            destination,
            outcome,
            quantity,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Degree is immutable Terms state and therefore selects the Resolution ABI.
///
/// This lives in its own frame because a full Terms decode is much larger than
/// the one-byte fact its caller retains.
#[inline(never)]
fn terms_basis_degree(terms_bytes: &[u8]) -> Outcome<u8> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_into(terms_bytes, &mut terms)?;
    Ok(terms.basis_degree)
}

#[inline(never)]
fn require_resolved_kernel_head(
    kernel_bytes: &[u8],
    market: Hash32,
    resolution_payout: u8,
    payout_count: u8,
    outcome_count: u8,
) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_bytes)?;
    require(
        kernel.market == market
            && kernel.phase == 1
            && kernel.resolved_payout == resolution_payout
            && kernel.payouts.count == payout_count
            && kernel.payouts.outcomes == outcome_count,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn kernel_redeem(
    kernel_data: &mut [u8],
    outcome_count: u8,
    collateral: u64,
    outcome: u8,
    quantity: u64,
) -> Outcome<u64> {
    let mut account = KernelAccount::decode(kernel_data)?;
    require(account.phase == 1, ClutchError::NotActive)?;
    let mut market = MarketState {
        outcomes: outcome_count,
        phase: Phase::Resolved,
        resolved_payout: account.resolved_payout,
        basis_mode: BasisMode::FinitePreset,
        resolved_vector: PayoutVector::ZERO,
        collateral,
        total_supply: account.total_supply,
        payouts: account.payouts,
    };
    let mut position = Position::EMPTY;
    position.external[usize::from(outcome)] = quantity;
    let payout = market.redeem_external(&mut position, outcome, quantity)?;
    account.total_supply = market.total_supply;
    account.encode(kernel_data)?;
    Ok(payout)
}

/// Redeem bearer claims against the v3 record-owned native vector.
///
/// The vector exists only in this stack frame. `reconstruct_native_market` is
/// the same projection used by internal redemption, so bearer exit cannot
/// invent a second interpretation or persist a copy into `KernelAccount`.
#[inline(never)]
fn kernel_redeem_native(
    kernel_data: &mut [u8],
    outcome_count: u8,
    collateral: u64,
    vector: PayoutVectorBytes,
    outcome: u8,
    quantity: u64,
) -> Outcome<u64> {
    let step = kernel_redeem_native_step(
        kernel_data,
        outcome_count,
        collateral,
        vector,
        outcome,
        quantity,
    )?;
    write_kernel_totals(kernel_data, &step.total_supply)?;
    Ok(step.payout)
}

/// Run the large derived market in a frame that holds no decoded aggregate
/// write-back value.
#[inline(never)]
fn kernel_redeem_native_step(
    kernel_data: &[u8],
    outcome_count: u8,
    collateral: u64,
    vector: PayoutVectorBytes,
    outcome: u8,
    quantity: u64,
) -> Outcome<NativeRedeemStep> {
    let installed = PayoutVector::new(vector.denominator, vector.weights);
    let mut market = reconstruct_native_market(kernel_data, outcome_count, collateral, installed)?;
    let mut position = Position::EMPTY;
    position.external[usize::from(outcome)] = quantity;
    let payout = market.redeem_external(&mut position, outcome, quantity)?;
    Ok(NativeRedeemStep {
        total_supply: market.total_supply,
        payout,
    })
}

/// Persist only the aggregate-supply projection. The record-owned vector is
/// not part of this write frame and cannot become a second persisted copy.
#[inline(never)]
fn write_kernel_totals(
    kernel_data: &mut [u8],
    total_supply: &[u64; clutch_solana_layout::MAX_OUTCOMES],
) -> Outcome<()> {
    let mut account = KernelAccount::decode(kernel_data)?;
    account.total_supply = *total_supply;
    account.encode(kernel_data)?;
    Ok(())
}

#[inline(never)]
fn admit_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market: &accounts::MarketFacts,
    hoard: &HoardAccount,
    request: &ExitRequest,
) -> Outcome<TokenSnapshot> {
    validate_token_program(&accounts[IX_TOKEN_PROGRAM])?;
    let observed = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        IX_OUTCOME_MINTS,
        *accounts[IX_MARKET].key,
        market.market,
        market.outcome_count,
        Some(request.outcome),
    )?;
    let mint_index = IX_OUTCOME_MINTS + usize::from(request.outcome);
    let policy_account = &accounts[IX_POLICY];
    require(
        !policy_account.is_writable
            && !policy_account.executable
            && policy_account.data_len() == collateral::COLLATERAL_POLICY_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let profile = ProfileAccount::decode(&accounts[IX_PROFILE].data.borrow())?;
    let policy = collateral::verify_profile_identity(&policy_account.data.borrow(), &profile)?;
    token::require_drivable_collateral(&policy)?;

    let collateral_mint = &accounts[IX_COLLATERAL_MINT];
    require(
        !collateral_mint.is_writable,
        ClutchError::UnexpectedWritable,
    )?;
    let collateral_observation =
        token::admit_mint(collateral_mint, &token::MintPolicy::collateral(&policy))?;
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

    let source = &accounts[IX_SOURCE];
    let destination = &accounts[IX_DESTINATION];
    let hoard_token = &accounts[IX_HOARD_TOKEN];
    require(
        source.is_writable && destination.is_writable && hoard_token.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !source.executable && !destination.executable && !hoard_token.executable,
        ClutchError::ExecutableAccount,
    )?;
    expect_pda(
        hoard_token.key,
        seeds::hoard_token_pda(program_id, &market.market.bytes()),
        None,
    )?;
    let source_observation = token::admit_token_account(
        source,
        &token::TokenAccountPolicy::holder(*accounts[mint_index].key, *accounts[IX_CLAIMANT].key),
    )?;
    let destination_observation = token::admit_token_account(
        destination,
        &token::TokenAccountPolicy::collateral_holder(&policy, *accounts[IX_CLAIMANT].key),
    )?;
    let hoard_observation = token::admit_token_account(
        hoard_token,
        &token::TokenAccountPolicy::hoard(&policy, *authority.key),
    )?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, hoard_observation.amount)?;
    require(
        source_observation.amount >= request.quantity,
        ClutchError::TokenDeltaMismatch,
    )?;
    Ok(TokenSnapshot {
        source: source_observation.amount,
        destination: destination_observation.amount,
        hoard: hoard_observation.amount,
        decimals: collateral_observation.decimals,
        authority_bump: authority_derived.1,
        mint_index,
        observed,
    })
}

/// Burn bearer claims and pay their exact resolved collateral value.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let request = decode_request(request)?;
    require(
        accounts.len() >= IX_OUTCOME_MINTS,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_CLAIMANT])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &STATE_ROLES)?;
    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    require(
        accounts.len() == IX_OUTCOME_MINTS + usize::from(market.outcome_count),
        ClutchError::AccountCount,
    )?;
    require(
        request.market == market.market
            && request.claimant.bytes() == accounts[IX_CLAIMANT].key.to_bytes()
            && request.source.bytes() == accounts[IX_SOURCE].key.to_bytes()
            && request.destination.bytes() == accounts[IX_DESTINATION].key.to_bytes()
            && usize::from(request.outcome) < usize::from(market.outcome_count),
        ClutchError::MismatchedState,
    )?;
    let terms = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?;
    let basis_degree = terms_basis_degree(&accounts[IX_TERMS].data.borrow())?;
    let resolution_len = if basis_degree == 0 {
        account_len::RESOLUTION
    } else {
        NATIVE_RESOLUTION_LEN
    };
    accounts::validate_state_roles(
        program_id,
        accounts,
        &[StateRole::read_only(IX_RESOLUTION, resolution_len)],
    )?;
    let mut hoard = HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?;
    let supply = accounts::read_supply(&accounts[IX_SUPPLY].data.borrow())?;
    let profile = accounts::read_profile(&accounts[IX_PROFILE].data.borrow())?;
    expect_pda(
        accounts[IX_PROFILE].key,
        seeds::profile_pda(program_id, &market.realm.bytes(), &market.profile.bytes()),
        None,
    )?;
    expect_pda(
        accounts[IX_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_HOARD].key,
        seeds::hoard_pda(program_id, &market.market.bytes()),
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        accounts[IX_KERNEL].key,
        seeds::kernel_pda(program_id, &market.market.bytes()),
        None,
    )?;
    expect_pda(
        accounts[IX_SUPPLY].key,
        seeds::supply_pda(program_id, &market.market.bytes()),
        Some(supply.stored_bump),
    )?;
    expect_pda(
        accounts[IX_TERMS].key,
        seeds::terms_pda(program_id, &market.realm.bytes(), &market.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    require(
        market.lifecycle == 1
            && hoard.market == market.market
            && hoard.realm == market.realm
            && supply.market == market.market
            && supply.realm == market.realm
            && supply.outcome_count == market.outcome_count
            && profile.profile == market.profile
            && profile.realm == market.realm,
        ClutchError::MismatchedState,
    )?;
    /* Terms and Resolution were fully decoded (including the terms digest) by
     * the small-facts readers above.  Resolve already bound the frozen payout
     * set into the program-owned kernel; redemption preserves that inductive
     * fact and need not hold Terms+Kernel together in one unsafe SBF frame. */
    require(
        terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && terms.feed == market.feed
            && terms.outcome_count == market.outcome_count
            && terms.collateral_cap == market.collateral_cap,
        ClutchError::MismatchedState,
    )?;
    let resolution_pda = seeds::resolution_pda(program_id, &market.market.bytes());
    expect_pda(accounts[IX_RESOLUTION].key, resolution_pda, None)?;
    let resolution = if basis_degree == 0 {
        let record = accounts::read_resolution(&accounts[IX_RESOLUTION].data.borrow())?;
        require(
            record.stored_bump == resolution_pda.1
                && record.market == market.market
                && record.terms == terms.terms
                && record.feed == terms.feed
                && record.resolved
                && record.payout_index < terms.payout_count,
            ClutchError::MismatchedState,
        )?;
        ResolutionBinding::Legacy {
            payout_index: record.payout_index,
        }
    } else {
        let bound = bound_native_resolution(
            &accounts[IX_RESOLUTION].data.borrow(),
            &accounts[IX_TERMS].data.borrow(),
            resolution_pda.1,
            market.market,
        )?;
        ResolutionBinding::Native {
            vector: bound.vector,
        }
    };
    require_resolved_kernel_head(
        &accounts[IX_KERNEL].data.borrow(),
        market.market,
        resolution.kernel_index(),
        terms.payout_count,
        terms.outcome_count,
    )?;
    let snapshot = admit_tokens(program_id, accounts, &market, &hoard, &request)?;

    {
        let mut supply_data = accounts[IX_SUPPLY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut kernel_data = accounts[IX_KERNEL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_truth::synchronize_external_truth(
            &mut supply_data,
            &mut kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &snapshot.observed,
        )?;
    }
    let payout = {
        let mut kernel_data = accounts[IX_KERNEL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        match resolution {
            ResolutionBinding::Legacy { .. } => kernel_redeem(
                &mut kernel_data,
                market.outcome_count,
                hoard.collateral_atoms,
                request.outcome,
                request.quantity,
            )?,
            ResolutionBinding::Native { vector } => kernel_redeem_native(
                &mut kernel_data,
                market.outcome_count,
                hoard.collateral_atoms,
                vector,
                request.outcome,
                request.quantity,
            )?,
        }
    };

    token::burn(
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_SOURCE],
        &accounts[snapshot.mint_index],
        &accounts[IX_CLAIMANT],
        request.quantity,
    )?;
    if payout != 0 {
        let market_bytes = market.market.bytes();
        let bump = [snapshot.authority_bump];
        let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY, &market_bytes, &bump];
        token::transfer_checked_signed(
            &accounts[IX_TOKEN_PROGRAM],
            &accounts[IX_HOARD_TOKEN],
            &accounts[IX_COLLATERAL_MINT],
            &accounts[IX_DESTINATION],
            &accounts[IX_HOARD_AUTHORITY],
            payout,
            snapshot.decimals,
            &signer,
        )?;
    }

    let after = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        IX_OUTCOME_MINTS,
        *accounts[IX_MARKET].key,
        market.market,
        market.outcome_count,
        Some(request.outcome),
    )?;
    claim_truth::require_exact_mint_vector_delta(
        &snapshot.observed,
        &after,
        Some((request.outcome, false, request.quantity)),
    )?;
    token::require_exact_debit(
        snapshot.source,
        token::token_amount(&accounts[IX_SOURCE])?,
        request.quantity,
    )?;
    token::require_exact_credit(
        snapshot.destination,
        token::token_amount(&accounts[IX_DESTINATION])?,
        payout,
    )?;
    token::require_exact_debit(
        snapshot.hoard,
        token::token_amount(&accounts[IX_HOARD_TOKEN])?,
        payout,
    )?;
    hoard.collateral_atoms = hoard
        .collateral_atoms
        .checked_sub(payout)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    token::require_hoard_covers_collateral(
        hoard.collateral_atoms,
        token::token_amount(&accounts[IX_HOARD_TOKEN])?,
    )?;
    {
        let mut supply_data = accounts[IX_SUPPLY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let kernel_data = accounts[IX_KERNEL]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_truth::commit_observed_supplies(
            &mut supply_data,
            &kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &after,
        )?;
    }
    hoard.encode(
        &mut accounts[IX_HOARD]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_kernel::{Error as KernelError, PayoutSet, MAX_PAYOUTS};
    use clutch_solana_layout::MAX_OUTCOMES;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn encoded_kernel(vector: PayoutVector, supply: u64) -> Vec<u8> {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = vector;
        let mut total = [0_u64; MAX_OUTCOMES];
        total[0] = supply;
        total[1] = supply;
        let mut bytes = vec![0; KERNEL_ACCOUNT_LEN];
        KernelAccount {
            market: h(1),
            phase: 1,
            resolved_payout: 0,
            payouts: PayoutSet::new(1, 2, vectors),
            total_supply: total,
        }
        .encode(&mut bytes)
        .unwrap();
        bytes
    }

    #[test]
    fn bearer_redemption_needs_no_owner_position() {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        let mut bytes = encoded_kernel(PayoutVector::new(1, weights), 5);
        assert_eq!(kernel_redeem(&mut bytes, 2, 5, 0, 2), Ok(2));
        let after = KernelAccount::decode(&bytes).unwrap();
        assert_eq!(after.total_supply[0], 3);
        assert_eq!(after.total_supply[1], 5);
    }

    #[test]
    fn inexact_fractional_exit_refuses_without_persisting() {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        let mut bytes = encoded_kernel(PayoutVector::new(2, weights), 5);
        let before = bytes.clone();
        assert_eq!(
            kernel_redeem(&mut bytes, 2, 5, 0, 1),
            Err(Refusal::Kernel(KernelError::RemainderRequired))
        );
        assert_eq!(bytes, before);
    }

    #[test]
    fn native_bearer_redemption_uses_the_ephemeral_vector_exactly() {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        let vector = PayoutVectorBytes {
            denominator: 2,
            weights,
        };
        let mut preset_weights = [0_u64; MAX_OUTCOMES];
        preset_weights[0] = 2;
        let mut bytes = encoded_kernel(PayoutVector::new(2, preset_weights), 5);
        let before = bytes.clone();
        assert_eq!(
            kernel_redeem_native(&mut bytes, 2, 5, vector, 0, 1),
            Err(Refusal::Kernel(KernelError::RemainderRequired))
        );
        assert_eq!(bytes, before);

        assert_eq!(kernel_redeem_native(&mut bytes, 2, 5, vector, 0, 2), Ok(1));
        let after = KernelAccount::decode(&bytes).unwrap();
        assert_eq!(after.total_supply[0], 3);
        assert_eq!(after.total_supply[1], 5);
        assert_eq!(
            after.payouts,
            KernelAccount::decode(&before).unwrap().payouts
        );
    }

    #[test]
    fn corrupt_native_kernel_prestate_refuses_before_write() {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        let vector = PayoutVectorBytes {
            denominator: 2,
            weights,
        };
        let mut preset_weights = [0_u64; MAX_OUTCOMES];
        preset_weights[0] = 2;
        let mut bytes = encoded_kernel(PayoutVector::new(2, preset_weights), 6);
        let before = bytes.clone();
        assert!(kernel_redeem_native(&mut bytes, 2, 5, vector, 0, 2).is_err());
        assert_eq!(bytes, before);
    }

    #[test]
    fn bearer_exit_has_no_replay_counter_but_requires_zero_envelope_sequence() {
        let action = Action::Layout(Intent::RedeemExternal {
            market: h(1),
            claimant: h(2),
            source: h(3),
            destination: h(4),
            outcome: 0,
            quantity: 1,
        });
        assert!(decode_request(&Request {
            sequence: 0,
            action
        })
        .is_ok());
        assert_eq!(
            decode_request(&Request {
                sequence: 1,
                action
            }),
            Err(ClutchError::Replay.into())
        );
    }
}
