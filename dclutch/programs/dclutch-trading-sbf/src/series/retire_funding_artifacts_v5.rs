//! Canonical current-source Series Retire AccountProfileV3/EffectV5 artifacts.
//!
//! Retire is the sole Ticket deletion author. Its ordinary Effect persists the
//! successor root counters, then FundingV5 closes the exact terminal Ticket
//! into the authenticated lifecycle RentCredit. The selected lifecycle policy
//! is empty, so no root lifecycle invocation is reachable for this action.

extern crate alloc;

use alloc::{vec, vec::Vec};
use dclutch_account_profile_contract::{
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
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_SELECTION_OFFSET,
    hot_v3::{HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3},
};
use dclutch_effect_kernel::{
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, OPERATION_BYTES as EFFECT_OPERATION_BYTES_V3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, ScalarCoordinateV3,
            encode_effect_program_v4_atomic,
        },
    },
    v4::{
        BorrowedRangePolicyV4, HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, ProgramV4,
        encode_program_v4_atomic,
    },
    v5::{
        FUNDING_ACTION_BYTES_V5, FundingActionV5, FundingOperationV5,
        HEADER_BYTES_V5 as EFFECT_HEADER_BYTES_V5, ProgramV5, encode_program_v5_atomic,
    },
};
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
};
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3,
    ScalarRegisterV3, encode_program_atomic,
};

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4, SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
        SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4, SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
        SERIES_CONSUME_ROOT_KIND_IDENTITY_V4, SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
        SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
    },
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionV3},
    lifecycle_policy_v5::{
        SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5, SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5,
        encode_series_empty_state_lifecycle_v5_atomic,
    },
    state::SERIES_TICKET_STATE_BYTES_V3,
};

/// Fixed root coordinate injected by Hot.
pub const SERIES_RETIRE_ROOT_COORDINATE_V5: u16 = 0;
/// Funding-owned terminal Ticket coordinate.
pub const SERIES_RETIRE_TICKET_COORDINATE_V5: u16 = 5;
/// Authenticated lifecycle RentCredit coordinate.
pub const SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5: u16 = 6;
/// Later privilege-free alias used to observe the complete Ticket balance.
///
/// FundingV5 owns lifecycle authority at the self-representative Ticket on
/// coordinate five.  The alias is deliberately later and readonly: it can
/// project donation-inclusive lamports without becoming a second close
/// authority or weakening the representative's `LifecycleBound` prestate.
pub const SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5: u16 = 8;
/// Complete fixed-account width.
pub const SERIES_RETIRE_FIXED_ACCOUNT_COUNT_V5: u16 = 9;
/// Complete common scalar width.
pub const SERIES_RETIRE_COMMON_SCALAR_COUNT_V5: u16 = 15;
/// Complete common identity width.
pub const SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5: u16 = 11;

/// Expected root revision register.
pub const SERIES_RETIRE_EXPECTED_ROOT_REVISION_SCALAR_V5: u16 = 1;
/// Expected Ticket revision register.
pub const SERIES_RETIRE_EXPECTED_TICKET_REVISION_SCALAR_V5: u16 = 2;
/// Observed root revision register.
pub const SERIES_RETIRE_OBSERVED_ROOT_REVISION_SCALAR_V5: u16 = 3;
/// Authenticated Portfolio tail-count register.
pub const SERIES_RETIRE_PRODUCT_TAIL_COUNT_SCALAR_V5: u16 = 4;
/// Observed Ticket revision register.
pub const SERIES_RETIRE_OBSERVED_TICKET_REVISION_SCALAR_V5: u16 = 13;
/// Observed outstanding-Ticket count register.
pub const SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5: u16 = 14;
/// Exact observed Ticket balance register.
pub const SERIES_RETIRE_TICKET_LAMPORTS_SCALAR_V5: u16 = 7;
/// Candidate root revision register.
pub const SERIES_RETIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5: u16 = 8;
/// Candidate outstanding-Ticket count register.
pub const SERIES_RETIRE_CANDIDATE_OUTSTANDING_SCALAR_V5: u16 = 9;
/// Observed root phase register.
pub const SERIES_RETIRE_ROOT_PHASE_SCALAR_V5: u16 = 10;
/// Observed root prepared flag register.
pub const SERIES_RETIRE_ROOT_PREPARED_SCALAR_V5: u16 = 11;
/// Observed Ticket phase register.
pub const SERIES_RETIRE_TICKET_PHASE_SCALAR_V5: u16 = 12;

