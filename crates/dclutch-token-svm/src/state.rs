//! Exact SPL Token Mint and base Account state parsing.

use core::convert::TryInto;

use crate::{Address, Error, Result};

/// Exact SPL Token Mint base width.
pub const MINT_BYTES: usize = 82;
/// Exact SPL Token Account base width.
pub const ACCOUNT_BYTES: usize = 165;

const MINT_AUTHORITY_OFFSET: usize = 0;
const MINT_SUPPLY_OFFSET: usize = 36;
const MINT_DECIMALS_OFFSET: usize = 44;
const MINT_INITIALIZED_OFFSET: usize = 45;
const MINT_FREEZE_AUTHORITY_OFFSET: usize = 46;

const ACCOUNT_MINT_OFFSET: usize = 0;
const ACCOUNT_OWNER_OFFSET: usize = 32;
const ACCOUNT_AMOUNT_OFFSET: usize = 64;
const ACCOUNT_DELEGATE_OFFSET: usize = 72;
const ACCOUNT_STATE_OFFSET: usize = 108;
const ACCOUNT_NATIVE_OFFSET: usize = 109;
const ACCOUNT_DELEGATED_AMOUNT_OFFSET: usize = 121;
const ACCOUNT_CLOSE_AUTHORITY_OFFSET: usize = 129;

/// The SVM's fixed four-byte optional-value representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum COption<T> {
    /// Exact little-endian zero tag; body bytes are semantically ignored.
    None,
    /// Exact little-endian one tag and its typed body.
    Some(T),
}

impl<T> COption<T> {
    /// Return true only for the absent variant.
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Borrow the present value using the ordinary Rust `Option` projection.
    pub const fn as_ref(&self) -> Option<&T> {
        match self {
            Self::None => None,
            Self::Some(value) => Some(value),
        }
    }
}

/// Exact token Account lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountState {
    /// The account has not been initialized.
    Uninitialized = 0,
    /// The account may participate in token operations.
    Initialized = 1,
    /// The account is frozen and cannot transfer.
    Frozen = 2,
}

impl AccountState {
    const fn parse(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Uninitialized),
            1 => Ok(Self::Initialized),
            2 => Ok(Self::Frozen),
            _ => Err(Error::InvalidAccountState),
        }
    }
}

/// Decoded exact-width Mint base state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mint {
    /// Optional mint authority.
    pub mint_authority: COption<Address>,
    /// Total raw token supply.
    pub supply: u64,
    /// Display decimal count used by checked instructions.
    pub decimals: u8,
    /// Whether the Mint is initialized.
    pub is_initialized: bool,
    /// Optional freeze authority.
    pub freeze_authority: COption<Address>,
}

impl Mint {
    /// Parse exactly 82 bytes, refusing truncation and every extension suffix.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != MINT_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            mint_authority: parse_coption_address(bytes, MINT_AUTHORITY_OFFSET)?,
            supply: read_u64(bytes, MINT_SUPPLY_OFFSET)?,
            decimals: read_byte(bytes, MINT_DECIMALS_OFFSET)?,
            is_initialized: parse_bool(read_byte(bytes, MINT_INITIALIZED_OFFSET)?)?,
            freeze_authority: parse_coption_address(bytes, MINT_FREEZE_AUTHORITY_OFFSET)?,
        })
    }
}

/// Decoded exact-width base token Account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccount {
    /// Mint associated with this account.
    pub mint: Address,
    /// Token authority or owner.
    pub owner: Address,
    /// Raw token amount.
    pub amount: u64,
    /// Optional transfer delegate.
    pub delegate: COption<Address>,
    /// Token lifecycle state.
    pub state: AccountState,
    /// Optional wrapped-native rent reserve.
    pub native_reserve: COption<u64>,
    /// Remaining amount authorized to a delegate.
    pub delegated_amount: u64,
    /// Optional authority distinct from the owner that may close the account.
    pub close_authority: COption<Address>,
}

