//! Compact complete-support receipt-retirement artifacts.
//!
//! The family request remains the exact fixed 400-byte `DCRLHC04` header.
//! Product N and ordered nonzero descriptor rows are derived from authenticated
//! accounts. The immutable effect artifact synthesizes the sole runtime-width
//! `DCRRLC02` Claims child; no wallet-supplied coordinate DTO survives.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE,
    OPERATION_BYTES as ACCOUNT_OPERATION_BYTES, RULE_BYTES as ACCOUNT_RULE_BYTES,
    TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
    encode::{
        AccountCoordinateV2, AccountOperationInputV2, AccountRuleWithPrestateInputV2,
        IdentityCoordinateV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic,
    },
};
use dclutch_capability_program_contract::v3::{CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES, RouteKindV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
    },
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_product_payoff_v2_codec::runtime_v3::{BASIS_WIDTH_OFFSET_V3, ProductBasisV3};
use dclutch_rational_representation_v2_contract::RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2;
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_HEADER_BYTES, RepresentationDescriptorV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    ABSENT_POSITION_REVISION_V2, LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_BYTES_V2,
    LIFECYCLE_HEADER_BYTES_V2, LIFECYCLE_REQUEST_MAGIC_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2,
    LIFECYCLE_VERSION_V2, LifecycleActionV2,
    compact_hot_v4::{
        RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4, RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4,
        RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4,
        RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_ADMISSION_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_POSITION_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_SHARD_MINT_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_STRUCTURED_CUSTODY_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_COEFFICIENT_V4,
        RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_OUTCOME_V4,
        RATIONAL_LIFECYCLE_COMPACT_SCALAR_PRODUCT_OUTCOME_COUNT_V4,
        RATIONAL_LIFECYCLE_COMPACT_SCALAR_SUPPORT_COUNT_V4, RationalLifecycleCompactHotLayoutV4,
        RationalLifecycleCompactHotRegisterLayoutV4,
    },
    hot_v3::{
        RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3, RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3, RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3, RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
        RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3, RATIONAL_LIFECYCLE_SCALAR_ACTION_V3,
        RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3, RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3,
        RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3, RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3, RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3,
        RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3, RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3,
        RationalLifecycleHotLayoutV3,
    },
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
    InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::{
    Error, Result,
    account_profile::rule,
    effect::{
        RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3, append_header_instructions, narrow_u16,
        narrow_u32, put, write_identity, write_u32, write_u64,
    },
};

const BASE_REQUEST_OPERATIONS: usize = 24;
/// Exact interpreted strategy width for compact retirement.
pub const RATIONAL_LIFECYCLE_COMPACT_STRATEGY_BYTES_V4: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgram descriptor width for compact retirement.
pub const RATIONAL_LIFECYCLE_COMPACT_DESCRIPTOR_BYTES_V4: usize = CAPABILITY_PROGRAM_V3_BYTES;
const VACANCY_LOGICAL_ACCOUNT_START: usize =
    RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3 as usize + LIFECYCLE_COMMON_ACCOUNT_COUNT_V2;

/// Same-finalized immutable inputs for one compact RetireReceipt artifact set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleCompactArtifactInputV4<'a> {
    /// Exact logical account data lengths in injected-prefix plus Claims order.
    pub logical_data_lengths: &'a [u32],
    /// Exact finalized ProductBasisV3 bytes authenticated at logical account four.
    pub product_basis: &'a [u8],
    /// Authenticated immutable Rational descriptor selected by the capability.
    pub descriptor: RepresentationDescriptorV2<'a>,
    /// Exact selected Claims program deriving the custody-owner PDA literals.
    pub claims_program: Pubkey,
}

/// Exact compact account/request/transition/effect artifact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleCompactArtifactsV4 {
    /// Descriptor-derived positive support width.
    pub support_count: u32,
    /// Exact logical account interpreter.
    pub account_profile: Vec<u8>,
    /// Fixed 400-byte family request interpreter.
    pub request_profile: Vec<u8>,
    /// Exact Product-width and ordered-support transition.
    pub transition: Vec<u8>,
    /// Sole Claims `Once` effect synthesizing `400 + 272*K` bytes.
    pub effect: Vec<u8>,
}

