//! Concrete Token-2022 parser policies and exact instruction encodings.

use crate::runtime_contract::{WrapperMintProjectionV1, WrapperTokenProjectionV1};
use crate::{
    is_zero, BoundDescriptorV1, CpiAccountMetaV1, Error, Key, Result, Token2022DecoderV1,
};

/// Exact Token-2022 CPI operation emitted by the current wrapper boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Token2022CpiV1 {
    /// Initialize an extension-free zero-decimal mint.
    InitializeMint {
        /// Pinned Token-2022 executable.
        token_program: Key,
        /// Canonical wrapper mint.
        mint: Key,
        /// Canonical mint-authority PDA.
        mint_authority: Key,
    },
    /// Mint the exact wrapper quantity after backing enters custody.
    MintChecked {
        /// Canonical wrapper mint.
        mint: Key,
        /// Holder token account.
        token: Key,
        /// Mint-authority PDA.
        authority: Key,
        /// Exact zero-decimal quantity.
        quantity: u64,
        /// Actual supply before CPI.
        supply_before: u64,
        /// Required supply after CPI.
        supply_after: u64,
        /// Actual holder balance before CPI.
        holder_before: u64,
        /// Required holder balance after CPI.
        holder_after: u64,
    },
    /// Burn the exact wrapper quantity before backing leaves custody.
    BurnChecked {
        /// Canonical wrapper mint.
        mint: Key,
        /// Holder token account.
        token: Key,
        /// Signing token owner.
        authority: Key,
        /// Exact zero-decimal quantity.
        quantity: u64,
        /// Actual supply before CPI.
        supply_before: u64,
        /// Required supply after CPI.
        supply_after: u64,
        /// Actual holder balance before CPI.
        holder_before: u64,
        /// Required holder balance after CPI.
        holder_after: u64,
    },
    /// Permanently revoke mint authority after the empty vault closes.
    RevokeMintAuthority {
        /// Canonical wrapper mint.
        mint: Key,
        /// Current mint-authority PDA.
        authority_before: Key,
        /// Absent authority, canonically zero.
        authority_after: Key,
    },
}

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
const TOKEN_2022_MINT_BYTES: usize = 82;
const TOKEN_2022_ACCOUNT_BYTES: usize = 165;
const TOKEN_2022_ACCOUNT_TYPE_ACCOUNT: u8 = 2;
const IMMUTABLE_OWNER_EXTENSION_TYPE: u16 = 7;

/// Concrete hostile Token-2022 decoder bound to one descriptor-selected mint
/// and one exact holder account.
///
/// This is the live parser implementation behind [`Token2022DecoderV1`], not
/// a projection supplied by a caller. It accepts exactly the base Mint layout
/// and either the base Account layout or the sole canonical zero-width
/// `ImmutableOwner` TLV suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalToken2022DecoderV1 {
    mint: WrapperMintParserPlanV1,
    token: WrapperTokenParserPlanV1,
}

impl CanonicalToken2022DecoderV1 {
    /// Bind one mint and holder parser plan, refusing detached policies.
    pub fn new(mint: WrapperMintParserPlanV1, token: WrapperTokenParserPlanV1) -> Result<Self> {
        if mint.token_program != token.token_program
            || mint.address != token.mint
            || is_zero(&mint.token_program)
            || is_zero(&mint.address)
            || is_zero(&mint.mint_authority)
            || is_zero(&token.address)
            || is_zero(&token.owner)
            || token.address == token.owner
            || token.address == token.mint
            || usize::from(mint.exact_data_len) != TOKEN_2022_MINT_BYTES
            || mint.decimals != 0
            || mint.allowed_extensions != 0
            || !mint.require_no_freeze_authority
            || !mint.require_initialized
            || usize::from(token.base_data_len) != TOKEN_2022_ACCOUNT_BYTES
            || usize::from(token.immutable_owner_data_len)
                != usize::from(TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES)
            || token.allowed_extensions != IMMUTABLE_OWNER_EXTENSION_MASK
            || !token.require_no_delegate
            || !token.require_no_close_authority
            || !token.require_non_native
            || !token.require_unfrozen
            || !token.require_initialized
        {
            return Err(Error::Token2022Boundary);
        }
        Ok(Self { mint, token })
    }
}

