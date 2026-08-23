//! Positionless redemption of bearer Token-2022 outcome claims.
//!
//! External Eggs are ordinary bearer tokens.  Their owner, transfer history,
//! and the Position that first materialized them are irrelevant at redemption:
//! the claimant proves authority over the exact source token account, burns the
//! claim, and receives the immutable resolved payout from the pooled Hoard.
//! Token-2022 mint supply is the aggregate external truth.

use crate::accounts::{self, expect_pda, require, require_signer, Outcome, StateRole};
use crate::claim_truth::{self, ObservedMintSupplies};
use crate::collateral_release::authenticate_realm_collateral_v2;
use crate::error::{ClutchError, Refusal};
use crate::instructions::observe_resolve::{bound_native_resolution, reconstruct_native_market};
use crate::instructions::split::validate_token_program;
use crate::{seeds, token};
use clutch_collateral_adapter_v2::{
    accept_claim_redemption_collateral_v2, admit_collateral_account_v2, admit_collateral_mint_v2,
    prepare_claim_redemption_collateral_v2, refine_market_collateral_v2, BoundCollateralProfileV2,
    ClaimRedemptionCollateralRequestV2, CollateralBackingV2, CpiAccountMetaV2, Id as CollateralId,
    MarketCollateralBindingV2, MintObservationV2, PreparedClaimRedemptionCollateralV2,
    RuntimeAccountViewV2, TokenAccountObservationV2, TokenAccountRoleV2, TransferAuthorityKindV2,
    TransferAuthorityV2,
};
use clutch_kernel::{BasisMode, MarketState, PayoutVector, Phase, Position};
use clutch_solana_layout::{
    account_len,
    native_resolution::NATIVE_RESOLUTION_LEN,
    occupation_resolution::{is_occupation_statistic, OCCUPATION_RESOLUTION_LEN},
    Hash32, HoardAccount, Intent, PayoutVectorBytes, TermsAccount,
};
use clutch_solana_reference::{Action, KernelAccount, Request, KERNEL_ACCOUNT_LEN};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
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
/// Realm-selected collateral token program.
pub const IX_COLLATERAL_TOKEN_PROGRAM: usize = 9;
/// Compatibility name for the pre-split collateral program role.
pub const IX_TOKEN_PROGRAM: usize = IX_COLLATERAL_TOKEN_PROGRAM;
/// Frozen collateral mint.
pub const IX_COLLATERAL_MINT: usize = 10;
/// Claimant-owned collateral destination (writable).
pub const IX_DESTINATION: usize = 11;
/// Hoard signing authority PDA.
pub const IX_HOARD_AUTHORITY: usize = 12;
/// Release-selected Hoard collateral account (writable).
pub const IX_HOARD_TOKEN: usize = 13;
/// Claimant-owned outcome-token source (writable).
pub const IX_SOURCE: usize = 14;
/// Immutable Realm account selecting Profile V2.
pub const IX_REALM: usize = 15;
/// Separately fixed Token-2022 outcome claim program.
pub const IX_OUTCOME_TOKEN_PROGRAM: usize = 16;
/// First mint in the canonical complete outcome-mint suffix.
pub const IX_OUTCOME_MINTS: usize = 17;

