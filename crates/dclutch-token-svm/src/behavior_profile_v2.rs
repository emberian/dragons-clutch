//! Immutable Token-2022 behavior profile with display-only decimals and
//! optional immutable, self-hosted metadata.
//!
//! Every economic quantity remains a raw `u64` base-unit integer. The Mint's
//! `decimals` byte is returned only as display metadata and is never used for
//! arithmetic. The complete `u8` domain is admitted; this is the bound imposed
//! by the authenticated Token-2022 Mint layout, not a protocol quantity bound.
//!
//! Claim Mints retain the two protocol lifecycle extensions used by Bearer and
//! Structured representations: `MintCloseAuthority` and `PermissionedBurn`.
//! The sole optional extension pair is an immutable `MetadataPointer` pointing
//! to the Mint itself plus an immutable, well-formed `TokenMetadata` value for
//! that Mint. Token Accounts remain exact base Accounts. Consequently transfer
//! fees, interest/scaled conversion, hooks, confidential state, permanent
//! delegates, default-frozen/non-transferable behavior, pausing, account-side
//! extensions, mutable metadata and unknown extensions are all refused.

use core::{convert::TryInto, str};

use crate::{
    Address, COption, Error, MINT_BYTES, Mint, Result, TOKEN_2022_PROGRAM_ID, TokenAccount,
    state::AccountState,
};

/// Canonical semantic preimage for the first lifted Token-2022 behavior
/// profile. Its digest is [`TOKEN_2022_BEHAVIOR_PROFILE_ID_V2`].
pub const TOKEN_2022_BEHAVIOR_PROFILE_PREIMAGE_V2: &[u8] = b"dclutch/token-behavior-profile/v2|program=token2022|base-amount=u64|display-decimals=0..255-no-conversion|mint-required=MintCloseAuthority+PermissionedBurn|mint-optional=immutable-self-MetadataPointer+immutable-self-TokenMetadata|account-extensions=none|authority=protocol-controller|freeze=none|instructions=initialize,mint-to-checked,transfer-checked,permissioned-burn-checked,close|refuse=transfer-fee,interest,scaled-ui,hook,confidential,permanent-delegate,default-frozen,nontransferable,pausable,unknown,mutable-metadata";

/// SHA-256 of [`TOKEN_2022_BEHAVIOR_PROFILE_PREIMAGE_V2`].
pub const TOKEN_2022_BEHAVIOR_PROFILE_ID_V2: Address = [
    0x12, 0x39, 0x3c, 0xc7, 0x3a, 0xb2, 0x58, 0xc7, 0x46, 0xa4, 0xa1, 0x85, 0xa4, 0x76, 0x06, 0x95,
    0x68, 0x41, 0xb4, 0xce, 0x0d, 0x53, 0xb3, 0xaa, 0x04, 0xc7, 0xe6, 0x14, 0xd4, 0x38, 0x14, 0x62,
];

/// Largest display-decimal value encoded by the Token-2022 Mint ABI.
///
/// This chain-format bound is deliberately the full `u8` domain. No dClutch
/// economic operation raises ten to this power or converts through UI units.
pub const MAX_DISPLAY_DECIMALS_V2: u8 = u8::MAX;

/// Largest admitted `TokenMetadata` value.
///
/// The bound is mathematical: one Token-2022 TLV length is an unsigned
/// 16-bit integer. Parsing is allocation-free and linear in this authenticated
/// slice.
pub const MAX_INERT_METADATA_VALUE_BYTES_V2: usize = 65_535;

const BASE_ACCOUNT_BYTES: usize = 165;
const ACCOUNT_TYPE_OFFSET: usize = BASE_ACCOUNT_BYTES;
const TLV_START_OFFSET: usize = 166;
const MINT_ACCOUNT_TYPE: u8 = 1;

const MINT_CLOSE_AUTHORITY_EXTENSION: u16 = 3;
const METADATA_POINTER_EXTENSION: u16 = 18;
const TOKEN_METADATA_EXTENSION: u16 = 19;
const PERMISSIONED_BURN_EXTENSION: u16 = 28;
const AUTHORITY_EXTENSION_BYTES: usize = 32;
const METADATA_POINTER_BYTES: usize = 64;
const TOKEN_METADATA_FIXED_BYTES: usize = 80;