/// Authenticated RentCredit beneficiary identity.
pub const SERIES_RETIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5: u16 = 6;
/// Current Trading program identity.
pub const SERIES_RETIRE_TRADING_PROGRAM_IDENTITY_V5: u16 = 7;
/// Immutable Template refund-owner identity.
pub const SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5: u16 = 8;
/// Expected Ticket content identity.
pub const SERIES_RETIRE_EXPECTED_TICKET_IDENTITY_V5: u16 = 9;
/// Observed Ticket content identity.
pub const SERIES_RETIRE_OBSERVED_TICKET_IDENTITY_V5: u16 = 10;

const PRODUCT_RECORD_BYTES: u32 = 112;
const ACTION_OFFSET: u32 = 12;
const PROOF_COUNT_OFFSET: u32 = 13;
const REQUEST_TICKET_OFFSET: u32 = 80;
const EXPECTED_ROOT_REVISION_OFFSET: u32 = 112;
const EXPECTED_TICKET_REVISION_OFFSET: u32 = 120;
const ROOT_PHASE_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 12;
const ROOT_PREPARED_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 13;
const ROOT_OUTSTANDING_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 20;
const ROOT_REVISION_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 24;
const TICKET_PHASE_OFFSET: u32 = 12;
const TICKET_REVISION_OFFSET: u32 = 16;
const TICKET_RECORD_OFFSET: u32 = 24;
const RENT_CREDIT_BENEFICIARY_OFFSET: u32 = 16;
const PROFILE_OPERATIONS: usize = 19;
const REQUEST_OPERATIONS: usize = 5;
const TRANSITION_OPERATIONS: usize = 15;
const EFFECT_OPERATIONS: usize = 2;

/// Exact embedded AccountProfileV2 width.
pub const SERIES_RETIRE_BASE_ACCOUNT_PROFILE_BYTES_V5: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + SERIES_RETIRE_FIXED_ACCOUNT_COUNT_V5 as usize * RULE_BYTES
    + PROFILE_OPERATIONS * OPERATION_BYTES;
/// Exact funding-refined AccountProfileV3 width.
pub const SERIES_RETIRE_ACCOUNT_PROFILE_BYTES_V5: usize =
    HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + SERIES_RETIRE_BASE_ACCOUNT_PROFILE_BYTES_V5;
/// Exact RequestProfileV1 width.
pub const SERIES_RETIRE_REQUEST_PROFILE_BYTES_V5: usize =
    REQUEST_HEADER_BYTES + REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES;
/// Exact TransitionV3 width.
pub const SERIES_RETIRE_TRANSITION_BYTES_V5: usize =
    TRANSITION_HEADER_BYTES + TRANSITION_OPERATIONS * TRANSITION_INSTRUCTION_BYTES;
/// Exact embedded zero-route EffectV3 width.
pub const SERIES_RETIRE_BASE_EFFECT_BYTES_V5: usize =
    EFFECT_HEADER_BYTES_V3 + EFFECT_OPERATIONS * EFFECT_OPERATION_BYTES_V3;
/// Exact zero-range EffectV4 width.
pub const SERIES_RETIRE_EFFECT_V4_BYTES_V5: usize =
    EFFECT_HEADER_BYTES_V4 + SERIES_RETIRE_BASE_EFFECT_BYTES_V5;
/// Exact one-Close EffectV5 width.
pub const SERIES_RETIRE_EFFECT_BYTES_V5: usize =
    EFFECT_HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + SERIES_RETIRE_EFFECT_V4_BYTES_V5;

const _: () = assert!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 == 5);

/// Complete canonical Retire artifact bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesRetireFundingArtifactsV5 {
    /// Funding-refined AccountProfileV3 bytes.
    pub account_profile: Vec<u8>,
    /// RequestProfileV1 bytes.
    pub request_profile: Vec<u8>,
    /// TransitionV3 bytes.
    pub transition: Vec<u8>,
    /// One-Close EffectV5 bytes.
    pub effect: Vec<u8>,
    /// Canonical plan-free lifecycle bytes.
    pub lifecycle: Vec<u8>,
}

/// Stable refusal during canonical Retire artifact emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesRetireFundingArtifactErrorV5 {
    /// Checked geometry differed.
    Geometry,
    /// AccountProfile refused.
    AccountProfile,
    /// RequestProfile refused.
    RequestProfile,
    /// Transition refused.
    Transition,
    /// Effect refused.
    Effect,
    /// Lifecycle policy refused.
    Lifecycle,
}