const STATE_ROLES: [StateRole; 7] = [
    StateRole::read_only(IX_REALM, account_len::REALM),
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
    authority_bump: u8,
    mint_index: usize,
    observed: ObservedMintSupplies,
    collateral_mint: MintObservationV2,
    collateral_destination: TokenAccountObservationV2,
    collateral_hoard: TokenAccountObservationV2,
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
    expected_mode: BasisMode,
    resolution_payout: u8,
    payout_count: u8,
    outcome_count: u8,
) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_bytes)?;
    if kernel.basis_mode != expected_mode {
        return Err(clutch_kernel::Error::WrongResolutionMode.into());
    }
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
    if account.basis_mode != BasisMode::FinitePreset {
        return Err(clutch_kernel::Error::WrongResolutionMode.into());
    }
    require(account.phase == 1, ClutchError::NotActive)?;
    let mut market = MarketState {
        outcomes: outcome_count,
        phase: Phase::Resolved,
        resolved_payout: account.resolved_payout,
        basis_mode: account.basis_mode,
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

/// Redeem bearer claims against the v3/v4 record-owned native vector.
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

fn runtime_account_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: CollateralId::from_bytes(account.key.to_bytes()),
        owner_program: CollateralId::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

/// Collateral and claim programs are independent roles even when both resolve
/// to Token-2022. Solana coalesces duplicate account metas, so this is the sole
/// admitted alias; every state, mint, token account, and authority stays
/// pairwise distinct.
fn require_redemption_account_distinctness(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let program_alias =
                left == IX_COLLATERAL_TOKEN_PROGRAM && right == IX_OUTCOME_TOKEN_PROGRAM;
            require(
                accounts[left].key != accounts[right].key || program_alias,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[inline(never)]
fn admit_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market: &accounts::MarketFacts,
    hoard: &HoardAccount,
    request: &ExitRequest,
    bound: BoundCollateralProfileV2,
) -> Outcome<TokenSnapshot> {
    validate_token_program(&accounts[IX_OUTCOME_TOKEN_PROGRAM])?;
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
    let collateral_mint = &accounts[IX_COLLATERAL_MINT];
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
    let collateral_mint_data = collateral_mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let collateral_destination_data = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let collateral_hoard_data = hoard_token
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let collateral_mint_observation = admit_collateral_mint_v2(
        bound,
        runtime_account_view(collateral_mint, &collateral_mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let collateral_destination_observation = admit_collateral_account_v2(
        bound,
        runtime_account_view(destination, &collateral_destination_data),
        TokenAccountRoleV2::ReceiveOnly {
            account: CollateralId::from_bytes(destination.key.to_bytes()),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let collateral_hoard_observation = admit_collateral_account_v2(
        bound,
        runtime_account_view(hoard_token, &collateral_hoard_data),
        TokenAccountRoleV2::Hoard,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        collateral_hoard_observation.amount_atoms >= hoard.collateral_atoms,
        ClutchError::TokenDeltaMismatch,
    )?;
    require(
        source_observation.amount >= request.quantity,
        ClutchError::TokenDeltaMismatch,
    )?;
    Ok(TokenSnapshot {
        source: source_observation.amount,
        authority_bump: authority_derived.1,
        mint_index,
        observed,
        collateral_mint: collateral_mint_observation,
        collateral_destination: collateral_destination_observation,
        collateral_hoard: collateral_hoard_observation,
    })
}

fn claim_redemption_id(
    market: Hash32,
    resolution_account: &Pubkey,
    request: &ExitRequest,
    payout_atoms: u64,
    source_supply_before: u64,
) -> CollateralId {
    const DOMAIN: &[u8] = b"dragons-clutch/external-claim-redemption/v2\0";
    CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DOMAIN,
            &market.bytes(),
            &resolution_account.to_bytes(),
            &request.claimant.bytes(),
            &request.source.bytes(),
            &request.destination.bytes(),
            &[request.outcome],
            &request.quantity.to_le_bytes(),
            &payout_atoms.to_le_bytes(),
            &source_supply_before.to_le_bytes(),
        ])
        .to_bytes(),
    )
}

fn cpi_account_meta(value: CpiAccountMetaV2) -> AccountMeta {
    AccountMeta {
        pubkey: Pubkey::new_from_array(value.address.bytes()),
        is_signer: value.signer,
        is_writable: value.writable,
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke_claim_collateral_payout<'a>(
    prepared: PreparedClaimRedemptionCollateralV2,
    mint: &AccountInfo<'a>,
    hoard: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer: &[&[u8]],
) -> Outcome<clutch_collateral_adapter_v2::AcceptedClaimRedemptionCollateralV2> {
    let cpi = prepared.cpi();
    require(
        cpi.program_signed
            && cpi.token_program == CollateralId::from_bytes(token_program.key.to_bytes())
            && cpi.accounts[0].address == CollateralId::from_bytes(hoard.key.to_bytes())
            && cpi.accounts[1].address == CollateralId::from_bytes(mint.key.to_bytes())
            && cpi.accounts[2].address == CollateralId::from_bytes(destination.key.to_bytes())
            && cpi.accounts[3].address == CollateralId::from_bytes(authority.key.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(cpi_account_meta).collect(),
    );
    let account_infos = [
        hoard.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    invoke_signed(&instruction, &account_infos, &[signer])
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;

    let mint_after = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_after = hoard
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_claim_redemption_collateral_v2(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(hoard, &hoard_after),
        runtime_account_view(destination, &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// Burn bearer claims and pay their exact resolved collateral value.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let request = decode_request(request)?;
    require(
        accounts.len() >= IX_OUTCOME_MINTS,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_CLAIMANT])?;
    require_redemption_account_distinctness(accounts)?;
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
    let occupation = is_occupation_statistic(terms.statistic_id);
    #[cfg(not(feature = "profile-full"))]
    require(!occupation, ClutchError::UnsupportedInstruction)?;
    let resolution_len = if basis_degree == 0 {
        account_len::RESOLUTION
    } else if occupation {
        OCCUPATION_RESOLUTION_LEN
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
    let realm_collateral = authenticate_realm_collateral_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
    )?;
    let bound_collateral = refine_market_collateral_v2(
        realm_collateral,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market.market.bytes()),
            realm: CollateralId::from_bytes(market.realm.bytes()),
            profile: CollateralId::from_bytes(market.profile.bytes()),
            collateral_cap_atoms: market.collateral_cap,
            hoard_authority: CollateralId::from_bytes(accounts[IX_HOARD_AUTHORITY].key.to_bytes()),
            hoard_token_account: CollateralId::from_bytes(accounts[IX_HOARD_TOKEN].key.to_bytes()),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
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
        if basis_degree == 0 {
            BasisMode::FinitePreset
        } else {
            BasisMode::DerivedBasis
        },
        resolution.kernel_index(),
        terms.payout_count,
        terms.outcome_count,
    )?;
    let snapshot = admit_tokens(
        program_id,
        accounts,
        &market,
        &hoard,
        &request,
        bound_collateral,
    )?;

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

    let prepared_collateral = if payout == 0 {
        None
    } else {
        let mint_data = accounts[IX_COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[IX_HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[IX_DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        Some(
            prepare_claim_redemption_collateral_v2(
                bound_collateral,
                ClaimRedemptionCollateralRequestV2 {
                    claim_redemption_id: claim_redemption_id(
                        market.market,
                        accounts[IX_RESOLUTION].key,
                        &request,
                        payout,
                        snapshot.observed.values[usize::from(request.outcome)],
                    ),
                    destination_token_account: CollateralId::from_bytes(
                        accounts[IX_DESTINATION].key.to_bytes(),
                    ),
                    claim_semantic_owner: CollateralId::from_bytes(request.claimant.bytes()),
                    payout_atoms: payout,
                    backing_before: CollateralBackingV2 {
                        locked_atoms: hoard.collateral_atoms,
                        cap_atoms: market.collateral_cap,
                    },
                },
                TransferAuthorityV2 {
                    address: CollateralId::from_bytes(accounts[IX_HOARD_AUTHORITY].key.to_bytes()),
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[IX_HOARD_AUTHORITY].is_writable,
                    executable: accounts[IX_HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[IX_HOARD_AUTHORITY].data_is_empty(),
                },
                runtime_account_view(&accounts[IX_COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[IX_HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[IX_DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?,
        )
    };

    token::burn(
        &accounts[IX_OUTCOME_TOKEN_PROGRAM],
        &accounts[IX_SOURCE],
        &accounts[snapshot.mint_index],
        &accounts[IX_CLAIMANT],
        request.quantity,
    )?;
    let accepted_collateral = if let Some(prepared) = prepared_collateral {
        let market_bytes = market.market.bytes();
        let bump = [snapshot.authority_bump];
        let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY, &market_bytes, &bump];
        Some(invoke_claim_collateral_payout(
            prepared,
            &accounts[IX_COLLATERAL_MINT],
            &accounts[IX_HOARD_TOKEN],
            &accounts[IX_DESTINATION],
            &accounts[IX_HOARD_AUTHORITY],
            &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
            &signer,
        )?)
    } else {
        None
    };

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
    let collateral_after = hoard
        .collateral_atoms
        .checked_sub(payout)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    if let Some(accepted) = accepted_collateral {
        require(
            accepted.backing_after.locked_atoms == collateral_after
                && accepted.backing_after.cap_atoms == market.collateral_cap,
            ClutchError::TokenDeltaMismatch,
        )?;
    } else {
        let mint_data = accounts[IX_COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[IX_DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[IX_HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mint_after = admit_collateral_mint_v2(
            bound_collateral,
            runtime_account_view(&accounts[IX_COLLATERAL_MINT], &mint_data),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
        let destination_after = admit_collateral_account_v2(
            bound_collateral,
            runtime_account_view(&accounts[IX_DESTINATION], &destination_data),
            TokenAccountRoleV2::ReceiveOnly {
                account: CollateralId::from_bytes(accounts[IX_DESTINATION].key.to_bytes()),
            },
        )
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
        let hoard_after = admit_collateral_account_v2(
            bound_collateral,
            runtime_account_view(&accounts[IX_HOARD_TOKEN], &hoard_data),
            TokenAccountRoleV2::Hoard,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
        require(
            payout == 0
                && mint_after == snapshot.collateral_mint
                && destination_after == snapshot.collateral_destination
                && hoard_after == snapshot.collateral_hoard,
            ClutchError::TokenDeltaMismatch,
        )?;
    }
    hoard.collateral_atoms = collateral_after;
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
    use clutch_solana_reference::Error as ReferenceError;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn encoded_kernel(vector: PayoutVector, supply: u64, basis_mode: BasisMode) -> Vec<u8> {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = vector;
        let mut total = [0_u64; MAX_OUTCOMES];
        total[0] = supply;
        total[1] = supply;
        let mut bytes = vec![0; KERNEL_ACCOUNT_LEN];
        KernelAccount {
            market: h(1),
            phase: 1,
            basis_mode,
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
        let mut bytes = encoded_kernel(PayoutVector::new(1, weights), 5, BasisMode::FinitePreset);
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
        let mut bytes = encoded_kernel(PayoutVector::new(2, weights), 5, BasisMode::FinitePreset);
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
        let mut bytes = encoded_kernel(
            PayoutVector::new(2, preset_weights),
            5,
            BasisMode::DerivedBasis,
        );
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
        let mut bytes = encoded_kernel(
            PayoutVector::new(2, preset_weights),
            6,
            BasisMode::DerivedBasis,
        );
        let before = bytes.clone();
        assert!(kernel_redeem_native(&mut bytes, 2, 5, vector, 0, 2).is_err());
        assert_eq!(bytes, before);
    }

    #[test]
    fn categorical_and_native_redemption_refuse_opposite_stored_modes_without_write() {
        let mut preset_weights = [0_u64; MAX_OUTCOMES];
        preset_weights[0] = 1;
        let preset = PayoutVector::new(1, preset_weights);

        let mut derived_bytes = encoded_kernel(preset, 5, BasisMode::DerivedBasis);
        let derived_before = derived_bytes.clone();
        assert_eq!(
            kernel_redeem(&mut derived_bytes, 2, 5, 0, 1),
            Err(Refusal::Kernel(KernelError::WrongResolutionMode))
        );
        assert_eq!(derived_bytes, derived_before);

        let mut finite_bytes = encoded_kernel(preset, 5, BasisMode::FinitePreset);
        let finite_before = finite_bytes.clone();
        assert_eq!(
            kernel_redeem_native(
                &mut finite_bytes,
                2,
                5,
                PayoutVectorBytes {
                    denominator: 1,
                    weights: preset_weights,
                },
                0,
                1,
            ),
            Err(Refusal::Reference(ReferenceError::Kernel(
                KernelError::WrongResolutionMode
            )))
        );
        assert_eq!(finite_bytes, finite_before);
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
