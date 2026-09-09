//! Ordered, bounded SHA-256 commitment to one complete Loader ELF tail.
//!
//! This is the artifact's on-chain code identity. Flat ELF SHA-256 remains a
//! separate build-provenance fact. Both host construction and Registry admission
//! advance this same state; only Registry-owned staging may persist progress.

use dclutch_sha256_adapter::{digest, digestv};

/// Provisional syscall chunk profile: one MiB per verification transaction.
///
/// At the measured syscall rate this spends approximately 524,288 CU on payload
/// hashing. Lifting or changing this profile requires a successor commitment
/// schema and a measured complete finalization transaction, not a local constant
/// bump: chunk geometry is part of the committed preimage.
pub const CODE_COMMITMENT_CHUNK_BYTES_V2: usize = 1_048_576;
/// Exact temporary progress tail: total length, next offset, and rolling digest.
pub const CODE_COMMITMENT_PROGRESS_BYTES_V2: usize = 48;
const INITIAL_DOMAIN: &[u8] = b"dclutch/code-commitment-v2/initial";
const CHUNK_DOMAIN: &[u8] = b"dclutch/code-commitment-v2/chunk";

/// A malformed commitment or an attempt to skip, repeat, or truncate coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeCommitmentErrorV2 {
    /// A Loader ELF tail cannot be empty.
    EmptyCode,
    /// Progress bytes or their derived geometry were not canonical.
    Encoding,
    /// The supplied offset did not equal the next unverified byte.
    Offset,
    /// The slice was not the complete next canonical chunk.
    ChunkLength,
    /// A final commitment was requested before every byte was covered.
    Incomplete,
    /// Length or offset arithmetic could not be represented.
    Arithmetic,
}

/// Private chain state for contiguous verified coverage of one ELF tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeCommitmentProgressV2 {
    total_length: u64,
    next_offset: u64,
    rolling: [u8; 32],
}

impl CodeCommitmentProgressV2 {
    /// Start coverage, binding total length and the canonical chunk profile.
    pub fn new(total_length: u64) -> Result<Self, CodeCommitmentErrorV2> {
        if total_length == 0 {
            return Err(CodeCommitmentErrorV2::EmptyCode);
        }
        let width = u64::try_from(CODE_COMMITMENT_CHUNK_BYTES_V2)
            .map_err(|_| CodeCommitmentErrorV2::Arithmetic)?;
        Ok(Self {
            total_length,
            next_offset: 0,
            rolling: digestv(&[
                INITIAL_DOMAIN,
                &total_length.to_le_bytes(),
                &width.to_le_bytes(),
            ]),
        })
    }

    /// Exact total ELF length bound at the first verification step.
    pub const fn total_length(self) -> u64 {
        self.total_length
    }
    /// Offset of the first byte not yet authenticated.
    pub const fn next_offset(self) -> u64 {
        self.next_offset
    }
    /// Whether every byte of the bound ELF has been authenticated.
    pub const fn is_complete(self) -> bool {
        self.next_offset == self.total_length
    }

    /// Verify the next complete slice and derive its successor without mutation.
    ///
    /// The runtime caller must borrow `chunk` from the native Loader-owned
    /// ProgramData after rechecking its pinned identity, slot, authority and
    /// total length. There is deliberately no API accepting a caller's digest.
    pub fn advance(self, offset: u64, chunk: &[u8]) -> Result<Self, CodeCommitmentErrorV2> {
        if offset != self.next_offset {
            return Err(CodeCommitmentErrorV2::Offset);
        }
        let remaining = self
            .total_length
            .checked_sub(self.next_offset)
            .ok_or(CodeCommitmentErrorV2::Encoding)?;
        let width = u64::try_from(CODE_COMMITMENT_CHUNK_BYTES_V2)
            .map_err(|_| CodeCommitmentErrorV2::Arithmetic)?;
        let actual = u64::try_from(chunk.len()).map_err(|_| CodeCommitmentErrorV2::Arithmetic)?;
        if actual == 0 || actual != remaining.min(width) {
            return Err(CodeCommitmentErrorV2::ChunkLength);
        }
        let chunk_digest = digest(chunk);
        Ok(Self {
            total_length: self.total_length,
            next_offset: offset
                .checked_add(actual)
                .ok_or(CodeCommitmentErrorV2::Arithmetic)?,
            rolling: digestv(&[
                CHUNK_DOMAIN,
                &self.rolling,
                &offset.to_le_bytes(),
                &actual.to_le_bytes(),
                &chunk_digest,
            ]),
        })
    }

