// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dealer-specific transition replay without a parallel asset truth.
//!
//! The Replay is the Dealer extension of the canonical purpose-owned Replay V3
//! envelope. The common prefix owns the Position/Replay keys, purpose binding,
//! generation, ordinal, lifecycle, extension hash, and rent. The exact Dealer
//! extension owns only the last transition intent and terminal State receipt.
//! It never duplicates State phase, Position balances, an active Lease, or Pot
//! custody. Runtime account ownership, PDA derivation, signature checks, and
//! receipt parsing remain adapter duties.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{DealerRuntimeActionV1, Error, FixedCodec, Id, Result};
use clutch_retirement::{
    DeletableRentOwnerV1 as ReplayRentOwnerV1, Identity32V1, PositionPurposeV3, ReplayV3Envelope,
    ReplayV3EnvelopeFields, ReplayV3EnvelopeHeader, ReplayV3ExtensionSchema, ReplayV3HashBackend,
    ReplayV3Lifecycle, ReplayV3PdaSeeds, RetirementErrorV2, PURPOSE_REPLAY_V3_PREFIX_BYTES,
};
use sha2::{Digest, Sha256};

/// Exact founding ordinal expected by the first transition intent.
pub const DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1: u64 = 0;
/// Exact Dealer extension schema (`"DDF1"` in little-endian wire order).
pub const DEALER_REPLAY_EXTENSION_SCHEMA_V1: u32 = 0x3146_4444;
/// Exact bytes in the Dealer-owned extension.
pub const DEALER_REPLAY_EXTENSION_BYTES_V1: usize = 64;
/// Exact bytes in the canonical Replay V3 Dealer variant.
pub const DEALER_FACILITY_REPLAY_BYTES_V1: usize =
    PURPOSE_REPLAY_V3_PREFIX_BYTES + DEALER_REPLAY_EXTENSION_BYTES_V1;

/// Local magic for a transition intent committed by the Replay.
pub const DEALER_TRANSITION_INTENT_MAGIC_V1: [u8; 8] = *b"DCDTRNI1";
/// Exact local transition-intent version.
pub const DEALER_TRANSITION_INTENT_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical transition intent.
pub const DEALER_TRANSITION_INTENT_BYTES_V1: usize = HEADER_BYTES + (9 * 32) + (3 * 8) + 8;
/// Content domain for one canonical transition intent.
pub const DEALER_TRANSITION_INTENT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-transition-intent/v1\0";

const _: () = assert!(DEALER_FACILITY_REPLAY_BYTES_V1 == 272);
const _: () = assert!(DEALER_TRANSITION_INTENT_BYTES_V1 == 332);
const _: () = assert!(DEALER_FACILITY_REPLAY_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_TRANSITION_INTENT_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

/// Exact Dealer-owned extension committed by the common Replay V3 prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayExtensionV1 {
    /// Last accepted transition-intent identity, or zero only at founding.
    pub last_transition_intent_id: Id,
    /// Exact State terminal receipt; nonzero only in a Terminal envelope.
    pub terminal_state_receipt_id: Id,
}

impl DealerReplayExtensionV1 {
    fn validate(self, lifecycle: ReplayV3Lifecycle, next_sequence: u64) -> Result<()> {
        match lifecycle {
            ReplayV3Lifecycle::Live if next_sequence == 0 => {
                if !self.last_transition_intent_id.is_zero()
                    || !self.terminal_state_receipt_id.is_zero()
                {
                    return Err(Error::InvalidParameter);
                }
            }
            ReplayV3Lifecycle::Live => {
                self.last_transition_intent_id.validate_live()?;
                if !self.terminal_state_receipt_id.is_zero() {
                    return Err(Error::InvalidParameter);
                }
            }
            ReplayV3Lifecycle::Terminal => {
                self.last_transition_intent_id.validate_live()?;
                self.terminal_state_receipt_id.validate_live()?;
            }
        }
        Ok(())
    }

    fn encode(self) -> [u8; DEALER_REPLAY_EXTENSION_BYTES_V1] {
        let mut output = [0u8; DEALER_REPLAY_EXTENSION_BYTES_V1];
        output[..32].copy_from_slice(&self.last_transition_intent_id.bytes());
        output[32..].copy_from_slice(&self.terminal_state_receipt_id.bytes());
        output
    }

    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < DEALER_REPLAY_EXTENSION_BYTES_V1 {
            return Err(Error::Truncated);
        }
        if input.len() > DEALER_REPLAY_EXTENSION_BYTES_V1 {
            return Err(Error::TrailingBytes);
        }
        let mut last = [0u8; 32];
        last.copy_from_slice(&input[..32]);
        let mut terminal = [0u8; 32];
        terminal.copy_from_slice(&input[32..]);
        Ok(Self {
            last_transition_intent_id: Id::from_bytes(last),
            terminal_state_receipt_id: Id::from_bytes(terminal),
        })
    }
}

