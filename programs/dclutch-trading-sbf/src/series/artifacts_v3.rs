//! Exact finalized-artifact join for recurring Series V3.
//!
//! CapabilityProgramSetV1 selects one complete CapabilityProgramV3 from the
//! action byte in the exact 128-byte Series header. The occurrence Merkle path
//! is an independently bounded trailing witness: RequestProfile never treats
//! it as Product-affine data. The selected descriptor then joins the exact
//! AccountProfile, RequestProfile, EffectProgram, ExecutionStrategy, and
//! underlying TransitionVM records. This module authenticates and projects no
//! state; the common Trading V3 outer remains the sole writer and CPI caller.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::{
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
    v3::CapabilityProgramV3,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        ProgramV3 as EffectProgramV3, ResolvedInvocationV3, RouteKindV3, RouteReceiptDependencyV3,
    },
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2,
};
use dclutch_request_profile_contract::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_series_v3_kernel::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3;
use dclutch_transition_vm::{MAX_IDENTITIES, MAX_SCALARS, v3::ProgramV3 as TransitionProgramV3};
use solana_program::hash::{hash, hashv};

use super::{
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3},
    state::SERIES_STATE_BYTES_V3,
};

/// Byte offset of the canonical one-byte action selector in the Series header.
pub const SERIES_ACTION_SELECTOR_OFFSET_V3: u32 = 12;
/// One exact Merkle sibling in the borrowed witness suffix.
pub const SERIES_WITNESS_ITEM_BYTES_V3: usize = 32;
/// Exact IR-owned Core request width for Consume.
pub const SERIES_CONSUME_CORE_REQUEST_BYTES_V3: usize =
    dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1;
/// Exact projected pre-founding Custody request width.
pub const SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3: usize =
    dclutch_custody_contract::PROJECTED_CUSTODY_REQUEST_BYTES_V1;
/// Exact Claims Founding V5 request width.
pub const SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3: usize =
    dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5;
/// Exact projected Lock-and-close-source receipt appended to Core Found.
pub const SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3: u16 = 320;
/// Exact projected realization receipt appended to Claims Founding V5.
pub const SERIES_CONSUME_REALIZE_RECEIPT_BYTES_V3: u16 = 320;
/// Exact Claims Founding V5 receipt appended to final Core Open.
pub const SERIES_CONSUME_CLAIMS_RECEIPT_BYTES_V3: u16 = 1008;
const _: () = assert!(dclutch_custody_contract::PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1 == 320);
const _: () = assert!(dclutch_custody_contract::PROJECTED_CUSTODY_RECEIPT_BYTES_V1 == 320);
const _: () = assert!(dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_RECEIPT_BYTES_V5 == 1008);
pub(crate) const SERIES_NO_RECEIPT_DEPENDENCIES_V3: [RouteReceiptDependencyV3; 0] = [];
pub(crate) const SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3: [RouteReceiptDependencyV3; 1] =
    [RouteReceiptDependencyV3::new(
        FixedRole::Custody,
        0,
        SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3,
    )];
pub(crate) const SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3: [RouteReceiptDependencyV3; 2] = [
    RouteReceiptDependencyV3::new(FixedRole::Custody, 0, SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3),
    RouteReceiptDependencyV3::new(
        FixedRole::Custody,
        2,
        SERIES_CONSUME_REALIZE_RECEIPT_BYTES_V3,
    ),
];
pub(crate) const SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3: [RouteReceiptDependencyV3; 1] =
    [RouteReceiptDependencyV3::new(
        FixedRole::Claims,
        3,
        SERIES_CONSUME_CLAIMS_RECEIPT_BYTES_V3,
    )];
/// Lock projected Hoard, Core Found, realize Hoard, Claims Founding, Core Open.
pub const SERIES_CONSUME_ROUTE_COUNT_V3: usize = 5;
/// Request-bank offset of projected SeriesEscrow-to-Hoard Lock.
pub const SERIES_CONSUME_LOCK_OFFSET_V3: usize = 0;
/// Request-bank offset of the first Core call, which creates Founding state.
pub const SERIES_CONSUME_CORE_FOUND_OFFSET_V3: usize =
    SERIES_CONSUME_LOCK_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
/// Request-bank offset of projected-Hoard realization after Core Found.
pub const SERIES_CONSUME_REALIZE_OFFSET_V3: usize =
    SERIES_CONSUME_CORE_FOUND_OFFSET_V3 + SERIES_CONSUME_CORE_REQUEST_BYTES_V3;
/// Request-bank offset of Claims Founding V5 after Custody realization.
pub const SERIES_CONSUME_CLAIMS_OFFSET_V3: usize =
    SERIES_CONSUME_REALIZE_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
/// Request-bank offset of the second Core call, which atomically opens Market.
pub const SERIES_CONSUME_CORE_OPEN_OFFSET_V3: usize =
    SERIES_CONSUME_CLAIMS_OFFSET_V3 + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3;
/// Exact flat IR request bank before borrowed proof and typed receipt dependencies.
pub const SERIES_CONSUME_IR_REQUEST_BYTES_V3: usize = 2 * SERIES_CONSUME_CORE_REQUEST_BYTES_V3
    + 2 * SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
    + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3;
/// Exact Projected Custody Lock child frame.
pub const SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3: u16 = 14;
/// Exact Core Found accounts other than the 1..16 ordered FundingStates.
///
/// This is the fixed 42-account Found/Series prefix plus the exact 15-account
/// permit/Custody/Claims evidence suffix. The FundingState slice is inserted
/// between them by the Core frame and is the only affine account dimension.
pub const SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3: u16 = 57;
/// Exact Projected Custody Realize child frame.
pub const SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3: u16 = 12;
/// Exact Claims Founding V5 child frame.
pub const SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3: u16 = 32;
/// Exact final Core Open frame; funding was consumed by the earlier Found route.
pub const SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3: u16 = 37;
/// Mathematical protocol bound on one occurrence's segregated funding list.
pub const SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3: u16 = 16;
/// Exact normal Custody request width for the prepared SeriesEscrow lifecycle.
pub const SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3: usize =
    dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1;
/// Projected Initialize/Open plus normal replay/Open/Lock.
pub const SERIES_PREPARE_ROUTE_COUNT_V3: usize = 5;
/// Projected Initialize request-bank offset.
pub const SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3: usize = 0;
/// Projected OpenHoard request-bank offset.
pub const SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3: usize =
    SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
/// Normal SeriesEscrow replay initialization request-bank offset.
pub const SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3: usize =
    SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
