// SPDX-License-Identifier: AGPL-3.0-or-later

//! State-owned authorization for sealing the canonical Dealer Replay V3.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerFacilityReplayV1, DealerPhaseV2, DealerPolicyV1, DealerPositionObservationV3,
    DealerReplayAccountBindingV1, DealerStateV2, DealerTransitionIntentV1, Error,
    FacilityPositionBindingV2, FixedCodec, Id, PreparedDealerReplayTransitionV1, Result,
    DEALER_TERMINAL_STATE_RECEIPT_CONTENT_DOMAIN_V2,
};
use clutch_retirement::PositionLifecycleV3;
use clutch_retirement::PositionTombstoneV3;

/// Exact local receipt magic.
pub const DEALER_TERMINAL_STATE_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCTRCPV2";
/// Exact local receipt version.
pub const DEALER_TERMINAL_STATE_RECEIPT_VERSION_V2: u16 = 2;
/// Exact bytes in the terminal State receipt.
pub const DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2: usize = HEADER_BYTES + (7 * 32) + (2 * 8);

/// Immutable receipt proving State reached the unique Replay-terminalizable cut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTerminalStateReceiptV2 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable canonical Position V3 purpose binding.
    pub facility_position_binding_id: Id,
    /// Authoritative State account.
    pub dealer_state_account_id: Id,
    /// Content identity of the exact Retiring State body.
    pub terminal_state_content_id: Id,
    /// Semantic identity of the exact empty CloseRequested Position V3 body.
    pub terminal_position_semantic_id: Id,
    /// Exact live Replay V3 account that may be sealed by this receipt.
    pub replay_account_id: Id,
    /// Exact terminal Position/Dealer generation.
    pub terminal_generation: u64,
    /// Exact State child sequence at the terminal cut.
    pub terminal_child_sequence: u64,
}

impl DealerTerminalStateReceiptV2 {
    /// Validate nonzero identities and nonzero generation.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.terminal_state_content_id,
            self.terminal_position_semantic_id,
            self.replay_account_id,
        ] {
            identity.validate_live()?;
        }
        if self.terminal_generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Exact immutable receipt identity stored by the Dealer Replay extension.
    pub fn receipt_id(&self) -> Result<Id> {
        self.content_id(DEALER_TERMINAL_STATE_RECEIPT_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for DealerTerminalStateReceiptV2 {
    const ENCODED_LEN: usize = DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_TERMINAL_STATE_RECEIPT_MAGIC_V2,
            DEALER_TERMINAL_STATE_RECEIPT_VERSION_V2,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.terminal_state_content_id,
            self.terminal_position_semantic_id,
            self.replay_account_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.terminal_generation);
        writer.u64(self.terminal_child_sequence);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_TERMINAL_STATE_RECEIPT_MAGIC_V2,
            DEALER_TERMINAL_STATE_RECEIPT_VERSION_V2,
        )?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            facility_position_binding_id: reader.id(),
            dealer_state_account_id: reader.id(),
            terminal_state_content_id: reader.id(),
            terminal_position_semantic_id: reader.id(),
            replay_account_id: reader.id(),
            terminal_generation: reader.u64(),
            terminal_child_sequence: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Opaque result of State-authorized Replay terminalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPreparedTerminalReplayV2 {
    /// Exact receipt committed by the Replay extension.
    pub receipt: DealerTerminalStateReceiptV2,
    /// Private-field canonical Replay transition prepared for atomic commit.
    pub prepared_replay: PreparedDealerReplayTransitionV1,
}

/// Mint the terminal State receipt and seal Replay only at the exact child cut.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_terminal_replay_v2(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    dealer_state_account_id: Id,
    terminal_position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_account_binding: DealerReplayAccountBindingV1,
    intent: DealerTransitionIntentV1,
) -> Result<DealerPreparedTerminalReplayV2> {
    state.validate_against_policy(policy)?;
    let binding_id = binding.binding_id()?;
    terminal_position.validate_against(binding, binding_id, policy)?;
    replay.validate()?;
    let position = terminal_position.projection.position();
    if state.phase != DealerPhaseV2::Retiring
        || state.children.facility_positions != 1
        || state.children.facility_replays != 1
        || state.children.lp_pages != 0
        || state.children.live_lp_positions != 0
        || state.children.unclaimed_lp_positions != 0
        || state.children.funded_dependencies != 0
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.terminal_allocations != 0
        || state.children.claim_work != 0
        || !state.funded_dependencies_account_id.is_zero()
        || dealer_state_account_id != binding.dealer_state_account_id
        || state.facility_position_binding_id != binding_id
        || terminal_position.account_id != state.facility_position_account_id
        || terminal_position.semantic_id != state.facility_position_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.facility_position_binding_id() != binding_id
        || replay.position_generation() != state.generation
        || replay.lifecycle() != clutch_retirement::ReplayV3Lifecycle::Live
        || position.lifecycle() != PositionLifecycleV3::CloseRequested
        || position.generation() != state.generation
        || Id::from_bytes(position.replay_account().bytes()) != state.facility_replay_account_id
        || position.cash_atoms() != 0
        || position.reserved_cash_atoms() != 0
        || position.native_eggs() != [0; crate::MAX_OUTCOMES]
        || position.outstanding_reservations() != 0
    {
        return Err(Error::InvalidChildGraph);
    }
    let state_content_id = state.state_content_id()?;
    let receipt = DealerTerminalStateReceiptV2 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        facility_position_binding_id: binding_id,
        dealer_state_account_id,
        terminal_state_content_id: state_content_id,
        terminal_position_semantic_id: terminal_position.semantic_id,
        replay_account_id: state.facility_replay_account_id,
        terminal_generation: state.generation,
        terminal_child_sequence: state.child_sequence,
    };
    let receipt_id = receipt.receipt_id()?;
    if intent.state_pre_content_id != state_content_id
        || intent.state_post_content_id != state_content_id
        || intent.position_pre_semantic_id != terminal_position.semantic_id
        || intent.position_post_semantic_id != terminal_position.semantic_id
        || intent.position_generation_before != state.generation
        || intent.position_generation_after != state.generation
        || intent.action != crate::DealerRuntimeActionV1::Retire
    {
        return Err(Error::MismatchedBinding);
    }
    let prepared_replay = replay.prepare_terminal_transition(
        replay_account_binding,
        intent,
        receipt_id,
    )?;
    Ok(DealerPreparedTerminalReplayV2 {
        receipt,
        prepared_replay,
    })
}

