//! Exact fractional-redemption successor over canonical full-width accounts.
//!
//! The first executable slice is action 2, exact internal redemption. It
//! mutates only the sole owners of the affected facts: Position V3 and GEN1
//! Replay for claimant state, ClaimLedger V3 for native supply, Hoard V2 for
//! locked-principal/cash classification, and `0xa5/v1` for the global
//! fractional sequence and aggregate numerator credit. The immutable
//! `0xa4/v2` policy commits the exact PDA-bound Resolution V5 data identity.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::claim_release::authenticate_claim_issuance_v1;
use crate::error::{ClutchError, Refusal};
use crate::{seeds, token};
use clutch_collateral_adapter_v2::{
    accept_fractional_bearer_claim_burn_v3, prepare_claim_redemption_collateral_v2,
    prepare_fractional_bearer_claim_burn_v3, prepare_zero_claim_redemption_collateral_v2,
    AcceptedBearerRedemptionCollateralV3, Id as CollateralId, TransferAuthorityKindV2,
    TransferAuthorityV2,
};
use clutch_fractional_redemption_runtime::{
    accept_bearer_exact_burn_v1, bind_fractional_context_v1, bind_fractional_internal_context_v1,
    finish_bearer_exact_v1, prepare_bearer_exact_v1, redeem_internal_exact_v1,
    seal_claims_exhausted_v1, BearerClaimPrestateV1, Error as FractionalError, FractionalLedgerV1,
    FractionalPolicyV2, FractionalRedeemIntentV1, FractionalRedemptionActionV1,
    FractionalTerminalIntentV1, InternalPositionV1, RedemptionSourcePoststateV1,
    FRACTIONAL_LEDGER_ACCOUNT_BYTES, FRACTIONAL_POLICY_ACCOUNT_BYTES,
};
use clutch_retirement::{Identity32V1, POSITION_V3_BYTES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    authenticate_resolution_v5,
};
use super::external_redemption_v3::{
    accept_zero_claim_collateral_payout, bearer_claim_observation_v3,
    invoke_claim_collateral_payout, observe_outcome_mints_for_bearer_v3, runtime_account_view,
};

/// Exact account count for action 2.
pub const REDEEM_INTERNAL_EXACT_ACCOUNT_COUNT_V1: usize = 15;
/// Fixed exact-bearer prefix before one canonical mint per active outcome.
pub const REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1: usize = 19;
/// Exact fixed account count for the supply-exhaustion seal.
pub const SEAL_CLAIMS_EXHAUSTED_ACCOUNT_COUNT_V1: usize = 12;

const IX_ACTOR: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_COLLATERAL_POLICY: usize = 3;
const IX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_MARKET_INSTANCE: usize = 7;
const IX_HOARD: usize = 8;
const IX_CLAIM_LEDGER: usize = 9;
const IX_RESOLUTION: usize = 10;
const IX_FRACTIONAL_POLICY: usize = 11;
const IX_FRACTIONAL_LEDGER: usize = 12;
const IX_POSITION: usize = 13;
const IX_REPLAY: usize = 14;

mod bearer_ix {
    pub const CLAIMANT: usize = 0;
    pub const REALM: usize = 1;
    pub const PROFILE: usize = 2;
    pub const COLLATERAL_POLICY: usize = 3;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 4;
    pub const MARKET_BINDING: usize = 5;
    pub const MARKET_RUNTIME: usize = 6;
    pub const MARKET_INSTANCE: usize = 7;
    pub const HOARD: usize = 8;
    pub const CLAIM_LEDGER: usize = 9;
    pub const RESOLUTION: usize = 10;
    pub const FRACTIONAL_POLICY: usize = 11;
    pub const FRACTIONAL_LEDGER: usize = 12;
    pub const COLLATERAL_MINT: usize = 13;
    pub const DESTINATION: usize = 14;
    pub const HOARD_AUTHORITY: usize = 15;
    pub const HOARD_TOKEN: usize = 16;
    pub const OUTCOME_TOKEN_PROGRAM: usize = 17;
    pub const SOURCE: usize = 18;
    pub const OUTCOME_MINTS: usize = super::REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1;
}

mod seal_ix {
    pub const REALM: usize = 0;
    pub const PROFILE: usize = 1;
    pub const COLLATERAL_POLICY: usize = 2;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 3;
    pub const MARKET_BINDING: usize = 4;
    pub const MARKET_RUNTIME: usize = 5;
    pub const MARKET_INSTANCE: usize = 6;
    pub const HOARD: usize = 7;
    pub const CLAIM_LEDGER: usize = 8;
    pub const RESOLUTION: usize = 9;
    pub const FRACTIONAL_POLICY: usize = 10;
    pub const FRACTIONAL_LEDGER: usize = 11;
}

