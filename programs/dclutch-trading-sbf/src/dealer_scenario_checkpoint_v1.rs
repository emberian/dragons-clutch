//! Lock-bounded Trading checkpoint lifecycle for Dealer scenario evaluation.
//!
//! This module executes the durable mechanics which do not need the original
//! 121-lock selector-9 frame: a Dealer-authorized PDA creation, six ordered
//! readonly transcript pages, producer-bound evaluation sealing, and
//! permissionless post-expiry cleanup. It intentionally does not execute the
//! final Claims/Custody liability mutation.
//!
//! The evaluation receipt's owner and PDA authenticate one producer for that
//! receipt. The caller which invokes this route must additionally authenticate
//! that producer through the release-selected admitted-accelerator artifacts;
//! until that common release-authenticated caller lands, this module is an
//! executable checkpoint primitive rather than completed Dealer acceptance.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_dealer_codec::{
    scenario_checkpoint_v1::{
        DEALER_SCENARIO_CHECKPOINT_BYTES_V1, DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
        DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1, DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1,
        DEALER_SCENARIO_PAGE_RECEIPT_DOMAIN_V1, DEALER_SCENARIO_PREPARATION_PAGES_V1,
        DealerScenarioCheckpointInputV1, DealerScenarioCheckpointV1, DealerScenarioEvaluationV1,
    },
    scenario_evaluation_receipt_v1::{
        DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1, DealerScenarioEvaluationReceiptV1,
    },
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{SysvarSerialize, clock::Clock},
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{TradingSbfError, dealer::v3_trade::DealerScenarioTradeRequestV3};

/// Create one request-scoped checkpoint PDA.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1: [u8; 8] = *b"DCLTDCP1";
/// Append the next canonical readonly transcript page.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1: [u8; 8] = *b"DCLTDPG1";
/// Seal one producer-bound evaluation after all pages exist.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1: [u8; 8] = *b"DCLTDEV1";
/// Close an expired checkpoint to its immutable beneficiary.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1: [u8; 8] = *b"DCLTDCL1";

/// Exact create instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact page instruction width: magic followed by the canonical page ordinal.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_INSTRUCTION_BYTES_V1: usize = 9;
/// Exact evaluate instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact cleanup instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_INSTRUCTION_BYTES_V1: usize = 8;

/// Exact create account count.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1: usize = 10;
/// Fixed account prefix before one page's readonly observations.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1: usize = 2;
/// Maximum readonly observations carried by one page transaction.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1: usize = 48;
/// Exact evaluation-seal account count.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1: usize = 8;
/// Exact permissionless-cleanup account count.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1: usize = 3;

const CREATE_PAYER: usize = 0;
const CREATE_DEALER_AUTHORITY: usize = 1;
const CREATE_REFUND_BENEFICIARY: usize = 2;
const CREATE_CHECKPOINT: usize = 3;
const CREATE_REQUEST: usize = 4;
const CREATE_ROOT: usize = 5;
const CREATE_OBLIGATION: usize = 6;
const CREATE_CLOCK: usize = 7;
const CREATE_RENT: usize = 8;
const CREATE_SYSTEM: usize = 9;

const PAGE_CHECKPOINT: usize = 0;
const PAGE_CLOCK: usize = 1;

const EVALUATE_CHECKPOINT: usize = 0;
const EVALUATE_CLOCK: usize = 1;
const EVALUATE_PRODUCER: usize = 2;
const EVALUATE_RECEIPT: usize = 3;
const EVALUATE_CANDIDATE_BANK: usize = 4;
const EVALUATE_CANDIDATE_OBLIGATION: usize = 5;
const EVALUATE_CLAIMS_DELTA: usize = 6;
const EVALUATE_EFFECTS: usize = 7;

const CLEANUP_CHECKPOINT: usize = 0;
const CLEANUP_BENEFICIARY: usize = 1;
const CLEANUP_CLOCK: usize = 2;

/// Return whether bytes select checkpoint creation.
#[must_use]
pub fn is_dealer_scenario_checkpoint_create_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1
}

/// Return whether bytes select one ordered page append.
#[must_use]
pub fn is_dealer_scenario_checkpoint_page_v1(data: &[u8]) -> bool {
    data.len() == DEALER_SCENARIO_CHECKPOINT_PAGE_INSTRUCTION_BYTES_V1
        && data.get(..8) == Some(DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1.as_slice())
}

/// Return whether bytes select evaluation sealing.
#[must_use]
pub fn is_dealer_scenario_checkpoint_evaluate_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1
}

/// Return whether bytes select permissionless expiry cleanup.
#[must_use]
pub fn is_dealer_scenario_checkpoint_cleanup_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1
}

