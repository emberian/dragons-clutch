//! Generated, runtime-width General EffectProgram V3 artifacts.
//!
//! Every child template is first constructed by its semantic-owner codec and
//! then patched only through typed public field coordinates.  Account starts
//! are derived from the frozen Hot38 logical prefix
//! `[root, config, product, portfolio, linked-basis]`; no physical Hot account
//! index enters the artifact.  Generic Trading remains the sole request
//! projector, account writer, CPI authority, and atomic committer.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, hot_v3::HOT_RUNTIME_ROOT_COORDINATE_V3,
};
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    affine_batch_v2::{
        AFFINE_BATCH_PLAN_HEADER_BYTES_V2, AFFINE_BATCH_POSITION_BYTES_V2,
        AFFINE_BATCH_ROW_BYTES_V2, AffineBatchPlanInputV2, AffineBatchPlanV2,
        AffineBatchPositionV2, AffineBatchRequestLayoutV2, AffineBatchRowInputV2, AffineBatchRowV2,
        DeltaDirectionV2, SignedMagnitudeV2,
    },
    frame_spec_v1::{
        AFFINE_FIXED_ACCOUNT_COUNT_V1, ClaimsFrameSpecV1, PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1,
        PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_REQUEST_BYTES_V2, ProtocolPositionActionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionPresenceV2, ProtocolPositionRequestLayoutV2, ProtocolPositionRequestV2,
    },
};
use dclutch_custody_contract::{
    CLOSE_REPLAY_ACCOUNT_COUNT_V1 as CUSTODY_CLOSE_REPLAY_ACCOUNT_COUNT_V1,
    CLOSE_VAULT_ACCOUNT_COUNT_V1 as CUSTODY_CLOSE_VAULT_ACCOUNT_COUNT_V1, CUSTODY_RECEIPT_BYTES_V1,
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyFrameSpecV1,
    CustodyRequestLayoutV1, CustodyRequestV1,
    INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as CUSTODY_INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
    OPEN_VAULT_ACCOUNT_COUNT_V1 as CUSTODY_OPEN_VAULT_ACCOUNT_COUNT_V1, OperationV1,
    TRANSFER_ACCOUNT_COUNT_V1 as CUSTODY_TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, OPERATION_BYTES as EFFECT_OPERATION_BYTES_V3,
        ProgramV3, RECEIPT_DEPENDENCY_BYTES as EFFECT_RECEIPT_DEPENDENCY_BYTES_V3,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES_V3, RouteKindV3, RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
    v4::{
        BorrowedRangePolicyV4, HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, ProgramV4,
        encode_program_v4_atomic,
    },
};
use dclutch_general_codec::{Action, CONTROLLER_REQUEST_BYTES};
use dclutch_general_config_contract::{
    GENERAL_ROOT_NEXT_BATCH_SEQUENCE_OFFSET_V2, GENERAL_ROOT_OPEN_BATCHES_OFFSET_V2,
    GENERAL_ROOT_REVISION_OFFSET_V2,
};

use crate::hot_candidate_v3::{
    GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
    GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, GENERAL_HOT_ITEM_SCALAR_STRIDE_V3, identity, item_scalar,
    scalar,
};
use crate::{
    candidate_v1::GeneralCandidateLayoutV1,
    collection_v1::{
        GENERAL_ORDER_ROW_BASE_V1, GENERAL_ORDER_ROW_DELIVER_OFFSET_V1,
        GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1, GENERAL_ORDER_ROW_STRIDE_V1, GeneralBatchLayoutV1,
        GeneralOrderLayoutV1,
    },
    local_state_v3::GeneralLocalStateLayoutV3,
    runtime_selection::RuntimeSelectionLayoutV2,
    runtime_verify::RuntimeVerifierLayoutV2,
    runtime_width::{SettlementCursorLayoutV2, VerifiedCandidateLayoutV2},
    state_artifacts_v3::{
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        GENERAL_VERIFY_PAYER_ACCOUNT_V3, GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3,
        GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3, general_child_account_start_v3,
    },
};

/// Frozen Hot38 logical fixed prefix ending at linked-basis coordinate four.
pub const GENERAL_HOT_LOGICAL_PREFIX_ACCOUNTS_V3: u16 = 5;
/// First action-selected General state account after the readonly Hot prefix.
pub const GENERAL_STATE_ACCOUNT_COORDINATE_V3: u16 = GENERAL_PRIMARY_STATE_ACCOUNT_V3;

/// One exact child semantic frame selected by a General effect route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralChildFrameV3 {
    /// Claims protocol-Position lifecycle frame.
    ClaimsProtocolPosition(ProtocolPositionActionV2),
    /// Claims runtime-width affine frame.
    ClaimsAffine {
        /// Exact nonzero sorted Position-table width.
        position_count: u32,
    },
    /// Custody operation frame.
    Custody(OperationV1),
}

impl GeneralChildFrameV3 {
    /// Exact logical account count for this child frame.
    pub fn account_count(self) -> Result<u16> {
        match self {
            Self::ClaimsProtocolPosition(action) => ClaimsFrameSpecV1::protocol_position(action)
                .account_count()
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry),
            Self::ClaimsAffine { position_count } => ClaimsFrameSpecV1::affine(position_count)
                .and_then(ClaimsFrameSpecV1::account_count)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry),
            Self::Custody(operation) => Ok(CustodyFrameSpecV1::new(operation).account_count()),
        }
    }
}

/// Exact logical placement of one child frame in a General AccountProfile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEffectRouteFrameV3 {
    /// First logical account coordinate in the complete runtime vector.
    pub account_start: u16,
    /// Child semantic owner and action-selected frame.
    pub frame: GeneralChildFrameV3,
}

/// Exact number of child routes selected by one General action.
#[must_use]
pub const fn general_effect_route_count_v3(action: Action) -> u16 {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        // The root-writing pair moves no Claims Position and no Custody vault:
        // its writes are the root tail and the batch record, both local.
        Action::OpenBatch | Action::CloseBatch => 0,
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Materialize | Action::Distribute => 2,
        Action::Close => 4,
        // The refund and the whole escrow teardown: claims leg, quote leg,
        // Position close, vault close, replay close. Cancel refunds the exact
        // reserve while the batch still collects; Release the observed
        // residual after the window.
        Action::CancelOrder | Action::ReleaseOrder => 5,
        // The admission and the whole escrow construction: replay create,
        // vault open, Position admit, claims escrow-in, quote deposit.
        Action::PlaceOrder => 5,
    }
}

/// Return one exact child frame and logical placement.
pub fn general_effect_route_frame_v3(
    action: Action,
    route: u16,
) -> Result<GeneralEffectRouteFrameV3> {
    let start = general_child_account_start_v3(action);
    let (account_start, frame) = match (action, route) {
        (Action::InitializeSettlement, 0) => (
            start,
            GeneralChildFrameV3::ClaimsProtocolPosition(ProtocolPositionActionV2::Admit),
        ),
        (Action::InitializeSettlement, 1) => (
            add_accounts(start, POSITION_ADMIT_ACCOUNTS)?,
            GeneralChildFrameV3::Custody(OperationV1::InitializeReplay),
        ),
        (Action::InitializeSettlement, 2) => (
            add_accounts(
                add_accounts(start, POSITION_ADMIT_ACCOUNTS)?,
                CUSTODY_INITIALIZE_ACCOUNTS,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::OpenVault),
        ),
        (Action::Collect | Action::Distribute, 0) => (
            start,
            GeneralChildFrameV3::ClaimsAffine { position_count: 2 },
        ),
        (Action::Materialize, 0) => (
            start,
            GeneralChildFrameV3::ClaimsAffine { position_count: 1 },
        ),
        (Action::Collect | Action::Distribute, 1) => (
            add_accounts(start, AFFINE_FIXED_ACCOUNTS + 2)?,
            GeneralChildFrameV3::Custody(OperationV1::Transfer),
        ),
        (Action::Materialize, 1) => (
            add_accounts(start, AFFINE_FIXED_ACCOUNTS + 1)?,
            GeneralChildFrameV3::Custody(OperationV1::Transfer),
        ),
        (Action::Close, 0) => (start, GeneralChildFrameV3::Custody(OperationV1::Transfer)),
        (Action::Close, 1) => (
            add_accounts(start, CUSTODY_TRANSFER_ACCOUNTS)?,
            GeneralChildFrameV3::ClaimsProtocolPosition(ProtocolPositionActionV2::Close),
        ),
        (Action::Close, 2) => (
            add_accounts(
                add_accounts(start, CUSTODY_TRANSFER_ACCOUNTS)?,
                POSITION_CLOSE_ACCOUNTS,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::CloseVault),
        ),
        (Action::Close, 3) => (
            add_accounts(
                add_accounts(
                    add_accounts(start, CUSTODY_TRANSFER_ACCOUNTS)?,
                    POSITION_CLOSE_ACCOUNTS,
                )?,
                CUSTODY_CLOSE_VAULT_ACCOUNTS,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::CloseReplay),
        ),
        (Action::PlaceOrder, 0) => (
            start,
            GeneralChildFrameV3::Custody(OperationV1::InitializeReplay),
        ),
        (Action::PlaceOrder, 1) => (
            add_accounts(start, CUSTODY_INITIALIZE_ACCOUNTS)?,
            GeneralChildFrameV3::Custody(OperationV1::OpenVault),
        ),
        (Action::PlaceOrder, 2) => (
            add_accounts(
                add_accounts(start, CUSTODY_INITIALIZE_ACCOUNTS)?,
                CUSTODY_OPEN_ACCOUNTS,
            )?,
            GeneralChildFrameV3::ClaimsProtocolPosition(ProtocolPositionActionV2::Admit),
        ),
        (Action::PlaceOrder, 3) => (
            add_accounts(
                add_accounts(
                    add_accounts(start, CUSTODY_INITIALIZE_ACCOUNTS)?,
                    CUSTODY_OPEN_ACCOUNTS,
                )?,
                POSITION_ADMIT_ACCOUNTS,
            )?,
            GeneralChildFrameV3::ClaimsAffine { position_count: 2 },
        ),
        (Action::PlaceOrder, 4) => (
            add_accounts(
                add_accounts(
                    add_accounts(
                        add_accounts(start, CUSTODY_INITIALIZE_ACCOUNTS)?,
                        CUSTODY_OPEN_ACCOUNTS,
                    )?,
                    POSITION_ADMIT_ACCOUNTS,
                )?,
                add_accounts(AFFINE_FIXED_ACCOUNTS, 2)?,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::Transfer),
        ),
        (Action::CancelOrder | Action::ReleaseOrder, 0) => (
            start,
            GeneralChildFrameV3::ClaimsAffine { position_count: 2 },
        ),
        (Action::CancelOrder | Action::ReleaseOrder, 1) => (
            add_accounts(start, AFFINE_FIXED_ACCOUNTS + 2)?,
            GeneralChildFrameV3::Custody(OperationV1::Transfer),
        ),
        (Action::CancelOrder | Action::ReleaseOrder, 2) => (
            add_accounts(
                add_accounts(start, AFFINE_FIXED_ACCOUNTS + 2)?,
                CUSTODY_TRANSFER_ACCOUNTS,
            )?,
            GeneralChildFrameV3::ClaimsProtocolPosition(ProtocolPositionActionV2::Close),
        ),
        (Action::CancelOrder | Action::ReleaseOrder, 3) => (
            add_accounts(
                add_accounts(
                    add_accounts(start, AFFINE_FIXED_ACCOUNTS + 2)?,
                    CUSTODY_TRANSFER_ACCOUNTS,
                )?,
                POSITION_CLOSE_ACCOUNTS,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::CloseVault),
        ),
        (Action::CancelOrder | Action::ReleaseOrder, 4) => (
            add_accounts(
                add_accounts(
                    add_accounts(
                        add_accounts(start, AFFINE_FIXED_ACCOUNTS + 2)?,
                        CUSTODY_TRANSFER_ACCOUNTS,
                    )?,
                    POSITION_CLOSE_ACCOUNTS,
                )?,
                CUSTODY_CLOSE_VAULT_ACCOUNTS,
            )?,
            GeneralChildFrameV3::Custody(OperationV1::CloseReplay),
        ),
        _ => return Err(GeneralEffectArtifactErrorV3::Geometry),
    };
    let selected = GeneralEffectRouteFrameV3 {
        account_start,
        frame,
    };
    let end = selected
        .account_start
        .checked_add(selected.frame.account_count()?)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    if end > general_effect_account_count_v3(action)? {
        return Err(GeneralEffectArtifactErrorV3::Geometry);
    }
    Ok(selected)
}

const POSITION_ADMIT_ACCOUNTS: u16 = PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1;
const POSITION_CLOSE_ACCOUNTS: u16 = PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1;
const AFFINE_FIXED_ACCOUNTS: u16 = AFFINE_FIXED_ACCOUNT_COUNT_V1;
const CUSTODY_INITIALIZE_ACCOUNTS: u16 = CUSTODY_INITIALIZE_REPLAY_ACCOUNT_COUNT_V1;
const CUSTODY_OPEN_ACCOUNTS: u16 = CUSTODY_OPEN_VAULT_ACCOUNT_COUNT_V1;
const CUSTODY_TRANSFER_ACCOUNTS: u16 = CUSTODY_TRANSFER_ACCOUNT_COUNT_V1;
const CUSTODY_CLOSE_VAULT_ACCOUNTS: u16 = CUSTODY_CLOSE_VAULT_ACCOUNT_COUNT_V1;
const CUSTODY_CLOSE_REPLAY_ACCOUNTS: u16 = CUSTODY_CLOSE_REPLAY_ACCOUNT_COUNT_V1;
const MAX_ROUTE_COUNT: usize = 5;