impl Token2022DecoderV1 for CanonicalToken2022DecoderV1 {
    fn decode_mint(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperMintProjectionV1, ()> {
        decode_mint_with_plan(self.mint, address, data)
    }

    fn decode_token(
        &self,
        address: Key,
        data: &[u8],
    ) -> core::result::Result<WrapperTokenProjectionV1, ()> {
        decode_token_with_plan(self.token, address, data)
    }
}

fn decode_mint_with_plan(
    plan: WrapperMintParserPlanV1,
    address: Key,
    data: &[u8],
) -> core::result::Result<WrapperMintProjectionV1, ()> {
    if address != plan.address || data.len() != TOKEN_2022_MINT_BYTES {
        return Err(());
    }
    let mint_authority = read_required_coption_key(data, 0)?;
    let supply = read_u64(data, 36)?;
    let decimals = *data.get(44).ok_or(())?;
    let initialized = match data.get(45) {
        Some(1) => true,
        _ => return Err(()),
    };
    require_absent_coption_key(data, 46)?;
    if mint_authority != plan.mint_authority || decimals != plan.decimals {
        return Err(());
    }
    Ok(WrapperMintProjectionV1 {
        address,
        mint_authority,
        supply,
        decimals,
        freeze_authority: [0; 32],
        extension_mask: 0,
        initialized,
    })
}

fn decode_token_with_plan(
    plan: WrapperTokenParserPlanV1,
    address: Key,
    data: &[u8],
) -> core::result::Result<WrapperTokenProjectionV1, ()> {
        if address != plan.address
            || (data.len() != TOKEN_2022_ACCOUNT_BYTES
                && data.len() != usize::from(TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES))
        {
            return Err(());
        }
        if data.len() == usize::from(TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES) {
            let extension_type = IMMUTABLE_OWNER_EXTENSION_TYPE.to_le_bytes();
            if data[165] != TOKEN_2022_ACCOUNT_TYPE_ACCOUNT
                || data[166..168] != extension_type
                || data[168..170] != [0, 0]
            {
                return Err(());
            }
        }
        let mint = read_key(data, 0)?;
        let owner = read_key(data, 32)?;
        let amount = read_u64(data, 64)?;
        require_absent_coption_key(data, 72)?;
        if data.get(108) != Some(&1)
            || data[109..113] != [0, 0, 0, 0]
            || data[113..121] != [0; 8]
            || data[121..129] != [0; 8]
        {
            return Err(());
        }
        require_absent_coption_key(data, 129)?;
        if mint != plan.mint || owner != plan.owner {
            return Err(());
        }
        Ok(WrapperTokenProjectionV1 {
            address,
            mint,
            owner,
            amount,
            initialized: true,
        })
}

fn read_key(data: &[u8], offset: usize) -> core::result::Result<Key, ()> {
    let mut value = [0; 32];
    value.copy_from_slice(data.get(offset..offset + 32).ok_or(())?);
    Ok(value)
}

fn read_u64(data: &[u8], offset: usize) -> core::result::Result<u64, ()> {
    let mut value = [0; 8];
    value.copy_from_slice(data.get(offset..offset + 8).ok_or(())?);
    Ok(u64::from_le_bytes(value))
}

fn read_required_coption_key(data: &[u8], offset: usize) -> core::result::Result<Key, ()> {
    if data.get(offset..offset + 4) != Some(&[1, 0, 0, 0][..]) {
        return Err(());
    }
    let value = read_key(data, offset + 4)?;
    if is_zero(&value) {
        return Err(());
    }
    Ok(value)
}

fn require_absent_coption_key(data: &[u8], offset: usize) -> core::result::Result<(), ()> {
    if data.get(offset..offset + 4) != Some(&[0, 0, 0, 0][..])
        || data.get(offset + 4..offset + 36) != Some(&[0; 32][..])
    {
        return Err(());
    }
    Ok(())
}

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
        exact_data_len: u16::try_from(crate::runtime_contract::WRAPPER_MINT_ACCOUNT_BYTES)
            .map_err(|_| Error::Arithmetic)?,
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

/// Decode one exact extension-free active wrapper mint from independently
/// authenticated account metadata.
pub fn decode_canonical_wrapper_mint_v1(
    token_program: Key,
    mint: Key,
    mint_authority: Key,
    data: &[u8],
) -> Result<WrapperMintProjectionV1> {
    let plan = WrapperMintParserPlanV1 {
        token_program,
        address: mint,
        exact_data_len: crate::runtime_contract::WRAPPER_MINT_ACCOUNT_BYTES
            .try_into()
            .map_err(|_| Error::Arithmetic)?,
        decimals: 0,
        mint_authority,
        allowed_extensions: 0,
        require_no_freeze_authority: true,
        require_initialized: true,
    };
    decode_mint_with_plan(plan, mint, data)
        .map_err(|_| Error::Token2022Boundary)
}

/// Decode the sole terminal wrapper-mint image after exact mint-authority
/// revocation. This is deliberately disjoint from the active-mint decoder:
/// the authority COption must be canonical None and supply must be exactly
/// zero, while decimals, initialization, extension width, and freeze authority
/// remain frozen to the original wrapper policy.
pub fn decode_retired_canonical_wrapper_mint_v1(
    token_program: Key,
    mint: Key,
    data: &[u8],
) -> Result<WrapperMintProjectionV1> {
    if is_zero(&token_program) || is_zero(&mint) || data.len() != TOKEN_2022_MINT_BYTES {
        return Err(Error::Token2022Boundary);
    }
    require_absent_coption_key(data, 0).map_err(|_| Error::Token2022Boundary)?;
    let supply = read_u64(data, 36).map_err(|_| Error::Token2022Boundary)?;
    if supply != 0 || data.get(44) != Some(&0) || data.get(45) != Some(&1) {
        return Err(Error::Token2022Boundary);
    }
    require_absent_coption_key(data, 46).map_err(|_| Error::Token2022Boundary)?;
    Ok(WrapperMintProjectionV1 {
        address: mint,
        mint_authority: [0; 32],
        supply,
        decimals: 0,
        freeze_authority: [0; 32],
        extension_mask: 0,
        initialized: true,
    })
}

/// Decode one exact initialized wrapper holder account. Only the
/// zero-extension base layout or sole ImmutableOwner extension is admitted.
pub fn decode_canonical_wrapper_token_v1(
    token_program: Key,
    mint: Key,
    address: Key,
    owner: Key,
    data: &[u8],
) -> Result<WrapperTokenProjectionV1> {
    let token_plan = WrapperTokenParserPlanV1 {
        token_program,
        address,
        mint,
        owner,
        base_data_len: TOKEN_2022_BASE_ACCOUNT_BYTES,
        immutable_owner_data_len: TOKEN_2022_IMMUTABLE_OWNER_ACCOUNT_BYTES,
        allowed_extensions: IMMUTABLE_OWNER_EXTENSION_MASK,
        require_no_delegate: true,
        require_no_close_authority: true,
        require_non_native: true,
        require_unfrozen: true,
        require_initialized: true,
    };
    decode_token_with_plan(token_plan, address, data)
        .map_err(|_| Error::Token2022Boundary)
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

    fn decoder() -> CanonicalToken2022DecoderV1 {
        CanonicalToken2022DecoderV1::new(
            WrapperMintParserPlanV1 {
                token_program: [4; 32],
                address: [1; 32],
                exact_data_len: 82,
                decimals: 0,
                mint_authority: [2; 32],
                allowed_extensions: 0,
                require_no_freeze_authority: true,
                require_initialized: true,
            },
            WrapperTokenParserPlanV1 {
                token_program: [4; 32],
                address: [3; 32],
                mint: [1; 32],
                owner: [5; 32],
                base_data_len: 165,
                immutable_owner_data_len: 170,
                allowed_extensions: IMMUTABLE_OWNER_EXTENSION_MASK,
                require_no_delegate: true,
                require_no_close_authority: true,
                require_non_native: true,
                require_unfrozen: true,
                require_initialized: true,
            },
        )
        .unwrap()
    }

    fn mint_bytes() -> [u8; 82] {
        let mut bytes = [0; 82];
        bytes[..4].copy_from_slice(&[1, 0, 0, 0]);
        bytes[4..36].copy_from_slice(&[2; 32]);
        bytes[36..44].copy_from_slice(&9_u64.to_le_bytes());
        bytes[45] = 1;
        bytes
    }

    fn token_bytes() -> [u8; 165] {
        let mut bytes = [0; 165];
        bytes[..32].copy_from_slice(&[1; 32]);
        bytes[32..64].copy_from_slice(&[5; 32]);
        bytes[64..72].copy_from_slice(&7_u64.to_le_bytes());
        bytes[108] = 1;
        bytes
    }

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

    #[test]
    fn canonical_decoder_accepts_only_exact_wrapper_states() {
        let decoder = decoder();
        let mint = decoder.decode_mint([1; 32], &mint_bytes()).unwrap();
        assert_eq!(mint.supply, 9);
        let token = decoder.decode_token([3; 32], &token_bytes()).unwrap();
        assert_eq!(token.amount, 7);

        let mut immutable = [0; 170];
        immutable[..165].copy_from_slice(&token_bytes());
        immutable[165] = TOKEN_2022_ACCOUNT_TYPE_ACCOUNT;
        immutable[166..168].copy_from_slice(&IMMUTABLE_OWNER_EXTENSION_TYPE.to_le_bytes());
        assert!(decoder.decode_token([3; 32], &immutable).is_ok());
    }

    #[test]
    fn hostile_token_options_and_extensions_fail_closed() {
        let decoder = decoder();
        let mut delegated = token_bytes();
        delegated[72..76].copy_from_slice(&[1, 0, 0, 0]);
        delegated[76..108].copy_from_slice(&[8; 32]);
        assert!(decoder.decode_token([3; 32], &delegated).is_err());

        let mut native = token_bytes();
        native[109..113].copy_from_slice(&[1, 0, 0, 0]);
        native[113..121].copy_from_slice(&1_u64.to_le_bytes());
        assert!(decoder.decode_token([3; 32], &native).is_err());

        let mut frozen = token_bytes();
        frozen[108] = 2;
        assert!(decoder.decode_token([3; 32], &frozen).is_err());

        let mut wrong_extension = [0; 170];
        wrong_extension[..165].copy_from_slice(&token_bytes());
        wrong_extension[165] = TOKEN_2022_ACCOUNT_TYPE_ACCOUNT;
        wrong_extension[166..168].copy_from_slice(&8_u16.to_le_bytes());
        assert!(decoder.decode_token([3; 32], &wrong_extension).is_err());

        let mut noncanonical_none = mint_bytes();
        noncanonical_none[50] = 1;
        assert!(decoder.decode_mint([1; 32], &noncanonical_none).is_err());
    }

    #[test]
    fn terminal_mint_decoder_is_disjoint_and_fail_closed() {
        let mut retired = mint_bytes();
        retired[..36].fill(0);
        retired[36..44].copy_from_slice(&0_u64.to_le_bytes());
        let observed = decode_retired_canonical_wrapper_mint_v1([4; 32], [1; 32], &retired)
            .expect("canonical revoked zero-supply mint");
        assert_eq!(observed.mint_authority, [0; 32]);
        assert_eq!(observed.supply, 0);
        assert!(decode_canonical_wrapper_mint_v1([4; 32], [1; 32], [2; 32], &retired).is_err());

        let mut live_supply = retired;
        live_supply[36..44].copy_from_slice(&1_u64.to_le_bytes());
        assert!(decode_retired_canonical_wrapper_mint_v1([4; 32], [1; 32], &live_supply).is_err());

        let mut hidden_authority = retired;
        hidden_authority[..4].copy_from_slice(&[1, 0, 0, 0]);
        hidden_authority[4..36].copy_from_slice(&[2; 32]);
        assert!(decode_retired_canonical_wrapper_mint_v1(
            [4; 32],
            [1; 32],
            &hidden_authority,
        )
        .is_err());
    }
}