/// Consume authenticated Position/Replay closes and decrement their two classes.
///
/// The runtime adapter must first commit the Position V3 tombstone and execute
/// the exact Replay close plan atomically. This transition owns only the
/// Dealer State counters and refuses any other live child.
pub fn close_dealer_position_replay_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    receipt: &DealerTerminalStateReceiptV2,
    position_tombstone: PositionTombstoneV3,
    replay_close: crate::DealerReplayClosePlanV1,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    receipt.validate()?;
    position_tombstone
        .validate()
        .map_err(|_| Error::MismatchedBinding)?;
    let position = position_tombstone.fields();
    if state.phase != DealerPhaseV2::Retiring
        || state.children.facility_positions != 1
        || state.children.facility_replays != 1
        || state.children.lp_pages != 0
        || state.children.live_lp_positions != 0
        || state.children.unclaimed_lp_positions != 0
        || state.children.funded_dependencies != 0
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.terminal_allocations != 0
        || state.children.claim_work != 0
        || receipt.policy_id != state.policy_id
        || receipt.facility_id != state.facility_id
        || receipt.facility_position_binding_id != state.facility_position_binding_id
        || receipt.terminal_state_content_id != state.state_content_id()?
        || receipt.terminal_position_semantic_id != state.facility_position_id
        || receipt.replay_account_id != state.facility_replay_account_id
        || receipt.terminal_generation != state.generation
        || receipt.terminal_child_sequence != state.child_sequence
        || replay_close.replay_account_id() != state.facility_replay_account_id
        || replay_close.terminal_state_receipt_id() != receipt.receipt_id()?
        || Id::from_bytes(position.owner.bytes()) != state.facility_id
        || Id::from_bytes(position.controller.bytes()) != receipt.dealer_state_account_id
        || Id::from_bytes(position.replay_account.bytes()) != state.facility_replay_account_id
        || Id::from_bytes(position.purpose_binding_id.bytes())
            != state.facility_position_binding_id
        || position.generation != state.generation
    {
        return Err(Error::InvalidChildGraph);
    }
    let mut next = *state;
    next.children.facility_positions = 0;
    next.children.facility_replays = 0;
    next.child_sequence = next
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    next.validate_against_policy(policy)?;
    Ok(next)
}

const _: () = assert!(DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2 == 252);
const _: () = assert!(DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