/// Result returned by Retire artifact emission.
pub type Result<T> = core::result::Result<T, SeriesRetireFundingArtifactErrorV5>;

/// Emit and hostile-decode the complete canonical Series Retire artifacts.
pub fn emit_series_retire_funding_artifacts_v5() -> Result<SeriesRetireFundingArtifactsV5> {
    Ok(SeriesRetireFundingArtifactsV5 {
        account_profile: emit_account_profile()?,
        request_profile: emit_request_profile()?,
        transition: emit_transition()?,
        effect: emit_effect()?,
        lifecycle: emit_lifecycle()?,
    })
}

fn emit_account_profile() -> Result<Vec<u8>> {
    let exact = |length, stride| AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: readonly(),
            effect_permissions: no_effects(),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: length,
            data_item_stride: stride,
        },
        prestate: AccountPrestateV2::Exact,
    };
    let rules = [
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: writable(),
                effect_permissions: write_data(),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        },
        exact(dclutch_series_v3_kernel::SERIES_TEMPLATE_BYTES_V3 as u32, 0),
        exact(PRODUCT_RECORD_BYTES, 0),
        exact(
            PORTFOLIO_HEADER_BYTES as u32,
            PORTFOLIO_COEFFICIENT_BYTES as u32,
        ),
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: writable(),
                effect_permissions: AccountEffectPermissionsV2::new(true, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: SERIES_TICKET_STATE_BYTES_V3 as u32,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: writable(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2
                    as u32,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: executable(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::Fixed(SERIES_RETIRE_TICKET_COORDINATE_V5),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        },
    ];
    let operations = profile_operations()?;
    let mut base_scratch = vec![0; SERIES_RETIRE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    let mut base = vec![0; SERIES_RETIRE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: SERIES_RETIRE_TRADING_PROGRAM_IDENTITY_V5,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: SERIES_RETIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_error| {
        #[cfg(test)]
        std::eprintln!("Retire base profile: {_error:?}");
        SeriesRetireFundingArtifactErrorV5::AccountProfile
    })?;
    AccountProfileV2::decode(&base)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::AccountProfile)?;
    let funding = [FundingBoundV3::new(
        SERIES_RETIRE_TICKET_COORDINATE_V5,
        FundingActionMaskV3::CLOSE,
        SERIES_TICKET_STATE_BYTES_V3 as u32,
    )];
    let mut scratch = vec![0; SERIES_RETIRE_ACCOUNT_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_RETIRE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_v3_atomic(&base, &funding, &mut scratch, &mut output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::AccountProfile)?;
    AccountProfileV3::decode(&output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::AccountProfile)?;
    Ok(output)
}

fn profile_operations() -> Result<[AccountOperationInputV2; PROFILE_OPERATIONS]> {
    Ok([
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3 as u16),
            destination: ScalarCoordinateV2::common(SERIES_RETIRE_PRODUCT_TAIL_COUNT_SCALAR_V5),
            data_offset: PORTFOLIO_COEFFICIENT_COUNT_OFFSET as u32,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(SERIES_RETIRE_ROOT_COORDINATE_V5),
            expected: IdentityCoordinateV2::common(SERIES_RETIRE_TRADING_PROGRAM_IDENTITY_V5),
        },
        root_identity(
            SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
            CAPABILITY_ROOT_MARKET_OFFSET,
        )?,
        project_u64(
            0,
            SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
            CAPABILITY_ROOT_GENERATION_OFFSET as u32,
        ),
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
        project_u64(
            0,
            SERIES_RETIRE_OBSERVED_ROOT_REVISION_SCALAR_V5,
            ROOT_REVISION_OFFSET,
        ),
        project_u32(
            0,
            SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5,
            ROOT_OUTSTANDING_OFFSET,
        ),
        project_u8(0, SERIES_RETIRE_ROOT_PHASE_SCALAR_V5, ROOT_PHASE_OFFSET),
        project_u8(
            0,
            SERIES_RETIRE_ROOT_PREPARED_SCALAR_V5,
            ROOT_PREPARED_OFFSET,
        ),
        AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5),
            destination: ScalarCoordinateV2::common(SERIES_RETIRE_TICKET_LAMPORTS_SCALAR_V5),
        },
        project_u64(
            SERIES_RETIRE_TICKET_COORDINATE_V5,
            SERIES_RETIRE_OBSERVED_TICKET_REVISION_SCALAR_V5,
            TICKET_REVISION_OFFSET,
        ),
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(SERIES_RETIRE_TICKET_COORDINATE_V5),
            destination: IdentityCoordinateV2::common(SERIES_RETIRE_OBSERVED_TICKET_IDENTITY_V5),
            data_offset: TICKET_RECORD_OFFSET,
        },
        project_u8(
            SERIES_RETIRE_TICKET_COORDINATE_V5,
            SERIES_RETIRE_TICKET_PHASE_SCALAR_V5,
            TICKET_PHASE_OFFSET,
        ),
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(1),
            destination: IdentityCoordinateV2::common(
                SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5,
            ),
            data_offset: dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3
                as u32,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5),
            destination: IdentityCoordinateV2::common(
                SERIES_RETIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5,
            ),
            data_offset: RENT_CREDIT_BENEFICIARY_OFFSET,
        },
    ])
}

