//! Canonical current-source Series Prepare ProfileV3/EffectV5 artifacts.
//!
//! Prepare owns exactly one lifecycle transition: creation of the vacant
//! Ticket PDA at coordinate five. The five child routes start after the Hot
//! prefix and Ticket, use the current thirteen-account Custody replay frame,
//! and never receive the Ticket. FundingV5 derives the Ticket only from the
//! existing Ticket domain, authenticated root key, authenticated Ticket
//! content identity, and the adapter-derived canonical bump.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_vm::account_profile::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::{
        AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES,
        RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        },
    },
    v3::{
        AccountProfileV3, FUNDING_BOUND_BYTES_V3, FundingActionMaskV3, FundingBoundV3,
        HEADER_BYTES_V3, encode_account_profile_v3_atomic,
    },
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_MARKET_OFFSET,
    CAPABILITY_ROOT_SELECTION_OFFSET,
    hot_v3::{HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3},
};
use dclutch_custody::ProjectedCustodyRequestLayoutV1;
use dclutch_vm::effect::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, OPERATION_BYTES as EFFECT_OPERATION_BYTES_V3,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES_V3, RouteKindV3,
        encode::{
            EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3, RequestSpaceV3,
            RouteInputV3, encode_effect_program_v4_atomic,
        },
    },
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4,
        HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, ProgramV4, RequestCoordinateV4,
        SEMANTIC_RANGE_ROUTE_V4, encode_program_v4_atomic,
    },
    v5::{
        FUNDING_ACTION_BYTES_V5, FUNDING_SEED_BYTES_V5, FundingActionV5, FundingOperationV5,
        FundingSeedV5, HEADER_BYTES_V5 as EFFECT_HEADER_BYTES_V5, ProgramV5,
        encode_program_v5_atomic,
    },
};
use dclutch_product::{PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES};
use dclutch_registry::release_set::{
    CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
};
use dclutch_vm::request_profile::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3, ScalarRegisterV3,
    encode_program_atomic,
};

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4, SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
        SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4, SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
        SERIES_CONSUME_ROOT_KIND_IDENTITY_V4, SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
        SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
    },
    artifacts_v3::{
        SERIES_NO_RECEIPT_DEPENDENCIES_V3, SERIES_PREPARE_IR_REQUEST_BYTES_V3,
        SERIES_WITNESS_ITEM_BYTES_V3,
    },
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionV3},
    lifecycle_policy_v5::{
        SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5, SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5,
        encode_series_empty_state_lifecycle_v5_atomic,
    },
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    state::{SERIES_TICKET_STATE_BYTES_V3, SERIES_TICKET_STATE_PDA_DOMAIN_V3},
};

/// Vacant Ticket state, the sole funding-owned Prepare coordinate.
pub const SERIES_PREPARE_TICKET_COORDINATE_V5: u16 = 5;
/// Gap-free starts after the five-account Hot prefix and outer Ticket.
pub const SERIES_PREPARE_ROUTE_STARTS_V5: [u16; 5] = [6, 53, 68, 81, 97];
/// Current child-owned route widths.
pub const SERIES_PREPARE_ROUTE_COUNTS_V5: [u16; 5] = [47, 15, 13, 16, 14];
/// Complete logical account width before exact route-alias compaction.
pub const SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5: u16 = 111;
/// Complete physical account width after exact route-alias compaction.
pub const SERIES_PREPARE_PHYSICAL_ACCOUNT_COUNT_V5: u16 = 55;
/// Exact payer representative shared by every creating child route.
pub const SERIES_PREPARE_PAYER_COORDINATE_V5: u16 = 14;
/// Distinct immutable surplus-refund destination.
pub const SERIES_PREPARE_REFUND_COORDINATE_V5: u16 = 80;
/// Exact executable System-program representative.
pub const SERIES_PREPARE_SYSTEM_COORDINATE_V5: u16 = 16;

