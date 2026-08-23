// SPDX-License-Identifier: AGPL-3.0-or-later

//! General-owned purpose Replay V3 extension and structural transition.
//!
//! This module is deliberately structural. It owns canonical bytes, hashes,
//! and the exhaustive General action/endpoint partition, but it does not
//! authenticate an SBF account owner, PDA, receipt, finalized row, or writable
//! account meta. A live action composer must rederive its concrete settlement
//! or structured-claim plan from authenticated prestates before committing the
//! Position and Replay postbodies together.

use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    PositionAccountV3, PositionPurposeV3, PositionV3Fields, PositionV3Sha256Backend,
    ReplayV3Envelope, ReplayV3EnvelopeHeader, ReplayV3HashBackend, ReplayV3Lifecycle,
};

use crate::{CodecError, Id32, Reader, Writer};

/// Exact General purpose-extension schema coordinate (`GEN1`).
pub const GENERAL_REPLAY_EXTENSION_SCHEMA_V1: u32 = u32::from_le_bytes(*b"GEN1");
/// Exact General purpose-extension width.
pub const GENERAL_REPLAY_EXTENSION_V1_BYTES: usize = 136;
/// Exact Replay V3 body width when carrying `GEN1`.
pub const GENERAL_REPLAY_ACCOUNT_V1_BYTES: usize =
    clutch_retirement::PURPOSE_REPLAY_V3_PREFIX_BYTES + GENERAL_REPLAY_EXTENSION_V1_BYTES;
/// Domain for one exact General Position pre/post delta.
pub const GENERAL_REPLAY_DELTA_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-replay/delta/v1\0";

const SETTLEMENT_FAMILY: u8 = 1;
const STRUCTURED_EXCHANGE_FAMILY: u8 = 2;
const TRANSITION_VERSION_V1: u8 = 1;
const OWNER_ACCOUNTING_ROLE: u8 = 1;
const OWNER_CASH_ROLE: u8 = 2;
const DIRECT_BUYER_ROLE: u8 = 3;
const DIRECT_SELLER_ROLE: u8 = 4;
const VIRTUAL_SPLIT_BUYER_ROLE: u8 = 5;
const VIRTUAL_MERGE_SELLER_ROLE: u8 = 6;
const STRUCTURED_GENERAL_ROLE: u8 = 7;

/// Exhaustive General Replay transition partition for schema v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReplayTransitionKindV1 {
    /// Action 25 owner accounting; the Position body is unchanged.
    AccountReceiptEnd,
    /// Action 38 owner cash realization.
    FinalizeOwnerSettlement,
    /// Action 26 buyer Position endpoint.
    DirectBuyer,
    /// Action 26 seller Position endpoint.
    DirectSeller,
    /// Action 36 real buyer Position endpoint.
    VirtualSplitBuyer,
    /// Action 37 real seller Position endpoint.
    VirtualMergeSeller,
    /// Action 35 General Position endpoint of a structured exchange.
    StructuredGeneral,
}

impl GeneralReplayTransitionKindV1 {
    fn coordinates(self) -> (u8, u8, u8, u8) {
        match self {
            Self::AccountReceiptEnd => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                25,
                OWNER_ACCOUNTING_ROLE,
            ),
            Self::FinalizeOwnerSettlement => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                38,
                OWNER_CASH_ROLE,
            ),
            Self::DirectBuyer => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                26,
                DIRECT_BUYER_ROLE,
            ),
            Self::DirectSeller => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                26,
                DIRECT_SELLER_ROLE,
            ),
            Self::VirtualSplitBuyer => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                36,
                VIRTUAL_SPLIT_BUYER_ROLE,
            ),
            Self::VirtualMergeSeller => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                37,
                VIRTUAL_MERGE_SELLER_ROLE,
            ),
            Self::StructuredGeneral => (
                STRUCTURED_EXCHANGE_FAMILY,
                TRANSITION_VERSION_V1,
                35,
                STRUCTURED_GENERAL_ROLE,
            ),
        }
    }

    fn from_coordinates(
        family: u8,
        version: u8,
        action: u8,
        role: u8,
    ) -> Result<Self, CodecError> {
        match (family, version, action, role) {
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 25, OWNER_ACCOUNTING_ROLE) => {
                Ok(Self::AccountReceiptEnd)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 38, OWNER_CASH_ROLE) => {
                Ok(Self::FinalizeOwnerSettlement)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 26, DIRECT_BUYER_ROLE) => {
                Ok(Self::DirectBuyer)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 26, DIRECT_SELLER_ROLE) => {
                Ok(Self::DirectSeller)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 36, VIRTUAL_SPLIT_BUYER_ROLE) => {
                Ok(Self::VirtualSplitBuyer)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 37, VIRTUAL_MERGE_SELLER_ROLE) => {
                Ok(Self::VirtualMergeSeller)
            }
            (
                STRUCTURED_EXCHANGE_FAMILY,
                TRANSITION_VERSION_V1,
                35,
                STRUCTURED_GENERAL_ROLE,
            ) => Ok(Self::StructuredGeneral),
            _ => Err(CodecError::InvalidState),
        }
    }

    /// Exact centrally allocated General action number.
    pub fn action(self) -> u8 {
        self.coordinates().2
    }

    /// Exact endpoint role within that action.
    pub fn role(self) -> u8 {
        self.coordinates().3
    }
}