/// Stable refusal from General artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEffectArtifactErrorV3 {
    /// An action, account, instruction, or template width overflowed.
    Geometry,
    /// A canonical Claims template could not be constructed.
    Claims,
    /// A canonical Custody template could not be constructed.
    Custody,
    /// The generic EffectProgram encoder refused the complete artifact.
    Effect(dclutch_effect_kernel::v3::Error),
    /// The V4 envelope encoder refused, or did not preserve its V3 base.
    Envelope,
    /// The action is a declared protocol selector with no authored artifacts.
    UnauthoredAction,
}

/// Whether one action's complete V3 artifact triple has been authored.
#[must_use]
pub const fn general_action_artifacts_authored_v3(_action: Action) -> bool {
    true
}

/// Refuse one action whose artifact triple has not been authored.
pub const fn require_authored_action_v3(action: Action) -> Result<()> {
    if general_action_artifacts_authored_v3(action) {
        Ok(())
    } else {
        Err(GeneralEffectArtifactErrorV3::UnauthoredAction)
    }
}

/// Result alias for General EffectProgram generation.
pub type Result<T> = core::result::Result<T, GeneralEffectArtifactErrorV3>;

/// A harmless initializer for caller-owned instruction workspaces.
///
/// Only the exact prefix reported by [`general_effect_instruction_count_v3`]
/// is consumed; every entry is overwritten before encoding.
pub const GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3: EffectInstructionV3 =
    EffectInstructionV3::write_request_u8(
        0,
        RequestSpaceV3::Fixed,
        0,
        ScalarCoordinateV3::common(0),
    );

/// Return `(fixed, repeated-item)` instruction counts for one action artifact.
pub const fn general_effect_instruction_count_v3(action: Action) -> (usize, usize) {
    match action {
        Action::SubmitCandidate => (23, 0),
        Action::VerifyCandidateRow => (53, 7),
        Action::CloseCandidate => (3, 0),
        Action::OpenBatch => (24, 0),
        Action::CloseBatch => (4, 0),
        Action::PlaceOrder => (96, 13),
        Action::CancelOrder => (92, 11),
        Action::ReleaseOrder => (90, 11),
        Action::Consider => (22, 0),
        Action::Freeze => (16, 0),
        Action::InitializeSettlement => (54, 1),
        Action::Collect | Action::Materialize | Action::Distribute => (48, 12),
        Action::Close => (92, 1),
    }
}

/// Return the exact child-template workspace width for one action artifact.
pub const fn general_effect_template_bytes_v3(action: Action) -> usize {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        Action::OpenBatch | Action::CloseBatch => 0,
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => {
            PROTOCOL_POSITION_REQUEST_BYTES_V2 + 2 * CUSTODY_REQUEST_BYTES_V1
        }
        Action::Collect | Action::Distribute => {
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2
                + 2 * AFFINE_BATCH_POSITION_BYTES_V2
                + AFFINE_BATCH_ROW_BYTES_V2
                + CUSTODY_REQUEST_BYTES_V1
        }
        Action::Materialize => {
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2
                + AFFINE_BATCH_POSITION_BYTES_V2
                + AFFINE_BATCH_ROW_BYTES_V2
                + CUSTODY_REQUEST_BYTES_V1
        }
        Action::Close => PROTOCOL_POSITION_REQUEST_BYTES_V2 + 3 * CUSTODY_REQUEST_BYTES_V1,
        Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => {
            AFFINE_BATCH_PLAN_HEADER_BYTES_V2
                + 2 * AFFINE_BATCH_POSITION_BYTES_V2
                + AFFINE_BATCH_ROW_BYTES_V2
                + PROTOCOL_POSITION_REQUEST_BYTES_V2
                + 3 * CUSTODY_REQUEST_BYTES_V1
        }
    }
}

/// Return the exact finalized V4-envelope EffectProgram width for one action.
///
/// This is the width the RELEASE carries. The V3 program is the semantic body;
/// the envelope is what the family-neutral Hot executor will decode.
pub fn general_effect_program_bytes_v4(action: Action) -> Result<usize> {
    EFFECT_HEADER_BYTES_V4
        .checked_add(general_effect_program_bytes_v3(action)?)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

/// Emit one action-selected General EffectProgram in its V4 envelope.
///
/// **Why this exists.** `process_hot_execution_v3` accepts exactly one effect
/// schema -- `dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4` -- and refuses
/// anything else with `UnsupportedContent` at both
/// `decode_sealed_effect_v4` and `decode_selected_effect_v4`. General emitted a
/// bare V3 program, so **nothing General published could enter the Hot executor
/// at all**, for any of the seven actions. That is not a General bug and not a
/// Trading bug; it is a schema generation the family never crossed, and it was
/// invisible because no General release had ever been executed through Hot.
///
/// The envelope is a pure extension: zero dynamic spans and zero borrowed
/// ranges, which is exactly the condition `decode_selected_effect_v4` requires
/// of a program whose register geometry comes from its V3 base. General's sole
/// dynamic span is declared by its ACCOUNT PROFILE (the trailing Trading-owned
/// scratch-page span), not by its effect, so there is nothing for the effect to
/// carry here and a nonempty span list would be a second, disagreeing author.
///
/// It is not free: the envelope moves the effect digest, and the certificate,
/// the admission, the strategy, the descriptor, the ProgramSet and the
/// capability seal are all content-addressed on it. Those move together, in one
/// batched regeneration.
pub fn encode_general_effect_program_v4_atomic(
    action: Action,
    instruction_workspace: &mut [EffectInstructionV3],
    template_workspace: &mut [u8],
    base_scratch: &mut [u8],
    base_output: &mut [u8],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let envelope = general_effect_program_bytes_v4(action)?;
    if scratch.len() != envelope || output.len() != envelope {
        return Err(GeneralEffectArtifactErrorV3::Geometry);
    }
    encode_general_effect_program_v3_atomic(
        action,
        instruction_workspace,
        template_workspace,
        base_scratch,
        base_output,
    )?;
    encode_program_v4_atomic(
        base_output,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(CONTROLLER_REQUEST_BYTES)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| GeneralEffectArtifactErrorV3::Envelope)?;
    // Encode then hostile-decode our own candidate, and prove the base survived
    // the wrap byte for byte: the envelope must add a header and change nothing.
    let decoded = ProgramV4::decode(output).map_err(|_| GeneralEffectArtifactErrorV3::Envelope)?;
    if decoded.base().bytes() != base_output || decoded.span_count() != 0 {
        return Err(GeneralEffectArtifactErrorV3::Envelope);
    }
    Ok(())
}

/// Return the exact finalized EffectProgram byte width for one action.
pub fn general_effect_program_bytes_v3(action: Action) -> Result<usize> {
    require_authored_action_v3(action)?;
    let (fixed, item) = general_effect_instruction_count_v3(action);
    EFFECT_HEADER_BYTES_V3
        .checked_add(
            route_count(action)
                .checked_mul(EFFECT_ROUTE_BYTES_V3)
                .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .and_then(|value| {
            value.checked_add(
                receipt_dependency_count(action).checked_mul(EFFECT_RECEIPT_DEPENDENCY_BYTES_V3)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                fixed
                    .checked_add(item)?
                    .checked_mul(EFFECT_OPERATION_BYTES_V3)?,
            )
        })
        .and_then(|value| value.checked_add(general_effect_template_bytes_v3(action)))
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

const fn receipt_dependency_count(action: Action) -> usize {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        Action::InitializeSettlement
        | Action::Close
        | Action::PlaceOrder
        | Action::CancelOrder
        | Action::ReleaseOrder => 1,
        Action::OpenBatch
        | Action::CloseBatch
        | Action::Consider
        | Action::Freeze
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 0,
    }
}

/// Coordinates carrying the release-selected Custody program, zero or one.
///
/// The family-neutral Hot executor resolves a child route's callee by scanning
/// the downgraded effect accounts for the key the Registry activation cache
/// names for that role, and a CPI's callee is never a member of its own account
/// list. `CustodyFrameRoleV1` has no `CustodyProgram` variant at all -- a
/// Custody frame names `CallerProgram`, which is Trading's -- so no Custody
/// frame can carry it and the topology must declare a coordinate of its own or
/// every Custody route refuses `AccountFrame` before any CPI. The Claims routes
/// need nothing: the Claims FrameSpecs declare `ClaimsProgram` inside their own
/// frames, which is why only Custody was missing.
///
/// `Consider` and `Freeze` route to no child at all, so they carry no callee
/// and pay no packet slot for one.
#[must_use]
pub const fn general_custody_callee_account_count_v3(action: Action) -> u16 {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        Action::OpenBatch | Action::CloseBatch | Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute
        | Action::Close
        | Action::PlaceOrder
        | Action::CancelOrder
        | Action::ReleaseOrder => 1,
    }
}

/// The callee coordinate itself, absent for the two child-free actions.
///
/// It is appended past every route range, so adding it renumbered no frame.
pub fn general_custody_callee_coordinate_v3(action: Action) -> Result<Option<u16>> {
    if general_custody_callee_account_count_v3(action) == 0 {
        return Ok(None);
    }
    general_effect_account_count_v3(action)?
        .checked_sub(1)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
        .map(Some)
}

/// Return the exact logical account width selected by one action AccountProfile.
pub fn general_effect_account_count_v3(action: Action) -> Result<u16> {
    require_authored_action_v3(action)?;
    let suffix = match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        Action::OpenBatch | Action::CloseBatch | Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => {
            POSITION_ADMIT_ACCOUNTS + CUSTODY_INITIALIZE_ACCOUNTS + CUSTODY_OPEN_ACCOUNTS
        }
        Action::Collect | Action::Distribute => {
            AFFINE_FIXED_ACCOUNTS + 2 + CUSTODY_TRANSFER_ACCOUNTS
        }
        Action::Materialize => AFFINE_FIXED_ACCOUNTS + 1 + CUSTODY_TRANSFER_ACCOUNTS,
        Action::Close => {
            CUSTODY_TRANSFER_ACCOUNTS
                + POSITION_CLOSE_ACCOUNTS
                + CUSTODY_CLOSE_VAULT_ACCOUNTS
                + CUSTODY_CLOSE_REPLAY_ACCOUNTS
        }
        Action::CancelOrder | Action::ReleaseOrder => {
            AFFINE_FIXED_ACCOUNTS
                + 2
                + CUSTODY_TRANSFER_ACCOUNTS
                + POSITION_CLOSE_ACCOUNTS
                + CUSTODY_CLOSE_VAULT_ACCOUNTS
                + CUSTODY_CLOSE_REPLAY_ACCOUNTS
        }
        Action::PlaceOrder => {
            CUSTODY_INITIALIZE_ACCOUNTS
                + CUSTODY_OPEN_ACCOUNTS
                + POSITION_ADMIT_ACCOUNTS
                + AFFINE_FIXED_ACCOUNTS
                + 2
                + CUSTODY_TRANSFER_ACCOUNTS
        }
    };
    general_child_account_start_v3(action)
        .checked_add(suffix)
        .and_then(|value| value.checked_add(general_custody_callee_account_count_v3(action)))
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

/// Generate one complete action-selected General EffectProgram atomically.
///
/// All four caller workspaces must have the exact reported width.  The output
/// remains untouched unless every child template and the complete generic
/// EffectProgram hostile-decode accepts.
pub fn encode_general_effect_program_v3_atomic(
    action: Action,
    instruction_workspace: &mut [EffectInstructionV3],
    template_workspace: &mut [u8],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let (fixed_count, item_count) = general_effect_instruction_count_v3(action);
    let expected_instructions = fixed_count
        .checked_add(item_count)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    let expected_program = general_effect_program_bytes_v3(action)?;
    if instruction_workspace.len() != expected_instructions
        || template_workspace.len() != general_effect_template_bytes_v3(action)
        || scratch.len() != expected_program
        || output.len() != expected_program
    {
        return Err(GeneralEffectArtifactErrorV3::Geometry);
    }
    template_workspace.fill(0);
    let mut fixed_cursor = 0;
    let mut item_cursor = fixed_count;
    let mut routes = [empty_route(); MAX_ROUTE_COUNT];
    let route_len = build_action(
        action,
        instruction_workspace,
        &mut fixed_cursor,
        &mut item_cursor,
        template_workspace,
        &mut routes,
    )?;
    append_general_state_patches(
        action,
        instruction_workspace,
        &mut fixed_cursor,
        &mut item_cursor,
    )?;
    if fixed_cursor != fixed_count || item_cursor != expected_instructions {
        return Err(GeneralEffectArtifactErrorV3::Geometry);
    }
    let geometry = EffectGeometryV3 {
        fixed_accounts: general_effect_account_count_v3(action)?,
        item_account_stride: 0,
        common_scalars: u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        item_scalar_stride: u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        common_identities: u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        item_identity_stride: u16::try_from(GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
    };
    encode_effect_program_v3_atomic(
        geometry,
        routes
            .get(..route_len)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        instruction_workspace
            .get(..fixed_count)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        instruction_workspace
            .get(fixed_count..)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        scratch,
        output,
    )
    .map_err(GeneralEffectArtifactErrorV3::Effect)?;
    ProgramV3::decode(output).map_err(GeneralEffectArtifactErrorV3::Effect)?;
    Ok(())
}

fn build_action<'a>(
    action: Action,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => Ok(0),
        Action::OpenBatch | Action::CloseBatch | Action::Consider | Action::Freeze => Ok(0),
        Action::InitializeSettlement => build_initialize(instructions, fixed, templates, routes),
        // The two compartment bytes are NOT restated here. They come from
        // `escrow_v1`, which is also what the packet builder and the artifact
        // join read -- see that module's header for the defect this retires,
        // and note that `Collect`'s row moved when it landed.
        Action::Collect => {
            build_settlement(action, 2, instructions, fixed, item, templates, routes)
        }
        Action::Materialize => {
            build_settlement(action, 1, instructions, fixed, item, templates, routes)
        }
        Action::Distribute => {
            build_settlement(action, 2, instructions, fixed, item, templates, routes)
        }
        Action::Close => build_close(instructions, fixed, templates, routes),
        Action::CancelOrder | Action::ReleaseOrder => {
            build_order_refund(action, instructions, fixed, item, templates, routes)
        }
        Action::PlaceOrder => build_place(instructions, fixed, item, templates, routes),
    }
}