/// Common scalar carrying the exact Ticket rent target.
pub const SERIES_PREPARE_TICKET_RENT_SCALAR_V5: u16 = 7;
/// Common identity carrying the authenticated outer root key.
pub const SERIES_PREPARE_ROOT_KEY_IDENTITY_V5: u16 = 6;
/// Common identity carrying the request-selected Ticket content identity.
pub const SERIES_PREPARE_TICKET_IDENTITY_V5: u16 = 7;
/// Common identity carrying the immutable Template refund owner.
pub const SERIES_PREPARE_REFUND_OWNER_IDENTITY_V5: u16 = 8;
/// Current Trading program identity register.
pub const SERIES_PREPARE_TRADING_PROGRAM_IDENTITY_V5: u16 = 0;

/// Complete common scalar register width.
pub const SERIES_PREPARE_COMMON_SCALAR_COUNT_V5: u16 = 8;
/// Complete common identity register width.
pub const SERIES_PREPARE_COMMON_IDENTITY_COUNT_V5: u16 = 9;

const ACTION_OFFSET: u32 = 12;
const PROOF_COUNT_OFFSET: u32 = 13;
const REQUEST_TICKET_OFFSET: u32 = 80;
const PROOF_OFFSET: u32 = 128;
const PROFILE_OPERATIONS: usize = 11;
const REQUEST_OPERATIONS: usize = 3;
const TRANSITION_OPERATIONS: usize = 4;
const EFFECT_OPERATIONS: usize = 2;

/// Exact embedded AccountProfileV2 width.
pub const SERIES_PREPARE_BASE_ACCOUNT_PROFILE_BYTES_V5: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize * RULE_BYTES
    + PROFILE_OPERATIONS * OPERATION_BYTES;
/// Exact funding-refined AccountProfileV3 width.
pub const SERIES_PREPARE_ACCOUNT_PROFILE_BYTES_V5: usize =
    HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + SERIES_PREPARE_BASE_ACCOUNT_PROFILE_BYTES_V5;
/// Exact RequestProfile width.
pub const SERIES_PREPARE_REQUEST_PROFILE_BYTES_V5: usize =
    REQUEST_HEADER_BYTES + REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES;
/// Exact Transition program width.
pub const SERIES_PREPARE_TRANSITION_BYTES_V5: usize =
    TRANSITION_HEADER_BYTES + TRANSITION_OPERATIONS * TRANSITION_INSTRUCTION_BYTES;
/// Exact five-route embedded EffectV3 width.
pub const SERIES_PREPARE_BASE_EFFECT_BYTES_V5: usize = EFFECT_HEADER_BYTES_V3
    + 5 * EFFECT_ROUTE_BYTES_V3
    + EFFECT_OPERATIONS * EFFECT_OPERATION_BYTES_V3
    + SERIES_PREPARE_IR_REQUEST_BYTES_V3;
/// Exact proof-borrowing EffectV4 width.
pub const SERIES_PREPARE_EFFECT_V4_BYTES_V5: usize =
    SERIES_PREPARE_BASE_EFFECT_BYTES_V5 + EFFECT_HEADER_BYTES_V4 + BORROWED_RANGE_BYTES_V4;
/// Exact one-Create/four-seed EffectV5 width.
pub const SERIES_PREPARE_EFFECT_BYTES_V5: usize = EFFECT_HEADER_BYTES_V5
    + FUNDING_ACTION_BYTES_V5
    + 4 * FUNDING_SEED_BYTES_V5
    + SERIES_PREPARE_EFFECT_V4_BYTES_V5;

const _: () = assert!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 == 5);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V5[4] + SERIES_PREPARE_ROUTE_COUNTS_V5[4] == 111);
const _: () = assert!(ROUTE_ALIASES.len() == 56);
const _: () = assert!(SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 - ROUTE_ALIASES.len() as u16 == 55);

#[derive(Clone, Copy, Debug)]
/// Exact observed widths used by the physical Prepare profile.
pub struct SeriesPrepareAccountProfileInputV5<'a> {
    /// Data length at every logical fixed coordinate before alias compaction.
    pub fixed_data_lengths: &'a [u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Complete current-source Prepare artifact bundle.