/// Registry-selected content identities wrapping one compact artifact set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleCompactBundleInputV4<'a> {
    /// Same-finalized descriptor/Product/account observations.
    pub artifacts: RationalLifecycleCompactArtifactInputV4<'a>,
    /// Manifest-selected Rational lifecycle capability kind.
    pub kind: [u8; 32],
    /// Manifest-selected mutable root-tail schema.
    pub root_schema: [u8; 32],
    /// Finalized StateLifecyclePolicy content identity.
    pub derivation_policy: [u8; 32],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root-tail byte width.
    pub root_state_bytes: u32,
}

/// Exact bytes finalized as one compact RetireReceipt capability bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleCompactBundleV4 {
    /// Descriptor-derived positive support width.
    pub support_count: u32,
    /// Exact logical account interpreter.
    pub account_profile: Vec<u8>,
    /// Exact fixed family request interpreter.
    pub request_profile: Vec<u8>,
    /// Exact economic transition interpreter.
    pub transition: Vec<u8>,
    /// Interpreted strategy selecting `transition`.
    pub strategy: [u8; RATIONAL_LIFECYCLE_COMPACT_STRATEGY_BYTES_V4],
    /// Sole Claims effect interpreter.
    pub effect: Vec<u8>,
    /// Descriptor selecting every exact artifact content identity.
    pub descriptor: [u8; RATIONAL_LIFECYCLE_COMPACT_DESCRIPTOR_BYTES_V4],
}

/// Build exact stateless compact RetireReceipt artifacts.
pub fn encode_rational_lifecycle_compact_artifacts_v4(
    input: RationalLifecycleCompactArtifactInputV4<'_>,
) -> Result<RationalLifecycleCompactArtifactsV4> {
    let rows = support_rows(input.descriptor)?;
    let support_count = u32::try_from(rows.len()).map_err(|_| Error::InvalidLength)?;
    let layout = RationalLifecycleCompactHotRegisterLayoutV4::new(rows.len());
    validate_inputs(input, rows.len())?;
    Ok(RationalLifecycleCompactArtifactsV4 {
        support_count,
        account_profile: encode_account_profile(input, layout)?,
        request_profile: encode_request_profile(layout)?,
        transition: encode_transition(layout)?,
        effect: encode_effect(input, layout, &rows)?,
    })
}

/// Build and hostile-join one content-addressed compact capability bundle.
pub fn build_rational_lifecycle_compact_bundle_v4(
    input: RationalLifecycleCompactBundleInputV4<'_>,
) -> Result<RationalLifecycleCompactBundleV4> {
    let artifacts = encode_rational_lifecycle_compact_artifacts_v4(input.artifacts)?;
    let strategy_value = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        digest(&artifacts.transition)?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(Error::Strategy)?;
    let strategy = strategy_value.to_bytes();
    let descriptor_value = CapabilityProgramV3::new(
        content(input.kind)?,
        content(
            dclutch_rational_representation_v2_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        )?,
        content(RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4)?,
        content(input.root_schema)?,
        digest(&artifacts.account_profile)?,
        content(input.derivation_policy)?,
        content(input.capacity_profile)?,
        digest(&artifacts.effect)?,
        content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        digest(&artifacts.request_profile)?,
        content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
        digest(&strategy)?,
        input.root_state_bytes,
    )
    .map_err(Error::Descriptor)?;
    let bundle = RationalLifecycleCompactBundleV4 {
        support_count: artifacts.support_count,
        account_profile: artifacts.account_profile,
        request_profile: artifacts.request_profile,
        transition: artifacts.transition,
        strategy,
        effect: artifacts.effect,
        descriptor: descriptor_value.encode(),
    };
    validate_rational_lifecycle_compact_bundle_v4(&bundle)?;
    Ok(bundle)
}

