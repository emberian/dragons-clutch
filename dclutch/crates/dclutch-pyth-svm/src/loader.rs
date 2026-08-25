//! Exact, unauthenticated Upgradeable Loader V3 byte views.
//!
//! An adapter must establish the expected loader owner, program executable
//! flag, expected Program and ProgramData public keys, and their linkage before
//! using these shape-only views.

/// Error returned while decoding an untrusted Upgradeable Loader V3 byte slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderV3Error {
    /// A Program account was not exactly 36 bytes.
    InvalidProgramLength {
        /// Observed number of input bytes.
        actual: usize,
    },
    /// A Program account did not have serialized enum variant two.
    InvalidProgramVariant {
        /// Observed little-endian enum variant.
        variant: u32,
    },
    /// A ProgramData account was too short to contain its variant, slot, and option tag.
    ProgramDataTooShort {
        /// Observed number of input bytes.
        actual: usize,
    },
    /// A ProgramData account did not have serialized enum variant three.
    InvalidProgramDataVariant {
        /// Observed little-endian enum variant.
        variant: u32,
    },
    /// The canonical `Option<Pubkey>` tag was neither zero nor one.
    InvalidUpgradeAuthorityTag {
        /// Observed option tag.
        tag: u8,
    },
    /// A `Some(Pubkey)` tag lacked all 32 public-key bytes.
    MissingUpgradeAuthority {
        /// Observed number of input bytes.
        actual: usize,
    },
    /// The ProgramData metadata had no byte remaining for ELF program data.
    EmptyElf,
}

/// Result alias for exact Upgradeable Loader V3 view parsing.
pub type LoaderV3Result<T> = core::result::Result<T, LoaderV3Error>;

/// Exact 36-byte Upgradeable Loader V3 Program account metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV3View {
    programdata_key: [u8; 32],
}

impl ProgramV3View {
    /// Parse one exact serialized Loader V3 Program account metadata block.
    pub fn parse(bytes: &[u8]) -> LoaderV3Result<Self> {
        if bytes.len() != 36 {
            return Err(LoaderV3Error::InvalidProgramLength {
                actual: bytes.len(),
            });
        }
        let variant = u32_at(bytes, 0)?;
        if variant != 2 {
            return Err(LoaderV3Error::InvalidProgramVariant { variant });
        }
        Ok(Self {
            programdata_key: array_at(bytes, 4)?,
        })
    }

    /// Return the serialized ProgramData account public key.
    pub const fn programdata_key(&self) -> [u8; 32] {
        self.programdata_key
    }
}

/// A borrowed exact Upgradeable Loader V3 ProgramData metadata and ELF view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramDataV3View<'a> {
    deployment_slot: u64,
    upgrade_authority: Option<[u8; 32]>,
    elf: &'a [u8],
}

impl<'a> ProgramDataV3View<'a> {
    /// Parse ProgramData metadata and require a nonempty byte tail for the ELF.
    pub fn parse(bytes: &'a [u8]) -> LoaderV3Result<Self> {
        if bytes.len() < 13 {
            return Err(LoaderV3Error::ProgramDataTooShort {
                actual: bytes.len(),
            });
        }
        let variant = u32_at(bytes, 0)?;
        if variant != 3 {
            return Err(LoaderV3Error::InvalidProgramDataVariant { variant });
        }
        let deployment_slot = u64_at(bytes, 4)?;
        let tag = byte_at(bytes, 12)?;
        let (upgrade_authority, elf_offset) = match tag {
            0 => (None, 13),
            1 => {
                if bytes.len() < 45 {
                    return Err(LoaderV3Error::MissingUpgradeAuthority {
                        actual: bytes.len(),
                    });
                }
                (Some(array_at(bytes, 13)?), 45)
            }
            _ => return Err(LoaderV3Error::InvalidUpgradeAuthorityTag { tag }),
        };
        let elf = bytes.get(elf_offset..).ok_or(LoaderV3Error::EmptyElf)?;
        if elf.is_empty() {
            return Err(LoaderV3Error::EmptyElf);
        }
        Ok(Self {
            deployment_slot,
            upgrade_authority,
            elf,
        })
    }

    /// Return the slot at which this ProgramData was last deployed.
    pub const fn deployment_slot(&self) -> u64 {
        self.deployment_slot
    }

    /// Return the optional serialized Loader V3 upgrade authority.
    pub const fn upgrade_authority(&self) -> Option<[u8; 32]> {
        self.upgrade_authority
    }

    /// Return the nonempty raw ELF byte tail.
    pub const fn elf(&self) -> &'a [u8] {
        self.elf
    }
}

fn byte_at(bytes: &[u8], offset: usize) -> LoaderV3Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })
}

fn u32_at(bytes: &[u8], offset: usize) -> LoaderV3Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?;
    let field: [u8; 4] = bytes
        .get(offset..end)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?;
    Ok(u32::from_le_bytes(field))
}

fn u64_at(bytes: &[u8], offset: usize) -> LoaderV3Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?;
    let field: [u8; 8] = bytes
        .get(offset..end)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?;
    Ok(u64::from_le_bytes(field))
}

