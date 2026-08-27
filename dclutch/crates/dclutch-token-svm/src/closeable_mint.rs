//! Exact Token-2022 Mint profile for reclaimable protocol assets.
//!
//! This parser accepts one representation only: the 82-byte initialized Mint
//! base, zero padding through Token-2022's 165-byte base-account boundary, the
//! Mint account-type byte, and exactly one 36-byte `MintCloseAuthority` TLV.
//! No other extension, spare capacity, duplicate, or trailing byte is admitted.

use core::convert::TryInto;

use crate::{Address, COption, Error, MINT_BYTES, Mint, Result, TOKEN_2022_PROGRAM_ID};

/// Exact Token-2022 Mint width with the sole `MintCloseAuthority` extension.
pub const TOKEN_2022_CLOSEABLE_MINT_BYTES_V2: usize = 202;

const BASE_ACCOUNT_BYTES: usize = 165;
const ACCOUNT_TYPE_OFFSET: usize = BASE_ACCOUNT_BYTES;
const TLV_TYPE_OFFSET: usize = 166;
const TLV_LENGTH_OFFSET: usize = 168;
const TLV_VALUE_OFFSET: usize = 170;
const TLV_VALUE_BYTES: usize = 32;

const MINT_ACCOUNT_TYPE: u8 = 1;
const MINT_CLOSE_AUTHORITY_EXTENSION_TYPE: u16 = 3;

/// Authenticated exact closeable-Mint facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022CloseableMintFactsV2 {
    mint: Mint,
    close_authority: Address,
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
}

/// Sole admitted closeable Token-2022 Mint profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token2022CloseableMintProfileV2;

impl Token2022CloseableMintProfileV2 {
    /// Authenticate one exact initialized Mint and all lifecycle-relevant
    /// authority, supply, decimal, and freeze facts.
    pub fn check_mint(
        program_id: Address,
        mint_data: &[u8],
        expected_mint_authority: Address,
        expected_close_authority: Address,
        expected_supply: u64,
        expected_decimals: u8,
    ) -> Result<Token2022CloseableMintFactsV2> {
        if program_id != TOKEN_2022_PROGRAM_ID {
            return Err(Error::ProfileProgramMismatch);
        }
        if mint_data.len() != TOKEN_2022_CLOSEABLE_MINT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if expected_mint_authority == [0; 32] || expected_close_authority == [0; 32] {
            return Err(Error::AuthorityMismatch);
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
            || read_u16(mint_data, TLV_TYPE_OFFSET)? != MINT_CLOSE_AUTHORITY_EXTENSION_TYPE
            || usize::from(read_u16(mint_data, TLV_LENGTH_OFFSET)?) != TLV_VALUE_BYTES
        {
            return Err(Error::InvalidExtensionLayout);
        }
        let close_authority: Address = mint_data
            .get(
                TLV_VALUE_OFFSET
                    ..TLV_VALUE_OFFSET
                        .checked_add(TLV_VALUE_BYTES)
                        .ok_or(Error::InvalidExtensionLayout)?,
            )
            .ok_or(Error::InvalidExtensionLayout)?
            .try_into()
            .map_err(|_| Error::InvalidExtensionLayout)?;
        if !base.is_initialized {
            return Err(Error::MintUninitialized);
        }
        if base.mint_authority != COption::Some(expected_mint_authority)
            || close_authority != expected_close_authority
        {
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
        Ok(Token2022CloseableMintFactsV2 {
            mint: base,
            close_authority,
        })
    }
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    input
        .get(offset..offset.checked_add(2).ok_or(Error::InvalidExtensionLayout)?)
        .ok_or(Error::InvalidExtensionLayout)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| Error::InvalidExtensionLayout)
}

#[cfg(test)]
mod tests {
    use crate::{LEGACY_TOKEN_PROGRAM_ID, state::fixtures::put};

    use super::*;

    const AUTHORITY: Address = [7; 32];
    const OTHER_AUTHORITY: Address = [8; 32];
    const MINT_AUTHORITY_OFFSET: usize = 0;
    const SUPPLY_OFFSET: usize = 36;
    const DECIMALS_OFFSET: usize = 44;
    const INITIALIZED_OFFSET: usize = 45;
    const FREEZE_AUTHORITY_OFFSET: usize = 46;