/// Normal SeriesEscrow Vault-open request-bank offset.
pub const SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3: usize =
    SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Normal founder-to-SeriesEscrow lock request-bank offset.
pub const SERIES_PREPARE_ESCROW_LOCK_OFFSET_V3: usize =
    SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Exact Prepare request-bank width.
pub const SERIES_PREPARE_IR_REQUEST_BYTES_V3: usize =
    2 * SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3 + 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Exact Projected Initialize child frame, including internal ProjectFound.
pub const SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3: u16 =
    PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1 as u16;
/// Exact Projected OpenHoard child frame.
pub const SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3: u16 =
    PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1 as u16;
/// Exact normal Custody InitializeReplay child frame.
pub const SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3: u16 = 12;
/// Exact normal Custody OpenVault child frame.
pub const SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3: u16 = 16;
/// Exact normal Custody Transfer child frame.
pub const SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3: u16 = 14;
/// Refund/escrow cleanup, empty projected-Hoard abort, and permit refund.
pub const SERIES_EXPIRE_ROUTE_COUNT_V3: usize = 5;
/// Normal SeriesEscrow refund request-bank offset.
pub const SERIES_EXPIRE_REFUND_OFFSET_V3: usize = 0;
/// Normal SeriesEscrow Vault-close request-bank offset.
pub const SERIES_EXPIRE_CLOSE_VAULT_OFFSET_V3: usize = SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Normal SeriesEscrow replay-close request-bank offset.
pub const SERIES_EXPIRE_CLOSE_REPLAY_OFFSET_V3: usize = 2 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Empty projected-Hoard abort request-bank offset.
pub const SERIES_EXPIRE_PROJECTED_ABORT_OFFSET_V3: usize =
    3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
/// Core permit-refund request-bank offset after every Custody cleanup.
pub const SERIES_EXPIRE_PERMIT_OFFSET_V3: usize =
    SERIES_EXPIRE_PROJECTED_ABORT_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
/// Exact Core permissionless permit-refund request width before proof.
pub const SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3: usize =
    dclutch_market_core_codec::SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1;
/// Exact Expire request-bank width.
pub const SERIES_EXPIRE_IR_REQUEST_BYTES_V3: usize = 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3
    + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
    + SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3;
/// Exact normal Custody Refund/Transfer child frame.
pub const SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3: u16 = 14;
/// Exact normal Custody CloseVault child frame.
pub const SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3: u16 = 14;
/// Exact normal Custody CloseReplay child frame.
pub const SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3: u16 = 10;
/// Exact Projected AbortOpenAndClose child frame.
pub const SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3: u16 = 11;
/// Exact Core permissionless permit-refund child frame.
pub const SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3: u16 = 25;
/// Semantic kind label for recurring Series V3 capability programs.
pub const SERIES_SUCCESSOR_KIND_PREIMAGE_V3: &[u8] = b"dclutch/kind/series-v3";
/// Family request schema covers the fixed semantic header, not its proof witness.
pub const SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/series-action-header-v3";
/// Mutable Series root-tail schema label.
pub const SERIES_ROOT_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/series-root-v3";
/// Ticket replay-account derivation-policy label.
pub const SERIES_TICKET_DERIVATION_PREIMAGE_V3: &[u8] = b"dclutch/derivation/series-ticket-v3";

/// Exact descriptor-selected raw finalized artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactBytesV3<'a> {
    /// Canonical action-to-program set bytes.
    pub program_set: &'a [u8],
    /// Action-selected CapabilityProgramV3 bytes.
    pub descriptor: &'a [u8],
    /// Exact runtime AccountProfile bytes.
    pub account_profile: &'a [u8],
    /// Exact 128-byte-header RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact generic ExecutionStrategy V2 bytes.
    pub strategy: &'a [u8],
    /// Exact underlying interpreted TransitionVM V3 bytes.
    pub transition: &'a [u8],
    /// Exact fixed-role/local EffectProgram V3 bytes.
    pub effect: &'a [u8],
}

/// Immutable manifest/root selections authenticated before this join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactSelectionV3 {
    /// Capability release selecting the exact ProgramSet record.
    pub program_set: [u8; 32],
    /// Manifest config selecting the exact Series Template content identity.
    pub template: ContentId,
}

/// Stable refusal from the complete Series artifact join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesArtifactErrorV3 {
    /// Selected identity was zero or differed from authenticated bytes/content.
    ContentIdentity,
    /// ProgramSet selector geometry or action selection refused.
    ProgramSet,
    /// Full Series request or its exact header/witness split refused.
    Request,
    /// Selected descriptor named another semantic family or schema.
    Descriptor,
    /// AccountProfile hostile decode refused.
    AccountProfile,
    /// RequestProfile hostile decode or header projection refused.
    RequestProfile,
    /// ExecutionStrategy hostile decode or descriptor join refused.
    Strategy,
    /// Underlying TransitionVM hostile decode or Strategy join refused.
    Transition,
    /// EffectProgram hostile decode or Series role grammar refused.
    Effect,
    /// Fixed non-affine account/register/request geometry differed.
    Geometry,
}

/// Result alias for Series V3 artifact joins.
pub type Result<T> = core::result::Result<T, SeriesArtifactErrorV3>;

/// Exact fixed header and independently bounded witness suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRequestSlicesV3<'a> {
    /// Exact 128-byte semantic header consumed by RequestProfile.
    pub header: &'a [u8],
    /// Exact no-leftover Merkle sibling suffix consumed by Series admission.
    pub witness: &'a [u8],
}

/// Fully joined borrowed artifact bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesArtifactBundleV3<'a> {
    /// Hostile-decoded complete family request.
    pub request: SeriesActionRequestV3<'a>,
    /// Explicit header/witness boundary.
    pub slices: SeriesRequestSlicesV3<'a>,
    /// Selected fixed descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Exact non-affine physical account interpreter.
    pub account_profile: AccountProfileV2<'a>,
    /// Exact fixed-header request interpreter.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact acyclic execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
    /// Exact strategy-selected TransitionVM program.
    pub transition: TransitionProgramV3<'a>,
    /// Exact local/fixed-role effect program.
    pub effect: EffectProgramV3<'a>,
}

