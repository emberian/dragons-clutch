//! The ensemble fold's receipt: which members answered, and what they folded to.
//!
//! One record, written once by the fold beside the terminal certificate, so
//! that after retirement closes the fragment seats a reader can still say which
//! of `k` sources answered and recompute the fold's `provider_evidence` from
//! the consumed fragments' digests. Every field is a projection of a fact the
//! fold committed elsewhere; the record's own canonicity is the agreement
//! between its bitmap, its count and its digests, which the Lean owner
//! (`EnsembleFoldReceiptV1Abi`) states and this decoder enforces.

use core::convert::TryInto;

use super::{
    ContentId, Error, Result,
    generated_ensemble_fold_receipt_v1::{
        ENSEMBLE_FOLD_RECEIPT_V1_BYTES, ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_BITMAP_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_COUNT_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_0_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_1_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_2_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_3_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_4_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_GENERATION_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_MAGIC,
        ENSEMBLE_FOLD_RECEIPT_V1_MAGIC_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_MARKET_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_MEDIAN_NUMERATOR_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_MEMBER_COUNT_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_QUORUM_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_HEADER_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_SELECTOR_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_SCHEMA_VERSION,
        ENSEMBLE_FOLD_RECEIPT_V1_SELECTOR_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_SOURCE_MATERIAL_OFFSET,
        ENSEMBLE_FOLD_RECEIPT_V1_TERMINAL_SEQUENCE_OFFSET, ENSEMBLE_FOLD_RECEIPT_V1_VERSION_OFFSET,
        ENSEMBLE_MAX_MEMBERS_V1,
    },
};

const ID_BYTES: usize = 32;
const RESERVED_HEADER_BYTES: usize = 2;
const RESERVED_SELECTOR_BYTES: usize = 4;
const FRAGMENT_DIGEST_STRIDE: usize = 32;

// This decoder reads the five digests by ONE stride from member zero's offset,
// which is a second author of where members one through four sit unless the
// two agree here. The Lean owner emits each offset by name; these are that
// emission, checked against the stride at compile time.
const _: () = assert!(
    ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_1_OFFSET
        == ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_0_OFFSET + FRAGMENT_DIGEST_STRIDE
);
const _: () = assert!(
    ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_2_OFFSET
        == ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_1_OFFSET + FRAGMENT_DIGEST_STRIDE
);
const _: () = assert!(
    ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_3_OFFSET
        == ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_2_OFFSET + FRAGMENT_DIGEST_STRIDE
);
const _: () = assert!(
    ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_4_OFFSET
        == ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_3_OFFSET + FRAGMENT_DIGEST_STRIDE
);
const _: () = assert!(
    ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_4_OFFSET + FRAGMENT_DIGEST_STRIDE
        == ENSEMBLE_FOLD_RECEIPT_V1_BYTES
);

/// The fold's durable receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnsembleFoldReceiptV1 {
    /// `k`, the sources the material declared.
    pub member_count: u8,
    /// `q`, the fragments the fold needed.
    pub quorum: u8,
    /// `n`, the fragments the fold consumed.
    pub consumed_count: u8,
    /// Bit `m` set exactly when member `m`'s fragment was consumed.
    pub consumed_bitmap: u8,
    /// The Core Market.
    pub market: [u8; 32],
    /// The Market generation.
    pub generation: u64,
    /// The terminal sequence the certificate and this receipt were written at.
    pub terminal_sequence: u64,
    /// The `SourceMaterialV3` content identity the fold ran under.
    pub source_material: [u8; 32],
    /// The median reading on the material's one scale, the certificate's
    /// `result_numerator`.
    pub median_numerator: i128,
    /// The cell the median fell in, the certificate's `selector`.
    pub selector: u32,
    /// Each consumed member's fragment digest in member order; zero for a
    /// member that did not answer.
    pub fragment_digests: [[u8; 32]; ENSEMBLE_MAX_MEMBERS_V1 as usize],
}

impl EnsembleFoldReceiptV1 {
    /// Whether member `member` was consumed by the fold.
    pub const fn consumed(self, member: u8) -> bool {
        member < self.member_count && (self.consumed_bitmap >> member) & 1 == 1
    }

