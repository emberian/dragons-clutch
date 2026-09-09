//! Generic complete-support retirement through the existing Claims V2 owner.
//!
//! The request's span width is an untrusted transport hint. Claims authenticates
//! every ordered nonzero coefficient against the finalized descriptor before
//! it closes the Mint. Neither coefficients nor support coordinates enter the
//! selected release. The five-account stride is Claims' vacancy ABI; the
//! maximum is this release's existing representation width. EffectV4's finite
//! extension bitset is a chain transport constraint: lifting it requires its
//! canonical successor, never silently narrowing the representation width.

use super::{
    Error, RationalLifecycleSelectedBundleInputV6, RationalLifecycleSelectedBundleV6, Result,
};
use dclutch_claims::rational_lifecycle::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_HEADER_BYTES_V2, LIFECYCLE_RECEIPT_BYTES_V2,
    LIFECYCLE_RECEIPT_MAGIC_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LifecycleActionV2,
    hot_v6::{
        DYNAMIC_RETIREMENT_MAGIC_V1, DYNAMIC_RETIREMENT_PREFIX_BYTES_V1,
        STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_market::capability_program::v4::{ArtifactReferenceV4, CapabilityProgramV4};
use dclutch_vm::account_profile::v2::{
    self as ap, AccountPrestateV2, AccountProfileV2, TrustedBuiltinIdentityV2,
    TrustedEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountOperationInputV2,
        AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2, RegisterGeometryV2,
        ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_vm::effect::{
    v2::FixedRole,
    v3::{
        self as e3, RouteKindV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
    },
    v4::{
        self as e4, BorrowedRangePolicyV4, BorrowedRangeV4, DynamicFixedSpanV4, ProgramV4,
        RequestCoordinateV4, encode_program_v4_atomic,
    },
};
use dclutch_vm::request_profile::{
    self as rp,
    encode::{
        RequestCoordinateV1 as C, RequestGeometryV1, RequestInstructionV1 as R,
        ScalarRegisterV1 as S, encode_request_profile_v1_atomic,
    },
    v3::{
        BorrowedWitnessPolicyV3, BorrowedWitnessRoleV3, REQUEST_PROFILE_V3_HEADER_BYTES,
        REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID, RequestProfileV3, encode_request_profile_v3_atomic,
    },
};
use dclutch_vm::v3::{
    self as vm, InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};
use solana_program::hash::hash;

const SCALARS: u16 = 11;
const IDENTITIES: u16 = 11;
const SPAN_SCALAR: u16 = 7;
const BYTES_SCALAR: u16 = 8;
const PRODUCT_SCALAR: u16 = 10;
const FIXED: usize = 5 + LIFECYCLE_COMMON_ACCOUNT_COUNT_V2;

fn u16n(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}
fn u32n(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::InvalidLength)
}
fn max_accounts(max_support: u32) -> Result<u32> {
    let max = max_support
        .checked_mul(u32n(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)?)
        .ok_or(Error::InvalidLength)?;
    if max_support == 0 || max >= u64::BITS {
        return Err(Error::ActionGeometry);
    }
    Ok(max)
}
/// Compile a market-free retirement bundle for every sparse support within the
/// selected representation width. `input` supplies the 25 common observations.
pub fn build_dynamic_retirement_bundle_v1(
    input: RationalLifecycleSelectedBundleInputV6<'_>,
    maximum_support: u32,
) -> Result<RationalLifecycleSelectedBundleV6> {
    if input.action != LifecycleActionV2::RetireReceipt {
        return Err(Error::ActionGeometry);
    }
    max_accounts(maximum_support)?;
    let counter = super::resource_counter::enabled(input.root_schema, input.root_state_bytes)?;
    let profile = account_profile(
        input.account_profile.logical_data_lengths,
        maximum_support,
        counter,
    )?;
    super::selected_bundle_v6::assemble_selected_bundle_v6(
        input,
        profile,
        request_profile(maximum_support, counter)?,
        transition(counter)?,
        effect(maximum_support, counter)?,
        hash(b"dclutch/schema/rational-lifecycle-dynamic-retirement-v1").to_bytes(),
        REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID,
    )
}

fn account_profile(lengths: &[u32], maximum_support: u32, counter: bool) -> Result<Vec<u8>> {
    if lengths.len() != FIXED {
        return Err(Error::AccountObservation);
    }
    let mut rules = Vec::with_capacity(FIXED);
    for index in 0..FIXED {
        let mut rule =
            super::account_profile::rule(LifecycleActionV2::RetireReceipt, index, lengths)?;
        if index == 0 {
            super::resource_counter::root_rule(&mut rule, counter)?;
        }
        rule.alias = AccountAliasInputV2::SelfCoordinate;
        let opaque = matches!(index, 6 | 7 | 8 | 9 | 10 | 13 | 17 | 18 | 20 | 23 | 24);
        let prestate = match index {
            4 => {
                rule.data_length =
                    u32n(dclutch_product::payoff::runtime_v3::BASIS_HEADER_BYTES_V3)?;
                AccountPrestateV2::AdapterAuthenticatedVariableData
            }
            14 => {
                // Claims owns descriptor content authentication and support.
                rule.data_length = 0;
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            }
            _ if opaque => {
                rule.data_length = 0;
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            }
            _ => AccountPrestateV2::Exact,
        };
        rules.push(AccountRuleWithPrestateInputV2 { rule, prestate });
    }
    let mut vacant = super::account_profile::rule(LifecycleActionV2::RetireReceipt, 0, &[0])?;
    vacant.privileges = ap::encode::AccountPrivilegesV2::new(false, false, false);
    vacant.alias = AccountAliasInputV2::SelfCoordinate;
    // Claims distinguishes inert authority names from resources, and checks
    // exact resource vacancy. The transport authenticates only readonly names.
    let span_rules = [AccountRuleWithPrestateInputV2 {
        rule: vacant,
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }];
    let spans = [DynamicFixedSpanInputV2 {
        insertion_coordinate: u16n(FIXED)?,
        count_scalar: SPAN_SCALAR,
        rule_start: 0,
        rule_stride: 1,
        minimum: u32n(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)?,
        maximum: max_accounts(maximum_support)?,
        step: u32n(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)?,
    }];
    let mut operations = vec![AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(PRODUCT_SCALAR),
        data_offset: u32n(dclutch_product::payoff::runtime_v3::BASIS_WIDTH_OFFSET_V3)?,
    }];
    super::resource_counter::project(
        &mut operations,
        usize::from(SCALARS),
        usize::from(IDENTITIES),
        counter,
    )?;
    let size = ap::DYNAMIC_FIXED_SPAN_HEADER_BYTES
        + ap::DYNAMIC_FIXED_SPAN_ENTRY_BYTES
        + (rules.len() + span_rules.len()) * ap::RULE_BYTES
        + operations.len() * ap::OPERATION_BYTES;
    let mut scratch = vec![0; size];
    let mut output = vec![0; size];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        super::resource_counter::trusted_identity(usize::from(IDENTITIES), counter)?,
        TrustedBuiltinIdentityV2::None,
        &spans,
        &rules,
        &span_rules,
        &operations,
        RegisterGeometryV2 {
            common_scalars: u16n(super::resource_counter::scalar_count(
                usize::from(SCALARS),
                counter,
            )?)?,
            item_scalar_stride: 0,
            common_identities: u16n(super::resource_counter::identity_count(
                usize::from(IDENTITIES),
                counter,
            )?)?,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfile)?;
    Ok(output)
}