/// Whether the admitted Mint carries the optional immutable metadata pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InertMetadataV2 {
    /// The Mint contains only the two required lifecycle extensions.
    Absent,
    /// The Mint contains an immutable self-pointer and immutable, well-formed
    /// TokenMetadata whose embedded Mint equals the authenticated Mint key.
    ImmutableSelfHosted,
}

/// Authenticated facts from one V2 behavior-profile Mint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022BehaviorMintFactsV2 {
    mint: Mint,
    controller: Address,
    metadata: InertMetadataV2,
}

impl Token2022BehaviorMintFactsV2 {
    /// Return the exact Token-owned base Mint state.
    pub const fn mint(self) -> Mint {
        self.mint
    }

    /// Return the protocol controller bound identically as Mint, close and
    /// permissioned-burn authority.
    pub const fn controller(self) -> Address {
        self.controller
    }

    /// Return the admitted inert-metadata shape.
    pub const fn metadata(self) -> InertMetadataV2 {
        self.metadata
    }

    /// Return the display-only decimals byte. Economic callers must continue
    /// using [`Self::base_supply`] and other raw base-unit quantities.
    pub const fn display_decimals(self) -> u8 {
        self.mint.decimals
    }

    /// Return the exact raw base-unit Mint supply.
    pub const fn base_supply(self) -> u64 {
        self.mint.supply
    }
}

/// Authenticated facts from one extension-free holder Token Account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022BehaviorAccountFactsV2 {
    account: TokenAccount,
}

impl Token2022BehaviorAccountFactsV2 {
    /// Return the exact Token-owned Account state, including its raw base-unit
    /// balance.
    pub const fn account(self) -> TokenAccount {
        self.account
    }

    /// Return the exact raw base-unit balance.
    pub const fn base_amount(self) -> u64 {
        self.account.amount
    }
}

/// Immutable Token-2022 behavior semantics selected by profile identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022BehaviorProfileV2;

impl Token2022BehaviorProfileV2 {
    /// Return the immutable profile content identity.
    pub const fn profile_id() -> Address {
        TOKEN_2022_BEHAVIOR_PROFILE_ID_V2
    }

