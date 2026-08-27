//! Generated, runtime-width General EffectProgram V3 artifacts.
//!
//! Every child template is first constructed by its semantic-owner codec and
//! then patched only through typed public field coordinates.  Account starts
//! are derived from the frozen Hot38 logical prefix
//! `[root, config, product, portfolio, linked-basis]`; no physical Hot account
//! index enters the artifact.  Generic Trading remains the sole request
//! projector, account writer, CPI authority, and atomic committer.

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
};
use dclutch_general_codec::Action;

use crate::hot_candidate_v3::{
    GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
    GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, GENERAL_HOT_ITEM_SCALAR_STRIDE_V3, identity, item_scalar,
    scalar,
};
use crate::{
    local_state_v3::GeneralLocalStateLayoutV3,
    runtime_selection::RuntimeSelectionLayoutV2,
    runtime_width::SettlementCursorLayoutV2,
    state_artifacts_v3::{
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        general_child_account_start_v3,
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
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Materialize | Action::Distribute => 2,
        Action::Close => 4,
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
const MAX_ROUTE_COUNT: usize = 4;

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
    }
}

/// Return the exact finalized EffectProgram byte width for one action.
pub fn general_effect_program_bytes_v3(action: Action) -> Result<usize> {
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
        Action::InitializeSettlement | Action::Close => 1,
        Action::Consider
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
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute
        | Action::Close => 1,
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
    let suffix = match action {
        Action::Consider | Action::Freeze => 0,
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
        Action::Consider | Action::Freeze => Ok(0),
        Action::InitializeSettlement => build_initialize(instructions, fixed, templates, routes),
        Action::Collect => build_settlement(
            action,
            2,
            CompartmentV1::External,
            CompartmentV1::Settlement,
            instructions,
            fixed,
            item,
            templates,
            routes,
        ),
        Action::Materialize => build_settlement(
            action,
            1,
            CompartmentV1::Settlement,
            CompartmentV1::HoardPrincipal,
            instructions,
            fixed,
            item,
            templates,
            routes,
        ),
        Action::Distribute => build_settlement(
            action,
            2,
            CompartmentV1::Settlement,
            CompartmentV1::External,
            instructions,
            fixed,
            item,
            templates,
            routes,
        ),
        Action::Close => build_close(instructions, fixed, templates, routes),
    }
}

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
    append_position_patches(instructions, fixed, 0)?;
    append_custody_initialize_patches(instructions, fixed, 1, false)?;
    append_custody_initialize_patches(instructions, fixed, 2, true)?;
    Ok(3)
}

#[allow(clippy::too_many_arguments)]
fn build_settlement<'a>(
    action: Action,
    position_count: u32,
    source: CompartmentV1,
    destination: CompartmentV1,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
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

fn build_close<'a>(
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    templates: &'a mut [u8],
    routes: &mut [RouteInputV3<'a>; MAX_ROUTE_COUNT],
) -> Result<usize> {
    let transfer = custody_template(
        OperationV1::Transfer,
        CompartmentV1::Settlement,
        CompartmentV1::External,
    )?;
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
    append_position_patches(instructions, fixed, 1)?;
    append_custody_close_patches(instructions, fixed, 2, false)?;
    append_custody_close_patches(instructions, fixed, 3, true)?;
    Ok(4)
}

fn append_general_state_patches(
    action: Action,
    instructions: &mut [EffectInstructionV3],
    fixed: &mut usize,
    item: &mut usize,
) -> Result<()> {
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

fn append_position_patches(
    output: &mut [EffectInstructionV3],
    cursor: &mut usize,
    route: u16,
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
            scalar::CLAIMS_MARKET_REVISION,
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

const fn route_count(action: Action) -> usize {
    match action {
        Action::Consider | Action::Freeze => 0,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Materialize | Action::Distribute => 2,
        Action::Close => 4,
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
    fn all_seven_actions_generate_exact_hot38_relative_artifacts() {
        for action in [
            Action::Consider,
            Action::Freeze,
            Action::InitializeSettlement,
            Action::Collect,
            Action::Materialize,
            Action::Distribute,
            Action::Close,
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
