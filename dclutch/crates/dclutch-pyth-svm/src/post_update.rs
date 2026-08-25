//! Exact borrowed decoding for the Pyth Receiver `PostUpdateParams` body.

/// Exact byte width of one Merkle proof element.
pub const POST_UPDATE_PROOF_ELEMENT_LEN: usize = 20;

/// Error returned while parsing an untrusted `PostUpdateParams` body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostUpdateParamsError {
    /// A checked field or vector extended beyond the instruction body.
    UnexpectedEof {
        /// Byte offset at which the field began.
        offset: usize,
        /// Number of bytes requested at that offset.
        requested: usize,
    },
    /// The encoded message length could not be represented by the target.
    MessageLengthOverflow {
        /// Encoded message length.
        length: u32,
    },
    /// The proof count could not be represented or multiplied safely.
    ProofLengthOverflow {
        /// Encoded number of proof elements.
        count: u32,
    },
    /// Bytes remained after the final one-byte treasury identifier.
    TrailingBytes {
        /// Number of unconsumed trailing bytes.
        count: usize,
    },
}

/// Result alias for exact `PostUpdateParams` parsing.
pub type PostUpdateParamsResult<T> = core::result::Result<T, PostUpdateParamsError>;

/// Allocation-free borrowed view of one exact `PostUpdateParams` body.
///
/// The eight-byte Anchor instruction discriminator is outside this body. V1
/// accepts any message or proof size representable by the wire format and
/// checked target arithmetic; it imposes no smaller provisional ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostUpdateParamsView<'a> {
    message: &'a [u8],
    proof: &'a [u8],
    proof_count: u32,
    treasury_id: u8,
}

impl<'a> PostUpdateParamsView<'a> {
    /// Parse one exact body through final treasury byte and exact EOF.
    pub fn parse(body: &'a [u8]) -> PostUpdateParamsResult<Self> {
        let mut cursor = WireCursor::new(body);
        let message_length = cursor.take_u32()?;
        let message_length = usize::try_from(message_length).map_err(|_| {
            PostUpdateParamsError::MessageLengthOverflow {
                length: message_length,
            }
        })?;
        let message = cursor.take(message_length)?;

        let proof_count = cursor.take_u32()?;
        let count = usize::try_from(proof_count)
            .map_err(|_| PostUpdateParamsError::ProofLengthOverflow { count: proof_count })?;
        let proof_length = count
            .checked_mul(POST_UPDATE_PROOF_ELEMENT_LEN)
            .ok_or(PostUpdateParamsError::ProofLengthOverflow { count: proof_count })?;
        let proof = cursor.take(proof_length)?;
        let treasury_id = cursor.take_u8()?;

        let trailing = cursor.remaining_len();
        if trailing != 0 {
            return Err(PostUpdateParamsError::TrailingBytes { count: trailing });
        }
        Ok(Self {
            message,
            proof,
            proof_count,
            treasury_id,
        })
    }

