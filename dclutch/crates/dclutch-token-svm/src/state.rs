//! Exact SPL Token Mint and base Account state parsing.

use core::convert::TryInto;

use crate::{
    Address, Error, Result,
    tlv::{
        ACCOUNT_ACCOUNT_TYPE, ACCOUNT_TYPE_OFFSET, IMMUTABLE_OWNER_EXTENSION, TLV_HEADER_BYTES,
        TLV_START_OFFSET, TlvCursor, require_extension,
    },
};

/// Exact SPL Token Mint base width.
pub const MINT_BYTES: usize = 82;
/// Exact SPL Token Account base width.
pub const ACCOUNT_BYTES: usize = 165;

/// Exact width of a Token-2022 account carrying only `ImmutableOwner`.
///
/// The base account, the account-type discriminant that follows it, and one
/// empty TLV entry: 165 + 1 + 4. This is what the Associated Token Account
/// program produces under Token-2022 for EVERY wallet, so it is the width a
/// stranger's payout destination actually has.
pub const IMMUTABLE_OWNER_ACCOUNT_BYTES: usize = TLV_START_OFFSET + TLV_HEADER_BYTES;

/// The exact bytes the ATA program appends to a base account under Token-2022.
///
/// The account-type discriminant, then `ImmutableOwner` at length zero. Spelled
/// from the named constants rather than typed, and pinned to
/// [`TokenAccount::parse_base_or_immutable_owner`] by that function's own
/// tests, so a producer staging this shape and the parser admitting it cannot
/// drift apart.
pub const IMMUTABLE_OWNER_ACCOUNT_SUFFIX: [u8; IMMUTABLE_OWNER_ACCOUNT_BYTES - ACCOUNT_BYTES] = {
    let extension = IMMUTABLE_OWNER_EXTENSION.to_le_bytes();
    let length = 0_u16.to_le_bytes();
    [
        ACCOUNT_ACCOUNT_TYPE,
        extension[0],
        extension[1],
        length[0],
        length[1],
    ]
};

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

/// Canonical base-token Mint byte coordinates.
///
/// The Mint half of [`TokenAccountLayoutV1`], and public for the same reason:
/// an adversarial fixture that stages a Mint-side refusal must name the field
/// it corrupts, not a number it typed. `IS_INITIALIZED` is the coordinate
/// [`ExactTransferProfileV1::check_mint`] reads, so a fixture that clears it
/// stages exactly `Error::MintUninitialized` and nothing else.
///
/// [`ExactTransferProfileV1::check_mint`]: crate::ExactTransferProfileV1::check_mint
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintLayoutV1;

impl MintLayoutV1 {
    /// Four-byte mint-authority option tag followed by its 32-byte body.
    pub const AUTHORITY: usize = MINT_AUTHORITY_OFFSET;
    /// Total minted supply.
    pub const SUPPLY: usize = MINT_SUPPLY_OFFSET;
    /// Base-unit exponent.
    pub const DECIMALS: usize = MINT_DECIMALS_OFFSET;
    /// Mint lifecycle byte: exactly one initialized Mint, zero otherwise.
    pub const IS_INITIALIZED: usize = MINT_INITIALIZED_OFFSET;
    /// Four-byte freeze-authority option tag followed by its 32-byte body.
    pub const FREEZE_AUTHORITY: usize = MINT_FREEZE_AUTHORITY_OFFSET;
}

/// Canonical base-token Account byte coordinates.
///
/// This layout is the single public owner used by adapters and adversarial
/// real-SBF fixtures that must name an exact field without restating SPL's
/// packed offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountLayoutV1;

