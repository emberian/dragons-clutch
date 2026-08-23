//! Concrete Token-2022 parser policies and exact instruction encodings.

use crate::{is_zero, BoundDescriptorV1, CpiAccountMetaV1, Error, Key, Result, Token2022CpiV1};

/// Base Token/Token-2022 account width.
pub const TOKEN_2022_BASE_ACCOUNT_BYTES: u16 = 165;
/// Token-2022 account carrying only the zero-width ImmutableOwner extension.
pub const TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES: u16 = 170;
/// Largest Token-2022 instruction body emitted by this adapter.
pub const TOKEN_2022_INSTRUCTION_DATA_CAPACITY: usize = 35;
/// Maximum account metas used by the emitted Token-2022 instructions.
pub const TOKEN_2022_CPI_META_CAPACITY: usize = 3;

const INITIALIZE_MINT2: u8 = 20;
const MINT_TO_CHECKED: u8 = 14;
const BURN_CHECKED: u8 = 15;
const SET_AUTHORITY: u8 = 6;
const AUTHORITY_TYPE_MINT_TOKENS: u8 = 0;
const COPTION_NONE: u8 = 0;
const IMMUTABLE_OWNER_EXTENSION_MASK: u64 = 1_u64 << 7;

/// Exact hostile-byte policy for the extension-free wrapper mint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperMintParserPlanV1 {
    /// Exact Token-2022 executable selected by the immutable descriptor.
    pub token_program: Key,
    /// Exact wrapper mint PDA.
    pub address: Key,
    /// Exact extension-free byte length; always 82.
    pub exact_data_len: u16,
    /// Exact decimals; always zero.
    pub decimals: u8,
    /// Exact mint-authority PDA while the descriptor is active.
    pub mint_authority: Key,
    /// Allowed extension bitset; always empty.
    pub allowed_extensions: u64,
    /// Freeze authority must be absent.
    pub require_no_freeze_authority: bool,
    /// Mint must be initialized.
    pub require_initialized: bool,
}

/// Exact hostile-byte policy for an ordinary wrapper holder account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperTokenParserPlanV1 {
    /// Exact Token-2022 executable selected by the immutable descriptor.
    pub token_program: Key,
    /// Exact holder token-account address.
    pub address: Key,
    /// Exact wrapper mint PDA.
    pub mint: Key,
    /// Exact token owner that must sign burns.
    pub owner: Key,
    /// Extension-free account width.
    pub base_data_len: u16,
    /// Alternative width carrying only ImmutableOwner.
    pub immutable_owner_data_len: u16,
    /// Only ImmutableOwner may be present.
    pub allowed_extensions: u64,
    /// Delegates must be absent.
    pub require_no_delegate: bool,
    /// Close authority must be absent.
    pub require_no_close_authority: bool,
    /// Native/wrapped-native state must be absent.
    pub require_non_native: bool,
    /// Frozen state is refused.
    pub require_unfrozen: bool,
    /// Token account must be initialized.
    pub require_initialized: bool,
}

/// Exact fixed-capacity Token-2022 instruction plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022InstructionPlanV1 {
    /// Exact Token-2022 executable.
    pub program_id: Key,
    /// Active instruction-data prefix length.
    pub data_len: u8,
    /// Exact active bytes followed by canonical zero padding.
    pub data: [u8; TOKEN_2022_INSTRUCTION_DATA_CAPACITY],
    /// Active account-meta prefix length.
    pub account_count: u8,
    /// Exact active metas followed by zero-address sentinels.
    pub accounts: [CpiAccountMetaV1; TOKEN_2022_CPI_META_CAPACITY],
}

/// Build the descriptor-selected wrapper-mint parser policy.
pub fn wrapper_mint_parser_plan_v1(
    descriptor: &BoundDescriptorV1,
) -> Result<WrapperMintParserPlanV1> {
    let identity = descriptor.identity();
    let addresses = descriptor.addresses();
    if is_zero(&identity.deployment.token_2022_program)
        || is_zero(&addresses.mint)
        || is_zero(&addresses.mint_authority)
    {
        return Err(Error::Token2022Boundary);
    }
    Ok(WrapperMintParserPlanV1 {
        token_program: identity.deployment.token_2022_program,
        address: addresses.mint,
        exact_data_len: crate::runtime_contract::WRAPPER_MINT_ACCOUNT_BYTES as u16,
        decimals: 0,
        mint_authority: addresses.mint_authority,
        allowed_extensions: 0,
        require_no_freeze_authority: true,
        require_initialized: true,
    })
}

