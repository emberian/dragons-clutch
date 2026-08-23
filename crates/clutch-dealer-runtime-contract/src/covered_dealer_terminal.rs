// SPDX-License-Identifier: AGPL-3.0-or-later

//! In-place terminal successor for one counted CoveredDealer attachment.
//!
//! The General settlement root remains the exhaustive owner of the opaque
//! Dealer-child count.  Finalize or pre-collection Abort replaces the live
//! selection body with this successor without closing its account or changing
//! its rent ownership.  A later General-owned retirement bridge may consume
//! the authenticated postwrite only after every ordinary General settlement
//! child is terminal; this body alone never decrements a General count.

use clutch_fee_runtime_contract::terminal::FeeTerminalOutcomeV1;
use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CoveredDealerSelectionV1, DealerActionReceiptV1, DealerFacilityReplayV1,
    DealerFeeTerminalJoinV1, DealerLeaseV2, DealerRuntimeActionV1, DealerStateV2,
    DeletableRentOwnerV1, Error, FixedCodec, Id, PreparedDealerLeasePotCloseV3, Result,
    SettlementPotV2, DEALER_COVERED_SELECTION_BYTES_V1, DELETABLE_RENT_OWNER_BYTES,
};

/// Inner magic for the terminal successor of tag `0xae`.
pub const DEALER_COVERED_TERMINAL_MAGIC_V2: [u8; 8] = *b"DCCOVDV2";
/// Inner version of the terminal successor.
pub const DEALER_COVERED_TERMINAL_VERSION_V2: u16 = 2;
/// The terminal successor preserves the live selection allocation exactly.
pub const DEALER_COVERED_TERMINAL_BYTES_V2: usize = DEALER_COVERED_SELECTION_BYTES_V1;
/// Semantic domain of the authenticated terminal postwrite.
pub const DEALER_COVERED_TERMINAL_CONTENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-covered-terminal/v2\0";

const TERMINAL_IDENTITY_COUNT_V2: usize = 33;
const TERMINAL_U64_COUNT_V2: usize = 7;
const TERMINAL_SCALAR_BYTES_V2: usize = 8;
const TERMINAL_RESERVED_BYTES_V2: usize = DEALER_COVERED_TERMINAL_BYTES_V2
    - HEADER_BYTES
    - (TERMINAL_IDENTITY_COUNT_V2 * 32)
    - (TERMINAL_U64_COUNT_V2 * 8)
    - TERMINAL_SCALAR_BYTES_V2
    - DELETABLE_RENT_OWNER_BYTES;

/// Persisted, rent-owned proof that Dealer economics reached one exact terminal postwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveredDealerTerminalV2 {
    selection_account_id: Id,
    selection_pre_semantic_id: Id,
    policy_id: Id,
    facility_id: Id,
    facility_position_binding_id: Id,
    dealer_state_account_id: Id,
    state_pre_content_id: Id,
    state_post_content_id: Id,
    market_instance_v2_id: Id,
    epoch_id: Id,
    epoch_binding_account_id: Id,
    settlement_root_account_id: Id,
    retained_feed_account_id: Id,
    order_set_id: Id,
    settlement_candidate_id: Id,
    selected_fee_record_account_id: Id,
    selected_fee_record_semantic_id: Id,
    selected_fee_binding_digest: Id,
    fee_revenue_policy_id: Id,
    lease_account_id: Id,
    lease_pre_semantic_id: Id,
    settlement_pot_account_id: Id,
    pot_pre_content_id: Id,
    lease_pot_close_transition_id: Id,
    facility_position_account_id: Id,
    position_pre_semantic_id: Id,
    position_post_semantic_id: Id,
    facility_replay_account_id: Id,
    replay_pre_semantic_id: Id,
    replay_post_semantic_id: Id,
    liveness_receipt_account_id: Id,
    liveness_receipt_semantic_id: Id,
    fee_terminal_receipt_id: Id,
    dealer_generation_before: u64,
    dealer_generation_after: u64,
    general_epoch_generation: u64,
    selected_ordinal: u64,
    replay_ordinal_before: u64,
    replay_ordinal_after: u64,
    terminal_slot: u64,
    action: DealerRuntimeActionV1,
    outcome: FeeTerminalOutcomeV1,
    stored_bump: u8,
    rent: DeletableRentOwnerV1,
}