/// Exact IR-owned Core base plus proof selected by an executed Consume route.
///
/// The common outer owns concatenation into CPI instruction data. Keeping the
/// typed 336-byte Core request and authenticated proof witness separate here
/// prevents the Series adapter from inventing either portion. The generic
/// role executor appends the independently typed prior Custody receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeInvocationV3<'a> {
    /// Exact IR-owned `SeriesCoreRequestV1` bytes.
    pub core_request: &'a [u8],
    /// Exact trailing occurrence-proof bytes borrowed from the family request.
    pub witness: &'a [u8],
    /// SHA-256 of `core_request || witness` before the typed receipt dependency.
    pub base_request_digest: [u8; 32],
}

/// Exact Core permissionless permit-refund base plus occurrence proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesExpireInvocationV3<'a> {
    /// Exact IR-owned `SeriesPermitExpiryRequestV1` bytes.
    pub permit_request: &'a [u8],
    /// Exact trailing occurrence-proof bytes borrowed from the family request.
    pub witness: &'a [u8],
    /// SHA-256 of `permit_request || witness`.
    pub child_request_digest: [u8; 32],
}

impl SeriesExpireInvocationV3<'_> {
    /// Exact concatenated Core instruction width.
    pub fn child_request_len(self) -> Result<usize> {
        self.permit_request
            .len()
            .checked_add(self.witness.len())
            .ok_or(SeriesArtifactErrorV3::Geometry)
    }
}

impl SeriesConsumeInvocationV3<'_> {
    /// Exact Core request-plus-proof width before the typed prior receipt.
    pub fn base_request_len(self) -> Result<usize> {
        self.core_request
            .len()
            .checked_add(self.witness.len())
            .ok_or(SeriesArtifactErrorV3::Geometry)
    }
}

/// Authenticate and join one complete recurring-Series action bundle.
pub fn authenticate_series_artifacts_v3<'a>(
    selection: SeriesArtifactSelectionV3,
    artifacts: SeriesArtifactBytesV3<'a>,
    family_request: &'a [u8],
) -> Result<SeriesArtifactBundleV3<'a>> {
    require_selected(selection.program_set, artifacts.program_set)?;
    let set = CapabilityProgramSetV1::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| SeriesArtifactErrorV3::ProgramSet)?;
    if set.selector_offset() != SERIES_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U8
    {
        return Err(SeriesArtifactErrorV3::ProgramSet);
    }

    let request = SeriesActionRequestV3::decode(family_request)
        .map_err(|_| SeriesArtifactErrorV3::Request)?;
    if request.template() != selection.template {
        return Err(SeriesArtifactErrorV3::ContentIdentity);
    }
    let slices = split_request(request, family_request)?;
    let selected_descriptor = set
        .select(slices.header)
        .map_err(|_| SeriesArtifactErrorV3::ProgramSet)?;
    if selected_descriptor.to_bytes() != digest(artifacts.descriptor) {
        return Err(SeriesArtifactErrorV3::ContentIdentity);
    }
    let descriptor = CapabilityProgramV3::decode(artifacts.descriptor)
        .map_err(|_| SeriesArtifactErrorV3::Descriptor)?;
    validate_descriptor(descriptor)?;

    require_content(
        descriptor.account_profile().to_bytes(),
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| SeriesArtifactErrorV3::AccountProfile)?;
    require_content(
        descriptor.request_profile_program().to_bytes(),
        artifacts.request_profile,
    )?;
    let request_profile = RequestProfileV1::decode_selected(
        descriptor.request_profile_program().to_bytes(),
        digest(artifacts.request_profile),
        artifacts.request_profile,
    )
    .map_err(|_| SeriesArtifactErrorV3::RequestProfile)?;
    validate_and_execute_header(request_profile, slices.header)?;

    let strategy_id = content_id(artifacts.strategy)?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| SeriesArtifactErrorV3::Strategy)?;
    strategy
        .validate_descriptor_selection(strategy_id, descriptor)
        .map_err(|_| SeriesArtifactErrorV3::Strategy)?;
    require_content(
        strategy.transition_program().to_bytes(),
        artifacts.transition,
    )?;
    if strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID {
        return Err(SeriesArtifactErrorV3::Transition);
    }
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| SeriesArtifactErrorV3::Transition)?;

    require_content(descriptor.effect_program().to_bytes(), artifacts.effect)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        digest(artifacts.effect),
        artifacts.effect,
    )
    .map_err(|_| SeriesArtifactErrorV3::Effect)?;
    validate_geometry(account_profile, request_profile, transition, effect)?;
    validate_routes(request.action(), effect)?;

    Ok(SeriesArtifactBundleV3 {
        request,
        slices,
        descriptor,
        account_profile,
        request_profile,
        strategy,
        transition,
        effect,
    })
}

/// Bind one post-strategy Consume invocation to the exact Series proof suffix.
///
/// `invocation` must come from the authenticated selected EffectProgram after
/// the common outer has executed RequestProfile and TransitionVM. This helper
/// deliberately receives no raw register index: the generic interpreter owns
/// that coordinate and [`ResolvedInvocationV3`] is its checked result.
pub fn validate_series_consume_invocation_v3<'a>(
    bundle: SeriesArtifactBundleV3<'_>,
    invocation: ResolvedInvocationV3,
    ir_request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<SeriesConsumeInvocationV3<'a>> {
    if bundle.request.action() != SeriesActionV3::Consume
        || invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
        || invocation.request_offset != SERIES_CONSUME_CORE_FOUND_OFFSET_V3
        || invocation.request_len != SERIES_CONSUME_CORE_REQUEST_BYTES_V3
        || !resolved_dependencies_match(
            bundle.effect,
            invocation,
            &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
        )
        || ir_request_bank.len() != SERIES_CONSUME_IR_REQUEST_BYTES_V3
        || family_request.get(..SERIES_ACTION_HEADER_BYTES_V3) != Some(bundle.slices.header)
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let core_request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let core_request = ir_request_bank
        .get(invocation.request_offset..core_request_end)
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    let borrowed = invocation
        .borrowed_witness
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    if borrowed.source_offset() != SERIES_ACTION_HEADER_BYTES_V3
        || borrowed.len() != bundle.slices.witness.len()
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let witness = borrowed
        .slice(family_request)
        .map_err(|_| SeriesArtifactErrorV3::Request)?;
    if witness != bundle.slices.witness
        || witness.len()
            != usize::from(bundle.request.proof_count())
                .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
                .ok_or(SeriesArtifactErrorV3::Geometry)?
    {
        return Err(SeriesArtifactErrorV3::Request);
    }
    Ok(SeriesConsumeInvocationV3 {
        core_request,
        witness,
        base_request_digest: hashv(&[core_request, witness]).to_bytes(),
    })
}