/// Hostile-decode and join every compact capability artifact.
pub fn validate_rational_lifecycle_compact_bundle_v4(
    bundle: &RationalLifecycleCompactBundleV4,
) -> Result<()> {
    let support = usize::try_from(bundle.support_count).map_err(|_| Error::InvalidLength)?;
    if support == 0 {
        return Err(Error::ArtifactGeometry);
    }
    let layout = RationalLifecycleCompactHotRegisterLayoutV4::new(support);
    let descriptor = CapabilityProgramV3::decode(&bundle.descriptor).map_err(Error::Descriptor)?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfile)?;
    let request = RequestProfileV1::decode_selected(
        descriptor.request_profile_program().to_bytes(),
        hash(&bundle.request_profile).to_bytes(),
        &bundle.request_profile,
    )
    .map_err(Error::RequestProfile)?;
    let transition = dclutch_transition_vm::v3::ProgramV3::decode(&bundle.transition)
        .map_err(Error::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy).map_err(Error::Strategy)?;
    let effect = dclutch_effect_kernel::v3::ProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        hash(&bundle.effect).to_bytes(),
        &bundle.effect,
    )
    .map_err(Error::Effect)?;
    strategy
        .validate_descriptor_selection(digest(&bundle.strategy)?, descriptor)
        .map_err(Error::Strategy)?;
    let common_scalars = narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?;
    let common_identities = narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?;
    let claims_accounts = support
        .checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
        .and_then(|tail| LIFECYCLE_COMMON_ACCOUNT_COUNT_V2.checked_add(tail))
        .ok_or(Error::InvalidLength)?;
    let logical_accounts = usize::from(RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3)
        .checked_add(claims_accounts)
        .ok_or(Error::InvalidLength)?;
    let child_bytes = support
        .checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
        .and_then(|tail| LIFECYCLE_HEADER_BYTES_V2.checked_add(tail))
        .ok_or(Error::InvalidLength)?;
    let support_projection = account
        .nonzero_u64_tail_rows_projection()
        .map_err(Error::AccountProfile)?
        .ok_or(Error::ArtifactGeometry)?;
    if descriptor.config_schema().to_bytes()
        != dclutch_rational_representation_v2_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3
        || descriptor.request_schema().to_bytes()
            != RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4
        || descriptor.account_profile() != digest(&bundle.account_profile)?
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.request_profile_program() != digest(&bundle.request_profile)?
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition_program() != digest(&bundle.strategy)?
        || descriptor.effect_program() != digest(&bundle.effect)?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || strategy.transition_program() != digest(&bundle.transition)?
        || account.artifact_profile() != NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
        || usize::from(account.fixed_account_count()) != logical_accounts
        || support_projection.account() != 1
        || usize::from(support_projection.count_destination())
            != RATIONAL_LIFECYCLE_COMPACT_SCALAR_SUPPORT_COUNT_V4
        || usize::from(support_projection.rows_destination())
            != layout.row_scalar(0, 0).ok_or(Error::InvalidLength)?
        || usize::try_from(support_projection.tail_offset()).ok() != Some(DESCRIPTOR_HEADER_BYTES)
        || support_projection.row_scalar_stride() != 2
        || request.fixed_request_bytes()
            != u32::try_from(RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4)
                .map_err(|_| Error::InvalidLength)?
        || request.item_request_bytes() != 0
        || usize::from(effect.fixed_account_count()) != logical_accounts
        || !geometry_matches(
            account,
            request,
            transition,
            effect,
            common_scalars,
            common_identities,
        )
    {
        return Err(Error::ArtifactGeometry);
    }
    let route = effect.route(0).map_err(Error::Effect)?;
    if effect.route_count() != 1
        || effect.receipt_dependency_count() != 0
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3
        || usize::from(route.fixed_account_count()) != claims_accounts
        || usize::try_from(route.fixed_request_bytes()).ok() != Some(child_bytes)
        || route.item_account_count() != 0
        || route.item_request_bytes() != 0
        || route.receipt_dependency_count() != 0
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

fn geometry_matches(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: dclutch_transition_vm::v3::ProgramV3<'_>,
    effect: dclutch_effect_kernel::v3::ProgramV3<'_>,
    scalars: u16,
    identities: u16,
) -> bool {
    scalars == account.common_scalar_count()
        && scalars == request.common_scalar_count()
        && scalars == transition.common_scalar_count()
        && scalars == effect.common_scalar_count()
        && identities == account.common_identity_count()
        && identities == request.common_identity_count()
        && identities == transition.common_identity_count()
        && identities == effect.common_identity_count()
        && account.item_scalar_stride() == 0
        && request.item_scalar_stride() == 0
        && transition.item_scalar_stride() == 0
        && effect.item_scalar_stride() == 0
        && account.item_identity_stride() == 0
        && request.item_identity_stride() == 0
        && transition.item_identity_stride() == 0
        && effect.item_identity_stride() == 0
}

