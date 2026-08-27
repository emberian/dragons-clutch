//! Prefund-safe permanent PDA allocation for descriptor and mint identities.

use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_sdk_ids::system_program;
use solana_sysvar::SysvarSerialize;

use crate::error::{Result, WrapperError};

/// Authenticate and decode the bank's current Rent sysvar.
pub fn rent(account: &AccountInfo<'_>) -> Result<Rent> {
    if !solana_sdk_ids::sysvar::rent::check_id(account.key)
        || account.is_writable
        || account.is_signer
    {
        return Err(WrapperError::Construction);
    }
    Rent::from_account_info(account).map_err(|_| WrapperError::Construction)
}

/// Allocate and assign one permanent predictable PDA after adding only its
/// exact rent shortfall. Existing prefund remains permanently locked and
/// grants no authority.
#[allow(clippy::too_many_arguments)]
pub fn create_permanent_pda<'a>(
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    owner_after: &Pubkey,
    space: usize,
    minimum: u64,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    if *system.key != system_program::ID
        || !system.executable
        || system.is_writable
        || !payer.is_signer
        || !payer.is_writable
        || !target.is_writable
        || target.executable
        || target.data_len() != 0
        || *target.owner != system_program::ID
        || minimum == 0
    {
        return Err(WrapperError::Construction);
    }
    let prefund = target.lamports();
    let shortfall = minimum.saturating_sub(prefund);
    if shortfall != 0 {
        let payer_before = payer.lamports();
        let instruction = Instruction::new_with_bytes(
            system_program::ID,
            &transfer_data(shortfall),
            vec![
                AccountMeta::new(*payer.key, true),
                AccountMeta::new(*target.key, false),
            ],
        );
        invoke(
            &instruction,
            &[payer.clone(), target.clone(), system.clone()],
        )
        .map_err(|_| WrapperError::Construction)?;
        if payer.lamports() != payer_before.checked_sub(shortfall).ok_or(WrapperError::Arithmetic)?
            || target.lamports()
                != prefund
                    .checked_add(shortfall)
                    .ok_or(WrapperError::Arithmetic)?
        {
            return Err(WrapperError::Construction);
        }
    }
    let allocate = Instruction::new_with_bytes(
        system_program::ID,
        &allocate_data(space)?,
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| WrapperError::Construction)?;
    let assign = Instruction::new_with_bytes(
        system_program::ID,
        &assign_data(owner_after),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| WrapperError::Construction)?;
    if target.data_len() != space || target.owner != owner_after || target.lamports() < minimum {
        return Err(WrapperError::Construction);
    }
    Ok(())
}

fn transfer_data(lamports: u64) -> [u8; 12] {
    let mut data = [0_u8; 12];
    data[0..4].copy_from_slice(&2_u32.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data
}

fn allocate_data(space: usize) -> Result<[u8; 12]> {
    let width = u64::try_from(space).map_err(|_| WrapperError::Arithmetic)?;
    let mut data = [0_u8; 12];
    data[0..4].copy_from_slice(&8_u32.to_le_bytes());
    data[4..12].copy_from_slice(&width.to_le_bytes());
    Ok(data)
}

fn assign_data(owner: &Pubkey) -> [u8; 36] {
    let mut data = [0_u8; 36];
    data[0..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(owner.as_ref());
    data
}
