//! Rent-owned General Reservation successor.
//!
//! V9 preserves the canonical Reservation economic/partial-payment state
//! machine but gives every account one exact, persisted rent owner. Earlier
//! Reservation versions cannot be interpreted through this codec: V9 uses a
//! fresh account version, semantic identity domain, account-data domain, and
//! General PDA seed domain. The live adapter must create only V9 and must
//! never infer the refund owner from a Position, order owner, or transaction
//! signer.

use super::{digest, CodecError, Hash32, Result, HASH_BYTES};
use crate::reservation::{
    canonical_reservation_id, ReservationAccount, ReservationPlan, RESERVATION_ACCOUNT_BYTES,
    RESERVATION_ACCOUNT_TAG, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED,
    RESERVATION_STATE_RELEASED,
};

/// Fresh rent-owned General Reservation account version.
pub const RESERVATION_ACCOUNT_VERSION_V9: u8 = 9;
/// Exact persisted rent-owner tail width.
pub const RESERVATION_RENT_OWNER_BYTES_V9: usize = 48;
/// Exact V9 account width: V4 economic body plus one rent-owner tail.
pub const RESERVATION_ACCOUNT_BYTES_V9: usize =
    RESERVATION_ACCOUNT_BYTES + RESERVATION_RENT_OWNER_BYTES_V9;
/// Fresh semantic Reservation identity domain.
pub const RESERVATION_SEMANTIC_ID_DOMAIN_V9: &[u8] = b"dragons-clutch/reservation/v9\0";
/// Fresh exact-account data identity domain.
pub const RESERVATION_ACCOUNT_DATA_ID_DOMAIN_V9: &[u8] =
    b"dragons-clutch/reservation-account-data/v9\0";

const RESERVATION_ID_OFFSET: usize = 2;
const MARKET_OFFSET: usize = RESERVATION_ID_OFFSET + HASH_BYTES;
const EPOCH_OFFSET: usize = MARKET_OFFSET + HASH_BYTES;
const OWNER_OFFSET: usize = EPOCH_OFFSET + HASH_BYTES;
const ORDER_ID_OFFSET: usize = OWNER_OFFSET + HASH_BYTES;
const POSITION_GENERATION_OFFSET: usize = 2 + 8 * HASH_BYTES;

/// Exact deletable rent/refund/donation owner persisted by each Reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableRentOwnerV1 {
    /// Signer that funded the refundable rent principal.
    pub payer: Hash32,
    /// Exact principal refundable only to `payer`.
    pub refundable_principal: u64,
    /// Observed hostile prefund/donation floor sent only to the neutral sink.
    pub donation_floor: u64,
}