/// Create one Dealer-authorized request-scoped checkpoint PDA.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_create_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_create_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let payer = account(accounts, CREATE_PAYER)?;
    let dealer = account(accounts, CREATE_DEALER_AUTHORITY)?;
    let beneficiary = account(accounts, CREATE_REFUND_BENEFICIARY)?;
    let checkpoint_account = account(accounts, CREATE_CHECKPOINT)?;
    let request_account = account(accounts, CREATE_REQUEST)?;
    let root = account(accounts, CREATE_ROOT)?;
    let obligation = account(accounts, CREATE_OBLIGATION)?;
    let clock_account = account(accounts, CREATE_CLOCK)?;
    let rent_account = account(accounts, CREATE_RENT)?;
    let system = account(accounts, CREATE_SYSTEM)?;
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || !dealer.is_signer
        || dealer.is_writable
        || dealer.executable
        || beneficiary.is_signer
        || beneficiary.is_writable
        || beneficiary.executable
        || !checkpoint_account.is_writable
        || checkpoint_account.is_signer
        || checkpoint_account.executable
        || checkpoint_account.owner != &system_program::ID
        || checkpoint_account.data_len() != 0
        || request_account.is_signer
        || request_account.is_writable
        || request_account.executable
        || root.is_signer
        || root.is_writable
        || root.executable
        || obligation.is_signer
        || obligation.is_writable
        || obligation.executable
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_data = request_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    if dealer.key.to_bytes() != request.dealer_owner
        || root.key.to_bytes() != request.child_root
        || root.owner != program_id
        || obligation.key.to_bytes() != request.obligation
        || obligation.owner != program_id
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_digest = hash(&request_data).to_bytes();
    let (expected_checkpoint, bump) = Pubkey::find_program_address(
        &[DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, &request_digest],
        program_id,
    );
    if checkpoint_account.key != &expected_checkpoint {
        return Err(TradingSbfError::Content.into());
    }
    let root_data = root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let obligation_data = obligation
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let obligation_digest = hash(&obligation_data).to_bytes();
    if obligation_digest != request.current_obligation_digest {
        return Err(TradingSbfError::Content.into());
    }
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let rent = Rent::from_account_info(rent_account).map_err(|_| TradingSbfError::Content)?;
    let checkpoint = DealerScenarioCheckpointV1::new(DealerScenarioCheckpointInputV1 {
        release_set: request.release_set,
        market: request.market,
        child_root: request.child_root,
        obligation: request.obligation,
        refund_beneficiary: beneficiary.key.to_bytes(),
        request_digest,
        root_prestate_digest: hash(&root_data).to_bytes(),
        claims_prestate_digest: [0; 32],
        obligation_prestate_digest: obligation_digest,
        custody_prestate_digest: [0; 32],
        generation: request.generation,
        created_slot: clock.slot,
        expires_at: request.expires_at,
    })
    .map_err(|_| TradingSbfError::Content)?;
    drop(obligation_data);
    drop(root_data);
    drop(request_data);

    let minimum = rent.minimum_balance(DEALER_SCENARIO_CHECKPOINT_BYTES_V1);
    let deficit = minimum.saturating_sub(checkpoint_account.lamports());
    if deficit > 0 {
        invoke(
            &transfer(payer.key, checkpoint_account.key, deficit),
            &[payer.clone(), checkpoint_account.clone(), system.clone()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
    }
    let bump_seed = [bump];
    let signer = [
        DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
        request_digest.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            checkpoint_account.key,
            u64::try_from(DEALER_SCENARIO_CHECKPOINT_BYTES_V1)
                .map_err(|_| TradingSbfError::Commit)?,
        ),
        &[checkpoint_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    invoke_signed(
        &assign(checkpoint_account.key, program_id),
        &[checkpoint_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    write_new_checkpoint(program_id, checkpoint_account, checkpoint)?;
    set_return_data(&request_digest);
    Ok(())
}

/// Append one canonical transcript page, deriving its receipt from observations.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_page_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_page_v1(instruction_data)
        || accounts.len() <= DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
        || accounts.len()
            > DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
                + DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let page_index = *instruction_data
        .get(8)
        .ok_or(TradingSbfError::UnsupportedContent)?;
    let checkpoint_account = account(accounts, PAGE_CHECKPOINT)?;
    let clock_account = account(accounts, PAGE_CLOCK)?;
    let observations = accounts
        .get(DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1..)
        .ok_or(TradingSbfError::Content)?;
    let (checkpoint, prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    if observations.iter().any(|current| {
        current.is_signer
            || current.is_writable
            || current.key == checkpoint_account.key
            || current.key == clock_account.key
    }) || has_duplicate_keys(observations)
    {
        return Err(TradingSbfError::Content.into());
    }
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let receipt_digest = page_receipt_digest(
        checkpoint_account.key,
        checkpoint,
        prestate_digest,
        page_index,
        observations,
    )?;
    let next = checkpoint
        .append_page(clock.slot, page_index, prestate_digest, receipt_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    write_checkpoint_last(program_id, checkpoint_account, checkpoint, next)?;
    set_return_data(&receipt_digest);
    Ok(())
}

/// Seal one producer-owned evaluation receipt after all six pages exist.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_evaluate_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_evaluate_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, EVALUATE_CHECKPOINT)?;
    let clock_account = account(accounts, EVALUATE_CLOCK)?;
    let producer = account(accounts, EVALUATE_PRODUCER)?;
    let receipt_account = account(accounts, EVALUATE_RECEIPT)?;
    let candidate_bank = account(accounts, EVALUATE_CANDIDATE_BANK)?;
    let candidate_obligation = account(accounts, EVALUATE_CANDIDATE_OBLIGATION)?;
    let claims_delta = account(accounts, EVALUATE_CLAIMS_DELTA)?;
    let effects = account(accounts, EVALUATE_EFFECTS)?;
    if !producer.executable
        || producer.is_signer
        || producer.is_writable
        || [
            receipt_account,
            candidate_bank,
            candidate_obligation,
            claims_delta,
            effects,
        ]
        .iter()
        .any(|current| {
            current.is_signer
                || current.is_writable
                || current.executable
                || current.owner != producer.key
        })
    {
        return Err(TradingSbfError::Content.into());
    }
    let (checkpoint, prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    let input = checkpoint.input();
    let expected_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
            &input.request_digest,
        ],
        producer.key,
    )
    .0;
    if receipt_account.key != &expected_receipt || has_duplicate_keys(accounts) {
        return Err(TradingSbfError::Content.into());
    }
    let claims_prestate_digest =
        joined_page_digest(DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1, checkpoint, 0, 3)?;
    let custody_prestate_digest =
        joined_page_digest(DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1, checkpoint, 3, 6)?;
    let receipt_data = receipt_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let receipt = DealerScenarioEvaluationReceiptV1::decode(&receipt_data)
        .map_err(|_| TradingSbfError::Content)?;
    let candidate_bank_digest = account_data_digest(candidate_bank)?;
    let candidate_obligation_digest = account_data_digest(candidate_obligation)?;
    let claims_delta_digest = account_data_digest(claims_delta)?;
    let effects_digest = account_data_digest(effects)?;
    if receipt.producer_program != producer.key.to_bytes()
        || receipt.checkpoint != checkpoint_account.key.to_bytes()
        || receipt.checkpoint_prestate_digest != prestate_digest
        || receipt.request_digest != input.request_digest
        || receipt.claims_prestate_digest != claims_prestate_digest
        || receipt.custody_prestate_digest != custody_prestate_digest
        || receipt.candidate_bank_digest != candidate_bank_digest
        || receipt.candidate_obligation_digest != candidate_obligation_digest
        || receipt.claims_delta_digest != claims_delta_digest
        || receipt.effects_digest != effects_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    let evaluation_receipt_digest = hash(&receipt_data).to_bytes();
    drop(receipt_data);
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let next = checkpoint
        .finish_evaluation(
            clock.slot,
            prestate_digest,
            claims_prestate_digest,
            custody_prestate_digest,
            DealerScenarioEvaluationV1 {
                evaluation_receipt_digest,
                candidate_bank_digest,
                candidate_obligation_digest,
                claims_delta_digest,
                effects_digest,
            },
        )
        .map_err(|_| TradingSbfError::Transition)?;
    write_checkpoint_last(program_id, checkpoint_account, checkpoint, next)?;
    set_return_data(&evaluation_receipt_digest);
    Ok(())
}

/// Close an expired checkpoint to its immutable refund beneficiary.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_cleanup_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_cleanup_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, CLEANUP_CHECKPOINT)?;
    let beneficiary = account(accounts, CLEANUP_BENEFICIARY)?;
    let clock_account = account(accounts, CLEANUP_CLOCK)?;
    let (checkpoint, _) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let expected = checkpoint
        .cleanup_beneficiary(clock.slot)
        .map_err(|_| TradingSbfError::Transition)?;
    if beneficiary.key.to_bytes() != expected
        || !checkpoint_account.is_writable
        || !beneficiary.is_writable
        || beneficiary.is_signer
        || beneficiary.executable
        || beneficiary.key == checkpoint_account.key
    {
        return Err(TradingSbfError::Commit.into());
    }
    let amount = checkpoint_account.lamports();
    let beneficiary_post = beneficiary
        .lamports()
        .checked_add(amount)
        .ok_or(TradingSbfError::Commit)?;
    **beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = beneficiary_post;
    **checkpoint_account
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = 0;
    checkpoint_account
        .resize(0)
        .map_err(|_| TradingSbfError::Commit)?;
    checkpoint_account.assign(&system_program::ID);
    Ok(())
}

fn read_checkpoint(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<(DealerScenarioCheckpointV1, [u8; 32]), ProgramError> {
    if account.owner != program_id
        || !account.is_writable
        || account.is_signer
        || account.executable
        || account.data_len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let digest = hash(&data).to_bytes();
    let checkpoint =
        DealerScenarioCheckpointV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((checkpoint, digest))
}

fn require_checkpoint_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    checkpoint: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    let expected = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
            &checkpoint.input().request_digest,
        ],
        program_id,
    )
    .0;
    if account.key == &expected {
        Ok(())
    } else {
        Err(TradingSbfError::Content.into())
    }
}