#[inline(never)]
fn build_initialize<'a>(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let (position_bytes, rest) = templates.split_at_mut(PROTOCOL_POSITION_REQUEST_BYTES_V2);
    let (initialize_bytes, rest) = rest.split_at_mut(CUSTODY_REQUEST_BYTES_V1);
    let open_bytes = rest
        .get_mut(..CUSTODY_REQUEST_BYTES_V1)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    position_bytes.copy_from_slice(&position_template(ProtocolPositionActionV2::Admit)?);
    initialize_bytes.copy_from_slice(&custody_template(
        OperationV1::InitializeReplay,
        CompartmentV1::None,
        CompartmentV1::None,
    )?);
    open_bytes.copy_from_slice(&custody_template(
        OperationV1::OpenVault,
        CompartmentV1::None,
        CompartmentV1::Settlement,
    )?);
    let position_start = general_child_account_start_v3(Action::InitializeSettlement);
    let initialize_start = add_accounts(position_start, POSITION_ADMIT_ACCOUNTS)?;
    let open_start = add_accounts(initialize_start, CUSTODY_INITIALIZE_ACCOUNTS)?;
    routes[0] = route(
        FixedRole::Claims,
        RouteKindV3::Once,
        None,
        None,
        position_start,
        POSITION_ADMIT_ACCOUNTS,
        position_bytes,
        &[],
    );
    routes[1] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        None,
        initialize_start,
        CUSTODY_INITIALIZE_ACCOUNTS,
        initialize_bytes,
        &[],
    );
    routes[2] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        Some(RouteReceiptDependencyV3::new(
            FixedRole::Custody,
            1,
            u16::try_from(CUSTODY_RECEIPT_BYTES_V1)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )),
        open_start,
        CUSTODY_OPEN_ACCOUNTS,
        open_bytes,
        &[],
    );
    append_position_patches(instructions, fixed, 0, scalar::CLAIMS_MARKET_REVISION)?;
    append_custody_initialize_patches(instructions, fixed, 1, false)?;
    append_custody_initialize_patches(instructions, fixed, 2, true)?;
    Ok(3)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn build_settlement<'a>(
    action: Action,
    position_count: u32,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let (source, destination) = action_template_compartments(action)?;
    let (affine_bytes, affine_len) = affine_template(position_count)?;
    let custody_bytes = custody_template(OperationV1::Transfer, source, destination)?;
    let affine_offset = 0;
    let custody_offset = affine_len;
    let expected_templates = affine_len
        .checked_add(CUSTODY_REQUEST_BYTES_V1)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    if templates.len() != expected_templates {
        return Err(GeneralEffectArtifactErrorV3::Geometry);
    }
    copy_at(
        templates,
        affine_offset,
        affine_bytes
            .get(..affine_len)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
    )?;
    copy_at(templates, custody_offset, &custody_bytes)?;

    let prefix = general_child_account_start_v3(action);
    let affine_accounts = AFFINE_FIXED_ACCOUNTS
        .checked_add(
            u16::try_from(position_count).map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    let affine_start = prefix;
    let custody_start = add_accounts(affine_start, affine_accounts)?;
    let affine_fixed_len = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
        + usize::try_from(position_count).map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?
            * AFFINE_BATCH_POSITION_BYTES_V2;
    routes[0] = route(
        FixedRole::Claims,
        RouteKindV3::AffineOnce,
        Some(scalar_u16(scalar::CLAIMS_AFFINE_ACTIVE)?),
        None,
        affine_start,
        affine_accounts,
        slice_at(templates, affine_offset, affine_fixed_len)?,
        slice_at(
            templates,
            affine_offset + affine_fixed_len,
            AFFINE_BATCH_ROW_BYTES_V2,
        )?,
    );
    routes[1] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        Some(scalar_u16(scalar::CUSTODY_ACTIVE)?),
        None,
        custody_start,
        CUSTODY_TRANSFER_ACCOUNTS,
        slice_at(templates, custody_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    append_affine_patches(instructions, fixed, item, 0, position_count)?;
    append_custody_transfer_patches(instructions, fixed, 1, action == Action::Materialize)?;
    Ok(2)
}

#[inline(never)]
fn build_close<'a>(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let (surplus_source, surplus_destination) = action_template_compartments(Action::Close)?;
    let transfer = custody_template(OperationV1::Transfer, surplus_source, surplus_destination)?;
    let close_vault = custody_template(
        OperationV1::CloseVault,
        CompartmentV1::Settlement,
        CompartmentV1::None,
    )?;
    let close_replay = custody_template(
        OperationV1::CloseReplay,
        CompartmentV1::None,
        CompartmentV1::None,
    )?;
    let position = position_template(ProtocolPositionActionV2::Close)?;
    copy_at(templates, 0, &transfer)?;
    copy_at(templates, CUSTODY_REQUEST_BYTES_V1, &position)?;
    let close_vault_offset = CUSTODY_REQUEST_BYTES_V1 + PROTOCOL_POSITION_REQUEST_BYTES_V2;
    let close_replay_offset = close_vault_offset + CUSTODY_REQUEST_BYTES_V1;
    copy_at(templates, close_vault_offset, &close_vault)?;
    copy_at(templates, close_replay_offset, &close_replay)?;
    let transfer_start = general_child_account_start_v3(Action::Close);
    let position_start = add_accounts(transfer_start, CUSTODY_TRANSFER_ACCOUNTS)?;
    let vault_start = add_accounts(position_start, POSITION_CLOSE_ACCOUNTS)?;
    let replay_start = add_accounts(vault_start, CUSTODY_CLOSE_VAULT_ACCOUNTS)?;
    routes[0] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        Some(scalar_u16(scalar::CUSTODY_ACTIVE)?),
        None,
        transfer_start,
        CUSTODY_TRANSFER_ACCOUNTS,
        slice_at(templates, 0, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[1] = route(
        FixedRole::Claims,
        RouteKindV3::Once,
        None,
        None,
        position_start,
        POSITION_CLOSE_ACCOUNTS,
        slice_at(
            templates,
            CUSTODY_REQUEST_BYTES_V1,
            PROTOCOL_POSITION_REQUEST_BYTES_V2,
        )?,
        &[],
    );
    routes[2] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        None,
        vault_start,
        CUSTODY_CLOSE_VAULT_ACCOUNTS,
        slice_at(templates, close_vault_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[3] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        Some(RouteReceiptDependencyV3::new(
            FixedRole::Custody,
            2,
            u16::try_from(CUSTODY_RECEIPT_BYTES_V1)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )),
        replay_start,
        CUSTODY_CLOSE_REPLAY_ACCOUNTS,
        slice_at(templates, close_replay_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    append_custody_transfer_patches(instructions, fixed, 0, false)?;
    append_position_patches(instructions, fixed, 1, scalar::CLAIMS_MARKET_REVISION)?;
    append_custody_close_patches(instructions, fixed, 2, false)?;
    append_custody_close_patches(instructions, fixed, 3, true)?;
    Ok(4)
}

/// CancelOrder and ReleaseOrder: the refund and the whole escrow teardown.
///
/// Route order is the money order: the claims leg empties the escrow
/// Position, the quote leg empties the vault, and only then do the
/// Position, the vault, and the replay close -- the Claims Position close
/// refuses a nonzero vector and the vault close a nonzero balance, so an
/// omitted or short refund row fails closed rather than stranding an atom.
/// The compartments come from `escrow_v1`'s one table
/// (`ReleaseCollateral`: `Settlement(order) -> External(owner)`), and the
/// per-route request patches are the same generic appenders every settlement
/// route reads. The two actions differ only in their frame start and their
/// state writes; the teardown is one shape.
#[inline(never)]
fn build_order_refund<'a>(
    action: Action,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let (source, destination) = action_template_compartments(action)?;
    let (affine_bytes, affine_len) = affine_template(2)?;
    let transfer = custody_template(OperationV1::Transfer, source, destination)?;
    let position = position_template(ProtocolPositionActionV2::Close)?;
    let close_vault = custody_template(
        OperationV1::CloseVault,
        CompartmentV1::Settlement,
        CompartmentV1::None,
    )?;
    let close_replay = custody_template(
        OperationV1::CloseReplay,
        CompartmentV1::None,
        CompartmentV1::None,
    )?;
    let affine_offset = 0;
    let transfer_offset = affine_len;
    let position_offset = transfer_offset + CUSTODY_REQUEST_BYTES_V1;
    let close_vault_offset = position_offset + PROTOCOL_POSITION_REQUEST_BYTES_V2;
    let close_replay_offset = close_vault_offset + CUSTODY_REQUEST_BYTES_V1;
    copy_at(
        templates,
        affine_offset,
        affine_bytes
            .get(..affine_len)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
    )?;
    copy_at(templates, transfer_offset, &transfer)?;
    copy_at(templates, position_offset, &position)?;
    copy_at(templates, close_vault_offset, &close_vault)?;
    copy_at(templates, close_replay_offset, &close_replay)?;
    let affine_start = general_child_account_start_v3(action);
    let affine_accounts = add_accounts(AFFINE_FIXED_ACCOUNTS, 2)?;
    let transfer_start = add_accounts(affine_start, affine_accounts)?;
    let position_start = add_accounts(transfer_start, CUSTODY_TRANSFER_ACCOUNTS)?;
    let vault_start = add_accounts(position_start, POSITION_CLOSE_ACCOUNTS)?;
    let replay_start = add_accounts(vault_start, CUSTODY_CLOSE_VAULT_ACCOUNTS)?;
    let affine_fixed_len = AFFINE_BATCH_PLAN_HEADER_BYTES_V2 + 2 * AFFINE_BATCH_POSITION_BYTES_V2;
    routes[0] = route(
        FixedRole::Claims,
        RouteKindV3::AffineOnce,
        Some(scalar_u16(scalar::CLAIMS_AFFINE_ACTIVE)?),
        None,
        affine_start,
        affine_accounts,
        slice_at(templates, affine_offset, affine_fixed_len)?,
        slice_at(
            templates,
            affine_offset + affine_fixed_len,
            AFFINE_BATCH_ROW_BYTES_V2,
        )?,
    );
    routes[1] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        Some(scalar_u16(scalar::CUSTODY_ACTIVE)?),
        None,
        transfer_start,
        CUSTODY_TRANSFER_ACCOUNTS,
        slice_at(templates, transfer_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[2] = route(
        FixedRole::Claims,
        RouteKindV3::Once,
        None,
        None,
        position_start,
        POSITION_CLOSE_ACCOUNTS,
        slice_at(
            templates,
            position_offset,
            PROTOCOL_POSITION_REQUEST_BYTES_V2,
        )?,
        &[],
    );
    routes[3] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        None,
        vault_start,
        CUSTODY_CLOSE_VAULT_ACCOUNTS,
        slice_at(templates, close_vault_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[4] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        Some(RouteReceiptDependencyV3::new(
            FixedRole::Custody,
            3,
            u16::try_from(CUSTODY_RECEIPT_BYTES_V1)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )),
        replay_start,
        CUSTODY_CLOSE_REPLAY_ACCOUNTS,
        slice_at(templates, close_replay_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    append_affine_patches(instructions, fixed, item, 0, 2)?;
    append_custody_transfer_patches(instructions, fixed, 1, false)?;
    // The Position close FOLLOWS the affine refund, so it expects the
    // post-affine market successor -- the observation would be stale by one.
    append_position_patches(instructions, fixed, 2, scalar::CLAIMS_POST_MARKET_REVISION)?;
    append_custody_close_patches(instructions, fixed, 3, false)?;
    append_custody_close_patches(instructions, fixed, 4, true)?;
    Ok(5)
}

/// PlaceOrder: the admission and the whole escrow construction.
///
/// Route order is the money order: the replay and the vault exist before
/// anything can arrive, the escrow Position is admitted before claims can
/// move into it, the claims escrow-in draws on the maker's Position, and the
/// quote deposit lands last -- guarded by a register the TRANSITION pins to
/// exactly nonzero-reserve, so a runtime bank can neither skip a deposit the
/// batch committed nor attempt a zero transfer for a pure-claims order. The
/// compartments come from `escrow_v1`'s one table (`EscrowCollateral`:
/// `External(owner) -> Settlement(order)`).
#[inline(never)]
fn build_place<'a>(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let (source, destination) = action_template_compartments(Action::PlaceOrder)?;
    let (affine_bytes, affine_len) = affine_template(2)?;
    let initialize = custody_template(
        OperationV1::InitializeReplay,
        CompartmentV1::None,
        CompartmentV1::None,
    )?;
    let open = custody_template(
        OperationV1::OpenVault,
        CompartmentV1::None,
        CompartmentV1::Settlement,
    )?;
    let transfer = custody_template(OperationV1::Transfer, source, destination)?;
    let position = position_template(ProtocolPositionActionV2::Admit)?;
    let initialize_offset = 0;
    let open_offset = CUSTODY_REQUEST_BYTES_V1;
    let position_offset = open_offset + CUSTODY_REQUEST_BYTES_V1;
    let affine_offset = position_offset + PROTOCOL_POSITION_REQUEST_BYTES_V2;
    let transfer_offset = affine_offset + affine_len;
    copy_at(templates, initialize_offset, &initialize)?;
    copy_at(templates, open_offset, &open)?;
    copy_at(templates, position_offset, &position)?;
    copy_at(
        templates,
        affine_offset,
        affine_bytes
            .get(..affine_len)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
    )?;
    copy_at(templates, transfer_offset, &transfer)?;
    let initialize_start = general_child_account_start_v3(Action::PlaceOrder);
    let open_start = add_accounts(initialize_start, CUSTODY_INITIALIZE_ACCOUNTS)?;
    let position_start = add_accounts(open_start, CUSTODY_OPEN_ACCOUNTS)?;
    let affine_start = add_accounts(position_start, POSITION_ADMIT_ACCOUNTS)?;
    let affine_accounts = add_accounts(AFFINE_FIXED_ACCOUNTS, 2)?;
    let transfer_start = add_accounts(affine_start, affine_accounts)?;
    let affine_fixed_len = AFFINE_BATCH_PLAN_HEADER_BYTES_V2 + 2 * AFFINE_BATCH_POSITION_BYTES_V2;
    routes[0] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        None,
        initialize_start,
        CUSTODY_INITIALIZE_ACCOUNTS,
        slice_at(templates, initialize_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[1] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        None,
        Some(RouteReceiptDependencyV3::new(
            FixedRole::Custody,
            0,
            u16::try_from(CUSTODY_RECEIPT_BYTES_V1)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )),
        open_start,
        CUSTODY_OPEN_ACCOUNTS,
        slice_at(templates, open_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    routes[2] = route(
        FixedRole::Claims,
        RouteKindV3::Once,
        None,
        None,
        position_start,
        POSITION_ADMIT_ACCOUNTS,
        slice_at(
            templates,
            position_offset,
            PROTOCOL_POSITION_REQUEST_BYTES_V2,
        )?,
        &[],
    );
    routes[3] = route(
        FixedRole::Claims,
        RouteKindV3::AffineOnce,
        None,
        None,
        affine_start,
        affine_accounts,
        slice_at(templates, affine_offset, affine_fixed_len)?,
        slice_at(
            templates,
            affine_offset + affine_fixed_len,
            AFFINE_BATCH_ROW_BYTES_V2,
        )?,
    );
    routes[4] = route(
        FixedRole::Custody,
        RouteKindV3::Once,
        Some(scalar_u16(scalar::CUSTODY_ACTIVE)?),
        None,
        transfer_start,
        CUSTODY_TRANSFER_ACCOUNTS,
        slice_at(templates, transfer_offset, CUSTODY_REQUEST_BYTES_V1)?,
        &[],
    );
    append_custody_initialize_patches(instructions, fixed, 0, false)?;
    append_custody_initialize_patches(instructions, fixed, 1, true)?;
    // A Position admit advances no market revision, so both the admit and the
    // affine that follows it expect the same observation.
    append_position_patches(instructions, fixed, 2, scalar::CLAIMS_MARKET_REVISION)?;
    append_affine_patches(instructions, fixed, item, 3, 2)?;
    append_custody_transfer_patches(instructions, fixed, 4, false)?;
    Ok(5)
}

#[inline(never)]
fn append_general_state_patches(
    action: Action,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
) -> Result<()> {
    if action == Action::CloseCandidate {
        let candidate = AccountCoordinateV3::fixed(GENERAL_STATE_ACCOUNT_COORDINATE_V3);
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::transfer_lamports(
                candidate,
                AccountCoordinateV3::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
                scalar_common(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
            ),
        )?;
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::transfer_lamports(
                candidate,
                AccountCoordinateV3::fixed(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
                scalar_common(scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION)?,
            ),
        )?;
        return push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::require_lamports_eq(
                candidate,
                scalar_common(scalar::PRIMARY_RENT_PRINCIPAL)?,
            ),
        );
    }
    if action == Action::VerifyCandidateRow {
        return append_verify_candidate_patches(instructions, fixed, item);
    }
    if action == Action::SubmitCandidate {
        let candidate = AccountCoordinateV3::fixed(GENERAL_STATE_ACCOUNT_COORDINATE_V3);
        append_local_state_header(instructions, fixed, candidate, false)?;
        for (offset, coordinate) in [
            (
                GeneralCandidateLayoutV1::MAGIC_OFFSET,
                scalar::VERIFY_REVISION_OBSERVATION,
            ),
            (
                GeneralCandidateLayoutV1::SUBMITTED_SLOT_OFFSET,
                scalar::CANDIDATE_SUBMITTED_SLOT,
            ),
            (
                GeneralCandidateLayoutV1::PAGE_REVISION_OFFSET,
                scalar::CANDIDATE_PAGE_REVISION,
            ),
            (
                GeneralCandidateLayoutV1::REWARD_RATE_OFFSET,
                scalar::CANDIDATE_REWARD_RATE,
            ),
            (
                GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET,
                scalar::CANDIDATE_POST_VERIFICATION_REMAINING,
            ),
            (
                GeneralCandidateLayoutV1::CLEANUP_REMAINING_OFFSET,
                scalar::CANDIDATE_POST_CLEANUP_REMAINING,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u64(
                    candidate,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u16(
                candidate,
                state_body_offset(offset_u32(GeneralCandidateLayoutV1::VERSION_OFFSET)?)?,
                scalar_common(scalar::ONE)?,
            ),
        )?;
        for (offset, coordinate) in [
            (
                GeneralCandidateLayoutV1::PHASE_OFFSET,
                scalar::VERIFY_POST_REVISION,
            ),
            (
                GeneralCandidateLayoutV1::STATUS_OFFSET,
                scalar::CANDIDATE_POST_STATUS,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u8(
                    candidate,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        for (offset, coordinate) in [
            (
                GeneralCandidateLayoutV1::OUTCOME_COUNT_OFFSET,
                scalar::OUTCOME_COUNT,
            ),
            (
                GeneralCandidateLayoutV1::PAGE_COUNT_OFFSET,
                scalar::CANDIDATE_PAGE_COUNT,
            ),
            (
                GeneralCandidateLayoutV1::ROW_COUNT_OFFSET,
                scalar::CANDIDATE_ROW_COUNT,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u32(
                    candidate,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        for (offset, coordinate) in [
            (
                GeneralCandidateLayoutV1::CANDIDATE_ID_OFFSET,
                identity::CANDIDATE,
            ),
            (
                GeneralCandidateLayoutV1::BATCH_ID_OFFSET,
                identity::SELECTION_BATCH,
            ),
            (GeneralCandidateLayoutV1::SOLVER_ID_OFFSET, identity::OWNER),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_identity(
                    candidate,
                    state_body_offset(offset_u32(offset)?)?,
                    identity_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::transfer_lamports(
                AccountCoordinateV3::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
                candidate,
                scalar_common(scalar::SCRATCH_A)?,
            ),
        )?;
        return push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::require_lamports_eq(candidate, scalar_common(scalar::SCRATCH_B)?),
        );
    }
    if matches!(action, Action::OpenBatch | Action::CloseBatch) {
        return append_batch_action_patches(action, instructions, fixed);
    }
    if action == Action::PlaceOrder {
        // The whole order record, written into the vacant secondary account
        // from registers the profile filled out of the SIGNED TERMS and the
        // authenticated environment -- the record is canonical by
        // construction, and the fixed mutable window plus interleaved rows
        // are exactly what the order-wire repair bought.
        let order = AccountCoordinateV3::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3);
        append_local_state_header(instructions, fixed, order, true)?;
        for (offset, coordinate) in [
            (GeneralOrderLayoutV1::MAGIC, scalar::SCRATCH_A),
            (GeneralOrderLayoutV1::NONCE, scalar::ORDER_NONCE),
            (GeneralOrderLayoutV1::GENERATION, scalar::GENERATION),
            (GeneralOrderLayoutV1::MAX_LOTS, scalar::ORDER_MAX_LOTS),
            (
                GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT,
                scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT,
            ),
            (
                GeneralOrderLayoutV1::VALID_UNTIL_SLOT,
                scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
            ),
            (
                GeneralOrderLayoutV1::STATE_ADMITTED_SLOT,
                scalar::CURRENT_SLOT,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u64(
                    order,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u16(
                order,
                state_body_offset(offset_u32(GeneralOrderLayoutV1::VERSION)?)?,
                scalar_common(scalar::ONE)?,
            ),
        )?;
        for (offset, coordinate) in [
            (GeneralOrderLayoutV1::PHASE, scalar::SCRATCH_B),
            (GeneralOrderLayoutV1::STATE_PHASE, scalar::ORDER_POST_PHASE),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u8(
                    order,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                order,
                state_body_offset(offset_u32(GeneralOrderLayoutV1::OUTCOME_COUNT)?)?,
                scalar_common(scalar::OUTCOME_COUNT)?,
            ),
        )?;
        for (offset, coordinate) in [
            (GeneralOrderLayoutV1::OWNER_ID, identity::OWNER),
            (GeneralOrderLayoutV1::MARKET, identity::MARKET),
            (GeneralOrderLayoutV1::BATCH_ID, identity::SELECTION_BATCH),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_identity(
                    order,
                    state_body_offset(offset_u32(offset)?)?,
                    identity_common(coordinate)?,
                ),
            )?;
        }
        // The per-outcome rows, one affine write per interleaved field: the
        // receive and deliver quantities the profile projected out of the
        // signed terms image.
        push_item(
            instructions,
            item,
            EffectInstructionV3::write_u64_affine(
                order,
                state_body_offset(offset_u32(
                    GENERAL_ORDER_ROW_BASE_V1 + GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1,
                )?)?,
                offset_u32(GENERAL_ORDER_ROW_STRIDE_V1)?,
                ScalarCoordinateV3::item(scalar_u16(item_scalar::CURSOR_INVENTORY)?),
            ),
        )?;
        push_item(
            instructions,
            item,
            EffectInstructionV3::write_u64_affine(
                order,
                state_body_offset(offset_u32(
                    GENERAL_ORDER_ROW_BASE_V1 + GENERAL_ORDER_ROW_DELIVER_OFFSET_V1,
                )?)?,
                offset_u32(GENERAL_ORDER_ROW_STRIDE_V1)?,
                ScalarCoordinateV3::item(scalar_u16(item_scalar::QUANTITY)?),
            ),
        )?;
        // The batch surrenders nothing and commits everything: one more
        // admission, and exactly the worst case this order escrowed.
        let batch = AccountCoordinateV3::fixed(GENERAL_STATE_ACCOUNT_COORDINATE_V3);
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                batch,
                state_body_offset(offset_u32(GeneralBatchLayoutV1::ORDER_COUNT)?)?,
                scalar_common(scalar::BATCH_POST_ORDER_COUNT)?,
            ),
        )?;
        return push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                batch,
                state_body_offset(offset_u32(GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE)?)?,
                scalar_common(scalar::BATCH_POST_QUOTE_RESERVE)?,
            ),
        );
    }
    if matches!(action, Action::CancelOrder | Action::ReleaseOrder) {
        // The order record's whole successor: the phase byte and the released
        // slot, both inside the fixed mutable window the order-wire repair
        // created. Every other byte of the record persists physically -- the
        // tombstone is the replay guard, and its identity bytes never move.
        // For Cancel the order is the SECONDARY state; for Release, the
        // primary.
        let order = AccountCoordinateV3::fixed(if action == Action::CancelOrder {
            GENERAL_TERMINAL_STATE_ACCOUNT_V3
        } else {
            GENERAL_STATE_ACCOUNT_COORDINATE_V3
        });
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u8(
                order,
                state_body_offset(offset_u32(GeneralOrderLayoutV1::STATE_PHASE)?)?,
                scalar_common(scalar::ORDER_POST_PHASE)?,
            ),
        )?;
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                order,
                state_body_offset(offset_u32(GeneralOrderLayoutV1::STATE_RELEASED_SLOT)?)?,
                scalar_common(scalar::ORDER_POST_RELEASED_SLOT)?,
            ),
        )?;
        if action == Action::ReleaseOrder {
            return Ok(());
        }
        // Cancel alone moves the batch counters: one more cancellation, and
        // the committed reserve surrenders exactly what admission committed.
        let batch = AccountCoordinateV3::fixed(GENERAL_STATE_ACCOUNT_COORDINATE_V3);
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                batch,
                state_body_offset(offset_u32(GeneralBatchLayoutV1::CANCELLED_COUNT)?)?,
                scalar_common(scalar::BATCH_POST_CANCELLED_COUNT)?,
            ),
        )?;
        return push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                batch,
                state_body_offset(offset_u32(GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE)?)?,
                scalar_common(scalar::BATCH_POST_QUOTE_RESERVE)?,
            ),
        );
    }
    let terminal = action == Action::Close;
    let state = AccountCoordinateV3::fixed(if terminal {
        GENERAL_TERMINAL_STATE_ACCOUNT_V3
    } else {
        GENERAL_STATE_ACCOUNT_COORDINATE_V3
    });
    if matches!(
        action,
        Action::Consider | Action::InitializeSettlement | Action::Close
    ) {
        append_local_state_header(instructions, fixed, state, terminal)?;
    }
    if matches!(action, Action::Consider | Action::Freeze) {
        for (offset, coordinate) in [
            (RuntimeSelectionLayoutV2::magic(), scalar::SELECTION_MAGIC),
            (
                RuntimeSelectionLayoutV2::revision(),
                scalar::SELECTION_REVISION,
            ),
            (
                RuntimeSelectionLayoutV2::best_verified_revision(),
                scalar::SELECTION_BEST_VERIFIED_REVISION,
            ),
            (
                RuntimeSelectionLayoutV2::price_scale(),
                scalar::SELECTION_PRICE_SCALE,
            ),
            (
                RuntimeSelectionLayoutV2::best_filled_lots(),
                scalar::SELECTION_BEST_FILLED_LOTS,
            ),
            (
                RuntimeSelectionLayoutV2::best_quote_surplus(),
                scalar::SELECTION_BEST_QUOTE_SURPLUS,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u64(
                    state,
                    state_body_offset(offset)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u16(
                state,
                state_body_offset(RuntimeSelectionLayoutV2::version())?,
                scalar_common(scalar::RUNTIME_WIDTH_VERSION)?,
            ),
        )?;
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u8(
                state,
                state_body_offset(RuntimeSelectionLayoutV2::phase())?,
                scalar_common(scalar::SELECTION_PHASE)?,
            ),
        )?;
        for (offset, coordinate) in [
            (
                RuntimeSelectionLayoutV2::outcome_count(),
                scalar::OUTCOME_COUNT,
            ),
            (
                RuntimeSelectionLayoutV2::submitted_count(),
                scalar::SELECTION_SUBMITTED_COUNT,
            ),
            (
                RuntimeSelectionLayoutV2::best_candidate_coordinate(),
                scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u32(
                    state,
                    state_body_offset(offset)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        for (offset, coordinate) in [
            (
                RuntimeSelectionLayoutV2::product_id(),
                identity::SELECTION_PRODUCT,
            ),
            (
                RuntimeSelectionLayoutV2::batch_id(),
                identity::SELECTION_BATCH,
            ),
            (
                RuntimeSelectionLayoutV2::policy_id(),
                identity::SELECTION_POLICY,
            ),
            (
                RuntimeSelectionLayoutV2::best_candidate_id(),
                identity::CANDIDATE,
            ),
            (
                RuntimeSelectionLayoutV2::best_verified_digest(),
                identity::BEST_VERIFIED_DIGEST,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_identity(
                    state,
                    state_body_offset(offset)?,
                    identity_common(coordinate)?,
                ),
            )?;
        }
        return Ok(());
    }

    for (offset, coordinate) in [
        (SettlementCursorLayoutV2::magic(), scalar::CURSOR_MAGIC),
        (
            SettlementCursorLayoutV2::revision(),
            scalar::CURSOR_RESULTING_REVISION,
        ),
        (
            SettlementCursorLayoutV2::quote_inventory(),
            scalar::CURSOR_QUOTE_INVENTORY,
        ),
        (
            SettlementCursorLayoutV2::complete_set_quantity(),
            scalar::CURSOR_COMPLETE_SET_QUANTITY,
        ),
        (
            SettlementCursorLayoutV2::terminal_coordinate(),
            scalar::CURSOR_TERMINAL_COORDINATE,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                state,
                state_body_offset(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u16(
            state,
            state_body_offset(SettlementCursorLayoutV2::version())?,
            scalar_common(scalar::RUNTIME_WIDTH_VERSION)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            state,
            state_body_offset(SettlementCursorLayoutV2::phase())?,
            scalar_common(scalar::CURSOR_PHASE)?,
        ),
    )?;
    for (offset, coordinate) in [
        (
            SettlementCursorLayoutV2::outcome_count(),
            scalar::OUTCOME_COUNT,
        ),
        (
            SettlementCursorLayoutV2::order_count(),
            scalar::CURSOR_ORDER_COUNT,
        ),
        (
            SettlementCursorLayoutV2::next_order(),
            scalar::CURSOR_NEXT_ORDER,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                state,
                state_body_offset(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_identity(
            state,
            state_body_offset(SettlementCursorLayoutV2::candidate_id())?,
            identity_common(identity::CANDIDATE)?,
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_affine(
            state,
            state_body_offset(SettlementCursorLayoutV2::inventory_base())?,
            SettlementCursorLayoutV2::inventory_stride(),
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CURSOR_INVENTORY)?),
        ),
    )
}

/// Persist one authenticated verifier step exactly once.
///
/// Candidate and Verifier always advance. The raw VerifiedCandidate result is
/// a terminal-only state: every one of its possible writes is statically
/// declared but resolves to `Noop` before touching the vacant result account
/// when `VERIFY_TERMINAL == 0`.
fn append_verify_candidate_patches(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
) -> Result<()> {
    let candidate = AccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3);
    let verifier = AccountCoordinateV3::fixed(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3);
    let result = AccountCoordinateV3::fixed(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3);
    let payer = AccountCoordinateV3::fixed(GENERAL_VERIFY_PAYER_ACCOUNT_V3);
    let terminal = scalar_u16(scalar::VERIFY_TERMINAL)?;

    // The submitted Candidate's complete mutable successor. The solver's
    // immutable opening remains byte-identical; terminal verification records
    // the exact certificate digest and revision produced in this same step.
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            candidate,
            state_body_offset(offset_u32(GeneralCandidateLayoutV1::STATUS_OFFSET)?)?,
            scalar_common(scalar::CANDIDATE_POST_STATUS)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_identity(
            candidate,
            state_body_offset(offset_u32(
                GeneralCandidateLayoutV1::VERIFIED_DIGEST_OFFSET,
            )?)?,
            identity_common(identity::BEST_VERIFIED_DIGEST)?,
        ),
    )?;
    for (offset, coordinate) in [
        (
            GeneralCandidateLayoutV1::VERIFIED_REVISION_OFFSET,
            scalar::SELECTION_BEST_VERIFIED_REVISION,
        ),
        (
            GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET,
            scalar::CANDIDATE_POST_VERIFICATION_REMAINING,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                candidate,
                state_body_offset(offset_u32(offset)?)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::transfer_lamports(candidate, payer, scalar_common(scalar::SCRATCH_A)?),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::require_lamports_eq(candidate, scalar_common(scalar::SCRATCH_B)?),
    )?;

    // Verifier is a resumable local envelope. Its body is the canonical
    // RuntimeCandidateVerifierV2 wire, not an adapter-specific projection.
    append_local_state_header(instructions, fixed, verifier, true)?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            verifier,
            state_body_offset(RuntimeVerifierLayoutV2::magic())?,
            scalar_common(scalar::CURSOR_MAGIC)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u16(
            verifier,
            state_body_offset(RuntimeVerifierLayoutV2::version())?,
            scalar_common(scalar::RUNTIME_WIDTH_VERSION)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            verifier,
            state_body_offset(RuntimeVerifierLayoutV2::has_current_order())?,
            scalar_common(scalar::CUSTODY_ACTIVE)?,
        ),
    )?;
    for (offset, coordinate) in [
        (
            RuntimeVerifierLayoutV2::outcome_count(),
            scalar::OUTCOME_COUNT,
        ),
        (
            RuntimeVerifierLayoutV2::page_count(),
            scalar::CANDIDATE_PAGE_COUNT,
        ),
        (
            RuntimeVerifierLayoutV2::next_page_index(),
            scalar::VERIFY_POST_PAGE,
        ),
        (
            RuntimeVerifierLayoutV2::next_row_index(),
            scalar::VERIFY_POST_ROW,
        ),
        (
            RuntimeVerifierLayoutV2::order_count(),
            scalar::VERIFY_POST_ORDER_COUNT,
        ),
        (
            RuntimeVerifierLayoutV2::candidate_coordinate(),
            scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                verifier,
                state_body_offset(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            verifier,
            state_body_offset(RuntimeVerifierLayoutV2::revision())?,
            scalar_common(scalar::VERIFY_POST_REVISION)?,
        ),
    )?;
    for (offset, coordinate) in [
        (RuntimeVerifierLayoutV2::candidate_id(), identity::CANDIDATE),
        (
            RuntimeVerifierLayoutV2::product_id(),
            identity::SELECTION_PRODUCT,
        ),
        (
            RuntimeVerifierLayoutV2::batch_id(),
            identity::SELECTION_BATCH,
        ),
        (RuntimeVerifierLayoutV2::current_order_id(), identity::ORDER),
        (RuntimeVerifierLayoutV2::current_owner_id(), identity::OWNER),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_identity(
                verifier,
                state_body_offset(offset)?,
                identity_common(coordinate)?,
            ),
        )?;
    }
    for (offset, coordinate) in [
        (
            RuntimeVerifierLayoutV2::price_scale(),
            scalar::SELECTION_PRICE_SCALE,
        ),
        (
            RuntimeVerifierLayoutV2::filled_lots(),
            scalar::SELECTION_BEST_FILLED_LOTS,
        ),
        (
            RuntimeVerifierLayoutV2::quote_debit(),
            scalar::ORDER_QUOTE_RESERVE,
        ),
        (
            RuntimeVerifierLayoutV2::quote_credit(),
            scalar::QUOTE_QUANTITY,
        ),
        (
            RuntimeVerifierLayoutV2::current_nonce(),
            scalar::ORDER_NONCE,
        ),
        (
            RuntimeVerifierLayoutV2::current_max_lots(),
            scalar::ORDER_MAX_LOTS,
        ),
        (
            RuntimeVerifierLayoutV2::current_max_quote_debit_per_lot(),
            scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT,
        ),
        (
            RuntimeVerifierLayoutV2::current_lots(),
            scalar::ORDER_VALID_UNTIL_SLOT,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                verifier,
                state_body_offset(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    for (offset, coordinate) in [
        (
            RuntimeVerifierLayoutV2::current_source_page_index(),
            scalar::PAGE_INDEX,
        ),
        (
            RuntimeVerifierLayoutV2::current_source_execution_index(),
            scalar::EXECUTION_INDEX,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32(
                verifier,
                state_body_offset(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }

    let verifier_tails = state_body_offset(RuntimeVerifierLayoutV2::tails_base())?;
    let tail_stride = RuntimeVerifierLayoutV2::tail_item_stride();
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_affine(
            verifier,
            verifier_tails,
            tail_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::QUANTITY)?),
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_second_tail_affine(
            verifier,
            verifier_tails,
            tail_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CURSOR_INVENTORY)?),
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_third_tail_affine(
            verifier,
            verifier_tails,
            tail_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CLAIMS_AGGREGATE_MAGNITUDE)?),
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_fourth_tail_affine(
            verifier,
            verifier_tails,
            tail_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CLAIMS_SOURCE_MAGNITUDE)?),
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_fifth_tail_affine(
            verifier,
            verifier_tails,
            tail_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CLAIMS_DESTINATION_MAGNITUDE)?),
        ),
    )?;

    // Result is raw VerifiedCandidateV2, created and written only on the
    // terminal row. Static validation still sees all possible ranges.
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64_if_nonzero(
            result,
            VerifiedCandidateLayoutV2::magic(),
            scalar_common(scalar::SELECTION_MAGIC)?,
            terminal,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u16_if_nonzero(
            result,
            VerifiedCandidateLayoutV2::version(),
            scalar_common(scalar::RUNTIME_WIDTH_VERSION)?,
            terminal,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8_if_nonzero(
            result,
            VerifiedCandidateLayoutV2::phase(),
            scalar_common(scalar::SELECTION_PHASE)?,
            terminal,
        ),
    )?;
    for (offset, coordinate) in [
        (
            VerifiedCandidateLayoutV2::outcome_count(),
            scalar::OUTCOME_COUNT,
        ),
        (
            VerifiedCandidateLayoutV2::page_count(),
            scalar::CANDIDATE_PAGE_COUNT,
        ),
        (
            VerifiedCandidateLayoutV2::candidate_coordinate(),
            scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u32_if_nonzero(
                result,
                offset,
                scalar_common(coordinate)?,
                terminal,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64_if_nonzero(
            result,
            VerifiedCandidateLayoutV2::revision(),
            scalar_common(scalar::VERIFY_POST_REVISION)?,
            terminal,
        ),
    )?;
    for (offset, coordinate) in [
        (
            VerifiedCandidateLayoutV2::candidate_id(),
            identity::CANDIDATE,
        ),
        (
            VerifiedCandidateLayoutV2::product_id(),
            identity::SELECTION_PRODUCT,
        ),
        (
            VerifiedCandidateLayoutV2::batch_id(),
            identity::SELECTION_BATCH,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_identity_if_nonzero(
                result,
                offset,
                identity_common(coordinate)?,
                terminal,
            ),
        )?;
    }
    for (offset, coordinate) in [
        (
            VerifiedCandidateLayoutV2::filled_lots(),
            scalar::SELECTION_BEST_FILLED_LOTS,
        ),
        (
            VerifiedCandidateLayoutV2::quote_debit(),
            scalar::ORDER_QUOTE_RESERVE,
        ),
        (
            VerifiedCandidateLayoutV2::quote_credit(),
            scalar::QUOTE_QUANTITY,
        ),
        (
            VerifiedCandidateLayoutV2::price_scale(),
            scalar::SELECTION_PRICE_SCALE,
        ),
    ] {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64_if_nonzero(
                result,
                offset,
                scalar_common(coordinate)?,
                terminal,
            ),
        )?;
    }
    let result_tails = VerifiedCandidateLayoutV2::claim_inputs_base();
    let result_stride = VerifiedCandidateLayoutV2::tail_item_stride();
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_affine_if_nonzero(
            result,
            result_tails,
            result_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CLAIMS_SOURCE_MAGNITUDE)?),
            terminal,
        ),
    )?;
    push_item(
        instructions,
        item,
        EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
            result,
            result_tails,
            result_stride,
            ScalarCoordinateV3::item(scalar_u16(item_scalar::CLAIMS_DESTINATION_MAGNITUDE)?),
            terminal,
        ),
    )?;

    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::require_lamports_eq(
            verifier,
            scalar_common(scalar::TERMINAL_RENT_PRINCIPAL)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::require_lamports_eq(
            result,
            scalar_common(scalar::RESULT_RENT_PRINCIPAL)?,
        ),
    )
}

/// The root-writing pair's complete state writes: the exact `GeneralRootV2`
/// tail successor behind the immutable capability header, and the batch
/// record.
///
/// The root offsets are past `CAPABILITY_ROOT_HEADER_BYTES_V1`, which is
/// precisely the boundary Trading's `require_root_write_is_state_only` guards
/// writes by; the AccountProfile grants coordinate 0 the data-effect
/// permission for exactly these two actions and no others.
#[inline(never)]
fn append_batch_action_patches(
    action: Action,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
) -> Result<()> {
    let root = AccountCoordinateV3::fixed(
        u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
            .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
    );
    let state = AccountCoordinateV3::fixed(GENERAL_STATE_ACCOUNT_COORDINATE_V3);
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            root,
            root_tail_offset(GENERAL_ROOT_REVISION_OFFSET_V2)?,
            scalar_common(scalar::ROOT_POST_REVISION)?,
        ),
    )?;
    if action == Action::OpenBatch {
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u64(
                root,
                root_tail_offset(GENERAL_ROOT_NEXT_BATCH_SEQUENCE_OFFSET_V2)?,
                scalar_common(scalar::ROOT_POST_BATCH_SEQUENCE)?,
            ),
        )?;
    }
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            root,
            root_tail_offset(GENERAL_ROOT_OPEN_BATCHES_OFFSET_V2)?,
            scalar_common(scalar::ROOT_POST_OPEN_BATCHES)?,
        ),
    )?;
    if action == Action::OpenBatch {
        append_local_state_header(instructions, fixed, state, false)?;
        for (offset, coordinate) in [
            (GeneralBatchLayoutV1::MAGIC, scalar::SCRATCH_A),
            (
                GeneralBatchLayoutV1::SEQUENCE,
                scalar::ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION,
            ),
            (GeneralBatchLayoutV1::GENERATION, scalar::GENERATION),
            (
                GeneralBatchLayoutV1::PRICE_SCALE,
                scalar::SELECTION_PRICE_SCALE,
            ),
            (
                GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT,
                scalar::BATCH_COLLECTION_CLOSE_SLOT,
            ),
            (
                GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT,
                scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
            ),
            (
                GeneralBatchLayoutV1::OPENED_ROOT_REVISION,
                scalar::ROOT_REVISION_OBSERVATION,
            ),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u64(
                    state,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        push_fixed(
            instructions,
            fixed,
            EffectInstructionV3::write_u16(
                state,
                state_body_offset(offset_u32(GeneralBatchLayoutV1::VERSION)?)?,
                scalar_common(scalar::ONE)?,
            ),
        )?;
        for (offset, coordinate) in [
            (GeneralBatchLayoutV1::PHASE, scalar::SCRATCH_B),
            (GeneralBatchLayoutV1::STATUS, scalar::BATCH_POST_STATUS),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u8(
                    state,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        for (offset, coordinate) in [
            (GeneralBatchLayoutV1::OUTCOME_COUNT, scalar::OUTCOME_COUNT),
            (GeneralBatchLayoutV1::MAX_ORDERS, scalar::CONFIG_MAX_ORDERS),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_u32(
                    state,
                    state_body_offset(offset_u32(offset)?)?,
                    scalar_common(coordinate)?,
                ),
            )?;
        }
        for (offset, coordinate) in [
            (GeneralBatchLayoutV1::MARKET, identity::MARKET),
            (
                GeneralBatchLayoutV1::PRODUCT_ID,
                identity::SELECTION_PRODUCT,
            ),
            (GeneralBatchLayoutV1::CONFIG_ID, identity::GENERAL_CONFIG_ID),
        ] {
            push_fixed(
                instructions,
                fixed,
                EffectInstructionV3::write_identity(
                    state,
                    state_body_offset(offset_u32(offset)?)?,
                    identity_common(coordinate)?,
                ),
            )?;
        }
        return Ok(());
    }
    // CloseBatch: the record already exists; only its status and the root
    // revision that closed it move. Every other byte persists physically.
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            state,
            state_body_offset(offset_u32(GeneralBatchLayoutV1::STATUS)?)?,
            scalar_common(scalar::BATCH_POST_STATUS)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            state,
            state_body_offset(offset_u32(GeneralBatchLayoutV1::CLOSED_ROOT_REVISION)?)?,
            scalar_common(scalar::ROOT_POST_REVISION)?,
        ),
    )
}

