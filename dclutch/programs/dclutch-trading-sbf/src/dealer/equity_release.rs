//! Schema-bound V4 release artifacts for Dealer junior equity selectors 1..=6.
//!
//! Equity's V3 account profile authenticates an already-live LP Position; it
//! never admits a vacant position.  The V5 lifecycle artifact is consequently
//! empty: Add and Remove acquire neither creation nor close authority.  Effect
//! V4 adds the one successor fact V3 could not state: P1/P2 lend exactly the
//! complete SignedDelta suffix to the sole Claims route.

extern crate alloc;

use alloc::vec;

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, HEADER_BYTES as LIFECYCLE_HEADER_BYTES_V5,
        StateLifecyclePolicyV5, encode::encode_lifecycle_policy_v5_atomic,
    },
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2},
};
use dclutch_capability_program_contract::CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1;
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::config_v4::DEALER_CONFIG_SCHEMA_PREIMAGE_V4;
use dclutch_effect_kernel::{
    v3::ProgramV3 as EffectProgramV3,
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4,
        HEADER_BYTES_V4 as EFFECT_V4_HEADER_BYTES, ProgramV4, RequestCoordinateV4,
        SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4, encode_program_v4_atomic,
    },
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::{
    SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1, v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID,
};
use dclutch_transition_vm::v3::{ProgramV3 as TransitionProgramV3, SCHEMA_RELEASE_ID};
use solana_program::hash::hash;

use super::{
    DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
    equity_artifacts::{
        authenticate_dealer_equity_artifacts_v3, dealer_equity_request_profile_bytes_v3,
        dealer_equity_transition_bytes_v3, dealer_equity_witness_bounds_v3,
        encode_dealer_equity_request_profile_v3, encode_dealer_equity_transition_v3,
    },
    equity_request::DEALER_EQUITY_HEADER_BYTES_V3,
    equity_effect::{
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3, DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3,
        dealer_equity_effect_program_bytes_v3, dealer_equity_identity_count_v3,
        dealer_equity_scalar_count_v3, encode_dealer_equity_effect_base_for_v4,
        encode_dealer_equity_effect_program_v3,
    },
    multi_lp::{MultiLpActionV3, MultiLpCustodyRequestV3},
    equity_profile::{
        DealerEquityAccountProfileInputV3, dealer_equity_logical_account_count_v3,
        encode_dealer_equity_account_profile_v3,
    },
    release::dealer_request_schema_v3,
};

/// Exact canonical empty V5 lifecycle width for every live equity selector.
pub const DEALER_EQUITY_LIFECYCLE_BYTES_V5: usize = LIFECYCLE_HEADER_BYTES_V5;

/// Exact V4 effect width for one action/P shape.
pub fn dealer_equity_effect_bytes_v4(
    action: MultiLpActionV3,
    signed_position_count: u32,
) -> Result<usize, DealerEquityReleaseErrorV4> {
    let base = dealer_equity_effect_program_bytes_v3(action, signed_position_count)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let ranges = usize::from(signed_position_count != 0);
    EFFECT_V4_HEADER_BYTES
        .checked_add(
            ranges
                .checked_mul(BORROWED_RANGE_BYTES_V4)
                .ok_or(DealerEquityReleaseErrorV4::Geometry)?,
        )
        .and_then(|value| value.checked_add(base))
        .ok_or(DealerEquityReleaseErrorV4::Geometry)
}

/// Stable refusal from V4 equity artifact generation or finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityReleaseErrorV4 {
    /// Selector/P shape or exact artifact width differed.
    Geometry,
    /// An artifact was absent, substituted, or hostile-decode invalid.
    Artifact,
    /// The selected strategy was not admitted AOT over the exact transition.
    Strategy,
    /// Descriptor schemas or immutable Dealer identities differed.
    Descriptor,
}

