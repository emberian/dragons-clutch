//! Exact Pyth Receiver SDK 2.0.0 `Config` account decoding.

/// Anchor account discriminator for Pyth Receiver SDK 2.0.0 `Config`.
pub const RECEIVER_CONFIG_V2_DISCRIMINATOR: [u8; 8] =
    [0x9b, 0x0c, 0xaa, 0xe0, 0x1e, 0xfa, 0xcc, 0x82];

/// Exact allocated size of the Pyth Receiver SDK 2.0.0 `Config` account.
pub const RECEIVER_CONFIG_V2_LEN: usize = 370;

/// Exact Borsh width of one `DataSource` element.
pub const DATA_SOURCE_V2_LEN: usize = 34;

/// Error returned while parsing an untrusted receiver `Config` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiverConfigV2Error {
    /// The account did not have the exact allocated V2 width.
    InvalidLength {
        /// Observed number of bytes.
        actual: usize,
    },
    /// The account discriminator was not the receiver V2 `Config` discriminator.
    InvalidDiscriminator,
    /// The Borsh `Option<Pubkey>` tag was neither zero nor one.
    InvalidTargetGovernanceTag {
        /// Observed option tag.
        tag: u8,
    },
    /// A checked Borsh field or vector extended beyond the account.
    UnexpectedEof {
        /// Byte offset at which the field began.
        offset: usize,
        /// Number of bytes requested at that offset.
        requested: usize,
    },
    /// The encoded data-source count could not be represented or multiplied safely.
    DataSourceLengthOverflow {
        /// Encoded number of data sources.
        count: u32,
    },
    /// A byte after the serialized Config body was not canonical zero allocation tail.
    NonzeroAllocationTail {
        /// Absolute account offset of the first nonzero tail byte.
        offset: usize,
        /// Observed nonzero byte.
        value: u8,
    },
}

/// Result alias for exact receiver Config parsing.
pub type ReceiverConfigV2Result<T> = core::result::Result<T, ReceiverConfigV2Error>;

/// One decoded fixed-width receiver data-source entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataSourceV2View {
    emitter_chain: u16,
    emitter_address: [u8; 32],
}

impl DataSourceV2View {
    /// Return the Wormhole emitter-chain identifier.
    pub const fn emitter_chain(&self) -> u16 {
        self.emitter_chain
    }

    /// Return the Wormhole emitter address.
    pub const fn emitter_address(&self) -> [u8; 32] {
        self.emitter_address
    }
}

/// Borrowed, allocation-free view of an exact receiver V2 `Config` account.
///
/// This validates only the serialized account shape. An SVM adapter must also
/// authenticate the account key, owner, release digest, and router program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiverConfigV2View<'a> {
    governance_authority: [u8; 32],
    target_governance_authority: Option<[u8; 32]>,
    router_program: [u8; 32],
    data_sources: &'a [u8],
    data_source_count: u32,
    fee: u64,
    minimum_signatures: u8,
}

impl<'a> ReceiverConfigV2View<'a> {
    /// Parse one exact receiver SDK 2.0.0 Config account.
    pub fn parse(bytes: &'a [u8]) -> ReceiverConfigV2Result<Self> {
        if bytes.len() != RECEIVER_CONFIG_V2_LEN {
            return Err(ReceiverConfigV2Error::InvalidLength {
                actual: bytes.len(),
            });
        }

        let mut cursor = BorshCursor::new(bytes);
        if cursor.take_array::<8>()? != RECEIVER_CONFIG_V2_DISCRIMINATOR {
            return Err(ReceiverConfigV2Error::InvalidDiscriminator);
        }
        let governance_authority = cursor.take_array()?;
        let target_governance_authority = match cursor.take_u8()? {
            0 => None,
            1 => Some(cursor.take_array()?),
            tag => return Err(ReceiverConfigV2Error::InvalidTargetGovernanceTag { tag }),
        };
        let router_program = cursor.take_array()?;
        let data_source_count = cursor.take_u32()?;
        let count = usize::try_from(data_source_count).map_err(|_| {
            ReceiverConfigV2Error::DataSourceLengthOverflow {
                count: data_source_count,
            }
        })?;
        let data_source_bytes = count.checked_mul(DATA_SOURCE_V2_LEN).ok_or(
            ReceiverConfigV2Error::DataSourceLengthOverflow {
                count: data_source_count,
            },
        )?;
        let data_sources = cursor.take(data_source_bytes)?;
        let fee = cursor.take_u64()?;
        let minimum_signatures = cursor.take_u8()?;

        let tail_offset = cursor.offset();
        for (relative_offset, value) in cursor.remaining().iter().copied().enumerate() {
            if value != 0 {
                let offset = tail_offset.checked_add(relative_offset).ok_or(
                    ReceiverConfigV2Error::UnexpectedEof {
                        offset: tail_offset,
                        requested: relative_offset,
                    },
                )?;
                return Err(ReceiverConfigV2Error::NonzeroAllocationTail { offset, value });
            }
        }

        Ok(Self {
            governance_authority,
            target_governance_authority,
            router_program,
            data_sources,
            data_source_count,
            fee,
            minimum_signatures,
        })
    }