fn array_at(bytes: &[u8], offset: usize) -> LoaderV3Result<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?;
    bytes
        .get(offset..end)
        .ok_or(LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| LoaderV3Error::ProgramDataTooShort {
            actual: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: [u8; N]) -> LoaderV3Result<()> {
        let actual = bytes.len();
        let end = offset
            .checked_add(N)
            .ok_or(LoaderV3Error::ProgramDataTooShort { actual })?;
        bytes
            .get_mut(offset..end)
            .ok_or(LoaderV3Error::ProgramDataTooShort { actual })?
            .copy_from_slice(&value);
        Ok(())
    }

    #[test]
    fn program_is_exact_and_little_endian() -> LoaderV3Result<()> {
        let mut bytes = [0_u8; 36];
        put(&mut bytes, 0, 2_u32.to_le_bytes())?;
        put(&mut bytes, 4, [0xa5; 32])?;
        let view = ProgramV3View::parse(&bytes)?;
        assert_eq!(view.programdata_key(), [0xa5; 32]);

        let short = bytes
            .get(..35)
            .ok_or(LoaderV3Error::InvalidProgramLength { actual: 35 })?;
        assert_eq!(
            ProgramV3View::parse(short),
            Err(LoaderV3Error::InvalidProgramLength { actual: 35 })
        );
        put(&mut bytes, 0, 3_u32.to_le_bytes())?;
        assert_eq!(
            ProgramV3View::parse(&bytes),
            Err(LoaderV3Error::InvalidProgramVariant { variant: 3 })
        );
        Ok(())
    }

    #[test]
    fn programdata_none_authority_has_thirteen_byte_prefix() -> LoaderV3Result<()> {
        let mut bytes = [0_u8; 15];
        put(&mut bytes, 0, 3_u32.to_le_bytes())?;
        put(&mut bytes, 4, 0x0102_0304_0506_0708_u64.to_le_bytes())?;
        put(&mut bytes, 12, [0])?;
        put(&mut bytes, 13, [0x7f, 0x45])?;
        let view = ProgramDataV3View::parse(&bytes)?;
        assert_eq!(view.deployment_slot(), 0x0102_0304_0506_0708);
        assert_eq!(view.upgrade_authority(), None);
        assert_eq!(view.elf(), [0x7f, 0x45]);
        Ok(())
    }

    #[test]
    fn programdata_some_authority_has_forty_five_byte_prefix() -> LoaderV3Result<()> {
        let mut bytes = [0_u8; 46];
        put(&mut bytes, 0, 3_u32.to_le_bytes())?;
        put(&mut bytes, 4, 99_u64.to_le_bytes())?;
        put(&mut bytes, 12, [1])?;
        put(&mut bytes, 13, [0x5a; 32])?;
        put(&mut bytes, 45, [0x7f])?;
        let view = ProgramDataV3View::parse(&bytes)?;
        assert_eq!(view.deployment_slot(), 99);
        assert_eq!(view.upgrade_authority(), Some([0x5a; 32]));
        assert_eq!(view.elf(), [0x7f]);
        Ok(())
    }

    #[test]
    fn hostile_programdata_prefixes_refuse() -> LoaderV3Result<()> {
        for length in 0..13 {
            let bytes = [0_u8; 13];
            let truncated = bytes
                .get(..length)
                .ok_or(LoaderV3Error::ProgramDataTooShort { actual: length })?;
            assert_eq!(
                ProgramDataV3View::parse(truncated),
                Err(LoaderV3Error::ProgramDataTooShort { actual: length })
            );
        }
        let mut wrong_variant = [0_u8; 14];
        put(&mut wrong_variant, 0, 2_u32.to_le_bytes())?;
        put(&mut wrong_variant, 12, [0])?;
        put(&mut wrong_variant, 13, [0x7f])?;
        assert_eq!(
            ProgramDataV3View::parse(&wrong_variant),
            Err(LoaderV3Error::InvalidProgramDataVariant { variant: 2 })
        );

        let mut invalid_tag = [0_u8; 14];
        put(&mut invalid_tag, 0, 3_u32.to_le_bytes())?;
        put(&mut invalid_tag, 12, [2])?;
        put(&mut invalid_tag, 13, [0x7f])?;
        assert_eq!(
            ProgramDataV3View::parse(&invalid_tag),
            Err(LoaderV3Error::InvalidUpgradeAuthorityTag { tag: 2 })
        );

        let mut missing_authority = [0_u8; 44];
        put(&mut missing_authority, 0, 3_u32.to_le_bytes())?;
        put(&mut missing_authority, 12, [1])?;
        assert_eq!(
            ProgramDataV3View::parse(&missing_authority),
            Err(LoaderV3Error::MissingUpgradeAuthority { actual: 44 })
        );

        let mut empty_elf_none = [0_u8; 13];
        put(&mut empty_elf_none, 0, 3_u32.to_le_bytes())?;
        assert_eq!(
            ProgramDataV3View::parse(&empty_elf_none),
            Err(LoaderV3Error::EmptyElf)
        );
        let mut empty_elf_some = [0_u8; 45];
        put(&mut empty_elf_some, 0, 3_u32.to_le_bytes())?;
        put(&mut empty_elf_some, 12, [1])?;
        assert_eq!(
            ProgramDataV3View::parse(&empty_elf_some),
            Err(LoaderV3Error::EmptyElf)
        );
        Ok(())
    }
}
