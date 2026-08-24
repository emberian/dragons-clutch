// SPDX-License-Identifier: AGPL-3.0-or-later

//! Counted close-grief guard for one Market-scoped treasury Position.
//!
//! This account is the persistent byte home for
//! [`clutch_liveness::TreasuryServiceLedger`].  It stores no fee balance and
//! no duplicate Revenue-policy facts: the immutable MarketBinding V4 remains
//! their semantic owner.  The adapter must authenticate that binding, this
//! account's PDA, program owner, full bytes, and action-specific epoch/root
//! authority before applying either counter transition.

use crate::{CodecError, DeletableRentOwnerV1, Id32, Reader, Writer};

/// Fresh counted treasury-service-ledger discriminator.
pub const TREASURY_SERVICE_LEDGER_ACCOUNT_TAG_V1: u8 = 0xbb;
/// First counted treasury-service-ledger schema.
pub const TREASURY_SERVICE_LEDGER_ACCOUNT_VERSION_V1: u8 = 1;
/// Exact canonical account width.
///
/// Header 2, MarketBinding 32, treasury Position 32, outstanding count 8,
/// deletable rent owner 48, generation 8, bump 1, flags 1.
pub const TREASURY_SERVICE_LEDGER_ACCOUNT_BYTES_V1: usize = 132;
/// PDA seed domain.  The sole suffix is the immutable MarketBinding account.
pub const TREASURY_SERVICE_LEDGER_SEED_DOMAIN_V1: &[u8] = b"treasury-service-ledger:v1";

const _: () = assert!(TREASURY_SERVICE_LEDGER_SEED_DOMAIN_V1.len() <= 32);
const _: () = assert!(TREASURY_SERVICE_LEDGER_ACCOUNT_BYTES_V1 == 2 + 32 + 32 + 8 + 48 + 8 + 1 + 1);