/// Canonical purpose-owned Replay V3 Dealer variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityReplayV1 {
    header: ReplayV3EnvelopeHeader,
    extension: DealerReplayExtensionV1,
}

impl DealerFacilityReplayV1 {
    /// Validate the common envelope and exact Dealer extension.
    pub fn validate(&self) -> Result<()> {
        let extension = self.extension.encode();
        ReplayV3Envelope::from_header(self.header, &extension, &DealerReplaySha256V1)
            .map_err(map_retirement_error)?;
        if self.header.purpose() != PositionPurposeV3::DealerFacility
            || self.header.extension_schema().get() != DEALER_REPLAY_EXTENSION_SCHEMA_V1
            || self.header.extension_len() as usize != DEALER_REPLAY_EXTENSION_BYTES_V1
        {
            return Err(Error::MismatchedBinding);
        }
        self.extension
            .validate(self.header.lifecycle(), self.header.next_sequence())
    }

    /// Construct the unique founding state for an authenticated account graph.
    #[allow(clippy::too_many_arguments)]
    pub fn founding(
        facility_position_account_id: Id,
        replay_account_id: Id,
        facility_position_binding_id: Id,
        initial_position_generation: u64,
        stored_bump: u8,
        rent: ReplayRentOwnerV1,
    ) -> Result<Self> {
        let extension = DealerReplayExtensionV1 {
            last_transition_intent_id: Id::ZERO,
            terminal_state_receipt_id: Id::ZERO,
        };
        let extension_bytes = extension.encode();
        let header = ReplayV3EnvelopeHeader::new_live(
            ReplayV3EnvelopeFields {
                position_account: retirement_id(facility_position_account_id)?,
                replay_account: retirement_id(replay_account_id)?,
                purpose: PositionPurposeV3::DealerFacility,
                purpose_binding_id: retirement_id(facility_position_binding_id)?,
                position_generation: initial_position_generation,
                next_sequence: DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1,
                stored_bump,
                rent,
            },
            dealer_extension_schema()?,
            &extension_bytes,
            &DealerReplaySha256V1,
        )
        .map_err(map_retirement_error)?;
        let replay = Self { header, extension };
        replay.validate()?;
        Ok(replay)
    }

    /// Canonical semantic identity of this exact Replay body.
    pub fn replay_id(&self) -> Result<Id> {
        self.validate()?;
        let extension = self.extension.encode();
        let envelope =
            ReplayV3Envelope::from_header(self.header, &extension, &DealerReplaySha256V1)
                .map_err(map_retirement_error)?;
        Ok(dealer_id(
            envelope
                .semantic_id(&DealerReplaySha256V1)
                .map_err(map_retirement_error)?,
        ))
    }

    /// Exact common PDA seed facts; the adapter derives and authenticates the address.
    pub const fn pda_seeds(&self) -> ReplayV3PdaSeeds {
        self.header.pda_seeds()
    }

    /// Exact canonical Position V3 account.
    pub const fn facility_position_account_id(&self) -> Id {
        dealer_id(self.header.position_account())
    }

    /// Exact Replay account key retained in the common body.
    pub const fn replay_account_id(&self) -> Id {
        dealer_id(self.header.replay_account())
    }

    /// Exact Dealer facility Position binding identity.
    pub const fn facility_position_binding_id(&self) -> Id {
        dealer_id(self.header.purpose_binding_id())
    }

    /// Current Position generation.
    pub const fn position_generation(&self) -> u64 {
        self.header.position_generation()
    }

    /// Ordinal the next accepted transition must carry.
    pub const fn next_transition_ordinal(&self) -> u64 {
        self.header.next_sequence()
    }

    /// Last accepted transition intent.
    pub const fn last_transition_intent_id(&self) -> Id {
        self.extension.last_transition_intent_id
    }

    /// Exact terminal State receipt, zero while live.
    pub const fn terminal_state_receipt_id(&self) -> Id {
        self.extension.terminal_state_receipt_id
    }

    /// Common Replay lifecycle.
    pub const fn lifecycle(&self) -> ReplayV3Lifecycle {
        self.header.lifecycle()
    }

