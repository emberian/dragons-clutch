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
    add, DealerPhaseV2, DealerSeriesObligationBindingV2, DealerSeriesObligationPhaseV1,
    DealerStateV3, Error, FixedCodec, Id, Result,
    DEALER_FUTURE_CREDIT_FUNDING_CONTENT_DOMAIN_V1,
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

    /// Close unused future-credit capital after Dealer Position/Replay terminalization.
    ///
    /// Product terminality is deliberately not an input. This value owner
    /// closes its own unused custody first and emits a Dealer-owned receipt
    /// which a later atomic Product composer may consume.
    pub fn prepare_unused_close(
        &self,
        dealer_state_account_id: Id,
        state: &DealerStateV3,
        live_obligation: &DealerSeriesObligationBindingV2,
        observed_balance_lamports: u64,
    ) -> Result<DealerFutureCreditUnusedCloseV1> {
        self.validate()?;
        state.validate()?;
        live_obligation.validate()?;
        dealer_state_account_id.validate_live()?;
        let terminal_state_receipt_id = state.base.terminal_state_receipt_id;
        terminal_state_receipt_id.validate_live()?;
        let dealer_obligation_presemantic_id = live_obligation.binding_id()?;
        let state_pre_semantic_id = state.state_id()?;
        if state.base.phase != DealerPhaseV2::Retiring
            || state.base.children.facility_positions != 0
            || state.base.children.facility_replays != 0
            || state.series_obligation_children != 1
            || state.series_obligation_binding_account_id
                != live_obligation.key.binding_account_id
            || state.series_obligation_binding_id != dealer_obligation_presemantic_id
            || live_obligation.phase != DealerSeriesObligationPhaseV1::Live
            || live_obligation.key.dealer_state_account_id != dealer_state_account_id
            || live_obligation.key.policy_id != state.base.policy_id
            || live_obligation.key.facility_id != state.base.facility_id
            || live_obligation.key.facility_position_binding_id
                != state.base.facility_position_binding_id
            || live_obligation.key.market_instance_v2_id != self.market_instance_v2_id
            || self.dealer_state_account_id != dealer_state_account_id
            || self.policy_id != state.base.policy_id
            || self.facility_id != state.base.facility_id
            || self.facility_position_account_id != state.base.facility_position_account_id
            || self.facility_position_binding_id != state.base.facility_position_binding_id
            || self.dealer_replay_account_id != state.base.facility_replay_account_id
            || self.neutral_sink != state.base.rent.neutral_sink
            || self.neutral_sink != live_obligation.rent.neutral_sink
            || state.base.generation < self.founding_generation
        {
            return Err(Error::MismatchedBinding);
        }
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
        let terminal_receipt_id = unused_close_receipt_id(
            funding_receipt_id,
            state_pre_semantic_id,
            terminal_state_receipt_id,
            dealer_obligation_presemantic_id,
            observed_balance_lamports,
            self,
        )?;
        Ok(DealerFutureCreditUnusedCloseV1 {
            funding_account_id: self.funding_account_id,
            funding_receipt_id,
            terminal_receipt_id,
            state_pre_semantic_id,
            terminal_state_receipt_id,
            dealer_obligation_presemantic_id,
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
    /// Exact Retiring State V3 semantic identity before value movement.
    pub state_pre_semantic_id: Id,
    /// Dealer terminal receipt authorizing the unused close.
    pub terminal_state_receipt_id: Id,
    /// Exact live Dealer obligation body consumed as the terminal prestate.
    pub dealer_obligation_presemantic_id: Id,
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

fn unused_close_receipt_id(
    funding_receipt_id: Id,
    state_pre_semantic_id: Id,
    terminal_state_receipt_id: Id,
    dealer_obligation_presemantic_id: Id,
    observed_balance_lamports: u64,
    funding: &DealerFutureCreditFundingV1,
) -> Result<Id> {
    for identity in [
        funding_receipt_id,
        state_pre_semantic_id,
        terminal_state_receipt_id,
        dealer_obligation_presemantic_id,
    ] {
        identity.validate_live()?;
    }
    let mut hasher = Sha256::new();
    hasher.update(DEALER_FUTURE_CREDIT_UNUSED_CLOSE_RECEIPT_DOMAIN_V1);
    hasher.update(funding_receipt_id.bytes());
    hasher.update(state_pre_semantic_id.bytes());
    hasher.update(terminal_state_receipt_id.bytes());
    hasher.update(dealer_obligation_presemantic_id.bytes());
    hasher.update(observed_balance_lamports.to_le_bytes());
    hasher.update(funding.funding_account_id.bytes());
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
    use crate::{
        DealerChildCountsV2, DealerSeriesObligationKeyV2, DeletableRentOwnerV1,
        RootRentOwnerV1, SponsorCapitalDispositionV1, MAX_OUTCOMES,
    };

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

    fn terminal_graph() -> (DealerStateV3, DealerSeriesObligationBindingV2) {
        let key = DealerSeriesObligationKeyV2 {
                binding_account_id: id(30),
                policy_id: id(2),
                facility_id: id(3),
                dealer_state_account_id: id(9),
                facility_position_binding_id: id(11),
                market_instance_v2_id: id(4),
                product_market_root_account_id: id(31),
                product_market_binding_id: id(32),
                series_plan_v5_id: id(33),
                series_market_link_account_id: id(34),
                compiler_bundle_v6_id: id(35),
                attachment_plan_v5_id: id(36),
                product_generation: 1,
                series_ordinal: 7,
            };
        let live = DealerSeriesObligationBindingV2::new_live(
            key,
            key.admission_owner_receipt_id(id(39), 1).unwrap(),
            id(38),
            id(39),
            id(40),
            1,
            DeletableRentOwnerV1 {
                payer: id(41),
                neutral_sink: id(14),
                refundable_principal: 9,
                donation_floor: 2,
            },
        )
        .unwrap();
        let terminal_owner_receipt = live
            .terminal_owner_receipt_id(id(26), id(44), 2)
            .unwrap();
        let terminal = live
            .terminalized(
                terminal_owner_receipt,
                id(43),
                id(44),
                id(45),
                id(26),
                2,
            )
            .unwrap();
        let base = crate::DealerStateV2 {
            policy_id: id(2),
            facility_id: id(3),
            facility_position_binding_id: id(11),
            facility_position_id: id(46),
            facility_position_account_id: id(10),
            facility_replay_account_id: id(12),
            sponsor: id(47),
            sponsor_refund_recipient: id(48),
            lp_page_head_id: Id::ZERO,
            lp_page_set_root: Id::ZERO,
            last_lp_owner: Id::ZERO,
            active_epoch_id: Id::ZERO,
            active_epoch_binding_account_id: Id::ZERO,
            active_lease_id: Id::ZERO,
            funded_dependencies_id: id(49),
            funded_dependencies_account_id: id(50),
            terminal_position_tombstone_id: id(51),
            terminal_replay_semantic_id: id(52),
            terminal_replay_intent_id: id(53),
            terminal_state_receipt_id: id(26),
            phase: DealerPhaseV2::Retiring,
            sponsor_capital_disposition: SponsorCapitalDispositionV1::Donated,
            outcome_count: 2,
            generation: 2,
            child_sequence: 4,
            total_shares: 0,
            queued_shares: 0,
            terminal_claimed_shares: 0,
            sponsor_capital_atoms: 1,
            net_sold: [0; MAX_OUTCOMES],
            children: DealerChildCountsV2 {
                funded_dependencies: 1,
                ..DealerChildCountsV2::default()
            },
            rent: RootRentOwnerV1 {
                payer: id(54),
                neutral_sink: id(14),
                refundable_live_principal: 3,
                permanent_tombstone_principal: 5,
                donation_floor: 0,
            },
        };
        let state = DealerStateV3 {
            base,
            series_obligation_binding_id: terminal.binding_id().unwrap(),
            series_obligation_binding_account_id: terminal.key.binding_account_id,
            series_obligation_children: 1,
            product_upgrade_rent: DeletableRentOwnerV1 {
                payer: id(55),
                neutral_sink: id(14),
                refundable_principal: 1,
                donation_floor: 0,
            },
        };
        state.validate().unwrap();
        (state, terminal)
    }

    fn value_terminal_graph() -> (DealerStateV3, DealerSeriesObligationBindingV2) {
        let (mut state, mut live) = terminal_graph();
        live.phase = DealerSeriesObligationPhaseV1::Live;
        live.terminal_owner_receipt_id = Id::ZERO;
        live.terminal_projection_id = Id::ZERO;
        live.terminal_link_pre_semantic_id = Id::ZERO;
        live.terminal_link_post_semantic_id = Id::ZERO;
        live.terminal_state_receipt_id = Id::ZERO;
        live.terminal_link_transition_sequence = 0;
        state.series_obligation_binding_id = live.binding_id().unwrap();
        state.validate().unwrap();
        (state, live)
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
        let (state, live) = value_terminal_graph();
        let plan = value
            .prepare_unused_close(id(9), &state, &live, 217)
            .unwrap();
        assert_eq!(plan.refundable_principal_lamports, 200);
        assert_eq!(plan.neutral_sink_credit_lamports, 17);
        assert_eq!(plan.terminal_state_receipt_id, id(26));
        assert_eq!(plan.dealer_obligation_presemantic_id, live.binding_id().unwrap());
        assert_ne!(plan.terminal_receipt_id, plan.funding_receipt_id);
    }

    #[test]
    fn unused_close_refuses_terminal_product_or_substituted_terminal_state() {
        let value = funding();
        let (state, terminal) = terminal_graph();
        assert_eq!(
            value.prepare_unused_close(id(9), &state, &terminal, 217),
            Err(Error::MismatchedBinding)
        );
        let (state, live) = value_terminal_graph();
        let mut substituted = state;
        substituted.base.terminal_state_receipt_id = id(56);
        assert_eq!(
            value.prepare_unused_close(id(9), &substituted, &live, 217),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn current_obligation_close_decrements_only_the_exact_terminal_child() {
        let (state, terminal) = terminal_graph();
        let close = crate::prepare_dealer_series_obligation_close_v2(state, &terminal, 13)
            .unwrap();
        assert_eq!(close.state_after.series_obligation_children, 0);
        assert_eq!(close.rent_payer_credit_lamports, 9);
        assert_eq!(close.neutral_sink_credit_lamports, 4);
        assert_eq!(
            crate::prepare_dealer_series_obligation_close_v2(state, &terminal, 10),
            Err(Error::InvalidPhase)
        );
        let mut substituted = terminal;
        substituted.key.binding_account_id = id(56);
        assert_eq!(
            crate::prepare_dealer_series_obligation_close_v2(state, &substituted, 13),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn value_receipt_closes_only_the_exact_live_current_obligation() {
        let (state, live) = value_terminal_graph();
        let value_receipt = id(61);
        let close = crate::prepare_dealer_series_obligation_value_close_v3(
            state,
            &live,
            value_receipt,
            13,
        )
        .unwrap();
        assert_eq!(close.state_after.series_obligation_children, 0);
        assert_eq!(close.live_binding_id, live.binding_id().unwrap());
        assert_eq!(close.dealer_value_terminal_receipt_id, value_receipt);
        assert_ne!(close.close_receipt_id, value_receipt);
        assert_eq!(close.rent_payer_credit_lamports, 9);
        assert_eq!(close.neutral_sink_credit_lamports, 4);

        let (_, terminal) = terminal_graph();
        assert_eq!(
            crate::prepare_dealer_series_obligation_value_close_v3(
                state,
                &terminal,
                value_receipt,
                13,
            ),
            Err(Error::InvalidPhase)
        );
        assert_eq!(
            crate::prepare_dealer_series_obligation_value_close_v3(
                state,
                &live,
                Id::ZERO,
                13,
            ),
            Err(Error::ZeroIdentity)
        );
    }
}
