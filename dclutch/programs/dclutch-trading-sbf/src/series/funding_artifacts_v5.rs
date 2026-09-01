//! Canonical Series AccountProfileV3 and EffectV5 funding artifacts.
//!
//! This module owns successor bytes, not a host fixture. Every emitted V3/V5
//! artifact is built from the canonical V2/V4 semantic program, hostile-
//! decoded by the contract that owns it, and returned only after the complete
//! successor bytes accept. The terminal Close bundle is deliberately empty
//! on the funding side: the selected lifecycle policy is the sole root-close
//! author, so a dummy FundingV5 action would be a second writer.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    v2::{
        AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES,
        Error as AccountProfileErrorV2, OPERATION_BYTES, RULE_BYTES, TrustedBuiltinIdentityV2,
        TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        },
    },
    v3::{
        AccountProfileV3, ErrorV3 as AccountProfileErrorV3, HEADER_BYTES_V3,
        encode_account_profile_v3_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_SELECTION_OFFSET,
    hot_v3::{
        HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
    },
};
use dclutch_effect_kernel::{
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, encode::EffectGeometryV3,
        encode::encode_effect_program_v4_atomic,
    },
    v4::{HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, ProgramV4, encode_program_v4_atomic},
    v5::{HEADER_BYTES_V5 as EFFECT_HEADER_BYTES_V5, ProgramV5, encode_program_v5_atomic},
};
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
};
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
        encode_request_profile_v1_atomic,
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
        SERIES_CONSUME_ROOT_COORDINATE_V4, SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4,
        SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4, SERIES_CONSUME_ROOT_KIND_IDENTITY_V4,
        SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4, SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
    },
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionV3},
    lifecycle_policy_v5::{
        SERIES_CLOSE_BENEFICIARY_IDENTITY_V5, SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5,
        SERIES_CLOSE_ROOT_PRINCIPAL_SCALAR_V5, SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
        SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5, encode_series_close_state_lifecycle_v5_atomic,
    },
};
use dclutch_series_v3_kernel::SERIES_TEMPLATE_BYTES_V3;

/// Exact terminal Close logical account count: five Hot accounts, RentCredit, Rent Program.
pub const SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5: u16 = 7;
/// Expanded Close common scalar geometry.
pub const SERIES_CLOSE_COMMON_SCALAR_COUNT_V5: u16 = 8;
/// Expanded Close common identity geometry.
pub const SERIES_CLOSE_COMMON_IDENTITY_COUNT_V5: u16 = 9;

/// Request-projected expected root revision.
pub const SERIES_CLOSE_EXPECTED_ROOT_REVISION_SCALAR_V5: u16 = 1;
/// AccountProfile-projected current root revision.
pub const SERIES_CLOSE_OBSERVED_ROOT_REVISION_SCALAR_V5: u16 = 2;
/// AccountProfile-protected current executing Trading identity.
pub const SERIES_CLOSE_TRADING_PROGRAM_IDENTITY_V5: u16 = 7;
/// Immutable Series Template refund owner projected independently of RentCredit.
pub const SERIES_CLOSE_TEMPLATE_REFUND_OWNER_IDENTITY_V5: u16 = 8;
/// Product-owned outcome count projected from the authenticated Portfolio.
pub const SERIES_CLOSE_PRODUCT_TAIL_COUNT_SCALAR_V5: u16 = 4;
/// Rent Program logical coordinate after the common prefix and RentCredit.
pub const SERIES_CLOSE_RENT_PROGRAM_COORDINATE_V5: u16 = 6;

const SERIES_PRODUCT_RECORD_BYTES_V5: u32 = 112;
const _: () = assert!(
    SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5 as usize == HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 + 2
);
const _: () = assert!(
    SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5 as usize == HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
);

const SERIES_ACTION_SELECTOR_OFFSET: u32 = 12;
const SERIES_PROOF_COUNT_OFFSET: u32 = 13;
const SERIES_EXPECTED_ROOT_REVISION_OFFSET: u32 = 112;
const SERIES_ROOT_REVISION_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 24;
const SERIES_ROOT_CLOSE_RENT_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 32;
const LIFECYCLE_RENT_CREDIT_REFUND_WALLET_OFFSET_V2: u32 = 16;