    /// Obtain the code commitment only after complete contiguous coverage.
    pub fn commitment(self) -> Result<[u8; 32], CodeCommitmentErrorV2> {
        if !self.is_complete() {
            return Err(CodeCommitmentErrorV2::Incomplete);
        }
        Ok(self.rolling)
    }

    /// Encode the exact temporary progress tail.
    pub fn to_bytes(self) -> [u8; CODE_COMMITMENT_PROGRESS_BYTES_V2] {
        let mut output = [0; CODE_COMMITMENT_PROGRESS_BYTES_V2];
        output[..8].copy_from_slice(&self.total_length.to_le_bytes());
        output[8..16].copy_from_slice(&self.next_offset.to_le_bytes());
        output[16..].copy_from_slice(&self.rolling);
        output
    }

    /// Decode canonical progress from an authenticated Registry staging account.
    ///
    /// Encoding validation cannot authenticate a past rolling digest: the
    /// account's Registry ownership and uninterrupted staging identity do that.
    pub fn decode(bytes: &[u8]) -> Result<Self, CodeCommitmentErrorV2> {
        if bytes.len() != CODE_COMMITMENT_PROGRESS_BYTES_V2 {
            return Err(CodeCommitmentErrorV2::Encoding);
        }
        let total_length = u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| CodeCommitmentErrorV2::Encoding)?,
        );
        let next_offset = u64::from_le_bytes(
            bytes[8..16]
                .try_into()
                .map_err(|_| CodeCommitmentErrorV2::Encoding)?,
        );
        let rolling = bytes[16..]
            .try_into()
            .map_err(|_| CodeCommitmentErrorV2::Encoding)?;
        let initial = Self::new(total_length)?;
        let width = u64::try_from(CODE_COMMITMENT_CHUNK_BYTES_V2)
            .map_err(|_| CodeCommitmentErrorV2::Arithmetic)?;
        if next_offset > total_length
            || rolling == [0; 32]
            || (next_offset != total_length && next_offset % width != 0)
            || (next_offset == 0 && rolling != initial.rolling)
        {
            return Err(CodeCommitmentErrorV2::Encoding);
        }
        Ok(Self {
            total_length,
            next_offset,
            rolling,
        })
    }
}

/// Compute the canonical code commitment from the complete actual ELF tail.
///
/// Host producers use this full traversal. On-chain finalization calls one
/// `advance` per transaction and persists its state in the existing cursor.
pub fn code_commitment_v2(elf: &[u8]) -> Result<[u8; 32], CodeCommitmentErrorV2> {
    let mut progress = CodeCommitmentProgressV2::new(
        u64::try_from(elf.len()).map_err(|_| CodeCommitmentErrorV2::Arithmetic)?,
    )?;
    for chunk in elf.chunks(CODE_COMMITMENT_CHUNK_BYTES_V2) {
        progress = progress.advance(progress.next_offset(), chunk)?;
    }
    progress.commitment()
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    #[test]
    fn commitment_requires_exact_order_length_and_complete_coverage() {
        let bytes = vec![0x5a; CODE_COMMITMENT_CHUNK_BYTES_V2 + 17];
        let initial = CodeCommitmentProgressV2::new(u64::try_from(bytes.len()).expect("length"))
            .expect("initial");
        assert_eq!(initial.commitment(), Err(CodeCommitmentErrorV2::Incomplete));
        assert_eq!(
            initial.advance(1, &bytes[..CODE_COMMITMENT_CHUNK_BYTES_V2]),
            Err(CodeCommitmentErrorV2::Offset)
        );
        assert_eq!(
            initial.advance(0, &bytes[..17]),
            Err(CodeCommitmentErrorV2::ChunkLength)
        );
        let first = initial
            .advance(0, &bytes[..CODE_COMMITMENT_CHUNK_BYTES_V2])
            .expect("first");
        assert_eq!(
            first.advance(0, &bytes[..CODE_COMMITMENT_CHUNK_BYTES_V2]),
            Err(CodeCommitmentErrorV2::Offset)
        );
        let second = first
            .advance(
                first.next_offset(),
                &bytes[CODE_COMMITMENT_CHUNK_BYTES_V2..],
            )
            .expect("second");
        assert_eq!(
            CodeCommitmentProgressV2::decode(&second.to_bytes()),
            Ok(second)
        );
        assert_eq!(second.commitment(), code_commitment_v2(&bytes));
        let mut altered = bytes.clone();
        altered[0] ^= 1;
        assert_ne!(code_commitment_v2(&altered), second.commitment());
        assert_ne!(
            code_commitment_v2(&bytes[..bytes.len() - 1]),
            second.commitment()
        );
        let mut extended = bytes;
        extended.push(0);
        assert_ne!(code_commitment_v2(&extended), second.commitment());
    }
}
