//! Capability-disabled SBF commit seam for General action 38.
//!
//! The pure General composer must first rederive the presence-explicit V2
//! owner realization, exact payer-allocation prestate data ID, and paired
//! Position/Replay successors. This module authenticates the fixed mutation
//! set and verifies the exact postimage. It deliberately exports no dispatcher
//! entry and performs no write: the authoritative owner-fee rent-ledger codec
//! and exhaustive signed-envelope loader have not landed, so no live handler
//! can honestly mint the pure plan yet.

use core::cell::Ref;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    payer_allocation_account_data_id_v1, OwnerFeeAction38PlanV2, Sha256BackendV1,
    SettlementCashPotV1AccountV1, OWNER_FEE_CARRY_ACCOUNT_BYTES,
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES, OWNER_SETTLEMENT_ACCOUNT_BYTES,
    PAYER_ALLOCATION_ACCOUNT_BYTES, SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
};
use clutch_owner_settlement::OwnerFinalizedRowDataHashV2;
use clutch_retirement::{
    PositionV3Sha256Backend, ReplayV3HashBackend, POSITION_V3_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;

/// Fixed mutation accounts. Selected-policy and signed-envelope evidence are
/// authenticated before this postimage seam and are intentionally not hidden
/// in an untyped remaining-account tail here.
pub const ACTION38_MUTATION_ACCOUNT_COUNT_V2: usize = 9;
/// Writable presence-explicit owner row.
pub const IX_OWNER_SETTLEMENT: usize = 0;
/// Writable canonical Position V3.
pub const IX_POSITION: usize = 1;
/// Writable purpose-owned Replay V3.
pub const IX_REPLAY: usize = 2;
/// Writable candidate settlement cash pot.
pub const IX_SETTLEMENT_CASH_POT: usize = 3;
/// Writable carry PDA reallocated in place.
pub const IX_OWNER_FEE_CARRY: usize = 4;
/// Writable temporary payer allocation closed atomically.
pub const IX_PAYER_ALLOCATION: usize = 5;
/// Writable signer funding only the exact carry rent delta.
pub const IX_CARRY_TOP_UP_PAYER: usize = 6;
/// Writable authenticated payer principal refund recipient.
pub const IX_PAYER_RENT_REFUND: usize = 7;
/// Writable canonical hostile-prefunding sink.
pub const IX_NEUTRAL_SINK: usize = 8;

const OWNER_SETTLEMENT_STORED_BUMP_OFFSET: usize = OWNER_SETTLEMENT_ACCOUNT_BYTES - 2;
const OWNER_FEE_CARRY_STORED_BUMP_OFFSET: usize = OWNER_FEE_CARRY_ACCOUNT_BYTES - 2;
const PAYER_ALLOCATION_STORED_BUMP_OFFSET: usize = PAYER_ALLOCATION_ACCOUNT_BYTES - 2;
const SETTLEMENT_CASH_POT_STORED_BUMP_OFFSET: usize = SETTLEMENT_CASH_POT_ACCOUNT_BYTES - 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl OwnerFinalizedRowDataHashV2 for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

/// Lamport observations retained across a future atomic write implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeAction38PreBalancesV2 {
    carry_top_up_payer: u64,
    payer_rent_refund: u64,
    neutral_sink: u64,
    owner_settlement_bump: u8,
    settlement_cash_pot_bump: u8,
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_writable_endpoint(account: &AccountInfo<'_>, signer: bool) -> Outcome<()> {
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    if signer {
        require(account.is_signer, ClutchError::MissingSignature)?;
    }
    Ok(())
}

fn key(account: &AccountInfo<'_>) -> [u8; 32] {
    account.key.to_bytes()
}

fn expect_header(data: &[u8], tag: u8, version: u8) -> Outcome<()> {
    require(data.len() >= 2, ClutchError::WrongDataLength)?;
    require(data[0] == tag, ClutchError::MismatchedState)?;
    require(data[1] == version, ClutchError::MismatchedState)
}

/// Authenticate the exact fixed prestate named by a privately constructed
/// pure action-38 plan. This does not authenticate the still-missing rent
/// ledger or signed-envelope family and therefore is not execution authority.
pub fn authenticate_owner_fee_action38_prestate_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    plan: &OwnerFeeAction38PlanV2,
) -> Outcome<OwnerFeeAction38PreBalancesV2> {
    require_count(accounts, ACTION38_MUTATION_ACCOUNT_COUNT_V2)?;
    for (index, len) in [
        (IX_OWNER_SETTLEMENT, OWNER_SETTLEMENT_ACCOUNT_BYTES),
        (IX_POSITION, POSITION_V3_BYTES),
        (IX_REPLAY, contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES),
        (IX_SETTLEMENT_CASH_POT, SETTLEMENT_CASH_POT_ACCOUNT_BYTES),
        (IX_OWNER_FEE_CARRY, OWNER_FEE_CARRY_ACCOUNT_BYTES),
        (IX_PAYER_ALLOCATION, PAYER_ALLOCATION_ACCOUNT_BYTES),
    ] {
        require_program_state(program_id, &accounts[index], len)?;
    }
    require_writable_endpoint(&accounts[IX_CARRY_TOP_UP_PAYER], true)?;
    require_writable_endpoint(&accounts[IX_PAYER_RENT_REFUND], false)?;
    require_writable_endpoint(&accounts[IX_NEUTRAL_SINK], false)?;

    let realization = plan.realization();
    let expectation = realization.expectation();
    let position = realization.position();
    let position_fields = position.semantic.fields();
    let replay = plan.replay();
    let finalization = plan.finalization();
    let top_up = plan.carry_top_up();
    let refund = plan.payer_rent_refund();
    let donation = plan.payer_donation_credit();
    require(key(&accounts[IX_OWNER_SETTLEMENT]) == realization.owner_settlement_account(), ClutchError::MismatchedState)?;
    require(key(&accounts[IX_POSITION]) == position.account, ClutchError::MismatchedState)?;
    require(key(&accounts[IX_REPLAY]) == replay.replay_account().bytes(), ClutchError::MismatchedState)?;
    require(
        key(&accounts[IX_SETTLEMENT_CASH_POT]) == finalization.semantic.settlement_cash_pot().0,
        ClutchError::MismatchedState,
    )?;
    require(key(&accounts[IX_OWNER_FEE_CARRY]) == plan.carry_account().bytes(), ClutchError::MismatchedState)?;
    require(key(&accounts[IX_PAYER_ALLOCATION]) == plan.payer_allocation_account().bytes(), ClutchError::MismatchedState)?;
    require(key(&accounts[IX_CARRY_TOP_UP_PAYER]) == top_up.source.bytes(), ClutchError::MismatchedState)?;
    require(key(&accounts[IX_PAYER_RENT_REFUND]) == refund.destination.bytes(), ClutchError::MismatchedState)?;
    require(key(&accounts[IX_NEUTRAL_SINK]) == donation.destination.bytes(), ClutchError::MismatchedState)?;

    let owner_settlement_data = borrow_data(&accounts[IX_OWNER_SETTLEMENT])?;
    let owner_settlement_bump = owner_settlement_data[OWNER_SETTLEMENT_STORED_BUMP_OFFSET];
    expect_header(
        &owner_settlement_data,
        contract::OWNER_SETTLEMENT_ACCOUNT_TAG,
        contract::OWNER_SETTLEMENT_ACCOUNT_VERSION,
    )?;
    expect_pda(
        accounts[IX_OWNER_SETTLEMENT].key,
        seeds::general_v2_owner_settlement_v2_pda(
            program_id,
            &expectation.epoch,
            &expectation.candidate,
            &expectation.owner,
        ),
        Some(owner_settlement_bump),
    )?;
    drop(owner_settlement_data);

    let carry_data = borrow_data(&accounts[IX_OWNER_FEE_CARRY])?;
    expect_header(
        &carry_data,
        contract::OWNER_FEE_CARRY_ACCOUNT_TAG,
        contract::OWNER_FEE_CARRY_ACCOUNT_VERSION,
    )?;
    let carry_bump = carry_data[OWNER_FEE_CARRY_STORED_BUMP_OFFSET];
    require(carry_bump == finalization.stored_bump, ClutchError::WrongBump)?;
    expect_pda(
        accounts[IX_OWNER_FEE_CARRY].key,
        seeds::general_v2_owner_fee_carry_pda(
            program_id,
            &finalization.semantic.fee_record().0,
            &expectation.owner,
        ),
        Some(carry_bump),
    )?;
    drop(carry_data);

    let payer_data = borrow_data(&accounts[IX_PAYER_ALLOCATION])?;
    expect_header(
        &payer_data,
        contract::PAYER_ALLOCATION_ACCOUNT_TAG,
        contract::PAYER_ALLOCATION_ACCOUNT_VERSION,
    )?;
    let payer_bump = payer_data[PAYER_ALLOCATION_STORED_BUMP_OFFSET];
    expect_pda(
        accounts[IX_PAYER_ALLOCATION].key,
        seeds::general_v2_payer_allocation_pda(
            program_id,
            &finalization.semantic.fee_record().0,
            &expectation.owner,
        ),
        Some(payer_bump),
    )?;
    let payer_data_id = payer_allocation_account_data_id_v1(&payer_data, &RuntimeSha256)?;
    require(
        payer_data_id == plan.payer_allocation_data_id(),
        ClutchError::MismatchedState,
    )?;
    drop(payer_data);

    let pot_data = borrow_data(&accounts[IX_SETTLEMENT_CASH_POT])?;
    expect_header(
        &pot_data,
        contract::SETTLEMENT_CASH_POT_ACCOUNT_TAG,
        contract::SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
    )?;
    let settlement_cash_pot_bump = pot_data[SETTLEMENT_CASH_POT_STORED_BUMP_OFFSET];
    expect_pda(
        accounts[IX_SETTLEMENT_CASH_POT].key,
        seeds::general_v2_settlement_cash_pot_pda(
            program_id,
            &expectation.epoch,
            &expectation.candidate,
        ),
        Some(settlement_cash_pot_bump),
    )?;
    drop(pot_data);

    let position_seeds = position.semantic.pda_seeds();
    expect_pda(
        accounts[IX_POSITION].key,
        seeds::position_v3_pda(
            program_id,
            &position_seeds.market_instance_id().bytes(),
            &position_seeds.owner().bytes(),
            position_seeds.purpose(),
            &position_seeds.purpose_binding_id().bytes(),
        ),
        Some(position_seeds.stored_bump()),
    )?;
    expect_pda(
        accounts[IX_REPLAY].key,
        seeds::purpose_replay_v3_pda(
            program_id,
            &position.account,
            position_fields.purpose,
            &position_fields.purpose_binding_id.bytes(),
        ),
        Some(replay.replay_poststate_body()[4]),
    )?;

    let carry_before = plan
        .carry_balance_after_lamports()
        .checked_sub(top_up.lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(accounts[IX_OWNER_FEE_CARRY].lamports() == carry_before, ClutchError::MismatchedState)?;
    require(
        accounts[IX_PAYER_ALLOCATION].lamports() == plan.payer_balance_before_lamports(),
        ClutchError::MismatchedState,
    )?;
    Ok(OwnerFeeAction38PreBalancesV2 {
        carry_top_up_payer: accounts[IX_CARRY_TOP_UP_PAYER].lamports(),
        payer_rent_refund: accounts[IX_PAYER_RENT_REFUND].lamports(),
        neutral_sink: accounts[IX_NEUTRAL_SINK].lamports(),
        owner_settlement_bump,
        settlement_cash_pot_bump,
    })
}

fn expected_endpoint_balance(
    account: [u8; 32],
    before: u64,
    plan: &OwnerFeeAction38PlanV2,
) -> Outcome<u64> {
    let mut after = before;
    for transfer in [
        plan.carry_top_up(),
        plan.payer_rent_refund(),
        plan.payer_donation_credit(),
    ] {
        if account == transfer.source.bytes() {
            after = after.checked_sub(transfer.lamports).ok_or(ClutchError::Arithmetic)?;
        }
        if account == transfer.destination.bytes() {
            after = after.checked_add(transfer.lamports).ok_or(ClutchError::Arithmetic)?;
        }
    }
    Ok(after)
}

fn require_exact_data(account: &AccountInfo<'_>, expected: &[u8]) -> Outcome<()> {
    let data = borrow_data(account)?;
    require(&*data == expected, ClutchError::MismatchedState)
}

/// Verify every byte, balance, close, and rent/surplus delta after a future
/// atomic writer applies the private pure plan. Any mismatch makes the whole
/// Solana instruction roll back.
pub fn verify_owner_fee_action38_poststate_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    before: OwnerFeeAction38PreBalancesV2,
    plan: &OwnerFeeAction38PlanV2,
) -> Outcome<()> {
    require_count(accounts, ACTION38_MUTATION_ACCOUNT_COUNT_V2)?;
    let realization = plan.realization();
    let mut owner_row = [0u8; OWNER_SETTLEMENT_ACCOUNT_BYTES];
    owner_row[0] = contract::OWNER_SETTLEMENT_ACCOUNT_TAG;
    owner_row[1] = contract::OWNER_SETTLEMENT_ACCOUNT_VERSION;
    owner_row[2..2 + clutch_owner_settlement::OWNER_SETTLEMENT_BODY_V2_BYTES]
        .copy_from_slice(realization.owner_settlement_body());
    owner_row[OWNER_SETTLEMENT_STORED_BUMP_OFFSET] = before.owner_settlement_bump;
    require_exact_data(&accounts[IX_OWNER_SETTLEMENT], &owner_row)?;

    let position = realization
        .position()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_exact_data(&accounts[IX_POSITION], &position)?;
    require_exact_data(&accounts[IX_REPLAY], plan.replay().replay_poststate_body())?;

    let mut pot = [0u8; SETTLEMENT_CASH_POT_ACCOUNT_BYTES];
    SettlementCashPotV1AccountV1 {
        semantic: realization.settlement_cash_pot(),
        stored_bump: before.settlement_cash_pot_bump,
        flags: 0,
    }
    .encode(&mut pot)?;
    require_exact_data(&accounts[IX_SETTLEMENT_CASH_POT], &pot)?;

    let mut finalization = [0u8; OWNER_FEE_FINALIZATION_ACCOUNT_BYTES];
    plan.finalization().encode(&mut finalization)?;
    require_exact_data(&accounts[IX_OWNER_FEE_CARRY], &finalization)?;
    require(
        accounts[IX_OWNER_FEE_CARRY].owner == program_id
            && accounts[IX_OWNER_FEE_CARRY].lamports()
                == plan.carry_balance_after_lamports(),
        ClutchError::MismatchedState,
    )?;
    require(
        accounts[IX_PAYER_ALLOCATION].data_len() == 0
            && accounts[IX_PAYER_ALLOCATION].lamports() == 0,
        ClutchError::MismatchedState,
    )?;

    for (index, balance_before) in [
        (IX_CARRY_TOP_UP_PAYER, before.carry_top_up_payer),
        (IX_PAYER_RENT_REFUND, before.payer_rent_refund),
        (IX_NEUTRAL_SINK, before.neutral_sink),
    ] {
        let expected = expected_endpoint_balance(key(&accounts[index]), balance_before, plan)?;
        require(accounts[index].lamports() == expected, ClutchError::MismatchedState)?;
    }
    Ok(())
}