/// One `GeneralRootV2` tail offset behind the immutable capability header.
fn root_tail_offset(offset: usize) -> Result<u32> {
    offset_u32(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(offset)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
    )
}

fn append_local_state_header(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    state: AccountCoordinateV3,
    terminal: bool,
) -> Result<()> {
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            state,
            GeneralLocalStateLayoutV3::magic(),
            scalar_common(scalar::LOCAL_STATE_MAGIC)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u16(
            state,
            GeneralLocalStateLayoutV3::version(),
            scalar_common(scalar::LOCAL_STATE_VERSION)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            state,
            GeneralLocalStateLayoutV3::kind(),
            scalar_common(scalar::LOCAL_STATE_KIND)?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u8(
            state,
            GeneralLocalStateLayoutV3::bump(),
            scalar_common(if terminal {
                scalar::TERMINAL_CANONICAL_BUMP
            } else {
                scalar::PRIMARY_CANONICAL_BUMP
            })?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_u64(
            state,
            GeneralLocalStateLayoutV3::rent_principal(),
            scalar_common(if terminal {
                scalar::TERMINAL_RENT_PRINCIPAL
            } else {
                scalar::PRIMARY_RENT_PRINCIPAL
            })?,
        ),
    )?;
    push_fixed(
        instructions,
        fixed,
        EffectInstructionV3::write_identity(
            state,
            GeneralLocalStateLayoutV3::beneficiary(),
            identity_common(if terminal {
                identity::TERMINAL_BENEFICIARY
            } else {
                identity::PRIMARY_BENEFICIARY
            })?,
        ),
    )
}

fn state_body_offset(offset: u32) -> Result<u32> {
    GeneralLocalStateLayoutV3::body()
        .checked_add(offset)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

/// `market_revision` is the register carrying the Claims market revision this
/// Position operation EXPECTS. An affine transfer advances the market by one,
/// so a Position close that FOLLOWS an affine (CancelOrder, ReleaseOrder)
/// expects the post-affine successor, while an operation with no affine before
/// it (Initialize's admit, Close's close, PlaceOrder's admit -- an admit
/// itself advances nothing) expects the observation.
fn append_position_patches(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    market_revision: u32,
) -> Result<()> {
    for (offset, coordinate) in [
        (
            ProtocolPositionRequestLayoutV2::RELEASE_SET,
            identity::RELEASE_SET,
        ),
        (ProtocolPositionRequestLayoutV2::MARKET, identity::MARKET),
        (
            ProtocolPositionRequestLayoutV2::POSITION_OWNER,
            identity::SETTLEMENT_POSITION_OWNER,
        ),
        (
            ProtocolPositionRequestLayoutV2::PARENT_REQUEST_DIGEST,
            identity::PARENT_REQUEST_DIGEST,
        ),
        (
            ProtocolPositionRequestLayoutV2::RENT_CREDIT,
            identity::RENT_CREDIT,
        ),
        (
            ProtocolPositionRequestLayoutV2::RENT_PROGRAM,
            identity::RENT_PROGRAM,
        ),
    ] {
        push_fixed(
            output,
            cursor,
            EffectInstructionV3::write_request_identity(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(offset)?,
                identity_common(coordinate)?,
            ),
        )?;
    }
    for (offset, coordinate) in [
        (
            ProtocolPositionRequestLayoutV2::GENERATION,
            scalar::GENERATION,
        ),
        (
            ProtocolPositionRequestLayoutV2::EXPECTED_MARKET_REVISION,
            market_revision,
        ),
        (
            ProtocolPositionRequestLayoutV2::EXPECTED_POSITION_REVISION,
            scalar::SETTLEMENT_POSITION_REVISION,
        ),
        (
            ProtocolPositionRequestLayoutV2::OBSERVED_POSITION_LAMPORTS,
            scalar::OBSERVED_POSITION_LAMPORTS,
        ),
        (
            ProtocolPositionRequestLayoutV2::OBSERVED_ADMISSION_LAMPORTS,
            scalar::OBSERVED_ADMISSION_LAMPORTS,
        ),
        (
            ProtocolPositionRequestLayoutV2::POSITION_RENT_PRINCIPAL,
            scalar::POSITION_RENT_PRINCIPAL,
        ),
        (
            ProtocolPositionRequestLayoutV2::ADMISSION_RENT_PRINCIPAL,
            scalar::ADMISSION_RENT_PRINCIPAL,
        ),
    ] {
        push_fixed(
            output,
            cursor,
            EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(offset)?,
                scalar_common(coordinate)?,
            ),
        )?;
    }
    Ok(())
}

fn append_affine_patches(
    output: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    route: u16,
    position_count: u32,
) -> Result<()> {
    for (offset, coordinate) in [
        (
            AffineBatchRequestLayoutV2::RELEASE_SET,
            identity::RELEASE_SET,
        ),
        (AffineBatchRequestLayoutV2::MARKET, identity::MARKET),
        (
            AffineBatchRequestLayoutV2::REQUEST_DIGEST,
            identity::PARENT_REQUEST_DIGEST,
        ),
        (
            AffineBatchRequestLayoutV2::PRODUCT_RECORD,
            identity::PRODUCT_RECORD_DIGEST,
        ),
        (
            AffineBatchRequestLayoutV2::SEMANTIC_BASIS,
            identity::SEMANTIC_BASIS_ID,
        ),
        (
            AffineBatchRequestLayoutV2::LINKED_BASIS_RECORD,
            identity::LINKED_BASIS_RECORD_DIGEST,
        ),
    ] {
        push_fixed(
            output,
            fixed,
            EffectInstructionV3::write_request_identity(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(offset)?,
                identity_common(coordinate)?,
            ),
        )?;
    }
    for (offset, coordinate, width) in [
        (
            AffineBatchRequestLayoutV2::EXPECTED_MARKET_REVISION,
            scalar::CLAIMS_MARKET_REVISION,
            8,
        ),
        (
            AffineBatchRequestLayoutV2::OUTCOME_COUNT,
            scalar::OUTCOME_COUNT,
            4,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_COUNT,
            scalar::CLAIMS_ROW_COUNT,
            4,
        ),
    ] {
        let instruction = match width {
            8 => EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(offset)?,
                scalar_common(coordinate)?,
            ),
            _ => EffectInstructionV3::write_request_u32(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(offset)?,
                scalar_common(coordinate)?,
            ),
        };
        push_fixed(output, fixed, instruction)?;
    }
    for position in 0..position_count {
        let owner = if position == 0 {
            identity::POSITION_ZERO_OWNER
        } else {
            identity::POSITION_ONE_OWNER
        };
        let revision = if position == 0 {
            scalar::POSITION_ZERO_REVISION
        } else {
            scalar::POSITION_ONE_REVISION
        };
        push_fixed(
            output,
            fixed,
            EffectInstructionV3::write_request_identity(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(
                    AffineBatchRequestLayoutV2::position_field(
                        position,
                        AffineBatchRequestLayoutV2::POSITION_OWNER,
                    )
                    .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
                )?,
                identity_common(owner)?,
            ),
        )?;
        push_fixed(
            output,
            fixed,
            EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Fixed,
                offset_u32(
                    AffineBatchRequestLayoutV2::position_field(
                        position,
                        AffineBatchRequestLayoutV2::POSITION_REVISION,
                    )
                    .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
                )?,
                scalar_common(revision)?,
            ),
        )?;
    }
    for (offset, coordinate, width) in [
        (
            AffineBatchRequestLayoutV2::ROW_SOURCE_PRESENT,
            scalar::CLAIMS_SOURCE_PRESENT,
            1,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_DESTINATION_PRESENT,
            scalar::CLAIMS_DESTINATION_PRESENT,
            1,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_OUTCOME,
            item_scalar::OUTCOME,
            4,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_SOURCE_INDEX,
            scalar::CLAIMS_SOURCE_POSITION_INDEX,
            4,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_DESTINATION_INDEX,
            scalar::CLAIMS_DESTINATION_POSITION_INDEX,
            4,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_AGGREGATE_DIRECTION,
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            1,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_AGGREGATE_MAGNITUDE,
            item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,
            8,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_SOURCE_DIRECTION,
            scalar::CLAIMS_SOURCE_DIRECTION,
            1,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_SOURCE_MAGNITUDE,
            item_scalar::CLAIMS_SOURCE_MAGNITUDE,
            8,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_DESTINATION_DIRECTION,
            scalar::CLAIMS_DESTINATION_DIRECTION,
            1,
        ),
        (
            AffineBatchRequestLayoutV2::ROW_DESTINATION_MAGNITUDE,
            item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
            8,
        ),
    ] {
        let item_coordinate = matches!(
            coordinate,
            item_scalar::OUTCOME
                | item_scalar::CLAIMS_AGGREGATE_MAGNITUDE
                | item_scalar::CLAIMS_SOURCE_MAGNITUDE
                | item_scalar::CLAIMS_DESTINATION_MAGNITUDE
        );
        let register = if item_coordinate {
            ScalarCoordinateV3::item(scalar_u16(coordinate)?)
        } else {
            scalar_common(coordinate)?
        };
        let instruction = match width {
            1 => EffectInstructionV3::write_request_u8(
                route,
                RequestSpaceV3::Item,
                offset_u32(offset)?,
                register,
            ),
            4 => EffectInstructionV3::write_request_u32(
                route,
                RequestSpaceV3::Item,
                offset_u32(offset)?,
                register,
            ),
            _ => EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Item,
                offset_u32(offset)?,
                register,
            ),
        };
        push_item(output, item, instruction)?;
    }
    Ok(())
}