impl TokenAccountLayoutV1 {
    /// Mint identity.
    pub const MINT: usize = ACCOUNT_MINT_OFFSET;
    /// Token authority.
    pub const OWNER: usize = ACCOUNT_OWNER_OFFSET;
    /// Raw token amount.
    pub const AMOUNT: usize = ACCOUNT_AMOUNT_OFFSET;
    /// Four-byte delegate option tag followed by its 32-byte body.
    pub const DELEGATE: usize = ACCOUNT_DELEGATE_OFFSET;
    /// Account lifecycle state byte.
    pub const STATE: usize = ACCOUNT_STATE_OFFSET;
    /// Four-byte native-reserve option tag followed by its eight-byte body.
    pub const NATIVE_RESERVE: usize = ACCOUNT_NATIVE_OFFSET;
    /// Remaining delegated amount.
    pub const DELEGATED_AMOUNT: usize = ACCOUNT_DELEGATED_AMOUNT_OFFSET;
    /// Four-byte close-authority option tag followed by its 32-byte body.
    pub const CLOSE_AUTHORITY: usize = ACCOUNT_CLOSE_AUTHORITY_OFFSET;
}

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
    /// Encode the canonical freshly initialized 165-byte base token Account.
    ///
    /// This is the exact poststate produced by `InitializeAccount3`: the
    /// supplied mint and owner, zero amount, `Initialized`, and no delegate,
    /// native reserve, or close authority. The output is hostile-decoded again
    /// before it is returned so callers share the same total layout owner as
    /// [`TokenAccount::parse`].
    pub fn initialized_base_bytes(mint: Address, owner: Address) -> Result<[u8; ACCOUNT_BYTES]> {
        let mut output = [0; ACCOUNT_BYTES];
        write_exact(&mut output, ACCOUNT_MINT_OFFSET, &mint)?;
        write_exact(&mut output, ACCOUNT_OWNER_OFFSET, &owner)?;
        write_exact(
            &mut output,
            ACCOUNT_STATE_OFFSET,
            &[AccountState::Initialized as u8],
        )?;
        Self::parse(&output)?;
        Ok(output)
    }

    /// Parse a base account, or the exact account the ATA program creates.
    ///
    /// THE CONTRADICTION THIS RESOLVES. The wallet-terminal input operator
    /// documents the owner's associated token account as "the conventional
    /// destination for a payout" and derives it. Under Token-2022 the ATA
    /// program ALWAYS adds `ImmutableOwner` -- it is not optional and no caller
    /// chooses it -- so that account is 170 bytes and [`TokenAccount::parse`],
    /// which refuses every extension suffix by design, refuses it. Measured on
    /// cohort-13, on chain: the founder's ATA was created, read back at 170
    /// bytes, and the payout refused it. "The conventional destination" and
    /// "every extension suffix refuses" cannot both stand, and the one that has
    /// to give is the refusal, because the alternative is that no ordinary
    /// wallet can ever be paid.
    ///
    /// WHY THIS ONE EXTENSION AND NO OTHER. `ImmutableOwner` says the token
    /// program will refuse `SetAuthority(AccountOwner)` on this account. Every
    /// check a payout makes against a destination -- its mint, its owner, its
    /// initialized state -- is STRENGTHENED by that, because the owner it
    /// authenticated cannot afterwards be changed. The base layout offers no
    /// such guarantee. No other extension has that property: a transfer hook, a
    /// transfer fee, a confidential balance or a CPI guard all change what a
    /// transfer MEANS, and each would have to be reasoned about on its own.
    /// They stay refused, and the width check is what refuses them -- 170 bytes
    /// admits exactly one empty TLV entry and the type check pins which one.
    pub fn parse_base_or_immutable_owner(bytes: &[u8]) -> Result<Self> {
        if bytes.len() == ACCOUNT_BYTES {
            return Self::parse(bytes);
        }
        if bytes.len() != IMMUTABLE_OWNER_ACCOUNT_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_byte(bytes, ACCOUNT_TYPE_OFFSET)? != ACCOUNT_ACCOUNT_TYPE {
            return Err(Error::InvalidExtensionLayout);
        }
        let mut cursor = TlvCursor::new(
            bytes
                .get(TLV_START_OFFSET..)
                .ok_or(Error::InvalidExtensionLayout)?,
        );
        let entry = cursor.next()?.ok_or(Error::InvalidExtensionLayout)?;
        require_extension(entry, IMMUTABLE_OWNER_EXTENSION, 0)?;
        if cursor.next()?.is_some() {
            return Err(Error::InvalidExtensionLayout);
        }
        Self::parse(
            bytes
                .get(..ACCOUNT_BYTES)
                .ok_or(Error::InvalidExtensionLayout)?,
        )
    }

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

    /// Project the exact base-account bytes after a balance-only transfer.
    ///
    /// Every byte except the amount field is copied from the hostile-decoded
    /// prestate. This mirrors the official base-account packer and gives
    /// callers one layout owner for exact post-CPI joins.
    pub fn project_amount_poststate(
        prestate: &[u8],
        amount_after: u64,
    ) -> Result<[u8; ACCOUNT_BYTES]> {
        Self::parse(prestate)?;
        let mut output: [u8; ACCOUNT_BYTES] =
            prestate.try_into().map_err(|_| Error::InvalidLength)?;
        write_exact(
            &mut output,
            ACCOUNT_AMOUNT_OFFSET,
            &amount_after.to_le_bytes(),
        )?;
        // The hostile decoder admitted every byte copied above. Replacing one
        // fixed-width `u64` cannot invalidate any tag, state, identity, or
        // account width, so a second complete decode would restate no check.
        Ok(output)
    }

    /// Project exact base-account bytes after a delegated source transfer.
    ///
    /// The official token packer writes the four-byte option tag and writes
    /// the body only for `Some`; a terminal `None` therefore preserves the
    /// old body bytes while clearing the tag. All unrelated bytes are copied
    /// exactly from the prestate.
    pub fn project_delegated_source_poststate(
        prestate: &[u8],
        amount_after: u64,
        delegate_after: COption<Address>,
        delegated_amount_after: u64,
    ) -> Result<[u8; ACCOUNT_BYTES]> {
        let mut output = Self::project_amount_poststate(prestate, amount_after)?;
        match delegate_after {
            COption::None => {
                write_exact(&mut output, ACCOUNT_DELEGATE_OFFSET, &[0; 4])?;
            }
            COption::Some(delegate) => {
                write_exact(&mut output, ACCOUNT_DELEGATE_OFFSET, &[1, 0, 0, 0])?;
                write_exact(&mut output, ACCOUNT_DELEGATE_OFFSET + 4, &delegate)?;
            }
        }
        write_exact(
            &mut output,
            ACCOUNT_DELEGATED_AMOUNT_OFFSET,
            &delegated_amount_after.to_le_bytes(),
        )?;
        // `project_amount_poststate` hostile-decoded the copied prestate and
        // every write above is a canonical typed option/`u64` encoding.
        Ok(output)
    }
}