    /// Return the borrowed accumulator message bytes.
    pub const fn message(&self) -> &'a [u8] {
        self.message
    }

    /// Return the number of 20-byte Merkle proof elements.
    pub const fn proof_count(&self) -> u32 {
        self.proof_count
    }

    /// Return one proof element, or `None` when `index` is out of range.
    pub fn proof_element(&self, index: u32) -> Option<[u8; POST_UPDATE_PROOF_ELEMENT_LEN]> {
        if index >= self.proof_count {
            return None;
        }
        let index = usize::try_from(index).ok()?;
        let start = index.checked_mul(POST_UPDATE_PROOF_ELEMENT_LEN)?;
        let end = start.checked_add(POST_UPDATE_PROOF_ELEMENT_LEN)?;
        self.proof.get(start..end)?.try_into().ok()
    }

    /// Return the final treasury identifier.
    pub const fn treasury_id(&self) -> u8 {
        self.treasury_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WireCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WireCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> PostUpdateParamsResult<&'a [u8]> {
        let start = self.offset;
        let end = start
            .checked_add(length)
            .ok_or(PostUpdateParamsError::UnexpectedEof {
                offset: start,
                requested: length,
            })?;
        let value = self
            .bytes
            .get(start..end)
            .ok_or(PostUpdateParamsError::UnexpectedEof {
                offset: start,
                requested: length,
            })?;
        self.offset = end;
        Ok(value)
    }

    fn take_u8(&mut self) -> PostUpdateParamsResult<u8> {
        let start = self.offset;
        self.take(1)?
            .first()
            .copied()
            .ok_or(PostUpdateParamsError::UnexpectedEof {
                offset: start,
                requested: 1,
            })
    }

    fn take_u32(&mut self) -> PostUpdateParamsResult<u32> {
        let start = self.offset;
        let bytes: [u8; 4] =
            self.take(4)?
                .try_into()
                .map_err(|_| PostUpdateParamsError::UnexpectedEof {
                    offset: start,
                    requested: 4,
                })?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn remaining_len(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURED_POST_UPDATE: &[u8; 102] = include_bytes!(
        "../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
    );

    #[test]
    fn captured_ninety_four_byte_body_is_exact_and_zero_proof_is_valid()
    -> PostUpdateParamsResult<()> {
        let body = CAPTURED_POST_UPDATE
            .get(8..)
            .ok_or(PostUpdateParamsError::UnexpectedEof {
                offset: 8,
                requested: 94,
            })?;
        assert_eq!(body.len(), 94);
        let view = PostUpdateParamsView::parse(body)?;
        assert_eq!(view.message().len(), 85);
        assert_eq!(view.message().first(), Some(&0));
        assert_eq!(view.proof_count(), 0);
        assert_eq!(view.proof_element(0), None);
        assert_eq!(view.treasury_id(), 0);
        Ok(())
    }

    #[test]
    fn one_proof_element_round_trips_without_allocation() -> PostUpdateParamsResult<()> {
        let mut body = [0_u8; 32];
        body[0..4].copy_from_slice(&3_u32.to_le_bytes());
        body[4..7].copy_from_slice(&[7, 8, 9]);
        body[7..11].copy_from_slice(&1_u32.to_le_bytes());
        body[11..31].copy_from_slice(&[0x5a; POST_UPDATE_PROOF_ELEMENT_LEN]);
        body[31] = 4;
        let view = PostUpdateParamsView::parse(&body)?;
        assert_eq!(view.message(), &[7, 8, 9]);
        assert_eq!(view.proof_count(), 1);
        assert_eq!(view.proof_element(0), Some([0x5a; 20]));
        assert_eq!(view.proof_element(1), None);
        assert_eq!(view.treasury_id(), 4);
        Ok(())
    }

    #[test]
    fn message_larger_than_old_prototype_ceiling_is_accepted() -> PostUpdateParamsResult<()> {
        const MESSAGE_LEN: usize = 1_024;
        const BODY_LEN: usize = 4 + MESSAGE_LEN + 4 + 1;
        let mut body = [0_u8; BODY_LEN];
        body[0..4].copy_from_slice(&1_024_u32.to_le_bytes());
        body[4..4 + MESSAGE_LEN].fill(0x3c);
        let proof_count_offset = 4 + MESSAGE_LEN;
        body.get_mut(proof_count_offset..proof_count_offset + 4)
            .ok_or(PostUpdateParamsError::UnexpectedEof {
                offset: proof_count_offset,
                requested: 4,
            })?
            .copy_from_slice(&0_u32.to_le_bytes());
        body[BODY_LEN - 1] = 6;

        let view = PostUpdateParamsView::parse(&body)?;
        assert_eq!(view.message().len(), MESSAGE_LEN);
        assert!(view.message().iter().all(|byte| *byte == 0x3c));
        assert_eq!(view.proof_count(), 0);
        assert_eq!(view.treasury_id(), 6);
        Ok(())
    }

    #[test]
    fn hostile_lengths_counts_and_trailing_bytes_refuse() {
        assert_eq!(
            PostUpdateParamsView::parse(&[]),
            Err(PostUpdateParamsError::UnexpectedEof {
                offset: 0,
                requested: 4
            })
        );

        let mut hostile_message = [0_u8; 9];
        hostile_message[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            PostUpdateParamsView::parse(&hostile_message),
            Err(PostUpdateParamsError::UnexpectedEof { offset: 4, .. })
                | Err(PostUpdateParamsError::MessageLengthOverflow { .. })
        ));

        let mut hostile_proof = [0_u8; 9];
        hostile_proof[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            PostUpdateParamsView::parse(&hostile_proof),
            Err(PostUpdateParamsError::UnexpectedEof { offset: 8, .. })
                | Err(PostUpdateParamsError::ProofLengthOverflow { .. })
        ));

        let mut trailing = [0_u8; 10];
        trailing[8] = 7;
        trailing[9] = 9;
        assert_eq!(
            PostUpdateParamsView::parse(&trailing),
            Err(PostUpdateParamsError::TrailingBytes { count: 1 })
        );

        let mut missing_treasury = [0_u8; 8];
        missing_treasury[0..4].copy_from_slice(&0_u32.to_le_bytes());
        missing_treasury[4..8].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            PostUpdateParamsView::parse(&missing_treasury),
            Err(PostUpdateParamsError::UnexpectedEof {
                offset: 8,
                requested: 1
            })
        );
    }
}