fn append_custody_transfer_patches(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    dynamic_compartments: bool,
) -> Result<()> {
    append_custody_identities(output, cursor, route, true, false, false)?;
    for (offset, coordinate, width) in [
        (
            CustodyRequestLayoutV1::TRANSFER_INDEX,
            scalar::TRANSFER_INDEX,
            2,
        ),
        (
            CustodyRequestLayoutV1::EXPECTED_REVISION,
            scalar::CUSTODY_EXPECTED_REVISION,
            8,
        ),
        (
            CustodyRequestLayoutV1::RESULTING_REVISION,
            scalar::CUSTODY_RESULTING_REVISION,
            8,
        ),
        (CustodyRequestLayoutV1::ORDER_NONCE, scalar::ORDER_NONCE, 8),
        (CustodyRequestLayoutV1::GENERATION, scalar::GENERATION, 8),
        (CustodyRequestLayoutV1::AMOUNT, scalar::CUSTODY_AMOUNT, 8),
        (CustodyRequestLayoutV1::PAGE_INDEX, scalar::PAGE_INDEX, 4),
        (
            CustodyRequestLayoutV1::EXECUTION_INDEX,
            scalar::EXECUTION_INDEX,
            4,
        ),
    ] {
        push_custody_scalar(output, cursor, route, offset, coordinate, width)?;
    }
    if dynamic_compartments {
        push_custody_scalar(
            output,
            cursor,
            route,
            CustodyRequestLayoutV1::SOURCE_COMPARTMENT,
            scalar::CUSTODY_SOURCE_COMPARTMENT,
            1,
        )?;
        push_custody_scalar(
            output,
            cursor,
            route,
            CustodyRequestLayoutV1::DESTINATION_COMPARTMENT,
            scalar::CUSTODY_DESTINATION_COMPARTMENT,
            1,
        )?;
    }
    Ok(())
}

