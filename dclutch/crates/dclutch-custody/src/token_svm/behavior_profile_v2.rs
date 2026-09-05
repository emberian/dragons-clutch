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

use crate::token_svm::{
    Address, COption, Error, MINT_BYTES, Mint, Result, TOKEN_2022_PROGRAM_ID, TokenAccount,
    state::AccountState,
    tlv::{
        ACCOUNT_TYPE_OFFSET, AUTHORITY_EXTENSION_BYTES, BASE_ACCOUNT_BYTES,
        METADATA_POINTER_EXTENSION, MINT_ACCOUNT_TYPE, MINT_CLOSE_AUTHORITY_EXTENSION,
        PERMISSIONED_BURN_EXTENSION, TLV_START_OFFSET, TOKEN_METADATA_EXTENSION, TlvCursor,
        require_extension, require_extension_at_least, require_key,
    },
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

/// Authenticated facts from one compacted Fractional shard Mint.
///
/// A second facts type rather than a widened first one, for the reason
/// [`Token2022BehaviorMintFactsV2::controller`] states in its own doc: that
/// method promises one key bound identically as Mint, close *and*
/// permissioned-burn authority. A compacted shard Mint is exactly the shape
/// where that promise stops holding, so it gets a type whose accessors say what
/// is true of it rather than a field that quietly makes the other type's doc a
/// lie.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022CompactedShardMintFactsV2 {
    mint: Mint,
    controller: Address,
    burn_authority: Address,
    metadata: InertMetadataV2,
}

impl Token2022CompactedShardMintFactsV2 {
    /// Return the exact Token-owned base Mint state.
    pub const fn mint(self) -> Mint {
        self.mint
    }

    /// Return the controller still bound as Mint authority and close authority.
    ///
    /// The Fractional capability root. It keeps both of these after the
    /// hand-off and loses only the burn.
    pub const fn controller(self) -> Address {
        self.controller
    }

    /// Return the permissioned-burn authority the compaction handed over.
    ///
    /// Never equal to [`Self::controller`]: a Mint whose burn still answers to
    /// the root has not been compacted, and
    /// [`Token2022BehaviorProfileV2::read_compacted_shard_mint`] refuses it.
    pub const fn burn_authority(self) -> Address {
        self.burn_authority
    }

    /// Return the admitted inert-metadata shape.
    pub const fn metadata(self) -> InertMetadataV2 {
        self.metadata
    }

    /// Return the display-only decimals byte.
    pub const fn display_decimals(self) -> u8 {
        self.mint.decimals
    }

    /// Return the exact raw base-unit Mint supply.
    ///
    /// Reported rather than pinned, and unbounded in both directions. See
    /// [`Token2022BehaviorProfileV2::read_compacted_shard_mint`] for why a
    /// compacted coordinate's supply is the one number no caller can name.
    pub const fn base_supply(self) -> u64 {
        self.mint.supply
    }
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
    /// profile, including its exact expected base supply.
    ///
    /// This is the profile an on-chain program wants. A caller that already
    /// knows what the supply must be — because a request header, a record or a
    /// prior transition states it — must use this and not [`Self::read_mint`],
    /// so that the claim is checked against the chain rather than taken from
    /// it.
    pub fn check_mint(
        program_id: Address,
        mint_key: Address,
        mint_data: &[u8],
        expected_controller: Address,
        expected_base_supply: u64,
    ) -> Result<Token2022BehaviorMintFactsV2> {
        let facts = Self::read_mint(program_id, mint_key, mint_data, expected_controller)?;
        if facts.base_supply() != expected_base_supply {
            return Err(Error::MintSupplyMismatch);
        }
        Ok(facts)
    }

