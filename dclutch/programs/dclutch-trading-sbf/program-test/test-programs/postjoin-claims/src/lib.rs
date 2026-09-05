#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF Claims adversary that commits the genuine sparse transfer, then
//! corrupts one nonselected aggregate supply before returning its genuine ACK.

extern crate std;

use dclutch_claims::{
    liability_basis_state_v2::{LiabilityBasisMarketLayoutV2, LiabilityBasisMarketViewV2},
    sparse_native_transfer_v1::{SPARSE_NATIVE_TRANSFER_BYTES_V1, SparseNativeTransferV1},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = SparseNativeTransferV1::decode(
        instruction_data
            .get(..SPARSE_NATIVE_TRANSFER_BYTES_V1)
            .ok_or(ProgramError::InvalidInstructionData)?,
    )
    .map_err(|_| ProgramError::InvalidInstructionData)?;
    let input = request.input();
    if input.claim_count < 2 {
        return Err(ProgramError::InvalidInstructionData);
    }
    dclutch_claims_sbf::process_instruction(program_id, accounts, instruction_data)?;

    let mut aggregate = None;
    for account in accounts {
        if account.owner != program_id || !account.is_writable {
            continue;
        }
        let data = account
            .try_borrow_data()
            .map_err(|_| ProgramError::AccountBorrowFailed)?;
        if LiabilityBasisMarketViewV2::decode(&data).is_ok() {
            drop(data);
            if aggregate.replace(account).is_some() {
                return Err(ProgramError::InvalidAccountData);
            }
        }
    }
    let aggregate = aggregate.ok_or(ProgramError::InvalidAccountData)?;
    let nonselected = if input.outcome == 0 { 1_u32 } else { 0_u32 };
    let byte = usize::try_from(nonselected)
        .ok()
        .and_then(|index| index.checked_mul(LiabilityBasisMarketLayoutV2::SUPPLY_STRIDE))
        .and_then(|offset| LiabilityBasisMarketLayoutV2::SUPPLIES.checked_add(offset))
        .ok_or(ProgramError::InvalidAccountData)?;
    let mut data = aggregate
        .try_borrow_mut_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?;
    let value = data.get_mut(byte).ok_or(ProgramError::InvalidAccountData)?;
    *value ^= 1;
    LiabilityBasisMarketViewV2::decode(&data).map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(())
}