pub struct SeriesPrepareFundingArtifactsV5 {
    /// Funding-refined ProfileV3 bytes.
    pub account_profile: Vec<u8>,
    /// Exact Prepare RequestProfile bytes.
    pub request_profile: Vec<u8>,
    /// Exact proof-width and Ticket-rent Transition bytes.
    pub transition: Vec<u8>,
    /// Five-route, one-Create EffectV5 bytes.
    pub effect: Vec<u8>,
    /// Exact plan-free lifecycle bytes.
    pub lifecycle: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Stable refusal from Prepare artifact emission.
pub enum SeriesPrepareFundingArtifactErrorV5 {
    /// Exact geometry or scalar input differed.
    Geometry,
    /// AccountProfile emission or hostile decode refused.
    AccountProfile,
    /// RequestProfile emission or hostile decode refused.
    RequestProfile,
    /// Transition emission or hostile decode refused.
    Transition,
    /// Effect emission or hostile decode refused.
    Effect,
    /// Empty lifecycle emission or hostile decode refused.
    Lifecycle,
}

type Result<T> = core::result::Result<T, SeriesPrepareFundingArtifactErrorV5>;

/// Emit and hostile-decode the complete current-source Prepare artifacts.
pub fn emit_series_prepare_funding_artifacts_v5(
    profile: SeriesPrepareAccountProfileInputV5<'_>,
    requests: SeriesPrepareChildRequestsV4<'_>,
    ticket_rent_lamports: u64,
) -> Result<SeriesPrepareFundingArtifactsV5> {
    if ticket_rent_lamports == 0 {
        return Err(SeriesPrepareFundingArtifactErrorV5::Geometry);
    }
    Ok(SeriesPrepareFundingArtifactsV5 {
        account_profile: emit_account_profile(profile)?,
        request_profile: emit_request_profile()?,
        transition: emit_transition(ticket_rent_lamports)?,
        effect: emit_effect(requests)?,
        lifecycle: emit_lifecycle()?,
    })
}

fn emit_account_profile(input: SeriesPrepareAccountProfileInputV5<'_>) -> Result<Vec<u8>> {
    let mut rules = Vec::with_capacity(SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize);
    for coordinate in 0..SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 {
        rules.push(account_rule(coordinate, input.fixed_data_lengths)?);
    }
    let operations = profile_operations()?;
    let mut base_scratch = vec![0; SERIES_PREPARE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    let mut base = vec![0; SERIES_PREPARE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: SERIES_PREPARE_TRADING_PROGRAM_IDENTITY_V5,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: SERIES_PREPARE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_PREPARE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesPrepareFundingArtifactErrorV5::AccountProfile)?;
    AccountProfileV2::decode(&base)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::AccountProfile)?;
    let funding = [FundingBoundV3::new(
        SERIES_PREPARE_TICKET_COORDINATE_V5,
        FundingActionMaskV3::CREATE,
        SERIES_TICKET_STATE_BYTES_V3 as u32,
    )];
    let mut scratch = vec![0; SERIES_PREPARE_ACCOUNT_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_PREPARE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_v3_atomic(&base, &funding, &mut scratch, &mut output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::AccountProfile)?;
    AccountProfileV3::decode(&output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::AccountProfile)?;
    Ok(output)
}

fn account_rule(
    coordinate: u16,
    lengths: &[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<AccountRuleWithPrestateInputV2> {
    if let Some(representative) = alias_representative(coordinate) {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::Fixed(representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        });
    }
    let (data_length, stride, prestate) = match coordinate {
        0 => (
            SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32,
            0,
            AccountPrestateV2::Exact,
        ),
        1 => (
            dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3 as u32,
            0,
            AccountPrestateV2::Exact,
        ),
        3 => (
            PORTFOLIO_HEADER_BYTES as u32,
            dclutch_product::PORTFOLIO_COEFFICIENT_BYTES as u32,
            AccountPrestateV2::Exact,
        ),
        SERIES_PREPARE_TICKET_COORDINATE_V5 => (
            SERIES_TICKET_STATE_BYTES_V3 as u32,
            0,
            AccountPrestateV2::LifecycleBound,
        ),
        _ => (
            *lengths
                .get(coordinate as usize)
                .ok_or(SeriesPrepareFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
    };
    let privileges = if coordinate == SERIES_PREPARE_PAYER_COORDINATE_V5 {
        signer_writable()
    } else if WRITABLE_REPRESENTATIVES.contains(&coordinate) {
        writable()
    } else if EXECUTABLE_REPRESENTATIVES.contains(&coordinate) {
        executable()
    } else {
        readonly()
    };
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: if coordinate == SERIES_PREPARE_TICKET_COORDINATE_V5 {
                AccountEffectPermissionsV2::new(true, true, true)
            } else if coordinate == 0 {
                write_data()
            } else {
                no_effects()
            },
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride: stride,
        },
        prestate,
    })
}

fn profile_operations() -> Result<[AccountOperationInputV2; PROFILE_OPERATIONS]> {
    Ok([
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3 as u16),
            destination: ScalarCoordinateV2::common(4),
            data_offset: PORTFOLIO_COEFFICIENT_COUNT_OFFSET as u32,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(0),
            expected: IdentityCoordinateV2::common(SERIES_PREPARE_TRADING_PROGRAM_IDENTITY_V5),
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(0),
            destination: IdentityCoordinateV2::common(SERIES_PREPARE_ROOT_KEY_IDENTITY_V5),
        },
        root_identity(
            SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
            CAPABILITY_ROOT_MARKET_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4),
            data_offset: CAPABILITY_ROOT_GENERATION_OFFSET as u32,
        },
        root_identity(
            SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4),
            data_offset: (CAPABILITY_ROOT_SELECTION_OFFSET
                + CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET)
                as u32,
        },
        root_identity(
            SERIES_CONSUME_ROOT_KIND_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
        )?,
        root_identity(
            SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
        )?,
        root_identity(
            SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(1),
            destination: IdentityCoordinateV2::common(SERIES_PREPARE_REFUND_OWNER_IDENTITY_V5),
            data_offset: dclutch_trading::series::generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3
                as u32,
        },
    ])
}

