//! Exact Token-2022 Mint profile for reclaimable protocol assets.
//!
//! This parser accepts one representation only: the 82-byte initialized Mint
//! base, zero padding through Token-2022's 165-byte base-account boundary, the
//! Mint account-type byte, and exactly the two lifecycle extensions the
//! protocol's terminal path needs — one `MintCloseAuthority` and one
//! `PermissionedBurn`, each carrying a single 32-byte authority. Relative order
//! of the two is not pinned, because Token-2022 fixes it nowhere; a duplicate,
//! any other extension, spare capacity or a trailing byte is refused.
//!
//! `PermissionedBurn` is required rather than optional because the protocol
//! really does burn through it: `BurnReceipt`, `BurnShard` and the Fractional
//! `WholeUnwrap` all emit `permissioned_burn` burn instructions against these
//! Mints. Token-2022 extensions are init-time-only, so a Mint created without
//! it can never be burned and can never be repaired — which makes the writer,
//! not the reader, the side that has to satisfy this profile.

use crate::{
    Address, COption, Error, MINT_BYTES, Mint, Result, TOKEN_2022_PROGRAM_ID,
    tlv::{
        ACCOUNT_TYPE_OFFSET, AUTHORITY_EXTENSION_BYTES, BASE_ACCOUNT_BYTES, MINT_ACCOUNT_TYPE,
        MINT_CLOSE_AUTHORITY_EXTENSION, PERMISSIONED_BURN_EXTENSION, TLV_HEADER_BYTES,
        TLV_START_OFFSET, TlvCursor, require_extension, require_key,
    },
};

/// Number of required lifecycle extensions on a closeable protocol Mint.
const REQUIRED_EXTENSIONS: usize = 2;

/// Exact Token-2022 Mint width carrying both required lifecycle extensions.
///
/// The account-type boundary plus one `MintCloseAuthority` and one
/// `PermissionedBurn` TLV, each a four-byte header over a 32-byte authority.
/// This is the width the Claims lifecycle allocates and the width its receipt
/// and shard rent principals are computed from.
pub const TOKEN_2022_CLOSEABLE_MINT_BYTES_V2: usize =
    TLV_START_OFFSET + REQUIRED_EXTENSIONS * (TLV_HEADER_BYTES + AUTHORITY_EXTENSION_BYTES);

/// Authenticated exact closeable-Mint facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022CloseableMintFactsV2 {
    mint: Mint,
    close_authority: Address,
    burn_authority: Address,
}

impl Token2022CloseableMintFactsV2 {
    /// Return the authenticated base Mint.
    pub const fn mint(self) -> Mint {
        self.mint
    }

    /// Return the sole authenticated close authority.
    pub const fn close_authority(self) -> Address {
        self.close_authority
    }

    /// Return the sole authenticated permissioned-burn authority.
    pub const fn burn_authority(self) -> Address {
        self.burn_authority
    }
}

/// Sole admitted closeable Token-2022 Mint profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022CloseableMintProfileV2;