fn append_custody_initialize_patches(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    open: bool,
) -> Result<()> {
    for (offset, coordinate) in [
        (CustodyRequestLayoutV1::RELEASE_SET, identity::RELEASE_SET),
        (CustodyRequestLayoutV1::MARKET, identity::MARKET),
        (CustodyRequestLayoutV1::REALM, identity::REALM),
        (CustodyRequestLayoutV1::CONTEXT, identity::GENERAL_ROOT),
        (
            CustodyRequestLayoutV1::CALLER_PROGRAM,
            identity::TRADING_PROGRAM,
        ),
        (
            CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
            identity::PARENT_REQUEST_DIGEST,
        ),
        (CustodyRequestLayoutV1::PAYER, identity::PAYER),
        (CustodyRequestLayoutV1::RENT_REFUND, identity::RENT_REFUND),
    ] {
        push_custody_identity(output, cursor, route, offset, coordinate)?;
    }
    if open {
        for (offset, coordinate) in [
            (
                CustodyRequestLayoutV1::DESTINATION,
                identity::CUSTODY_DESTINATION,
            ),
            (
                CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
                identity::GENERAL_ROOT,
            ),
            (CustodyRequestLayoutV1::MINT, identity::MINT),
            (
                CustodyRequestLayoutV1::TOKEN_PROGRAM,
                identity::TOKEN_PROGRAM,
            ),
        ] {
            push_custody_identity(output, cursor, route, offset, coordinate)?;
        }
    }
    push_custody_scalar(
        output,
        cursor,
        route,
        CustodyRequestLayoutV1::GENERATION,
        scalar::GENERATION,
        8,
    )?;
    push_custody_scalar(
        output,
        cursor,
        route,
        CustodyRequestLayoutV1::RENT_LAMPORTS,
        if open {
            scalar::CUSTODY_VAULT_RENT_LAMPORTS
        } else {
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS
        },
        8,
    )
}