const CLOSE_BASE_PROFILE_OPERATION_COUNT: usize = 13;
/// Exact Close embedded V2 profile width.
pub const SERIES_CLOSE_BASE_ACCOUNT_PROFILE_BYTES_V5: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5 as usize * RULE_BYTES
    + CLOSE_BASE_PROFILE_OPERATION_COUNT * OPERATION_BYTES;
/// Exact Close AccountProfileV3 width with a canonical empty funding table.
pub const SERIES_CLOSE_ACCOUNT_PROFILE_BYTES_V5: usize =
    HEADER_BYTES_V3 + SERIES_CLOSE_BASE_ACCOUNT_PROFILE_BYTES_V5;
/// Exact Close RequestProfile width.
pub const SERIES_CLOSE_REQUEST_PROFILE_BYTES_V5: usize =
    REQUEST_HEADER_BYTES + 3 * REQUEST_OPERATION_BYTES;
/// Exact Close Transition width.
pub const SERIES_CLOSE_TRANSITION_BYTES_V5: usize =
    TRANSITION_HEADER_BYTES + 3 * TRANSITION_INSTRUCTION_BYTES;
/// Exact zero-route embedded EffectV3 width.
pub const SERIES_CLOSE_BASE_EFFECT_BYTES_V5: usize = EFFECT_HEADER_BYTES_V3;
/// Exact empty-range EffectV4 width.
pub const SERIES_CLOSE_EFFECT_V4_BYTES_V5: usize =
    SERIES_CLOSE_BASE_EFFECT_BYTES_V5 + EFFECT_HEADER_BYTES_V4;
/// Exact empty-funding EffectV5 width.
pub const SERIES_CLOSE_EFFECT_BYTES_V5: usize =
    EFFECT_HEADER_BYTES_V5 + SERIES_CLOSE_EFFECT_V4_BYTES_V5;

/// Stable refusal from canonical Series funding artifact emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFundingArtifactErrorV5 {
    /// A checked width or fixed geometry differed.
    Geometry,
    /// Embedded AccountProfileV2 emission or hostile decode refused.
    AccountProfileV2(AccountProfileErrorV2),
    /// AccountProfileV3 emission or hostile decode refused.
    AccountProfileV3(AccountProfileErrorV3),
    /// RequestProfile emission or hostile decode refused.
    RequestProfile,
    /// Transition emission or hostile decode refused.
    Transition,
    /// Effect V3/V4/V5 emission or hostile decode refused.
    Effect,
    /// The canonical Close lifecycle policy refused.
    Lifecycle,
}

/// Result alias for Series funding artifact emission.
pub type Result<T> = core::result::Result<T, SeriesFundingArtifactErrorV5>;

/// Complete canonical artifact bytes selected by Series Close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesCloseFundingArtifactsV5 {
    /// Exact AccountProfileV3 bytes with an empty funding refinement.
    pub account_profile: Vec<u8>,
    /// Exact fixed-header request projection.
    pub request_profile: Vec<u8>,
    /// Exact optimistic root-revision transition.
    pub transition: Vec<u8>,
    /// Exact empty-funding EffectV5 over the zero-route V4 program.
    pub effect: Vec<u8>,
    /// Exact sole root-close StateLifecyclePolicyV5 bytes.
    pub lifecycle: Vec<u8>,
}

/// Emit and hostile-redecode the complete canonical Series Close artifacts.
pub fn emit_series_close_funding_artifacts_v5() -> Result<SeriesCloseFundingArtifactsV5> {
    let account_profile = emit_close_account_profile_v5()?;
    let request_profile = emit_close_request_profile_v5()?;
    let transition = emit_close_transition_v5()?;
    let effect = emit_close_effect_v5()?;
    let lifecycle = emit_close_lifecycle_v5()?;
    Ok(SeriesCloseFundingArtifactsV5 {
        account_profile,
        request_profile,
        transition,
        effect,
        lifecycle,
    })
}

