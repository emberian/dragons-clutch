// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{Error, Id, Result};

pub(crate) const HEADER_BYTES: usize = 12;

pub(crate) struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
    failed: bool,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(output: &'a mut [u8], expected: usize) -> Result<Self> {
        if output.len() < expected {
            return Err(Error::Truncated);
        }
        if output.len() > expected {
            return Err(Error::TrailingBytes);
        }
        output.fill(0);
        Ok(Self {
            output,
            cursor: 0,
            failed: false,
        })
    }

    pub(crate) fn header(&mut self, magic: &[u8; 8], version: u16) {
        self.bytes(magic);
        self.u16(version);
        self.u16(0);
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        let Some(end) = self.cursor.checked_add(value.len()) else {
            self.failed = true;
            return;
        };
        if end > self.output.len() {
            self.failed = true;
            return;
        }
        self.output[self.cursor..end].copy_from_slice(value);
        self.cursor = end;
    }

    pub(crate) fn id(&mut self, value: Id) {
        self.bytes(&value.bytes());
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub(crate) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn reserved(&mut self, length: usize) {
        let Some(end) = self.cursor.checked_add(length) else {
            self.failed = true;
            return;
        };
        if end > self.output.len() {
            self.failed = true;
            return;
        }
        self.cursor = end;
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.failed || self.cursor < self.output.len() {
            Err(Error::Truncated)
        } else if self.cursor > self.output.len() {
            Err(Error::TrailingBytes)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Reader<'a> {
    input: &'a [u8],
    cursor: usize,
    failed: bool,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(input: &'a [u8], expected: usize) -> Result<Self> {
        if input.len() < expected {
            return Err(Error::Truncated);
        }
        if input.len() > expected {
            return Err(Error::TrailingBytes);
        }
        Ok(Self {
            input,
            cursor: 0,
            failed: false,
        })
    }

    pub(crate) fn header(&mut self, magic: &[u8; 8], version: u16) -> Result<()> {
        if self.bytes::<8>() != *magic {
            return Err(Error::BadMagic);
        }
        if self.u16() != version {
            return Err(Error::BadVersion);
        }
        self.reserved(2)
    }

    pub(crate) fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut value = [0; N];
        let Some(end) = self.cursor.checked_add(N) else {
            self.failed = true;
            return value;
        };
        if end > self.input.len() {
            self.failed = true;
            return value;
        }
        value.copy_from_slice(&self.input[self.cursor..end]);
        self.cursor = end;
        value
    }

    pub(crate) fn id(&mut self) -> Id {
        Id::from_bytes(self.bytes())
    }

    pub(crate) fn u8(&mut self) -> u8 {
        self.bytes::<1>()[0]
    }

    pub(crate) fn bool(&mut self) -> Result<bool> {
        match self.u8() {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidParameter),
        }
    }

    pub(crate) fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes())
    }

    pub(crate) fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes())
    }

    pub(crate) fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.bytes())
    }

    pub(crate) fn i64(&mut self) -> i64 {
        i64::from_le_bytes(self.bytes())
    }

    pub(crate) fn reserved(&mut self, length: usize) -> Result<()> {
        let Some(end) = self.cursor.checked_add(length) else {
            self.failed = true;
            return Err(Error::Truncated);
        };
        if end > self.input.len() {
            self.failed = true;
            return Err(Error::Truncated);
        }
        if self.input[self.cursor..end].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        self.cursor = end;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.failed || self.cursor < self.input.len() {
            Err(Error::Truncated)
        } else if self.cursor > self.input.len() {
            Err(Error::TrailingBytes)
        } else {
            Ok(())
        }
    }
}