/// Founding versus advanced General extension state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralReplayExtensionStateV1 {
    Initial,
    Advanced,
}

/// Canonical fixed General extension under the common Replay V3 prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReplayExtensionV1 {
    general_market_runtime: Id32,
    current_position_semantic_id: Id32,
    last_transition_id: Id32,
    last_delta_id: Id32,
    last_kind: Option<GeneralReplayTransitionKindV1>,
    state: GeneralReplayExtensionStateV1,
}

impl GeneralReplayExtensionV1 {
    /// Construct the unique founding extension for an exact current Position.
    pub fn initial(
        general_market_runtime: Id32,
        current_position_semantic_id: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            general_market_runtime,
            current_position_semantic_id,
            last_transition_id: Id32::ZERO,
            last_delta_id: Id32::ZERO,
            last_kind: None,
            state: GeneralReplayExtensionStateV1::Initial,
        };
        value.validate()?;
        Ok(value)
    }

    fn advanced(
        self,
        kind: GeneralReplayTransitionKindV1,
        transition_id: Id32,
        delta_id: Id32,
        current_position_semantic_id: Id32,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        let value = Self {
            general_market_runtime: self.general_market_runtime,
            current_position_semantic_id,
            last_transition_id: transition_id,
            last_delta_id: delta_id,
            last_kind: Some(kind),
            state: GeneralReplayExtensionStateV1::Advanced,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), CodecError> {
        if self.general_market_runtime.is_zero() || self.current_position_semantic_id.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        match (self.state, self.last_kind) {
            (GeneralReplayExtensionStateV1::Initial, None)
                if self.last_transition_id.is_zero() && self.last_delta_id.is_zero() =>
            {
                Ok(())
            }
            (GeneralReplayExtensionStateV1::Advanced, Some(_))
                if !self.last_transition_id.is_zero()
                    && !self.last_delta_id.is_zero()
                    && self.last_transition_id != self.last_delta_id =>
            {
                Ok(())
            }
            _ => Err(CodecError::InvalidState),
        }
    }

    /// General runtime Market PDA.
    pub const fn general_market_runtime(self) -> Id32 {
        self.general_market_runtime
    }

    /// Position semantic ID current at this Replay state.
    pub const fn current_position_semantic_id(self) -> Id32 {
        self.current_position_semantic_id
    }

    /// Most recently consumed transition identity, absent at founding.
    pub const fn last_transition_id(self) -> Id32 {
        self.last_transition_id
    }

    /// Most recently consumed Position-delta identity, absent at founding.
    pub const fn last_delta_id(self) -> Id32 {
        self.last_delta_id
    }

    /// Most recently consumed exact action/role tuple, absent at founding.
    pub const fn last_kind(self) -> Option<GeneralReplayTransitionKindV1> {
        self.last_kind
    }

    /// Encode exactly 136 canonical bytes.
    pub fn encode(self) -> Result<[u8; GENERAL_REPLAY_EXTENSION_V1_BYTES], CodecError> {
        self.validate()?;
        let mut output = [0_u8; GENERAL_REPLAY_EXTENSION_V1_BYTES];
        let mut writer = Writer::exact(&mut output, GENERAL_REPLAY_EXTENSION_V1_BYTES)?;
        writer.bytes(&self.general_market_runtime.bytes())?;
        writer.bytes(&self.current_position_semantic_id.bytes())?;
        writer.bytes(&self.last_transition_id.bytes())?;
        writer.bytes(&self.last_delta_id.bytes())?;
        let (family, version, action, role) = match self.last_kind {
            Some(kind) => kind.coordinates(),
            None => (0, 0, 0, 0),
        };
        writer.u8(action)?;
        writer.u8(match self.state {
            GeneralReplayExtensionStateV1::Initial => 0,
            GeneralReplayExtensionStateV1::Advanced => 1,
        })?;
        writer.u8(family)?;
        writer.u8(version)?;
        writer.u8(role)?;
        writer.bytes(&[0; 3])?;
        writer.finish()?;
        Ok(output)
    }

    /// Decode exactly 136 hostile bytes and reject every unallocated tuple.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, GENERAL_REPLAY_EXTENSION_V1_BYTES)?;
        let general_market_runtime = Id32::new(reader.array()?)?;
        let current_position_semantic_id = Id32::new(reader.array()?)?;
        let last_transition_id = Id32::from_bytes(reader.array()?);
        let last_delta_id = Id32::from_bytes(reader.array()?);
        let action = reader.u8()?;
        let state = reader.u8()?;
        let family = reader.u8()?;
        let version = reader.u8()?;
        let role = reader.u8()?;
        if reader.array::<3>()? != [0; 3] {
            return Err(CodecError::NonCanonicalPadding);
        }
        reader.finish()?;
        let (state, last_kind) = match state {
            0 if family == 0 && version == 0 && action == 0 && role == 0 => {
                (GeneralReplayExtensionStateV1::Initial, None)
            }
            1 => (
                GeneralReplayExtensionStateV1::Advanced,
                Some(GeneralReplayTransitionKindV1::from_coordinates(
                    family, version, action, role,
                )?),
            ),
            _ => return Err(CodecError::InvalidState),
        };
        let value = Self {
            general_market_runtime,
            current_position_semantic_id,
            last_transition_id,
            last_delta_id,
            last_kind,
            state,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Hash-checked structural General Position/Replay prestate.
///
/// Private fields prevent callers from assembling one without passing the
/// exact canonical Position and Replay bodies through the projection below.
/// This still does not authenticate an SBF program owner or PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionReplayPrestateV1 {
    position: AuthenticatedPositionV3,
    replay_account: Id32,
    replay_semantic_id: Id32,
    replay_header: ReplayV3EnvelopeHeader,
    extension: GeneralReplayExtensionV1,
}

impl GeneralPositionReplayPrestateV1 {
    /// Exact checked Position prestate.
    pub const fn position(self) -> AuthenticatedPositionV3 {
        self.position
    }

    /// Exact Replay V3 account key retained by its body.
    pub const fn replay_account(self) -> Id32 {
        self.replay_account
    }

    /// Internally derived Replay V3 semantic ID.
    pub const fn replay_semantic_id(self) -> Id32 {
        self.replay_semantic_id
    }

    /// Exact ordinal consumed by the next General transition.
    pub const fn next_sequence(self) -> u64 {
        self.replay_header.next_sequence()
    }

    /// Exact decoded General extension.
    pub const fn extension(self) -> GeneralReplayExtensionV1 {
        self.extension
    }
}

/// Decode and structurally bind exact Position V3 and General Replay V3 bytes.
pub fn project_general_position_replay_prestate_v1<B>(
    replay_account: Id32,
    canonical_replay_bump: u8,
    expected_next_sequence: u64,
    replay_body: &[u8],
    position: AuthenticatedPositionV3,
    backend: &B,
) -> Result<GeneralPositionReplayPrestateV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    position.validate().map_err(|_| CodecError::MismatchedBinding)?;
    let position_fields = position.semantic.fields();
    let derived_position_id = position
        .semantic
        .semantic_id(backend)
        .map_err(|_| CodecError::MismatchedBinding)?;
    if derived_position_id.bytes() != position.semantic_id {
        return Err(CodecError::MismatchedBinding);
    }
    let envelope = ReplayV3Envelope::decode(replay_body, backend)
        .map_err(|_| CodecError::MismatchedBinding)?;
    let header = envelope.header();
    let extension = GeneralReplayExtensionV1::decode(envelope.extension())?;
    let replay_semantic_id = Id32::new(
        envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::MismatchedBinding)?
            .bytes(),
    )?;
    if replay_account.is_zero()
        || header.lifecycle() != ReplayV3Lifecycle::Live
        || header.purpose() != PositionPurposeV3::General
        || header.extension_schema().get() != GENERAL_REPLAY_EXTENSION_SCHEMA_V1
        || usize::try_from(header.extension_len()).map_err(|_| CodecError::ArithmeticOverflow)?
            != GENERAL_REPLAY_EXTENSION_V1_BYTES
        || header.replay_account().bytes() != replay_account.bytes()
        || header.stored_bump() != canonical_replay_bump
        || header.next_sequence() != expected_next_sequence
        || header.replay_account() != position_fields.replay_account
        || header.position_account().bytes() != position.account
        || header.purpose() != position_fields.purpose
        || header.purpose_binding_id() != position_fields.purpose_binding_id
        || header.position_generation() != position_fields.generation
        || extension.general_market_runtime.bytes() != position.general_market_runtime
        || extension.current_position_semantic_id.bytes() != position.semantic_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(GeneralPositionReplayPrestateV1 {
        position,
        replay_account,
        replay_semantic_id,
        replay_header: header,
        extension,
    })
}