fn emit_close_account_profile_v5() -> Result<Vec<u8>> {
    let root = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(true, true, true),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
                .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            data_item_stride: 0,
        },
        // Close consumes an already-live root. LifecycleBound would require an
        // AuthenticateOrCreate alternative and thereby make a non-Close root
        // lifecycle plan reachable; exact live state plus the Hot root check
        // and the owner anchor below is the canonical terminal prestate.
        prestate: AccountPrestateV2::Exact,
    };
    let exact_readonly = |data_length, data_item_stride| AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    };
    let linked_basis = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        // Hot has already authenticated the Product-selected linked-basis raw
        // record before profile evaluation. The profile neither projects nor
        // restates its independent semantic width.
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    };
    let rent_credit = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, true, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2)
                .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    };
    let rent_program = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, true),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    };
    let operations = [
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(
                u16::try_from(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3)
                    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            ),
            destination: ScalarCoordinateV2::common(SERIES_CLOSE_PRODUCT_TAIL_COUNT_SCALAR_V5),
            data_offset: u32::try_from(PORTFOLIO_COEFFICIENT_COUNT_OFFSET)
                .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            expected: IdentityCoordinateV2::common(SERIES_CLOSE_TRADING_PROGRAM_IDENTITY_V5),
        },
        root_identity(
            SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
            CAPABILITY_ROOT_MARKET_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4),
            data_offset: root_offset(CAPABILITY_ROOT_GENERATION_OFFSET, 0)?,
        },
        root_identity(
            SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4),
            data_offset: root_offset(
                CAPABILITY_ROOT_SELECTION_OFFSET,
                CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET,
            )?,
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
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CLOSE_OBSERVED_ROOT_REVISION_SCALAR_V5),
            data_offset: SERIES_ROOT_REVISION_OFFSET,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CLOSE_ROOT_PRINCIPAL_SCALAR_V5),
            data_offset: SERIES_ROOT_CLOSE_RENT_OFFSET,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5),
            destination: IdentityCoordinateV2::common(SERIES_CLOSE_BENEFICIARY_IDENTITY_V5),
            data_offset: LIFECYCLE_RENT_CREDIT_REFUND_WALLET_OFFSET_V2,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(
                u16::try_from(HOT_RUNTIME_CONFIG_COORDINATE_V3)
                    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            ),
            destination: IdentityCoordinateV2::common(
                SERIES_CLOSE_TEMPLATE_REFUND_OWNER_IDENTITY_V5,
            ),
            data_offset: u32::try_from(
                dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
            )
            .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
        },
    ];
    let mut base_scratch = vec![0_u8; SERIES_CLOSE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    let mut base = vec![0_u8; SERIES_CLOSE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: SERIES_CLOSE_TRADING_PROGRAM_IDENTITY_V5,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &[
            root,
            exact_readonly(
                u32::try_from(SERIES_TEMPLATE_BYTES_V3)
                    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
                0,
            ),
            exact_readonly(SERIES_PRODUCT_RECORD_BYTES_V5, 0),
            exact_readonly(
                u32::try_from(PORTFOLIO_HEADER_BYTES)
                    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
                u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            ),
            linked_basis,
            rent_credit,
            rent_program,
        ],
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: SERIES_CLOSE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_CLOSE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &mut base_scratch,
        &mut base,
    )
    .map_err(SeriesFundingArtifactErrorV5::AccountProfileV2)?;
    AccountProfileV2::decode(&base).map_err(SeriesFundingArtifactErrorV5::AccountProfileV2)?;
    let mut scratch = vec![0_u8; SERIES_CLOSE_ACCOUNT_PROFILE_BYTES_V5];
    let mut output = vec![0_u8; SERIES_CLOSE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_v3_atomic(&base, &[], &mut scratch, &mut output)
        .map_err(SeriesFundingArtifactErrorV5::AccountProfileV3)?;
    AccountProfileV3::decode(&output).map_err(SeriesFundingArtifactErrorV5::AccountProfileV3)?;
    Ok(output)
}

fn emit_close_request_profile_v5() -> Result<Vec<u8>> {
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(SERIES_ACTION_SELECTOR_OFFSET),
            SeriesActionV3::Close as u8,
        ),
        RequestInstructionV1::require_u8(RequestCoordinateV1::fixed(SERIES_PROOF_COUNT_OFFSET), 0),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(SERIES_EXPECTED_ROOT_REVISION_OFFSET),
            ScalarRegisterV1::common(SERIES_CLOSE_EXPECTED_ROOT_REVISION_SCALAR_V5),
        ),
    ];
    let mut scratch = vec![0_u8; SERIES_CLOSE_REQUEST_PROFILE_BYTES_V5];
    let mut output = vec![0_u8; SERIES_CLOSE_REQUEST_PROFILE_BYTES_V5];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
            0,
            SERIES_CLOSE_COMMON_SCALAR_COUNT_V5,
            0,
            SERIES_CLOSE_COMMON_IDENTITY_COUNT_V5,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesFundingArtifactErrorV5::RequestProfile)?;
    RequestProfileV1::decode(&output).map_err(|_| SeriesFundingArtifactErrorV5::RequestProfile)?;
    Ok(output)
}