impl CoveredDealerTerminalV2 {
    /// Construct only from the exact pure close capability and its authenticated prestates.
    #[allow(clippy::too_many_arguments)]
    pub fn from_prepared(
        selection: &CoveredDealerSelectionV1,
        state_before: &DealerStateV2,
        lease: &DealerLeaseV2,
        pot: &SettlementPotV2,
        replay_before: &DealerFacilityReplayV1,
        fee_terminal: &DealerFeeTerminalJoinV1,
        action_receipt: &DealerActionReceiptV1,
        prepared: PreparedDealerLeasePotCloseV3,
        terminal_slot: u64,
    ) -> Result<Self> {
        selection.validate()?;
        state_before.validate()?;
        lease.validate()?;
        pot.validate_against_lease(lease)?;
        replay_before.validate()?;
        action_receipt.validate()?;
        let state_after = prepared.state_after();
        let close = prepared.close();
        let transfer = prepared.transfer();
        let bundle = transfer.bundle();
        let replay_after = prepared.replay().replay_post();
        let selection_pre_semantic_id = selection.selection_id()?;
        let state_pre_content_id = state_before.state_content_id()?;
        let state_post_content_id = state_after.state_content_id()?;
        let lease_pre_semantic_id = lease.lease_id()?;
        let replay_pre_semantic_id = replay_before.replay_id()?;
        let replay_post_semantic_id = replay_after.replay_id()?;
        let liveness_receipt_semantic_id = action_receipt.semantic_receipt_id()?;
        let replay_ordinal_after = replay_before
            .next_transition_ordinal()
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if !matches!(
            close.action(),
            DealerRuntimeActionV1::FinalizeSettlement
                | DealerRuntimeActionV1::AbortBeforeCollection
        ) || close.action() != action_receipt.action
            || close.action() != action_for_outcome(fee_terminal.outcome)
            || terminal_slot == 0
            || selection.policy_id != state_before.policy_id
            || selection.facility_id != state_before.facility_id
            || selection.facility_position_binding_id
                != state_before.facility_position_binding_id
            || selection.dealer_state_account_id != close.state_account_id()
            || selection.lease_account_id != lease.lease_account_id
            || selection.settlement_pot_account_id != close.pot_account_id()
            || selection.settlement_candidate_id != fee_terminal.settlement_candidate_id
            || selection.selected_fee_record_account_id
                != fee_terminal.selected_fee_record_account_id
            || selection.selected_fee_record_semantic_id
                != fee_terminal.selected_fee_record_semantic_id
            || selection.fee_revenue_policy_id != fee_terminal.fee_revenue_policy_id
            || selection.dealer_generation != state_before.generation
            || lease.post_generation != state_after.generation
            || state_after.children.leases != 0
            || state_after.children.settlement_pots != 0
            || !state_after.active_lease_id.is_zero()
            || close.pot_pre_content_id() != pot.pot_content_id()?
            || close.position_account_id() != state_before.facility_position_account_id
            || close.position_pre_semantic_id() != state_before.facility_position_id
            || close.fee_terminal_receipt_id() != fee_terminal.terminal_receipt_id
            || close.liveness_receipt_id() != liveness_receipt_semantic_id
            || bundle.destination_account_id != state_before.facility_position_account_id
            || bundle.destination_pre_semantic_id != state_before.facility_position_id
            || bundle.destination_post_semantic_id != state_after.facility_position_id
            || replay_before.replay_account_id() != state_before.facility_replay_account_id
            || replay_after.replay_account_id() != state_before.facility_replay_account_id
            || replay_after.position_generation() != state_after.generation
            || replay_after.next_transition_ordinal() != replay_ordinal_after
            || action_receipt.policy_id != selection.policy_id
            || action_receipt.facility_id != selection.facility_id
            || action_receipt.dealer_state_account_id != selection.dealer_state_account_id
            || action_receipt.replay_account_id != state_before.facility_replay_account_id
            || action_receipt.facility_generation != state_before.generation
            || action_receipt.expected_replay_ordinal
                != replay_before.next_transition_ordinal()
            || action_receipt.receipt_account_id == selection.selection_account_id
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            selection_account_id: selection.selection_account_id,
            selection_pre_semantic_id,
            policy_id: selection.policy_id,
            facility_id: selection.facility_id,
            facility_position_binding_id: selection.facility_position_binding_id,
            dealer_state_account_id: selection.dealer_state_account_id,
            state_pre_content_id,
            state_post_content_id,
            market_instance_v2_id: selection.market_instance_v2_id,
            epoch_id: selection.epoch_id,
            epoch_binding_account_id: selection.epoch_binding_account_id,
            settlement_root_account_id: selection.settlement_root_account_id,
            retained_feed_account_id: selection.retained_feed_account_id,
            order_set_id: selection.order_set_id,
            settlement_candidate_id: selection.settlement_candidate_id,
            selected_fee_record_account_id: selection.selected_fee_record_account_id,
            selected_fee_record_semantic_id: selection.selected_fee_record_semantic_id,
            selected_fee_binding_digest: selection.selected_fee_binding_digest,
            fee_revenue_policy_id: selection.fee_revenue_policy_id,
            lease_account_id: selection.lease_account_id,
            lease_pre_semantic_id,
            settlement_pot_account_id: selection.settlement_pot_account_id,
            pot_pre_content_id: close.pot_pre_content_id(),
            lease_pot_close_transition_id: close.transition_id(),
            facility_position_account_id: state_before.facility_position_account_id,
            position_pre_semantic_id: state_before.facility_position_id,
            position_post_semantic_id: state_after.facility_position_id,
            facility_replay_account_id: state_before.facility_replay_account_id,
            replay_pre_semantic_id,
            replay_post_semantic_id,
            liveness_receipt_account_id: action_receipt.receipt_account_id,
            liveness_receipt_semantic_id,
            fee_terminal_receipt_id: fee_terminal.terminal_receipt_id,
            dealer_generation_before: state_before.generation,
            dealer_generation_after: state_after.generation,
            general_epoch_generation: selection.general_epoch_generation,
            selected_ordinal: selection.selected_ordinal,
            replay_ordinal_before: replay_before.next_transition_ordinal(),
            replay_ordinal_after,
            terminal_slot,
            action: close.action(),
            outcome: fee_terminal.outcome,
            stored_bump: selection.stored_bump,
            rent: selection.rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate the persisted postwrite without granting account authority.
    pub fn validate(&self) -> Result<()> {
        for identity in self.identities() {
            identity.validate_live()?;
        }
        self.rent.validate()?;
        let physical = [
            self.selection_account_id,
            self.dealer_state_account_id,
            self.epoch_binding_account_id,
            self.settlement_root_account_id,
            self.retained_feed_account_id,
            self.selected_fee_record_account_id,
            self.lease_account_id,
            self.settlement_pot_account_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.liveness_receipt_account_id,
            self.fee_terminal_receipt_id,
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
        if self.action != action_for_outcome(self.outcome)
            || self.dealer_generation_before == 0
            || self.dealer_generation_after
                != self
                    .dealer_generation_before
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || self.general_epoch_generation == 0
            || self.selected_ordinal == 0
            || self.replay_ordinal_after
                != self
                    .replay_ordinal_before
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || self.terminal_slot == 0
            || self.position_pre_semantic_id == self.position_post_semantic_id
            || self.replay_pre_semantic_id == self.replay_post_semantic_id
            || self.state_pre_content_id == self.state_post_content_id
            || self.rent.payer == self.rent.neutral_sink
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Canonical semantic identity of this terminal postwrite.
    pub fn terminal_id(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(DEALER_COVERED_TERMINAL_CONTENT_DOMAIN_V2);
        for identity in self.identities() {
            hasher.update(identity.bytes());
        }
        for value in self.numbers() {
            hasher.update(value.to_le_bytes());
        }
        hasher.update([
            action_byte(self.action),
            outcome_byte(self.outcome),
            self.stored_bump,
        ]);
        hasher.update(self.rent.payer.bytes());
        hasher.update(self.rent.neutral_sink.bytes());
        hasher.update(self.rent.refundable_principal.to_le_bytes());
        hasher.update(self.rent.donation_floor.to_le_bytes());
        let value = Id::from_bytes(hasher.finalize().into());
        value.validate_live()?;
        Ok(value)
    }

    /// Physical counted attachment account retained for General retirement.
    pub const fn selection_account_id(&self) -> Id { self.selection_account_id }
    /// Semantic identity of the live selection body replaced atomically.
    pub const fn selection_pre_semantic_id(&self) -> Id { self.selection_pre_semantic_id }
    /// Counted General settlement root.
    pub const fn settlement_root_account_id(&self) -> Id { self.settlement_root_account_id }
    /// Final settlement candidate.
    pub const fn settlement_candidate_id(&self) -> Id { self.settlement_candidate_id }
    /// Full Product Market identity.
    pub const fn market_instance_v2_id(&self) -> Id { self.market_instance_v2_id }
    /// Dealer policy.
    pub const fn policy_id(&self) -> Id { self.policy_id }
    /// Dealer facility.
    pub const fn facility_id(&self) -> Id { self.facility_id }
    /// Canonical facility Position-purpose binding.
    pub const fn facility_position_binding_id(&self) -> Id { self.facility_position_binding_id }
    /// Authoritative Dealer State account.
    pub const fn dealer_state_account_id(&self) -> Id { self.dealer_state_account_id }
    /// Semantic Dealer Epoch.
    pub const fn epoch_id(&self) -> Id { self.epoch_id }
    /// Counted Dealer Epoch-binding account.
    pub const fn epoch_binding_account_id(&self) -> Id { self.epoch_binding_account_id }
    /// General retained Feed account.
    pub const fn retained_feed_account_id(&self) -> Id { self.retained_feed_account_id }
    /// Frozen General order-set identity.
    pub const fn order_set_id(&self) -> Id { self.order_set_id }
    /// Selected fee-record account.
    pub const fn selected_fee_record_account_id(&self) -> Id { self.selected_fee_record_account_id }
    /// Selected fee-record semantic preidentity.
    pub const fn selected_fee_record_semantic_id(&self) -> Id { self.selected_fee_record_semantic_id }
    /// Exact immutable selected-fee projection digest.
    pub const fn selected_fee_binding_digest(&self) -> Id { self.selected_fee_binding_digest }
    /// Exact owner-netted revenue-policy identity.
    pub const fn fee_revenue_policy_id(&self) -> Id { self.fee_revenue_policy_id }
    /// Deleted Lease account.
    pub const fn lease_account_id(&self) -> Id { self.lease_account_id }
    /// Deleted SettlementPot account.
    pub const fn settlement_pot_account_id(&self) -> Id { self.settlement_pot_account_id }
    /// Facility Position account.
    pub const fn facility_position_account_id(&self) -> Id { self.facility_position_account_id }
    /// Facility Replay account.
    pub const fn facility_replay_account_id(&self) -> Id { self.facility_replay_account_id }
    /// General Epoch generation.
    pub const fn general_epoch_generation(&self) -> u64 { self.general_epoch_generation }
    /// Window-owned selected ordinal.
    pub const fn selected_ordinal(&self) -> u64 { self.selected_ordinal }
    /// Exact terminal action.
    pub const fn action(&self) -> DealerRuntimeActionV1 { self.action }
    /// Fee-owned terminal outcome.
    pub const fn outcome(&self) -> FeeTerminalOutcomeV1 { self.outcome }
    /// Fee terminal receipt.
    pub const fn fee_terminal_receipt_id(&self) -> Id { self.fee_terminal_receipt_id }
    /// State postimage.
    pub const fn state_post_content_id(&self) -> Id { self.state_post_content_id }
    /// Dealer generation before the terminal action.
    pub const fn dealer_generation_before(&self) -> u64 { self.dealer_generation_before }
    /// Dealer generation after the terminal action.
    pub const fn dealer_generation_after(&self) -> u64 { self.dealer_generation_after }
    /// Facility Position postimage.
    pub const fn position_post_semantic_id(&self) -> Id { self.position_post_semantic_id }
    /// Facility Replay postimage.
    pub const fn replay_post_semantic_id(&self) -> Id { self.replay_post_semantic_id }
    /// Replay ordinal consumed by the terminal action.
    pub const fn replay_ordinal_before(&self) -> u64 { self.replay_ordinal_before }
    /// Replay ordinal after the terminal action.
    pub const fn replay_ordinal_after(&self) -> u64 { self.replay_ordinal_after }
    /// Clock slot authenticated by the terminal action.
    pub const fn terminal_slot(&self) -> u64 { self.terminal_slot }
    /// Lease/Pot deletion transition.
    pub const fn lease_pot_close_transition_id(&self) -> Id { self.lease_pot_close_transition_id }
    /// Terminal liveness receipt account.
    pub const fn liveness_receipt_account_id(&self) -> Id { self.liveness_receipt_account_id }
    /// Terminal liveness receipt semantic identity.
    pub const fn liveness_receipt_semantic_id(&self) -> Id { self.liveness_receipt_semantic_id }
    /// Retained rent owner of the counted attachment.
    pub const fn rent(&self) -> DeletableRentOwnerV1 { self.rent }
    /// Stored PDA bump.
    pub const fn stored_bump(&self) -> u8 { self.stored_bump }

    fn identities(&self) -> [Id; TERMINAL_IDENTITY_COUNT_V2] {
        [
            self.selection_account_id, self.selection_pre_semantic_id, self.policy_id,
            self.facility_id, self.facility_position_binding_id, self.dealer_state_account_id,
            self.state_pre_content_id, self.state_post_content_id, self.market_instance_v2_id,
            self.epoch_id, self.epoch_binding_account_id, self.settlement_root_account_id,
            self.retained_feed_account_id, self.order_set_id, self.settlement_candidate_id,
            self.selected_fee_record_account_id, self.selected_fee_record_semantic_id,
            self.selected_fee_binding_digest, self.fee_revenue_policy_id, self.lease_account_id,
            self.lease_pre_semantic_id, self.settlement_pot_account_id, self.pot_pre_content_id,
            self.lease_pot_close_transition_id, self.facility_position_account_id,
            self.position_pre_semantic_id, self.position_post_semantic_id,
            self.facility_replay_account_id, self.replay_pre_semantic_id,
            self.replay_post_semantic_id, self.liveness_receipt_account_id,
            self.liveness_receipt_semantic_id, self.fee_terminal_receipt_id,
        ]
    }

    const fn numbers(&self) -> [u64; TERMINAL_U64_COUNT_V2] {
        [
            self.dealer_generation_before, self.dealer_generation_after,
            self.general_epoch_generation, self.selected_ordinal,
            self.replay_ordinal_before, self.replay_ordinal_after, self.terminal_slot,
        ]
    }
}

impl FixedCodec for CoveredDealerTerminalV2 {
    const ENCODED_LEN: usize = DEALER_COVERED_TERMINAL_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_COVERED_TERMINAL_MAGIC_V2, DEALER_COVERED_TERMINAL_VERSION_V2);
        for identity in self.identities() { writer.id(identity); }
        for value in self.numbers() { writer.u64(value); }
        writer.u8(action_byte(self.action));
        writer.u8(outcome_byte(self.outcome));
        writer.u8(self.stored_bump);
        writer.reserved(5);
        writer.reserved(TERMINAL_RESERVED_BYTES_V2);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_COVERED_TERMINAL_MAGIC_V2, DEALER_COVERED_TERMINAL_VERSION_V2)?;
        let mut identities = [Id::ZERO; TERMINAL_IDENTITY_COUNT_V2];
        let mut index = 0usize;
        while index < identities.len() { identities[index] = reader.id(); index += 1; }
        let mut numbers = [0u64; TERMINAL_U64_COUNT_V2];
        index = 0;
        while index < numbers.len() { numbers[index] = reader.u64(); index += 1; }
        let action = decode_action(reader.u8())?;
        let outcome = decode_outcome(reader.u8())?;
        let stored_bump = reader.u8();
        reader.reserved(5)?;
        reader.reserved(TERMINAL_RESERVED_BYTES_V2)?;
        let value = Self {
            selection_account_id: identities[0], selection_pre_semantic_id: identities[1],
            policy_id: identities[2], facility_id: identities[3],
            facility_position_binding_id: identities[4], dealer_state_account_id: identities[5],
            state_pre_content_id: identities[6], state_post_content_id: identities[7],
            market_instance_v2_id: identities[8], epoch_id: identities[9],
            epoch_binding_account_id: identities[10], settlement_root_account_id: identities[11],
            retained_feed_account_id: identities[12], order_set_id: identities[13],
            settlement_candidate_id: identities[14], selected_fee_record_account_id: identities[15],
            selected_fee_record_semantic_id: identities[16], selected_fee_binding_digest: identities[17],
            fee_revenue_policy_id: identities[18], lease_account_id: identities[19],
            lease_pre_semantic_id: identities[20], settlement_pot_account_id: identities[21],
            pot_pre_content_id: identities[22], lease_pot_close_transition_id: identities[23],
            facility_position_account_id: identities[24], position_pre_semantic_id: identities[25],
            position_post_semantic_id: identities[26], facility_replay_account_id: identities[27],
            replay_pre_semantic_id: identities[28], replay_post_semantic_id: identities[29],
            liveness_receipt_account_id: identities[30], liveness_receipt_semantic_id: identities[31],
            fee_terminal_receipt_id: identities[32], dealer_generation_before: numbers[0],
            dealer_generation_after: numbers[1], general_epoch_generation: numbers[2],
            selected_ordinal: numbers[3], replay_ordinal_before: numbers[4],
            replay_ordinal_after: numbers[5], terminal_slot: numbers[6], action, outcome,
            stored_bump, rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const fn action_for_outcome(outcome: FeeTerminalOutcomeV1) -> DealerRuntimeActionV1 {
    match outcome {
        FeeTerminalOutcomeV1::Settled => DealerRuntimeActionV1::FinalizeSettlement,
        FeeTerminalOutcomeV1::Aborted => DealerRuntimeActionV1::AbortBeforeCollection,
    }
}

const fn action_byte(action: DealerRuntimeActionV1) -> u8 {
    match action {
        DealerRuntimeActionV1::FinalizeSettlement => 13,
        DealerRuntimeActionV1::AbortBeforeCollection => 14,
        _ => 0,
    }
}

fn decode_action(value: u8) -> Result<DealerRuntimeActionV1> {
    match value {
        13 => Ok(DealerRuntimeActionV1::FinalizeSettlement),
        14 => Ok(DealerRuntimeActionV1::AbortBeforeCollection),
        _ => Err(Error::InvalidParameter),
    }
}

const fn outcome_byte(outcome: FeeTerminalOutcomeV1) -> u8 {
    match outcome { FeeTerminalOutcomeV1::Settled => 1, FeeTerminalOutcomeV1::Aborted => 2 }
}

fn decode_outcome(value: u8) -> Result<FeeTerminalOutcomeV1> {
    match value { 1 => Ok(FeeTerminalOutcomeV1::Settled), 2 => Ok(FeeTerminalOutcomeV1::Aborted), _ => Err(Error::InvalidParameter) }
}

const _: () = assert!(DEALER_COVERED_TERMINAL_BYTES_V2 == 5_436);
const _: () = assert!(TERMINAL_RESERVED_BYTES_V2 == 4_224);

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn id(byte: u8) -> Id { Id::from_bytes([byte; 32]) }

    fn terminal() -> CoveredDealerTerminalV2 {
        let mut identities = [Id::ZERO; TERMINAL_IDENTITY_COUNT_V2];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index] = id(u8::try_from(index + 1).unwrap());
            index += 1;
        }
        CoveredDealerTerminalV2 {
            selection_account_id: identities[0], selection_pre_semantic_id: identities[1],
            policy_id: identities[2], facility_id: identities[3],
            facility_position_binding_id: identities[4], dealer_state_account_id: identities[5],
            state_pre_content_id: identities[6], state_post_content_id: identities[7],
            market_instance_v2_id: identities[8], epoch_id: identities[9],
            epoch_binding_account_id: identities[10], settlement_root_account_id: identities[11],
            retained_feed_account_id: identities[12], order_set_id: identities[13],
            settlement_candidate_id: identities[14], selected_fee_record_account_id: identities[15],
            selected_fee_record_semantic_id: identities[16], selected_fee_binding_digest: identities[17],
            fee_revenue_policy_id: identities[18], lease_account_id: identities[19],
            lease_pre_semantic_id: identities[20], settlement_pot_account_id: identities[21],
            pot_pre_content_id: identities[22], lease_pot_close_transition_id: identities[23],
            facility_position_account_id: identities[24], position_pre_semantic_id: identities[25],
            position_post_semantic_id: identities[26], facility_replay_account_id: identities[27],
            replay_pre_semantic_id: identities[28], replay_post_semantic_id: identities[29],
            liveness_receipt_account_id: identities[30], liveness_receipt_semantic_id: identities[31],
            fee_terminal_receipt_id: identities[32], dealer_generation_before: 7,
            dealer_generation_after: 8, general_epoch_generation: 9, selected_ordinal: 10,
            replay_ordinal_before: 11, replay_ordinal_after: 12, terminal_slot: 13,
            action: DealerRuntimeActionV1::FinalizeSettlement,
            outcome: FeeTerminalOutcomeV1::Settled, stored_bump: 17,
            rent: DeletableRentOwnerV1 {
                payer: id(100), neutral_sink: id(101), refundable_principal: 102,
                donation_floor: 103,
            },
        }
    }

