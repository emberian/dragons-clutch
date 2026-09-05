#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Minimal real-SBF legacy-token adversary for one `TransferChecked`: it
//! applies the exact requested balances/delegate terminal, then installs a
//! destination close authority so Trading's complete-byte post-CPI join must
//! roll back a field Custody's amount-level receipt deliberately does not own.

extern crate std;

use dclutch_custody::token_svm::state::TokenAccountLayoutV1;
use dclutch_custody::token_svm::{ACCOUNT_BYTES, COption, Mint, TokenAccount};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

const TRANSFER_CHECKED_TAG: u8 = 12;
const TRANSFER_CHECKED_BYTES: usize = 10;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != TRANSFER_CHECKED_BYTES
        || instruction_data.first().copied() != Some(TRANSFER_CHECKED_TAG)
        || accounts.len() != 4
    {
        return Err(ProgramError::InvalidInstructionData);
    }
    let source = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    let mint = accounts.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?;
    let destination = accounts.get(2).ok_or(ProgramError::NotEnoughAccountKeys)?;
    let authority = accounts.get(3).ok_or(ProgramError::NotEnoughAccountKeys)?;
    if !source.is_writable
        || !destination.is_writable
        || !authority.is_signer
        || source.owner != program_id
        || destination.owner != program_id
        || mint.owner != program_id
    {
        return Err(ProgramError::InvalidAccountData);
    }
    let amount = u64::from_le_bytes(
        instruction_data
            .get(1..9)
            .ok_or(ProgramError::InvalidInstructionData)?
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );
    let decimals = *instruction_data
        .get(9)
        .ok_or(ProgramError::InvalidInstructionData)?;
    let mint_state = Mint::parse(
        &mint
            .try_borrow_data()
            .map_err(|_| ProgramError::AccountBorrowFailed)?,
    )
    .map_err(|_| ProgramError::InvalidAccountData)?;
    if !mint_state.is_initialized || mint_state.decimals != decimals {
        return Err(ProgramError::InvalidAccountData);
    }
    let source_pre = source
        .try_borrow_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?;
    let source_state =
        TokenAccount::parse(&source_pre).map_err(|_| ProgramError::InvalidAccountData)?;
    let destination_pre = destination
        .try_borrow_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?;
    let destination_state =
        TokenAccount::parse(&destination_pre).map_err(|_| ProgramError::InvalidAccountData)?;
    if source_state.mint != destination_state.mint
        || source_state.mint != mint.key.to_bytes()
        || source_state.delegate != COption::Some(authority.key.to_bytes())
        || destination_state.close_authority != COption::None
        || source_state.amount < amount
        || source_state.delegated_amount < amount
    {
        return Err(ProgramError::InvalidAccountData);
    }
    let source_amount = source_state
        .amount
        .checked_sub(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    let delegated_amount = source_state
        .delegated_amount
        .checked_sub(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    let destination_amount = destination_state
        .amount
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    let delegate = if delegated_amount == 0 {
        COption::None
    } else {
        COption::Some(authority.key.to_bytes())
    };
    let source_post = TokenAccount::project_delegated_source_poststate(
        &source_pre,
        source_amount,
        delegate,
        delegated_amount,
    )
    .map_err(|_| ProgramError::InvalidAccountData)?;
    let mut destination_post =
        TokenAccount::project_amount_poststate(&destination_pre, destination_amount)
            .map_err(|_| ProgramError::InvalidAccountData)?;
    drop(source_pre);
    drop(destination_pre);
    destination_post
        .get_mut(TokenAccountLayoutV1::CLOSE_AUTHORITY..TokenAccountLayoutV1::CLOSE_AUTHORITY + 4)
        .ok_or(ProgramError::InvalidAccountData)?
        .copy_from_slice(&[1, 0, 0, 0]);
    destination_post
        .get_mut(
            TokenAccountLayoutV1::CLOSE_AUTHORITY + 4..TokenAccountLayoutV1::CLOSE_AUTHORITY + 36,
        )
        .ok_or(ProgramError::InvalidAccountData)?
        .copy_from_slice(&authority.key.to_bytes());
    if source_post.len() != ACCOUNT_BYTES || destination_post.len() != ACCOUNT_BYTES {
        return Err(ProgramError::InvalidAccountData);
    }
    source
        .try_borrow_mut_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?
        .copy_from_slice(&source_post);
    destination
        .try_borrow_mut_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?
        .copy_from_slice(&destination_post);
    Ok(())
}
