// SPDX-License-Identifier: AGPL-3.0-or-later

//! One-shot funding owner for the facility Fractional-credit account.
//!
//! Resolution does not exist when a Dealer facility is initialized, so the
//! canonical owner-scoped Fractional credit PDA cannot yet be derived. This
//! account holds only the exact future credit rent principal until Resolve.
//! It owns no collateral, claim, work, liveness, or fee principal.

use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    add, Error, FixedCodec, Id, Result, DEALER_FUTURE_CREDIT_FUNDING_CONTENT_DOMAIN_V1,
};

/// Exact local semantic-body magic.
pub const DEALER_FUTURE_CREDIT_FUNDING_MAGIC_V1: [u8; 8] = *b"DCFCRF01";
/// Exact local semantic-body version.
pub const DEALER_FUTURE_CREDIT_FUNDING_VERSION_V1: u16 = 1;
/// Canonical future Fractional-credit account version.
pub const DEALER_FUTURE_CREDIT_ACCOUNT_VERSION_V1: u8 = 2;
/// Canonical future Fractional-credit live bytes.
pub const DEALER_FUTURE_CREDIT_ACCOUNT_BYTES_V1: u64 = 296;
/// Exact semantic body width, excluding the Dealer global envelope.
pub const DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1: usize = HEADER_BYTES + (14 * 32) + 56;

const DEALER_FUTURE_CREDIT_CONSUMPTION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/future-credit-consumption/v1\0";
const DEALER_FUTURE_CREDIT_UNUSED_CLOSE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-runtime/future-credit-unused-close/v1\0";

/// Exact one-shot rent-capital owner created with the Dealer facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFutureCreditFundingV1 {
    /// Physical `0xbc/v1` funding account.
    pub funding_account_id: Id,
    /// Immutable Dealer policy.
    pub policy_id: Id,
    /// Immutable facility semantic owner.
    pub facility_id: Id,
    /// Full-width Product Market identity.
    pub market_instance_v2_id: Id,
    /// Immutable Realm selecting collateral.
    pub realm_id: Id,
    /// Exact Realm-selected collateral policy.
    pub collateral_policy_id: Id,
    /// Exact Realm-selected collateral release.
    pub collateral_release_id: Id,
    /// Exact same-instruction collateral deployment/value receipt at founding.
    pub collateral_value_receipt_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Canonical facility Position account.
    pub facility_position_account_id: Id,
    /// Immutable Position-purpose binding.
    pub facility_position_binding_id: Id,
    /// Canonical purpose-owned Dealer Replay account.
    pub dealer_replay_account_id: Id,
    /// Sole recipient of both refundable rent-principal compartments.
    pub refund_owner: Id,
    /// Immutable Realm-neutral lamport sink.
    pub neutral_sink: Id,
    /// Dealer generation at founding; canonically one.
    pub founding_generation: u64,
    /// Exact refundable principal for this deletable funding account.
    pub funding_account_principal_lamports: u64,
    /// Future credit live-to-tombstone refundable rent delta.
    pub credit_refundable_principal_lamports: u64,
    /// Future credit permanent tombstone rent principal.
    pub credit_tombstone_principal_lamports: u64,
    /// Hostile prefund observed before the full-principal funding debit.
    pub donation_floor_lamports: u64,
}