    /// Authenticate one representation Mint against the complete V2 behavior
    /// profile.
    pub fn check_mint(
        program_id: Address,
        mint_key: Address,
        mint_data: &[u8],
        expected_controller: Address,
        expected_base_supply: u64,
    ) -> Result<Token2022BehaviorMintFactsV2> {
        if program_id != TOKEN_2022_PROGRAM_ID {
            return Err(Error::ProfileProgramMismatch);
        }
        if mint_key == [0; 32] || expected_controller == [0; 32] {
            return Err(Error::AuthorityMismatch);
        }
        if mint_data.len() < TLV_START_OFFSET {
            return Err(Error::InvalidLength);
        }
        let base = Mint::parse(
            mint_data
                .get(..MINT_BYTES)
                .ok_or(Error::InvalidExtensionLayout)?,
        )?;
        if mint_data
            .get(MINT_BYTES..BASE_ACCOUNT_BYTES)
            .ok_or(Error::InvalidExtensionLayout)?
            .iter()
            .any(|byte| *byte != 0)
            || mint_data.get(ACCOUNT_TYPE_OFFSET).copied() != Some(MINT_ACCOUNT_TYPE)
        {
            return Err(Error::InvalidExtensionLayout);
        }
        if !base.is_initialized {
            return Err(Error::MintUninitialized);
        }
        if base.mint_authority != COption::Some(expected_controller) {
            return Err(Error::AuthorityMismatch);
        }
        if base.freeze_authority != COption::None {
            return Err(Error::FreezeAuthorityPresent);
        }
        if base.supply != expected_base_supply {
            return Err(Error::MintSupplyMismatch);
        }

        let mut cursor = TlvCursor::new(
            mint_data
                .get(TLV_START_OFFSET..)
                .ok_or(Error::InvalidExtensionLayout)?,
        );
        let mut close_seen = false;
        let mut burn_seen = false;
        let mut pointer_seen = false;
        let mut metadata_seen = false;
        while let Some(entry) = cursor.next()? {
            match entry.extension_type {
                MINT_CLOSE_AUTHORITY_EXTENSION if !close_seen => {
                    require_extension(
                        entry,
                        MINT_CLOSE_AUTHORITY_EXTENSION,
                        AUTHORITY_EXTENSION_BYTES,
                    )?;
                    require_key(entry.value, expected_controller)?;
                    close_seen = true;
                }
                PERMISSIONED_BURN_EXTENSION if !burn_seen => {
                    require_extension(
                        entry,
                        PERMISSIONED_BURN_EXTENSION,
                        AUTHORITY_EXTENSION_BYTES,
                    )?;
                    require_key(entry.value, expected_controller)?;
                    burn_seen = true;
                }
                METADATA_POINTER_EXTENSION if !pointer_seen => {
                    require_extension(entry, METADATA_POINTER_EXTENSION, METADATA_POINTER_BYTES)?;
                    require_immutable_self_pointer(entry.value, mint_key)?;
                    pointer_seen = true;
                }
                TOKEN_METADATA_EXTENSION if !metadata_seen => {
                    if entry.value.len() > MAX_INERT_METADATA_VALUE_BYTES_V2 {
                        return Err(Error::InvalidExtensionLayout);
                    }
                    require_extension_at_least(
                        entry,
                        TOKEN_METADATA_EXTENSION,
                        TOKEN_METADATA_FIXED_BYTES,
                    )?;
                    validate_immutable_token_metadata(entry.value, mint_key)?;
                    metadata_seen = true;
                }
                _ => return Err(Error::InvalidExtensionLayout),
            }
        }
        if !close_seen || !burn_seen || pointer_seen != metadata_seen {
            return Err(Error::InvalidExtensionLayout);
        }
        let metadata = if metadata_seen {
            InertMetadataV2::ImmutableSelfHosted
        } else {
            InertMetadataV2::Absent
        };

        Ok(Token2022BehaviorMintFactsV2 {
            mint: base,
            controller: expected_controller,
            metadata,
        })
    }

    /// Authenticate one holder/custody Token Account. V2 deliberately admits
    /// no Account extensions because even apparently convenient Account
    /// extensions can change transfer, authority, freeze or close behavior.
    pub fn check_account(
        program_id: Address,
        account_data: &[u8],
        expected_mint: Address,
        expected_owner: Address,
        minimum_base_amount: u64,
    ) -> Result<Token2022BehaviorAccountFactsV2> {
        if program_id != TOKEN_2022_PROGRAM_ID {
            return Err(Error::ProfileProgramMismatch);
        }
        if account_data.len() != BASE_ACCOUNT_BYTES {
            return Err(Error::InvalidLength);
        }
        if expected_mint == [0; 32] || expected_owner == [0; 32] {
            return Err(Error::AuthorityMismatch);
        }
        let account = TokenAccount::parse(account_data)?;
        if account.mint != expected_mint {
            return Err(Error::MintMismatch);
        }
        if account.owner != expected_owner {
            return Err(Error::AuthorityMismatch);
        }
        if account.state != AccountState::Initialized {
            return match account.state {
                AccountState::Uninitialized => Err(Error::AccountUninitialized),
                AccountState::Initialized => Ok(Token2022BehaviorAccountFactsV2 { account }),
                AccountState::Frozen => Err(Error::AccountFrozen),
            };
        }
        if !account.native_reserve.is_none() {
            return Err(Error::NativeAccount);
        }
        if !account.delegate.is_none() || account.delegated_amount != 0 {
            return Err(Error::DelegatePresent);
        }
        if !account.close_authority.is_none() {
            return Err(Error::CloseAuthorityPresent);
        }
        if account.amount < minimum_base_amount {
            return Err(Error::InsufficientFunds);
        }
        Ok(Token2022BehaviorAccountFactsV2 { account })
    }
}

#[derive(Clone, Copy)]
struct TlvEntry<'a> {
    extension_type: u16,
    value: &'a [u8],
}

struct TlvCursor<'a> {
    remaining: &'a [u8],
}