fn emit_request_profile() -> Result<Vec<u8>> {
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_OFFSET),
            SeriesActionV3::Prepare as u8,
        ),
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET),
            ScalarRegisterV1::common(2),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(REQUEST_TICKET_OFFSET),
            IdentityRegisterV1::common(SERIES_PREPARE_TICKET_IDENTITY_V5),
        ),
    ];
    let mut scratch = vec![0; SERIES_PREPARE_REQUEST_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_PREPARE_REQUEST_PROFILE_BYTES_V5];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            SERIES_ACTION_HEADER_BYTES_V3 as u32,
            0,
            SERIES_PREPARE_COMMON_SCALAR_COUNT_V5,
            0,
            SERIES_PREPARE_COMMON_IDENTITY_COUNT_V5,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesPrepareFundingArtifactErrorV5::RequestProfile)?;
    RequestProfileV1::decode(&output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::RequestProfile)?;
    Ok(output)
}

fn emit_transition(ticket_rent_lamports: u64) -> Result<Vec<u8>> {
    let s = ScalarRegisterV3::common;
    let instructions = [
        InstructionV3::load_const(s(0), SERIES_ACTION_HEADER_BYTES_V3 as u64),
        InstructionV3::load_const(s(3), SERIES_WITNESS_ITEM_BYTES_V3 as u64),
        InstructionV3::load_const(
            s(SERIES_PREPARE_TICKET_RENT_SCALAR_V5),
            ticket_rent_lamports,
        ),
        InstructionV3::checked_mul_into(s(2), s(3), s(1)),
    ];
    let mut scratch = vec![0; SERIES_PREPARE_TRANSITION_BYTES_V5];
    let mut output = vec![0; SERIES_PREPARE_TRANSITION_BYTES_V5];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_PREPARE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_PREPARE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Transition)?;
    TransitionProgramV3::decode(&output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Transition)?;
    Ok(output)
}