/// Bind the terminal Expire Core route to one exact permit request and proof.
pub fn validate_series_expire_invocation_v3<'a>(
    bundle: SeriesArtifactBundleV3<'_>,
    invocation: ResolvedInvocationV3,
    ir_request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<SeriesExpireInvocationV3<'a>> {
    if bundle.request.action() != SeriesActionV3::Expire
        || invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
        || invocation.request_offset != SERIES_EXPIRE_PERMIT_OFFSET_V3
        || invocation.request_len != SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3
        || invocation.receipt_dependency.is_some()
        || ir_request_bank.len() != SERIES_EXPIRE_IR_REQUEST_BYTES_V3
        || family_request.get(..SERIES_ACTION_HEADER_BYTES_V3) != Some(bundle.slices.header)
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let permit_request = ir_request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    dclutch_market_core_codec::SeriesPermitExpiryRequestV1::decode(permit_request)
        .map_err(|_| SeriesArtifactErrorV3::Effect)?;
    let borrowed = invocation
        .borrowed_witness
        .ok_or(SeriesArtifactErrorV3::Effect)?;
    if borrowed.source_offset() != SERIES_ACTION_HEADER_BYTES_V3
        || borrowed.len() != bundle.slices.witness.len()
    {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let witness = borrowed
        .slice(family_request)
        .map_err(|_| SeriesArtifactErrorV3::Request)?;
    if witness != bundle.slices.witness
        || witness.len()
            != usize::from(bundle.request.proof_count())
                .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
                .ok_or(SeriesArtifactErrorV3::Geometry)?
    {
        return Err(SeriesArtifactErrorV3::Request);
    }
    Ok(SeriesExpireInvocationV3 {
        permit_request,
        witness,
        child_request_digest: hashv(&[permit_request, witness]).to_bytes(),
    })
}

fn split_request<'a>(
    request: SeriesActionRequestV3<'_>,
    bytes: &'a [u8],
) -> Result<SeriesRequestSlicesV3<'a>> {
    let header = bytes
        .get(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    let witness = bytes
        .get(SERIES_ACTION_HEADER_BYTES_V3..)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    let expected = usize::from(request.proof_count())
        .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
        .ok_or(SeriesArtifactErrorV3::Request)?;
    if witness.len() != expected || witness != request.proof_bytes() {
        return Err(SeriesArtifactErrorV3::Request);
    }
    Ok(SeriesRequestSlicesV3 { header, witness })
}

fn validate_descriptor(descriptor: CapabilityProgramV3) -> Result<()> {
    if descriptor.kind().to_bytes() != digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)
        || descriptor.config_schema().to_bytes() != SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        || descriptor.request_schema().to_bytes() != digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)
        || descriptor.root_schema().to_bytes() != digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)
        || descriptor.derivation_policy().to_bytes() != digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| SeriesArtifactErrorV3::Geometry)?
            != SERIES_STATE_BYTES_V3
    {
        return Err(SeriesArtifactErrorV3::Descriptor);
    }
    Ok(())
}