fn write_exact(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = checked_add(offset, input.len())?;
    let destination = output.get_mut(offset..end).ok_or(Error::InvalidLength)?;
    destination.copy_from_slice(input);
    Ok(())
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
mod immutable_owner_tests {
    use super::fixtures::{MINT_KEY, OWNER_KEY, account};
    use super::*;
    use crate::tlv::put_tlv;
    use std::vec::Vec;

    /// Cohort-13's founder ATA `4BENW7YgFnAbagoRr8rAdD8p2kqhwiyvtyDzNKj8ef6W`,
    /// read off devnet at finalized commitment.
    ///
    /// Not a fixture shaped like an ATA: the bytes the Associated Token Account
    /// program itself wrote, for a real wallet, on the cluster this protocol
    /// runs on. Its last five bytes are the whole finding -- account type `2`,
    /// then extension `7` at length `0` -- and they are why the payout refused
    /// it and why a 165-byte auxiliary account had to be created by hand.
    const DEVNET_FOUNDER_ATA_V1: [u8; IMMUTABLE_OWNER_ACCOUNT_BYTES] = [
        0xcc, 0x23, 0xee, 0x31, 0x28, 0xc7, 0x38, 0x13, 0x07, 0x1e, 0xba, 0x3e, 0x6d, 0x84, 0x68,
        0x6f, 0xe0, 0x02, 0x09, 0x39, 0x73, 0xe7, 0x07, 0x26, 0x22, 0x39, 0xca, 0xbd, 0x45, 0x8e,
        0xaf, 0x14, 0xd2, 0xb7, 0x0b, 0x80, 0xfb, 0x1b, 0x46, 0x47, 0x55, 0x97, 0xf0, 0xdb, 0x11,
        0xe8, 0x7d, 0xb4, 0x61, 0x27, 0xb8, 0x1c, 0x0e, 0xf5, 0x19, 0xf8, 0x1e, 0x38, 0x48, 0x1a,
        0x60, 0x98, 0xa0, 0x97, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x07, 0x00, 0x00, 0x00,
    ];

    /// One base account with an arbitrary extension suffix appended.
    fn suffixed(account_type: u8, extension_type: u16, value: &[u8]) -> Vec<u8> {
        let mut bytes = account(OWNER_KEY, 7).to_vec();
        bytes.push(account_type);
        put_tlv(&mut bytes, extension_type, value);
        bytes
    }

    #[test]
    fn the_real_devnet_ata_is_admitted_and_its_base_fields_survive_the_suffix() {
        assert_eq!(DEVNET_FOUNDER_ATA_V1.len(), 170);
        // The three bytes that made it unpayable, named rather than implied.
        assert_eq!(
            DEVNET_FOUNDER_ATA_V1[ACCOUNT_TYPE_OFFSET],
            ACCOUNT_ACCOUNT_TYPE
        );
        assert_eq!(
            u16::from_le_bytes([DEVNET_FOUNDER_ATA_V1[166], DEVNET_FOUNDER_ATA_V1[167]]),
            IMMUTABLE_OWNER_EXTENSION,
        );
        assert_eq!(
            u16::from_le_bytes([DEVNET_FOUNDER_ATA_V1[168], DEVNET_FOUNDER_ATA_V1[169]]),
            0,
        );

        assert_eq!(
            TokenAccount::parse(&DEVNET_FOUNDER_ATA_V1),
            Err(Error::InvalidLength),
            "the base parser still refuses it; the admission is a separate function",
        );
        let parsed = TokenAccount::parse_base_or_immutable_owner(&DEVNET_FOUNDER_ATA_V1)
            .expect("the ATA program's own output is admitted");
        assert_eq!(parsed.state, AccountState::Initialized);
        assert_eq!(parsed.amount, 0);
        assert!(parsed.delegate.is_none());
        assert!(parsed.close_authority.is_none());
        assert!(parsed.native_reserve.is_none());
        // Read from the same bytes by hand, so the parser is not its own witness.
        assert_eq!(parsed.mint.as_slice(), &DEVNET_FOUNDER_ATA_V1[0..32]);
        assert_eq!(parsed.owner.as_slice(), &DEVNET_FOUNDER_ATA_V1[32..64]);
    }

    #[test]
    fn the_published_suffix_is_the_one_the_ata_program_wrote_and_the_one_admitted() {
        assert_eq!(
            DEVNET_FOUNDER_ATA_V1[ACCOUNT_BYTES..],
            IMMUTABLE_OWNER_ACCOUNT_SUFFIX,
            "the constant and the chain agree byte for byte",
        );
        let mut staged = account(OWNER_KEY, 3).to_vec();
        staged.extend_from_slice(&IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&staged)
                .expect("the published suffix is admitted")
                .amount,
            3,
        );
    }

    #[test]
    fn a_base_account_still_parses_through_the_admitting_function() {
        let bytes = account(OWNER_KEY, 41);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&bytes),
            TokenAccount::parse(&bytes),
        );
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&bytes)
                .expect("base account")
                .mint,
            MINT_KEY,
        );
    }

    #[test]
    fn every_other_extension_suffix_is_still_refused() {
        // The four that would each change what a transfer MEANS, at the exact
        // width `ImmutableOwner` occupies where their own value is empty. Only
        // the type distinguishes them, so only the type check can refuse them.
        for extension_type in [
            1_u16, // TransferFeeConfig
            5,     // NonTransferable
            11,    // CpiGuard
            14,    // TransferHookAccount
            6,     // ImmutableOwner is 7; 6 is its neighbour and is not it
            8,     // MemoTransfer
        ] {
            let bytes = suffixed(ACCOUNT_ACCOUNT_TYPE, extension_type, &[]);
            assert_eq!(bytes.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES);
            assert_eq!(
                TokenAccount::parse_base_or_immutable_owner(&bytes),
                Err(Error::InvalidExtensionLayout),
                "extension {extension_type} must not be admitted",
            );
        }
    }

    #[test]
    fn the_admission_is_pinned_to_one_width_one_account_type_and_one_entry() {
        // A Mint's account-type discriminant with ImmutableOwner's bytes.
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&suffixed(
                1,
                IMMUTABLE_OWNER_EXTENSION,
                &[]
            )),
            Err(Error::InvalidExtensionLayout),
        );
        // ImmutableOwner with a nonempty value: the right type at the wrong width.
        let wide = suffixed(ACCOUNT_ACCOUNT_TYPE, IMMUTABLE_OWNER_EXTENSION, &[0]);
        assert_eq!(wide.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES + 1);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&wide),
            Err(Error::InvalidLength),
        );
        // ImmutableOwner followed by a second entry.
        let mut two = suffixed(ACCOUNT_ACCOUNT_TYPE, IMMUTABLE_OWNER_EXTENSION, &[]);
        put_tlv(&mut two, 11, &[0]);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&two),
            Err(Error::InvalidLength),
        );
        // One byte of the base account cut away, with the suffix intact.
        let mut short = DEVNET_FOUNDER_ATA_V1.to_vec();
        short.remove(0);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&short),
            Err(Error::InvalidLength),
        );
        // The base account with the account-type byte and nothing after it.
        let mut typed = account(OWNER_KEY, 1).to_vec();
        typed.push(ACCOUNT_ACCOUNT_TYPE);
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&typed),
            Err(Error::InvalidLength),
        );
    }

    #[test]
    fn the_admitted_suffix_does_not_launder_a_corrupt_base_account() {
        // The extension says the OWNER cannot change. It says nothing about the
        // lifecycle byte, and admitting it must not skip any base check.
        let mut bytes = DEVNET_FOUNDER_ATA_V1;
        bytes[ACCOUNT_STATE_OFFSET] = 9;
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&bytes),
            Err(Error::InvalidAccountState),
        );
        let mut tagged = DEVNET_FOUNDER_ATA_V1;
        tagged[ACCOUNT_DELEGATE_OFFSET] = 2;
        assert_eq!(
            TokenAccount::parse_base_or_immutable_owner(&tagged),
            Err(Error::InvalidOptionTag),
        );
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

    #[test]
    fn exact_transfer_projection_preserves_every_unrelated_byte() {
        let mut bytes = account(OWNER_KEY, 100);
        put(&mut bytes, ACCOUNT_DELEGATE_OFFSET, &[1, 0, 0, 0]);
        put(&mut bytes, ACCOUNT_DELEGATE_OFFSET + 4, &[5; 32]);
        put(
            &mut bytes,
            ACCOUNT_DELEGATED_AMOUNT_OFFSET,
            &40_u64.to_le_bytes(),
        );
        put(&mut bytes, ACCOUNT_NATIVE_OFFSET + 4, &[0x5a; 8]);
        put(&mut bytes, ACCOUNT_CLOSE_AUTHORITY_OFFSET + 4, &[0x33; 32]);

        let destination =
            TokenAccount::project_amount_poststate(&bytes, 130).expect("balance-only projection");
        let mut expected_destination = bytes;
        put(
            &mut expected_destination,
            ACCOUNT_AMOUNT_OFFSET,
            &130_u64.to_le_bytes(),
        );
        assert_eq!(destination, expected_destination);

        let source = TokenAccount::project_delegated_source_poststate(&bytes, 70, COption::None, 0)
            .expect("terminal delegated projection");
        let mut expected_source = bytes;
        put(
            &mut expected_source,
            ACCOUNT_AMOUNT_OFFSET,
            &70_u64.to_le_bytes(),
        );
        put(&mut expected_source, ACCOUNT_DELEGATE_OFFSET, &[0; 4]);
        put(
            &mut expected_source,
            ACCOUNT_DELEGATED_AMOUNT_OFFSET,
            &0_u64.to_le_bytes(),
        );
        assert_eq!(source, expected_source);
        assert_eq!(
            TokenAccount::parse(&source).map(|value| value.delegate),
            Ok(COption::None)
        );
    }

    #[test]
    fn initialized_base_bytes_round_trip_with_nonzero_identities() {
        let mint = [0xa5; 32];
        let owner = [0x5a; 32];
        let bytes = TokenAccount::initialized_base_bytes(mint, owner)
            .expect("canonical initialized base account");
        assert_eq!(bytes.len(), ACCOUNT_BYTES);
        assert_eq!(
            TokenAccount::parse(&bytes),
            Ok(TokenAccount {
                mint,
                owner,
                amount: 0,
                delegate: COption::None,
                state: AccountState::Initialized,
                native_reserve: COption::None,
                delegated_amount: 0,
                close_authority: COption::None,
            })
        );
        assert!(bytes[ACCOUNT_AMOUNT_OFFSET..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn initialized_base_bytes_has_no_hidden_option_or_authority_state() {
        let bytes = TokenAccount::initialized_base_bytes([1; 32], [2; 32])
            .expect("canonical initialized base account");
        let mut expected = [0; ACCOUNT_BYTES];
        put(&mut expected, ACCOUNT_MINT_OFFSET, &[1; 32]);
        put(&mut expected, ACCOUNT_OWNER_OFFSET, &[2; 32]);
        put(
            &mut expected,
            ACCOUNT_STATE_OFFSET,
            &[AccountState::Initialized as u8],
        );
        assert_eq!(bytes, expected);

        let mut hostile = bytes;
        put(&mut hostile, ACCOUNT_CLOSE_AUTHORITY_OFFSET, &[2, 0, 0, 0]);
        assert_eq!(TokenAccount::parse(&hostile), Err(Error::InvalidOptionTag));
    }
}