/// Structural exact Replay successor; never a standalone execution capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReplayTransitionPlanV1 {
    replay_account: Id32,
    replay_prestate_semantic_id: Id32,
    replay_poststate_semantic_id: Id32,
    replay_poststate_body: [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
    position_account: Id32,
    position_prestate_semantic_id: Id32,
    position_poststate_semantic_id: Id32,
    kind: GeneralReplayTransitionKindV1,
    transition_id: Id32,
    transition_authority_data_id: Id32,
    delta_id: Id32,
    consumed_sequence: u64,
    next_sequence: u64,
}

impl GeneralReplayTransitionPlanV1 {
    /// Replay account to compare-and-write.
    pub const fn replay_account(&self) -> Id32 {
        self.replay_account
    }

    /// Exact Replay prestate semantic ID.
    pub const fn replay_prestate_semantic_id(&self) -> Id32 {
        self.replay_prestate_semantic_id
    }

    /// Exact internally derived Replay successor semantic ID.
    pub const fn replay_poststate_semantic_id(&self) -> Id32 {
        self.replay_poststate_semantic_id
    }

    /// Exact canonical 344-byte Replay successor body.
    pub const fn replay_poststate_body(&self) -> &[u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES] {
        &self.replay_poststate_body
    }

    /// Position account serialized by this Replay.
    pub const fn position_account(&self) -> Id32 {
        self.position_account
    }