fn append_custody_close_patches(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    replay: bool,
) -> Result<()> {
    append_custody_identities(output, cursor, route, false, true, replay)?;
    let (expected, resulting, rent) = if replay {
        (
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION,
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
        )
    } else {
        (
            scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION,
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            scalar::CUSTODY_VAULT_RENT_LAMPORTS,
        )
    };
    for (offset, coordinate, width) in [
        (
            CustodyRequestLayoutV1::TRANSFER_INDEX,
            scalar::TRANSFER_INDEX,
            2,
        ),
        (CustodyRequestLayoutV1::EXPECTED_REVISION, expected, 8),
        (CustodyRequestLayoutV1::RESULTING_REVISION, resulting, 8),
        (CustodyRequestLayoutV1::ORDER_NONCE, scalar::ORDER_NONCE, 8),
        (CustodyRequestLayoutV1::GENERATION, scalar::GENERATION, 8),
        (CustodyRequestLayoutV1::RENT_LAMPORTS, rent, 8),
        (CustodyRequestLayoutV1::PAGE_INDEX, scalar::PAGE_INDEX, 4),
        (
            CustodyRequestLayoutV1::EXECUTION_INDEX,
            scalar::EXECUTION_INDEX,
            4,
        ),
    ] {
        push_custody_scalar(output, cursor, route, offset, coordinate, width)?;
    }
    Ok(())
}

fn append_custody_identities(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    transfer: bool,
    close: bool,
    replay: bool,
) -> Result<()> {
    for (offset, coordinate) in [
        (CustodyRequestLayoutV1::RELEASE_SET, identity::RELEASE_SET),
        (CustodyRequestLayoutV1::MARKET, identity::MARKET),
        (CustodyRequestLayoutV1::REALM, identity::REALM),
        (CustodyRequestLayoutV1::CONTEXT, identity::GENERAL_ROOT),
        (
            CustodyRequestLayoutV1::CALLER_PROGRAM,
            identity::TRADING_PROGRAM,
        ),
        (CustodyRequestLayoutV1::CANDIDATE, identity::CANDIDATE),
        (CustodyRequestLayoutV1::ORDER, identity::ORDER),
        (
            CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
            identity::PARENT_REQUEST_DIGEST,
        ),
    ] {
        push_custody_identity(output, cursor, route, offset, coordinate)?;
    }
    if transfer {
        for (offset, coordinate) in [
            (
                CustodyRequestLayoutV1::SOURCE_OWNER,
                identity::CUSTODY_SOURCE_OWNER,
            ),
            (
                CustodyRequestLayoutV1::DESTINATION_OWNER,
                identity::CUSTODY_DESTINATION_OWNER,
            ),
            (CustodyRequestLayoutV1::SOURCE, identity::CUSTODY_SOURCE),
            (
                CustodyRequestLayoutV1::DESTINATION,
                identity::CUSTODY_DESTINATION,
            ),
            (
                CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT,
                identity::SOURCE_VAULT_CONTEXT,
            ),
            (
                CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
                identity::DESTINATION_VAULT_CONTEXT,
            ),
            (CustodyRequestLayoutV1::MINT, identity::MINT),
            (
                CustodyRequestLayoutV1::TOKEN_PROGRAM,
                identity::TOKEN_PROGRAM,
            ),
        ] {
            push_custody_identity(output, cursor, route, offset, coordinate)?;
        }
    } else if close {
        push_custody_identity(
            output,
            cursor,
            route,
            CustodyRequestLayoutV1::RENT_REFUND,
            identity::RENT_REFUND,
        )?;
        if !replay {
            for (offset, coordinate) in [
                (CustodyRequestLayoutV1::SOURCE, identity::CUSTODY_SOURCE),
                (
                    CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT,
                    identity::GENERAL_ROOT,
                ),
                (CustodyRequestLayoutV1::MINT, identity::MINT),
                (
                    CustodyRequestLayoutV1::TOKEN_PROGRAM,
                    identity::TOKEN_PROGRAM,
                ),
            ] {
                push_custody_identity(output, cursor, route, offset, coordinate)?;
            }
        }
    }
    Ok(())
}

fn push_custody_identity(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    offset: usize,
    coordinate: u32,
) -> Result<()> {
    push_fixed(
        output,
        cursor,
        EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            offset_u32(offset)?,
            identity_common(coordinate)?,
        ),
    )
}

fn push_custody_scalar(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
    offset: usize,
    coordinate: u32,
    width: usize,
) -> Result<()> {
    let instruction = match width {
        1 => EffectInstructionV3::write_request_u8(
            route,
            RequestSpaceV3::Fixed,
            offset_u32(offset)?,
            scalar_common(coordinate)?,
        ),
        2 => EffectInstructionV3::write_request_u16(
            route,
            RequestSpaceV3::Fixed,
            offset_u32(offset)?,
            scalar_common(coordinate)?,
        ),
        4 => EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            offset_u32(offset)?,
            scalar_common(coordinate)?,
        ),
        8 => EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            offset_u32(offset)?,
            scalar_common(coordinate)?,
        ),
        _ => return Err(GeneralEffectArtifactErrorV3::Geometry),
    };
    push_fixed(output, cursor, instruction)
}

fn position_template(
    action: ProtocolPositionActionV2,
) -> Result<[u8; PROTOCOL_POSITION_REQUEST_BYTES_V2]> {
    ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: id(1),
        market: id(2),
        position_owner: id(3),
        parent_request_digest: id(4),
        rent_credit: id(5),
        rent_program: id(6),
        generation: 1,
        expected_market_revision: 1,
        expected_position_revision: if action == ProtocolPositionActionV2::Admit {
            0
        } else {
            1
        },
        observed_position_lamports: 2,
        observed_admission_lamports: 2,
        position_rent_principal: 1,
        admission_rent_principal: 1,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .to_bytes()
    .map_err(|_| GeneralEffectArtifactErrorV3::Claims)
}

fn affine_template(position_count: u32) -> Result<([u8; 384], usize)> {
    let mut bytes = [0_u8; 384];
    let positions = [
        AffineBatchPositionV2::new(id(7), 1).map_err(|_| GeneralEffectArtifactErrorV3::Claims)?,
        AffineBatchPositionV2::new(id(8), 1).map_err(|_| GeneralEffectArtifactErrorV3::Claims)?,
    ];
    let neutral = SignedMagnitudeV2::new(DeltaDirectionV2::Neutral, 0)
        .map_err(|_| GeneralEffectArtifactErrorV3::Claims)?;
    let debit = SignedMagnitudeV2::new(DeltaDirectionV2::Debit, 1)
        .map_err(|_| GeneralEffectArtifactErrorV3::Claims)?;
    let credit = SignedMagnitudeV2::new(DeltaDirectionV2::Credit, 1)
        .map_err(|_| GeneralEffectArtifactErrorV3::Claims)?;
    let row = if position_count == 1 {
        AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: false,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 0,
                aggregate_delta: credit,
                source_delta: neutral,
                destination_delta: credit,
            },
            1,
            1,
        )
    } else {
        AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: neutral,
                source_delta: debit,
                destination_delta: credit,
            },
            1,
            2,
        )
    }
    .map_err(|_| GeneralEffectArtifactErrorV3::Claims)?;
    let len = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
        .checked_add(
            usize::try_from(position_count)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?
                .checked_mul(AFFINE_BATCH_POSITION_BYTES_V2)
                .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .and_then(|value| value.checked_add(AFFINE_BATCH_ROW_BYTES_V2))
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    let selected_positions = positions
        .get(
            ..usize::try_from(position_count)
                .map_err(|_| GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    AffineBatchPlanV2::encode_into(
        AffineBatchPlanInputV2 {
            caller_role: ClaimsCallerRole::Trading,
            release_set: id(1),
            market: id(2),
            request_id: id(3),
            product_record_digest: id(4),
            semantic_basis_id: id(5),
            linked_basis_record_digest: id(6),
            expected_market_revision: 1,
            outcome_count: 1,
        },
        selected_positions,
        &[row],
        bytes
            .get_mut(..len)
            .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralEffectArtifactErrorV3::Claims)?;
    Ok((bytes, len))
}

fn custody_template(
    operation: OperationV1,
    source_compartment: CompartmentV1,
    destination_compartment: CompartmentV1,
) -> Result<[u8; CUSTODY_REQUEST_BYTES_V1]> {
    let transfer = operation == OperationV1::Transfer;
    let initialize = operation == OperationV1::InitializeReplay;
    let open = operation == OperationV1::OpenVault;
    let close_vault = operation == OperationV1::CloseVault;
    let context = id(4);
    CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set: id(1),
        market: id(2),
        realm: id(3),
        context,
        caller_program: id(5),
        semantic: ContextV1 {
            candidate: if transfer || close_vault {
                id(6)
            } else {
                [0; 32]
            },
            source_owner: if transfer && source_compartment == CompartmentV1::External {
                id(7)
            } else {
                [0; 32]
            },
            destination_owner: if transfer && destination_compartment == CompartmentV1::External {
                id(8)
            } else {
                [0; 32]
            },
            order: if transfer || close_vault {
                id(9)
            } else {
                [0; 32]
            },
            parent_request_digest: id(10),
            order_nonce: if transfer || close_vault { 1 } else { 0 },
            generation: 1,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: if transfer || close_vault {
            id(11)
        } else {
            [0; 32]
        },
        destination: if transfer || open { id(12) } else { [0; 32] },
        source_vault_context: if (transfer && source_compartment != CompartmentV1::External)
            || close_vault
        {
            context
        } else {
            [0; 32]
        },
        destination_vault_context: if open
            || (transfer && destination_compartment != CompartmentV1::External)
        {
            context
        } else {
            [0; 32]
        },
        mint: if transfer || open || close_vault {
            id(13)
        } else {
            [0; 32]
        },
        token_program: if transfer || open || close_vault {
            id(14)
        } else {
            [0; 32]
        },
        payer: if initialize || open { id(15) } else { [0; 32] },
        rent_refund: if initialize || open || close_vault || operation == OperationV1::CloseReplay {
            id(16)
        } else {
            [0; 32]
        },
        expected_revision: if initialize { 0 } else { 1 },
        resulting_revision: if initialize { 1 } else { 2 },
        amount: if transfer { 1 } else { 0 },
        rent_lamports: if transfer { 0 } else { 1 },
    }
    .to_bytes()
    .map_err(|_| GeneralEffectArtifactErrorV3::Custody)
}

const fn empty_route<'a>() -> RouteInputV3<'a> {
    route(
        FixedRole::Core,
        RouteKindV3::Once,
        None,
        None,
        0,
        0,
        &[],
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
const fn route<'a>(
    role: FixedRole,
    kind: RouteKindV3,
    enable_common_scalar: Option<u16>,
    receipt_dependency: Option<RouteReceiptDependencyV3>,
    fixed_account_start: u16,
    fixed_account_count: u16,
    fixed_request: &'a [u8],
    item_request: &'a [u8],
) -> RouteInputV3<'a> {
    RouteInputV3 {
        role,
        kind,
        enable_common_scalar,
        witness_range_common_scalar: None,
        receipt_dependency,
        fixed_account_start,
        fixed_account_count,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request,
        item_request,
    }
}

fn push_fixed(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    value: EffectInstructionV3,
) -> Result<()> {
    let slot = output
        .get_mut(*cursor)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    *slot = value;
    *cursor = cursor
        .checked_add(1)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?;
    Ok(())
}

fn push_item(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    value: EffectInstructionV3,
) -> Result<()> {
    push_fixed(output, cursor, value)
}

fn scalar_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralEffectArtifactErrorV3::Geometry)
}

fn scalar_common(value: u32) -> Result<ScalarCoordinateV3> {
    Ok(ScalarCoordinateV3::common(scalar_u16(value)?))
}