fn digest(bytes: &[u8]) -> Result<ContentId> {
    content(hash(bytes).to_bytes())
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::ContentIdentity)
}

fn validate_inputs(input: RationalLifecycleCompactArtifactInputV4<'_>, rows: usize) -> Result<()> {
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountObservation)?;
    let expected_accounts = VACANCY_LOGICAL_ACCOUNT_START
        .checked_add(
            rows.checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    if input.claims_program == Pubkey::default()
        || rows == 0
        || basis.basis_width() != input.descriptor.outcome_count()
        || input.logical_data_lengths.len() != expected_accounts
        || input.logical_data_lengths.get(4).copied()
            != u32::try_from(input.product_basis.len()).ok()
        || input.logical_data_lengths.get(1).copied()
            != u32::try_from(
                DESCRIPTOR_HEADER_BYTES
                    .checked_add(
                        usize::try_from(input.descriptor.outcome_count())
                            .ok()
                            .and_then(|width| width.checked_mul(8))
                            .ok_or(Error::InvalidLength)?,
                    )
                    .ok_or(Error::InvalidLength)?,
            )
            .ok()
        || input.logical_data_lengths.get(14) != input.logical_data_lengths.get(1)
    {
        return Err(Error::AccountObservation);
    }
    Ok(())
}

fn support_rows(descriptor: RepresentationDescriptorV2<'_>) -> Result<Vec<(u32, u64)>> {
    let mut rows = Vec::new();
    for outcome in 0..descriptor.outcome_count() {
        let coefficient = descriptor
            .coefficient(outcome)
            .map_err(|_| Error::AccountObservation)?;
        if coefficient != 0 {
            rows.push((outcome, coefficient));
        }
    }
    if rows.is_empty() {
        return Err(Error::AccountObservation);
    }
    Ok(rows)
}

fn encode_account_profile(
    input: RationalLifecycleCompactArtifactInputV4<'_>,
    layout: RationalLifecycleCompactHotRegisterLayoutV4,
) -> Result<Vec<u8>> {
    let mut rules = Vec::with_capacity(input.logical_data_lengths.len());
    for index in 0..input.logical_data_lengths.len() {
        let mut value = rule(
            LifecycleActionV2::RetireReceipt,
            index,
            input.logical_data_lengths,
        )?;
        let prestate = match index {
            1 => {
                value.data_length = narrow_u32(DESCRIPTOR_HEADER_BYTES)?;
                AccountPrestateV2::AdapterAuthenticatedVariableData
            }
            14 => {
                value.data_length = 0;
                AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
            }
            _ => AccountPrestateV2::Exact,
        };
        rules.push(AccountRuleWithPrestateInputV2 {
            rule: value,
            prestate,
        });
    }
    let mut operations = Vec::with_capacity(
        layout
            .support_count()
            .checked_mul(4)
            .and_then(|count| count.checked_add(2))
            .ok_or(Error::InvalidLength)?,
    );
    operations.push(AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(narrow_u16(
            RATIONAL_LIFECYCLE_COMPACT_SCALAR_PRODUCT_OUTCOME_COUNT_V4,
        )?),
        data_offset: narrow_u32(BASIS_WIDTH_OFFSET_V3)?,
    });
    operations.push(AccountOperationInputV2::ProjectNonzeroU64TailRows {
        account: AccountCoordinateV2::fixed(1),
        count_destination: ScalarCoordinateV2::common(narrow_u16(
            RATIONAL_LIFECYCLE_COMPACT_SCALAR_SUPPORT_COUNT_V4,
        )?),
        rows_destination: ScalarCoordinateV2::common(narrow_u16(
            layout.row_scalar(0, 0).ok_or(Error::InvalidLength)?,
        )?),
        tail_offset: narrow_u32(DESCRIPTOR_HEADER_BYTES)?,
        row_scalar_stride: 2,
    });
    for row in 0..layout.support_count() {
        let account_start = VACANCY_LOGICAL_ACCOUNT_START
            .checked_add(
                row.checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        for field in 0..4 {
            operations.push(AccountOperationInputV2::ProjectKey {
                account: AccountCoordinateV2::fixed(narrow_u16(
                    account_start
                        .checked_add(field)
                        .ok_or(Error::InvalidLength)?,
                )?),
                destination: IdentityCoordinateV2::common(narrow_u16(
                    layout
                        .row_identity(row, field)
                        .ok_or(Error::InvalidLength)?,
                )?),
            });
        }
    }
    let geometry = RegisterGeometryV2 {
        common_scalars: narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_RULE_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .and_then(|prefix| {
            operations
                .len()
                .checked_mul(ACCOUNT_OPERATION_BYTES)
                .and_then(|ops| prefix.checked_add(ops))
        })
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        &rules,
        &[],
        &operations,
        &[],
        geometry,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfile)?;
    Ok(output)
}