fn write_new_checkpoint(
    program_id: &Pubkey,
    target: &AccountInfo<'_>,
    checkpoint: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    let bytes = checkpoint.to_bytes().map_err(|_| TradingSbfError::Commit)?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if target.owner != program_id || data.len() != bytes.len() || data.iter().any(|byte| *byte != 0)
    {
        return Err(TradingSbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

fn write_checkpoint_last(
    program_id: &Pubkey,
    target: &AccountInfo<'_>,
    expected: DealerScenarioCheckpointV1,
    next: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if target.owner != program_id
        || data.len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
        || DealerScenarioCheckpointV1::decode(&data).map_err(|_| TradingSbfError::Commit)?
            != expected
    {
        return Err(TradingSbfError::Commit.into());
    }
    let next_bytes = next.to_bytes().map_err(|_| TradingSbfError::Commit)?;
    data.copy_from_slice(&next_bytes);
    Ok(())
}

fn page_receipt_digest(
    checkpoint_key: &Pubkey,
    checkpoint: DealerScenarioCheckpointV1,
    checkpoint_prestate_digest: [u8; 32],
    page_index: u8,
    observations: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    let mut digests = Vec::with_capacity(observations.len());
    for current in observations {
        let data = current
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        let lamports = current.lamports().to_le_bytes();
        let data_len = u64::try_from(data.len())
            .map_err(|_| TradingSbfError::Content)?
            .to_le_bytes();
        let executable = [u8::from(current.executable)];
        digests.push(
            hashv(&[
                current.key.as_ref(),
                current.owner.as_ref(),
                &lamports,
                &data_len,
                &executable,
                &data,
            ])
            .to_bytes(),
        );
    }
    let page = [page_index];
    let input = checkpoint.input();
    let mut parts = vec![
        DEALER_SCENARIO_PAGE_RECEIPT_DOMAIN_V1,
        checkpoint_key.as_ref(),
        checkpoint_prestate_digest.as_slice(),
        page.as_slice(),
        input.request_digest.as_slice(),
    ];
    parts.extend(digests.iter().map(<[u8; 32]>::as_slice));
    Ok(hashv(&parts).to_bytes())
}

fn joined_page_digest(
    domain: &[u8],
    checkpoint: DealerScenarioCheckpointV1,
    start: u8,
    end: u8,
) -> Result<[u8; 32], ProgramError> {
    let mut digests = Vec::new();
    for page in start..end {
        digests.push(
            checkpoint
                .page_receipt_digest(page)
                .map_err(|_| TradingSbfError::Transition)?,
        );
    }
    let input = checkpoint.input();
    let mut parts = vec![domain, input.request_digest.as_slice()];
    parts.extend(digests.iter().map(<[u8; 32]>::as_slice));
    Ok(hashv(&parts).to_bytes())
}

fn account_data_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.is_empty() {
        return Err(TradingSbfError::Content.into());
    }
    Ok(hash(&data).to_bytes())
}

fn has_duplicate_keys(accounts: &[AccountInfo<'_>]) -> bool {
    accounts.iter().enumerate().any(|(index, current)| {
        accounts
            .get(index.saturating_add(1)..)
            .unwrap_or(&[])
            .iter()
            .any(|other| current.key == other.key)
    })
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

const _: () = assert!(DEALER_SCENARIO_PREPARATION_PAGES_V1 == 6);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1 + 1 < 64);
const _: () = assert!(
    DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
        + DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1
        + 1
        < 64
);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1 + 1 < 64);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1 + 1 < 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_exact_and_page_ordinal_is_not_a_selector() {
        assert!(is_dealer_scenario_checkpoint_create_v1(b"DCLTDCP1"));
        assert!(is_dealer_scenario_checkpoint_page_v1(b"DCLTDPG1\x05"));
        assert!(is_dealer_scenario_checkpoint_evaluate_v1(b"DCLTDEV1"));
        assert!(is_dealer_scenario_checkpoint_cleanup_v1(b"DCLTDCL1"));
        assert!(!is_dealer_scenario_checkpoint_page_v1(b"DCLTDPG1"));
        assert!(!is_dealer_scenario_checkpoint_create_v1(b"DCLTDCP1\x00"));
    }
}
