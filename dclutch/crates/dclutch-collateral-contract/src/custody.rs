//! Fixed collateral-custody root owning Vault-rent refund authority.

use core::convert::TryInto;

use crate::{Error, Result};

/// Exact byte width of one [`CollateralCustodyV1`] record.
pub const COLLATERAL_CUSTODY_BYTES: usize = 88;
/// Canonical custody-root magic.
pub const COLLATERAL_CUSTODY_MAGIC: [u8; 8] = *b"DCLTCUS1";
/// Implemented custody-root schema.
pub const COLLATERAL_CUSTODY_SCHEMA_VERSION: u16 = 1;
/// Chain-derived maximum byte width of one Solana PDA seed component.
pub const SVM_MAX_PDA_SEED_BYTES: usize = 32;
/// PDA domain preceding the Market key for the program-owned custody root.
pub const COLLATERAL_CUSTODY_PDA_DOMAIN: &[u8] = b"dclutch/collateral-custody/v1";
/// PDA domain preceding the Market key for its token-program-owned Vault.
pub const COLLATERAL_VAULT_PDA_DOMAIN: &[u8] = b"dclutch/collateral-vault/v1";

const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const MARKET_OFFSET: usize = 16;
const GENERATION_OFFSET: usize = 48;
const RENT_REFUND_OFFSET: usize = 56;

/// One Market's collateral-custody root.
///
/// This direct Market child is the semantic root of the token-program-owned
/// Vault derived from the same Market. It persists the only recipient of both
/// Vault and custody-account rent at retirement. Those lamports are sponsor
/// principal, never protocol revenue or a caller bounty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralCustodyV1 {
    market: [u8; 32],
    generation: u64,
    rent_refund: [u8; 32],
}

impl CollateralCustodyV1 {
    /// Validate and construct one immutable custody root.
    pub fn new(market: [u8; 32], generation: u64, rent_refund: [u8; 32]) -> Result<Self> {
        require_nonzero(&market)?;
        require_nonzero(&rent_refund)?;
        Ok(Self {
            market,
            generation,
            rent_refund,
        })
    }

    /// Decode one exact canonical custody root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != COLLATERAL_CUSTODY_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != COLLATERAL_CUSTODY_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != COLLATERAL_CUSTODY_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        Self::new(
            read_array(bytes, MARKET_OFFSET)?,
            u64::from_le_bytes(read_array(bytes, GENERATION_OFFSET)?),
            read_array(bytes, RENT_REFUND_OFFSET)?,
        )
    }

    /// Return exact canonical bytes.
    pub fn to_bytes(self) -> [u8; COLLATERAL_CUSTODY_BYTES] {
        let mut output = [0; COLLATERAL_CUSTODY_BYTES];
        put(&mut output, 0, &COLLATERAL_CUSTODY_MAGIC);
        put(
            &mut output,
            8,
            &COLLATERAL_CUSTODY_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, MARKET_OFFSET, &self.market);
        put(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(&mut output, RENT_REFUND_OFFSET, &self.rent_refund);
        output
    }

    /// Encode into an exact caller-owned buffer without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != COLLATERAL_CUSTODY_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the exact Market address and PDA seed component.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the authenticated recipient of all custody-compartment rent.
    pub const fn rent_refund(self) -> [u8; 32] {
        self.rent_refund
    }
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_round_trip_and_hostile_layouts() {
        assert_eq!(COLLATERAL_CUSTODY_PDA_DOMAIN.len(), 29);
        assert_eq!(COLLATERAL_VAULT_PDA_DOMAIN.len(), 27);
        assert!(COLLATERAL_CUSTODY_PDA_DOMAIN.len() <= SVM_MAX_PDA_SEED_BYTES);
        assert!(COLLATERAL_VAULT_PDA_DOMAIN.len() <= SVM_MAX_PDA_SEED_BYTES);
        let custody = CollateralCustodyV1::new([1; 32], 9, [2; 32]).expect("valid custody");
        let bytes = custody.to_bytes();
        assert_eq!(CollateralCustodyV1::decode(&bytes), Ok(custody));
        for length in 0..COLLATERAL_CUSTODY_BYTES {
            if let Some(short) = bytes.get(..length) {
                assert_eq!(
                    CollateralCustodyV1::decode(short),
                    Err(Error::InvalidLength)
                );
            }
        }
        let mut changed = bytes;
        if let Some(byte) = changed.get_mut(RESERVED_OFFSET) {
            *byte = 1;
        }
        assert_eq!(
            CollateralCustodyV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        assert_eq!(
            CollateralCustodyV1::new([0; 32], 9, [2; 32]),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            CollateralCustodyV1::new([1; 32], 9, [0; 32]),
            Err(Error::ZeroIdentifier)
        );
    }
}