fn emit_effect(requests: SeriesPrepareChildRequestsV4<'_>) -> Result<Vec<u8>> {
    let parent_root_offset =
        u32::try_from(ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET)
            .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Geometry)?;
    let mut projected_initialize = *requests.projected_initialize;
    let mut projected_open = *requests.projected_open;
    clear_projected_parent_root(&mut projected_initialize)?;
    clear_projected_parent_root(&mut projected_open)?;
    let routes = [
        route(
            SERIES_PREPARE_ROUTE_STARTS_V5[0],
            SERIES_PREPARE_ROUTE_COUNTS_V5[0],
            &projected_initialize,
        ),
        route(
            SERIES_PREPARE_ROUTE_STARTS_V5[1],
            SERIES_PREPARE_ROUTE_COUNTS_V5[1],
            &projected_open,
        ),
        route(
            SERIES_PREPARE_ROUTE_STARTS_V5[2],
            SERIES_PREPARE_ROUTE_COUNTS_V5[2],
            requests.replay_initialize,
        ),
        route(
            SERIES_PREPARE_ROUTE_STARTS_V5[3],
            SERIES_PREPARE_ROUTE_COUNTS_V5[3],
            requests.escrow_open,
        ),
        route(
            SERIES_PREPARE_ROUTE_STARTS_V5[4],
            SERIES_PREPARE_ROUTE_COUNTS_V5[4],
            requests.escrow_lock,
        ),
    ];
    let dependencies = [&SERIES_NO_RECEIPT_DEPENDENCIES_V3[..]; 5];
    let root = IdentityCoordinateV3::common(SERIES_PREPARE_ROOT_KEY_IDENTITY_V5);
    let operations = [
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            parent_root_offset,
            root,
        ),
        EffectInstructionV3::write_request_identity(
            1,
            RequestSpaceV3::Fixed,
            parent_root_offset,
            root,
        ),
    ];
    let mut base_scratch = vec![0; SERIES_PREPARE_BASE_EFFECT_BYTES_V5];
    let mut base = vec![0; SERIES_PREPARE_BASE_EFFECT_BYTES_V5];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5,
            item_account_stride: 0,
            common_scalars: SERIES_PREPARE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_PREPARE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &routes,
        &dependencies,
        &operations,
        &[],
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?;
    let ranges = [BorrowedRangeV4::new(
        SEMANTIC_RANGE_ROUTE_V4,
        RequestCoordinateV4::Fixed(PROOF_OFFSET),
        RequestCoordinateV4::CommonScalar(1),
    )];
    let mut v4_scratch = vec![0; SERIES_PREPARE_EFFECT_V4_BYTES_V5];
    let mut v4 = vec![0; SERIES_PREPARE_EFFECT_V4_BYTES_V5];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        SERIES_ACTION_HEADER_BYTES_V3 as u32,
        &[],
        &ranges,
        &mut v4_scratch,
        &mut v4,
    )
    .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?;
    ProgramV4::decode(&v4).map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?;
    let actions = [FundingActionV5::create(
        SERIES_PREPARE_TICKET_COORDINATE_V5,
        SERIES_PREPARE_PAYER_COORDINATE_V5,
        SERIES_PREPARE_REFUND_COORDINATE_V5,
        SERIES_PREPARE_SYSTEM_COORDINATE_V5,
        SERIES_PREPARE_TICKET_RENT_SCALAR_V5,
        SERIES_PREPARE_REFUND_OWNER_IDENTITY_V5,
        SERIES_TICKET_STATE_BYTES_V3 as u32,
        0,
        4,
    )];
    let seeds = [
        FundingSeedV5::literal(SERIES_TICKET_STATE_PDA_DOMAIN_V3)
            .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?,
        FundingSeedV5::CommonIdentity {
            index: SERIES_PREPARE_ROOT_KEY_IDENTITY_V5,
        },
        FundingSeedV5::CommonIdentity {
            index: SERIES_PREPARE_TICKET_IDENTITY_V5,
        },
        FundingSeedV5::CanonicalBump,
    ];
    let mut scratch = vec![0; SERIES_PREPARE_EFFECT_BYTES_V5];
    let mut output = vec![0; SERIES_PREPARE_EFFECT_BYTES_V5];
    encode_program_v5_atomic(&v4, &actions, &seeds, &mut scratch, &mut output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?;
    let effect =
        ProgramV5::decode(&output).map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?;
    if effect.funding_action_count() != 1
        || effect.funding_seed_count() != 4
        || effect
            .funding_action(0)
            .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Effect)?
            .operation()
            != FundingOperationV5::Create
    {
        return Err(SeriesPrepareFundingArtifactErrorV5::Effect);
    }
    Ok(output)
}