/// Build the descriptor-selected ordinary holder parser policy.
pub fn wrapper_token_parser_plan_v1(
    descriptor: &BoundDescriptorV1,
    address: Key,
    owner: Key,
) -> Result<WrapperTokenParserPlanV1> {
    if is_zero(&address) || is_zero(&owner) || address == owner {
        return Err(Error::Token2022Boundary);
    }
    Ok(WrapperTokenParserPlanV1 {
        token_program: descriptor.identity().deployment.token_2022_program,
        address,
        mint: descriptor.addresses().mint,
        owner,
        base_data_len: TOKEN_2022_BASE_ACCOUNT_BYTES,
        immutable_owner_data_len: TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES,
        allowed_extensions: IMMUTABLE_OWNER_EXTENSION_MASK,
        require_no_delegate: true,
        require_no_close_authority: true,
        require_non_native: true,
        require_unfrozen: true,
        require_initialized: true,
    })
}

/// Encode one staged Token-2022 operation into exact CPI bytes and metas.
pub fn plan_token_2022_cpi_v1(
    token_program: Key,
    operation: Token2022CpiV1,
) -> Result<Token2022InstructionPlanV1> {
    if is_zero(&token_program) {
        return Err(Error::Token2022Boundary);
    }
    let mut plan = empty_plan(token_program);
    match operation {
        Token2022CpiV1::InitializeMint {
            token_program: selected,
            mint,
            mint_authority,
        } => {
            if selected != token_program || is_zero(&mint) || is_zero(&mint_authority) {
                return Err(Error::Token2022Boundary);
            }
            plan.data_len = 35;
            plan.data[0] = INITIALIZE_MINT2;
            plan.data[1] = 0;
            plan.data[2..34].copy_from_slice(&mint_authority);
            plan.data[34] = COPTION_NONE;
            plan.account_count = 1;
            plan.accounts[0] = meta(mint, false, true);
        }
        Token2022CpiV1::MintChecked {
            mint,
            token,
            authority,
            quantity,
            supply_before,
            supply_after,
            holder_before,
            holder_after,
        } => {
            require_exact_credit(supply_before, supply_after, quantity)?;
            require_exact_credit(holder_before, holder_after, quantity)?;
            checked_quantity(quantity, [mint, token, authority])?;
            plan.data_len = 10;
            plan.data[0] = MINT_TO_CHECKED;
            plan.data[1..9].copy_from_slice(&quantity.to_le_bytes());
            plan.data[9] = 0;
            plan.account_count = 3;
            plan.accounts = [
                meta(mint, false, true),
                meta(token, false, true),
                meta(authority, true, false),
            ];
        }
        Token2022CpiV1::BurnChecked {
            mint,
            token,
            authority,
            quantity,
            supply_before,
            supply_after,
            holder_before,
            holder_after,
        } => {
            require_exact_debit(supply_before, supply_after, quantity)?;
            require_exact_debit(holder_before, holder_after, quantity)?;
            checked_quantity(quantity, [mint, token, authority])?;
            plan.data_len = 10;
            plan.data[0] = BURN_CHECKED;
            plan.data[1..9].copy_from_slice(&quantity.to_le_bytes());
            plan.data[9] = 0;
            plan.account_count = 3;
            plan.accounts = [
                meta(token, false, true),
                meta(mint, false, true),
                meta(authority, true, false),
            ];
        }
        Token2022CpiV1::RevokeMintAuthority {
            mint,
            authority_before,
            authority_after,
        } => {
            if is_zero(&mint) || is_zero(&authority_before) || authority_after != [0; 32] {
                return Err(Error::Token2022Boundary);
            }
            plan.data_len = 3;
            plan.data[0] = SET_AUTHORITY;
            plan.data[1] = AUTHORITY_TYPE_MINT_TOKENS;
            plan.data[2] = COPTION_NONE;
            plan.account_count = 2;
            plan.accounts[0] = meta(mint, false, true);
            plan.accounts[1] = meta(authority_before, true, false);
        }
    }
    Ok(plan)
}