fn emit_close_transition_v5() -> Result<Vec<u8>> {
    let instructions = [
        InstructionV3::load_const(
            ScalarRegisterV3::common(0),
            u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
        ),
        InstructionV3::scalar_eq(
            ScalarRegisterV3::common(SERIES_CLOSE_EXPECTED_ROOT_REVISION_SCALAR_V5),
            ScalarRegisterV3::common(SERIES_CLOSE_OBSERVED_ROOT_REVISION_SCALAR_V5),
        ),
        InstructionV3::identity_eq(
            IdentityRegisterV3::common(SERIES_CLOSE_TEMPLATE_REFUND_OWNER_IDENTITY_V5),
            IdentityRegisterV3::common(SERIES_CLOSE_BENEFICIARY_IDENTITY_V5),
        ),
    ];
    let mut scratch = vec![0_u8; SERIES_CLOSE_TRANSITION_BYTES_V5];
    let mut output = vec![0_u8; SERIES_CLOSE_TRANSITION_BYTES_V5];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_CLOSE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_CLOSE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesFundingArtifactErrorV5::Transition)?;
    TransitionProgramV3::decode(&output).map_err(|_| SeriesFundingArtifactErrorV5::Transition)?;
    Ok(output)
}

fn emit_close_effect_v5() -> Result<Vec<u8>> {
    let mut base_scratch = vec![0_u8; SERIES_CLOSE_BASE_EFFECT_BYTES_V5];
    let mut base = vec![0_u8; SERIES_CLOSE_BASE_EFFECT_BYTES_V5];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5,
            item_account_stride: 0,
            common_scalars: SERIES_CLOSE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_CLOSE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &[],
        &[],
        &[],
        &[],
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesFundingArtifactErrorV5::Effect)?;
    let mut v4_scratch = vec![0_u8; SERIES_CLOSE_EFFECT_V4_BYTES_V5];
    let mut v4 = vec![0_u8; SERIES_CLOSE_EFFECT_V4_BYTES_V5];
    encode_program_v4_atomic(
        &base,
        dclutch_effect_kernel::v4::BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
            .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)?,
        &[],
        &[],
        &mut v4_scratch,
        &mut v4,
    )
    .map_err(|_| SeriesFundingArtifactErrorV5::Effect)?;
    ProgramV4::decode(&v4).map_err(|_| SeriesFundingArtifactErrorV5::Effect)?;
    let mut v5_scratch = vec![0_u8; SERIES_CLOSE_EFFECT_BYTES_V5];
    let mut v5 = vec![0_u8; SERIES_CLOSE_EFFECT_BYTES_V5];
    encode_program_v5_atomic(&v4, &[], &[], &mut v5_scratch, &mut v5)
        .map_err(|_| SeriesFundingArtifactErrorV5::Effect)?;
    ProgramV5::decode(&v5).map_err(|_| SeriesFundingArtifactErrorV5::Effect)?;
    Ok(v5)
}

fn emit_close_lifecycle_v5() -> Result<Vec<u8>> {
    let mut scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
    let mut output = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
    encode_series_close_state_lifecycle_v5_atomic(&mut scratch, &mut output)
        .map_err(|_| SeriesFundingArtifactErrorV5::Lifecycle)?;
    Ok(output)
}

fn root_identity(destination: u16, relative_offset: usize) -> Result<AccountOperationInputV2> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
        destination: IdentityCoordinateV2::common(destination),
        data_offset: root_offset(relative_offset, 0)?,
    })
}

fn root_offset(left: usize, right: usize) -> Result<u32> {
    u32::try_from(
        left.checked_add(right)
            .ok_or(SeriesFundingArtifactErrorV5::Geometry)?,
    )
    .map_err(|_| SeriesFundingArtifactErrorV5::Geometry)
}