    fn closeable_mint() -> [u8; TOKEN_2022_CLOSEABLE_MINT_BYTES_V2] {
        let mut output = [0; TOKEN_2022_CLOSEABLE_MINT_BYTES_V2];
        put(&mut output, MINT_AUTHORITY_OFFSET, &1_u32.to_le_bytes());
        put(&mut output, MINT_AUTHORITY_OFFSET + 4, &AUTHORITY);
        put(&mut output, SUPPLY_OFFSET, &11_u64.to_le_bytes());
        put(&mut output, DECIMALS_OFFSET, &[0]);
        put(&mut output, INITIALIZED_OFFSET, &[1]);
        put(&mut output, ACCOUNT_TYPE_OFFSET, &[MINT_ACCOUNT_TYPE]);
        put(
            &mut output,
            TLV_TYPE_OFFSET,
            &MINT_CLOSE_AUTHORITY_EXTENSION_TYPE.to_le_bytes(),
        );
        put(
            &mut output,
            TLV_LENGTH_OFFSET,
            &u16::try_from(TLV_VALUE_BYTES)
                .expect("authority width")
                .to_le_bytes(),
        );
        put(&mut output, TLV_VALUE_OFFSET, &AUTHORITY);
        output
    }

    fn check(bytes: &[u8]) -> Result<Token2022CloseableMintFactsV2> {
        Token2022CloseableMintProfileV2::check_mint(
            TOKEN_2022_PROGRAM_ID,
            bytes,
            AUTHORITY,
            AUTHORITY,
            11,
            0,
        )
    }

    #[test]
    fn exact_single_close_authority_layout_is_typed() {
        let facts = check(&closeable_mint()).expect("canonical closeable Mint");
        assert_eq!(facts.mint().supply, 11);
        assert_eq!(facts.mint().decimals, 0);
        assert_eq!(facts.close_authority(), AUTHORITY);
    }

    #[test]
    fn exact_width_padding_account_type_and_tlv_shape_are_closed() {
        let canonical = closeable_mint();
        for length in 0..TOKEN_2022_CLOSEABLE_MINT_BYTES_V2 {
            assert_eq!(
                check(canonical.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut trailing = canonical.to_vec();
        trailing.push(0);
        assert_eq!(check(&trailing), Err(Error::InvalidLength));

        for offset in [MINT_BYTES, BASE_ACCOUNT_BYTES - 1] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("padding byte") = 1;
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
        for account_type in [0, 2, 255] {
            let mut hostile = canonical;
            *hostile.get_mut(ACCOUNT_TYPE_OFFSET).expect("account type") = account_type;
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
        for extension_type in [0_u16, 1, 2, 4, u16::MAX] {
            let mut hostile = canonical;
            put(&mut hostile, TLV_TYPE_OFFSET, &extension_type.to_le_bytes());
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
        for length in [0_u16, 31, 33, u16::MAX] {
            let mut hostile = canonical;
            put(&mut hostile, TLV_LENGTH_OFFSET, &length.to_le_bytes());
            assert_eq!(check(&hostile), Err(Error::InvalidExtensionLayout));
        }
    }

    #[test]
    fn base_and_extension_authorities_supply_decimals_and_freeze_are_exact() {
        let canonical = closeable_mint();
        let mut uninitialized = canonical;
        *uninitialized
            .get_mut(INITIALIZED_OFFSET)
            .expect("initialized") = 0;
        assert_eq!(check(&uninitialized), Err(Error::MintUninitialized));

        let mut mint_authority = canonical;
        put(
            &mut mint_authority,
            MINT_AUTHORITY_OFFSET + 4,
            &OTHER_AUTHORITY,
        );
        assert_eq!(check(&mint_authority), Err(Error::AuthorityMismatch));

        let mut close_authority = canonical;
        put(&mut close_authority, TLV_VALUE_OFFSET, &OTHER_AUTHORITY);
        assert_eq!(check(&close_authority), Err(Error::AuthorityMismatch));

        let mut supply = canonical;
        put(&mut supply, SUPPLY_OFFSET, &12_u64.to_le_bytes());
        assert_eq!(check(&supply), Err(Error::MintSupplyMismatch));

        let mut decimals = canonical;
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
                11,
                0,
            ),
            Err(Error::ProfileProgramMismatch)
        );
        for authority in [[0; 32], OTHER_AUTHORITY] {
            assert_eq!(
                Token2022CloseableMintProfileV2::check_mint(
                    TOKEN_2022_PROGRAM_ID,
                    &canonical,
                    authority,
                    AUTHORITY,
                    11,
                    0,
                ),
                Err(Error::AuthorityMismatch)
            );
            assert_eq!(
                Token2022CloseableMintProfileV2::check_mint(
                    TOKEN_2022_PROGRAM_ID,
                    &canonical,
                    AUTHORITY,
                    authority,
                    11,
                    0,
                ),
                Err(Error::AuthorityMismatch)
            );
        }
    }
}