    /// Authenticate one representation Mint against every part of the V2
    /// behavior profile except its supply, and report the supply observed.
    ///
    /// Identity, ownership, controller, freeze absence, base-state shape and
    /// the exact required extension set are all authenticated here; only the
    /// supply is reported rather than pinned. Everything [`Self::check_mint`]
    /// refuses, this refuses too.
    ///
    /// It exists for the one caller class that legitimately cannot pin it: a
    /// transaction builder that must *discover* the current supply from chain
    /// state in order to stage it into a request the on-chain program will then
    /// pin. For that caller an expected supply would have to be the value it
    /// just read from these same bytes, and comparing a value to itself
    /// authenticates nothing — a vacuous assertion that would read like a check.
    /// The real check happens on-chain, against the number the builder staged.
    ///
    /// Any caller that has an independent expectation of the supply is not that
    /// caller and must use [`Self::check_mint`].
    pub fn read_mint(
        program_id: Address,
        mint_key: Address,
        mint_data: &[u8],
        expected_controller: Address,
    ) -> Result<Token2022BehaviorMintFactsV2> {
        // The burn role is named separately and then named as the controller,
        // which is the whole of what "one controller, three times over" means.
        // Stating it rather than letting the walk default to it is what makes
        // `read_compacted_shard_mint` below a second nomination rather than a
        // relaxation of this one.
        let (mint, metadata) = read_profile_mint(
            program_id,
            mint_key,
            mint_data,
            expected_controller,
            expected_controller,
        )?;
        Ok(Token2022BehaviorMintFactsV2 {
            mint,
            controller: expected_controller,
            metadata,
        })
    }