fn request_profile(_maximum_support: u32, counter: bool) -> Result<Vec<u8>> {
    let ops = [
        R::require_u64(C::fixed(0), u64::from_le_bytes(DYNAMIC_RETIREMENT_MAGIC_V1)),
        R::require_u16(C::fixed(8), 1),
        R::require_u8(
            C::fixed(10),
            u8::try_from(STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1)
                .map_err(|_| Error::InvalidLength)?,
        ),
        R::require_zero(C::fixed(11), 1),
        R::project_u32(C::fixed(12), S::common(SPAN_SCALAR)),
        R::project_u32(C::fixed(16), S::common(BYTES_SCALAR)),
        R::require_zero(C::fixed(20), 4),
    ];
    let size = rp::HEADER_BYTES + ops.len() * rp::OPERATION_BYTES;
    let mut scratch = vec![0; size];
    let mut base = vec![0; size];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32n(DYNAMIC_RETIREMENT_PREFIX_BYTES_V1)?,
            0,
            u16n(super::resource_counter::scalar_count(
                usize::from(SCALARS),
                counter,
            )?)?,
            0,
            u16n(super::resource_counter::identity_count(
                usize::from(IDENTITIES),
                counter,
            )?)?,
            0,
        ),
        &ops,
        &[],
        &mut scratch,
        &mut base,
    )
    .map_err(Error::RequestProfile)?;
    let mut scratch = vec![0; REQUEST_PROFILE_V3_HEADER_BYTES + base.len()];
    let mut output = scratch.clone();
    encode_request_profile_v3_atomic(
        &base,
        BorrowedWitnessPolicyV3 {
            minimum_bytes: u32n(LIFECYCLE_HEADER_BYTES_V2)?,
            maximum_bytes: u32n(LIFECYCLE_HEADER_BYTES_V2)?,
            consumer_role: BorrowedWitnessRoleV3::Claims,
            child_request_magic: dclutch_claims::rational_lifecycle::compact_hot_v4::RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4,
            child_receipt_magic: LIFECYCLE_RECEIPT_MAGIC_V2,
            child_receipt_bytes: u32n(LIFECYCLE_RECEIPT_BYTES_V2)?,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| Error::ArtifactGeometry)?;
    Ok(output)
}