    /// Common Replay rent owner.
    pub const fn rent(&self) -> ReplayRentOwnerV1 {
        self.header.rent()
    }

    /// Prepare an atomic transition without mutating the replay projection.
    pub fn prepare_transition(
        &self,
        account_binding: DealerReplayAccountBindingV1,
        intent: DealerTransitionIntentV1,
    ) -> Result<PreparedDealerReplayTransitionV1> {
        self.validate()?;
        account_binding.validate()?;
        intent.validate()?;
        let replay_pre_id = self.replay_id()?;
        if intent.action == DealerRuntimeActionV1::Retire
            || account_binding.position_replay_account_id != account_binding.replay_account_id
            || account_binding.replay_account_id != self.replay_account_id()
            || intent.replay_account_id != account_binding.replay_account_id
            || intent.replay_pre_id != replay_pre_id
            || intent.expected_ordinal != self.next_transition_ordinal()
            || intent.position_generation_before != self.position_generation()
        {
            return Err(Error::MismatchedBinding);
        }
        let intent_id = intent.intent_id()?;
        let extension = DealerReplayExtensionV1 {
            last_transition_intent_id: intent_id,
            terminal_state_receipt_id: Id::ZERO,
        };
        let extension_bytes = extension.encode();
        let header = self
            .header
            .advanced_live(
                intent.position_generation_after,
                &extension_bytes,
                &DealerReplaySha256V1,
            )
            .map_err(map_retirement_error)?;
        let replay_post = Self { header, extension };
        replay_post.validate()?;
        Ok(PreparedDealerReplayTransitionV1 {
            replay_account_id: account_binding.replay_account_id,
            replay_pre_id,
            replay_post,
            intent,
            intent_id,
        })
    }

    /// Purpose-owner-only terminal advance after complete State V2 validation.
    ///
    /// This is crate-visible so the authoritative Dealer State transition can
    /// expose a private-field terminal capability; a runtime adapter cannot set
    /// the common terminal bit from unauthenticated booleans.
    pub(crate) fn prepare_terminal_transition(
        &self,
        account_binding: DealerReplayAccountBindingV1,
        intent: DealerTransitionIntentV1,
        terminal_state_receipt_id: Id,
    ) -> Result<PreparedDealerReplayTransitionV1> {
        self.validate()?;
        account_binding.validate()?;
        intent.validate()?;
        terminal_state_receipt_id.validate_live()?;
        let replay_pre_id = self.replay_id()?;
        if intent.action != DealerRuntimeActionV1::Retire
            || account_binding.replay_account_id != self.replay_account_id()
            || intent.replay_account_id != account_binding.replay_account_id
            || intent.replay_pre_id != replay_pre_id
            || intent.expected_ordinal != self.next_transition_ordinal()
            || intent.position_generation_before != self.position_generation()
        {
            return Err(Error::MismatchedBinding);
        }
        let intent_id = intent.intent_id()?;
        let extension = DealerReplayExtensionV1 {
            last_transition_intent_id: intent_id,
            terminal_state_receipt_id,
        };
        let extension_bytes = extension.encode();
        let header = self
            .header
            .terminalized(
                intent.position_generation_after,
                &extension_bytes,
                &DealerReplaySha256V1,
            )
            .map_err(map_retirement_error)?;
        let replay_post = Self { header, extension };
        replay_post.validate()?;
        Ok(PreparedDealerReplayTransitionV1 {
            replay_account_id: account_binding.replay_account_id,
            replay_pre_id,
            replay_post,
            intent,
            intent_id,
        })
    }
}

impl FixedCodec for DealerFacilityReplayV1 {
    const ENCODED_LEN: usize = DEALER_FACILITY_REPLAY_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let extension = self.extension.encode();
        ReplayV3Envelope::from_header(self.header, &extension, &DealerReplaySha256V1)
            .and_then(|envelope| envelope.encode_into(output, &DealerReplaySha256V1))
            .map_err(map_retirement_error)
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let envelope =
            ReplayV3Envelope::decode(input, &DealerReplaySha256V1).map_err(map_retirement_error)?;
        let value = Self {
            header: envelope.header(),
            extension: DealerReplayExtensionV1::decode(envelope.extension())?,
        };
        value.validate()?;
        Ok(value)
    }
}

const fn dealer_id(identity: Identity32V1) -> Id {
    Id::from_bytes(identity.bytes())
}

fn retirement_id(identity: Id) -> Result<Identity32V1> {
    Identity32V1::new(identity.bytes()).map_err(|_| Error::ZeroIdentity)
}

