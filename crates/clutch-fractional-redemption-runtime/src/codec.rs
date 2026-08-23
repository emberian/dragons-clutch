// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::Identity32V1;

use crate::{Error, Result};

pub(crate) fn exact(input: &[u8], expected: usize) -> Result<()> {
    if input.len() < expected {
        Err(Error::Truncated)
    } else if input.len() > expected {
        Err(Error::TrailingBytes)
    } else {
        Ok(())
    }
}

pub(crate) fn identity(input: &[u8], offset: usize) -> Result<Identity32V1> {
    let end = offset.checked_add(32).ok_or(Error::Arithmetic)?;
    let bytes: [u8; 32] = input
        .get(offset..end)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Identity32V1::new(bytes).map_err(|_| Error::ZeroIdentity)
}

pub(crate) fn put_identity(output: &mut [u8], offset: usize, value: Identity32V1) -> Result<()> {
    let end = offset.checked_add(32).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Truncated)?
        .copy_from_slice(&value.bytes());
    Ok(())
}

pub(crate) fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(Error::Arithmetic)?;
    Ok(u64::from_le_bytes(
        input
            .get(offset..end)
            .ok_or(Error::Truncated)?
            .try_into()
            .map_err(|_| Error::Truncated)?,
    ))
}

pub(crate) fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let end = offset.checked_add(8).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Truncated)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(crate) fn u128_at(input: &[u8], offset: usize) -> Result<u128> {
    let end = offset.checked_add(16).ok_or(Error::Arithmetic)?;
    Ok(u128::from_le_bytes(
        input
            .get(offset..end)
            .ok_or(Error::Truncated)?
            .try_into()
            .map_err(|_| Error::Truncated)?,
    ))
}

pub(crate) fn put_u128(output: &mut [u8], offset: usize, value: u128) -> Result<()> {
    let end = offset.checked_add(16).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Truncated)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(crate) fn require_zeroes(input: &[u8], start: usize, end: usize) -> Result<()> {
    let bytes = input.get(start..end).ok_or(Error::Truncated)?;
    if bytes.iter().any(|byte| *byte != 0) {
        Err(Error::NonCanonicalPadding)
    } else {
        Ok(())
    }
}
