//! Exact upgradeable-loader Program/ProgramData authentication.

use solana_account_info::AccountInfo;

use crate::error::{Result, WrapperError};

/// Upgradeable Loader v3 executable.
pub const UPGRADEABLE_LOADER_ID: [u8; 32] = [
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61,
    22, 193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
];

/// Exact authenticated deployment link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeploymentV1 {
    /// ProgramData address linked by the executable account.
    pub program_data: [u8; 32],
    /// Deployment or last-upgrade slot in ProgramData.
    pub slot: u64,
}

/// Decode one read-only loader-v3 Program/ProgramData pair.
pub fn authenticate(program: &AccountInfo<'_>, data: &AccountInfo<'_>) -> Result<DeploymentV1> {
    if program.owner.to_bytes() != UPGRADEABLE_LOADER_ID
        || data.owner.to_bytes() != UPGRADEABLE_LOADER_ID
        || !program.executable
        || data.executable
        || program.is_writable
        || data.is_writable
        || program.is_signer
        || data.is_signer
    {
        return Err(WrapperError::Deployment);
    }
    let program_body = program
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let data_body = data.try_borrow_data().map_err(|_| WrapperError::Borrow)?;
    if program_body.len() < 36
        || data_body.len() < 45
        || program_body[0..4] != 2_u32.to_le_bytes()
        || data_body[0..4] != 3_u32.to_le_bytes()
        || program_body[4..36] != data.key.to_bytes()
        || !matches!(data_body[12], 0 | 1)
    {
        return Err(WrapperError::Deployment);
    }
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(&data_body[4..12]);
    Ok(DeploymentV1 {
        program_data: data.key.to_bytes(),
        slot: u64::from_le_bytes(slot),
    })
}