    /// Return the active governance authority.
    pub const fn governance_authority(&self) -> [u8; 32] {
        self.governance_authority
    }

    /// Return the optional pending governance authority.
    pub const fn target_governance_authority(&self) -> Option<[u8; 32]> {
        self.target_governance_authority
    }

    /// Return the serialized Wormhole/router program public key.
    pub const fn router_program(&self) -> [u8; 32] {
        self.router_program
    }

    /// Return the number of serialized valid data sources.
    pub const fn data_source_count(&self) -> u32 {
        self.data_source_count
    }

    /// Return one decoded data source, or `None` when `index` is out of range.
    pub fn data_source(&self, index: u32) -> Option<DataSourceV2View> {
        if index >= self.data_source_count {
            return None;
        }
        let index = usize::try_from(index).ok()?;
        let start = index.checked_mul(DATA_SOURCE_V2_LEN)?;
        let end = start.checked_add(DATA_SOURCE_V2_LEN)?;
        let bytes = self.data_sources.get(start..end)?;
        let chain_bytes: [u8; 2] = bytes.get(0..2)?.try_into().ok()?;
        let emitter_address = bytes.get(2..34)?.try_into().ok()?;
        Some(DataSourceV2View {
            emitter_chain: u16::from_le_bytes(chain_bytes),
            emitter_address,
        })
    }

    /// Return the provider fee in lamports encoded by the receiver Config.
    pub const fn fee(&self) -> u64 {
        self.fee
    }

    /// Return the receiver Config's local minimum-signatures policy.
    ///
    /// This is not the release contract's router guardian-set cardinality or
    /// strict-majority fact; the release binds the complete Config digest.
    pub const fn minimum_signatures(&self) -> u8 {
        self.minimum_signatures
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BorshCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BorshCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> ReceiverConfigV2Result<&'a [u8]> {
        let start = self.offset;
        let end = start
            .checked_add(length)
            .ok_or(ReceiverConfigV2Error::UnexpectedEof {
                offset: start,
                requested: length,
            })?;
        let value = self
            .bytes
            .get(start..end)
            .ok_or(ReceiverConfigV2Error::UnexpectedEof {
                offset: start,
                requested: length,
            })?;
        self.offset = end;
        Ok(value)
    }

    fn take_array<const N: usize>(&mut self) -> ReceiverConfigV2Result<[u8; N]> {
        let start = self.offset;
        self.take(N)?
            .try_into()
            .map_err(|_| ReceiverConfigV2Error::UnexpectedEof {
                offset: start,
                requested: N,
            })
    }

    fn take_u8(&mut self) -> ReceiverConfigV2Result<u8> {
        let start = self.offset;
        self.take(1)?
            .first()
            .copied()
            .ok_or(ReceiverConfigV2Error::UnexpectedEof {
                offset: start,
                requested: 1,
            })
    }

    fn take_u32(&mut self) -> ReceiverConfigV2Result<u32> {
        Ok(u32::from_le_bytes(self.take_array()?))
    }

