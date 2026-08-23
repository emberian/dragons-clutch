// SPDX-License-Identifier: AGPL-3.0-or-later

/// Total refusal from one strict General V2 codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// Input or output length was not the one exact required length.
    WrongLength,
    /// Account discriminator differed.
    WrongTag,
    /// Account schema version differed.
    WrongVersion,
    /// A required identity was zero.
    ZeroIdentity,
    /// A count or active width was outside its admitted range.
    InvalidCount,
    /// An enum or lifecycle combination was invalid.
    InvalidState,
    /// Canonically inactive bytes were nonzero.
    NonCanonicalPadding,
    /// Cross-field identities or dimensions disagreed.
    MismatchedBinding,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
}

/// Exact little-endian reader used by fixed-layout codecs.
#[derive(Debug)]
pub struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// Create a reader only for the one exact frame length.
    pub fn exact(input: &'a [u8], expected: usize) -> Result<Self, CodecError> {
        if input.len() != expected {
            return Err(CodecError::WrongLength);
        }
        Ok(Self { input, at: 0 })
    }

    /// Read one byte.
    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    /// Read one little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    /// Read one little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// Read one little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// Read one little-endian `u128`.
    pub fn u128(&mut self) -> Result<u128, CodecError> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    /// Read one exact byte array.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let bytes = self
            .input
            .get(self.at..end)
            .ok_or(CodecError::WrongLength)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        self.at = end;
        Ok(out)
    }

    /// Refuse trailing bytes.
    pub fn finish(self) -> Result<(), CodecError> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(CodecError::WrongLength)
        }
    }
}

/// Exact little-endian writer used by fixed-layout codecs.
#[derive(Debug)]
pub struct Writer<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    /// Create a writer only for the one exact frame length.
    pub fn exact(output: &'a mut [u8], expected: usize) -> Result<Self, CodecError> {
        if output.len() != expected {
            return Err(CodecError::WrongLength);
        }
        Ok(Self { output, at: 0 })
    }

    /// Write one byte.
    pub fn u8(&mut self, value: u8) -> Result<(), CodecError> {
        self.bytes(&[value])
    }
    /// Write one little-endian `u16`.
    pub fn u16(&mut self, value: u16) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }
    /// Write one little-endian `u32`.
    pub fn u32(&mut self, value: u32) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }
    /// Write one little-endian `u64`.
    pub fn u64(&mut self, value: u64) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }
    /// Write one little-endian `u128`.
    pub fn u128(&mut self, value: u128) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    /// Write exact bytes.
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), CodecError> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(CodecError::ArithmeticOverflow)?;
        let destination = self
            .output
            .get_mut(self.at..end)
            .ok_or(CodecError::WrongLength)?;
        destination.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    /// Refuse an incompletely written frame.
    pub fn finish(self) -> Result<(), CodecError> {
        if self.at == self.output.len() {
            Ok(())
        } else {
            Err(CodecError::WrongLength)
        }
    }
}