impl DeletableRentOwnerV1 {
    /// Validate identity, nonzero principal, and exact lamport-domain sum.
    pub fn validate(self) -> Result<()> {
        if self.payer == Hash32::ZERO || self.refundable_principal == 0 {
            return Err(CodecError::ZeroValue);
        }
        self.refundable_principal
            .checked_add(self.donation_floor)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Sole future General per-order Reservation account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationAccountV9 {
    body: ReservationAccount,
    rent: DeletableRentOwnerV1,
}

impl ReservationAccountV9 {
    /// Bind one exact V9 semantic body to its persisted rent owner.
    pub fn new(body: ReservationAccount, rent: DeletableRentOwnerV1) -> Result<Self> {
        let value = Self { body, rent };
        value.validate()?;
        Ok(value)
    }

    /// Construct an active V9 Reservation from one checked order plan.
    #[allow(clippy::too_many_arguments)]
    pub fn active(
        market: Hash32,
        epoch: Hash32,
        owner: Hash32,
        order_id: Hash32,
        price_grid: Hash32,
        terms: Hash32,
        policy: Hash32,
        position_generation: u64,
        order_generation: u64,
        page_index: u16,
        stored_bump: u8,
        plan: ReservationPlan,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        let mut body = ReservationAccount::active(
            market,
            epoch,
            owner,
            order_id,
            price_grid,
            terms,
            policy,
            position_generation,
            order_generation,
            page_index,
            stored_bump,
            plan,
        )?;
        body.reservation =
            canonical_reservation_id_v9(market, epoch, owner, position_generation, order_id);
        Self::new(body, rent)
    }

    /// Validate the full economic body under only the V9 identity domain.
    pub fn validate(self) -> Result<()> {
        let expected = canonical_reservation_id_v9(
            self.body.market,
            self.body.epoch,
            self.body.owner,
            self.body.position_generation,
            self.body.order_id,
        );
        self.body.validate_with_identity(expected)?;
        self.rent.validate()
    }

    /// Return the exact Reservation economic/ledger body.
    pub const fn body(self) -> ReservationAccount {
        self.body
    }

    /// Return the exact persisted rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Return a released V9 poststate without changing rent ownership.
    pub fn released(self, release_generation: u64) -> Result<Self> {
        self.validate()?;
        if self.body.state != RESERVATION_STATE_ACTIVE
            || release_generation <= self.body.order_generation
        {
            return Err(CodecError::MismatchedBinding);
        }
        let mut next = self;
        next.body.remaining_cash_atoms = 0;
        next.body.remaining_internal = [0; super::MAX_OUTCOMES];
        next.body.release_generation = release_generation;
        next.body.state = RESERVATION_STATE_RELEASED;
        next.validate()?;
        Ok(next)
    }

    /// Stamp the exact first entitlement while preserving rent ownership.
    pub fn entitled(self, entitled_units: u64) -> Result<Self> {
        self.validate()?;
        if self.body.state != RESERVATION_STATE_ACTIVE || entitled_units == 0 {
            return Err(CodecError::MismatchedBinding);
        }
        let mut next = self;
        next.body.entitled_units = entitled_units;
        next.body.state = RESERVATION_STATE_ENTITLED;
        next.validate()?;
        Ok(next)
    }

    /// Require an already-entitled V9 account to carry one exact stamp.
    pub fn requires_stamp(self, entitled_units: u64) -> Result<()> {
        self.validate()?;
        if self.body.state != RESERVATION_STATE_ENTITLED
            || self.body.entitled_units != entitled_units
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly 666 canonical bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        if output.len() < RESERVATION_ACCOUNT_BYTES_V9 {
            return Err(CodecError::OutputTooSmall);
        }
        if output.len() > RESERVATION_ACCOUNT_BYTES_V9 {
            return Err(CodecError::TrailingBytes);
        }
        let mut historical = self.body;
        historical.reservation = canonical_reservation_id(
            historical.market,
            historical.epoch,
            historical.owner,
            historical.position_generation,
            historical.order_id,
        );
        historical.encode(&mut output[..RESERVATION_ACCOUNT_BYTES])?;
        output[1] = RESERVATION_ACCOUNT_VERSION_V9;
        output[RESERVATION_ID_OFFSET..RESERVATION_ID_OFFSET + HASH_BYTES]
            .copy_from_slice(&self.body.reservation.bytes());
        let rent_at = RESERVATION_ACCOUNT_BYTES;
        output[rent_at..rent_at + HASH_BYTES].copy_from_slice(&self.rent.payer.bytes());
        output[rent_at + HASH_BYTES..rent_at + HASH_BYTES + 8]
            .copy_from_slice(&self.rent.refundable_principal.to_le_bytes());
        output[rent_at + HASH_BYTES + 8..].copy_from_slice(&self.rent.donation_floor.to_le_bytes());
        Ok(())
    }

    /// Decode exactly 666 hostile bytes and refuse every earlier version.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < RESERVATION_ACCOUNT_BYTES_V9 {
            return Err(CodecError::Truncated);
        }
        if input.len() > RESERVATION_ACCOUNT_BYTES_V9 {
            return Err(CodecError::TrailingBytes);
        }
        if input[0] != RESERVATION_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != RESERVATION_ACCOUNT_VERSION_V9 {
            return Err(CodecError::WrongVersion);
        }
        let stored_reservation = hash_at(input, RESERVATION_ID_OFFSET);
        let market = hash_at(input, MARKET_OFFSET);
        let epoch = hash_at(input, EPOCH_OFFSET);
        let owner = hash_at(input, OWNER_OFFSET);
        let order_id = hash_at(input, ORDER_ID_OFFSET);
        let position_generation = u64_at(input, POSITION_GENERATION_OFFSET);
        let mut historical = [0u8; RESERVATION_ACCOUNT_BYTES];
        historical.copy_from_slice(&input[..RESERVATION_ACCOUNT_BYTES]);
        historical[1] = crate::reservation::RESERVATION_ACCOUNT_VERSION;
        historical[RESERVATION_ID_OFFSET..RESERVATION_ID_OFFSET + HASH_BYTES].copy_from_slice(
            &canonical_reservation_id(market, epoch, owner, position_generation, order_id).bytes(),
        );
        let mut body = ReservationAccount::decode(&historical)?;
        body.reservation = stored_reservation;
        let rent_at = RESERVATION_ACCOUNT_BYTES;
        let rent = DeletableRentOwnerV1 {
            payer: hash_at(input, rent_at),
            refundable_principal: u64_at(input, rent_at + HASH_BYTES),
            donation_floor: u64_at(input, rent_at + HASH_BYTES + 8),
        };
        Self::new(body, rent)
    }