    /// Position semantic ID before the action.
    pub const fn position_prestate_semantic_id(&self) -> Id32 {
        self.position_prestate_semantic_id
    }

    /// Internally derived Position semantic ID after the action.
    pub const fn position_poststate_semantic_id(&self) -> Id32 {
        self.position_poststate_semantic_id
    }

    /// Exact action and endpoint role.
    pub const fn kind(&self) -> GeneralReplayTransitionKindV1 {
        self.kind
    }

    /// Exact receipt/finalized-row transition identity.
    pub const fn transition_id(&self) -> Id32 {
        self.transition_id
    }

    /// Data ID of the semantic owner that must authenticate the transition.
    pub const fn transition_authority_data_id(&self) -> Id32 {
        self.transition_authority_data_id
    }

    /// Domain-separated Position delta identity.
    pub const fn delta_id(&self) -> Id32 {
        self.delta_id
    }

    /// Ordinal consumed by this transition.
    pub const fn consumed_sequence(&self) -> u64 {
        self.consumed_sequence
    }

    /// Exact successor ordinal.
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

/// Project one exact structural General Replay successor.
///
/// `transition_id` and `transition_authority_data_id` are committed but not
/// authenticated here. A live action-specific composer must obtain them from
/// its private receipt/finalized-row plan and then rederive this whole result.
pub fn project_general_replay_transition_v1<B>(
    prestate: GeneralPositionReplayPrestateV1,
    position_poststate: PositionSettlementPoststateV3,
    kind: GeneralReplayTransitionKindV1,
    transition_id: Id32,
    transition_authority_data_id: Id32,
    backend: &B,
) -> Result<GeneralReplayTransitionPlanV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    if transition_id.is_zero() || transition_authority_data_id.is_zero() {
        return Err(CodecError::ZeroIdentity);
    }
    let position_prestate = prestate.position;
    let pre_fields = position_prestate.semantic.fields();
    let post_fields = position_poststate.semantic.fields();
    if position_poststate.account != position_prestate.account
        || position_poststate.general_market_runtime != position_prestate.general_market_runtime
        || position_poststate.prestate_semantic_id != position_prestate.semantic_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    let expected_poststate = PositionAccountV3::new(PositionV3Fields {
        cash_atoms: post_fields.cash_atoms,
        reserved_cash_atoms: post_fields.reserved_cash_atoms,
        native_eggs: post_fields.native_eggs,
        ..pre_fields
    })
    .map_err(|_| CodecError::InvalidState)?;
    if position_poststate.semantic != expected_poststate {
        return Err(CodecError::MismatchedBinding);
    }
    let position_poststate_semantic_id = Id32::new(
        position_poststate
            .semantic
            .semantic_id(backend)
            .map_err(|_| CodecError::MismatchedBinding)?
            .bytes(),
    )?;
    let position_prestate_semantic_id = Id32::new(position_prestate.semantic_id)?;
    let unchanged_required = kind == GeneralReplayTransitionKindV1::AccountReceiptEnd;
    let changed_required = matches!(
        kind,
        GeneralReplayTransitionKindV1::DirectBuyer
            | GeneralReplayTransitionKindV1::VirtualSplitBuyer
            | GeneralReplayTransitionKindV1::StructuredGeneral
    );
    if (unchanged_required
        && (position_poststate.semantic != position_prestate.semantic
            || position_poststate_semantic_id != position_prestate_semantic_id))
        || (changed_required
            && (position_poststate.semantic == position_prestate.semantic
                || position_poststate_semantic_id == position_prestate_semantic_id))
    {
        return Err(CodecError::InvalidState);
    }
    let consumed_sequence = prestate.replay_header.next_sequence();
    let (family, version, action, role) = kind.coordinates();
    let delta_id = Id32::new(backend.sha256_parts(&[
        GENERAL_REPLAY_DELTA_DOMAIN_V1,
        &[family],
        &[version],
        &[action],
        &[role],
        &consumed_sequence.to_le_bytes(),
        &transition_id.bytes(),
        &transition_authority_data_id.bytes(),
        &position_prestate.account,
        &position_prestate.semantic_id,
        &position_poststate_semantic_id.bytes(),
        &pre_fields.generation.to_le_bytes(),
        &post_fields.generation.to_le_bytes(),
    ]))?;
    let extension = prestate.extension.advanced(
        kind,
        transition_id,
        delta_id,
        position_poststate_semantic_id,
    )?;
    let extension_body = extension.encode()?;
    let replay_header = prestate
        .replay_header
        .advanced_live(post_fields.generation, &extension_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let replay_envelope = ReplayV3Envelope::from_header(replay_header, &extension_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let replay_poststate_semantic_id = Id32::new(
        replay_envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let mut replay_poststate_body = [0_u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES];
    replay_envelope
        .encode_into(&mut replay_poststate_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    Ok(GeneralReplayTransitionPlanV1 {
        replay_account: prestate.replay_account,
        replay_prestate_semantic_id: prestate.replay_semantic_id,
        replay_poststate_semantic_id,
        replay_poststate_body,
        position_account: Id32::new(position_prestate.account)?,
        position_prestate_semantic_id,
        position_poststate_semantic_id,
        kind,
        transition_id,
        transition_authority_data_id,
        delta_id,
        consumed_sequence,
        next_sequence: replay_header.next_sequence(),
    })
}