fn emit_request_profile() -> Result<Vec<u8>> {
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_OFFSET),
            SeriesActionV3::Retire as u8,
        ),
        RequestInstructionV1::require_u8(RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET), 0),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(EXPECTED_ROOT_REVISION_OFFSET),
            ScalarRegisterV1::common(SERIES_RETIRE_EXPECTED_ROOT_REVISION_SCALAR_V5),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(EXPECTED_TICKET_REVISION_OFFSET),
            ScalarRegisterV1::common(SERIES_RETIRE_EXPECTED_TICKET_REVISION_SCALAR_V5),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(REQUEST_TICKET_OFFSET),
            IdentityRegisterV1::common(SERIES_RETIRE_EXPECTED_TICKET_IDENTITY_V5),
        ),
    ];
    let mut scratch = vec![0; SERIES_RETIRE_REQUEST_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_RETIRE_REQUEST_PROFILE_BYTES_V5];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            SERIES_ACTION_HEADER_BYTES_V3 as u32,
            0,
            SERIES_RETIRE_COMMON_SCALAR_COUNT_V5,
            0,
            SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesRetireFundingArtifactErrorV5::RequestProfile)?;
    RequestProfileV1::decode(&output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::RequestProfile)?;
    Ok(output)
}

fn emit_transition() -> Result<Vec<u8>> {
    let s = ScalarRegisterV3::common;
    let i = IdentityRegisterV3::common;
    let instructions = [
        InstructionV3::load_const(s(0), SERIES_ACTION_HEADER_BYTES_V3 as u64),
        InstructionV3::scalar_eq(
            s(SERIES_RETIRE_EXPECTED_ROOT_REVISION_SCALAR_V5),
            s(SERIES_RETIRE_OBSERVED_ROOT_REVISION_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_RETIRE_EXPECTED_TICKET_REVISION_SCALAR_V5),
            s(SERIES_RETIRE_OBSERVED_TICKET_REVISION_SCALAR_V5),
        ),
        InstructionV3::identity_eq(
            i(SERIES_RETIRE_EXPECTED_TICKET_IDENTITY_V5),
            i(SERIES_RETIRE_OBSERVED_TICKET_IDENTITY_V5),
        ),
        InstructionV3::identity_eq(
            i(SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5),
            i(SERIES_RETIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5),
        ),
        InstructionV3::load_const(s(0), 0),
        InstructionV3::load_const(s(1), 1),
        InstructionV3::load_const(s(2), 3),
        InstructionV3::scalar_eq(s(SERIES_RETIRE_ROOT_PHASE_SCALAR_V5), s(1)),
        InstructionV3::scalar_eq(s(SERIES_RETIRE_ROOT_PREPARED_SCALAR_V5), s(0)),
        InstructionV3::nonzero(s(SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5)),
        InstructionV3::scalar_lt(s(0), s(SERIES_RETIRE_TICKET_PHASE_SCALAR_V5)),
        InstructionV3::scalar_lt(s(SERIES_RETIRE_TICKET_PHASE_SCALAR_V5), s(2)),
        InstructionV3::increment_into(
            s(SERIES_RETIRE_OBSERVED_ROOT_REVISION_SCALAR_V5),
            s(SERIES_RETIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5),
        ),
        InstructionV3::sub_into(
            s(SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5),
            s(1),
            s(SERIES_RETIRE_CANDIDATE_OUTSTANDING_SCALAR_V5),
        ),
    ];
    let mut scratch = vec![0; SERIES_RETIRE_TRANSITION_BYTES_V5];
    let mut output = vec![0; SERIES_RETIRE_TRANSITION_BYTES_V5];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_RETIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesRetireFundingArtifactErrorV5::Transition)?;
    TransitionProgramV3::decode(&output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::Transition)?;
    Ok(output)
}