    /// Derive the exact V9 account-data identity.
    pub fn data_id(self) -> Result<Hash32> {
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
        self.encode(&mut bytes)?;
        Ok(digest(RESERVATION_ACCOUNT_DATA_ID_DOMAIN_V9, &[&bytes]))
    }
}

/// Derive the fresh V9 Reservation semantic identity.
pub fn canonical_reservation_id_v9(
    market: Hash32,
    epoch: Hash32,
    owner: Hash32,
    position_generation: u64,
    order_id: Hash32,
) -> Hash32 {
    digest(
        RESERVATION_SEMANTIC_ID_DOMAIN_V9,
        &[
            &market.0,
            &epoch.0,
            &owner.0,
            &position_generation.to_le_bytes(),
            &order_id.0,
        ],
    )
}

fn hash_at(input: &[u8], offset: usize) -> Hash32 {
    let mut bytes = [0u8; HASH_BYTES];
    bytes.copy_from_slice(&input[offset..offset + HASH_BYTES]);
    Hash32::from_bytes(bytes)
}

fn u64_at(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

const _: () = assert!(RESERVATION_ACCOUNT_TAG == 0x13);
const _: () = assert!(RESERVATION_ACCOUNT_BYTES == 618);
const _: () = assert!(RESERVATION_ACCOUNT_BYTES_V9 == 666);

#[cfg(test)]
mod tests {
    use super::*;

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; HASH_BYTES])
    }

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer: h(9),
            refundable_principal: 2_000,
            donation_floor: 17,
        }
    }

    fn active() -> ReservationAccountV9 {
        ReservationAccountV9::active(
            h(1),
            h(2),
            h(3),
            crate::canonical_order_id(17),
            h(5),
            h(6),
            h(7),
            8,
            9,
            1,
            10,
            ReservationPlan {
                cash_atoms: 11,
                internal: [0; super::super::MAX_OUTCOMES],
                max_fee_atoms: 1,
                outcome_count: 2,
                order_kind: 0,
                side: 0,
            },
            rent(),
        )
        .unwrap()
    }

    #[test]
    fn exact_codec_round_trips_fresh_identity_and_rent_owner() {
        let value = active();
        let mut bytes = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
        value.encode(&mut bytes).unwrap();
        assert_eq!(bytes[0], RESERVATION_ACCOUNT_TAG);
        assert_eq!(bytes[1], RESERVATION_ACCOUNT_VERSION_V9);
        assert_eq!(ReservationAccountV9::decode(&bytes), Ok(value));
        assert_eq!(value.rent(), rent());
        assert_eq!(
            value.body().reservation,
            canonical_reservation_id_v9(
                h(1),
                h(2),
                h(3),
                8,
                crate::canonical_order_id(17),
            )
        );
        assert_ne!(
            value.body().reservation,
            canonical_reservation_id(
                h(1),
                h(2),
                h(3),
                8,
                crate::canonical_order_id(17),
            )
        );
    }

    #[test]
    fn every_other_version_and_nonexact_length_refuses() {
        let value = active();
        let mut exact = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
        value.encode(&mut exact).unwrap();
        for version in [4u8, 5, 7, 8] {
            let mut hostile = exact;
            hostile[1] = version;
            assert_eq!(
                ReservationAccountV9::decode(&hostile),
                Err(CodecError::WrongVersion)
            );
        }
        let mut oversized = [0u8; RESERVATION_ACCOUNT_BYTES_V9 + 1];
        oversized[..RESERVATION_ACCOUNT_BYTES_V9].copy_from_slice(&exact);
        for len in 0..=RESERVATION_ACCOUNT_BYTES_V9 + 1 {
            let result = ReservationAccountV9::decode(&oversized[..len]);
            if len == RESERVATION_ACCOUNT_BYTES_V9 {
                assert_eq!(result, Ok(value));
            } else if len < RESERVATION_ACCOUNT_BYTES_V9 {
                assert_eq!(result, Err(CodecError::Truncated));
            } else {
                assert_eq!(result, Err(CodecError::TrailingBytes));
            }
        }
    }

    #[test]
    fn rent_owner_refuses_zero_and_overflowing_compartments() {
        assert_eq!(
            DeletableRentOwnerV1 {
                payer: Hash32::ZERO,
                refundable_principal: 1,
                donation_floor: 0,
            }
            .validate(),
            Err(CodecError::ZeroValue)
        );
        assert_eq!(
            DeletableRentOwnerV1 {
                payer: h(9),
                refundable_principal: 0,
                donation_floor: 0,
            }
            .validate(),
            Err(CodecError::ZeroValue)
        );
        assert_eq!(
            DeletableRentOwnerV1 {
                payer: h(9),
                refundable_principal: u64::MAX,
                donation_floor: 1,
            }
            .validate(),
            Err(CodecError::ArithmeticOverflow)
        );
    }
}