fn empty_plan(program_id: Key) -> Token2022InstructionPlanV1 {
    Token2022InstructionPlanV1 {
        program_id,
        data_len: 0,
        data: [0; TOKEN_2022_INSTRUCTION_DATA_CAPACITY],
        account_count: 0,
        accounts: [meta([0; 32], false, false); TOKEN_2022_CPI_META_CAPACITY],
    }
}

const fn meta(address: Key, signer: bool, writable: bool) -> CpiAccountMetaV1 {
    CpiAccountMetaV1 {
        address,
        signer,
        writable,
    }
}

fn checked_quantity(quantity: u64, identities: [Key; 3]) -> Result<()> {
    if quantity == 0 || identities.iter().any(is_zero) {
        return Err(Error::Token2022Boundary);
    }
    if identities[0] == identities[1]
        || identities[0] == identities[2]
        || identities[1] == identities[2]
    {
        return Err(Error::Token2022Boundary);
    }
    Ok(())
}

fn require_exact_credit(before: u64, after: u64, quantity: u64) -> Result<()> {
    if before.checked_add(quantity) != Some(after) {
        return Err(Error::PostStateMismatch);
    }
    Ok(())
}

fn require_exact_debit(before: u64, after: u64, quantity: u64) -> Result<()> {
    if before.checked_sub(quantity) != Some(after) {
        return Err(Error::PostStateMismatch);
    }
    Ok(())
}

const _: () = assert!(crate::runtime_contract::WRAPPER_MINT_ACCOUNT_BYTES == 82);
const _: () = assert!(TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES == 170);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_mint_and_burn_emit_real_token_2022_bytes() {
        let mint = [1; 32];
        let token = [2; 32];
        let authority = [3; 32];
        let mint_plan = plan_token_2022_cpi_v1(
            [4; 32],
            Token2022CpiV1::MintChecked {
                mint,
                token,
                authority,
                quantity: 9,
                supply_before: 10,
                supply_after: 19,
                holder_before: 11,
                holder_after: 20,
            },
        )
        .unwrap();
        assert_eq!(mint_plan.data_len, 10);
        assert_eq!(mint_plan.data[0], 14);
        assert_eq!(&mint_plan.data[1..9], &9_u64.to_le_bytes());
        assert_eq!(mint_plan.data[9], 0);

        let burn_plan = plan_token_2022_cpi_v1(
            [4; 32],
            Token2022CpiV1::BurnChecked {
                mint,
                token,
                authority,
                quantity: 9,
                supply_before: 19,
                supply_after: 10,
                holder_before: 20,
                holder_after: 11,
            },
        )
        .unwrap();
        assert_eq!(burn_plan.data[0], 15);
        assert_eq!(burn_plan.accounts[0].address, token);
        assert_eq!(burn_plan.accounts[1].address, mint);
    }

    #[test]
    fn authority_revocation_is_set_authority_mint_tokens_none() {
        let plan = plan_token_2022_cpi_v1(
            [4; 32],
            Token2022CpiV1::RevokeMintAuthority {
                mint: [1; 32],
                authority_before: [2; 32],
                authority_after: [0; 32],
            },
        )
        .unwrap();
        assert_eq!(plan.data_len, 3);
        assert_eq!(&plan.data[..3], &[6, 0, 0]);
        assert_eq!(plan.account_count, 2);
    }

    #[test]
    fn initialize_mint2_uses_instruction_coption_not_state_coption() {
        let plan = plan_token_2022_cpi_v1(
            [4; 32],
            Token2022CpiV1::InitializeMint {
                token_program: [4; 32],
                mint: [1; 32],
                mint_authority: [2; 32],
            },
        )
        .unwrap();
        assert_eq!(plan.data_len, 35);
        assert_eq!(&plan.data[..2], &[20, 0]);
        assert_eq!(&plan.data[2..34], &[2; 32]);
        assert_eq!(plan.data[34], 0);
    }
}
