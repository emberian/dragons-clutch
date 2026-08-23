// SPDX-License-Identifier: AGPL-3.0-or-later

//! State-owned authorization for sealing the canonical Dealer Replay V3.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerActionLivenessAuthorizationV1, DealerEmptyAssetTransferBundleV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2, DealerLivenessScheduleV1, DealerPhaseV2, DealerPolicyV1,
    DealerPositionObservationV3, DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DealerTransitionIntentV1,
    DealerTransitionLivenessModeV1, Error, FacilityPositionBindingV2, FixedCodec, Id,
    PreparedDealerReplayTransitionV1, Result, DEALER_TERMINAL_STATE_RECEIPT_CONTENT_DOMAIN_V2,
};
use clutch_retirement::{PositionLifecycleV3, PositionTombstoneV3, PositionV3Sha256Backend};
use sha2::{Digest, Sha256};

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
    receipt: DealerTerminalStateReceiptV2,
    prepared_replay: PreparedDealerReplayTransitionV1,
}

impl DealerPreparedTerminalReplayV2 {
    /// Exact receipt committed by the Replay extension.
    pub const fn receipt(self) -> DealerTerminalStateReceiptV2 {
        self.receipt
    }

    /// Private-field canonical Replay transition prepared for atomic commit.
    pub const fn prepared_replay(self) -> PreparedDealerReplayTransitionV1 {
        self.prepared_replay
    }
}

/// Mint the terminal State receipt and seal Replay only at the exact child cut.
///
/// The funded-dependency child deliberately remains live here. The atomic
/// terminal Replay transition consumes the exact Retirement-compartment work
/// receipt before Position and Replay close; only then may the external
/// seven-account runtime terminalize and the dependency child release rent.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_terminal_replay_v2(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    dealer_state_account_id: Id,
    terminal_position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_account_binding: DealerReplayAccountBindingV1,
    dependency_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    intent: DealerTransitionIntentV1,
) -> Result<DealerPreparedTerminalReplayV2> {
    state.validate_against_policy(policy)?;
    let binding_id = binding.binding_id()?;
    terminal_position.validate_against(binding, binding_id, policy)?;
    replay.validate()?;
    dependency.validate()?;
    dependency_account_id.validate_live()?;
    schedule.validate_for_facility_runtime()?;
    runtime.validate()?;
    authorization.validate_against(schedule, runtime)?;
    let position = terminal_position.projection.position();
    if state.phase != DealerPhaseV2::Retiring
        || state.children.facility_positions != 1
        || state.children.facility_replays != 1
        || state.children.lp_pages != 0
        || state.children.live_lp_positions != 0
        || state.children.unclaimed_lp_positions != 0
        || state.children.exit_tickets != 0
        || state.children.funded_dependencies != 1
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.terminal_allocations != 0
        || state.children.claim_work != 0
        || state.funded_dependencies_account_id != dependency_account_id
        || dealer_state_account_id != binding.dealer_state_account_id
        || state.facility_position_binding_id != binding_id
        || state.funded_dependencies_id != dependency.dependency_id()?
        || dependency.facility_position_binding_id != binding_id
        || dependency.bindings.policy_id != state.policy_id
        || dependency.bindings.facility_id != state.facility_id
        || dependency.bindings.asset_vault_authority_account_id != dealer_state_account_id
        || dependency.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
        || dependency.bindings.liveness_schedule_id != policy.liveness_policy_id
        || dependency.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
        || dependency.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
        || dependency.bindings.fee_policy_id != policy.fee_policy_id
        || dependency.bindings.collateral_mint != policy.collateral_mint
        || dependency.bindings.token_program != policy.token_program
        || dependency.bindings.neutral_sink != policy.neutral_sink
        || dependency.bindings.dealer_liveness_work_principal_lamports
            != schedule.dealer_runtime_work_principal_lamports()?
        || dependency.rent.neutral_sink != policy.neutral_sink
        || runtime.lifecycle_id != state.facility_id
        || runtime.realm_id != policy.realm_id
        || runtime.neutral_sink != policy.neutral_sink
        || dependency_account_id == dealer_state_account_id
        || dependency_account_id == state.facility_position_account_id
        || dependency_account_id == state.facility_replay_account_id
        || authorization.action != DealerRuntimeActionV1::Retire
        || authorization.owner != dealer_state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
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
        || intent.action != DealerRuntimeActionV1::Retire
        || intent.liveness_mode != DealerTransitionLivenessModeV1::ExternalReceipt
        || intent.liveness_receipt_semantic_id != authorization.receipt_semantic_id
        || !intent.fee_evidence_id.is_zero()
        || intent.asset_transfer_bundle_id
            != (DealerEmptyAssetTransferBundleV1 {
                action: DealerRuntimeActionV1::Retire,
            })
            .bundle_id()?
    {
        return Err(Error::MismatchedBinding);
    }
    let prepared_replay =
        replay.prepare_terminal_transition(replay_account_binding, intent, receipt_id)?;
    Ok(DealerPreparedTerminalReplayV2 {
        receipt,
        prepared_replay,
    })
}

