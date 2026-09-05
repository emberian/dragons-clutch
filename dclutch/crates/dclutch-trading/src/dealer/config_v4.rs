//! Immutable, acyclic Dealer config.
//!
//! The logical Market is deliberately absent. The finalized capability
//! manifest selects this config digest, while the Core Market PDA commits to
//! that manifest digest; persisting Market here would therefore require a
//! cryptographic fixed point. Common Trading Hot already authenticates the
//! Core Market and immutable child root, so that context remains the sole
//! Market authority. This record owns only facts that do not depend on the
//! Market address.

/// Canonical immutable Dealer config magic.
pub const DEALER_CONFIG_MAGIC_V4: [u8; 8] = *b"DCLDDC04";
/// Canonical immutable Dealer config ABI version.
pub const DEALER_CONFIG_VERSION_V4: u16 = 4;
/// Exact immutable Dealer config width.
pub const DEALER_CONFIG_BYTES_V4: usize = 128;
/// Canonical finalized-record schema label.
pub const DEALER_CONFIG_SCHEMA_PREIMAGE_V4: &[u8] = b"dclutch/schema/dealer-immutable-config-v4";

/// Selected release-set identity byte offset.
pub const DEALER_CONFIG_RELEASE_SET_OFFSET_V4: usize = 16;
/// Immutable Realm identity byte offset.
pub const DEALER_CONFIG_REALM_OFFSET_V4: usize = 48;
/// Canonical Dealer Claims Position owner byte offset.
pub const DEALER_CONFIG_POSITION_OWNER_OFFSET_V4: usize = 80;
/// Locked-capital floor byte offset.
pub const DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4: usize = 112;

/// Stable hostile-decode refusal for the immutable Dealer config.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerConfigErrorV4 {
    /// The byte slice did not have the one exact width.
    InvalidLength,
    /// Magic or version selected another config ABI.
    UnsupportedFormat,
    /// A reserved byte was nonzero.
    NonCanonical,
    /// A required immutable identity was zero.
    ZeroIdentity,
}

/// Immutable acyclic facts shared by every action in one Dealer capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerConfigV4 {
    release_set: [u8; 32],
    realm: [u8; 32],
    position_owner: [u8; 32],
    locked_capital_floor: u64,
}

impl DealerConfigV4 {
    /// Construct one canonical immutable config.
    pub fn new(
        release_set: [u8; 32],
        realm: [u8; 32],
        position_owner: [u8; 32],
        locked_capital_floor: u64,
    ) -> Result<Self, DealerConfigErrorV4> {
        require_identities([release_set, realm, position_owner])?;
        Ok(Self {
            release_set,
            realm,
            position_owner,
            locked_capital_floor,
        })
    }

    /// Hostile-decode one exact canonical representation.
    pub fn decode(bytes: &[u8]) -> Result<Self, DealerConfigErrorV4> {
        if bytes.len() != DEALER_CONFIG_BYTES_V4 {
            return Err(DealerConfigErrorV4::InvalidLength);
        }
        if bytes.get(..8) != Some(DEALER_CONFIG_MAGIC_V4.as_slice())
            || read_u16(bytes, 8)? != DEALER_CONFIG_VERSION_V4
        {
            return Err(DealerConfigErrorV4::UnsupportedFormat);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 120, 8)?;
        Self::new(
            read_identity(bytes, DEALER_CONFIG_RELEASE_SET_OFFSET_V4)?,
            read_identity(bytes, DEALER_CONFIG_REALM_OFFSET_V4)?,
            read_identity(bytes, DEALER_CONFIG_POSITION_OWNER_OFFSET_V4)?,
            read_u64(bytes, DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4)?,
        )
    }