fn dealer_extension_schema() -> Result<ReplayV3ExtensionSchema> {
    ReplayV3ExtensionSchema::new(DEALER_REPLAY_EXTENSION_SCHEMA_V1).map_err(map_retirement_error)
}

fn map_retirement_error(error: RetirementErrorV2) -> Error {
    match error {
        RetirementErrorV2::Truncated => Error::Truncated,
        RetirementErrorV2::TrailingBytes => Error::TrailingBytes,
        RetirementErrorV2::WrongTag => Error::BadMagic,
        RetirementErrorV2::WrongVersion => Error::BadVersion,
        RetirementErrorV2::ZeroIdentity => Error::ZeroIdentity,
        RetirementErrorV2::ArithmeticOverflow => Error::ArithmeticOverflow,
        RetirementErrorV2::WrongPhase | RetirementErrorV2::AlreadyTerminal => Error::InvalidPhase,
        RetirementErrorV2::WrongGeneration
        | RetirementErrorV2::WrongParent
        | RetirementErrorV2::ReplayMismatch => Error::MismatchedBinding,
        _ => Error::InvalidParameter,
    }
}

#[derive(Clone, Copy, Debug)]
struct DealerReplaySha256V1;

impl ReplayV3HashBackend for DealerReplaySha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        let mut index = 0usize;
        while index < parts.len() {
            hasher.update(parts[index]);
            index += 1;
        }
        hasher.finalize().into()
    }
}

/// Runtime-authenticated actual Replay account joined to Position V3.
///
/// Fields remain public because this pure carrier is not an authentication
/// capability. The adapter must derive the Replay PDA and parse Position V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayAccountBindingV1 {
    /// Actual Replay account presented to the instruction.
    pub replay_account_id: Id,
    /// Exact Replay account retained by the authenticated Position V3 body.
    pub position_replay_account_id: Id,
}

impl DealerReplayAccountBindingV1 {
    fn validate(self) -> Result<()> {
        self.replay_account_id.validate_live()?;
        self.position_replay_account_id.validate_live()?;
        if self.replay_account_id != self.position_replay_account_id {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

/// How one transition obtains execution liveness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerTransitionLivenessModeV1 {
    /// The transaction caller pays; no external receipt may be supplied.
    CallerFunded = 0,
    /// The exact canonical external-runtime receipt is consumed.
    ExternalReceipt = 1,
}

impl DealerTransitionLivenessModeV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::CallerFunded),
            1 => Ok(Self::ExternalReceipt),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Canonical transition intent whose identity becomes Replay's last intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTransitionIntentV1 {
    /// Actual, PDA-authenticated Replay account.
    pub replay_account_id: Id,
    /// Semantic identity of the exact Replay pre-state.
    pub replay_pre_id: Id,
    /// Exact Dealer State content identity before the transition.
    pub state_pre_content_id: Id,
    /// Exact Dealer State content identity after the transition.
    pub state_post_content_id: Id,
    /// Exact canonical Position V3 semantic identity before the transition.
    pub position_pre_semantic_id: Id,
    /// Exact canonical Position V3 semantic identity after the transition.
    pub position_post_semantic_id: Id,
    /// Exact external liveness receipt, or zero only for caller-funded actions.
    pub liveness_receipt_semantic_id: Id,
    /// Exact canonical fee settlement/abort receipt, or zero when inapplicable.
    pub fee_receipt_semantic_id: Id,
    /// Content identity of the complete exact asset-transfer bundle.
    pub asset_transfer_bundle_id: Id,
    /// Position generation authenticated before the transition.
    pub position_generation_before: u64,
    /// Position generation expected after the transition.
    ///
    /// It is equal to the pre-generation for ordinary steps and exactly one
    /// greater for a generation-consuming Finalize, Abort, or terminal step.
    pub position_generation_after: u64,
    /// Ordinal consumed from Replay.
    pub expected_ordinal: u64,
    /// Exact Dealer action being committed.
    pub action: DealerRuntimeActionV1,
    /// Caller-funded or exact external-receipt liveness.
    pub liveness_mode: DealerTransitionLivenessModeV1,
}

