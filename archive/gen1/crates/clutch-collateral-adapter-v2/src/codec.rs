// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{Error, Id, Result, ID_BYTES};

pub(crate) struct Writer<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(output: &'a mut [u8], exact: usize) -> Result<Self> {
        if output.len() < exact {
            return Err(Error::Truncated);
        }
        if output.len() > exact {
            return Err(Error::TrailingBytes);
        }
        output.fill(0);
        Ok(Self { output, at: 0 })
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<()> {
        let end = self.at.checked_add(value.len()).ok_or(Error::Arithmetic)?;
        if end > self.output.len() {
            return Err(Error::Truncated);
        }
        self.output[self.at..end].copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    pub(crate) fn id(&mut self, value: Id) -> Result<()> {
        self.bytes(&value.bytes())
    }

    pub(crate) fn u8(&mut self, value: u8) -> Result<()> {
        self.bytes(&[value])
    }

    pub(crate) fn u16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.at == self.output.len() {
            Ok(())
        } else {
            Err(Error::Truncated)
        }
    }
}

pub(crate) struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(input: &'a [u8], exact: usize) -> Result<Self> {
        if input.len() < exact {
            return Err(Error::Truncated);
        }
        if input.len() > exact {
            return Err(Error::TrailingBytes);
        }
        Ok(Self { input, at: 0 })
    }

    pub(crate) fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::Arithmetic)?;
        if end > self.input.len() {
            return Err(Error::Truncated);
        }
        let mut value = [0; N];
        value.copy_from_slice(&self.input[self.at..end]);
        self.at = end;
        Ok(value)
    }

    pub(crate) fn id(&mut self) -> Result<Id> {
        Ok(Id::from_bytes(self.bytes::<ID_BYTES>()?))
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes::<2>()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes::<8>()?))
    }

    pub(crate) fn require_zeroes(&mut self, count: usize) -> Result<()> {
        let end = self.at.checked_add(count).ok_or(Error::Arithmetic)?;
        if end > self.input.len() {
            return Err(Error::Truncated);
        }
        if self.input[self.at..end].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        self.at = end;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(Error::TrailingBytes)
        }
    }
}