    /// Hostile-decode one exact 280-byte receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ENSEMBLE_FOLD_RECEIPT_V1_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, ENSEMBLE_FOLD_RECEIPT_V1_MAGIC_OFFSET)?
            != ENSEMBLE_FOLD_RECEIPT_V1_MAGIC
        {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, ENSEMBLE_FOLD_RECEIPT_V1_VERSION_OFFSET)?)
            != ENSEMBLE_FOLD_RECEIPT_V1_SCHEMA_VERSION
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_HEADER_OFFSET,
            RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            bytes,
            ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_SELECTOR_OFFSET,
            RESERVED_SELECTOR_BYTES,
        )?;
        let mut fragment_digests = [[0_u8; 32]; ENSEMBLE_MAX_MEMBERS_V1 as usize];
        for (member, digest) in fragment_digests.iter_mut().enumerate() {
            let offset = ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_0_OFFSET
                .checked_add(
                    member
                        .checked_mul(FRAGMENT_DIGEST_STRIDE)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            *digest = array::<ID_BYTES>(bytes, offset)?;
        }
        let value = Self {
            member_count: byte(bytes, ENSEMBLE_FOLD_RECEIPT_V1_MEMBER_COUNT_OFFSET)?,
            quorum: byte(bytes, ENSEMBLE_FOLD_RECEIPT_V1_QUORUM_OFFSET)?,
            consumed_count: byte(bytes, ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_COUNT_OFFSET)?,
            consumed_bitmap: byte(bytes, ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_BITMAP_OFFSET)?,
            market: array(bytes, ENSEMBLE_FOLD_RECEIPT_V1_MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(
                bytes,
                ENSEMBLE_FOLD_RECEIPT_V1_GENERATION_OFFSET,
            )?),
            terminal_sequence: u64::from_le_bytes(array(
                bytes,
                ENSEMBLE_FOLD_RECEIPT_V1_TERMINAL_SEQUENCE_OFFSET,
            )?),
            source_material: array(bytes, ENSEMBLE_FOLD_RECEIPT_V1_SOURCE_MATERIAL_OFFSET)?,
            median_numerator: i128::from_le_bytes(array(
                bytes,
                ENSEMBLE_FOLD_RECEIPT_V1_MEDIAN_NUMERATOR_OFFSET,
            )?),
            selector: u32::from_le_bytes(array(bytes, ENSEMBLE_FOLD_RECEIPT_V1_SELECTOR_OFFSET)?),
            fragment_digests,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode the exact canonical receipt, refusing a shape the decoder would.
    pub fn to_bytes(self) -> Result<[u8; ENSEMBLE_FOLD_RECEIPT_V1_BYTES]> {
        self.validate_shape()?;
        let mut output = [0_u8; ENSEMBLE_FOLD_RECEIPT_V1_BYTES];
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_MAGIC_OFFSET,
            &ENSEMBLE_FOLD_RECEIPT_V1_MAGIC,
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_VERSION_OFFSET,
            &ENSEMBLE_FOLD_RECEIPT_V1_SCHEMA_VERSION.to_le_bytes(),
        );
        output[ENSEMBLE_FOLD_RECEIPT_V1_MEMBER_COUNT_OFFSET] = self.member_count;
        output[ENSEMBLE_FOLD_RECEIPT_V1_QUORUM_OFFSET] = self.quorum;
        output[ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_COUNT_OFFSET] = self.consumed_count;
        output[ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_BITMAP_OFFSET] = self.consumed_bitmap;
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_MARKET_OFFSET,
            &self.market,
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_TERMINAL_SEQUENCE_OFFSET,
            &self.terminal_sequence.to_le_bytes(),
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_SOURCE_MATERIAL_OFFSET,
            &self.source_material,
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_MEDIAN_NUMERATOR_OFFSET,
            &self.median_numerator.to_le_bytes(),
        );
        put(
            &mut output,
            ENSEMBLE_FOLD_RECEIPT_V1_SELECTOR_OFFSET,
            &self.selector.to_le_bytes(),
        );
        for (member, digest) in self.fragment_digests.iter().enumerate() {
            let offset = ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_0_OFFSET
                .saturating_add(member.saturating_mul(FRAGMENT_DIGEST_STRIDE));
            put(&mut output, offset, digest);
        }
        Ok(output)
    }

    /// The bitmap, the count and the digests are one fact stated three ways.
    fn validate_shape(self) -> Result<()> {
        if self.member_count < 2
            || self.member_count > ENSEMBLE_MAX_MEMBERS_V1
            || self.quorum == 0
            || self.quorum > self.member_count
            || self.consumed_count < self.quorum
            || self.consumed_count > self.member_count
            || u32::from(self.consumed_bitmap) >= 1_u32 << u32::from(self.member_count)
            || self.consumed_bitmap.count_ones() != u32::from(self.consumed_count)
        {
            return Err(Error::NonCanonicalEnsemble);
        }
        if self.market.iter().all(|byte| *byte == 0)
            || self.source_material.iter().all(|byte| *byte == 0)
            || self.generation == 0
            || self.terminal_sequence == 0
        {
            return Err(Error::ZeroContentId);
        }
        if self.selector > u32::from(u8::MAX) {
            return Err(Error::InvalidResultSelector);
        }
        for (member, digest) in self.fragment_digests.iter().enumerate() {
            let consumed = member < usize::from(self.member_count)
                && (self.consumed_bitmap >> member) & 1 == 1;
            let nonzero = digest.iter().any(|byte| *byte != 0);
            if consumed != nonzero {
                return Err(Error::NonCanonicalEnsemble);
            }
        }
        Ok(())
    }

    /// The consumed digests as content identities, in member order.
    pub fn consumed_digests(self) -> Result<[Option<ContentId>; ENSEMBLE_MAX_MEMBERS_V1 as usize]> {
        let mut out = [None; ENSEMBLE_MAX_MEMBERS_V1 as usize];
        for (member, slot) in out.iter_mut().enumerate() {
            let index = u8::try_from(member).map_err(|_| Error::ArithmeticOverflow)?;
            if self.consumed(index) {
                let digest = self
                    .fragment_digests
                    .get(member)
                    .ok_or(Error::ArithmeticOverflow)?;
                *slot = Some(ContentId::new(*digest)?);
            }
        }
        Ok(out)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(slot) = output.get_mut(offset..offset.saturating_add(value.len())) {
        slot.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_ensemble_fold_receipt_v1::{
        ENSEMBLE_FOLD_RECEIPT_V1_EXAMPLE, ENSEMBLE_FOLD_RECEIPT_V1_PARTIAL_EXAMPLE,
        ENSEMBLE_FOLD_RECEIPT_V1_REFUSAL_CORPUS, ENSEMBLE_FOLD_RECEIPT_V1_REFUSAL_COUNT,
    };

    fn id(tag: u8) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        bytes
    }

    fn example() -> EnsembleFoldReceiptV1 {
        EnsembleFoldReceiptV1 {
            member_count: 3,
            quorum: 3,
            consumed_count: 3,
            consumed_bitmap: 0b111,
            market: id(11),
            generation: 7,
            terminal_sequence: 1,
            source_material: id(12),
            median_numerator: 10_375_000_000,
            selector: 1,
            fragment_digests: [id(21), id(22), id(23), [0; 32], [0; 32]],
        }
    }

    #[test]
    fn lean_generated_examples_agree() {
        assert_eq!(
            example().to_bytes().expect("example"),
            ENSEMBLE_FOLD_RECEIPT_V1_EXAMPLE
        );
        let partial = EnsembleFoldReceiptV1 {
            quorum: 2,
            consumed_count: 2,
            consumed_bitmap: 0b101,
            fragment_digests: [id(21), [0; 32], id(23), [0; 32], [0; 32]],
            ..example()
        };
        assert_eq!(
            partial.to_bytes().expect("partial"),
            ENSEMBLE_FOLD_RECEIPT_V1_PARTIAL_EXAMPLE
        );
        assert_eq!(
            EnsembleFoldReceiptV1::decode(&ENSEMBLE_FOLD_RECEIPT_V1_EXAMPLE),
            Ok(example())
        );
        assert_eq!(
            EnsembleFoldReceiptV1::decode(&ENSEMBLE_FOLD_RECEIPT_V1_PARTIAL_EXAMPLE),
            Ok(partial)
        );
        assert!(partial.consumed(0) && !partial.consumed(1) && partial.consumed(2));
        assert!(
            !partial.consumed(3),
            "a member the record does not declare was not consumed"
        );
    }

    #[test]
    fn lean_generated_refusal_corpus_fails_closed() {
        assert_eq!(
            ENSEMBLE_FOLD_RECEIPT_V1_REFUSAL_CORPUS.len(),
            ENSEMBLE_FOLD_RECEIPT_V1_REFUSAL_COUNT
        );
        for hostile in ENSEMBLE_FOLD_RECEIPT_V1_REFUSAL_CORPUS {
            assert!(EnsembleFoldReceiptV1::decode(&hostile).is_err());
        }
    }

    /// The bitmap and the count are one fact: a receipt cannot claim three
    /// answered over a bitmap naming two, in either direction.
    #[test]
    fn the_bitmap_and_the_count_are_one_fact() {
        let short = EnsembleFoldReceiptV1 {
            consumed_bitmap: 0b011,
            ..example()
        };
        assert_eq!(short.to_bytes(), Err(Error::NonCanonicalEnsemble));
        let unset_digest = EnsembleFoldReceiptV1 {
            fragment_digests: [id(21), id(22), [0; 32], [0; 32], [0; 32]],
            ..example()
        };
        assert_eq!(unset_digest.to_bytes(), Err(Error::NonCanonicalEnsemble));
    }
}