    #[test]
    fn terminal_codec_rejects_noncanonical_tail_padding() {
        let value = terminal();
        let mut bytes = [0u8; DEALER_COVERED_TERMINAL_BYTES_V2];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(CoveredDealerTerminalV2::decode(&bytes).unwrap(), value);
        bytes[HEADER_BYTES + (TERMINAL_IDENTITY_COUNT_V2 * 32)
            + (TERMINAL_U64_COUNT_V2 * 8) + TERMINAL_SCALAR_BYTES_V2] = 1;
        assert_eq!(
            CoveredDealerTerminalV2::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn stale_replay_and_outcome_action_mismatch_refuse() {
        let mut stale = terminal();
        stale.replay_ordinal_after = stale.replay_ordinal_before;
        assert_eq!(stale.validate(), Err(Error::InvalidParameter));
        let mut wrong_outcome = terminal();
        wrong_outcome.outcome = FeeTerminalOutcomeV1::Aborted;
        assert_eq!(wrong_outcome.validate(), Err(Error::InvalidParameter));
    }

    #[test]
    fn physical_identity_swap_cannot_collapse_terminal_evidence() {
        let mut aliased = terminal();
        aliased.fee_terminal_receipt_id = aliased.selected_fee_record_account_id;
        assert_eq!(aliased.validate(), Err(Error::MismatchedBinding));
        let mut root_as_lease = terminal();
        root_as_lease.settlement_root_account_id = root_as_lease.lease_account_id;
        assert_eq!(root_as_lease.validate(), Err(Error::MismatchedBinding));
    }
}