fn encode_request_profile(layout: RationalLifecycleCompactHotRegisterLayoutV4) -> Result<Vec<u8>> {
    let instructions = base_request_instructions()?;
    let geometry = RequestGeometryV1::new(
        narrow_u32(RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4)?,
        0,
        narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        0,
        narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        0,
    );
    let bytes = REQUEST_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(REQUEST_OPERATION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_request_profile_v1_atomic(geometry, &instructions, &[], &mut scratch, &mut output)
        .map_err(Error::RequestProfile)?;
    Ok(output)
}

fn encode_transition(layout: RationalLifecycleCompactHotRegisterLayoutV4) -> Result<Vec<u8>> {
    let mut instructions = Vec::with_capacity(
        layout
            .support_count()
            .checked_mul(2)
            .and_then(|count| count.checked_add(1))
            .ok_or(Error::InvalidLength)?,
    );
    instructions.push(InstructionV3::scalar_eq(
        scalar_v3(RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3)?,
        scalar_v3(RATIONAL_LIFECYCLE_COMPACT_SCALAR_PRODUCT_OUTCOME_COUNT_V4)?,
    ));
    for row in 0..layout.support_count() {
        instructions.push(InstructionV3::scalar_lt(
            scalar_v3(
                layout
                    .row_scalar(row, RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_OUTCOME_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
            scalar_v3(RATIONAL_LIFECYCLE_COMPACT_SCALAR_PRODUCT_OUTCOME_COUNT_V4)?,
        ));
        instructions.push(InstructionV3::nonzero(scalar_v3(
            layout
                .row_scalar(row, RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_COEFFICIENT_V4)
                .ok_or(Error::InvalidLength)?,
        )?));
    }
    let geometry = ProgramGeometryV3 {
        common_scalars: narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = TRANSITION_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(TRANSITION_INSTRUCTION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_program_atomic(geometry, &instructions, &[], &[], &mut scratch, &mut output)
        .map_err(Error::Transition)?;
    Ok(output)
}

fn encode_effect(
    input: RationalLifecycleCompactArtifactInputV4<'_>,
    layout: RationalLifecycleCompactHotRegisterLayoutV4,
    rows: &[(u32, u64)],
) -> Result<Vec<u8>> {
    let template = child_template(input, rows)?;
    let claims_accounts = LIFECYCLE_COMMON_ACCOUNT_COUNT_V2
        .checked_add(
            rows.len()
                .checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let logical_accounts = RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(narrow_u16(claims_accounts)?)
        .ok_or(Error::InvalidLength)?;
    let routes = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3,
        fixed_account_count: narrow_u16(claims_accounts)?,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let mut instructions = Vec::with_capacity(
        rows.len()
            .checked_mul(6)
            .and_then(|count| count.checked_add(18))
            .ok_or(Error::InvalidLength)?,
    );
    append_header_instructions(&mut instructions)?;
    for row in 0..rows.len() {
        let base = LIFECYCLE_HEADER_BYTES_V2
            .checked_add(
                row.checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let at = |field: usize| base.checked_add(field).ok_or(Error::InvalidLength);
        instructions.extend([
            write_u32(
                at(RationalLifecycleHotLayoutV3::ITEM_OUTCOME)?,
                layout
                    .row_scalar(row, RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_OUTCOME_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
            write_u64(
                at(RationalLifecycleHotLayoutV3::ITEM_COEFFICIENT)?,
                layout
                    .row_scalar(row, RATIONAL_LIFECYCLE_COMPACT_ROW_SCALAR_COEFFICIENT_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
            write_identity(
                at(RationalLifecycleHotLayoutV3::ITEM_SHARD_MINT)?,
                layout
                    .row_identity(row, RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_SHARD_MINT_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
            write_identity(
                at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_CUSTODY)?,
                layout
                    .row_identity(
                        row,
                        RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_STRUCTURED_CUSTODY_V4,
                    )
                    .ok_or(Error::InvalidLength)?,
            )?,
            write_identity(
                at(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_POSITION)?,
                layout
                    .row_identity(row, RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_POSITION_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
            write_identity(
                at(RationalLifecycleHotLayoutV3::ITEM_POSITION_ADMISSION)?,
                layout
                    .row_identity(row, RATIONAL_LIFECYCLE_COMPACT_ROW_IDENTITY_ADMISSION_V4)
                    .ok_or(Error::InvalidLength)?,
            )?,
        ]);
    }
    let geometry = EffectGeometryV3 {
        fixed_accounts: logical_accounts,
        item_account_stride: 0,
        common_scalars: narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = EFFECT_HEADER_BYTES
        .checked_add(EFFECT_ROUTE_BYTES)
        .and_then(|prefix| {
            instructions
                .len()
                .checked_mul(EFFECT_OPERATION_BYTES)
                .and_then(|ops| prefix.checked_add(ops))
        })
        .and_then(|prefix| prefix.checked_add(template.len()))
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_effect_program_v3_atomic(
        geometry,
        &routes,
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::Effect)?;
    Ok(output)
}

fn child_template(
    input: RationalLifecycleCompactArtifactInputV4<'_>,
    rows: &[(u32, u64)],
) -> Result<Vec<u8>> {
    let bytes = rows
        .len()
        .checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
        .and_then(|tail| LIFECYCLE_HEADER_BYTES_V2.checked_add(tail))
        .ok_or(Error::InvalidLength)?;
    let mut output = vec![0_u8; bytes];
    put(&mut output, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
    put(&mut output, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
    put(
        &mut output,
        RationalLifecycleCompactHotLayoutV4::ACTION,
        &[LifecycleActionV2::RetireReceipt.tag()],
    )?;
    put(
        &mut output,
        RationalLifecycleCompactHotLayoutV4::COORDINATE_COUNT,
        &u32::try_from(rows.len())
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    for (row, (outcome, _)) in rows.iter().copied().enumerate() {
        let base = LIFECYCLE_HEADER_BYTES_V2
            .checked_add(
                row.checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let owner = Pubkey::find_program_address(
            &[
                RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
                input.descriptor.descriptor_id().as_slice(),
                &outcome.to_le_bytes(),
            ],
            &input.claims_program,
        )
        .0;
        put(
            &mut output,
            base.checked_add(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_OWNER)
                .ok_or(Error::InvalidLength)?,
            owner.as_ref(),
        )?;
        put(
            &mut output,
            base.checked_add(RationalLifecycleHotLayoutV3::ITEM_POSITION_REVISION)
                .ok_or(Error::InvalidLength)?,
            &ABSENT_POSITION_REVISION_V2.to_le_bytes(),
        )?;
    }
    Ok(output)
}

fn base_request_instructions() -> Result<[RequestInstructionV1; BASE_REQUEST_OPERATIONS]> {
    Ok([
        require_u64(
            RationalLifecycleCompactHotLayoutV4::MAGIC,
            u64::from_le_bytes(RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4),
        )?,
        require_u16(
            RationalLifecycleCompactHotLayoutV4::VERSION,
            RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4,
        )?,
        require_u8(
            RationalLifecycleCompactHotLayoutV4::ACTION,
            LifecycleActionV2::RetireReceipt.tag(),
        )?,
        require_zero(RationalLifecycleCompactHotLayoutV4::RESERVED_HEADER, 5)?,
        require_zero(RationalLifecycleCompactHotLayoutV4::PARENT_CONTEXT, 32)?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::RELEASE_SET,
            RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::MARKET,
            RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::GRAPH_ID,
            RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::DESCRIPTOR_ID,
            RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::REPRESENTATION_AUTHORITY,
            RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::RECEIPT_MINT,
            RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::TOKEN_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::RENT_CREDIT,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3,
        )?,
        project_identity(
            RationalLifecycleCompactHotLayoutV4::RENT_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
        )?,
        project_u8(
            RationalLifecycleCompactHotLayoutV4::ACTION,
            RATIONAL_LIFECYCLE_SCALAR_ACTION_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::GENERATION,
            RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::EXPECTED_MARKET_REVISION,
            RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::OBSERVED_RECEIPT_LAMPORTS,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::RECEIPT_RENT_PRINCIPAL,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::EXPECTED_RECEIPT_SUPPLY,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3,
        )?,
        project_u32(
            RationalLifecycleCompactHotLayoutV4::OUTCOME_COUNT,
            RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3,
        )?,
        require_u32(RationalLifecycleCompactHotLayoutV4::COORDINATE_COUNT, 0)?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::RENT_CREDIT_BEFORE,
            RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3,
        )?,
        project_u64(
            RationalLifecycleCompactHotLayoutV4::RENT_CREDIT_AFTER,
            RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3,
        )?,
    ])
}

fn request(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::fixed(narrow_u32(offset)?))
}

fn identity(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::common(narrow_u16(index)?))
}

fn scalar(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::common(narrow_u16(index)?))
}

fn scalar_v3(index: usize) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::common(narrow_u16(index)?))
}

fn require_u8(offset: usize, value: u8) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u8(request(offset)?, value))
}

fn require_u16(offset: usize, value: u16) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u16(request(offset)?, value))
}

fn require_u32(offset: usize, value: u32) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u32(request(offset)?, value))
}

fn require_u64(offset: usize, value: u64) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u64(request(offset)?, value))
}

fn require_zero(offset: usize, bytes: u32) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_zero(request(offset)?, bytes))
}

fn project_identity(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_identity(
        request(offset)?,
        identity(register)?,
    ))
}

fn project_u8(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u8(
        request(offset)?,
        scalar(register)?,
    ))
}