    fn take_u64(&mut self) -> ReceiverConfigV2Result<u64> {
        Ok(u64::from_le_bytes(self.take_array()?))
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn remaining(&self) -> &'a [u8] {
        self.bytes.get(self.offset..).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURED_CONFIG: &[u8; RECEIVER_CONFIG_V2_LEN] =
        include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");

    #[test]
    fn captured_config_exposes_real_router_fee_minimum_policy_and_source()
    -> ReceiverConfigV2Result<()> {
        let view = ReceiverConfigV2View::parse(CAPTURED_CONFIG)?;
        assert_eq!(view.target_governance_authority(), None);
        assert_ne!(view.governance_authority(), [0_u8; 32]);
        assert_eq!(
            view.router_program(),
            [
                0xf1, 0x0b, 0x0a, 0xdc, 0x78, 0x68, 0xf4, 0x55, 0x66, 0x57, 0xa9, 0x05, 0xf7, 0x14,
                0x45, 0xce, 0xec, 0x42, 0x07, 0xac, 0x77, 0xd7, 0xc5, 0xc2, 0xb7, 0x62, 0xdf, 0x13,
                0x94, 0x66, 0x4b, 0x87,
            ]
        );
        assert_eq!(view.data_source_count(), 1);
        assert_eq!(
            view.data_source(0),
            Some(DataSourceV2View {
                emitter_chain: 1,
                emitter_address: [1_u8; 32],
            })
        );
        assert_eq!(view.data_source(1), None);
        assert_eq!(view.fee(), 1);
        assert_eq!(view.minimum_signatures(), 5);
        Ok(())
    }

    #[test]
    fn exact_allocation_discriminator_and_option_tag_are_required() {
        for length in 0..RECEIVER_CONFIG_V2_LEN {
            let truncated = CAPTURED_CONFIG.get(..length).unwrap_or_default();
            assert_eq!(
                ReceiverConfigV2View::parse(truncated),
                Err(ReceiverConfigV2Error::InvalidLength { actual: length })
            );
        }
        let mut long = [0_u8; RECEIVER_CONFIG_V2_LEN + 1];
        if let Some(prefix) = long.get_mut(..RECEIVER_CONFIG_V2_LEN) {
            prefix.copy_from_slice(CAPTURED_CONFIG);
        }
        assert_eq!(
            ReceiverConfigV2View::parse(&long),
            Err(ReceiverConfigV2Error::InvalidLength {
                actual: RECEIVER_CONFIG_V2_LEN + 1
            })
        );

        let mut wrong_discriminator = *CAPTURED_CONFIG;
        wrong_discriminator[0] ^= 1;
        assert_eq!(
            ReceiverConfigV2View::parse(&wrong_discriminator),
            Err(ReceiverConfigV2Error::InvalidDiscriminator)
        );
        let mut wrong_option = *CAPTURED_CONFIG;
        wrong_option[40] = 2;
        assert_eq!(
            ReceiverConfigV2View::parse(&wrong_option),
            Err(ReceiverConfigV2Error::InvalidTargetGovernanceTag { tag: 2 })
        );
    }

    #[test]
    fn some_target_governance_shifts_following_borsh_fields_exactly() -> ReceiverConfigV2Result<()>
    {
        let mut bytes = [0_u8; RECEIVER_CONFIG_V2_LEN];
        bytes[0..8].copy_from_slice(&RECEIVER_CONFIG_V2_DISCRIMINATOR);
        bytes[8..40].copy_from_slice(&[1_u8; 32]);
        bytes[40] = 1;
        bytes[41..73].copy_from_slice(&[2_u8; 32]);
        bytes[73..105].copy_from_slice(&[3_u8; 32]);
        bytes[105..109].copy_from_slice(&0_u32.to_le_bytes());
        bytes[109..117].copy_from_slice(&9_u64.to_le_bytes());
        bytes[117] = 4;

        let view = ReceiverConfigV2View::parse(&bytes)?;
        assert_eq!(view.governance_authority(), [1_u8; 32]);
        assert_eq!(view.target_governance_authority(), Some([2_u8; 32]));
        assert_eq!(view.router_program(), [3_u8; 32]);
        assert_eq!(view.data_source_count(), 0);
        assert_eq!(view.fee(), 9);
        assert_eq!(view.minimum_signatures(), 4);
        Ok(())
    }

    #[test]
    fn hostile_vector_counts_and_nonzero_allocation_tail_refuse() {
        let mut too_many_sources = *CAPTURED_CONFIG;
        too_many_sources[73..77].copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(
            ReceiverConfigV2View::parse(&too_many_sources),
            Err(ReceiverConfigV2Error::UnexpectedEof {
                offset: 77,
                requested: 9 * DATA_SOURCE_V2_LEN
            })
        );

        let mut maximal_count = *CAPTURED_CONFIG;
        maximal_count[73..77].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            ReceiverConfigV2View::parse(&maximal_count),
            Err(ReceiverConfigV2Error::UnexpectedEof { offset: 77, .. })
                | Err(ReceiverConfigV2Error::DataSourceLengthOverflow { .. })
        ));

        let mut nonzero_tail = *CAPTURED_CONFIG;
        nonzero_tail[120] = 0x5a;
        assert_eq!(
            ReceiverConfigV2View::parse(&nonzero_tail),
            Err(ReceiverConfigV2Error::NonzeroAllocationTail {
                offset: 120,
                value: 0x5a
            })
        );
    }
}