/// Exact finalized inputs for one selector-1..=6 V4 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityFinalizedArtifactsV4<'a> {
    /// Existing V3 physical-profile input; its rules require live local state.
    pub account_profile_input: DealerEquityAccountProfileInputV3<'a>,
    /// Exact AccountProfile V2 artifact.
    pub account_profile: &'a [u8],
    /// Exact empty V5 lifecycle artifact.
    pub lifecycle_policy: &'a [u8],
    /// Exact immutable physical capacity profile.
    pub capacity_profile: &'a [u8],
    /// Exact Effect V4 artifact around the canonical V3 effect.
    pub effect: &'a [u8],
    /// Exact P0/P1/P2 RequestProfile artifact.
    pub request_profile: &'a [u8],
    /// Exact admitted-AOT ExecutionStrategy V2 artifact.
    pub execution_strategy: &'a [u8],
    /// Exact underlying Transition V3 artifact.
    pub transition: &'a [u8],
    /// Exact typed Custody templates in the V3 route order.
    pub custody_templates: &'a [MultiLpCustodyRequestV3],
}

/// Encode the canonical no-authority V5 lifecycle policy for equity.
///
/// The live LP Position is authenticated by the V3 AccountProfile and semantic
/// executor.  Declaring a lifecycle recipe here would create a parallel owner
/// for a state that equity neither creates nor closes.
pub fn encode_dealer_equity_lifecycle_v5(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityReleaseErrorV4> {
    if scratch.len() != DEALER_EQUITY_LIFECYCLE_BYTES_V5
        || output.len() != DEALER_EQUITY_LIFECYCLE_BYTES_V5
    {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], scratch, output)
        .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    let id = digest(output);
    let policy = StateLifecyclePolicyV5::decode_selected(id, id, output)
        .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    if !policy.is_empty() {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }
    Ok(())
}

/// Encode canonical V3 equity semantics inside the sole Effect V4 schema.
pub fn encode_dealer_equity_effect_v4(
    action: MultiLpActionV3,
    signed_position_count: u32,
    custody_templates: &[MultiLpCustodyRequestV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityReleaseErrorV4> {
    let expected = dealer_equity_effect_bytes_v4(action, signed_position_count)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }
    let base_bytes = dealer_equity_effect_program_bytes_v3(action, signed_position_count)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_dealer_equity_effect_base_for_v4(
        action,
        signed_position_count,
        custody_templates,
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    let ranges = signed_delta_suffix_ranges(signed_position_count)?;
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?,
        &[],
        ranges.as_slice(),
        scratch,
        output,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    validate_effect_v4(action, signed_position_count, &base, output)?;
    Ok(())
}

/// Finalize one selector-1..=6 `CapabilityProgramV4` after exact rederivation.
pub fn finalize_dealer_equity_descriptor_v4(
    artifacts: DealerEquityFinalizedArtifactsV4<'_>,
) -> Result<[u8; CAPABILITY_PROGRAM_V4_BYTES], DealerEquityReleaseErrorV4> {
    if artifacts.capacity_profile.is_empty() || artifacts.execution_strategy.is_empty() {
        return Err(DealerEquityReleaseErrorV4::Artifact);
    }
    validate_generated_artifacts(artifacts)?;
    let input = artifacts.account_profile_input;
    let selector = equity_selector(input.action, input.signed_position_count)?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.execution_strategy)
        .map_err(|_| DealerEquityReleaseErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transition_schema().to_bytes() != SCHEMA_RELEASE_ID
        || strategy.transition_program().to_bytes() != digest(artifacts.transition)
    {
        return Err(DealerEquityReleaseErrorV4::Strategy);
    }
    let lifecycle_program = content(digest(artifacts.lifecycle_policy))?;
    let request_schema = if input.signed_position_count == 0 {
        REQUEST_PROFILE_SCHEMA_ID_V1
    } else {
        REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
    };
    let descriptor = CapabilityProgramV4::new(
        content(digest(DEALER_KIND_PREIMAGE_V2))?,
        content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))?,
        dealer_request_schema_v3(selector).map_err(|_| DealerEquityReleaseErrorV4::Descriptor)?,
        content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2))?,
        // Per-root constant, never this selector's lifecycle digest: a manifest
        // carries ONE `child_derivation_id` per root and `validate_selection`
        // requires this field to equal it, so a per-action value here admits
        // exactly one selector per root. The descriptor still binds its own
        // lifecycle by content digest in `artifacts.lifecycle` below.
        content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)?,
        content(digest(artifacts.capacity_profile))?,
        CapabilityArtifactsV4 {
            account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V2, artifacts.account_profile)?,
            request_profile: reference(request_schema, artifacts.request_profile)?,
            lifecycle: ArtifactReferenceV4::new(
                content(CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5)?,
                lifecycle_program,
            ),
            strategy: reference(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                artifacts.execution_strategy,
            )?,
            transition: reference(SCHEMA_RELEASE_ID, artifacts.transition)?,
            effect: reference(EFFECT_SCHEMA_ID_V4, artifacts.effect)?,
        },
        u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Descriptor)?;
    strategy
        .validate_descriptor_selection_v4(
            content(digest(artifacts.execution_strategy))?,
            descriptor,
        )
        .map_err(|_| DealerEquityReleaseErrorV4::Strategy)?;
    Ok(descriptor.encode())
}

