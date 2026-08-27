//! Pre-fund-safe construction plan for permanent descriptor and mint accounts.

use crate::{Error, Result, StructuredClaimRuntimeAddressesV1, DESCRIPTOR_ACCOUNT_BYTES};

/// Exact extension-free Token-2022 Mint account width.
pub const WRAPPER_MINT_ACCOUNT_BYTES: usize = 82;

/// Read-only pre-allocation projection for one predictable PDA target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PermanentTargetProjectionV1 {
    /// Exact target address.
    pub address: [u8; 32],
    /// Current account owner.
    pub owner: [u8; 32],
    /// Current lamports, including hostile or benevolent pre-funding.
    pub lamports: u64,
    /// Current data length. Version one admits only an unallocated target.
    pub data_len: u32,
    /// Executable targets always refuse.
    pub executable: bool,
}

/// Exact shortfalls and permanently locked pre-funding for descriptor creation.
///
/// Descriptor and mint are permanent identity tombstones. Their pre-existing
/// lamports therefore never become caller, keeper, fee, treasury, or refund
/// authority. The creator funds only the shortfall to the exact rent minimum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PermanentIdentityFundingPlanV1 {
    /// Creator that transfers both shortfalls.
    pub payer: [u8; 32],
    /// Exact descriptor allocation width.
    pub descriptor_data_len: u32,
    /// Exact extension-free mint allocation width.
    pub mint_data_len: u32,
    /// Lamports the creator must add to the descriptor.
    pub descriptor_shortfall_lamports: u64,
    /// Lamports the creator must add to the mint.
    pub mint_shortfall_lamports: u64,
    /// Pre-existing descriptor lamports that remain permanently locked.
    pub descriptor_locked_prefund_lamports: u64,
    /// Pre-existing mint lamports that remain permanently locked.
    pub mint_locked_prefund_lamports: u64,
    /// Exact descriptor lamports after funding, before allocation/assignment.
    pub descriptor_final_lamports: u64,
    /// Exact mint lamports after funding, before allocation/assignment.
    pub mint_final_lamports: u64,
}

/// Prepare predictable-PDA funding without granting authority to a pre-funder.
///
/// The SBF adapter must next invoke PDA-signed System allocate/assign for the
/// descriptor and Token-2022 InitializeMint2 for the mint, checking exact
/// owner/data/post-lamports after every CPI. Vault Position and Replay rent use
/// the base program's separately owned rent split and are not conflated here.
pub fn prepare_permanent_identity_funding_v1(
    payer: [u8; 32],
    system_program: [u8; 32],
    wrapper_program: [u8; 32],
    token_2022_program: [u8; 32],
    addresses: StructuredClaimRuntimeAddressesV1,
    descriptor: PermanentTargetProjectionV1,
    mint: PermanentTargetProjectionV1,
    descriptor_rent_minimum: u64,
    mint_rent_minimum: u64,
) -> Result<PermanentIdentityFundingPlanV1> {
    addresses.validate()?;
    let identities = [
        payer,
        system_program,
        wrapper_program,
        token_2022_program,
        addresses.descriptor,
        addresses.mint,
        addresses.mint_authority,
        addresses.vault_owner,
    ];
    require_distinct_nonzero(&identities)?;
    if descriptor.address != addresses.descriptor
        || mint.address != addresses.mint
        || descriptor.address == mint.address
        || descriptor.owner != system_program
        || mint.owner != system_program
        || descriptor.data_len != 0
        || mint.data_len != 0
        || descriptor.executable
        || mint.executable
        || descriptor_rent_minimum == 0
        || mint_rent_minimum == 0
    {
        return Err(Error::InvalidAccount);
    }
    let descriptor_shortfall_lamports = descriptor_rent_minimum
        .checked_sub(descriptor.lamports)
        .unwrap_or(0);
    let mint_shortfall_lamports = mint_rent_minimum.checked_sub(mint.lamports).unwrap_or(0);
    let descriptor_final_lamports = descriptor
        .lamports
        .checked_add(descriptor_shortfall_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let mint_final_lamports = mint
        .lamports
        .checked_add(mint_shortfall_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    if descriptor_final_lamports < descriptor_rent_minimum
        || mint_final_lamports < mint_rent_minimum
    {
        return Err(Error::InvariantViolation);
    }
    Ok(PermanentIdentityFundingPlanV1 {
        payer,
        descriptor_data_len: u32::try_from(DESCRIPTOR_ACCOUNT_BYTES)
            .map_err(|_| Error::ArithmeticOverflow)?,
        mint_data_len: u32::try_from(WRAPPER_MINT_ACCOUNT_BYTES)
            .map_err(|_| Error::ArithmeticOverflow)?,
        descriptor_shortfall_lamports,
        mint_shortfall_lamports,
        descriptor_locked_prefund_lamports: descriptor.lamports,
        mint_locked_prefund_lamports: mint.lamports,
        descriptor_final_lamports,
        mint_final_lamports,
    })
}

fn require_distinct_nonzero(keys: &[[u8; 32]]) -> Result<()> {
    let mut left = 0_usize;
    while left < keys.len() {
        if keys[left] == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut right = left + 1;
        while right < keys.len() {
            if keys[left] == keys[right] {
                return Err(Error::InvalidIdentity);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}