impl DealerFutureCreditFundingV1 {
    /// Validate the immutable identity graph and disjoint rent compartments.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.funding_account_id,
            self.policy_id,
            self.facility_id,
            self.market_instance_v2_id,
            self.realm_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.collateral_value_receipt_id,
            self.dealer_state_account_id,
            self.facility_position_account_id,
            self.facility_position_binding_id,
            self.dealer_replay_account_id,
            self.refund_owner,
            self.neutral_sink,
        ] {
            identity.validate_live()?;
        }
        let physical = [
            self.funding_account_id,
            self.dealer_state_account_id,
            self.facility_position_account_id,
            self.dealer_replay_account_id,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(Error::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        if self.refund_owner == self.neutral_sink
            || self.founding_generation != 1
            || self.funding_account_principal_lamports == 0
            || self.credit_refundable_principal_lamports == 0
            || self.credit_tombstone_principal_lamports == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.minimum_balance_lamports()?;
        Ok(())
    }

    /// Exact future credit live-account principal.
    pub fn credit_principal_lamports(&self) -> Result<u64> {
        add(
            self.credit_refundable_principal_lamports,
            self.credit_tombstone_principal_lamports,
        )
    }

    /// Exact minimum balance retained before consumption.
    pub fn minimum_balance_lamports(&self) -> Result<u64> {
        add(
            add(
                self.funding_account_principal_lamports,
                self.credit_principal_lamports()?,
            )?,
            self.donation_floor_lamports,
        )
    }

    /// Exact content identity retained by Fractional's bound prestate.
    pub fn funding_receipt_id(&self) -> Result<Id> {
        self.content_id(DEALER_FUTURE_CREDIT_FUNDING_CONTENT_DOMAIN_V1)
    }

    /// Prepare consumption into the exact post-Resolution facility credit.
    pub fn prepare_consumption(
        &self,
        observed_balance_lamports: u64,
        current_dealer_generation: u64,
        fractional_policy_account_id: Id,
        facility_credit_account_id: Id,
    ) -> Result<DealerFutureCreditConsumptionV1> {
        self.validate()?;
        fractional_policy_account_id.validate_live()?;
        facility_credit_account_id.validate_live()?;
        if current_dealer_generation < self.founding_generation
            || facility_credit_account_id == self.funding_account_id
            || facility_credit_account_id == self.refund_owner
            || facility_credit_account_id == self.neutral_sink
            || fractional_policy_account_id == facility_credit_account_id
        {
            return Err(Error::MismatchedBinding);
        }
        let minimum = self.minimum_balance_lamports()?;
        if observed_balance_lamports < minimum {
            return Err(Error::InvalidParameter);
        }
        let credit_principal_lamports = self.credit_principal_lamports()?;
        let neutral_sink_credit_lamports = observed_balance_lamports
            .checked_sub(self.funding_account_principal_lamports)
            .and_then(|value| value.checked_sub(credit_principal_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        let funding_receipt_id = self.funding_receipt_id()?;
        let terminal_receipt_id = consumption_receipt_id(
            DEALER_FUTURE_CREDIT_CONSUMPTION_RECEIPT_DOMAIN_V1,
            funding_receipt_id,
            current_dealer_generation,
            fractional_policy_account_id,
            facility_credit_account_id,
            observed_balance_lamports,
            self,
        )?;
        Ok(DealerFutureCreditConsumptionV1 {
            funding_account_id: self.funding_account_id,
            funding_receipt_id,
            terminal_receipt_id,
            fractional_policy_account_id,
            facility_credit_account_id,
            refund_owner: self.refund_owner,
            neutral_sink: self.neutral_sink,
            funding_account_principal_lamports: self.funding_account_principal_lamports,
            credit_refundable_principal_lamports: self.credit_refundable_principal_lamports,
            credit_tombstone_principal_lamports: self.credit_tombstone_principal_lamports,
            neutral_sink_credit_lamports,
            observed_balance_lamports,
            current_dealer_generation,
        })
    }

    /// Close unused future-credit capital only at complete Dealer retirement.
    pub fn prepare_unused_close(
        &self,
        observed_balance_lamports: u64,
        terminal_state_receipt_id: Id,
    ) -> Result<DealerFutureCreditUnusedCloseV1> {
        self.validate()?;
        terminal_state_receipt_id.validate_live()?;
        if observed_balance_lamports < self.minimum_balance_lamports()? {
            return Err(Error::InvalidParameter);
        }
        let refundable_principal_lamports = add(
            self.funding_account_principal_lamports,
            self.credit_principal_lamports()?,
        )?;
        let neutral_sink_credit_lamports = observed_balance_lamports
            .checked_sub(refundable_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let funding_receipt_id = self.funding_receipt_id()?;
        let terminal_receipt_id = consumption_receipt_id(
            DEALER_FUTURE_CREDIT_UNUSED_CLOSE_RECEIPT_DOMAIN_V1,
            funding_receipt_id,
            self.founding_generation,
            terminal_state_receipt_id,
            self.funding_account_id,
            observed_balance_lamports,
            self,
        )?;
        Ok(DealerFutureCreditUnusedCloseV1 {
            funding_account_id: self.funding_account_id,
            funding_receipt_id,
            terminal_receipt_id,
            terminal_state_receipt_id,
            refund_owner: self.refund_owner,
            neutral_sink: self.neutral_sink,
            refundable_principal_lamports,
            neutral_sink_credit_lamports,
            observed_balance_lamports,
        })
    }
}

/// Exact one-shot conversion plan consumed inside Fractional action 23.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFutureCreditConsumptionV1 {
    /// Deleted funding account.
    pub funding_account_id: Id,
    /// Exact open funding body identity.
    pub funding_receipt_id: Id,
    /// One-shot close-and-fund receipt.
    pub terminal_receipt_id: Id,
    /// Exact a4/v3 account used in the a6 PDA.
    pub fractional_policy_account_id: Id,
    /// Fresh exact a6/v2 account.
    pub facility_credit_account_id: Id,
    /// Recipient of this account's refundable rent principal.
    pub refund_owner: Id,
    /// Recipient of hostile prefund and later surplus.
    pub neutral_sink: Id,
    /// Refund issued while deleting the funding owner.
    pub funding_account_principal_lamports: u64,
    /// Refundable live-to-tombstone principal placed in a6.
    pub credit_refundable_principal_lamports: u64,
    /// Permanent tombstone principal placed in a6.
    pub credit_tombstone_principal_lamports: u64,
    /// Donation and surplus removed from the funding owner.
    pub neutral_sink_credit_lamports: u64,
    /// Exact funding-account balance consumed.
    pub observed_balance_lamports: u64,
    /// Exact Dealer generation at Resolve.
    pub current_dealer_generation: u64,
}

/// Exact terminal close when the facility never creates an a6 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFutureCreditUnusedCloseV1 {
    /// Deleted funding account.
    pub funding_account_id: Id,
    /// Exact open funding body identity.
    pub funding_receipt_id: Id,
    /// One-shot unused-close receipt.
    pub terminal_receipt_id: Id,
    /// Dealer terminal receipt authorizing the unused close.
    pub terminal_state_receipt_id: Id,
    /// Recipient of both unused principal compartments.
    pub refund_owner: Id,
    /// Recipient of hostile prefund and later surplus.
    pub neutral_sink: Id,
    /// Account plus unused future-credit principal refund.
    pub refundable_principal_lamports: u64,
    /// Donation and surplus disposition.
    pub neutral_sink_credit_lamports: u64,
    /// Exact funding-account balance consumed.
    pub observed_balance_lamports: u64,
}

impl FixedCodec for DealerFutureCreditFundingV1 {
    const ENCODED_LEN: usize = DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FUTURE_CREDIT_FUNDING_MAGIC_V1,
            DEALER_FUTURE_CREDIT_FUNDING_VERSION_V1,
        );
        for identity in [
            self.funding_account_id,
            self.policy_id,
            self.facility_id,
            self.market_instance_v2_id,
            self.realm_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.collateral_value_receipt_id,
            self.dealer_state_account_id,
            self.facility_position_account_id,
            self.facility_position_binding_id,
            self.dealer_replay_account_id,
            self.refund_owner,
            self.neutral_sink,
        ] {
            writer.id(identity);
        }
        writer.u64(self.founding_generation);
        writer.u64(self.funding_account_principal_lamports);
        writer.u64(self.credit_refundable_principal_lamports);
        writer.u64(self.credit_tombstone_principal_lamports);
        writer.u64(self.donation_floor_lamports);
        writer.u64(DEALER_FUTURE_CREDIT_ACCOUNT_BYTES_V1);
        writer.u8(DEALER_FUTURE_CREDIT_ACCOUNT_VERSION_V1);
        writer.reserved(7);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FUTURE_CREDIT_FUNDING_MAGIC_V1,
            DEALER_FUTURE_CREDIT_FUNDING_VERSION_V1,
        )?;
        let value = Self {
            funding_account_id: reader.id(),
            policy_id: reader.id(),
            facility_id: reader.id(),
            market_instance_v2_id: reader.id(),
            realm_id: reader.id(),
            collateral_policy_id: reader.id(),
            collateral_release_id: reader.id(),
            collateral_value_receipt_id: reader.id(),
            dealer_state_account_id: reader.id(),
            facility_position_account_id: reader.id(),
            facility_position_binding_id: reader.id(),
            dealer_replay_account_id: reader.id(),
            refund_owner: reader.id(),
            neutral_sink: reader.id(),
            founding_generation: reader.u64(),
            funding_account_principal_lamports: reader.u64(),
            credit_refundable_principal_lamports: reader.u64(),
            credit_tombstone_principal_lamports: reader.u64(),
            donation_floor_lamports: reader.u64(),
        };
        if reader.u64() != DEALER_FUTURE_CREDIT_ACCOUNT_BYTES_V1
            || reader.u8() != DEALER_FUTURE_CREDIT_ACCOUNT_VERSION_V1
        {
            return Err(Error::MismatchedBinding);
        }
        reader.reserved(7)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn consumption_receipt_id(
    domain: &[u8],
    funding_receipt_id: Id,
    generation: u64,
    first: Id,
    second: Id,
    observed_balance_lamports: u64,
    funding: &DealerFutureCreditFundingV1,
) -> Result<Id> {
    funding_receipt_id.validate_live()?;
    first.validate_live()?;
    second.validate_live()?;
    if generation == 0 {
        return Err(Error::InvalidParameter);
    }
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(funding_receipt_id.bytes());
    hasher.update(generation.to_le_bytes());
    hasher.update(first.bytes());
    hasher.update(second.bytes());
    hasher.update(observed_balance_lamports.to_le_bytes());
    hasher.update(funding.refund_owner.bytes());
    hasher.update(funding.neutral_sink.bytes());
    hasher.update(funding.funding_account_principal_lamports.to_le_bytes());
    hasher.update(funding.credit_refundable_principal_lamports.to_le_bytes());
    hasher.update(funding.credit_tombstone_principal_lamports.to_le_bytes());
    hasher.update(funding.donation_floor_lamports.to_le_bytes());
    Ok(Id::from_bytes(hasher.finalize().into()))
}