    /// Authenticate one Fractional shard Mint whose burn has been handed over.
    ///
    /// The same profile as [`Self::read_mint`] with the burn role nominated
    /// separately, and the *only* shape that entry point cannot describe.
    /// Fractional compaction re-points the shard Mint's permissioned-burn
    /// authority from the capability root to the Claims escrow, while the root
    /// is still alive to authorize it, because a shard burn needs a second
    /// signature and the root's is not one Claims can ever produce. After that
    /// one `SetAuthority`, the Mint authority and the close authority are still
    /// the root and the burn authority is the escrow, so `read_mint` refuses it
    /// — correctly, since every route that reads `read_mint` requires root
    /// control of the burn and would be weakened by admitting anything else.
    ///
    /// **The two entry points are disjoint over all Mint bytes, not merely
    /// different in intent.** This one refuses `expected_burn_authority ==
    /// expected_controller`, so a Mint it admits has a burn authority its
    /// mint authority does not equal, and `read_mint` requires those to be the
    /// same key. No byte string is admitted by both, whatever either caller
    /// nominates. A Mint that has not been handed off is therefore a *refusal*
    /// here rather than a pass, which is what stops the compacted arm from
    /// standing in for the live one.
    ///
    /// **Supply is reported and never pinned, and this is a third caller class
    /// rather than a hole in [`Self::read_mint`]'s discipline.** That doc says
    /// an on-chain caller with an independent expectation of the supply must
    /// use [`Self::check_mint`]. A compacted coordinate's caller has no such
    /// expectation and cannot acquire one: the outstanding shard supply *is*
    /// the durable claim, and any holder's redemption lowers it between the
    /// moment a request is built and the moment it lands. Pinning it would
    /// refuse an honest retirement because somebody else redeemed first. There
    /// is deliberately no `check_compacted_shard_mint`; a caller that wants one
    /// is a caller that has not noticed this.
    pub fn read_compacted_shard_mint(
        program_id: Address,
        mint_key: Address,
        mint_data: &[u8],
        expected_controller: Address,
        expected_burn_authority: Address,
    ) -> Result<Token2022CompactedShardMintFactsV2> {
        // A burn authority equal to the controller is a Mint the hand-off never
        // touched; a zero one is a Mint whose holders can never burn at all,
        // and `SetAuthority` will happily produce that, so it is refused here
        // rather than inherited from the base guard.
        if expected_burn_authority == [0; 32] || expected_burn_authority == expected_controller {
            return Err(Error::AuthorityMismatch);
        }
        let (mint, metadata) = read_profile_mint(
            program_id,
            mint_key,
            mint_data,
            expected_controller,
            expected_burn_authority,
        )?;
        Ok(Token2022CompactedShardMintFactsV2 {
            mint,
            controller: expected_controller,
            burn_authority: expected_burn_authority,
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

/// The sole V2 Mint walk, with every authority role named by its caller.
///
/// One author for the base-state shape, the padding, the account-type byte, the
/// freeze refusal, the extension set and the optional metadata pair — because
/// two readers of the same account bytes that maintained those separately is
/// exactly how a compacted arm quietly becomes a weaker arm. The two public
/// entry points differ in what they nominate and in nothing else.
fn read_profile_mint(
    program_id: Address,
    mint_key: Address,
    mint_data: &[u8],
    expected_controller: Address,
    expected_burn_authority: Address,
) -> Result<(Mint, InertMetadataV2)> {
    if program_id != TOKEN_2022_PROGRAM_ID {
        return Err(Error::ProfileProgramMismatch);
    }
    if mint_key == [0; 32] || expected_controller == [0; 32] || expected_burn_authority == [0; 32] {
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
                require_key(entry.value, expected_burn_authority)?;
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
    Ok((base, metadata))
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

    use crate::token_svm::{ACCOUNT_BYTES, LEGACY_TOKEN_PROGRAM_ID, state::fixtures::put};

    use super::*;

    const MINT_KEY: Address = [7; 32];
    const CONTROLLER: Address = [8; 32];
    const HOLDER: Address = [9; 32];
    /// The Claims escrow PDA a fractional compaction hands the burn to.
    const ESCROW: Address = [10; 32];

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
        mint_burning_to(decimals, with_metadata, CONTROLLER)
    }

    /// The same Mint with its permissioned-burn authority named separately.
    ///
    /// One fixture author for both arms, for the reason `read_profile_mint` has
    /// one: two builders maintaining the same base state and extension order
    /// separately is how a compacted fixture drifts into testing a shape the
    /// hand-off would never produce.
    fn mint_burning_to(
        decimals: u8,
        with_metadata: bool,
        burn_authority: Address,
    ) -> std::vec::Vec<u8> {
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
        put_tlv(&mut output, PERMISSIONED_BURN_EXTENSION, &burn_authority);
        output
    }

    /// Exactly what one `SetAuthority(PermissionedBurn)` leaves behind.
    fn compacted_mint(decimals: u8, with_metadata: bool) -> std::vec::Vec<u8> {
        mint_burning_to(decimals, with_metadata, ESCROW)
    }

    fn read_compacted(bytes: &[u8]) -> Result<Token2022CompactedShardMintFactsV2> {
        Token2022BehaviorProfileV2::read_compacted_shard_mint(
            TOKEN_2022_PROGRAM_ID,
            MINT_KEY,
            bytes,
            CONTROLLER,
            ESCROW,
        )
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

    fn read(bytes: &[u8]) -> Result<Token2022BehaviorMintFactsV2> {
        Token2022BehaviorProfileV2::read_mint(TOKEN_2022_PROGRAM_ID, MINT_KEY, bytes, CONTROLLER)
    }

    /// Base-Mint supply offset, for moving the one field these two entry
    /// points disagree about.
    const SUPPLY_OFFSET: usize = 36;

    /// The weaker entry point differs from `check_mint` on exactly one axis.
    ///
    /// `read_mint` reports whatever supply the account holds; `check_mint`
    /// admits only the supply its caller named. Asserting both directions on
    /// the same bytes is what keeps "weaker" from quietly becoming "weaker in
    /// some other way too".
    #[test]
    fn read_mint_reports_the_supply_that_check_mint_pins() {
        let canonical = mint(0, false);
        for supply in [0_u64, 1, 11, u64::MAX] {
            let mut bytes = canonical.clone();
            put(&mut bytes, SUPPLY_OFFSET, &supply.to_le_bytes());

            let observed = read(&bytes).expect("read admits every supply");
            assert_eq!(observed.base_supply(), supply);
            assert_eq!(observed.controller(), CONTROLLER);
            assert_eq!(observed.metadata(), InertMetadataV2::Absent);

            assert_eq!(
                Token2022BehaviorProfileV2::check_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    &bytes,
                    CONTROLLER,
                    supply,
                ),
                Ok(observed),
                "check admits the true supply"
            );
            assert_eq!(
                Token2022BehaviorProfileV2::check_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    &bytes,
                    CONTROLLER,
                    supply.wrapping_add(1),
                ),
                Err(Error::MintSupplyMismatch),
                "check refuses any other supply"
            );
        }
    }

    /// What `read_mint` refuses is the extension SET, not the account's size.
    ///
    /// The distinction is the whole point of the wallet-side fix: a builder
    /// that refused by length would refuse the correct Mint and admit a
    /// wrong-but-same-length one. So the control is two Mints of DIFFERENT
    /// lengths and one of IDENTICAL length, all refused for the same reason.
    #[test]
    fn read_mint_refuses_on_the_extension_set_and_never_on_the_length() {
        let canonical = mint(0, false);
        let base = canonical
            .get(..TLV_START_OFFSET)
            .expect("base account bytes")
            .to_vec();
        read(&canonical).expect("the shape the lifecycle writes is admitted");

        // Shorter, and wrong: the exact pre-fix 202-byte close-only Mint.
        let mut close_only = base.clone();
        put_tlv(&mut close_only, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        assert_eq!(close_only.len(), 202);
        assert_eq!(read(&close_only), Err(Error::InvalidExtensionLayout));

        // IDENTICAL length to the canonical Mint, and still wrong: the burn
        // extension replaced by a second close extension. A length check
        // cannot tell this from the real thing; the walk can.
        let mut same_length_wrong_set = base.clone();
        put_tlv(
            &mut same_length_wrong_set,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &CONTROLLER,
        );
        put_tlv(
            &mut same_length_wrong_set,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &CONTROLLER,
        );
        assert_eq!(same_length_wrong_set.len(), canonical.len());
        assert_eq!(
            read(&same_length_wrong_set),
            Err(Error::InvalidExtensionLayout)
        );

        // Longer, and wrong: both required extensions plus one nobody admits.
        let mut extra = canonical.clone();
        put_tlv(&mut extra, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
        assert_eq!(read(&extra), Err(Error::InvalidExtensionLayout));

        // The new entry point is not a hole in the program pin either.
        assert_eq!(
            Token2022BehaviorProfileV2::read_mint(
                LEGACY_TOKEN_PROGRAM_ID,
                MINT_KEY,
                &canonical,
                CONTROLLER,
            ),
            Err(Error::ProfileProgramMismatch)
        );
        assert_eq!(
            Token2022BehaviorProfileV2::read_mint(
                TOKEN_2022_PROGRAM_ID,
                MINT_KEY,
                &canonical,
                HOLDER,
            ),
            Err(Error::AuthorityMismatch)
        );
    }

    /// The compacted arm admits the hand-off's result, and reports both halves.
    ///
    /// The Mint authority and the close authority are still the Fractional
    /// root; only the burn moved. A facts type that reported one "controller"
    /// would have to pick which of those two facts to tell.
    #[test]
    fn the_compacted_arm_admits_what_the_hand_off_produces_and_names_both_halves() {
        for with_metadata in [false, true] {
            let bytes = compacted_mint(0, with_metadata);
            let facts = read_compacted(&bytes).expect("the shape SetAuthority leaves behind");
            assert_eq!(facts.controller(), CONTROLLER);
            assert_eq!(facts.burn_authority(), ESCROW);
            assert_ne!(facts.controller(), facts.burn_authority());
            assert_eq!(facts.base_supply(), 11);
            assert_eq!(facts.display_decimals(), 0);
            assert_eq!(
                facts.metadata(),
                if with_metadata {
                    InertMetadataV2::ImmutableSelfHosted
                } else {
                    InertMetadataV2::Absent
                }
            );
        }
    }

    /// **No Mint bytes are admitted by both arms.** The split's whole safety.
    ///
    /// Stated over the fixture family and over every nomination either arm
    /// could be handed, rather than as two separate "this one passes / that one
    /// fails" assertions, because the property that matters is a disjunction:
    /// if some Mint could satisfy both, the compacted arm would be a second way
    /// to reach every route the live arm gates. It cannot, and the reason is
    /// structural rather than incidental — the live arm requires the burn key
    /// to equal the mint authority and the compacted arm requires it not to.
    #[test]
    fn no_mint_bytes_are_admitted_by_both_arms() {
        let mut families = std::vec::Vec::new();
        for with_metadata in [false, true] {
            for burn in [CONTROLLER, ESCROW, HOLDER, [0; 32]] {
                families.push(mint_burning_to(0, with_metadata, burn));
            }
        }
        let mut live_admitted = 0_usize;
        let mut compacted_admitted = 0_usize;
        for bytes in &families {
            for controller in [CONTROLLER, ESCROW, HOLDER] {
                let live = Token2022BehaviorProfileV2::read_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    bytes,
                    controller,
                )
                .is_ok();
                if live {
                    live_admitted = live_admitted.saturating_add(1);
                }
                for burn in [CONTROLLER, ESCROW, HOLDER, [0; 32]] {
                    let compacted = Token2022BehaviorProfileV2::read_compacted_shard_mint(
                        TOKEN_2022_PROGRAM_ID,
                        MINT_KEY,
                        bytes,
                        controller,
                        burn,
                    )
                    .is_ok();
                    if compacted {
                        compacted_admitted = compacted_admitted.saturating_add(1);
                    }
                    assert!(
                        !(live && compacted),
                        "one Mint may never satisfy both arms at once"
                    );
                }
            }
        }
        // And neither arm is vacuous: a disjointness that held because nothing
        // was ever admitted would prove nothing at all. Counted exactly, so a
        // future edit that widened either arm moves a number here rather than
        // passing silently.
        //
        // Live: the two fixtures (metadata absent, present) whose burn TLV is
        // the controller, each admitted under exactly one of three nominations.
        assert_eq!(live_admitted, 2);
        // Compacted: the two escrow-burn fixtures AND the two holder-burn
        // fixtures, each admitted under exactly one controller/burn pair. The
        // holder-burn ones count because this profile does not care WHO holds
        // the burn after the hand-off, only that it is not the root -- which is
        // the right shape, since who it should be is the compaction route's
        // question and it derives the escrow itself rather than reading it.
        assert_eq!(compacted_admitted, 4);
    }

    /// The live arm was not relaxed, stated against the shipped function.
    ///
    /// This is the claim the split has to earn: every route reading `read_mint`
    /// today requires the Fractional root to control the burn, and a compacted
    /// Mint must remain refused there however it is nominated. A future edit
    /// that widened `read_mint` to accept a handed-off Mint has to argue with
    /// this test.
    #[test]
    fn the_live_arm_still_requires_root_control_of_the_burn() {
        let compacted = compacted_mint(0, false);
        // Named as the root: the burn extension no longer carries it.
        assert_eq!(read(&compacted), Err(Error::AuthorityMismatch));
        // Named as the escrow: the base Mint authority no longer carries that.
        assert_eq!(
            Token2022BehaviorProfileV2::read_mint(
                TOKEN_2022_PROGRAM_ID,
                MINT_KEY,
                &compacted,
                ESCROW,
            ),
            Err(Error::AuthorityMismatch)
        );
        // The supply-pinning entry point inherits the refusal rather than
        // reaching its own supply comparison first.
        assert_eq!(
            Token2022BehaviorProfileV2::check_mint(
                TOKEN_2022_PROGRAM_ID,
                MINT_KEY,
                &compacted,
                CONTROLLER,
                11,
            ),
            Err(Error::AuthorityMismatch)
        );
        // And the un-handed-off Mint is still admitted by the live arm exactly
        // as it was, which is the other half of "not relaxed": the refactor
        // moved the walk without moving what it admits.
        let live = mint(0, false);
        let facts = read(&live).expect("the uncompacted shard Mint is unchanged");
        assert_eq!(facts.controller(), CONTROLLER);
        assert_eq!(facts.base_supply(), 11);
        // The mirror: an uncompacted Mint is not a compacted one.
        assert_eq!(read_compacted(&live), Err(Error::AuthorityMismatch));
    }

    /// Every authority role is nominated, and none is inferred from another.
    ///
    /// `closeable_mint`'s own idiom, applied one profile over: two roles that
    /// happen to hold related keys still have to be stated, so a route that
    /// later separates them cannot pass by accident.
    #[test]
    fn the_compacted_arm_nominates_every_authority_and_infers_none() {
        let bytes = compacted_mint(0, false);
        for wrong in [[0; 32], HOLDER, MINT_KEY] {
            assert_eq!(
                Token2022BehaviorProfileV2::read_compacted_shard_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    &bytes,
                    wrong,
                    ESCROW,
                ),
                Err(Error::AuthorityMismatch),
                "the controller is not inferred from the Mint"
            );
            assert_eq!(
                Token2022BehaviorProfileV2::read_compacted_shard_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    &bytes,
                    CONTROLLER,
                    wrong,
                ),
                Err(Error::AuthorityMismatch),
                "the burn authority is not inferred from the Mint"
            );
        }
        // The Mint key still authenticates the metadata self-pointer, and the
        // program pin is not a hole here either.
        assert_eq!(
            Token2022BehaviorProfileV2::read_compacted_shard_mint(
                TOKEN_2022_PROGRAM_ID,
                [0; 32],
                &bytes,
                CONTROLLER,
                ESCROW,
            ),
            Err(Error::AuthorityMismatch)
        );
        assert_eq!(
            Token2022BehaviorProfileV2::read_compacted_shard_mint(
                LEGACY_TOKEN_PROGRAM_ID,
                MINT_KEY,
                &bytes,
                CONTROLLER,
                ESCROW,
            ),
            Err(Error::ProfileProgramMismatch)
        );
    }

    /// A hand-off to nobody is a real on-chain state, and it is refused.
    ///
    /// `SetAuthority` accepts `None` as the new authority, so a Mint whose
    /// permissioned-burn extension carries the zero key is producible by the
    /// same instruction the compaction uses. Its holders could never burn
    /// again, and every shard outstanding against it would be stranded
    /// permanently. That is refused at the nomination and at the bytes, so
    /// neither a caller mistake nor a chain state can reach it.
    #[test]
    fn a_hand_off_to_nobody_is_refused_at_the_nomination_and_at_the_bytes() {
        let bytes = compacted_mint(0, false);
        assert_eq!(
            Token2022BehaviorProfileV2::read_compacted_shard_mint(
                TOKEN_2022_PROGRAM_ID,
                MINT_KEY,
                &bytes,
                CONTROLLER,
                [0; 32],
            ),
            Err(Error::AuthorityMismatch)
        );
        let burned_to_nobody = mint_burning_to(0, false, [0; 32]);
        for burn in [[0; 32], CONTROLLER, ESCROW, HOLDER] {
            assert!(
                Token2022BehaviorProfileV2::read_compacted_shard_mint(
                    TOKEN_2022_PROGRAM_ID,
                    MINT_KEY,
                    &burned_to_nobody,
                    CONTROLLER,
                    burn,
                )
                .is_err(),
                "a Mint nobody can burn is admitted under no nomination"
            );
        }
        assert_eq!(read(&burned_to_nobody), Err(Error::AuthorityMismatch));
    }

    /// A Mint that has not been handed off is a refusal, not a pass.
    ///
    /// The direction that keeps the compacted arm from standing in for the live
    /// one. `RetireCoordinate`'s compacted arm skips the Mint close and admits
    /// a nonzero supply; reaching it with a Mint the root still controls would
    /// let a caller skip the close on a coordinate that was never compacted.
    #[test]
    fn a_mint_the_root_still_burns_is_refused_by_the_compacted_arm() {
        let uncompacted = mint(0, false);
        assert_eq!(
            Token2022BehaviorProfileV2::read_compacted_shard_mint(
                TOKEN_2022_PROGRAM_ID,
                MINT_KEY,
                &uncompacted,
                CONTROLLER,
                CONTROLLER,
            ),
            Err(Error::AuthorityMismatch),
            "naming one key twice is the un-compacted shape, and is refused"
        );
        assert_eq!(read_compacted(&uncompacted), Err(Error::AuthorityMismatch));
    }

    /// The compacted arm refuses the extension SET exactly as the live one does.
    ///
    /// The shared-walk claim, executed rather than asserted by construction:
    /// the three shapes `read_mint_refuses_on_the_extension_set_and_never_on_the_length`
    /// pins, rebuilt with the burn handed over, refuse identically.
    #[test]
    fn the_compacted_arm_refuses_the_extension_set_exactly_as_the_live_one_does() {
        let canonical = compacted_mint(0, false);
        let base = canonical
            .get(..TLV_START_OFFSET)
            .expect("base account bytes")
            .to_vec();
        read_compacted(&canonical).expect("the canonical compacted shape is admitted");

        // No burn extension at all: a Mint the hand-off could not have produced.
        let mut close_only = base.clone();
        put_tlv(&mut close_only, MINT_CLOSE_AUTHORITY_EXTENSION, &CONTROLLER);
        assert_eq!(close_only.len(), 202);
        assert_eq!(
            read_compacted(&close_only),
            Err(Error::InvalidExtensionLayout)
        );

        // Identical length, wrong set. A width check cannot tell these apart.
        let mut same_length_wrong_set = base.clone();
        put_tlv(
            &mut same_length_wrong_set,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &CONTROLLER,
        );
        put_tlv(
            &mut same_length_wrong_set,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &CONTROLLER,
        );
        assert_eq!(same_length_wrong_set.len(), canonical.len());
        assert_eq!(
            read_compacted(&same_length_wrong_set),
            Err(Error::InvalidExtensionLayout)
        );

        // A second burn extension, which is how a hostile Mint would try to
        // carry both authorities at once and satisfy both arms. It satisfies
        // neither, and the two arms refuse it at DIFFERENT clauses -- which is
        // the walk being honest rather than an inconsistency. The compacted arm
        // matches the escrow's entry, sets `burn_seen`, and refuses the second
        // on the layout; the live arm reaches the escrow's entry first and
        // refuses on the authority before a duplicate is even visible. Asserted
        // as "refused" for the live arm rather than as a code, because pinning
        // the code here would pin an ordering nothing else depends on.
        let mut two_burns = canonical.clone();
        put_tlv(&mut two_burns, PERMISSIONED_BURN_EXTENSION, &CONTROLLER);
        assert_eq!(
            read_compacted(&two_burns),
            Err(Error::InvalidExtensionLayout)
        );
        assert!(read(&two_burns).is_err());

        // The close authority is still the root's, and is still checked.
        let mut close_handed_over = std::vec![0; TLV_START_OFFSET];
        put(&mut close_handed_over, 0, &1_u32.to_le_bytes());
        put(&mut close_handed_over, 4, &CONTROLLER);
        put(&mut close_handed_over, 36, &11_u64.to_le_bytes());
        put(&mut close_handed_over, 45, &[1]);
        put(
            &mut close_handed_over,
            ACCOUNT_TYPE_OFFSET,
            &[MINT_ACCOUNT_TYPE],
        );
        put_tlv(
            &mut close_handed_over,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &ESCROW,
        );
        put_tlv(&mut close_handed_over, PERMISSIONED_BURN_EXTENSION, &ESCROW);
        assert_eq!(
            read_compacted(&close_handed_over),
            Err(Error::AuthorityMismatch),
            "compaction hands over the burn and nothing else"
        );
    }

    /// A compacted coordinate's supply is reported and pinned by nobody.
    ///
    /// The mirror of `read_mint_reports_the_supply_that_check_mint_pins`, and
    /// the reason there is no `check_compacted_shard_mint` to compare against:
    /// the outstanding shard supply is the durable claim, and any holder's
    /// redemption lowers it between a request being built and it landing.
    #[test]
    fn a_compacted_mint_reports_every_supply_and_pins_none() {
        let canonical = compacted_mint(0, false);
        for supply in [0_u64, 1, 11, u64::MAX] {
            let mut bytes = canonical.clone();
            put(&mut bytes, SUPPLY_OFFSET, &supply.to_le_bytes());
            let facts = read_compacted(&bytes).expect("every supply is admitted");
            assert_eq!(facts.base_supply(), supply);
            assert_eq!(facts.controller(), CONTROLLER);
            assert_eq!(facts.burn_authority(), ESCROW);
        }
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