/// Consume authenticated Position/Replay closes and decrement their two classes.
///
/// The runtime adapter must first commit the Position V3 tombstone and execute
/// the exact Replay close plan atomically. This transition owns only the
/// Dealer State counters and refuses every live economic child. The funded
/// dependency is the sole survivor because it owns the still-required
/// external-runtime terminalization and rent-release join.
pub fn close_dealer_position_replay_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    receipt: &DealerTerminalStateReceiptV2,
    terminal_position: &DealerPositionObservationV3,
    position_tombstone: PositionTombstoneV3,
    replay_close: crate::DealerReplayClosePlanV1,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    receipt.validate()?;
    position_tombstone
        .validate()
        .map_err(|_| Error::MismatchedBinding)?;
    let expected_tombstone = terminal_position
        .projection
        .position()
        .terminal_projection()
        .and_then(|projection| projection.tombstone())
        .map_err(|_| Error::MismatchedBinding)?;
    if position_tombstone != expected_tombstone {
        return Err(Error::MismatchedBinding);
    }
    let position = position_tombstone.fields();
    if state.phase != DealerPhaseV2::Retiring
        || state.children.facility_positions != 1
        || state.children.facility_replays != 1
        || state.children.lp_pages != 0
        || state.children.live_lp_positions != 0
        || state.children.unclaimed_lp_positions != 0
        || state.children.exit_tickets != 0
        || state.children.funded_dependencies != 1
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.terminal_allocations != 0
        || state.children.claim_work != 0
        || state.funded_dependencies_account_id.is_zero()
        || receipt.policy_id != state.policy_id
        || receipt.facility_id != state.facility_id
        || receipt.facility_position_binding_id != state.facility_position_binding_id
        || receipt.terminal_state_content_id != state.state_content_id()?
        || receipt.terminal_position_semantic_id != state.facility_position_id
        || receipt.replay_account_id != state.facility_replay_account_id
        || receipt.terminal_generation != state.generation
        || receipt.terminal_child_sequence != state.child_sequence
        || terminal_position.account_id != state.facility_position_account_id
        || terminal_position.semantic_id != state.facility_position_id
        || replay_close.replay_account_id() != state.facility_replay_account_id
        || replay_close.terminal_state_receipt_id() != receipt.receipt_id()?
        || Id::from_bytes(position.owner.bytes()) != state.facility_id
        || Id::from_bytes(position.controller.bytes()) != receipt.dealer_state_account_id
        || Id::from_bytes(position.replay_account.bytes()) != state.facility_replay_account_id
        || Id::from_bytes(position.purpose_binding_id.bytes()) != state.facility_position_binding_id
        || position.generation != state.generation
    {
        return Err(Error::InvalidChildGraph);
    }
    let mut next = *state;
    next.children.facility_positions = 0;
    next.children.facility_replays = 0;
    next.terminal_position_tombstone_id = Id::from_bytes(
        position_tombstone
            .semantic_id(&DealerTerminalStateSha256V2)
            .map_err(|_| Error::MismatchedBinding)?
            .bytes(),
    );
    next.terminal_replay_semantic_id = replay_close.terminal_replay_semantic_id();
    next.terminal_replay_intent_id = replay_close.last_transition_intent_id();
    next.terminal_state_receipt_id = replay_close.terminal_state_receipt_id();
    next.child_sequence = next
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    next.validate_against_policy(policy)?;
    Ok(next)
}

const _: () = assert!(DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2 == 252);
const _: () = assert!(DEALER_TERMINAL_STATE_RECEIPT_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[derive(Clone, Copy, Debug)]
struct DealerTerminalStateSha256V2;

impl PositionV3Sha256Backend for DealerTerminalStateSha256V2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}