/// Canonical live service-ledger body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreasuryServiceLedgerV1AccountV1 {
    /// Immutable current General MarketBinding V4 account.
    pub market_binding: Id32,
    /// Exact ordinary Position V3 protected against mid-service close.
    pub treasury_position: Id32,
    /// Fee-bearing Epoch services begun but not terminally settled.
    pub outstanding_service_count: u64,
    /// Exact separately funded deletable rent owner.
    pub rent: DeletableRentOwnerV1,
    /// Monotone account generation; V1 is founded once at generation one.
    pub generation: u64,
    /// Canonical PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl TreasuryServiceLedgerV1AccountV1 {
    /// Construct the unique empty founding body.
    pub fn new_live(
        market_binding: Id32,
        treasury_position: Id32,
        rent: DeletableRentOwnerV1,
        stored_bump: u8,
    ) -> Result<Self, CodecError> {
        let value = Self {
            market_binding,
            treasury_position,
            outstanding_service_count: 0,
            rent,
            generation: 1,
            stored_bump,
            flags: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate hostile decoded state and reconstruct the liveness owner.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.market_binding.is_zero()
            || self.treasury_position.is_zero()
            || self.market_binding == self.treasury_position
            || self.generation != 1
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        self.rent.validate()?;
        clutch_liveness::TreasuryServiceLedger::admit_counted(
            clutch_liveness::Id::from_bytes(self.treasury_position.bytes()),
            self.outstanding_service_count,
        )
        .map_err(|_| CodecError::InvalidState)?;
        Ok(())
    }

    /// Begin exactly one authenticated fee-bearing Epoch service.
    pub fn begin_service(self) -> Result<Self, CodecError> {
        self.validate()?;
        let ledger = clutch_liveness::TreasuryServiceLedger::admit_counted(
            clutch_liveness::Id::from_bytes(self.treasury_position.bytes()),
            self.outstanding_service_count,
        )
        .and_then(clutch_liveness::TreasuryServiceLedger::begin_service)
        .map_err(|error| match error {
            clutch_liveness::Error::ArithmeticOverflow => CodecError::ArithmeticOverflow,
            _ => CodecError::InvalidState,
        })?;
        Ok(Self {
            outstanding_service_count: ledger.outstanding(),
            ..self
        })
    }

    /// Settle exactly one authenticated terminal Epoch service.
    pub fn settle_service(self) -> Result<Self, CodecError> {
        self.validate()?;
        let ledger = clutch_liveness::TreasuryServiceLedger::admit_counted(
            clutch_liveness::Id::from_bytes(self.treasury_position.bytes()),
            self.outstanding_service_count,
        )
        .and_then(clutch_liveness::TreasuryServiceLedger::settle_service)
        .map_err(|_| CodecError::InvalidState)?;
        Ok(Self {
            outstanding_service_count: ledger.outstanding(),
            ..self
        })
    }

    /// Check the sole close precondition without producing a reusable token.
    pub fn require_closeable(self) -> Result<(), CodecError> {
        self.validate()?;
        clutch_liveness::TreasuryServiceLedger::admit_counted(
            clutch_liveness::Id::from_bytes(self.treasury_position.bytes()),
            self.outstanding_service_count,
        )
        .and_then(clutch_liveness::TreasuryServiceLedger::close)
        .map_err(|_| CodecError::InvalidState)?;
        Ok(())
    }

    /// Encode the exact canonical account image.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, TREASURY_SERVICE_LEDGER_ACCOUNT_BYTES_V1)?;
        writer.u8(TREASURY_SERVICE_LEDGER_ACCOUNT_TAG_V1)?;
        writer.u8(TREASURY_SERVICE_LEDGER_ACCOUNT_VERSION_V1)?;
        writer.bytes(&self.market_binding.bytes())?;
        writer.bytes(&self.treasury_position.bytes())?;
        writer.u64(self.outstanding_service_count)?;
        writer.bytes(&self.rent.payer.bytes())?;
        writer.u64(self.rent.refundable_principal)?;
        writer.u64(self.rent.donation_floor)?;
        writer.u64(self.generation)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode one exact hostile account image.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, TREASURY_SERVICE_LEDGER_ACCOUNT_BYTES_V1)?;
        if reader.u8()? != TREASURY_SERVICE_LEDGER_ACCOUNT_TAG_V1 {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != TREASURY_SERVICE_LEDGER_ACCOUNT_VERSION_V1 {
            return Err(CodecError::WrongVersion);
        }
        let value = Self {
            market_binding: Id32::new(reader.array()?)?,
            treasury_position: Id32::new(reader.array()?)?,
            outstanding_service_count: reader.u64()?,
            rent: DeletableRentOwnerV1 {
                payer: Id32::new(reader.array()?)?,
                refundable_principal: reader.u64()?,
                donation_floor: reader.u64()?,
            },
            generation: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 { Id32::new([byte; 32]).unwrap() }

    fn ledger() -> TreasuryServiceLedgerV1AccountV1 {
        TreasuryServiceLedgerV1AccountV1::new_live(
            id(1),
            id(2),
            DeletableRentOwnerV1 {
                payer: id(3),
                refundable_principal: 11,
                donation_floor: 7,
            },
            254,
        )
        .unwrap()
    }

    #[test]
    fn exact_codec_and_counted_close_rule_refuse_hostile_shapes() {
        let value = ledger();
        let mut bytes = [0u8; TREASURY_SERVICE_LEDGER_ACCOUNT_BYTES_V1];
        value.encode(&mut bytes).unwrap();
        assert_eq!(TreasuryServiceLedgerV1AccountV1::decode(&bytes), Ok(value));
        assert!(value.require_closeable().is_ok());
        let serving = value.begin_service().unwrap();
        assert_eq!(serving.outstanding_service_count, 1);
        assert!(serving.require_closeable().is_err());
        assert_eq!(serving.settle_service().unwrap(), value);
        assert!(value.settle_service().is_err());

        for hostile in [
            TreasuryServiceLedgerV1AccountV1::hostile(
                value, Id32::ZERO, value.treasury_position, value.generation, value.flags,
            ),
            TreasuryServiceLedgerV1AccountV1::hostile(
                value, value.market_binding, value.market_binding, value.generation, value.flags,
            ),
            TreasuryServiceLedgerV1AccountV1::hostile(
                value, value.market_binding, value.treasury_position, 2, value.flags,
            ),
            TreasuryServiceLedgerV1AccountV1::hostile(
                value, value.market_binding, value.treasury_position, value.generation, 1,
            ),
        ] {
            assert!(hostile.validate().is_err());
        }
        bytes[0] ^= 1;
        assert!(TreasuryServiceLedgerV1AccountV1::decode(&bytes).is_err());
    }

    impl TreasuryServiceLedgerV1AccountV1 {
        fn hostile(
            value: Self,
            market_binding: Id32,
            treasury_position: Id32,
            generation: u64,
            flags: u8,
        ) -> Self {
            Self { market_binding, treasury_position, generation, flags, ..value }
        }
    }
}