fn transition(counter: bool) -> Result<Vec<u8>> {
    // Preserve both request-derived transport scalars. Claims checks their
    // meaning from the authenticated complete child; no economic fact is minted.
    let mut ops = vec![
        InstructionV3::nonzero(ScalarRegisterV3::common(SPAN_SCALAR)),
        InstructionV3::nonzero(ScalarRegisterV3::common(BYTES_SCALAR)),
    ];
    super::resource_counter::transition(
        &mut ops,
        LifecycleActionV2::RetireReceipt,
        usize::from(SCALARS),
        counter,
    )?;
    let size = vm::HEADER_BYTES + ops.len() * vm::INSTRUCTION_BYTES;
    let mut scratch = vec![0; size];
    let mut output = vec![0; size];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: u16n(super::resource_counter::scalar_count(
                usize::from(SCALARS),
                counter,
            )?)?,
            item_scalar_stride: 0,
            common_identities: u16n(super::resource_counter::identity_count(
                usize::from(IDENTITIES),
                counter,
            )?)?,
            item_identity_stride: 0,
        },
        &ops,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::Transition)?;
    Ok(output)
}

fn effect(maximum_support: u32, counter: bool) -> Result<Vec<u8>> {
    let routes = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: 5,
        fixed_account_count: u16n(LIFECYCLE_COMMON_ACCOUNT_COUNT_V2)?,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &[],
        item_request: &[],
    }];
    let mut ops = Vec::new();
    super::resource_counter::effect(&mut ops, usize::from(SCALARS), counter)?;
    let size = e3::HEADER_BYTES + e3::ROUTE_BYTES + ops.len() * e3::OPERATION_BYTES;
    let mut scratch = vec![0; size];
    let mut base = vec![0; size];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: u16n(FIXED)?,
            item_account_stride: 0,
            common_scalars: u16n(super::resource_counter::scalar_count(
                usize::from(SCALARS),
                counter,
            )?)?,
            item_scalar_stride: 0,
            common_identities: u16n(super::resource_counter::identity_count(
                usize::from(IDENTITIES),
                counter,
            )?)?,
            item_identity_stride: 0,
        },
        &routes,
        &ops,
        &[],
        &mut scratch,
        &mut base,
    )
    .map_err(Error::Effect)?;
    let mut allowed = 0_u64;
    for support in 1..=maximum_support {
        allowed |= 1_u64
            .checked_shl(max_accounts(support)?)
            .ok_or(Error::InvalidLength)?;
    }
    let spans = [DynamicFixedSpanV4::new(
        0,
        SPAN_SCALAR,
        u16n(LIFECYCLE_COMMON_ACCOUNT_COUNT_V2)?,
        allowed,
    )];
    let ranges = [BorrowedRangeV4::new(
        0,
        RequestCoordinateV4::Fixed(u32n(DYNAMIC_RETIREMENT_PREFIX_BYTES_V1)?),
        RequestCoordinateV4::CommonScalar(BYTES_SCALAR),
    )];
    let size =
        e4::HEADER_BYTES_V4 + base.len() + e4::DYNAMIC_SPAN_BYTES_V4 + e4::BORROWED_RANGE_BYTES_V4;
    let mut scratch = vec![0; size];
    let mut output = vec![0; size];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32n(DYNAMIC_RETIREMENT_PREFIX_BYTES_V1)?,
        &spans,
        &ranges,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::EffectV4)?;
    Ok(output)
}