    /// Encode the one canonical config representation.
    #[must_use]
    pub fn encode(self) -> [u8; DEALER_CONFIG_BYTES_V4] {
        let mut output = [0_u8; DEALER_CONFIG_BYTES_V4];
        output[..8].copy_from_slice(&DEALER_CONFIG_MAGIC_V4);
        output[8..10].copy_from_slice(&DEALER_CONFIG_VERSION_V4.to_le_bytes());
        output[DEALER_CONFIG_RELEASE_SET_OFFSET_V4..DEALER_CONFIG_REALM_OFFSET_V4]
            .copy_from_slice(&self.release_set);
        output[DEALER_CONFIG_REALM_OFFSET_V4..DEALER_CONFIG_POSITION_OWNER_OFFSET_V4]
            .copy_from_slice(&self.realm);
        output
            [DEALER_CONFIG_POSITION_OWNER_OFFSET_V4..DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4]
            .copy_from_slice(&self.position_owner);
        output[DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4..120]
            .copy_from_slice(&self.locked_capital_floor.to_le_bytes());
        output
    }

    /// Immutable selected release set.
    #[must_use]
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }

    /// Immutable Realm selecting collateral behavior.
    #[must_use]
    pub const fn realm(self) -> [u8; 32] {
        self.realm
    }

    /// Canonical Dealer Claims Position owner.
    #[must_use]
    pub const fn position_owner(self) -> [u8; 32] {
        self.position_owner
    }

    /// Exact minimum scenario residual in collateral atoms.
    #[must_use]
    pub const fn locked_capital_floor(self) -> u64 {
        self.locked_capital_floor
    }
}

fn require_identities(identities: [[u8; 32]; 3]) -> Result<(), DealerConfigErrorV4> {
    if identities.contains(&[0; 32]) {
        Err(DealerConfigErrorV4::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), DealerConfigErrorV4> {
    let end = offset
        .checked_add(width)
        .ok_or(DealerConfigErrorV4::InvalidLength)?;
    if bytes
        .get(offset..end)
        .is_some_and(|reserved| reserved.iter().all(|byte| *byte == 0))
    {
        Ok(())
    } else {
        Err(DealerConfigErrorV4::NonCanonical)
    }
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], DealerConfigErrorV4> {
    bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(DealerConfigErrorV4::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, DealerConfigErrorV4> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(DealerConfigErrorV4::InvalidLength)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DealerConfigErrorV4> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(DealerConfigErrorV4::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn config() -> DealerConfigV4 {
        DealerConfigV4::new(id(1), id(2), id(3), 17).expect("config")
    }

    #[test]
    fn exact_round_trip_owns_only_acyclic_facts() {
        let value = config();
        let bytes = value.encode();
        assert_eq!(bytes.len(), DEALER_CONFIG_BYTES_V4);
        assert_eq!(DealerConfigV4::decode(&bytes), Ok(value));
        assert_eq!(value.release_set(), id(1));
        assert_eq!(value.realm(), id(2));
        assert_eq!(value.position_owner(), id(3));
        assert_eq!(value.locked_capital_floor(), 17);
    }

    #[test]
    fn hostile_width_format_padding_and_identity_refuse() {
        let bytes = config().encode();
        assert_eq!(
            DealerConfigV4::decode(&bytes[..DEALER_CONFIG_BYTES_V4 - 1]),
            Err(DealerConfigErrorV4::InvalidLength)
        );
        for index in [0_usize, 8, 10, 120] {
            let mut hostile = bytes;
            hostile[index] ^= 1;
            assert!(DealerConfigV4::decode(&hostile).is_err());
        }
        for offset in [
            DEALER_CONFIG_RELEASE_SET_OFFSET_V4,
            DEALER_CONFIG_REALM_OFFSET_V4,
            DEALER_CONFIG_POSITION_OWNER_OFFSET_V4,
        ] {
            let mut hostile = bytes;
            hostile[offset..offset + 32].fill(0);
            assert_eq!(
                DealerConfigV4::decode(&hostile),
                Err(DealerConfigErrorV4::ZeroIdentity)
            );
        }
    }

    #[test]
    fn market_is_not_a_config_coordinate() {
        let bytes = config().encode();
        assert_eq!(bytes.len(), 128);
        assert_eq!(&bytes[16..48], &id(1));
        assert_eq!(&bytes[48..80], &id(2));
        assert_eq!(&bytes[80..112], &id(3));
    }
}