impl TokenAccount {
    /// Parse exactly 165 bytes, refusing truncation and every extension suffix.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ACCOUNT_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            mint: read_array(bytes, ACCOUNT_MINT_OFFSET)?,
            owner: read_array(bytes, ACCOUNT_OWNER_OFFSET)?,
            amount: read_u64(bytes, ACCOUNT_AMOUNT_OFFSET)?,
            delegate: parse_coption_address(bytes, ACCOUNT_DELEGATE_OFFSET)?,
            state: AccountState::parse(read_byte(bytes, ACCOUNT_STATE_OFFSET)?)?,
            native_reserve: parse_coption_u64(bytes, ACCOUNT_NATIVE_OFFSET)?,
            delegated_amount: read_u64(bytes, ACCOUNT_DELEGATED_AMOUNT_OFFSET)?,
            close_authority: parse_coption_address(bytes, ACCOUNT_CLOSE_AUTHORITY_OFFSET)?,
        })
    }
}

fn parse_bool(value: u8) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::InvalidBoolean),
    }
}

fn parse_coption_address(bytes: &[u8], offset: usize) -> Result<COption<Address>> {
    match read_u32(bytes, offset)? {
        0 => Ok(COption::None),
        1 => Ok(COption::Some(read_array(bytes, checked_add(offset, 4)?)?)),
        _ => Err(Error::InvalidOptionTag),
    }
}