impl<'a> TlvCursor<'a> {
    const fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }

    fn next(&mut self) -> Result<Option<TlvEntry<'a>>> {
        if self.remaining.is_empty() {
            return Ok(None);
        }
        let extension_type = read_u16(self.remaining, 0)?;
        let length = usize::from(read_u16(self.remaining, 2)?);
        if extension_type == 0 {
            return Err(Error::InvalidExtensionLayout);
        }
        let end = 4usize
            .checked_add(length)
            .ok_or(Error::InvalidExtensionLayout)?;
        let value = self
            .remaining
            .get(4..end)
            .ok_or(Error::InvalidExtensionLayout)?;
        self.remaining = self
            .remaining
            .get(end..)
            .ok_or(Error::InvalidExtensionLayout)?;
        Ok(Some(TlvEntry {
            extension_type,
            value,
        }))
    }
}

fn require_extension(entry: TlvEntry<'_>, extension_type: u16, length: usize) -> Result<()> {
    if entry.extension_type != extension_type || entry.value.len() != length {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

fn require_extension_at_least(
    entry: TlvEntry<'_>,
    extension_type: u16,
    minimum_length: usize,
) -> Result<()> {
    if entry.extension_type != extension_type || entry.value.len() < minimum_length {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

fn require_key(bytes: &[u8], expected: Address) -> Result<()> {
    let observed: Address = bytes
        .try_into()
        .map_err(|_| Error::InvalidExtensionLayout)?;
    if observed != expected {
        return Err(Error::AuthorityMismatch);
    }
    Ok(())
}

fn require_immutable_self_pointer(bytes: &[u8], mint_key: Address) -> Result<()> {
    let authority = bytes.get(..32).ok_or(Error::InvalidExtensionLayout)?;
    let address = bytes.get(32..64).ok_or(Error::InvalidExtensionLayout)?;
    if authority != [0; 32] || address != mint_key {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

fn validate_immutable_token_metadata(bytes: &[u8], mint_key: Address) -> Result<()> {
    if bytes.get(..32) != Some(&[0; 32]) || bytes.get(32..64) != Some(mint_key.as_slice()) {
        return Err(Error::InvalidExtensionLayout);
    }
    let mut offset = 64usize;
    for _ in 0..3 {
        offset = consume_borsh_string(bytes, offset)?;
    }
    let pair_count =
        usize::try_from(read_u32(bytes, offset)?).map_err(|_| Error::InvalidExtensionLayout)?;
    offset = offset.checked_add(4).ok_or(Error::InvalidExtensionLayout)?;
    // Each pair needs two four-byte lengths. This proves an inexpensive upper
    // bound before the loop and prevents a hostile count from driving work.
    if pair_count > bytes.len().saturating_sub(offset) / 8 {
        return Err(Error::InvalidExtensionLayout);
    }
    for _ in 0..pair_count {
        offset = consume_borsh_string(bytes, offset)?;
        offset = consume_borsh_string(bytes, offset)?;
    }
    if offset != bytes.len() {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

fn consume_borsh_string(bytes: &[u8], offset: usize) -> Result<usize> {
    let length =
        usize::try_from(read_u32(bytes, offset)?).map_err(|_| Error::InvalidExtensionLayout)?;
    let start = offset.checked_add(4).ok_or(Error::InvalidExtensionLayout)?;
    let end = start
        .checked_add(length)
        .ok_or(Error::InvalidExtensionLayout)?;
    let value = bytes.get(start..end).ok_or(Error::InvalidExtensionLayout)?;
    str::from_utf8(value).map_err(|_| Error::InvalidExtensionLayout)?;
    Ok(end)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    bytes
        .get(offset..offset.checked_add(2).ok_or(Error::InvalidExtensionLayout)?)
        .ok_or(Error::InvalidExtensionLayout)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| Error::InvalidExtensionLayout)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    bytes
        .get(offset..offset.checked_add(4).ok_or(Error::InvalidExtensionLayout)?)
        .ok_or(Error::InvalidExtensionLayout)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| Error::InvalidExtensionLayout)
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use solana_address::Address as SolanaAddress;
    use solana_nullable::MaybeNull;
    use solana_program_option::COption as SolanaCOption;
    use spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensionsMut, StateWithExtensionsMut, metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority, permissioned_burn::PermissionedBurnConfig,
        },
        state::Mint as SplMint,
    };
    use spl_token_metadata_interface::state::TokenMetadata;

    use crate::{ACCOUNT_BYTES, state::fixtures::put};

    use super::*;

    const MINT_KEY: Address = [7; 32];
    const CONTROLLER: Address = [8; 32];
    const HOLDER: Address = [9; 32];

    fn put_tlv(output: &mut std::vec::Vec<u8>, extension_type: u16, value: &[u8]) {
        output.extend_from_slice(&extension_type.to_le_bytes());
        output.extend_from_slice(
            &u16::try_from(value.len())
                .expect("test TLV length")
                .to_le_bytes(),
        );
        output.extend_from_slice(value);
    }

    fn metadata_value() -> std::vec::Vec<u8> {
        let mut value = std::vec::Vec::new();
        value.extend_from_slice(&[0; 32]);
        value.extend_from_slice(&MINT_KEY);
        for string in [
            b"Claim".as_slice(),
            b"DCLM".as_slice(),
            b"https://example.invalid/7".as_slice(),
        ] {
            value.extend_from_slice(
                &u32::try_from(string.len())
                    .expect("test string length")
                    .to_le_bytes(),
            );
            value.extend_from_slice(string);
        }
        value.extend_from_slice(&1_u32.to_le_bytes());
        for string in [b"outcome".as_slice(), b"7".as_slice()] {
            value.extend_from_slice(
                &u32::try_from(string.len())
                    .expect("test string length")
                    .to_le_bytes(),
            );
            value.extend_from_slice(string);
        }
        value
    }

    fn mint(decimals: u8, with_metadata: bool) -> std::vec::Vec<u8> {
        let mut output = std::vec![0; TLV_START_OFFSET];
        put(&mut output, 0, &1_u32.to_le_bytes());
        put(&mut output, 4, &CONTROLLER);
        put(&mut output, 36, &11_u64.to_le_bytes());
        put(&mut output, 44, &[decimals]);
        put(&mut output, 45, &[1]);
        put(&mut output, ACCOUNT_TYPE_OFFSET, &[MINT_ACCOUNT_TYPE]);
        put_tlv(&mut output, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        if with_metadata {
            let mut pointer = [0; METADATA_POINTER_BYTES];
            put(&mut pointer, 32, &MINT_KEY);
            put_tlv(&mut output, METADATA_POINTER_EXTENSION, &pointer);
            put_tlv(&mut output, TOKEN_METADATA_EXTENSION, &metadata_value());
        }
        put_tlv(&mut output, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
        output
    }

    fn official_interface_mint(decimals: u8) -> std::vec::Vec<u8> {
        let mint_key = SolanaAddress::new_from_array(MINT_KEY);
        let controller = SolanaAddress::new_from_array(CONTROLLER);
        let metadata = TokenMetadata {
            update_authority: MaybeNull::default(),
            mint: mint_key,
            name: "Claim".into(),
            symbol: "DCLM".into(),
            uri: "https://example.invalid/7".into(),
            additional_metadata: std::vec![("outcome".into(), "7".into())],
        };
        let metadata_bytes = metadata_value().len();
        let mut output = std::vec![
            0;
            TLV_START_OFFSET
                + 4
                + AUTHORITY_EXTENSION_BYTES
                + 4
                + METADATA_POINTER_BYTES
                + 4
                + metadata_bytes
                + 4
                + AUTHORITY_EXTENSION_BYTES
        ];
        let mut state = StateWithExtensionsMut::<SplMint>::unpack_uninitialized(&mut output)
            .expect("official extension state");
        state.base = SplMint {
            mint_authority: SolanaCOption::Some(controller),
            supply: 11,
            decimals,
            is_initialized: true,
            freeze_authority: SolanaCOption::None,
        };
        let close = state
            .init_extension::<MintCloseAuthority>(false)
            .expect("close authority");
        close.close_authority = Some(controller).try_into().expect("nonzero controller");
        let pointer = state
            .init_extension::<MetadataPointer>(false)
            .expect("metadata pointer");
        pointer.authority = MaybeNull::default();
        pointer.metadata_address = Some(mint_key).try_into().expect("nonzero Mint");
        state
            .init_variable_len_extension(&metadata, false)
            .expect("token metadata");
        let burn = state
            .init_extension::<PermissionedBurnConfig>(false)
            .expect("permissioned burn");
        burn.authority = Some(controller).try_into().expect("nonzero controller");
        state.init_account_type().expect("Mint account type");
        state.pack_base();
        output
    }

    fn check(bytes: &[u8]) -> Result<Token2022BehaviorMintFactsV2> {
        Token2022BehaviorProfileV2::check_mint(
            TOKEN_2022_PROGRAM_ID,
            MINT_KEY,
            bytes,
            CONTROLLER,
            11,
        )
    }

    #[test]
    fn profile_identity_and_full_display_decimal_domain_are_stable() {
        assert_eq!(
            Sha256::digest(TOKEN_2022_BEHAVIOR_PROFILE_PREIMAGE_V2).as_slice(),
            TOKEN_2022_BEHAVIOR_PROFILE_ID_V2
        );
        for decimals in [0, 1, 6, 9, 18, 19, u8::MAX] {
            let facts = check(&mint(decimals, false)).expect("admitted decimals");
            assert_eq!(facts.display_decimals(), decimals);
            assert_eq!(facts.base_supply(), 11);
            assert_eq!(facts.metadata(), InertMetadataV2::Absent);
        }
    }

    #[test]
    fn immutable_self_metadata_is_admitted_and_fully_consumed() {
        let canonical = mint(9, true);
        let facts = check(&canonical).expect("immutable metadata");
        assert_eq!(facts.metadata(), InertMetadataV2::ImmutableSelfHosted);

        let mut mutable_pointer = canonical.clone();
        let pointer_value = TLV_START_OFFSET + 4 + AUTHORITY_EXTENSION_BYTES + 4;
        *mutable_pointer
            .get_mut(pointer_value)
            .expect("pointer authority") = 1;
        assert_eq!(check(&mutable_pointer), Err(Error::InvalidExtensionLayout));

        let mut external_pointer = canonical.clone();
        *external_pointer
            .get_mut(pointer_value + 32)
            .expect("pointer address") = 1;
        assert_eq!(check(&external_pointer), Err(Error::InvalidExtensionLayout));

        let metadata_offset = pointer_value + METADATA_POINTER_BYTES + 4;
        let mut mutable_metadata = canonical.clone();
        *mutable_metadata
            .get_mut(metadata_offset)
            .expect("metadata update authority") = 1;
        assert_eq!(check(&mutable_metadata), Err(Error::InvalidExtensionLayout));

        let mut trailing = canonical;
        trailing.push(0);
        assert_eq!(check(&trailing), Err(Error::InvalidExtensionLayout));
    }

    #[test]
    fn official_token_2022_interface_encoding_matches_the_profile() {
        let bytes = official_interface_mint(255);
        let facts = check(&bytes).expect("official Token-2022 interface Mint");
        assert_eq!(facts.display_decimals(), 255);
        assert_eq!(facts.base_supply(), 11);
        assert_eq!(facts.metadata(), InertMetadataV2::ImmutableSelfHosted);
    }

    #[test]
    fn every_behavior_changing_unknown_or_duplicate_extension_refuses() {
        for hostile_type in [
            1_u16, // TransferFeeConfig
            4,     // ConfidentialTransferMint
            6,     // DefaultAccountState
            9,     // NonTransferable
            10,    // InterestBearingConfig
            12,    // PermanentDelegate
            14,    // TransferHook
            16,    // ConfidentialTransferFeeConfig
            24,    // ConfidentialMintBurn
            25,    // ScaledUiAmount
            26,    // Pausable
            u16::MAX,
        ] {
            let mut hostile = mint(6, false);
            hostile.truncate(TLV_START_OFFSET);
            put_tlv(&mut hostile, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
            put_tlv(&mut hostile, hostile_type, &[]);
            put_tlv(&mut hostile, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }

        let mut duplicate = mint(6, false);
        duplicate.truncate(TLV_START_OFFSET);
        put_tlv(&mut duplicate, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        put_tlv(&mut duplicate, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        put_tlv(&mut duplicate, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
        assert_eq!(check(&duplicate), Err(Error::InvalidExtensionLayout));
    }

    #[test]
    fn physical_tlv_order_does_not_become_a_semantic_restriction() {
        let mut reordered = mint(6, false);
        reordered.truncate(TLV_START_OFFSET);
        put_tlv(&mut reordered, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
        put_tlv(&mut reordered, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        assert!(check(&reordered).is_ok());

        let mut metadata_first = mint(6, false);
        metadata_first.truncate(TLV_START_OFFSET);
        let mut pointer = [0; METADATA_POINTER_BYTES];
        put(&mut pointer, 32, &MINT_KEY);
        put_tlv(
            &mut metadata_first,
            TOKEN_METADATA_EXTENSION,
            &metadata_value(),
        );
        put_tlv(
            &mut metadata_first,
            PERMISSIONED_BURN_EXTENSION,
            &CONTROLLER,
        );
        put_tlv(&mut metadata_first, METADATA_POINTER_EXTENSION, &pointer);
        put_tlv(
            &mut metadata_first,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &CONTROLLER,
        );
        assert!(check(&metadata_first).is_ok());
    }

    #[test]
    fn authorities_base_state_and_metadata_borsh_are_hostile() {
        let canonical = mint(6, true);
        for offset in [4, 46, TLV_START_OFFSET + 4] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("authority byte") ^= 1;
            assert!(check(&hostile).is_err());
        }
        let mut wrong_supply = canonical.clone();
        put(&mut wrong_supply, 36, &12_u64.to_le_bytes());
        assert_eq!(check(&wrong_supply), Err(Error::MintSupplyMismatch));
        let mut uninitialized = canonical.clone();
        *uninitialized.get_mut(45).expect("initialized") = 0;
        assert_eq!(check(&uninitialized), Err(Error::MintUninitialized));

        let metadata_header =
            TLV_START_OFFSET + 4 + AUTHORITY_EXTENSION_BYTES + 4 + METADATA_POINTER_BYTES + 4;
        let mut invalid_utf8 = canonical.clone();
        let name_start = metadata_header + 64 + 4;
        *invalid_utf8.get_mut(name_start).expect("name byte") = 0xff;
        assert_eq!(check(&invalid_utf8), Err(Error::InvalidExtensionLayout));
        let mut hostile_count = canonical;
        let count_offset = metadata_header + metadata_value().len() - (4 + 4 + 7 + 4 + 1);
        put(&mut hostile_count, count_offset, &u32::MAX.to_le_bytes());
        assert_eq!(check(&hostile_count), Err(Error::InvalidExtensionLayout));
    }

    #[test]
    fn token_accounts_are_extension_free_and_exact_base_units() {
        let mut account = [0; ACCOUNT_BYTES];
        put(&mut account, 0, &MINT_KEY);
        put(&mut account, 32, &HOLDER);
        put(&mut account, 64, &17_u64.to_le_bytes());
        put(&mut account, 108, &[1]);
        let facts = Token2022BehaviorProfileV2::check_account(
            TOKEN_2022_PROGRAM_ID,
            &account,
            MINT_KEY,
            HOLDER,
            17,
        )
        .expect("base Account");
        assert_eq!(facts.base_amount(), 17);

        let mut extension = account.to_vec();
        extension.extend_from_slice(&[2, 0, 0, 0]);
        assert_eq!(
            Token2022BehaviorProfileV2::check_account(
                TOKEN_2022_PROGRAM_ID,
                &extension,
                MINT_KEY,
                HOLDER,
                0,
            ),
            Err(Error::InvalidLength)
        );

        for (offset, bytes, error) in [
            (108, [2_u8, 0, 0, 0], Error::AccountFrozen),
            (109, [1_u8, 0, 0, 0], Error::NativeAccount),
            (72, [1_u8, 0, 0, 0], Error::DelegatePresent),
            (129, [1_u8, 0, 0, 0], Error::CloseAuthorityPresent),
        ] {
            let mut hostile = account;
            put(&mut hostile, offset, &bytes);
            assert_eq!(
                Token2022BehaviorProfileV2::check_account(
                    TOKEN_2022_PROGRAM_ID,
                    &hostile,
                    MINT_KEY,
                    HOLDER,
                    0,
                ),
                Err(error)
            );
        }
    }
}
