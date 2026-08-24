//! Canonical prepaid Pyth-resolution funding layout.

use crate::{Error, Result, array, nonzero, zero};

/// Exact byte width of [`ResolutionFundV1`].
pub const FUNDING_BYTES: usize = 112;
/// Funding-account magic.
pub const FUNDING_MAGIC: [u8; 8] = *b"DCLTFND1";
/// Implemented funding schema.
pub const FUNDING_SCHEMA_VERSION: u16 = 1;

/// Immutable prepaid funding facts for one Market generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionFundV1 {
    market: [u8; 32],
    generation: u64,
    sponsor_refund: [u8; 32],
    provider_fee_reimbursement: u64,
    success_bounty: u64,
}

impl ResolutionFundV1 {
    /// Construct validated immutable funding facts. `success_bounty` is the
    /// separately reserved positive resolver payment.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        sponsor_refund: [u8; 32],
        provider_fee_reimbursement: u64,
        success_bounty: u64,
    ) -> Result<Self> {
        if !nonzero(&market) || !nonzero(&sponsor_refund) {
            return Err(Error::ZeroIdentifier);
        }
        if success_bounty == 0 {
            return Err(Error::ZeroBounty);
        }
        provider_fee_reimbursement
            .checked_add(success_bounty)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            market,
            generation,
            sponsor_refund,
            provider_fee_reimbursement,
            success_bounty,
        })
    }

    /// Decode the exact canonical funding layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FUNDING_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != FUNDING_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if !zero(bytes.get(10..16).ok_or(Error::InvalidLength)?)
            || !zero(bytes.get(104..112).ok_or(Error::InvalidLength)?)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        Self::new(
            array(bytes, 16)?,
            u64::from_le_bytes(array(bytes, 48)?),
            array(bytes, 56)?,
            u64::from_le_bytes(array(bytes, 88)?),
            u64::from_le_bytes(array(bytes, 96)?),
        )
    }

    /// Encode this value into its exact canonical fixed-width bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_BYTES] {
        let mut out = [0; FUNDING_BYTES];
        out[..8].copy_from_slice(&FUNDING_MAGIC);
        out[8..10].copy_from_slice(&FUNDING_SCHEMA_VERSION.to_le_bytes());
        out[16..48].copy_from_slice(&self.market);
        out[48..56].copy_from_slice(&self.generation.to_le_bytes());
        out[56..88].copy_from_slice(&self.sponsor_refund);
        out[88..96].copy_from_slice(&self.provider_fee_reimbursement.to_le_bytes());
        out[96..104].copy_from_slice(&self.success_bounty.to_le_bytes());
        out
    }

    /// Encode into an exact-width caller buffer without changing it on refusal.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != FUNDING_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the Market identifier.
    pub const fn market(&self) -> &[u8; 32] {
        &self.market
    }
    /// Return the immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return the immutable recipient of any sponsor refund excess.
    pub const fn sponsor_refund(&self) -> &[u8; 32] {
        &self.sponsor_refund
    }
    /// Return the prepaid provider fee reimbursement, never protocol revenue.
    pub const fn provider_fee_reimbursement(&self) -> u64 {
        self.provider_fee_reimbursement
    }
    /// Return the immutable positive resolver success bounty.
    pub const fn success_bounty(&self) -> u64 {
        self.success_bounty
    }

    /// Return checked rent plus provider reimbursement plus success bounty.
    pub fn minimum_balance(&self, rent: u64) -> Result<u64> {
        rent.checked_add(self.provider_fee_reimbursement)
            .and_then(|value| value.checked_add(self.success_bounty))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Classify a balance without demanding exact equality.  The result binds
    /// any excess to its immutable sponsor-refund recipient rather than
    /// anonymous funds.
    pub fn classify_balance(&self, actual: u64, rent: u64) -> Result<BalanceClassification> {
        let minimum = self.minimum_balance(rent)?;
        let sponsor_refund_excess = actual.checked_sub(minimum).ok_or(Error::Underfunded)?;
        Ok(BalanceClassification {
            sponsor_refund_recipient: self.sponsor_refund,
            minimum,
            sponsor_refund_excess,
        })
    }
}

/// Exact funding classification, including the only recipient of excess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalanceClassification {
    sponsor_refund_recipient: [u8; 32],
    minimum: u64,
    sponsor_refund_excess: u64,
}

impl BalanceClassification {
    /// Return the immutable sponsor-refund recipient for the classified excess.
    pub const fn sponsor_refund_recipient(&self) -> &[u8; 32] {
        &self.sponsor_refund_recipient
    }
    /// Return the exact required rent, reimbursement, and bounty minimum.
    pub const fn minimum(&self) -> u64 {
        self.minimum
    }
    /// Return the exact excess payable only to the refund recipient.
    pub const fn sponsor_refund_excess(&self) -> u64 {
        self.sponsor_refund_excess
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn round_trip_and_excess_are_exact() {
        let funding = ResolutionFundV1::new(id(1), 9, id(2), 7, 11).expect("valid funding");
        assert_eq!(ResolutionFundV1::decode(&funding.to_bytes()), Ok(funding));
        assert_eq!(funding.minimum_balance(3), Ok(21));
        let classified = funding.classify_balance(28, 3).expect("funded");
        assert_eq!(classified.sponsor_refund_recipient(), &id(2));
        assert_eq!(classified.minimum(), 21);
        assert_eq!(classified.sponsor_refund_excess(), 7);
        assert_eq!(funding.classify_balance(20, 3), Err(Error::Underfunded));
    }

    #[test]
    fn hostile_layouts_and_overflow_refuse() {
        let funding = ResolutionFundV1::new(id(1), 0, id(2), 0, 1).expect("valid funding");
        let bytes = funding.to_bytes();
        for length in 0..FUNDING_BYTES {
            if let Some(short) = bytes.get(..length) {
                assert_eq!(ResolutionFundV1::decode(short), Err(Error::InvalidLength));
            }
        }
        assert_eq!(
            ResolutionFundV1::decode(&[0; 113]),
            Err(Error::InvalidLength)
        );
        let mut changed = bytes;
        if let Some(slot) = changed.get_mut(0) {
            *slot = 0;
        }
        assert_eq!(ResolutionFundV1::decode(&changed), Err(Error::InvalidMagic));
        let mut changed = bytes;
        if let Some(slot) = changed.get_mut(8) {
            *slot = 2;
        }
        assert_eq!(
            ResolutionFundV1::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = bytes;
        if let Some(slot) = changed.get_mut(10) {
            *slot = 1;
        }
        assert_eq!(
            ResolutionFundV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        assert_eq!(
            ResolutionFundV1::new([0; 32], 0, id(2), 0, 1),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            ResolutionFundV1::new(id(1), 0, [0; 32], 0, 1),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            ResolutionFundV1::new(id(1), 0, id(2), 0, 0),
            Err(Error::ZeroBounty)
        );
        assert_eq!(
            ResolutionFundV1::new(id(1), 0, id(2), u64::MAX, 1),
            Err(Error::ArithmeticOverflow)
        );
    }
}