#[cfg(test)]
mod tests {
    use dclutch_account_profile_contract::{
        AccountObservationV1,
        lifecycle_v3::{LifecycleOperationV3, StateLifecyclePolicyV5},
        v2::{AccountPrestateV2, ProjectionRegistersV2, derive_effect_permissions, project_atomic},
        v3::AccountProfileV3,
    };
    use dclutch_effect_kernel::v2::AccountPermission;
    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};

    use super::*;

    #[test]
    fn close_bundle_redecodes_and_joins_the_sole_root_close() {
        let artifacts = emit_series_close_funding_artifacts_v5().expect("Close artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("ProfileV3");
        let effect = ProgramV5::decode(&artifacts.effect).expect("EffectV5");
        let lifecycle =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &artifacts.lifecycle)
                .expect("close lifecycle");
        let join = lifecycle
            .validate_account_profile_with_external_funding_join(profile)
            .expect("empty funding refinement joins lifecycle-owned root");
        let selected = lifecycle
            .action_plan(SeriesActionV3::Close as u32, 0)
            .expect("sole Close plan")
            .with_validated_join(join);
        let indices = selected
            .project_account_indices(profile.base(), 0, None)
            .expect("fixed Close indices");
        assert_eq!(selected.operation(), LifecycleOperationV3::Close);
        assert_eq!(indices.state(), 0);
        assert_eq!(indices.rent_credit(), Some(5));
        assert_eq!(selected.target_data_bytes(0), Ok(296));
        assert_eq!(profile.funding_bound_count(), 0);
        assert_eq!(effect.funding_action_count(), 0);
        assert_eq!(effect.funding_seed_count(), 0);
        assert_eq!(effect.base().base().fixed_account_count(), 7);
        assert_eq!(effect.base().base().common_scalar_count(), 8);
        assert_eq!(effect.base().base().common_identity_count(), 9);
    }

    #[test]
    fn close_profile_projects_exact_widths_and_refuses_hybrid_successors() {
        let first = emit_series_close_funding_artifacts_v5().expect("first");
        let second = emit_series_close_funding_artifacts_v5().expect("second");
        assert_eq!(first, second);
        let profile = AccountProfileV3::decode(&first.account_profile).expect("ProfileV3");
        assert_eq!(profile.base().fixed_account_count(), 7);
        assert_eq!(
            profile.base().rule(false, 0).map(|rule| rule.prestate()),
            Ok(AccountPrestateV2::Exact)
        );
        assert_eq!(
            profile.base().rule(false, 5).map(|rule| rule.data_length()),
            Ok(128)
        );
        assert_eq!(
            first.account_profile.len(),
            SERIES_CLOSE_ACCOUNT_PROFILE_BYTES_V5
        );
        assert_eq!(first.effect.len(), SERIES_CLOSE_EFFECT_BYTES_V5);

        let mut profile_substitution = first.account_profile.clone();
        profile_substitution[10] = 1;
        assert!(AccountProfileV3::decode(&profile_substitution).is_err());
        let mut effect_substitution = first.effect;
        effect_substitution[6] = 1;
        assert!(ProgramV5::decode(&effect_substitution).is_err());
    }

    #[test]
    fn close_profile_satisfies_hot_prefix_permissions_and_product_tail_agreement() {
        let artifacts = emit_series_close_funding_artifacts_v5().expect("Close artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile)
            .expect("ProfileV3")
            .base();
        let tail_count = 3_u32;

        let mut permissions = [AccountPermission::read_only(); 7];
        derive_effect_permissions(profile, tail_count, &mut permissions)
            .expect("fixed permission geometry");
        for coordinate in 1..HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 {
            assert_eq!(permissions[coordinate], AccountPermission::read_only());
        }

        let rent_program = [0x92; 32];
        let keys = [
            [0x10; 32], [0x20; 32], [0x30; 32], [0x40; 32], [0x50; 32], [0x60; 32], [0x70; 32],
        ];
        let owners = [[0x11; 32]; 7];
        let root = vec![0_u8; SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5];
        let refund_owner = [0x93; 32];
        let mut config = vec![0_u8; SERIES_TEMPLATE_BYTES_V3];
        let template_refund_offset =
            dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3;
        config[template_refund_offset..template_refund_offset + 32].copy_from_slice(&refund_owner);
        let product = vec![0_u8; usize::try_from(SERIES_PRODUCT_RECORD_BYTES_V5).expect("width")];
        let mut portfolio = vec![
            0_u8;
            PORTFOLIO_HEADER_BYTES
                + usize::try_from(tail_count).expect("N")
                    * PORTFOLIO_COEFFICIENT_BYTES
        ];
        portfolio[PORTFOLIO_COEFFICIENT_COUNT_OFFSET..PORTFOLIO_COEFFICIENT_COUNT_OFFSET + 4]
            .copy_from_slice(&tail_count.to_le_bytes());
        let basis = [0x51_u8; 9];
        let mut credit = vec![0_u8; LIFECYCLE_RENT_CREDIT_BYTES_V2];
        credit[LIFECYCLE_RENT_CREDIT_REFUND_WALLET_OFFSET_V2 as usize
            ..LIFECYCLE_RENT_CREDIT_REFUND_WALLET_OFFSET_V2 as usize + 32]
            .copy_from_slice(&refund_owner);
        let rent_data = [];
        let observations = [
            AccountObservationV1::new(&keys[0], &owners[0], 80, &root, false, true, false),
            AccountObservationV1::new(&keys[1], &owners[1], 1, &config, false, false, false),
            AccountObservationV1::new(&keys[2], &owners[2], 1, &product, false, false, false),
            AccountObservationV1::new(&keys[3], &owners[3], 1, &portfolio, false, false, false),
            AccountObservationV1::new(&keys[4], &owners[4], 1, &basis, false, false, false),
            AccountObservationV1::new(&keys[5], &rent_program, 1, &credit, false, true, false),
            AccountObservationV1::new(&rent_program, &owners[6], 1, &rent_data, false, false, true),
        ];
        let input_scalars = [0_u64; 8];
        let mut input_identities = [[0_u8; 32]; 9];
        input_identities[usize::from(SERIES_CLOSE_TRADING_PROGRAM_IDENTITY_V5)] = owners[0];
        let mut scratch_scalars = [0_u64; 8];
        let mut scratch_identities = [[0_u8; 32]; 9];
        let mut output_scalars = [0_u64; 8];
        let mut output_identities = [[0_u8; 32]; 9];
        project_atomic(
            profile,
            tail_count,
            &observations,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("Hot-equivalent account projection");
        assert_eq!(
            output_scalars[usize::from(SERIES_CLOSE_PRODUCT_TAIL_COUNT_SCALAR_V5)],
            u64::from(tail_count)
        );
        assert_eq!(
            output_identities[usize::from(SERIES_CLOSE_BENEFICIARY_IDENTITY_V5)],
            refund_owner
        );
        assert_eq!(
            output_identities[usize::from(SERIES_CLOSE_TEMPLATE_REFUND_OWNER_IDENTITY_V5)],
            refund_owner
        );
        let transition = TransitionProgramV3::decode(&artifacts.transition).expect("transition");
        let mut transition_scratch_scalars = [0_u64; 8];
        let mut transition_scratch_identities = [[0_u8; 32]; 9];
        let mut transition_output_scalars = [0_u64; 8];
        let mut transition_output_identities = [[0_u8; 32]; 9];
        execute_fold_atomic(
            transition,
            tail_count,
            RegisterInput {
                scalars: &output_scalars,
                identities: &output_identities,
            },
            RegisterOutput {
                scalars: &mut transition_scratch_scalars,
                identities: &mut transition_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut transition_output_scalars,
                identities: &mut transition_output_identities,
            },
        )
        .expect("immutable Template owner equals authenticated credit beneficiary");

        let mut substituted = output_identities;
        substituted[usize::from(SERIES_CLOSE_BENEFICIARY_IDENTITY_V5)] = [0xa4; 32];
        let mut refused_scalars = [0x55_u64; 8];
        let before_scalars = refused_scalars;
        let mut refused_identities = [[0x55_u8; 32]; 9];
        let before_identities = refused_identities;
        assert!(
            execute_fold_atomic(
                transition,
                tail_count,
                RegisterInput {
                    scalars: &output_scalars,
                    identities: &substituted,
                },
                RegisterOutput {
                    scalars: &mut transition_scratch_scalars,
                    identities: &mut transition_scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut refused_scalars,
                    identities: &mut refused_identities,
                },
            )
            .is_err()
        );
        assert_eq!(refused_scalars, before_scalars);
        assert_eq!(refused_identities, before_identities);
        assert_eq!(profile.fixed_account_count(), 7);
        assert_eq!(SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5, 5);
        assert_eq!(SERIES_CLOSE_RENT_PROGRAM_COORDINATE_V5, 6);
    }
}