pub(super) fn validate_dynamic_retirement_bundle_v1(
    bundle: &RationalLifecycleSelectedBundleV6,
) -> Result<()> {
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::Descriptor)?;
    let counter = super::resource_counter::enabled(
        descriptor.root_schema().to_bytes(),
        descriptor.root_state_bytes(),
    )?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfile)?;
    if bundle.action != LifecycleActionV2::RetireReceipt
        || account.dynamic_fixed_span_count() != 1
        || usize::from(account.fixed_account_count()) != FIXED
    {
        return Err(Error::ArtifactGeometry);
    }
    let span = account
        .dynamic_fixed_span(0)
        .map_err(Error::AccountProfile)?;
    let maximum_support = span.maximum() / u32n(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)?;
    let lengths = (0..account.fixed_account_count())
        .map(|i| {
            account
                .rule(false, i)
                .map(|rule| rule.data_length())
                .map_err(Error::AccountProfile)
        })
        .collect::<Result<Vec<_>>>()?;
    if bundle.account_profile != account_profile(&lengths, maximum_support, counter)?
        || bundle.request_profile != request_profile(maximum_support, counter)?
        || bundle.transition != transition(counter)?
        || bundle.effect != effect(maximum_support, counter)?
    {
        return Err(Error::ArtifactGeometry);
    }
    RequestProfileV3::decode(&bundle.request_profile).map_err(|_| Error::ArtifactGeometry)?;
    ProgramV4::decode(&bundle.effect).map_err(Error::EffectV4)?;
    let selection = dclutch_custody::token_svm::TokenBehaviorSelectionV2::decode(
        &bundle.token_behavior_selection,
    )
    .map_err(Error::TokenBehavior)?;
    let reference = |schema: [u8; 32], bytes: &[u8]| -> Result<ArtifactReferenceV4> {
        Ok(ArtifactReferenceV4::new(
            ContentId::new(schema).map_err(|_| Error::ContentIdentity)?,
            ContentId::new(hash(bytes).to_bytes()).map_err(|_| Error::ContentIdentity)?,
        ))
    };
    let strategy = dclutch_market::execution_strategy::v2::ExecutionStrategyProgramV2::decode(
        &bundle.strategy,
    )
    .map_err(Error::Strategy)?;
    if bundle.release_set != selection.release_set()
        || bundle.token_program != selection.token_program()
        || descriptor.request_schema().to_bytes()
            != hash(b"dclutch/schema/rational-lifecycle-dynamic-retirement-v1").to_bytes()
        || descriptor.account_profile()
            != reference(ap::SCHEMA_RELEASE_ID, &bundle.account_profile)?
        || descriptor.request_profile()
            != reference(
                REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID,
                &bundle.request_profile,
            )?
        || descriptor.transition() != reference(vm::SCHEMA_RELEASE_ID, &bundle.transition)?
        || descriptor.effect() != reference(e4::SCHEMA_RELEASE_ID_V4, &bundle.effect)?
        || descriptor.lifecycle()
            != reference(
                dclutch_vm::account_profile::lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
                &bundle.lifecycle_policy,
            )?
        || descriptor.strategy()
            != reference(
                dclutch_market::execution_strategy::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                &bundle.strategy,
            )?
        || strategy.transition_program() != descriptor.transition().program()
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.disposition()
            != dclutch_market::execution_strategy::v2::StrategyDispositionV2::Interpreted
    {
        return Err(Error::ArtifactGeometry);
    }
    dclutch_vm::account_profile::lifecycle_v3::StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        hash(&bundle.lifecycle_policy).to_bytes(),
        &bundle.lifecycle_policy,
    )
    .map_err(Error::LifecycleArtifact)?
    .validate_account_profile(account)
    .map_err(Error::LifecycleArtifact)?;
    Ok(())
}
