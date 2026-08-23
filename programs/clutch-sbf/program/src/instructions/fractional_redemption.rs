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
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_fractional_redemption_runtime::{
    bind_fractional_internal_context_v1, redeem_internal_exact_v1, Error as FractionalError,
    FractionalLedgerV1, FractionalPolicyV2, FractionalRedeemIntentV1, FractionalRedemptionActionV1,
    InternalPositionV1, RedemptionSourcePoststateV1, FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    FRACTIONAL_POLICY_ACCOUNT_BYTES,
};
use clutch_retirement::{Identity32V1, POSITION_V3_BYTES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    authenticate_resolution_v5,
};

/// Exact account count for action 2.
pub const REDEEM_INTERNAL_EXACT_ACCOUNT_COUNT_V1: usize = 15;

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
) -> Outcome<(FractionalPolicyV2, FractionalLedgerV1)> {
    require_program_state(
        program_id,
        &accounts[IX_FRACTIONAL_POLICY],
        false,
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        &accounts[IX_FRACTIONAL_LEDGER],
        true,
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    )?;
    let policy = FractionalPolicyV2::decode(&accounts[IX_FRACTIONAL_POLICY].data.borrow())
        .map_err(map_fractional)?;
    let ledger = FractionalLedgerV1::decode(&accounts[IX_FRACTIONAL_LEDGER].data.borrow())
        .map_err(map_fractional)?;
    let policy_seeds = policy.pda_seeds();
    expect_pda(
        accounts[IX_FRACTIONAL_POLICY].key,
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
        accounts[IX_FRACTIONAL_LEDGER].key,
        seeds::fractional_ledger_v1_pda(program_id, &ledger_seeds.policy_account().bytes()),
        Some(ledger_seeds.stored_bump()),
    )?;
    require(
        policy_seeds.resolution_account().bytes() == accounts[IX_RESOLUTION].key.to_bytes()
            && ledger_seeds.policy_account().bytes()
                == accounts[IX_FRACTIONAL_POLICY].key.to_bytes(),
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
    let (policy, ledger) = decode_fractional_accounts(program_id, accounts)?;
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

const _: () = assert!(POSITION_V3_BYTES == 480);