fn project_u32(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u32(
        request(offset)?,
        scalar(register)?,
    ))
}

fn project_u64(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u64(
        request(offset)?,
        scalar(register)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::v2::{
        AccountProfileV2, NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE,
    };
    use dclutch_effect_kernel::v3::ProgramV3 as EffectProgramV3;
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };
    use dclutch_rational_representation_v2_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_MAGIC_V3, DESCRIPTOR_SCHEMA_VERSION_V3,
        DescriptorAdmissionV2,
    };
    use dclutch_request_profile_contract::RequestProfileV1;
    use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: 5,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    fn descriptor_bytes(coefficients: &[u64; 5]) -> Vec<u8> {
        let mut output = vec![0_u8; DESCRIPTOR_HEADER_BYTES + 5 * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut output, 0, &DESCRIPTOR_MAGIC_V3).expect("magic");
        put(&mut output, 8, &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes()).expect("version");
        for (offset, value) in [
            (16, id(11)),
            (48, id(12)),
            (80, id(13)),
            (112, id(14)),
            (144, id(15)),
            (176, id(16)),
            (208, dclutch_token_svm::TOKEN_2022_PROGRAM_ID),
        ] {
            put(&mut output, offset, &value).expect("identity");
        }
        put(&mut output, 240, &5_u32.to_le_bytes()).expect("width");
        put(&mut output, 248, &10_u64.to_le_bytes()).expect("denominator");
        for (index, coefficient) in coefficients.iter().enumerate() {
            put(
                &mut output,
                DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
                &coefficient.to_le_bytes(),
            )
            .expect("coefficient");
        }
        output
    }

    fn descriptor<'a>(bytes: &'a [u8]) -> RepresentationDescriptorV2<'a> {
        RepresentationDescriptorV2::decode(
            bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: id(21),
                finalized_descriptor_id: id(21),
                recomputed_descriptor_digest: id(21),
                finalized_descriptor_digest: id(21),
                record_authenticated: true,
                derived_representation_authority: id(22),
                authority_derivation_authenticated: true,
            },
        )
        .expect("descriptor")
    }

    fn lengths(product_bytes: usize, descriptor_bytes: usize, support: usize) -> Vec<u32> {
        let count = VACANCY_LOGICAL_ACCOUNT_START + support * LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2;
        let mut output = vec![0_u32; count];
        *output.get_mut(1).expect("descriptor") =
            u32::try_from(descriptor_bytes).expect("descriptor width");
        *output.get_mut(4).expect("Product") = u32::try_from(product_bytes).expect("Product width");
        *output.get_mut(14).expect("descriptor alias") =
            u32::try_from(descriptor_bytes).expect("descriptor width");
        output
    }

    #[test]
    fn k3_compact_artifacts_keep_family_fixed_and_synthesize_exact_child() {
        let basis = basis();
        let descriptor_bytes = descriptor_bytes(&[0, 7, 5, 0, 9]);
        let lengths = lengths(basis.len(), descriptor_bytes.len(), 3);
        let input = RationalLifecycleCompactArtifactInputV4 {
            logical_data_lengths: &lengths,
            product_basis: &basis,
            descriptor: descriptor(&descriptor_bytes),
            claims_program: Pubkey::new_from_array(id(31)),
        };
        let artifacts =
            encode_rational_lifecycle_compact_artifacts_v4(input).expect("compact artifacts");
        assert_eq!(artifacts.support_count, 3);

        let account = AccountProfileV2::decode(&artifacts.account_profile).expect("account");
        let request = RequestProfileV1::decode(&artifacts.request_profile).expect("request");
        let transition = TransitionProgramV3::decode(&artifacts.transition).expect("transition");
        let effect = EffectProgramV3::decode(&artifacts.effect).expect("effect");
        let layout = RationalLifecycleCompactHotRegisterLayoutV4::new(3);
        assert_eq!(
            account.artifact_profile(),
            NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
        );
        assert_eq!(request.fixed_request_bytes(), 400);
        assert_eq!(request.item_request_bytes(), 0);
        assert_eq!(account.common_scalar_count(), 17);
        assert_eq!(account.common_identity_count(), 22);
        assert_eq!(transition.common_scalar_count(), 17);
        assert_eq!(effect.common_identity_count(), 22);
        assert_eq!(layout.scalar_count(), Some(17));
        let route = effect.route(0).expect("route");
        assert_eq!(route.role(), FixedRole::Claims);
        assert_eq!(route.kind(), RouteKindV3::Once);
        assert_eq!(route.fixed_account_start(), 5);
        assert_eq!(route.fixed_account_count(), 32);
        assert_eq!(route.fixed_request_bytes(), 1_216);
        assert_eq!(route.item_request_bytes(), 0);

        let bundle =
            build_rational_lifecycle_compact_bundle_v4(RationalLifecycleCompactBundleInputV4 {
                artifacts: input,
                kind: id(41),
                root_schema: id(42),
                derivation_policy: id(43),
                capacity_profile: id(44),
                root_state_bytes: 64,
            })
            .expect("content-addressed compact bundle");
        assert_eq!(bundle.support_count, 3);
        validate_rational_lifecycle_compact_bundle_v4(&bundle).expect("joined bundle");
        let mut substituted = bundle;
        assert!(
            substituted
                .effect
                .first_mut()
                .map(|byte| *byte ^= 1)
                .is_some()
        );
        assert!(validate_rational_lifecycle_compact_bundle_v4(&substituted).is_err());
    }

    #[test]
    fn descriptor_support_and_product_width_cannot_be_substituted() {
        let basis = basis();
        let descriptor_bytes = descriptor_bytes(&[0, 7, 5, 0, 9]);
        let short_lengths = lengths(basis.len(), descriptor_bytes.len(), 2);
        let input = RationalLifecycleCompactArtifactInputV4 {
            logical_data_lengths: &short_lengths,
            product_basis: &basis,
            descriptor: descriptor(&descriptor_bytes),
            claims_program: Pubkey::new_from_array(id(31)),
        };
        assert_eq!(
            encode_rational_lifecycle_compact_artifacts_v4(input),
            Err(Error::AccountObservation)
        );

        let lengths = lengths(basis.len(), descriptor_bytes.len(), 3);
        assert_eq!(
            encode_rational_lifecycle_compact_artifacts_v4(
                RationalLifecycleCompactArtifactInputV4 {
                    logical_data_lengths: &lengths,
                    product_basis: &basis,
                    descriptor: descriptor(&descriptor_bytes),
                    claims_program: Pubkey::default(),
                }
            ),
            Err(Error::AccountObservation)
        );
    }
}