fn map_fractional(error: FractionalError) -> Refusal {
    match error {
        FractionalError::ReplayMismatch | FractionalError::ReplayRefused => {
            Refusal::Adapter(ClutchError::Replay)
        }
        FractionalError::Arithmetic => Refusal::Adapter(ClutchError::Arithmetic),
        FractionalError::Truncated
        | FractionalError::TrailingBytes
        | FractionalError::WrongTag
        | FractionalError::WrongVersion
        | FractionalError::NonCanonicalPadding => Refusal::Adapter(ClutchError::NonCanonical),
        _ => Refusal::Adapter(ClutchError::MismatchedState),
    }
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

fn decode_fractional_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    policy_index: usize,
    ledger_index: usize,
    resolution_index: usize,
) -> Outcome<(FractionalPolicyV2, FractionalLedgerV1)> {
    require_program_state(
        program_id,
        &accounts[policy_index],
        false,
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        &accounts[ledger_index],
        true,
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    )?;
    let policy = FractionalPolicyV2::decode(&accounts[policy_index].data.borrow())
        .map_err(map_fractional)?;
    let ledger = FractionalLedgerV1::decode(&accounts[ledger_index].data.borrow())
        .map_err(map_fractional)?;
    let policy_seeds = policy.pda_seeds();
    expect_pda(
        accounts[policy_index].key,
        seeds::fractional_policy_v2_pda(
            program_id,
            &policy_seeds.market_instance().bytes(),
            &policy_seeds.resolution_account().bytes(),
            &policy_seeds.resolution_data_id().bytes(),
        ),
        Some(policy_seeds.stored_bump()),
    )?;
    let ledger_seeds = ledger.pda_seeds();
    expect_pda(
        accounts[ledger_index].key,
        seeds::fractional_ledger_v1_pda(program_id, &ledger_seeds.policy_account().bytes()),
        Some(ledger_seeds.stored_bump()),
    )?;
    require(
        policy_seeds.resolution_account().bytes() == accounts[resolution_index].key.to_bytes()
            && ledger_seeds.policy_account().bytes() == accounts[policy_index].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok((policy, ledger))
}

/// Decode and execute one admitted FractionalRedemption successor action.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    action: FractionalRedemptionActionV1,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        FractionalRedemptionActionV1::RedeemInternalExact => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_internal_exact(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::RedeemBearerExact => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_bearer_exact(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::SealClaimsExhausted => {
            let intent = FractionalTerminalIntentV1::decode(payload).map_err(map_fractional)?;
            process_seal_claims_exhausted(program_id, accounts, envelope_sequence, intent)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[inline(never)]
fn process_redeem_internal_exact(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require_count(accounts, REDEEM_INTERNAL_EXACT_ACCOUNT_COUNT_V1)?;
    require_distinct(accounts)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(
        !accounts[IX_ACTOR].is_writable && !accounts[IX_ACTOR].executable,
        ClutchError::UnexpectedWritable,
    )?;
    let mut index = 1usize;
    while index < accounts.len() {
        require(!accounts[index].is_signer, ClutchError::MismatchedState)?;
        index += 1;
    }
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_position_replay_sequence != 0
            && intent.expected_credit_sequence == 0
            && intent.credit_mode == 0
            && accounts[IX_ACTOR].key.to_bytes() == intent.claimant.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.claim_source.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.payout_target.bytes()
            && accounts[IX_FRACTIONAL_POLICY].key.to_bytes() == intent.credit_or_policy.bytes(),
        ClutchError::MismatchedState,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        true,
        true,
    )?;
    require(
        intent.outcome < liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let resolution = authenticate_resolution_v5(program_id, &accounts[IX_RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        IX_FRACTIONAL_POLICY,
        IX_FRACTIONAL_LEDGER,
        IX_RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes() == accounts[IX_CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let position = authenticate_general_position_replay_v1(
        program_id,
        liabilities.bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        intent.claimant.bytes(),
        intent.expected_position_replay_sequence,
    )?;
    let context = bind_fractional_internal_context_v1(
        Identity32V1::new(accounts[IX_FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[IX_FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[IX_CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = redeem_internal_exact_v1(
        context,
        intent.expected_ledger_sequence,
        intent.expected_position_replay_sequence,
        InternalPositionV1 {
            position_replay: position.replay,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    require(
        plan.credit_after.is_none()
            && plan.claimant_numerator_after == 0
            && plan.custody_after.payout_atoms() == plan.paid_atoms,
        ClutchError::MismatchedState,
    )?;
    let RedemptionSourcePoststateV1::Internal(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        source_after.position_account.bytes() == accounts[IX_POSITION].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    accounts[IX_FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[IX_HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[IX_CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &source_after
                .position_after
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[IX_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(source_after.replay.replay_poststate_body());
    Ok(())
}

fn require_bearer_account_contract(
    accounts: &[AccountInfo<'_>],
    outcome_count: u8,
    selected_outcome: u8,
) -> Outcome<()> {
    let expected_count = REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1
        .checked_add(usize::from(outcome_count))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require_signer(&accounts[bearer_ix::CLAIMANT])?;
    let selected_mint = bearer_ix::OUTCOME_MINTS + usize::from(selected_outcome);
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(
            index,
            bearer_ix::HOARD
                | bearer_ix::CLAIM_LEDGER
                | bearer_ix::FRACTIONAL_LEDGER
                | bearer_ix::DESTINATION
                | bearer_ix::HOARD_TOKEN
                | bearer_ix::SOURCE
        ) || index == selected_mint;
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            accounts[index].is_signer == (index == bearer_ix::CLAIMANT),
            ClutchError::MismatchedState,
        )?;
        let mut other = index + 1;
        while other < accounts.len() {
            let token_program_alias = (index == bearer_ix::COLLATERAL_TOKEN_PROGRAM
                && other == bearer_ix::OUTCOME_TOKEN_PROGRAM)
                || (index == bearer_ix::OUTCOME_TOKEN_PROGRAM
                    && other == bearer_ix::COLLATERAL_TOKEN_PROGRAM);
            if !token_program_alias {
                require(
                    accounts[index].key != accounts[other].key,
                    ClutchError::AccountAlias,
                )?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

#[inline(never)]
fn process_redeem_bearer_exact(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require(
        accounts.len() >= REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1,
        ClutchError::WrongAccountCount,
    )?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_credit_sequence == 0
            && intent.expected_position_replay_sequence == 0
            && intent.credit_mode == 0
            && accounts[bearer_ix::CLAIMANT].key.to_bytes() == intent.claimant.bytes()
            && accounts[bearer_ix::SOURCE].key.to_bytes() == intent.claim_source.bytes()
            && accounts[bearer_ix::DESTINATION].key.to_bytes() == intent.payout_target.bytes()
            && accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes()
                == intent.credit_or_policy.bytes(),
        ClutchError::MismatchedState,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[bearer_ix::REALM],
        &accounts[bearer_ix::PROFILE],
        &accounts[bearer_ix::COLLATERAL_POLICY],
        &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[bearer_ix::MARKET_BINDING],
        &accounts[bearer_ix::MARKET_RUNTIME],
        &accounts[bearer_ix::MARKET_INSTANCE],
        &accounts[bearer_ix::HOARD],
        &accounts[bearer_ix::CLAIM_LEDGER],
        true,
        true,
    )?;
    require(
        intent.outcome < liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;
    require_bearer_account_contract(
        accounts,
        liabilities.market_binding.outcome_count,
        intent.outcome,
    )?;
    require(
        accounts[bearer_ix::COLLATERAL_MINT].key.to_bytes()
            == liabilities.bound.policy().mint.bytes()
            && accounts[bearer_ix::HOARD_TOKEN].key.to_bytes()
                == liabilities.hoard.token_account.bytes()
            && accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes()
                == liabilities.hoard.authority.bytes()
            && !accounts[bearer_ix::HOARD_AUTHORITY].executable
            && accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let market_bytes = liabilities.market_binding.market_instance_v2_id.bytes();
    expect_pda(
        accounts[bearer_ix::HOARD_AUTHORITY].key,
        seeds::hoard_authority_v2_pda(program_id, &market_bytes),
        None,
    )?;
    expect_pda(
        accounts[bearer_ix::HOARD_TOKEN].key,
        seeds::hoard_token_v2_pda(program_id, &market_bytes),
        None,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[bearer_ix::RESOLUTION], liabilities)?;
    let claim = authenticate_claim_issuance_v1(
        liabilities.bound,
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
    )?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        bearer_ix::FRACTIONAL_POLICY,
        bearer_ix::FRACTIONAL_LEDGER,
        bearer_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == market_bytes
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_before = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.outcome_count,
        intent.outcome,
    )?;
    let selected_mint = &accounts[bearer_ix::OUTCOME_MINTS + usize::from(intent.outcome)];
    let token_before = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let context = bind_fractional_context_v1(
        Identity32V1::new(accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[bearer_ix::FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
        claim,
    )
    .map_err(map_fractional)?;
    let prepared = prepare_bearer_exact_v1(
        context,
        intent.expected_ledger_sequence,
        BearerClaimPrestateV1 {
            claimant: intent.claimant,
            claim_token_account: intent.claim_source,
            claim_mint: Identity32V1::new(selected_mint.key.to_bytes())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            collateral_destination: intent.payout_target,
            claim_issuance_binding: policy.claim_issuance_binding,
            source_claim_atoms: token_before.source_atoms,
            observed_materialized_supply: observed_before.values,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    let prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        claim,
        CollateralId::from_bytes(accounts[bearer_ix::MARKET_RUNTIME].key.to_bytes()),
        CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes()),
        intent.outcome,
        intent.quantity,
        observed_before.values,
        token_before,
        prepared.fractional_claim_ledger(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let burn = prepared_burn.burn_intent();
    require(
        burn.mint == CollateralId::from_bytes(selected_mint.key.to_bytes())
            && burn.source_token_account
                == CollateralId::from_bytes(accounts[bearer_ix::SOURCE].key.to_bytes())
            && burn.claimant
                == CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes())
            && burn.quantity == intent.quantity,
        ClutchError::MismatchedState,
    )?;
    token::burn(
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[bearer_ix::SOURCE],
        selected_mint,
        &accounts[bearer_ix::CLAIMANT],
        intent.quantity,
    )?;
    let observed_after = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.outcome_count,
        intent.outcome,
    )?;
    let token_after = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let accepted_burn =
        accept_fractional_bearer_claim_burn_v3(prepared_burn, observed_after.values, token_after)
            .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    let burned = accept_bearer_exact_burn_v1(prepared, accepted_burn).map_err(map_fractional)?;
    let collateral_request = burned.collateral_request();
    let collateral = {
        let mint_data = accounts[bearer_ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[bearer_ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[bearer_ix::DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        if collateral_request.payout_atoms == 0 {
            let prepared = prepare_zero_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            AcceptedBearerRedemptionCollateralV3::Zero(accept_zero_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
            )?)
        } else {
            let prepared = prepare_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                TransferAuthorityV2 {
                    address: CollateralId::from_bytes(
                        accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes(),
                    ),
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[bearer_ix::HOARD_AUTHORITY].is_writable,
                    executable: accounts[bearer_ix::HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
                },
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            let bump = [seeds::hoard_authority_v2_pda(program_id, &market_bytes).1];
            let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
            AcceptedBearerRedemptionCollateralV3::Nonzero(invoke_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
                &accounts[bearer_ix::HOARD_AUTHORITY],
                &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
                &signer,
            )?)
        }
    };
    let plan = finish_bearer_exact_v1(burned, collateral).map_err(map_fractional)?;
    let RedemptionSourcePoststateV1::Bearer(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        plan.credit_after.is_none()
            && plan.claimant_numerator_after == 0
            && source_after.transition_id.bytes()
                == accepted_burn.fractional().transition_id().bytes()
            && source_after.burn_receipt_id.map(Identity32V1::bytes)
                == Some(accepted_burn.burn_receipt_id().bytes())
            && source_after.claim_token_account.bytes()
                == accounts[bearer_ix::SOURCE].key.to_bytes()
            && source_after.claim_mint.bytes() == selected_mint.key.to_bytes()
            && source_after.collateral_destination.bytes()
                == accounts[bearer_ix::DESTINATION].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    accounts[bearer_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[bearer_ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[bearer_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

#[inline(never)]
fn process_seal_claims_exhausted(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalTerminalIntentV1,
) -> Outcome<()> {
    require_count(accounts, SEAL_CLAIMS_EXHAUSTED_ACCOUNT_COUNT_V1)?;
    require_distinct(accounts)?;
    require(
        envelope_sequence == intent.expected_ledger_sequence,
        ClutchError::Replay,
    )?;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(index, seal_ix::CLAIM_LEDGER | seal_ix::FRACTIONAL_LEDGER);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(!accounts[index].is_signer, ClutchError::MismatchedState)?;
        index += 1;
    }
    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[seal_ix::REALM],
        &accounts[seal_ix::PROFILE],
        &accounts[seal_ix::COLLATERAL_POLICY],
        &accounts[seal_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[seal_ix::MARKET_BINDING],
        &accounts[seal_ix::MARKET_RUNTIME],
        &accounts[seal_ix::MARKET_INSTANCE],
        &accounts[seal_ix::HOARD],
        &accounts[seal_ix::CLAIM_LEDGER],
        false,
        true,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[seal_ix::RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        seal_ix::FRACTIONAL_POLICY,
        seal_ix::FRACTIONAL_LEDGER,
        seal_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[seal_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let context = bind_fractional_internal_context_v1(
        Identity32V1::new(accounts[seal_ix::FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[seal_ix::FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[seal_ix::CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = seal_claims_exhausted_v1(context, intent.expected_ledger_sequence)
        .map_err(map_fractional)?;
    require(
        plan.claim_ledger_after.consumed_sequence() == intent.expected_ledger_sequence,
        ClutchError::MismatchedState,
    )?;
    accounts[seal_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.claim_ledger_after
        .claim_ledger_after()
        .encode(
            &mut accounts[seal_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

const _: () = assert!(POSITION_V3_BYTES == 480);