fn validate_and_execute_header(profile: RequestProfileV1<'_>, header: &[u8]) -> Result<()> {
    if profile
        .request_bytes(0)
        .map_err(|_| SeriesArtifactErrorV3::RequestProfile)?
        != SERIES_ACTION_HEADER_BYTES_V3
        || profile.item_request_bytes() != 0
        || profile.item_scalar_stride() != 0
        || profile.item_identity_stride() != 0
    {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    let scalars = usize::from(profile.common_scalar_count());
    let identities = usize::from(profile.common_identity_count());
    if scalars > MAX_SCALARS || identities > MAX_IDENTITIES {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    let input_scalars = [0_u64; MAX_SCALARS];
    let input_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut scratch_scalars = [0_u64; MAX_SCALARS];
    let mut scratch_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut output_scalars = [0_u64; MAX_SCALARS];
    let mut output_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let input_scalars = input_scalars
        .get(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let input_identities = input_identities
        .get(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let scratch_scalars = scratch_scalars
        .get_mut(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let scratch_identities = scratch_identities
        .get_mut(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let output_scalars = output_scalars
        .get_mut(..scalars)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    let output_identities = output_identities
        .get_mut(..identities)
        .ok_or(SeriesArtifactErrorV3::Geometry)?;
    project_atomic(
        profile,
        0,
        header,
        ProjectionRegistersV1 {
            input_scalars,
            input_identities,
            scratch_scalars,
            scratch_identities,
            output_scalars,
            output_identities,
        },
    )
    .map_err(|_| SeriesArtifactErrorV3::RequestProfile)
}

fn validate_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    let common_scalars = account.common_scalar_count();
    let common_identities = account.common_identity_count();
    if account.item_account_stride() != 0
        || account.item_scalar_stride() != 0
        || account.item_identity_stride() != 0
        || request.common_scalar_count() != common_scalars
        || request.common_identity_count() != common_identities
        || transition.common_scalar_count() != common_scalars
        || transition.common_identity_count() != common_identities
        || transition.item_scalar_stride() != 0
        || transition.item_identity_stride() != 0
        || effect.fixed_account_count() != account.fixed_account_count()
        || effect.item_account_stride() != 0
        || effect.common_scalar_count() != common_scalars
        || effect.common_identity_count() != common_identities
        || effect.item_scalar_stride() != 0
        || effect.item_identity_stride() != 0
        || effect.item_operation_count() != 0
    {
        return Err(SeriesArtifactErrorV3::Geometry);
    }
    Ok(())
}

fn validate_routes(action: SeriesActionV3, effect: EffectProgramV3<'_>) -> Result<()> {
    let count = match action {
        SeriesActionV3::Prepare => u16::try_from(SERIES_PREPARE_ROUTE_COUNT_V3)
            .map_err(|_| SeriesArtifactErrorV3::Geometry)?,
        SeriesActionV3::Consume => u16::try_from(SERIES_CONSUME_ROUTE_COUNT_V3)
            .map_err(|_| SeriesArtifactErrorV3::Geometry)?,
        SeriesActionV3::Expire => u16::try_from(SERIES_EXPIRE_ROUTE_COUNT_V3)
            .map_err(|_| SeriesArtifactErrorV3::Geometry)?,
        SeriesActionV3::Retire | SeriesActionV3::Close => 0,
    };
    if effect.route_count() != count {
        return Err(SeriesArtifactErrorV3::Effect);
    }
    let funding_state_count = if action == SeriesActionV3::Consume {
        let found = effect.route(1).map_err(|_| SeriesArtifactErrorV3::Effect)?;
        let count = found
            .fixed_account_count()
            .checked_sub(SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3)
            .ok_or(SeriesArtifactErrorV3::Geometry)?;
        if count == 0 || count > SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3 {
            return Err(SeriesArtifactErrorV3::Geometry);
        }
        count
    } else {
        0
    };
    let mut index = 0_u16;
    while index < count {
        let route = effect
            .route(index)
            .map_err(|_| SeriesArtifactErrorV3::Effect)?;
        let (expected_role, expected_width, borrows_witness, expected_accounts, expected_receipts) =
            match action {
                SeriesActionV3::Prepare => match index {
                    0 => (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    1 => (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    2 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    3 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    4 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    _ => return Err(SeriesArtifactErrorV3::Effect),
                },
                SeriesActionV3::Consume => match index {
                    0 => (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    1 => (
                        FixedRole::Core,
                        SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                        true,
                        SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3.checked_add(funding_state_count),
                        &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    2 => (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    3 => (
                        FixedRole::Claims,
                        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3),
                        &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    4 => (
                        FixedRole::Core,
                        SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                        true,
                        Some(SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3),
                        &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    _ => return Err(SeriesArtifactErrorV3::Effect),
                },
                SeriesActionV3::Expire => match index {
                    0 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    1 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    2 => (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    3 => (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        false,
                        Some(SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    4 => (
                        FixedRole::Core,
                        SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3,
                        true,
                        Some(SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3),
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
                    ),
                    _ => return Err(SeriesArtifactErrorV3::Effect),
                },
                SeriesActionV3::Retire | SeriesActionV3::Close => {
                    return Err(SeriesArtifactErrorV3::Effect);
                }
            };
        if route.role() != expected_role
            || route.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once
            || usize::try_from(route.fixed_request_bytes())
                .map_err(|_| SeriesArtifactErrorV3::Geometry)?
                != expected_width
            || expected_accounts.is_some_and(|accounts| route.fixed_account_count() != accounts)
            || route.item_request_bytes() != 0
            || route.borrows_witness() != borrows_witness
            || !route_dependencies_match(effect, index, expected_receipts)
        {
            return Err(SeriesArtifactErrorV3::Effect);
        }
        index = index
            .checked_add(1)
            .ok_or(SeriesArtifactErrorV3::Geometry)?;
    }
    Ok(())
}

fn route_dependencies_match(
    effect: EffectProgramV3<'_>,
    route_index: u16,
    expected: &[RouteReceiptDependencyV3],
) -> bool {
    let Ok(route) = effect.route(route_index) else {
        return false;
    };
    if usize::from(route.receipt_dependency_count()) != expected.len() {
        return false;
    }
    let mut dependency_index = 0_u16;
    while usize::from(dependency_index) < expected.len() {
        if effect.route_receipt_dependency(route_index, dependency_index)
            != expected
                .get(usize::from(dependency_index))
                .copied()
                .ok_or(dclutch_effect_kernel::v3::Error::InvalidReceiptDependency)
        {
            return false;
        }
        let Some(next) = dependency_index.checked_add(1) else {
            return false;
        };
        dependency_index = next;
    }
    true
}

pub(crate) fn resolved_dependencies_match(
    effect: EffectProgramV3<'_>,
    invocation: ResolvedInvocationV3,
    expected: &[RouteReceiptDependencyV3],
) -> bool {
    if usize::from(invocation.receipt_dependencies.len()) != expected.len() {
        return false;
    }
    let mut dependency_index = 0_u16;
    while usize::from(dependency_index) < expected.len() {
        let Ok(resolved) =
            effect.resolved_receipt_dependency(invocation.receipt_dependencies, dependency_index)
        else {
            return false;
        };
        let Some(expected_dependency) = expected.get(usize::from(dependency_index)) else {
            return false;
        };
        if resolved.producer_role != expected_dependency.producer_role()
            || resolved.producer_route != expected_dependency.producer_route()
            || resolved.producer_invocation != 0
            || resolved.expected_receipt_bytes != expected_dependency.expected_receipt_bytes()
        {
            return false;
        }
        let Some(next) = dependency_index.checked_add(1) else {
            return false;
        };
        dependency_index = next;
    }
    true
}

fn require_selected(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    if selected == [0; 32] || selected != digest(bytes) {
        Err(SeriesArtifactErrorV3::ContentIdentity)
    } else {
        Ok(())
    }
}

fn require_content(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    require_selected(selected, bytes)
}

fn content_id(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(digest(bytes)).map_err(|_| SeriesArtifactErrorV3::ContentIdentity)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES;
    use dclutch_effect_kernel::v3::encode::{
        EffectGeometryV3, RouteInputV3, encode_effect_program_v4_atomic,
    };
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        StrategyDispositionV2,
    };
    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::{vec, vec::Vec};

    use super::*;
    use crate::series::instruction::encode_series_action_header_v3;

    const FIXTURE_SCALARS: u16 = 4;
    const REQUEST_OPERATIONS: u16 = 2;
    const TRANSITION_OPERATIONS: u16 = 3;

    struct Fixture {
        set: Vec<u8>,
        descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
        account: Vec<u8>,
        request_profile: Vec<u8>,
        strategy:
            [u8; dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
        transition: Vec<u8>,
        effect: Vec<u8>,
        request: Vec<u8>,
        template: ContentId,
    }

    impl Fixture {
        fn artifacts(&self) -> SeriesArtifactBytesV3<'_> {
            SeriesArtifactBytesV3 {
                program_set: &self.set,
                descriptor: &self.descriptor,
                account_profile: &self.account,
                request_profile: &self.request_profile,
                strategy: &self.strategy,
                transition: &self.transition,
                effect: &self.effect,
            }
        }

        fn selection(&self) -> SeriesArtifactSelectionV3 {
            SeriesArtifactSelectionV3 {
                program_set: digest(&self.set),
                template: self.template,
            }
        }
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture slice")
            .copy_from_slice(value);
    }

    fn set_byte(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("nonzero fixture identity")
    }

    fn core_id(value: u8) -> dclutch_market_core_codec::Identity {
        dclutch_market_core_codec::Identity::new([value; 32]).expect("nonzero Core identity")
    }

    fn permit_expiry_bytes() -> [u8; SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3] {
        let intent = dclutch_market_core_codec::FoundingIntentV5::new(
            255,
            core_id(1),
            core_id(2),
            core_id(3),
            core_id(4),
            core_id(5),
            core_id(6),
            core_id(7),
            core_id(8),
            core_id(9),
            core_id(10),
            core_id(11),
            core_id(12),
            core_id(13),
            core_id(14),
            core_id(15),
            1,
            1,
            1,
            1,
            4,
            1,
        )
        .expect("founding intent");
        let permit = dclutch_market_core_codec::SeriesFoundingPermitV1::new(
            intent,
            core_id(16),
            core_id(17),
        )
        .expect("founding permit");
        dclutch_market_core_codec::SeriesPermitExpiryRequestV1::new(permit)
            .encode()
            .expect("permit expiry request")
    }

    fn fixture_fixed_accounts(action: SeriesActionV3) -> u16 {
        match action {
            SeriesActionV3::Prepare => SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3,
            SeriesActionV3::Consume => SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3 + 1,
            SeriesActionV3::Expire => SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3,
            SeriesActionV3::Retire | SeriesActionV3::Close => 1,
        }
    }

    fn account_profile(action: SeriesActionV3) -> Vec<u8> {
        let fixed_accounts = fixture_fixed_accounts(action);
        let mut output = vec![
            0_u8;
            dclutch_account_profile_contract::v2::HEADER_BYTES
                + usize::from(fixed_accounts)
                    * dclutch_account_profile_contract::v2::RULE_BYTES
        ];
        put(&mut output, 0, &dclutch_account_profile_contract::v2::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_account_profile_contract::v2::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_account_profile_contract::v2::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(&mut output, 12, &fixed_accounts.to_le_bytes());
        put(&mut output, 20, &FIXTURE_SCALARS.to_le_bytes());
        output
    }

    fn request_profile(action: SeriesActionV3) -> Vec<u8> {
        let mut output = vec![
            0_u8;
            dclutch_request_profile_contract::HEADER_BYTES
                + usize::from(REQUEST_OPERATIONS)
                    * dclutch_request_profile_contract::OPERATION_BYTES
        ];
        put(&mut output, 0, &dclutch_request_profile_contract::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_request_profile_contract::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_request_profile_contract::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(
            &mut output,
            12,
            &u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .expect("request width")
                .to_le_bytes(),
        );
        put(&mut output, 20, &REQUEST_OPERATIONS.to_le_bytes());
        put(&mut output, 24, &FIXTURE_SCALARS.to_le_bytes());

        let require_action = dclutch_request_profile_contract::HEADER_BYTES;
        set_byte(&mut output, require_action, 0);
        put(
            &mut output,
            require_action + 4,
            &SERIES_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        put(
            &mut output,
            require_action + 12,
            &u64::from(action as u8).to_le_bytes(),
        );

        let project_proof_count =
            require_action + dclutch_request_profile_contract::OPERATION_BYTES;
        set_byte(&mut output, project_proof_count, 5);
        put(&mut output, project_proof_count + 4, &13_u32.to_le_bytes());
        put(&mut output, project_proof_count + 8, &2_u16.to_le_bytes());
        output
    }

    fn transition() -> Vec<u8> {
        let mut output = vec![
            0_u8;
            dclutch_transition_vm::v3::HEADER_BYTES
                + usize::from(TRANSITION_OPERATIONS)
                    * dclutch_transition_vm::v3::INSTRUCTION_BYTES
        ];
        put(&mut output, 0, &dclutch_transition_vm::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_transition_vm::v3::VERSION);
        put(&mut output, 6, &TRANSITION_OPERATIONS.to_le_bytes());
        put(&mut output, 12, &FIXTURE_SCALARS.to_le_bytes());

        let load_offset = dclutch_transition_vm::v3::HEADER_BYTES;
        set_byte(&mut output, load_offset, 0);
        put(&mut output, load_offset + 2, &0_u16.to_le_bytes());
        put(
            &mut output,
            load_offset + 16,
            &u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .expect("header width")
                .to_le_bytes(),
        );

        let load_multiplier = load_offset + dclutch_transition_vm::v3::INSTRUCTION_BYTES;
        set_byte(&mut output, load_multiplier, 0);
        put(&mut output, load_multiplier + 2, &3_u16.to_le_bytes());
        put(
            &mut output,
            load_multiplier + 16,
            &u64::try_from(SERIES_WITNESS_ITEM_BYTES_V3)
                .expect("sibling width")
                .to_le_bytes(),
        );

        let multiply = load_multiplier + dclutch_transition_vm::v3::INSTRUCTION_BYTES;
        set_byte(&mut output, multiply, 17);
        put(&mut output, multiply + 2, &2_u16.to_le_bytes());
        put(&mut output, multiply + 4, &3_u16.to_le_bytes());
        put(&mut output, multiply + 6, &1_u16.to_le_bytes());
        output
    }

    fn effect(action: SeriesActionV3) -> Vec<u8> {
        let request_bytes = match action {
            SeriesActionV3::Prepare => SERIES_PREPARE_IR_REQUEST_BYTES_V3,
            SeriesActionV3::Consume => SERIES_CONSUME_IR_REQUEST_BYTES_V3,
            SeriesActionV3::Expire => SERIES_EXPIRE_IR_REQUEST_BYTES_V3,
            SeriesActionV3::Retire | SeriesActionV3::Close => 0,
        };
        let request_bank = vec![0_u8; request_bytes];
        let route_specs: &[(FixedRole, usize, u16, bool, &[RouteReceiptDependencyV3])] =
            match action {
                SeriesActionV3::Prepare => &[
                    (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                ],
                SeriesActionV3::Consume => &[
                    (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Core,
                        SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                        SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3 + 1,
                        true,
                        &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Claims,
                        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
                        SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Core,
                        SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                        SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
                        true,
                        &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3,
                    ),
                ],
                SeriesActionV3::Expire => &[
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Custody,
                        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                        SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3,
                        false,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                    (
                        FixedRole::Core,
                        SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3,
                        SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3,
                        true,
                        &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                    ),
                ],
                SeriesActionV3::Retire | SeriesActionV3::Close => &[],
            };
        let mut routes = Vec::with_capacity(route_specs.len());
        let mut dependencies = Vec::with_capacity(route_specs.len());
        let mut cursor = 0_usize;
        for (role, width, account_count, borrows_witness, route_dependencies) in route_specs {
            let end = cursor.checked_add(*width).expect("request width");
            routes.push(RouteInputV3 {
                role: *role,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: borrows_witness.then_some(0),
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: *account_count,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: request_bank.get(cursor..end).expect("request route"),
                item_request: &[],
            });
            dependencies.push(*route_dependencies);
            cursor = end;
        }
        assert_eq!(cursor, request_bank.len());
        let dependency_count = dependencies.iter().map(|items| items.len()).sum::<usize>();
        let output_bytes = dclutch_effect_kernel::v3::HEADER_BYTES
            + routes.len() * dclutch_effect_kernel::v3::ROUTE_BYTES
            + dependency_count * dclutch_effect_kernel::v3::RECEIPT_DEPENDENCY_BYTES
            + request_bytes;
        let mut scratch = vec![0_u8; output_bytes];
        let mut output = vec![0_u8; output_bytes];
        encode_effect_program_v4_atomic(
            EffectGeometryV3 {
                fixed_accounts: fixture_fixed_accounts(action),
                item_account_stride: 0,
                common_scalars: FIXTURE_SCALARS,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &routes,
            &dependencies,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("canonical EffectProgram V4 fixture");
        output
    }

    fn program_set(action: SeriesActionV3, descriptor: [u8; 32]) -> Vec<u8> {
        let mut output = vec![0_u8; 72];
        put(&mut output, 0, b"DCLTCPS1");
        put(&mut output, 8, &1_u16.to_le_bytes());
        put(&mut output, 10, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &SERIES_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        set_byte(&mut output, 16, 1);
        put(&mut output, 18, &1_u16.to_le_bytes());
        put(&mut output, 32, &u32::from(action as u8).to_le_bytes());
        put(&mut output, 36, &descriptor);
        output
    }

    fn family_request(action: SeriesActionV3, template: ContentId) -> Vec<u8> {
        let occurrence = action.occurrence_bound().then_some(id([42; 32]));
        let ticket = (action != SeriesActionV3::Close).then_some(id([43; 32]));
        let proof_count = u8::from(action.occurrence_bound()) * 2;
        let ticket_revision = match action {
            SeriesActionV3::Prepare | SeriesActionV3::Close => 0,
            SeriesActionV3::Consume | SeriesActionV3::Expire | SeriesActionV3::Retire => 3,
        };
        let header = encode_series_action_header_v3(
            action,
            template,
            occurrence,
            ticket,
            7,
            ticket_revision,
            proof_count,
        )
        .expect("Series header");
        let mut output = vec![0_u8; header.len() + usize::from(proof_count) * 32];
        output
            .get_mut(..header.len())
            .expect("header destination")
            .copy_from_slice(&header);
        for (index, value) in output
            .get_mut(header.len()..)
            .expect("witness destination")
            .iter_mut()
            .enumerate()
        {
            *value = u8::try_from(index + 1).expect("bounded witness byte");
        }
        output
    }

    fn fixture(action: SeriesActionV3) -> Fixture {
        let template = id([41; 32]);
        let account = account_profile(action);
        let request_profile = request_profile(action);
        let transition = transition();
        let effect = effect(action);
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::Interpreted,
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            None,
            id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            None,
            id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("interpreted strategy")
        .to_bytes();
        let descriptor = CapabilityProgramV3::new(
            id(digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)),
            id(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3),
            id(digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)),
            id(digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)),
            id(digest(&account)),
            id(digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)),
            id([90; 32]),
            id(digest(&effect)),
            id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
            id(digest(&request_profile)),
            id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
            id(digest(&strategy)),
            u32::try_from(SERIES_STATE_BYTES_V3).expect("root width"),
        )
        .expect("Series descriptor")
        .encode();
        Fixture {
            set: program_set(action, digest(&descriptor)),
            descriptor,
            account,
            request_profile,
            strategy,
            transition,
            effect,
            request: family_request(action, template),
            template,
        }
    }

    fn projected_scalars(fixture: &Fixture, bundle: SeriesArtifactBundleV3<'_>) -> [u64; 4] {
        let input_scalars = [0_u64; 4];
        let input_identities: [[u8; 32]; 0] = [];
        let mut profile_scratch = [0_u64; 4];
        let mut profile_output = [0_u64; 4];
        let mut identity_scratch: [[u8; 32]; 0] = [];
        let mut identity_output: [[u8; 32]; 0] = [];
        project_atomic(
            bundle.request_profile,
            0,
            bundle.slices.header,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut profile_scratch,
                scratch_identities: &mut identity_scratch,
                output_scalars: &mut profile_output,
                output_identities: &mut identity_output,
            },
        )
        .expect("request projection");
        let mut transition_scratch = [0_u64; 4];
        let mut transition_output = [0_u64; 4];
        let mut transition_identity_scratch: [[u8; 32]; 0] = [];
        let mut transition_identity_output: [[u8; 32]; 0] = [];
        execute_fold_atomic(
            bundle.transition,
            0,
            RegisterInput {
                scalars: &profile_output,
                identities: &identity_output,
            },
            RegisterOutput {
                scalars: &mut transition_scratch,
                identities: &mut transition_identity_scratch,
            },
            RegisterOutput {
                scalars: &mut transition_output,
                identities: &mut transition_identity_output,
            },
        )
        .expect("strategy transition");
        assert_eq!(
            usize::try_from(*transition_output.get(1).expect("witness-length register"))
                .expect("witness len"),
            fixture.request.len() - SERIES_ACTION_HEADER_BYTES_V3
        );
        transition_output
    }

    #[test]
    fn every_series_action_joins_one_exact_program_set_bundle() {
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            let fixture = fixture(action);
            let joined = authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &fixture.request,
            )
            .expect("joined Series artifact bundle");
            assert_eq!(joined.request.action(), action);
            assert_eq!(joined.slices.header.len(), SERIES_ACTION_HEADER_BYTES_V3);
            assert_eq!(
                joined.slices.witness.len(),
                usize::from(joined.request.proof_count()) * 32
            );
            let _ = projected_scalars(&fixture, joined);
        }
    }

    #[test]
    fn consume_borrows_only_the_exact_authenticated_proof_suffix() {
        let fixture = fixture(SeriesActionV3::Consume);
        let bundle = authenticate_series_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &fixture.request,
        )
        .expect("Consume bundle");
        let scalars = projected_scalars(&fixture, bundle);
        let identities: [[u8; 32]; 0] = [];
        let invocation = bundle
            .effect
            .resolved_invocation(1, 0, 0, &scalars, &identities)
            .expect("resolved Core invocation");
        let mut request_bank = vec![0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        request_bank
            .get_mut(
                SERIES_CONSUME_CORE_FOUND_OFFSET_V3
                    ..SERIES_CONSUME_CORE_FOUND_OFFSET_V3 + SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            )
            .expect("Core request region")
            .fill(17);
        let selected = validate_series_consume_invocation_v3(
            bundle,
            invocation,
            &request_bank,
            &fixture.request,
        )
        .expect("exact borrowed witness");
        assert_eq!(
            selected.core_request,
            &[17; SERIES_CONSUME_CORE_REQUEST_BYTES_V3]
        );
        assert_eq!(
            selected.witness,
            fixture
                .request
                .get(SERIES_ACTION_HEADER_BYTES_V3..)
                .expect("witness")
        );
        assert_eq!(
            selected.base_request_digest,
            hashv(&[selected.core_request, selected.witness]).to_bytes()
        );

        let mut padded = fixture.request.clone();
        padded.push(0);
        assert_eq!(
            validate_series_consume_invocation_v3(bundle, invocation, &request_bank, &padded,),
            Err(SeriesArtifactErrorV3::Request)
        );
    }

    #[test]
    fn consume_receipt_chain_is_exact_backward_and_typed() {
        let fixture = fixture(SeriesActionV3::Consume);
        let program = EffectProgramV3::decode(&fixture.effect).expect("Consume EffectProgram");
        assert_eq!(
            program.route(0).expect("Lock route").receipt_dependency(),
            None
        );
        assert_eq!(
            program
                .route(1)
                .expect("Core Found route")
                .receipt_dependency(),
            Some(RouteReceiptDependencyV3::new(
                FixedRole::Custody,
                0,
                SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3,
            ))
        );
        assert_eq!(
            program
                .route(2)
                .expect("Realize route")
                .receipt_dependency(),
            None
        );
        assert_eq!(
            program
                .route(3)
                .expect("Claims route")
                .receipt_dependency_count(),
            2
        );
        assert_eq!(
            program
                .route_receipt_dependency(3, 0)
                .expect("Claims Lock dependency"),
            RouteReceiptDependencyV3::new(
                FixedRole::Custody,
                0,
                SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3,
            )
        );
        assert_eq!(
            program
                .route_receipt_dependency(3, 1)
                .expect("Claims Realize dependency"),
            RouteReceiptDependencyV3::new(
                FixedRole::Custody,
                2,
                SERIES_CONSUME_REALIZE_RECEIPT_BYTES_V3,
            )
        );
        assert_eq!(
            program
                .route(4)
                .expect("Core Open route")
                .receipt_dependency(),
            Some(RouteReceiptDependencyV3::new(
                FixedRole::Claims,
                3,
                SERIES_CONSUME_CLAIMS_RECEIPT_BYTES_V3,
            ))
        );

        let dependency_table = dclutch_effect_kernel::v3::HEADER_BYTES
            + SERIES_CONSUME_ROUTE_COUNT_V3 * dclutch_effect_kernel::v3::ROUTE_BYTES;
        let mut wrong_width = fixture.effect.clone();
        put(
            &mut wrong_width,
            dependency_table + 4,
            &(SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3 + 1).to_le_bytes(),
        );
        let wrong = EffectProgramV3::decode(&wrong_width).expect("canonical wrong-width route");
        assert_eq!(
            validate_routes(SeriesActionV3::Consume, wrong),
            Err(SeriesArtifactErrorV3::Effect)
        );

        let mut forward = fixture.effect.clone();
        put(
            &mut forward,
            dependency_table + dclutch_effect_kernel::v3::RECEIPT_DEPENDENCY_BYTES + 2,
            &4_u16.to_le_bytes(),
        );
        assert!(EffectProgramV3::decode(&forward).is_err());
    }

    #[test]
    fn expire_borrows_proof_only_after_exact_permit_candidate() {
        let fixture = fixture(SeriesActionV3::Expire);
        let bundle = authenticate_series_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &fixture.request,
        )
        .expect("Expire bundle");
        let scalars = projected_scalars(&fixture, bundle);
        let identities: [[u8; 32]; 0] = [];
        let invocation = bundle
            .effect
            .resolved_invocation(4, 0, 0, &scalars, &identities)
            .expect("resolved permit-refund invocation");
        let permit = permit_expiry_bytes();
        let mut request_bank = vec![0_u8; SERIES_EXPIRE_IR_REQUEST_BYTES_V3];
        request_bank
            .get_mut(
                SERIES_EXPIRE_PERMIT_OFFSET_V3
                    ..SERIES_EXPIRE_PERMIT_OFFSET_V3 + SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3,
            )
            .expect("permit request region")
            .copy_from_slice(&permit);
        let selected = validate_series_expire_invocation_v3(
            bundle,
            invocation,
            &request_bank,
            &fixture.request,
        )
        .expect("exact permit plus occurrence proof");
        assert_eq!(selected.permit_request, permit);
        assert_eq!(
            selected.witness,
            fixture
                .request
                .get(SERIES_ACTION_HEADER_BYTES_V3..)
                .expect("proof")
        );
        assert_eq!(
            selected.child_request_digest,
            hashv(&[selected.permit_request, selected.witness]).to_bytes()
        );

        *request_bank
            .get_mut(SERIES_EXPIRE_PERMIT_OFFSET_V3)
            .expect("magic byte") ^= 1;
        assert_eq!(
            validate_series_expire_invocation_v3(
                bundle,
                invocation,
                &request_bank,
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::Effect)
        );
    }

    #[test]
    fn action_descriptor_profile_and_witness_substitution_refuse() {
        let fixture = fixture(SeriesActionV3::Prepare);
        let mut wrong_selection = fixture.selection();
        *wrong_selection
            .program_set
            .get_mut(0)
            .expect("selection mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                wrong_selection,
                fixture.artifacts(),
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_descriptor = fixture.descriptor;
        *wrong_descriptor.get_mut(64).expect("descriptor mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                SeriesArtifactBytesV3 {
                    descriptor: &wrong_descriptor,
                    ..fixture.artifacts()
                },
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_profile = fixture.request_profile.clone();
        *wrong_profile.get_mut(36).expect("profile mutation") ^= 1;
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                SeriesArtifactBytesV3 {
                    request_profile: &wrong_profile,
                    ..fixture.artifacts()
                },
                &fixture.request,
            ),
            Err(SeriesArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_action = fixture.request.clone();
        *wrong_action.get_mut(12).expect("action mutation") = SeriesActionV3::Expire as u8;
        assert!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &wrong_action,
            )
            .is_err()
        );

        let mut short_witness = fixture.request.clone();
        short_witness.pop();
        assert_eq!(
            authenticate_series_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &short_witness,
            ),
            Err(SeriesArtifactErrorV3::Request)
        );
    }
}