impl DealerTransitionIntentV1 {
    /// Validate identity presence and action-specific receipt requirements.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.replay_account_id,
            self.replay_pre_id,
            self.state_pre_content_id,
            self.state_post_content_id,
            self.position_pre_semantic_id,
            self.position_post_semantic_id,
            self.asset_transfer_bundle_id,
        ] {
            identity.validate_live()?;
        }
        let external_receipt_present = !self.liveness_receipt_semantic_id.is_zero();
        match self.liveness_mode {
            DealerTransitionLivenessModeV1::CallerFunded => {
                if external_receipt_present {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerTransitionLivenessModeV1::ExternalReceipt => {
                self.liveness_receipt_semantic_id.validate_live()?;
            }
        }
        match liveness_policy(self.action)? {
            DealerActionLivenessPolicyV1::CallerOnly => {
                if self.liveness_mode != DealerTransitionLivenessModeV1::CallerFunded {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerActionLivenessPolicyV1::ExternalOnly => {
                if self.liveness_mode != DealerTransitionLivenessModeV1::ExternalReceipt {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerActionLivenessPolicyV1::Either => {}
        }
        if action_requires_fee_receipt(self.action) {
            self.fee_receipt_semantic_id.validate_live()?;
        } else if !self.fee_receipt_semantic_id.is_zero() {
            return Err(Error::MismatchedBinding);
        }
        if self.position_generation_before == 0
            || (self.position_generation_after != self.position_generation_before
                && self.position_generation_before.checked_add(1)
                    != Some(self.position_generation_after))
        {
            return Err(Error::MismatchedBinding);
        }
        let consumes_generation = matches!(
            self.action,
            DealerRuntimeActionV1::FinalizeSettlement
                | DealerRuntimeActionV1::AbortBeforeCollection
        );
        if consumes_generation
            != (self.position_generation_after != self.position_generation_before)
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical transition-intent identity.
    pub fn intent_id(&self) -> Result<Id> {
        let id = self.content_id(DEALER_TRANSITION_INTENT_CONTENT_DOMAIN_V1)?;
        id.validate_live()?;
        Ok(id)
    }
}

impl FixedCodec for DealerTransitionIntentV1 {
    const ENCODED_LEN: usize = DEALER_TRANSITION_INTENT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_TRANSITION_INTENT_MAGIC_V1,
            DEALER_TRANSITION_INTENT_VERSION_V1,
        );
        for identity in [
            self.replay_account_id,
            self.replay_pre_id,
            self.state_pre_content_id,
            self.state_post_content_id,
            self.position_pre_semantic_id,
            self.position_post_semantic_id,
            self.liveness_receipt_semantic_id,
            self.fee_receipt_semantic_id,
            self.asset_transfer_bundle_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.position_generation_before);
        writer.u64(self.position_generation_after);
        writer.u64(self.expected_ordinal);
        writer.u8(action_byte(self.action));
        writer.u8(liveness_mode_byte(self.liveness_mode));
        writer.reserved(6);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_TRANSITION_INTENT_MAGIC_V1,
            DEALER_TRANSITION_INTENT_VERSION_V1,
        )?;
        let replay_account_id = reader.id();
        let replay_pre_id = reader.id();
        let state_pre_content_id = reader.id();
        let state_post_content_id = reader.id();
        let position_pre_semantic_id = reader.id();
        let position_post_semantic_id = reader.id();
        let liveness_receipt_semantic_id = reader.id();
        let fee_receipt_semantic_id = reader.id();
        let asset_transfer_bundle_id = reader.id();
        let position_generation_before = reader.u64();
        let position_generation_after = reader.u64();
        let expected_ordinal = reader.u64();
        let action = decode_action(reader.u8())?;
        let liveness_mode = DealerTransitionLivenessModeV1::decode(reader.u8())?;
        reader.reserved(6)?;
        reader.finish()?;
        let value = Self {
            replay_account_id,
            replay_pre_id,
            state_pre_content_id,
            state_post_content_id,
            position_pre_semantic_id,
            position_post_semantic_id,
            liveness_receipt_semantic_id,
            fee_receipt_semantic_id,
            asset_transfer_bundle_id,
            position_generation_before,
            position_generation_after,
            expected_ordinal,
            action,
            liveness_mode,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Opaque prepared replay advance; no field can be forged independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerReplayTransitionV1 {
    replay_account_id: Id,
    replay_pre_id: Id,
    replay_post: DealerFacilityReplayV1,
    intent: DealerTransitionIntentV1,
    intent_id: Id,
}

impl PreparedDealerReplayTransitionV1 {
    /// Expected canonical Replay post-state.
    pub const fn replay_post(self) -> DealerFacilityReplayV1 {
        self.replay_post
    }

    /// Exact accepted intent identity.
    pub const fn intent_id(self) -> Id {
        self.intent_id
    }
}

/// Runtime-observed atomic postconditions for an accepted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTransitionCommitObservationV1 {
    /// Actual mutated Replay account.
    pub replay_account_id: Id,
    /// Semantic identity recomputed from the Replay bytes before mutation.
    pub replay_pre_id: Id,
    /// Reloaded and decoded Replay post-state.
    pub replay_post: DealerFacilityReplayV1,
    /// Exact Dealer State content identity after all writes.
    pub state_post_content_id: Id,
    /// Exact canonical Position V3 semantic identity after all writes.
    pub position_post_semantic_id: Id,
    /// Exact executed asset-transfer receipt binding the complete bundle.
    pub asset_transfer_receipt_id: Id,
    /// Exact consumed liveness receipt, or zero for caller-funded execution.
    pub liveness_receipt_semantic_id: Id,
    /// Exact consumed fee receipt, or zero when inapplicable.
    pub fee_receipt_semantic_id: Id,
}

/// Accept a replay advance only after every State, Position, asset, fee, and
/// liveness postcondition has been reloaded and matched atomically.
pub fn accept_dealer_replay_transition_v1(
    prepared: PreparedDealerReplayTransitionV1,
    observed: DealerTransitionCommitObservationV1,
) -> Result<DealerFacilityReplayV1> {
    observed.replay_post.validate()?;
    if observed.replay_account_id != prepared.replay_account_id
        || observed.replay_pre_id != prepared.replay_pre_id
        || observed.replay_post != prepared.replay_post
        || observed.replay_post.last_transition_intent_id() != prepared.intent_id
        || observed.state_post_content_id != prepared.intent.state_post_content_id
        || observed.position_post_semantic_id != prepared.intent.position_post_semantic_id
        || observed.asset_transfer_receipt_id != prepared.intent.asset_transfer_bundle_id
        || observed.liveness_receipt_semantic_id != prepared.intent.liveness_receipt_semantic_id
        || observed.fee_receipt_semantic_id != prepared.intent.fee_receipt_semantic_id
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(observed.replay_post)
}

/// Forgeable terminal projection consumed only after runtime authentication.
///
/// Public fields confer no terminal authority. The Dealer State handler must
/// first authenticate and exhaust its exact child graph; retirement separately
/// authenticates the terminal Position and Replay account bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayTerminalJoinV1 {
    /// Actual Replay account being deleted.
    pub replay_account_id: Id,
    /// Exact Replay account retained by terminal Position V3.
    pub position_replay_account_id: Id,
    /// Exact canonical Position V3 account.
    pub facility_position_account_id: Id,
    /// Exact facility Position purpose-binding identity.
    pub facility_position_binding_id: Id,
    /// Exact terminal Position generation.
    pub position_generation: u64,
    /// Exact State-owned terminal receipt committed by the Dealer extension.
    pub terminal_state_receipt_id: Id,
    /// Canonical Realm neutral lamport sink authenticated outside Replay.
    pub neutral_sink: Id,
}

/// Lamport-only Replay close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayClosePlanV1 {
    /// Replay account deleted to zero lamports/data.
    replay_account_id: Id,
    /// Semantic identity of the exact terminal Replay bytes.
    terminal_replay_semantic_id: Id,
    /// Last terminal transition intent committed by the Dealer extension.
    last_transition_intent_id: Id,
    /// Exact terminal State receipt committed by the Dealer extension.
    terminal_state_receipt_id: Id,
    /// Rent payer and sole principal-refund recipient.
    payer: Id,
    /// Canonical Realm neutral lamport sink.
    neutral_sink: Id,
    /// Exact lamport principal refunded to the payer.
    payer_refund_lamports: u64,
    /// Donation floor and every later surplus routed to the neutral sink.
    neutral_surplus_lamports: u64,
    /// Expected payer balance after the atomic close.
    payer_balance_after: u64,
    /// Expected neutral-sink balance after the atomic close.
    neutral_sink_balance_after: u64,
}

impl DealerReplayClosePlanV1 {
    /// Exact Replay account deleted by the combined Position retirement commit.
    pub const fn replay_account_id(self) -> Id {
        self.replay_account_id
    }

    /// Semantic identity of the terminal Replay envelope and Dealer extension.
    pub const fn terminal_replay_semantic_id(self) -> Id {
        self.terminal_replay_semantic_id
    }

    /// Exact terminal transition intent retained as Dealer evidence.
    pub const fn last_transition_intent_id(self) -> Id {
        self.last_transition_intent_id
    }

    /// Exact terminal State receipt retained as Dealer evidence.
    pub const fn terminal_state_receipt_id(self) -> Id {
        self.terminal_state_receipt_id
    }

    /// Replay rent payer and sole principal-refund recipient.
    pub const fn payer(self) -> Id {
        self.payer
    }

    /// Canonical Realm neutral lamport sink.
    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    /// Exact refundable Replay rent principal.
    pub const fn payer_refund_lamports(self) -> u64 {
        self.payer_refund_lamports
    }

    /// Donation floor and later Replay surplus routed neutral.
    pub const fn neutral_surplus_lamports(self) -> u64 {
        self.neutral_surplus_lamports
    }

    /// Expected payer balance after atomic Replay deletion.
    pub const fn payer_balance_after(self) -> u64 {
        self.payer_balance_after
    }

    /// Expected neutral-sink balance after atomic Replay deletion.
    pub const fn neutral_sink_balance_after(self) -> u64 {
        self.neutral_sink_balance_after
    }
}

/// Plan Replay deletion only after exact State/Position terminal joins.
///
/// This function moves lamports only. Position cash, native Eggs, Hoard
/// principal, fees, and liveness compartments are absent by construction.
pub fn plan_dealer_replay_close_v1(
    replay: DealerFacilityReplayV1,
    join: DealerReplayTerminalJoinV1,
    replay_lamports: u64,
    payer_balance_before: u64,
    neutral_sink_balance_before: u64,
) -> Result<DealerReplayClosePlanV1> {
    replay.validate()?;
    for identity in [
        join.replay_account_id,
        join.position_replay_account_id,
        join.facility_position_account_id,
        join.facility_position_binding_id,
        join.terminal_state_receipt_id,
        join.neutral_sink,
    ] {
        identity.validate_live()?;
    }
    if join.replay_account_id != join.position_replay_account_id
        || join.replay_account_id != replay.replay_account_id()
        || join.facility_position_account_id != replay.facility_position_account_id()
        || join.facility_position_binding_id != replay.facility_position_binding_id()
        || join.position_generation != replay.position_generation()
        || join.terminal_state_receipt_id != replay.terminal_state_receipt_id()
        || replay.lifecycle() != ReplayV3Lifecycle::Terminal
    {
        return Err(Error::MismatchedBinding);
    }
    let rent = replay.rent();
    let payer = dealer_id(rent.payer());
    if payer == join.neutral_sink {
        return Err(Error::InvalidParameter);
    }
    let minimum_balance = rent
        .refundable_principal()
        .checked_add(rent.donation_floor())
        .ok_or(Error::ArithmeticOverflow)?;
    if replay_lamports < minimum_balance {
        return Err(Error::InvalidParameter);
    }
    let neutral_surplus_lamports = replay_lamports
        .checked_sub(rent.refundable_principal())
        .ok_or(Error::ArithmeticOverflow)?;
    let payer_balance_after = payer_balance_before
        .checked_add(rent.refundable_principal())
        .ok_or(Error::ArithmeticOverflow)?;
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(neutral_surplus_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let terminal_replay_semantic_id = replay.replay_id()?;
    Ok(DealerReplayClosePlanV1 {
        replay_account_id: join.replay_account_id,
        terminal_replay_semantic_id,
        last_transition_intent_id: replay.last_transition_intent_id(),
        terminal_state_receipt_id: replay.terminal_state_receipt_id(),
        payer,
        neutral_sink: join.neutral_sink,
        payer_refund_lamports: rent.refundable_principal(),
        neutral_surplus_lamports,
        payer_balance_after,
        neutral_sink_balance_after,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DealerActionLivenessPolicyV1 {
    CallerOnly,
    ExternalOnly,
    Either,
}

fn liveness_policy(action: DealerRuntimeActionV1) -> Result<DealerActionLivenessPolicyV1> {
    match action {
        DealerRuntimeActionV1::CreatePolicy => Err(Error::InvalidParameter),
        DealerRuntimeActionV1::Contribute
        | DealerRuntimeActionV1::WithdrawFunding
        | DealerRuntimeActionV1::SponsorHalt => Ok(DealerActionLivenessPolicyV1::CallerOnly),
        DealerRuntimeActionV1::QueueExit => Ok(DealerActionLivenessPolicyV1::Either),
        DealerRuntimeActionV1::Initialize
        | DealerRuntimeActionV1::CreateLpPage
        | DealerRuntimeActionV1::Activate
        | DealerRuntimeActionV1::CancelFunding
        | DealerRuntimeActionV1::RefundCancelledSponsor
        | DealerRuntimeActionV1::BindEpoch
        | DealerRuntimeActionV1::LapseEpoch
        | DealerRuntimeActionV1::SelectLeaseAndBegin
        | DealerRuntimeActionV1::Collect
        | DealerRuntimeActionV1::Deliver
        | DealerRuntimeActionV1::FinalizeSettlement
        | DealerRuntimeActionV1::AbortBeforeCollection
        | DealerRuntimeActionV1::EnterUnwind
        | DealerRuntimeActionV1::TimedClose
        | DealerRuntimeActionV1::Resolve
        | DealerRuntimeActionV1::Claim
        | DealerRuntimeActionV1::Retire => Ok(DealerActionLivenessPolicyV1::ExternalOnly),
    }
}

const fn action_requires_fee_receipt(action: DealerRuntimeActionV1) -> bool {
    matches!(
        action,
        DealerRuntimeActionV1::SelectLeaseAndBegin
            | DealerRuntimeActionV1::Collect
            | DealerRuntimeActionV1::Deliver
            | DealerRuntimeActionV1::FinalizeSettlement
            | DealerRuntimeActionV1::AbortBeforeCollection
    )
}

const fn liveness_mode_byte(mode: DealerTransitionLivenessModeV1) -> u8 {
    match mode {
        DealerTransitionLivenessModeV1::CallerFunded => 0,
        DealerTransitionLivenessModeV1::ExternalReceipt => 1,
    }
}

pub(crate) const fn action_byte(action: DealerRuntimeActionV1) -> u8 {
    match action {
        DealerRuntimeActionV1::CreatePolicy => 0,
        DealerRuntimeActionV1::Initialize => 1,
        DealerRuntimeActionV1::CreateLpPage => 2,
        DealerRuntimeActionV1::Contribute => 3,
        DealerRuntimeActionV1::WithdrawFunding => 4,
        DealerRuntimeActionV1::Activate => 5,
        DealerRuntimeActionV1::CancelFunding => 6,
        DealerRuntimeActionV1::RefundCancelledSponsor => 7,
        DealerRuntimeActionV1::BindEpoch => 8,
        DealerRuntimeActionV1::LapseEpoch => 9,
        DealerRuntimeActionV1::SelectLeaseAndBegin => 10,
        DealerRuntimeActionV1::Collect => 11,
        DealerRuntimeActionV1::Deliver => 12,
        DealerRuntimeActionV1::FinalizeSettlement => 13,
        DealerRuntimeActionV1::AbortBeforeCollection => 14,
        DealerRuntimeActionV1::QueueExit => 15,
        DealerRuntimeActionV1::SponsorHalt => 16,
        DealerRuntimeActionV1::EnterUnwind => 17,
        DealerRuntimeActionV1::TimedClose => 18,
        DealerRuntimeActionV1::Resolve => 19,
        DealerRuntimeActionV1::Claim => 20,
        DealerRuntimeActionV1::Retire => 21,
    }
}

pub(crate) fn decode_action(value: u8) -> Result<DealerRuntimeActionV1> {
    match value {
        0 => Ok(DealerRuntimeActionV1::CreatePolicy),
        1 => Ok(DealerRuntimeActionV1::Initialize),
        2 => Ok(DealerRuntimeActionV1::CreateLpPage),
        3 => Ok(DealerRuntimeActionV1::Contribute),
        4 => Ok(DealerRuntimeActionV1::WithdrawFunding),
        5 => Ok(DealerRuntimeActionV1::Activate),
        6 => Ok(DealerRuntimeActionV1::CancelFunding),
        7 => Ok(DealerRuntimeActionV1::RefundCancelledSponsor),
        8 => Ok(DealerRuntimeActionV1::BindEpoch),
        9 => Ok(DealerRuntimeActionV1::LapseEpoch),
        10 => Ok(DealerRuntimeActionV1::SelectLeaseAndBegin),
        11 => Ok(DealerRuntimeActionV1::Collect),
        12 => Ok(DealerRuntimeActionV1::Deliver),
        13 => Ok(DealerRuntimeActionV1::FinalizeSettlement),
        14 => Ok(DealerRuntimeActionV1::AbortBeforeCollection),
        15 => Ok(DealerRuntimeActionV1::QueueExit),
        16 => Ok(DealerRuntimeActionV1::SponsorHalt),
        17 => Ok(DealerRuntimeActionV1::EnterUnwind),
        18 => Ok(DealerRuntimeActionV1::TimedClose),
        19 => Ok(DealerRuntimeActionV1::Resolve),
        20 => Ok(DealerRuntimeActionV1::Claim),
        21 => Ok(DealerRuntimeActionV1::Retire),
        _ => Err(Error::InvalidParameter),
    }
}
