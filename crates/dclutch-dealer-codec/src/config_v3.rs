//! Immutable runtime-width Dealer config.
//!
//! This fixed-layout record is the sole semantic owner of the scenario
//! solvency floor.  It joins that floor to one release, logical Market,
//! immutable Realm, and canonical Dealer Claims Position owner.  Requests,
//! operators, and admitted accelerators may project these facts, but must not
//! accept a caller-provided copy as authority.

/// Canonical immutable Dealer config magic.
pub const DEALER_CONFIG_MAGIC_V3: [u8; 8] = *b"DCLDDC03";
/// Canonical immutable Dealer config ABI version.
pub const DEALER_CONFIG_VERSION_V3: u16 = 3;
/// Exact immutable Dealer config width.
pub const DEALER_CONFIG_BYTES_V3: usize = 160;
/// Canonical finalized-record schema label.
pub const DEALER_CONFIG_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/dealer-immutable-config-v3";

const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const REALM_OFFSET: usize = 80;
const POSITION_OWNER_OFFSET: usize = 112;
const LOCKED_CAPITAL_FLOOR_OFFSET: usize = 144;

/// Stable hostile-decode refusal for the immutable Dealer descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerConfigErrorV3 {
    /// The byte slice did not have the one exact width.
    InvalidLength,
    /// Magic or version selected another descriptor ABI.
    UnsupportedFormat,
    /// A reserved byte was nonzero.
    NonCanonical,
    /// A required immutable identity was zero.
    ZeroIdentity,
}

/// Immutable facts shared by every action in one Dealer capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerConfigV3 {
    release_set: [u8; 32],
    market: [u8; 32],
    realm: [u8; 32],
    position_owner: [u8; 32],
    locked_capital_floor: u64,
}

impl DealerConfigV3 {
    /// Construct one canonical immutable descriptor.
    pub fn new(
        release_set: [u8; 32],
        market: [u8; 32],
        realm: [u8; 32],
        position_owner: [u8; 32],
        locked_capital_floor: u64,
    ) -> Result<Self, DealerConfigErrorV3> {
        require_identities([release_set, market, realm, position_owner])?;
        Ok(Self {
            release_set,
            market,
            realm,
            position_owner,
            locked_capital_floor,
        })
    }

    /// Hostile-decode one exact canonical descriptor.
    pub fn decode(bytes: &[u8]) -> Result<Self, DealerConfigErrorV3> {
        if bytes.len() != DEALER_CONFIG_BYTES_V3 {
            return Err(DealerConfigErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(DEALER_CONFIG_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_CONFIG_VERSION_V3
        {
            return Err(DealerConfigErrorV3::UnsupportedFormat);
        }
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 152, 8)?;
        Self::new(
            read_identity(bytes, RELEASE_SET_OFFSET)?,
            read_identity(bytes, MARKET_OFFSET)?,
            read_identity(bytes, REALM_OFFSET)?,
            read_identity(bytes, POSITION_OWNER_OFFSET)?,
            read_u64(bytes, LOCKED_CAPITAL_FLOOR_OFFSET)?,
        )
    }

    /// Encode the one canonical descriptor representation.
    #[must_use]
    pub fn encode(self) -> [u8; DEALER_CONFIG_BYTES_V3] {
        let mut output = [0_u8; DEALER_CONFIG_BYTES_V3];
        output[..8].copy_from_slice(&DEALER_CONFIG_MAGIC_V3);
        output[8..10].copy_from_slice(&DEALER_CONFIG_VERSION_V3.to_le_bytes());
        output[RELEASE_SET_OFFSET..MARKET_OFFSET].copy_from_slice(&self.release_set);
        output[MARKET_OFFSET..REALM_OFFSET].copy_from_slice(&self.market);
        output[REALM_OFFSET..POSITION_OWNER_OFFSET].copy_from_slice(&self.realm);
        output[POSITION_OWNER_OFFSET..LOCKED_CAPITAL_FLOOR_OFFSET]
            .copy_from_slice(&self.position_owner);
        output[LOCKED_CAPITAL_FLOOR_OFFSET..152]
            .copy_from_slice(&self.locked_capital_floor.to_le_bytes());
        output
    }

    /// Immutable selected release set.
    #[must_use]
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }

    /// Logical Core Market.
    #[must_use]
    pub const fn market(self) -> [u8; 32] {
        self.market
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

fn require_identities(identities: [[u8; 32]; 4]) -> Result<(), DealerConfigErrorV3> {
    if identities.contains(&[0; 32]) {
        Err(DealerConfigErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), DealerConfigErrorV3> {
    let end = offset
        .checked_add(width)
        .ok_or(DealerConfigErrorV3::InvalidLength)?;
    if bytes
        .get(offset..end)
        .is_some_and(|reserved| reserved.iter().all(|byte| *byte == 0))
    {
        Ok(())
    } else {
        Err(DealerConfigErrorV3::NonCanonical)
    }
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], DealerConfigErrorV3> {
    bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(DealerConfigErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, DealerConfigErrorV3> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(DealerConfigErrorV3::InvalidLength)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DealerConfigErrorV3> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(DealerConfigErrorV3::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn config() -> DealerConfigV3 {
        DealerConfigV3::new(id(1), id(2), id(3), id(4), 17).expect("config")
    }

    #[test]
    fn exact_round_trip_owns_the_solvency_floor() {
        let value = config();
        let bytes = value.encode();
        assert_eq!(bytes.len(), DEALER_CONFIG_BYTES_V3);
        assert_eq!(DealerConfigV3::decode(&bytes), Ok(value));
        assert_eq!(value.release_set(), id(1));
        assert_eq!(value.market(), id(2));
        assert_eq!(value.realm(), id(3));
        assert_eq!(value.position_owner(), id(4));
        assert_eq!(value.locked_capital_floor(), 17);
    }

    #[test]
    fn hostile_width_format_padding_and_identity_refuse() {
        let bytes = config().encode();
        assert_eq!(
            DealerConfigV3::decode(&bytes[..159]),
            Err(DealerConfigErrorV3::InvalidLength)
        );
        for index in [0_usize, 8, 10, 152] {
            let mut hostile = bytes;
            hostile[index] ^= 1;
            assert!(DealerConfigV3::decode(&hostile).is_err());
        }
        for offset in [
            RELEASE_SET_OFFSET,
            MARKET_OFFSET,
            REALM_OFFSET,
            POSITION_OWNER_OFFSET,
        ] {
            let mut hostile = bytes;
            hostile[offset..offset + 32].fill(0);
            assert_eq!(
                DealerConfigV3::decode(&hostile),
                Err(DealerConfigErrorV3::ZeroIdentity)
            );
        }
    }

    #[test]
    fn floor_is_not_a_caller_side_coordinate() {
        let mut bytes = config().encode();
        bytes[LOCKED_CAPITAL_FLOOR_OFFSET..152].copy_from_slice(&18_u64.to_le_bytes());
        let changed = DealerConfigV3::decode(&bytes).expect("canonical changed config");
        assert_eq!(changed.locked_capital_floor(), 18);
        assert_ne!(changed, config());
    }
}
