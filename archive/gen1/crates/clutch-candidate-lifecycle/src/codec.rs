// SPDX-License-Identifier: AGPL-3.0-or-later

/// Total refusal from a versioned fixed-layout codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    WrongLength,
    WrongTag,
    WrongVersion,
    InvalidEnum,
    InvalidCount,
    ZeroIdentity,
    NonCanonicalPadding,
    MismatchedBinding,
    ArithmeticOverflow,
}

pub(crate) struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    pub(crate) fn exact(out: &'a mut [u8], expected: usize) -> Result<Self, CodecError> {
        if out.len() != expected {
            return Err(CodecError::WrongLength);
        }
        Ok(Self { out, at: 0 })
    }

    pub(crate) fn u8(&mut self, value: u8) -> Result<(), CodecError> {
        self.bytes(&[value])
    }

    pub(crate) fn u16(&mut self, value: u16) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn u64(&mut self, value: u64) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn u128(&mut self, value: u128) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), CodecError> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(CodecError::ArithmeticOverflow)?;
        let destination = self
            .out
            .get_mut(self.at..end)
            .ok_or(CodecError::WrongLength)?;
        destination.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(), CodecError> {
        if self.at != self.out.len() {
            return Err(CodecError::WrongLength);
        }
        Ok(())
    }
}

pub(crate) struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn exact(input: &'a [u8], expected: usize) -> Result<Self, CodecError> {
        if input.len() != expected {
            return Err(CodecError::WrongLength);
        }
        Ok(Self { input, at: 0 })
    }

    pub(crate) fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn u128(&mut self) -> Result<u128, CodecError> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.at..end)
            .ok_or(CodecError::WrongLength)?;
        let mut value = [0; N];
        value.copy_from_slice(source);
        self.at = end;
        Ok(value)
    }

    pub(crate) fn finish(self) -> Result<(), CodecError> {
        if self.at != self.input.len() {
            return Err(CodecError::WrongLength);
        }
        Ok(())
    }
}

pub(crate) fn put_header(writer: &mut Writer<'_>, tag: u8, version: u8) -> Result<(), CodecError> {
    writer.u8(tag)?;
    writer.u8(version)
}

pub(crate) fn check_header(
    reader: &mut Reader<'_>,
    tag: u8,
    version: u8,
) -> Result<(), CodecError> {
    if reader.u8()? != tag {
        return Err(CodecError::WrongTag);
    }
    if reader.u8()? != version {
        return Err(CodecError::WrongVersion);
    }
    Ok(())
}