fn emit_effect() -> Result<Vec<u8>> {
    let operations = [
        EffectInstructionV3::write_u32(
            AccountCoordinateV3::fixed(SERIES_RETIRE_ROOT_COORDINATE_V5),
            ROOT_OUTSTANDING_OFFSET,
            ScalarCoordinateV3::common(SERIES_RETIRE_CANDIDATE_OUTSTANDING_SCALAR_V5),
        ),
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(SERIES_RETIRE_ROOT_COORDINATE_V5),
            ROOT_REVISION_OFFSET,
            ScalarCoordinateV3::common(SERIES_RETIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5),
        ),
    ];
    let mut base_scratch = vec![0; SERIES_RETIRE_BASE_EFFECT_BYTES_V5];
    let mut base = vec![0; SERIES_RETIRE_BASE_EFFECT_BYTES_V5];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: SERIES_RETIRE_FIXED_ACCOUNT_COUNT_V5,
            item_account_stride: 0,
            common_scalars: SERIES_RETIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &[],
        &[],
        &operations,
        &[],
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?;
    let mut v4_scratch = vec![0; SERIES_RETIRE_EFFECT_V4_BYTES_V5];
    let mut v4 = vec![0; SERIES_RETIRE_EFFECT_V4_BYTES_V5];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        SERIES_ACTION_HEADER_BYTES_V3 as u32,
        &[],
        &[],
        &mut v4_scratch,
        &mut v4,
    )
    .map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?;
    ProgramV4::decode(&v4).map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?;
    let actions = [FundingActionV5::close(
        SERIES_RETIRE_TICKET_COORDINATE_V5,
        SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5,
        SERIES_RETIRE_TICKET_LAMPORTS_SCALAR_V5,
        SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5,
    )];
    let mut scratch = vec![0; SERIES_RETIRE_EFFECT_BYTES_V5];
    let mut output = vec![0; SERIES_RETIRE_EFFECT_BYTES_V5];
    encode_program_v5_atomic(&v4, &actions, &[], &mut scratch, &mut output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?;
    let effect =
        ProgramV5::decode(&output).map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?;
    if effect.funding_action_count() != 1
        || effect.funding_seed_count() != 0
        || effect
            .funding_action(0)
            .map_err(|_| SeriesRetireFundingArtifactErrorV5::Effect)?
            .operation()
            != FundingOperationV5::Close
    {
        return Err(SeriesRetireFundingArtifactErrorV5::Effect);
    }
    Ok(output)
}

fn emit_lifecycle() -> Result<Vec<u8>> {
    let mut scratch = vec![0; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    let mut output = vec![0; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    encode_series_empty_state_lifecycle_v5_atomic(&mut scratch, &mut output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
        .map_err(|_| SeriesRetireFundingArtifactErrorV5::Lifecycle)?;
    Ok(output)
}

fn root_identity(destination: u16, offset: usize) -> Result<AccountOperationInputV2> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(0),
        destination: IdentityCoordinateV2::common(destination),
        data_offset: u32::try_from(offset)
            .map_err(|_| SeriesRetireFundingArtifactErrorV5::Geometry)?,
    })
}