fn validate_generated_artifacts(
    artifacts: DealerEquityFinalizedArtifactsV4<'_>,
) -> Result<(), DealerEquityReleaseErrorV4> {
    let input = artifacts.account_profile_input;
    let account_count =
        dealer_equity_logical_account_count_v3(input.action, input.signed_position_count)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    if input.logical_data_lengths.len() != usize::from(account_count) {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }
    let expected_profile = encode_dealer_equity_account_profile_v3(input)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    require_exact(&expected_profile, artifacts.account_profile)?;
    let profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    if profile.fixed_account_count() != account_count
        || profile.item_account_stride() != 0
        || usize::from(profile.common_scalar_count())
            != dealer_equity_scalar_count_v3(input.action)
                .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?
        || profile.item_scalar_stride() != 0
        || usize::from(profile.common_identity_count())
            != dealer_equity_identity_count_v3(input.action)
                .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?
        || profile.item_identity_stride() != 0
    {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }

    let mut lifecycle_scratch = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
    let mut lifecycle = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
    encode_dealer_equity_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)?;
    require_exact(&lifecycle, artifacts.lifecycle_policy)?;
    let lifecycle_id = digest(artifacts.lifecycle_policy);
    let policy = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        artifacts.lifecycle_policy,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    policy
        .validate_account_profile(profile)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let selector = equity_selector(input.action, input.signed_position_count)?;
    if !policy.is_empty() || policy.action_plan_count(u32::from(selector)) != Ok(0) {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }

    let request_bytes = dealer_equity_request_profile_bytes_v3(input.signed_position_count)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut request_scratch = vec![0_u8; request_bytes];
    let mut request = vec![0_u8; request_bytes];
    encode_dealer_equity_request_profile_v3(
        input.action,
        input.signed_position_count,
        &mut request_scratch,
        &mut request,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    require_exact(&request, artifacts.request_profile)?;

    let transition_bytes = dealer_equity_transition_bytes_v3(input.signed_position_count)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut transition_scratch = vec![0_u8; transition_bytes];
    let mut transition = vec![0_u8; transition_bytes];
    encode_dealer_equity_transition_v3(
        input.action,
        input.signed_position_count,
        &mut transition_scratch,
        &mut transition,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    require_exact(&transition, artifacts.transition)?;
    TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;

    let effect_bytes = dealer_equity_effect_bytes_v4(input.action, input.signed_position_count)?;
    let mut effect_scratch = vec![0_u8; effect_bytes];
    let mut effect = vec![0_u8; effect_bytes];
    encode_dealer_equity_effect_v4(
        input.action,
        input.signed_position_count,
        artifacts.custody_templates,
        &mut effect_scratch,
        &mut effect,
    )?;
    require_exact(&effect, artifacts.effect)?;

    ProgramV4::decode(artifacts.effect).map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    let legacy_effect_bytes =
        dealer_equity_effect_program_bytes_v3(input.action, input.signed_position_count)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut legacy_effect_scratch = vec![0_u8; legacy_effect_bytes];
    let mut legacy_effect = vec![0_u8; legacy_effect_bytes];
    encode_dealer_equity_effect_program_v3(
        input.action,
        input.signed_position_count,
        artifacts.custody_templates,
        &mut legacy_effect_scratch,
        &mut legacy_effect,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let scalars = dealer_equity_scalar_count_v3(input.action)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let identities = dealer_equity_identity_count_v3(input.action)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut scalar_scratch = vec![0_u64; scalars];
    let mut identity_scratch = vec![[0_u8; 32]; identities];
    authenticate_dealer_equity_artifacts_v3(
        input.action,
        input.signed_position_count,
        artifacts.request_profile,
        artifacts.transition,
        &legacy_effect,
        &mut scalar_scratch,
        &mut identity_scratch,
    )
    .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    Ok(())
}

fn validate_effect_v4(
    action: MultiLpActionV3,
    signed_position_count: u32,
    base: &[u8],
    output: &[u8],
) -> Result<(), DealerEquityReleaseErrorV4> {
    let effect = ProgramV4::decode(output).map_err(|_| DealerEquityReleaseErrorV4::Artifact)?;
    if effect.base().bytes() != base
        || EffectProgramV3::decode(effect.base().bytes()).is_err()
        || effect.semantic_prefix_bytes()
            != u32::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
                .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?
        || effect.borrowed_range_policy() != BorrowedRangePolicyV4::DisjointExactCoverage
    {
        return Err(DealerEquityReleaseErrorV4::Artifact);
    }
    let expected_ranges = signed_delta_suffix_ranges(signed_position_count)?;
    if usize::from(effect.range_count()) != expected_ranges.len() {
        return Err(DealerEquityReleaseErrorV4::Geometry);
    }
    for (index, expected) in expected_ranges.iter().copied().enumerate() {
        if effect
            .borrowed_range(u16::try_from(index).map_err(|_| DealerEquityReleaseErrorV4::Geometry)?)
            != Ok(expected)
        {
            return Err(DealerEquityReleaseErrorV4::Geometry);
        }
    }
    let scalar_count =
        dealer_equity_scalar_count_v3(action).map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let identity_count = dealer_equity_identity_count_v3(action)
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    let mut scalars = vec![0_u64; scalar_count];
    let identities = vec![[0_u8; 32]; identity_count];
    let suffix = if signed_position_count == 0 {
        0_u32
    } else {
        dealer_equity_witness_bounds_v3(signed_position_count)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?
            .0
    };
    *scalars
        .get_mut(usize::from(DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3))
        .ok_or(DealerEquityReleaseErrorV4::Geometry)? =
        u64::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
            .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?;
    *scalars
        .get_mut(usize::from(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3))
        .ok_or(DealerEquityReleaseErrorV4::Geometry)? = u64::from(suffix);
    effect
        .validate_request_coverage(
            DEALER_EQUITY_HEADER_BYTES_V3
                .checked_add(
                    usize::try_from(suffix).map_err(|_| DealerEquityReleaseErrorV4::Geometry)?,
                )
                .ok_or(DealerEquityReleaseErrorV4::Geometry)?,
            0,
            &scalars,
            &identities,
        )
        .map_err(|_| DealerEquityReleaseErrorV4::Geometry)
}

fn signed_delta_suffix_ranges(
    signed_position_count: u32,
) -> Result<alloc::vec::Vec<BorrowedRangeV4>, DealerEquityReleaseErrorV4> {
    match signed_position_count {
        0 => Ok(vec![]),
        1 | 2 => Ok(vec![BorrowedRangeV4::new(
            1,
            RequestCoordinateV4::Fixed(
                u32::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
                    .map_err(|_| DealerEquityReleaseErrorV4::Geometry)?,
            ),
            RequestCoordinateV4::CommonScalar(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3),
        )]),
        _ => Err(DealerEquityReleaseErrorV4::Geometry),
    }
}

fn equity_selector(
    action: MultiLpActionV3,
    signed_position_count: u32,
) -> Result<u16, DealerEquityReleaseErrorV4> {
    use super::equity_request::{
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3, DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3,
        DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3, DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
        DEALER_EQUITY_REDEEM_P1_SELECTOR_V3, DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
    };
    match (action, signed_position_count) {
        (MultiLpActionV3::Add, 0) => Ok(DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3),
        (MultiLpActionV3::Add, 1) => Ok(DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3),
        (MultiLpActionV3::Add, 2) => Ok(DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3),
        (MultiLpActionV3::Remove, 0) => Ok(DEALER_EQUITY_REDEEM_P0_SELECTOR_V3),
        (MultiLpActionV3::Remove, 1) => Ok(DEALER_EQUITY_REDEEM_P1_SELECTOR_V3),
        (MultiLpActionV3::Remove, 2) => Ok(DEALER_EQUITY_REDEEM_P2_SELECTOR_V3),
        _ => Err(DealerEquityReleaseErrorV4::Geometry),
    }
}

fn reference(
    schema: [u8; 32],
    bytes: &[u8],
) -> Result<ArtifactReferenceV4, DealerEquityReleaseErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(digest(bytes))?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DealerEquityReleaseErrorV4> {
    ContentId::new(bytes).map_err(|_| DealerEquityReleaseErrorV4::Artifact)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

fn require_exact(expected: &[u8], actual: &[u8]) -> Result<(), DealerEquityReleaseErrorV4> {
    if expected == actual {
        Ok(())
    } else {
        Err(DealerEquityReleaseErrorV4::Geometry)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use dclutch_custody_contract::{
        CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, DelegatedCustodyRequestV2,
        OperationV1,
    };
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    };

    use super::super::equity_profile::LINKED_BASIS_CONTENT_ACCOUNT_V3;
    use super::*;

    fn transfer(
        source_compartment: CompartmentV1,
        destination_compartment: CompartmentV1,
        marker: u8,
    ) -> CustodyRequestV1 {
        let source_external = source_compartment == CompartmentV1::External;
        let destination_external = destination_compartment == CompartmentV1::External;
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment,
            destination_compartment,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            context: [4; 32],
            caller_program: [5; 32],
            semantic: ContextV1 {
                candidate: [6; 32],
                source_owner: if source_external { [7; 32] } else { [0; 32] },
                destination_owner: if destination_external {
                    [8; 32]
                } else {
                    [0; 32]
                },
                order: [9; 32],
                parent_request_digest: [10; 32],
                order_nonce: 11,
                generation: 12,
                page_index: 13,
                execution_index: 14,
                transfer_index: u16::from(marker),
            },
            source: [marker; 32],
            destination: [marker.saturating_add(1); 32],
            source_vault_context: if source_external { [0; 32] } else { [15; 32] },
            destination_vault_context: if destination_external {
                [0; 32]
            } else {
                [16; 32]
            },
            mint: [17; 32],
            token_program: [18; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 19,
            resulting_revision: 20,
            amount: 21,
            rent_lamports: 0,
        }
    }

    fn templates(action: MultiLpActionV3) -> Vec<MultiLpCustodyRequestV3> {
        match action {
            MultiLpActionV3::Add => {
                let custody =
                    transfer(CompartmentV1::External, CompartmentV1::TradingPrincipal, 22);
                vec![
                    MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
                        custody,
                        starts_atomic_debit: true,
                        terminal: true,
                        delegate_before: [31; 32],
                        delegate_after: [0; 32],
                        total_debit: custody.amount,
                        allowance_before: custody.amount,
                        allowance_after: 0,
                    }),
                    MultiLpCustodyRequestV3::Canonical(transfer(
                        CompartmentV1::HoardPrincipal,
                        CompartmentV1::TradingPrincipal,
                        24,
                    )),
                ]
            }
            MultiLpActionV3::Remove => vec![
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::HoardPrincipal,
                    22,
                )),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::External,
                    24,
                )),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::HoardPrincipal,
                    CompartmentV1::TradingPrincipal,
                    26,
                )),
            ],
        }
    }

    fn admitted_strategy(transition: &[u8]) -> Vec<u8> {
        ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            content(SCHEMA_RELEASE_ID).expect("transition schema"),
            content(digest(transition)).expect("transition program"),
            content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("certificate schema"),
            Some(content([0x71; 32]).expect("certificate")),
            content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("admission schema"),
            Some(content([0x72; 32]).expect("admission")),
            content(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("request schema"),
            content(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("ack schema"),
        )
        .expect("admitted strategy")
        .to_bytes()
        .to_vec()
    }

    #[test]
    fn v5_lifecycle_has_no_equity_create_or_close_authority() {
        let mut scratch = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        let mut output = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        encode_dealer_equity_lifecycle_v5(&mut scratch, &mut output).expect("lifecycle");
        let id = digest(&output);
        let policy = StateLifecyclePolicyV5::decode_selected(id, id, &output).expect("decode");
        assert!(policy.is_empty());
        for selector in 1_u32..=6 {
            assert_eq!(policy.action_plan_count(selector), Ok(0));
        }
    }

    #[test]
    fn p1_and_p2_effects_cover_only_the_complete_signed_delta_suffix() {
        for (action, positions) in [
            (MultiLpActionV3::Add, 0),
            (MultiLpActionV3::Add, 1),
            (MultiLpActionV3::Remove, 2),
        ] {
            let custody = templates(action);
            let bytes = dealer_equity_effect_bytes_v4(action, positions).expect("width");
            let mut scratch = vec![0_u8; bytes];
            let mut output = vec![0_u8; bytes];
            encode_dealer_equity_effect_v4(action, positions, &custody, &mut scratch, &mut output)
                .expect("effect");
            let effect = ProgramV4::decode(&output).expect("decode");
            assert_eq!(effect.semantic_prefix_bytes(), 480);
            assert_eq!(effect.range_count(), u16::from(positions != 0));
            let base_claims = effect.base().route(1).expect("Claims route");
            assert!(!base_claims.borrows_witness());
            for route in 0..effect.base().route_count() {
                assert!(
                    !effect
                        .base()
                        .route(route)
                        .expect("base route")
                        .borrows_witness(),
                    "Effect V4 is the sole owner of every borrowed request range"
                );
            }
            if positions != 0 {
                assert_eq!(effect.borrowed_range_count_for_route(1), Ok(1));
                assert_eq!(
                    effect.borrowed_range(0),
                    Ok(signed_delta_suffix_ranges(positions).expect("range")[0])
                );
            }
        }
    }

    #[test]
    fn descriptor_rederives_profile_effect_and_range_artifacts() {
        let action = MultiLpActionV3::Add;
        let positions = 1;
        let custody = templates(action);
        let mut lengths = vec![
            0_u32;
            usize::from(
                dealer_equity_logical_account_count_v3(action, positions).expect("account count"),
            )
        ];
        // The linked-basis content coordinate is the topology's one
        // `AdapterAuthenticatedVariableData` rule, and its declared width is a
        // FLOOR the runtime enforces (`account.data().len() < exact_data_length`
        // refuses), not a knowable exact width -- which is why the prestate is
        // variable at all. A zero floor promises nothing and the encoder refuses
        // it, so the minimum honest floor is one byte, exactly as `equity_profile`'s
        // own `every_equity_shape_emits_exact_live_profile` declares it.
        *lengths
            .get_mut(usize::from(LINKED_BASIS_CONTENT_ACCOUNT_V3))
            .expect("linked-basis content coordinate") = 1;
        let input = DealerEquityAccountProfileInputV3 {
            action,
            signed_position_count: positions,
            logical_data_lengths: &lengths,
        };
        let profile = encode_dealer_equity_account_profile_v3(input).expect("profile");
        let mut lifecycle_scratch = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        let mut lifecycle = vec![0_u8; DEALER_EQUITY_LIFECYCLE_BYTES_V5];
        encode_dealer_equity_lifecycle_v5(&mut lifecycle_scratch, &mut lifecycle)
            .expect("lifecycle");
        let request_bytes =
            dealer_equity_request_profile_bytes_v3(positions).expect("request width");
        let mut request_scratch = vec![0_u8; request_bytes];
        let mut request = vec![0_u8; request_bytes];
        encode_dealer_equity_request_profile_v3(
            action,
            positions,
            &mut request_scratch,
            &mut request,
        )
        .expect("request");
        let transition_bytes =
            dealer_equity_transition_bytes_v3(positions).expect("transition width");
        let mut transition_scratch = vec![0_u8; transition_bytes];
        let mut transition = vec![0_u8; transition_bytes];
        encode_dealer_equity_transition_v3(
            action,
            positions,
            &mut transition_scratch,
            &mut transition,
        )
        .expect("transition");
        let effect_bytes = dealer_equity_effect_bytes_v4(action, positions).expect("effect width");
        let mut effect_scratch = vec![0_u8; effect_bytes];
        let mut effect = vec![0_u8; effect_bytes];
        encode_dealer_equity_effect_v4(
            action,
            positions,
            &custody,
            &mut effect_scratch,
            &mut effect,
        )
        .expect("effect");
        let strategy = admitted_strategy(&transition);
        let artifacts = DealerEquityFinalizedArtifactsV4 {
            account_profile_input: input,
            account_profile: &profile,
            lifecycle_policy: &lifecycle,
            capacity_profile: &[1],
            effect: &effect,
            request_profile: &request,
            execution_strategy: &strategy,
            transition: &transition,
            custody_templates: &custody,
        };
        let descriptor = finalize_dealer_equity_descriptor_v4(artifacts).expect("descriptor");
        let decoded = CapabilityProgramV4::decode(&descriptor).expect("decode descriptor");
        assert_eq!(
            decoded.request_schema(),
            dealer_request_schema_v3(2).expect("schema")
        );
        assert_eq!(decoded.effect().schema().to_bytes(), EFFECT_SCHEMA_ID_V4);

        let mut substituted_profile = profile.clone();
        *substituted_profile.last_mut().expect("profile byte") ^= 1;
        assert_eq!(
            finalize_dealer_equity_descriptor_v4(DealerEquityFinalizedArtifactsV4 {
                account_profile: &substituted_profile,
                ..artifacts
            }),
            Err(DealerEquityReleaseErrorV4::Geometry)
        );
        let mut substituted_effect = effect.clone();
        *substituted_effect.last_mut().expect("effect byte") ^= 1;
        assert_eq!(
            finalize_dealer_equity_descriptor_v4(DealerEquityFinalizedArtifactsV4 {
                effect: &substituted_effect,
                ..artifacts
            }),
            Err(DealerEquityReleaseErrorV4::Geometry)
        );
        let mut substituted_range = effect.clone();
        *substituted_range
            .get_mut(EFFECT_V4_HEADER_BYTES)
            .expect("borrowed-range route byte") ^= 1;
        assert_eq!(
            finalize_dealer_equity_descriptor_v4(DealerEquityFinalizedArtifactsV4 {
                effect: &substituted_range,
                ..artifacts
            }),
            Err(DealerEquityReleaseErrorV4::Geometry)
        );
    }
}