const _: () = assert!(DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1 == 516);
const _: () = assert!(DEALER_FUTURE_CREDIT_ACCOUNT_BYTES_V1 == 296);
const _: () = assert!(DEALER_FUTURE_CREDIT_ACCOUNT_VERSION_V1 == 2);
const _: () = assert!(DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn funding() -> DealerFutureCreditFundingV1 {
        DealerFutureCreditFundingV1 {
            funding_account_id: id(1),
            policy_id: id(2),
            facility_id: id(3),
            market_instance_v2_id: id(4),
            realm_id: id(5),
            collateral_policy_id: id(6),
            collateral_release_id: id(7),
            collateral_value_receipt_id: id(8),
            dealer_state_account_id: id(9),
            facility_position_account_id: id(10),
            facility_position_binding_id: id(11),
            dealer_replay_account_id: id(12),
            refund_owner: id(13),
            neutral_sink: id(14),
            founding_generation: 1,
            funding_account_principal_lamports: 100,
            credit_refundable_principal_lamports: 20,
            credit_tombstone_principal_lamports: 80,
            donation_floor_lamports: 7,
        }
    }

    #[test]
    fn codec_and_consumption_preserve_both_principal_compartments() {
        let value = funding();
        let mut bytes = [0u8; DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(DealerFutureCreditFundingV1::decode(&bytes), Ok(value));
        let plan = value.prepare_consumption(212, 9, id(15), id(16)).unwrap();
        assert_eq!(plan.funding_account_principal_lamports, 100);
        assert_eq!(plan.credit_refundable_principal_lamports, 20);
        assert_eq!(plan.credit_tombstone_principal_lamports, 80);
        assert_eq!(plan.neutral_sink_credit_lamports, 12);
    }

    #[test]
    fn underfunding_alias_and_schema_substitution_refuse() {
        let value = funding();
        assert_eq!(
            value.prepare_consumption(206, 1, id(15), id(16)),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            value.prepare_consumption(207, 1, id(15), value.funding_account_id),
            Err(Error::MismatchedBinding)
        );
        let mut bytes = [0u8; DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1];
        value.encode_into(&mut bytes).unwrap();
        bytes[DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1 - 8] = 3;
        assert_eq!(
            DealerFutureCreditFundingV1::decode(&bytes),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn unused_close_refunds_both_principals_and_neutralizes_donation() {
        let value = funding();
        let plan = value.prepare_unused_close(217, id(15)).unwrap();
        assert_eq!(plan.refundable_principal_lamports, 200);
        assert_eq!(plan.neutral_sink_credit_lamports, 17);
        assert_ne!(plan.terminal_receipt_id, plan.funding_receipt_id);
    }
}