const fn project_u64(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU64 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
    }
}
const fn project_u32(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU32 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
    }
}
const fn project_u8(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU8 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
    }
}
const fn readonly() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, false)
}
const fn writable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, true, false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_is_ticket_funding_close_and_not_root_lifecycle() {
        let artifacts = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        let bound = profile.funding_bound(0).expect("Ticket bound");
        assert_eq!(profile.funding_bound_count(), 1);
        assert_eq!(bound.coordinate(), SERIES_RETIRE_TICKET_COORDINATE_V5);
        assert!(bound.actions().permits_close());
        assert!(!bound.actions().permits_create());
        assert_eq!(
            profile.funding_bound_for(SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5),
            Ok(None)
        );
        let representative = profile
            .base()
            .rule(false, SERIES_RETIRE_TICKET_COORDINATE_V5)
            .expect("funding representative");
        assert_eq!(representative.prestate(), AccountPrestateV2::LifecycleBound);
        assert_eq!(
            representative.alias_kind(),
            dclutch_account_profile_contract::v2::AliasKindV2::SelfCoordinate
        );
        let balance_alias = profile
            .base()
            .rule(false, SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5)
            .expect("balance alias");
        assert_eq!(
            balance_alias.prestate(),
            AccountPrestateV2::AuthenticatedRouteAlias
        );
        assert_eq!(
            balance_alias.alias_kind(),
            dclutch_account_profile_contract::v2::AliasKindV2::Fixed
        );
        assert_eq!(
            balance_alias.alias_index(),
            SERIES_RETIRE_TICKET_COORDINATE_V5
        );
        assert_eq!(balance_alias.privileges(), 0);
        assert_eq!(balance_alias.effect_permissions(), 0);
        let effect = ProgramV5::decode(&artifacts.effect).expect("EffectV5");
        let action = effect.funding_action(0).expect("Close");
        assert_eq!(action.operation(), FundingOperationV5::Close);
        assert_eq!(action.state(), SERIES_RETIRE_TICKET_COORDINATE_V5);
        assert_eq!(
            action.rent_credit(),
            Some(SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5)
        );
        assert_eq!(
            action.refund_owner_identity(),
            SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5
        );
        assert_eq!(
            action.lamports_scalar(),
            SERIES_RETIRE_TICKET_LAMPORTS_SCALAR_V5
        );
        let lifecycle =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &artifacts.lifecycle)
                .expect("empty lifecycle");
        for selected in 0..5_u32 {
            assert_eq!(lifecycle.action_plan_count(selected), Ok(0));
        }
    }

    #[test]
    fn retire_profile_effect_and_empty_lifecycle_join() {
        let artifacts = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        let lifecycle =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &artifacts.lifecycle)
                .expect("empty lifecycle");
        lifecycle
            .validate_account_profile_with_external_funding_join(profile)
            .expect("funding join");
        let base = ProgramV5::decode(&artifacts.effect)
            .expect("EffectV5")
            .base()
            .base();
        assert_eq!(
            base.fixed_account_count(),
            SERIES_RETIRE_FIXED_ACCOUNT_COUNT_V5
        );
        assert_eq!(base.route_count(), 0);
        assert_eq!(base.fixed_operation_count(), 2);
    }

    #[test]
    fn retire_request_transition_and_hostile_funding_are_pinned() {
        let artifacts = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let request = RequestProfileV1::decode(&artifacts.request_profile).expect("request");
        assert_eq!(
            request.fixed_request_bytes(),
            SERIES_ACTION_HEADER_BYTES_V3 as u32
        );
        let transition = TransitionProgramV3::decode(&artifacts.transition).expect("transition");
        assert_eq!(
            transition.common_scalar_count(),
            SERIES_RETIRE_COMMON_SCALAR_COUNT_V5
        );
        assert_eq!(
            transition.common_identity_count(),
            SERIES_RETIRE_COMMON_IDENTITY_COUNT_V5
        );
        assert_eq!(
            artifacts.transition.len(),
            SERIES_RETIRE_TRANSITION_BYTES_V5
        );
        let mut hostile = artifacts.effect;
        hostile[6..8].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            ProgramV5::decode(&hostile),
            Err(dclutch_effect_kernel::v5::ErrorV5::Wire)
        );
    }

    #[test]
    fn retire_alias_schema_refuses_self_forward_privilege_and_effect_authority() {
        let artifacts = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let base_start = HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3;
        let alias_start = base_start
            + DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + usize::from(SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5) * RULE_BYTES;
        let assert_base_refuses = |hostile: Vec<u8>| {
            // The hostile mutation stayed inside the embedded base, rather
            // than invalidating the V3 wrapper and testing the wrong wall.
            assert_eq!(
                &hostile[..base_start],
                &artifacts.account_profile[..base_start]
            );
            assert_eq!(
                AccountProfileV3::decode(&hostile),
                Err(dclutch_account_profile_contract::v3::ErrorV3::BaseProfile)
            );
        };

        let mut self_alias = artifacts.account_profile.clone();
        self_alias[alias_start + 2] = 0;
        assert_base_refuses(self_alias);

        let mut forward_alias = artifacts.account_profile.clone();
        forward_alias[alias_start + 4..alias_start + 6]
            .copy_from_slice(&SERIES_RETIRE_TICKET_BALANCE_ALIAS_COORDINATE_V5.to_le_bytes());
        assert_base_refuses(forward_alias);

        let mut writable_alias = artifacts.account_profile.clone();
        writable_alias[alias_start] = 2;
        assert_base_refuses(writable_alias);

        let mut effect_authority = artifacts.account_profile.clone();
        effect_authority[alias_start + 1] = 2;
        assert_base_refuses(effect_authority);
    }
}