fn identity_common(value: u32) -> Result<IdentityCoordinateV3> {
    Ok(IdentityCoordinateV3::common(scalar_u16(value)?))
}

fn offset_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| GeneralEffectArtifactErrorV3::Geometry)
}

fn add_accounts(left: u16, right: u16) -> Result<u16> {
    left.checked_add(right)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

fn copy_at(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)?
        .copy_from_slice(value);
    Ok(())
}

fn slice_at(input: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(len)
                    .ok_or(GeneralEffectArtifactErrorV3::Geometry)?,
        )
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

/// Compartments one action's Custody `Transfer` template must carry.
///
/// The values live in [`crate::escrow_v1`]. An action that reaches a template
/// builder and names no movement there is a wiring error, not a shape: it is
/// refused rather than defaulted, because a default would be a second author of
/// the fact this indirection exists to have exactly one author for.
fn action_template_compartments(action: Action) -> Result<(CompartmentV1, CompartmentV1)> {
    crate::escrow_v1::general_action_template_compartments_v1(action)
        .ok_or(GeneralEffectArtifactErrorV3::Geometry)
}

const fn route_count(action: Action) -> usize {
    match action {
        Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => 0,
        Action::OpenBatch | Action::CloseBatch => 0,
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Materialize | Action::Distribute => 2,
        Action::Close => 4,
        Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => 5,
    }
}

const fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn artifact(action: Action) -> vec::Vec<u8> {
        let (fixed, item) = general_effect_instruction_count_v3(action);
        let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; fixed + item];
        let mut templates = vec![0; general_effect_template_bytes_v3(action)];
        let len = general_effect_program_bytes_v3(action).expect("program width");
        let mut scratch = vec![0; len];
        let mut output = vec![0xa5; len];
        encode_general_effect_program_v3_atomic(
            action,
            &mut instructions,
            &mut templates,
            &mut scratch,
            &mut output,
        )
        .expect("action artifact");
        output
    }

    #[test]
    fn every_authored_action_generates_exact_hot38_relative_artifacts() {
        for action in [
            Action::Consider,
            Action::Freeze,
            Action::InitializeSettlement,
            Action::Collect,
            Action::Materialize,
            Action::Distribute,
            Action::Close,
            Action::OpenBatch,
            Action::CloseBatch,
            Action::SubmitCandidate,
            Action::VerifyCandidateRow,
            Action::PlaceOrder,
            Action::CancelOrder,
            Action::ReleaseOrder,
        ] {
            let bytes = artifact(action);
            let program = ProgramV3::decode(&bytes).expect("decoded artifact");
            assert_eq!(
                program.route_count(),
                u16::try_from(route_count(action)).expect("route count")
            );
            assert_eq!(
                program.fixed_account_count(),
                general_effect_account_count_v3(action).expect("accounts")
            );
            assert_eq!(
                program.common_scalar_count(),
                u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3).expect("common scalars")
            );
            assert_eq!(
                program.item_scalar_stride(),
                u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3).expect("item stride")
            );
        }
    }

    #[test]
    fn submit_candidate_effect_resolves_exact_creation_funding_at_runtime_widths() {
        let bytes = artifact(Action::SubmitCandidate);
        let program = ProgramV3::decode(&bytes).expect("SubmitCandidate effect");
        assert_eq!(program.route_count(), 0);
        assert_eq!(program.fixed_account_count(), 11);
        for count in [1_u32, 258] {
            let mut scalars = vec![
                0_u64;
                usize::try_from(
                    GENERAL_HOT_COMMON_SCALARS_V3 + count * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
                )
                .expect("scalar width")
            ];
            scalars[usize::try_from(scalar::ONE).expect("version")] = 1;
            scalars[usize::try_from(scalar::CANDIDATE_POST_STATUS).expect("status")] = 1;
            scalars[usize::try_from(scalar::OUTCOME_COUNT).expect("outcomes")] = u64::from(count);
            scalars[usize::try_from(scalar::CANDIDATE_PAGE_COUNT).expect("pages")] =
                u64::from(count);
            scalars[usize::try_from(scalar::CANDIDATE_ROW_COUNT).expect("rows")] = u64::from(count);
            scalars[usize::try_from(scalar::SCRATCH_A).expect("work escrow")] = 777;
            scalars[usize::try_from(scalar::SCRATCH_B).expect("funded state")] = 1_777;
            let identities = vec![
                [0x44_u8; 32];
                usize::try_from(
                    GENERAL_HOT_COMMON_IDENTITIES_V3 + count * GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3,
                )
                .expect("identity width")
            ];

            for operation in 0..23_u16 {
                program
                    .resolved_fixed_effect(operation, count, &scalars, &identities)
                    .expect("every fixed effect resolves at the authenticated runtime width");
            }
            assert_eq!(
                program.resolved_fixed_effect(21, count, &scalars, &identities),
                Ok(
                    dclutch_effect_kernel::v3::ResolvedEffectV3::TransferLamports {
                        source: usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
                        destination: usize::from(GENERAL_STATE_ACCOUNT_COORDINATE_V3),
                        amount: 777,
                    }
                ),
            );
            assert_eq!(
                program.resolved_fixed_effect(22, count, &scalars, &identities),
                Ok(
                    dclutch_effect_kernel::v3::ResolvedEffectV3::RequireLamportsEq {
                        account: usize::from(GENERAL_STATE_ACCOUNT_COORDINATE_V3),
                        value: 1_777,
                    }
                ),
            );
        }
    }

    #[test]
    fn verify_effect_persists_five_planar_tails_and_only_terminal_result() {
        let bytes = artifact(Action::VerifyCandidateRow);
        let program = ProgramV3::decode(&bytes).expect("VerifyCandidateRow effect");
        assert_eq!(program.route_count(), 0);
        assert_eq!(program.fixed_account_count(), 15);
        assert_eq!(program.fixed_operation_count(), 53);
        assert_eq!(program.item_operation_count(), 7);

        for count in [1_u32, 258] {
            let scalar_len = usize::try_from(
                GENERAL_HOT_COMMON_SCALARS_V3 + count * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
            )
            .expect("scalar width");
            let identity_len = usize::try_from(
                GENERAL_HOT_COMMON_IDENTITIES_V3 + count * GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3,
            )
            .expect("identity width");
            let mut scalars = vec![0_u64; scalar_len];
            let identities = vec![[0x44_u8; 32]; identity_len];
            scalars[usize::try_from(scalar::ONE).expect("one")] = 1;
            scalars[usize::try_from(scalar::LOCAL_STATE_VERSION).expect("local version")] = 3;
            scalars[usize::try_from(scalar::LOCAL_STATE_KIND).expect("local kind")] = 5;
            scalars[usize::try_from(scalar::RUNTIME_WIDTH_VERSION).expect("runtime version")] = 2;
            scalars[usize::try_from(scalar::OUTCOME_COUNT).expect("outcomes")] = u64::from(count);

            let selected_item = count - 1;
            let selected_base = usize::try_from(
                GENERAL_HOT_COMMON_SCALARS_V3 + selected_item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
            )
            .expect("selected item base");
            for (coordinate, value) in [
                (item_scalar::QUANTITY, 11_u64),
                (item_scalar::CURSOR_INVENTORY, 22),
                (item_scalar::CLAIMS_AGGREGATE_MAGNITUDE, 33),
                (item_scalar::CLAIMS_SOURCE_MAGNITUDE, 44),
                (item_scalar::CLAIMS_DESTINATION_MAGNITUDE, 55),
            ] {
                scalars[selected_base + usize::try_from(coordinate).expect("item coordinate")] =
                    value;
            }

            // Disabled Result creation is a true no-op: all verifier writes
            // still resolve, but neither the fixed result header nor either
            // result tail touches the vacant account.
            for operation in 0..53_u16 {
                let resolved = program
                    .resolved_fixed_effect(operation, count, &scalars, &identities)
                    .expect("every disabled fixed effect resolves");
                if (37..=50).contains(&operation) {
                    assert_eq!(resolved, dclutch_effect_kernel::v3::ResolvedEffectV3::Noop);
                }
            }
            for operation in 0..7_u16 {
                let resolved = program
                    .resolved_item_effect(selected_item, operation, count, &scalars, &identities)
                    .expect("every disabled item effect resolves");
                if operation >= 5 {
                    assert_eq!(resolved, dclutch_effect_kernel::v3::ResolvedEffectV3::Noop);
                }
            }

            scalars[usize::try_from(scalar::VERIFY_TERMINAL).expect("terminal")] = 1;
            let verifier_base =
                GeneralLocalStateLayoutV3::body() + RuntimeVerifierLayoutV2::tails_base();
            let item_offset = selected_item * RuntimeVerifierLayoutV2::tail_item_stride();
            for (operation, tail, value) in [
                (0_u16, 0_u32, 11_u64),
                (1, 1, 22),
                (2, 2, 33),
                (3, 3, 44),
                (4, 4, 55),
            ] {
                assert_eq!(
                    program.resolved_item_effect(
                        selected_item,
                        operation,
                        count,
                        &scalars,
                        &identities,
                    ),
                    Ok(dclutch_effect_kernel::v3::ResolvedEffectV3::WriteScalar {
                        account: usize::from(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
                        offset: verifier_base
                            + (tail * count * RuntimeVerifierLayoutV2::tail_item_stride())
                            + item_offset,
                        value,
                    })
                );
            }
            assert_eq!(
                program.resolved_item_effect(selected_item, 5, count, &scalars, &identities,),
                Ok(dclutch_effect_kernel::v3::ResolvedEffectV3::WriteScalar {
                    account: usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3),
                    offset: VerifiedCandidateLayoutV2::claim_inputs_base()
                        + selected_item * VerifiedCandidateLayoutV2::tail_item_stride(),
                    value: 44,
                })
            );
            assert_eq!(
                program.resolved_item_effect(selected_item, 6, count, &scalars, &identities,),
                Ok(dclutch_effect_kernel::v3::ResolvedEffectV3::WriteScalar {
                    account: usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3),
                    offset: VerifiedCandidateLayoutV2::claim_inputs_base()
                        + (count + selected_item) * VerifiedCandidateLayoutV2::tail_item_stride(),
                    value: 55,
                })
            );
        }
    }

    #[test]
    fn runtime_width_routes_bind_exact_child_templates_and_receipts() {
        for action in [Action::Collect, Action::Materialize, Action::Distribute] {
            let bytes = artifact(action);
            let program = ProgramV3::decode(&bytes).expect("decoded artifact");
            assert_eq!(program.route_count(), 2);
            let custody = program.route(1).expect("custody route");
            assert_eq!(custody.fixed_account_count(), CUSTODY_TRANSFER_ACCOUNTS);
            assert_eq!(custody.receipt_dependency(), None);
            let (custody_template, item) = program.route_template(1).expect("custody template");
            assert!(item.is_empty());
            assert_eq!(
                CustodyRequestV1::decode(custody_template)
                    .expect("canonical custody")
                    .operation,
                OperationV1::Transfer
            );
        }
    }

    #[test]
    fn terminal_close_orders_optional_transfer_then_vault_and_replay_close() {
        let bytes = artifact(Action::Close);
        let program = ProgramV3::decode(&bytes).expect("close artifact");
        assert_eq!(
            program.route(0).expect("transfer").fixed_account_start(),
            general_child_account_start_v3(Action::Close)
        );
        let (position, _) = program.route_template(1).expect("position close");
        assert_eq!(
            ProtocolPositionRequestV2::decode(position)
                .expect("position request")
                .action,
            ProtocolPositionActionV2::Close
        );
        let (vault, _) = program.route_template(2).expect("vault close");
        let (replay, _) = program.route_template(3).expect("replay close");
        assert_eq!(
            CustodyRequestV1::decode(vault)
                .expect("vault request")
                .operation,
            OperationV1::CloseVault
        );
        assert_eq!(
            CustodyRequestV1::decode(replay)
                .expect("replay request")
                .operation,
            OperationV1::CloseReplay
        );
        let dependency = program
            .route(3)
            .expect("replay route")
            .receipt_dependency()
            .expect("vault receipt dependency");
        assert_eq!(dependency.producer_route(), 2);
        assert_eq!(dependency.producer_role(), FixedRole::Custody);
        assert_eq!(
            usize::from(dependency.expected_receipt_bytes()),
            CUSTODY_RECEIPT_BYTES_V1
        );
    }

    #[test]
    fn position_lifecycle_is_admitted_once_then_closed_once() {
        let initialize = artifact(Action::InitializeSettlement);
        let initialize = ProgramV3::decode(&initialize).expect("initialize");
        let (admit, _) = initialize.route_template(0).expect("admit route");
        assert_eq!(
            ProtocolPositionRequestV2::decode(admit)
                .expect("admit request")
                .action,
            ProtocolPositionActionV2::Admit
        );
        for action in [Action::Collect, Action::Materialize, Action::Distribute] {
            let bytes = artifact(action);
            let program = ProgramV3::decode(&bytes).expect("settlement");
            assert_eq!(program.route_count(), 2);
            assert_eq!(program.route(0).expect("affine").role(), FixedRole::Claims);
            assert_eq!(
                program.route(0).expect("affine").kind(),
                RouteKindV3::AffineOnce
            );
        }
        let close = artifact(Action::Close);
        let close = ProgramV3::decode(&close).expect("close");
        assert_eq!(close.route_count(), 4);
    }

    #[test]
    fn nonexact_workspaces_refuse_without_output_mutation() {
        let action = Action::Materialize;
        let (fixed, item) = general_effect_instruction_count_v3(action);
        let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; fixed + item - 1];
        let mut templates = vec![0; general_effect_template_bytes_v3(action)];
        let len = general_effect_program_bytes_v3(action).expect("program width");
        let mut scratch = vec![0; len];
        let mut output = vec![0xa5; len];
        assert_eq!(
            encode_general_effect_program_v3_atomic(
                action,
                &mut instructions,
                &mut templates,
                &mut scratch,
                &mut output,
            ),
            Err(GeneralEffectArtifactErrorV3::Geometry)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
}