fn clear_projected_parent_root(
    request: &mut [u8; dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
) -> Result<()> {
    request
        .get_mut(
            ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET
                ..ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET + 32,
        )
        .ok_or(SeriesPrepareFundingArtifactErrorV5::Geometry)?
        .fill(0);
    Ok(())
}

fn emit_lifecycle() -> Result<Vec<u8>> {
    let mut scratch = vec![0; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    let mut output = vec![0; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    encode_series_empty_state_lifecycle_v5_atomic(&mut scratch, &mut output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
        .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Lifecycle)?;
    Ok(output)
}

fn root_identity(destination: u16, offset: usize) -> Result<AccountOperationInputV2> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(0),
        destination: IdentityCoordinateV2::common(destination),
        data_offset: u32::try_from(offset)
            .map_err(|_| SeriesPrepareFundingArtifactErrorV5::Geometry)?,
    })
}

const fn route<'a>(start: u16, count: u16, request: &'a [u8]) -> RouteInputV3<'a> {
    RouteInputV3 {
        role: FixedRole::Custody,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: start,
        fixed_account_count: count,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: request,
        item_request: &[],
    }
}

const fn readonly() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, false)
}
const fn writable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, true, false)
}
const fn signer_writable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(true, true, false)
}
const fn executable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, true)
}
const fn no_effects() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}
const fn write_data() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, true)
}

const WRITABLE_REPRESENTATIVES: &[u16] = &[0, 5, 7, 12, 14, 60, 76, 80, 91, 107];
const EXECUTABLE_REPRESENTATIVES: &[u16] = &[9, 10, 13, 16, 20, 42, 43, 44, 49, 52, 63];

const ROUTE_ALIASES: &[(u16, u16)] = &[
    (17, 14),
    (19, 12),
    (41, 8),
    (42, 13),
    (44, 9),
    (45, 16),
    (53, 6),
    (54, 7),
    (55, 8),
    (56, 9),
    (57, 10),
    (58, 11),
    (59, 12),
    (64, 14),
    (65, 15),
    (66, 16),
    (67, 18),
    (68, 6),
    (69, 18),
    (70, 8),
    (71, 9),
    (72, 10),
    (73, 11),
    (74, 21),
    (75, 22),
    (77, 14),
    (78, 16),
    (79, 15),
    (81, 6),
    (82, 18),
    (83, 8),
    (84, 9),
    (85, 10),
    (86, 11),
    (87, 21),
    (88, 22),
    (89, 76),
    (90, 62),
    (92, 61),
    (93, 63),
    (94, 14),
    (95, 16),
    (96, 15),
    (97, 6),
    (98, 18),
    (99, 8),
    (100, 9),
    (101, 10),
    (102, 11),
    (103, 21),
    (104, 22),
    (105, 76),
    (106, 62),
    (108, 91),
    (109, 61),
    (110, 63),
];

fn alias_representative(coordinate: u16) -> Option<u16> {
    ROUTE_ALIASES
        .iter()
        .find_map(|(alias, representative)| (*alias == coordinate).then_some(*representative))
}

#[cfg(test)]
mod tests {
    use dclutch_vm::account_profile::v2::{AliasKindV2, Error as ProfileError};
    use dclutch_vm::effect::v5::ErrorV5;