fn parse_coption_u64(bytes: &[u8], offset: usize) -> Result<COption<u64>> {
    match read_u32(bytes, offset)? {
        0 => Ok(COption::None),
        1 => Ok(COption::Some(read_u64(bytes, checked_add(offset, 4)?)?)),
        _ => Err(Error::InvalidOptionTag),
    }
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = checked_add(offset, N)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn checked_add(first: usize, second: usize) -> Result<usize> {
    first.checked_add(second).ok_or(Error::InvalidLength)
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub(crate) const MINT_KEY: Address = [7; 32];
    pub(crate) const OWNER_KEY: Address = [8; 32];
    pub(crate) const DESTINATION_OWNER_KEY: Address = [9; 32];

    pub(crate) fn mint() -> [u8; MINT_BYTES] {
        let mut bytes = [0; MINT_BYTES];
        put(&mut bytes, MINT_SUPPLY_OFFSET, &1_000u64.to_le_bytes());
        put(&mut bytes, MINT_DECIMALS_OFFSET, &[6]);
        put(&mut bytes, MINT_INITIALIZED_OFFSET, &[1]);
        bytes
    }

    pub(crate) fn account(owner: Address, amount: u64) -> [u8; ACCOUNT_BYTES] {
        let mut bytes = [0; ACCOUNT_BYTES];
        put(&mut bytes, ACCOUNT_MINT_OFFSET, &MINT_KEY);
        put(&mut bytes, ACCOUNT_OWNER_OFFSET, &owner);
        put(&mut bytes, ACCOUNT_AMOUNT_OFFSET, &amount.to_le_bytes());
        put(
            &mut bytes,
            ACCOUNT_STATE_OFFSET,
            &[AccountState::Initialized as u8],
        );
        bytes
    }

    pub(crate) fn put(output: &mut [u8], offset: usize, input: &[u8]) {
        for (destination, source) in output.iter_mut().skip(offset).zip(input) {
            *destination = *source;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{OWNER_KEY, account, mint, put};
    use super::*;

    #[test]
    fn exact_mint_and_account_fields_are_typed() {
        let mut mint_bytes = mint();
        put(&mut mint_bytes, MINT_AUTHORITY_OFFSET, &[1, 0, 0, 0]);
        put(&mut mint_bytes, MINT_AUTHORITY_OFFSET + 4, &[3; 32]);
        put(&mut mint_bytes, MINT_FREEZE_AUTHORITY_OFFSET, &[1, 0, 0, 0]);
        put(&mut mint_bytes, MINT_FREEZE_AUTHORITY_OFFSET + 4, &[4; 32]);
        let value = Mint::parse(&mint_bytes);
        assert_eq!(
            value,
            Ok(Mint {
                mint_authority: COption::Some([3; 32]),
                supply: 1_000,
                decimals: 6,
                is_initialized: true,
                freeze_authority: COption::Some([4; 32]),
            })
        );

        let mut account_bytes = account(OWNER_KEY, 77);
        put(&mut account_bytes, ACCOUNT_DELEGATE_OFFSET, &[1, 0, 0, 0]);
        put(&mut account_bytes, ACCOUNT_DELEGATE_OFFSET + 4, &[5; 32]);
        put(&mut account_bytes, ACCOUNT_NATIVE_OFFSET, &[1, 0, 0, 0]);
        put(
            &mut account_bytes,
            ACCOUNT_NATIVE_OFFSET + 4,
            &11u64.to_le_bytes(),
        );
        put(
            &mut account_bytes,
            ACCOUNT_DELEGATED_AMOUNT_OFFSET,
            &12u64.to_le_bytes(),
        );
        put(
            &mut account_bytes,
            ACCOUNT_CLOSE_AUTHORITY_OFFSET,
            &[1, 0, 0, 0],
        );
        put(
            &mut account_bytes,
            ACCOUNT_CLOSE_AUTHORITY_OFFSET + 4,
            &[6; 32],
        );
        let value = TokenAccount::parse(&account_bytes);
        assert_eq!(
            value,
            Ok(TokenAccount {
                mint: [7; 32],
                owner: OWNER_KEY,
                amount: 77,
                delegate: COption::Some([5; 32]),
                state: AccountState::Initialized,
                native_reserve: COption::Some(11),
                delegated_amount: 12,
                close_authority: COption::Some([6; 32]),
            })
        );
    }

    #[test]
    fn every_truncation_and_extension_suffix_is_refused() {
        let mint_bytes = mint();
        for length in 0..MINT_BYTES {
            let prefix = mint_bytes.get(..length).unwrap_or(&[]);
            assert_eq!(Mint::parse(prefix), Err(Error::InvalidLength));
        }
        let account_bytes = account(OWNER_KEY, 1);
        for length in 0..ACCOUNT_BYTES {
            let prefix = account_bytes.get(..length).unwrap_or(&[]);
            assert_eq!(TokenAccount::parse(prefix), Err(Error::InvalidLength));
        }
        let mint_extended = [0; MINT_BYTES + 1];
        let account_extended = [0; ACCOUNT_BYTES + 1];
        assert_eq!(Mint::parse(&mint_extended), Err(Error::InvalidLength));
        assert_eq!(
            TokenAccount::parse(&account_extended),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn hostile_option_boolean_and_state_tags_are_refused() {
        for tag in [[2, 0, 0, 0], [0, 1, 0, 0], [255; 4]] {
            let mut bytes = mint();
            put(&mut bytes, MINT_AUTHORITY_OFFSET, &tag);
            assert_eq!(Mint::parse(&bytes), Err(Error::InvalidOptionTag));
        }
        let mut bytes = mint();
        put(&mut bytes, MINT_INITIALIZED_OFFSET, &[2]);
        assert_eq!(Mint::parse(&bytes), Err(Error::InvalidBoolean));

        for offset in [
            ACCOUNT_DELEGATE_OFFSET,
            ACCOUNT_NATIVE_OFFSET,
            ACCOUNT_CLOSE_AUTHORITY_OFFSET,
        ] {
            let mut bytes = account(OWNER_KEY, 1);
            put(&mut bytes, offset, &[2, 0, 0, 0]);
            assert_eq!(TokenAccount::parse(&bytes), Err(Error::InvalidOptionTag));
        }
        for state in [3, 255] {
            let mut bytes = account(OWNER_KEY, 1);
            put(&mut bytes, ACCOUNT_STATE_OFFSET, &[state]);
            assert_eq!(TokenAccount::parse(&bytes), Err(Error::InvalidAccountState));
        }
    }

    #[test]
    fn none_option_body_is_ignored_as_the_official_abi_requires() {
        let mut bytes = account(OWNER_KEY, 1);
        put(&mut bytes, ACCOUNT_DELEGATE_OFFSET + 4, &[0xa5; 32]);
        put(&mut bytes, ACCOUNT_NATIVE_OFFSET + 4, &[0x5a; 8]);
        put(&mut bytes, ACCOUNT_CLOSE_AUTHORITY_OFFSET + 4, &[0x33; 32]);
        let parsed = TokenAccount::parse(&bytes);
        assert!(parsed.is_ok());
        if let Ok(value) = parsed {
            assert_eq!(value.delegate, COption::None);
            assert_eq!(value.native_reserve, COption::None);
            assert_eq!(value.close_authority, COption::None);
        }
    }
}
