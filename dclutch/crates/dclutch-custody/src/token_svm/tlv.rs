//! One Token-2022 extension-TLV walker, shared by every V2 Mint profile.
//!
//! Both admitted Mint profiles read the same account bytes, so they read them
//! with the same code. A profile states which extension types it requires and
//! what each value must contain; the walk itself refuses zero-typed, truncated,
//! over-long and trailing entries identically for all of them. Nothing here
//! indexes a TLV by a fixed offset: a profile that hardcoded where its second
//! extension begins would be satisfied by a fixture that put the right bytes in
//! the right place and by nothing else.

use core::convert::TryInto;

use crate::token_svm::{Address, Error, Result};

/// Token-2022 base-account width. Extension storage begins after the
/// account-type byte that follows it.
pub(crate) const BASE_ACCOUNT_BYTES: usize = 165;

/// Offset of the Token-2022 account-type discriminant.
pub(crate) const ACCOUNT_TYPE_OFFSET: usize = BASE_ACCOUNT_BYTES;

/// First byte of extension TLV storage.
pub(crate) const TLV_START_OFFSET: usize = 166;

/// Account-type discriminant of a Mint.
pub(crate) const MINT_ACCOUNT_TYPE: u8 = 1;

/// Account-type discriminant of a token Account.
pub(crate) const ACCOUNT_ACCOUNT_TYPE: u8 = 2;

/// Bytes one TLV entry spends on its type and length header.
pub(crate) const TLV_HEADER_BYTES: usize = 4;

/// Value width of every single-authority Mint extension.
pub(crate) const AUTHORITY_EXTENSION_BYTES: usize = 32;

/// `MintCloseAuthority` extension type.
pub(crate) const MINT_CLOSE_AUTHORITY_EXTENSION: u16 = 3;

/// `ImmutableOwner` extension type. Its value is empty; the type IS the fact.
pub(crate) const IMMUTABLE_OWNER_EXTENSION: u16 = 7;

/// `MetadataPointer` extension type.
pub(crate) const METADATA_POINTER_EXTENSION: u16 = 18;

/// `TokenMetadata` extension type.
pub(crate) const TOKEN_METADATA_EXTENSION: u16 = 19;

/// `PermissionedBurn` extension type.
pub(crate) const PERMISSIONED_BURN_EXTENSION: u16 = 28;

/// One borrowed extension entry.
#[derive(Clone, Copy)]
pub(crate) struct TlvEntry<'a> {
    /// Token-2022 extension discriminant.
    pub(crate) extension_type: u16,
    /// Borrowed extension value, exactly as long as the entry declared.
    pub(crate) value: &'a [u8],
}

/// Forward-only walk over an authenticated extension-storage slice.
pub(crate) struct TlvCursor<'a> {
    remaining: &'a [u8],
}

impl<'a> TlvCursor<'a> {
    /// Begin a walk at the first byte of extension storage.
    pub(crate) const fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }

    /// Return the next entry, or `None` once the slice is exactly consumed.
    ///
    /// Spare capacity is refused rather than skipped: Token-2022 writes a zero
    /// discriminant nowhere, so a zero type means unwritten storage inside an
    /// account this profile requires to be exactly full.
    pub(crate) fn next(&mut self) -> Result<Option<TlvEntry<'a>>> {
        if self.remaining.is_empty() {
            return Ok(None);
        }
        let extension_type = read_u16(self.remaining, 0)?;
        let length = usize::from(read_u16(self.remaining, 2)?);
        if extension_type == 0 {
            return Err(Error::InvalidExtensionLayout);
        }
        let end = TLV_HEADER_BYTES
            .checked_add(length)
            .ok_or(Error::InvalidExtensionLayout)?;
        let value = self
            .remaining
            .get(TLV_HEADER_BYTES..end)
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

/// Require one entry to be exactly the named extension at exactly one width.
pub(crate) fn require_extension(
    entry: TlvEntry<'_>,
    extension_type: u16,
    length: usize,
) -> Result<()> {
    if entry.extension_type != extension_type || entry.value.len() != length {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

/// Require one entry to be the named extension at or above a minimum width.
pub(crate) fn require_extension_at_least(
    entry: TlvEntry<'_>,
    extension_type: u16,
    minimum_length: usize,
) -> Result<()> {
    if entry.extension_type != extension_type || entry.value.len() < minimum_length {
        return Err(Error::InvalidExtensionLayout);
    }
    Ok(())
}

/// Require one extension value to be exactly the expected authority key.
pub(crate) fn require_key(bytes: &[u8], expected: Address) -> Result<()> {
    let observed: Address = bytes
        .try_into()
        .map_err(|_| Error::InvalidExtensionLayout)?;
    if observed != expected {
        return Err(Error::AuthorityMismatch);
    }
    Ok(())
}

/// Read one little-endian `u16` at an offset, refusing a short read.
pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    bytes
        .get(offset..offset.checked_add(2).ok_or(Error::InvalidExtensionLayout)?)
        .ok_or(Error::InvalidExtensionLayout)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| Error::InvalidExtensionLayout)
}

/// Append one TLV entry to a test fixture.
#[cfg(test)]
pub(crate) fn put_tlv(output: &mut std::vec::Vec<u8>, extension_type: u16, value: &[u8]) {
    output.extend_from_slice(&extension_type.to_le_bytes());
    output.extend_from_slice(
        &u16::try_from(value.len())
            .expect("test TLV length")
            .to_le_bytes(),
    );
    output.extend_from_slice(value);
}