    use super::*;
    use crate::series::artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    };

    fn requests() -> (
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    ) {
        (
            [1; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [2; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [3; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [4; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [5; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        )
    }

    #[test]
    fn prepare_has_one_create_and_current_gap_free_geometry() {
        let lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let (a, b, c, d, e) = requests();
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &lengths,
            },
            SeriesPrepareChildRequestsV4 {
                projected_initialize: &a,
                projected_open: &b,
                replay_initialize: &c,
                escrow_open: &d,
                escrow_lock: &e,
            },
            123,
        )
        .expect("Prepare artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        assert_eq!(profile.funding_bound_count(), 1);
        let bound = profile.funding_bound(0).expect("Ticket bound");
        assert_eq!(bound.coordinate(), SERIES_PREPARE_TICKET_COORDINATE_V5);
        assert!(bound.actions().permits_create());
        assert!(!bound.actions().permits_close());
        assert_eq!(
            profile.base().physical_account_count(0),
            Ok(SERIES_PREPARE_PHYSICAL_ACCOUNT_COUNT_V5 as usize)
        );
        let effect = ProgramV5::decode(&artifacts.effect).expect("EffectV5");
        let action = effect.funding_action(0).expect("Create");
        assert_eq!(action.operation(), FundingOperationV5::Create);
        assert_eq!(action.state(), SERIES_PREPARE_TICKET_COORDINATE_V5);
        assert_eq!(action.payer(), Some(SERIES_PREPARE_PAYER_COORDINATE_V5));
        assert_eq!(
            action.refund_destination(),
            Some(SERIES_PREPARE_REFUND_COORDINATE_V5)
        );
        assert_eq!(
            action.system_program(),
            Some(SERIES_PREPARE_SYSTEM_COORDINATE_V5)
        );
        let base = effect.base().base();
        assert_eq!(
            base.fixed_account_count(),
            SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5
        );
        for index in 0..5_u16 {
            let route = base.route(index).expect("route");
            assert_eq!(
                route.fixed_account_start(),
                SERIES_PREPARE_ROUTE_STARTS_V5[index as usize]
            );
            assert_eq!(
                route.fixed_account_count(),
                SERIES_PREPARE_ROUTE_COUNTS_V5[index as usize]
            );
        }
    }

    #[test]
    fn zero_rent_and_old_twelve_account_end_are_refused() {
        let lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let (a, b, c, d, e) = requests();
        assert_eq!(
            emit_series_prepare_funding_artifacts_v5(
                SeriesPrepareAccountProfileInputV5 {
                    fixed_data_lengths: &lengths
                },
                SeriesPrepareChildRequestsV4 {
                    projected_initialize: &a,
                    projected_open: &b,
                    replay_initialize: &c,
                    escrow_open: &d,
                    escrow_lock: &e
                },
                0,
            ),
            Err(SeriesPrepareFundingArtifactErrorV5::Geometry)
        );
        assert_ne!(
            SERIES_PREPARE_ROUTE_STARTS_V5[4] + SERIES_PREPARE_ROUTE_COUNTS_V5[4],
            110
        );
    }

    #[test]
    fn prepare_pins_seed_authority_privileges_and_no_phantom_close() {
        let lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let (a, b, c, d, e) = requests();
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &lengths,
            },
            SeriesPrepareChildRequestsV4 {
                projected_initialize: &a,
                projected_open: &b,
                replay_initialize: &c,
                escrow_open: &d,
                escrow_lock: &e,
            },
            123,
        )
        .expect("Prepare artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        let ticket = profile
            .base()
            .rule(false, SERIES_PREPARE_TICKET_COORDINATE_V5)
            .expect("Ticket");
        assert_eq!(ticket.prestate(), AccountPrestateV2::LifecycleBound);
        assert_eq!(ticket.alias_kind(), AliasKindV2::SelfCoordinate);
        assert_eq!(ticket.privileges() & 2, 2);
        assert_eq!(ticket.privileges() & 1, 0);
        let payer = profile
            .base()
            .rule(false, SERIES_PREPARE_PAYER_COORDINATE_V5)
            .expect("payer");
        assert_eq!(payer.privileges() & 1, 1);
        assert_eq!(payer.privileges() & 2, 2);
        assert_eq!(payer.alias_kind(), AliasKindV2::SelfCoordinate);
        let refund = profile
            .base()
            .rule(false, SERIES_PREPARE_REFUND_COORDINATE_V5)
            .expect("surplus refund");
        assert_eq!(refund.privileges() & 1, 0);
        assert_eq!(refund.privileges() & 2, 2);
        assert_eq!(refund.alias_kind(), AliasKindV2::SelfCoordinate);
        let system = profile
            .base()
            .rule(false, SERIES_PREPARE_SYSTEM_COORDINATE_V5)
            .expect("System program");
        assert_eq!(system.privileges() & 4, 4);
        assert_eq!(system.privileges() & 1, 0);
        assert_eq!(system.privileges() & 2, 0);
        assert_eq!(system.alias_kind(), AliasKindV2::SelfCoordinate);
        for (alias, representative) in ROUTE_ALIASES {
            assert!(alias > representative, "all aliases are strictly backward");
            let rule = profile.base().rule(false, *alias).expect("route alias");
            assert_eq!(rule.prestate(), AccountPrestateV2::AuthenticatedRouteAlias);
            assert_eq!(rule.alias_kind(), AliasKindV2::Fixed);
            assert_eq!(rule.alias_index(), *representative);
            assert_eq!(rule.privileges(), 0);
            assert_eq!(rule.effect_permissions(), 0);
        }

        let effect = ProgramV5::decode(&artifacts.effect).expect("EffectV5");
        assert_eq!(effect.funding_action_count(), 1);
        let action = effect.funding_action(0).expect("sole Create");
        assert_eq!(action.operation(), FundingOperationV5::Create);
        assert_eq!(action.seed_start(), 0);
        assert_eq!(action.seed_count(), 4);
        assert_eq!(action.live_bytes(), SERIES_TICKET_STATE_BYTES_V3 as u32);
        assert_eq!(
            action.refund_owner_identity(),
            SERIES_PREPARE_REFUND_OWNER_IDENTITY_V5
        );
        assert_eq!(
            action.lamports_scalar(),
            SERIES_PREPARE_TICKET_RENT_SCALAR_V5
        );
        match effect.funding_seed(0).expect("domain") {
            FundingSeedV5::Literal { bytes, len } => {
                assert_eq!(usize::from(len), SERIES_TICKET_STATE_PDA_DOMAIN_V3.len());
                assert_eq!(
                    &bytes[..usize::from(len)],
                    SERIES_TICKET_STATE_PDA_DOMAIN_V3
                );
            }
            _ => panic!("literal Ticket domain required"),
        }
        assert_eq!(
            effect.funding_seed(1),
            Ok(FundingSeedV5::CommonIdentity {
                index: SERIES_PREPARE_ROOT_KEY_IDENTITY_V5,
            })
        );
        assert_eq!(
            effect.funding_seed(2),
            Ok(FundingSeedV5::CommonIdentity {
                index: SERIES_PREPARE_TICKET_IDENTITY_V5,
            })
        );
        assert_eq!(effect.funding_seed(3), Ok(FundingSeedV5::CanonicalBump));
        assert_eq!(effect.funding_action(1), Err(ErrorV5::ActionTable));
        assert_eq!(profile.funding_bound_for(0), Ok(None));
        assert_eq!(profile.funding_bound_for(6), Ok(None));
    }

    #[test]
    fn prepare_funding_encoder_failure_is_atomic() {
        let lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let (a, b, c, d, e) = requests();
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &lengths,
            },
            SeriesPrepareChildRequestsV4 {
                projected_initialize: &a,
                projected_open: &b,
                replay_initialize: &c,
                escrow_open: &d,
                escrow_lock: &e,
            },
            123,
        )
        .expect("Prepare artifacts");
        let effect = ProgramV5::decode(&artifacts.effect).expect("EffectV5");
        let actions = [effect.funding_action(0).expect("Create")];
        let seeds = [
            effect.funding_seed(0).expect("domain"),
            effect.funding_seed(1).expect("root"),
            effect.funding_seed(2).expect("Ticket"),
            effect.funding_seed(3).expect("bump"),
        ];
        let mut scratch = vec![0_u8; SERIES_PREPARE_EFFECT_BYTES_V5];
        let mut short_output = vec![0xA5_u8; SERIES_PREPARE_EFFECT_BYTES_V5 - 1];
        assert_eq!(
            encode_program_v5_atomic(
                effect.base().bytes(),
                &actions,
                &seeds,
                &mut scratch,
                &mut short_output,
            ),
            Err(ErrorV5::Wire)
        );
        assert!(short_output.iter().all(|byte| *byte == 0xA5));

        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        assert_eq!(
            profile
                .base()
                .rule(false, SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5),
            Err(ProfileError::InvalidCoordinate)
        );
    }
}