impl Token2022CloseableMintProfileV2 {
    /// Authenticate one exact initialized Mint and all lifecycle-relevant
    /// authority, supply, decimal, freeze and extension facts.
    ///
    /// Each authority role is named by the caller. Two roles that happen to
    /// hold the same key on today's route still have to be stated, so that a
    /// route which later separates them cannot pass by accident.
    pub fn check_mint(
        program_id: Address,
        mint_data: &[u8],
        expected_mint_authority: Address,
        expected_close_authority: Address,
        expected_burn_authority: Address,
        expected_supply: u64,
        expected_decimals: u8,
    ) -> Result<Token2022CloseableMintFactsV2> {
        if program_id != TOKEN_2022_PROGRAM_ID {
            return Err(Error::ProfileProgramMismatch);
        }
        if expected_mint_authority == [0; 32]
            || expected_close_authority == [0; 32]
            || expected_burn_authority == [0; 32]
        {
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
        if base.mint_authority != COption::Some(expected_mint_authority) {
            return Err(Error::AuthorityMismatch);
        }
        if base.freeze_authority != COption::None {
            return Err(Error::FreezeAuthorityPresent);
        }
        if base.supply != expected_supply {
            return Err(Error::MintSupplyMismatch);
        }
        if base.decimals != expected_decimals {
            return Err(Error::DecimalsMismatch);
        }

        let mut cursor = TlvCursor::new(
            mint_data
                .get(TLV_START_OFFSET..)
                .ok_or(Error::InvalidExtensionLayout)?,
        );
        let mut close_seen = false;
        let mut burn_seen = false;
        while let Some(entry) = cursor.next()? {
            match entry.extension_type {
                MINT_CLOSE_AUTHORITY_EXTENSION if !close_seen => {
                    require_extension(
                        entry,
                        MINT_CLOSE_AUTHORITY_EXTENSION,
                        AUTHORITY_EXTENSION_BYTES,
                    )?;
                    require_key(entry.value, expected_close_authority)?;
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
                _ => return Err(Error::InvalidExtensionLayout),
            }
        }
        if !close_seen || !burn_seen {
            return Err(Error::InvalidExtensionLayout);
        }

        Ok(Token2022CloseableMintFactsV2 {
            mint: base,
            close_authority: expected_close_authority,
            burn_authority: expected_burn_authority,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        LEGACY_TOKEN_PROGRAM_ID,
        state::fixtures::put,
        tlv::{METADATA_POINTER_EXTENSION, put_tlv},
    };

    use super::*;

    const AUTHORITY: Address = [7; 32];
    const OTHER_AUTHORITY: Address = [8; 32];
    const BURN_AUTHORITY: Address = [9; 32];
    const MINT_AUTHORITY_OFFSET: usize = 0;
    const SUPPLY_OFFSET: usize = 36;
    const DECIMALS_OFFSET: usize = 44;
    const INITIALIZED_OFFSET: usize = 45;
    const FREEZE_AUTHORITY_OFFSET: usize = 46;

    /// The base account bytes, with no extension storage at all.
    fn base_account() -> std::vec::Vec<u8> {
        let mut output = std::vec![0; TLV_START_OFFSET];
        put(&mut output, MINT_AUTHORITY_OFFSET, &1_u32.to_le_bytes());
        put(&mut output, MINT_AUTHORITY_OFFSET + 4, &AUTHORITY);
        put(&mut output, SUPPLY_OFFSET, &11_u64.to_le_bytes());
        put(&mut output, DECIMALS_OFFSET, &[0]);
        put(&mut output, INITIALIZED_OFFSET, &[1]);
        put(&mut output, ACCOUNT_TYPE_OFFSET, &[MINT_ACCOUNT_TYPE]);
        output
    }

    /// The exact shape the Claims lifecycle writes, in the order it writes it.
    fn closeable_mint() -> std::vec::Vec<u8> {
        let mut output = base_account();
        put_tlv(&mut output, MINT_CLOSE_AUTHORITY_EXTENSION, &AUTHORITY);
        put_tlv(&mut output, PERMISSIONED_BURN_EXTENSION, &BURN_AUTHORITY);
        output
    }

    fn check(bytes: &[u8]) -> Result<Token2022CloseableMintFactsV2> {
        Token2022CloseableMintProfileV2::check_mint(
            TOKEN_2022_PROGRAM_ID,
            bytes,
            AUTHORITY,
            AUTHORITY,
            BURN_AUTHORITY,
            11,
            0,
        )
    }

    #[test]
    fn the_written_width_is_the_two_required_extensions_and_nothing_else() {
        assert_eq!(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2, 238);
        assert_eq!(closeable_mint().len(), TOKEN_2022_CLOSEABLE_MINT_BYTES_V2);
    }

    #[test]
    fn both_lifecycle_extensions_are_typed_in_either_order() {
        let facts = check(&closeable_mint()).expect("canonical closeable Mint");
        assert_eq!(facts.mint().supply, 11);
        assert_eq!(facts.mint().decimals, 0);
        assert_eq!(facts.close_authority(), AUTHORITY);
        assert_eq!(facts.burn_authority(), BURN_AUTHORITY);

        let mut reordered = base_account();
        put_tlv(&mut reordered, PERMISSIONED_BURN_EXTENSION, &BURN_AUTHORITY);
        put_tlv(&mut reordered, MINT_CLOSE_AUTHORITY_EXTENSION, &AUTHORITY);
        assert_eq!(reordered.len(), TOKEN_2022_CLOSEABLE_MINT_BYTES_V2);
        assert_eq!(check(&reordered), Ok(facts));
    }

    /// The defect this profile exists to refuse: the shape the Claims lifecycle
    /// used to write. It is a well-formed 202-byte Token-2022 Mint carrying a
    /// correct `MintCloseAuthority` and nothing else, so no base-state,
    /// authority or length-arithmetic check catches it — only the requirement
    /// that `PermissionedBurn` be present does.
    #[test]
    fn a_mint_without_permissioned_burn_is_refused_however_well_formed() {
        let mut close_only = base_account();
        put_tlv(&mut close_only, MINT_CLOSE_AUTHORITY_EXTENSION, &AUTHORITY);
        assert_eq!(close_only.len(), 202);
        assert_eq!(check(&close_only), Err(Error::InvalidExtensionLayout));

        let mut burn_only = base_account();
        put_tlv(&mut burn_only, PERMISSIONED_BURN_EXTENSION, &BURN_AUTHORITY);
        assert_eq!(check(&burn_only), Err(Error::InvalidExtensionLayout));

        assert_eq!(check(&base_account()), Err(Error::InvalidExtensionLayout));
    }

    #[test]
    fn duplicate_unknown_and_extra_extensions_are_refused() {
        let mut duplicate_close = base_account();
        put_tlv(
            &mut duplicate_close,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &AUTHORITY,
        );
        put_tlv(
            &mut duplicate_close,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &AUTHORITY,
        );
        assert_eq!(duplicate_close.len(), TOKEN_2022_CLOSEABLE_MINT_BYTES_V2);
        assert_eq!(check(&duplicate_close), Err(Error::InvalidExtensionLayout));

        let mut duplicate_burn = closeable_mint();
        put_tlv(
            &mut duplicate_burn,
            PERMISSIONED_BURN_EXTENSION,
            &BURN_AUTHORITY,
        );
        assert_eq!(check(&duplicate_burn), Err(Error::InvalidExtensionLayout));

        // Admitted by the wider behavior profile, refused by this one: the
        // lifecycle's own Mints carry no metadata.
        let mut with_metadata_pointer = closeable_mint();
        put_tlv(
            &mut with_metadata_pointer,
            METADATA_POINTER_EXTENSION,
            &[0; 64],
        );
        assert_eq!(
            check(&with_metadata_pointer),
            Err(Error::InvalidExtensionLayout)
        );

        for extension_type in [1_u16, 2, 4, u16::MAX] {
            let mut unknown = closeable_mint();
            put_tlv(&mut unknown, extension_type, &AUTHORITY);
            assert_eq!(check(&unknown), Err(Error::InvalidExtensionLayout));
        }
    }

    #[test]
    fn spare_capacity_truncation_padding_and_account_type_are_closed() {
        let canonical = closeable_mint();
        for length in 0..canonical.len() {
            let prefix = canonical.get(..length).expect("prefix");
            let expected = if length < TLV_START_OFFSET {
                Error::InvalidLength
            } else {
                Error::InvalidExtensionLayout
            };
            assert_eq!(check(prefix), Err(expected), "truncated to {length}");
        }
        for spare in 1..=TLV_HEADER_BYTES {
            let mut trailing = canonical.clone();
            trailing.extend_from_slice(&std::vec![0; spare]);
            assert_eq!(check(&trailing), Err(Error::InvalidExtensionLayout));
        }

        for offset in [MINT_BYTES, BASE_ACCOUNT_BYTES - 1] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("padding byte") = 1;
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
        for account_type in [0, 2, 255] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(ACCOUNT_TYPE_OFFSET).expect("account type") = account_type;
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
        for length in [0_u16, 31, 33, u16::MAX] {
            let mut hostile = base_account();
            put_tlv(&mut hostile, MINT_CLOSE_AUTHORITY_EXTENSION, &AUTHORITY);
            put_tlv(&mut hostile, PERMISSIONED_BURN_EXTENSION, &BURN_AUTHORITY);
            put(&mut hostile, TLV_START_OFFSET + 2, &length.to_le_bytes());
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
    }

    #[test]
    fn base_and_extension_authorities_supply_decimals_and_freeze_are_exact() {
        let canonical = closeable_mint();
        let mut uninitialized = canonical.clone();
        *uninitialized
            .get_mut(INITIALIZED_OFFSET)
            .expect("initialized") = 0;
        assert_eq!(check(&uninitialized), Err(Error::MintUninitialized));

        let mut mint_authority = canonical.clone();
        put(
            &mut mint_authority,
            MINT_AUTHORITY_OFFSET + 4,
            &OTHER_AUTHORITY,
        );
        assert_eq!(check(&mint_authority), Err(Error::AuthorityMismatch));

        let mut close_authority = base_account();
        put_tlv(
            &mut close_authority,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &OTHER_AUTHORITY,
        );
        put_tlv(
            &mut close_authority,
            PERMISSIONED_BURN_EXTENSION,
            &BURN_AUTHORITY,
        );
        assert_eq!(check(&close_authority), Err(Error::AuthorityMismatch));

        let mut burn_authority = base_account();
        put_tlv(
            &mut burn_authority,
            MINT_CLOSE_AUTHORITY_EXTENSION,
            &AUTHORITY,
        );
        put_tlv(
            &mut burn_authority,
            PERMISSIONED_BURN_EXTENSION,
            &OTHER_AUTHORITY,
        );
        assert_eq!(check(&burn_authority), Err(Error::AuthorityMismatch));

        let mut supply = canonical.clone();
        put(&mut supply, SUPPLY_OFFSET, &12_u64.to_le_bytes());
        assert_eq!(check(&supply), Err(Error::MintSupplyMismatch));

        let mut decimals = canonical.clone();
        *decimals.get_mut(DECIMALS_OFFSET).expect("decimals") = 1;
        assert_eq!(check(&decimals), Err(Error::DecimalsMismatch));

        let mut freeze = canonical;
        put(&mut freeze, FREEZE_AUTHORITY_OFFSET, &1_u32.to_le_bytes());
        put(&mut freeze, FREEZE_AUTHORITY_OFFSET + 4, &OTHER_AUTHORITY);
        assert_eq!(check(&freeze), Err(Error::FreezeAuthorityPresent));
    }

    #[test]
    fn program_and_expected_authorities_are_not_inferred() {
        let canonical = closeable_mint();
        assert_eq!(
            Token2022CloseableMintProfileV2::check_mint(
                LEGACY_TOKEN_PROGRAM_ID,
                &canonical,
                AUTHORITY,
                AUTHORITY,
                BURN_AUTHORITY,
                11,
                0,
            ),
            Err(Error::ProfileProgramMismatch)
        );
        for authority in [[0; 32], OTHER_AUTHORITY] {
            for roles in [
                (authority, AUTHORITY, BURN_AUTHORITY),
                (AUTHORITY, authority, BURN_AUTHORITY),
                (AUTHORITY, AUTHORITY, authority),
            ] {
                let (mint_authority, close_authority, burn_authority) = roles;
                assert_eq!(
                    Token2022CloseableMintProfileV2::check_mint(
                        TOKEN_2022_PROGRAM_ID,
                        &canonical,
                        mint_authority,
                        close_authority,
                        burn_authority,
                        11,
                        0,
                    ),
                    Err(Error::AuthorityMismatch)
                );
            }
        }
    }
}
