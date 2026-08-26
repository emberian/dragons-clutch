//! Finalized Dealer V3 descriptors and one global selector authority.
//!
//! The selector space is intentionally global across Dealer request wires:
//! junior equity occupies 1..=6, LP lifecycle 7..=8, and scenario exact-fill
//! 9. The ProgramSet encoder owns that ordering and never admits legacy aliases.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::{
    set_v1::{
        CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V1, CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1,
        CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1, CAPABILITY_PROGRAM_SET_MAGIC_V1,
        CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V1, CapabilityProgramSetV1, SelectorWidthV1,
    },
    v3::{CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3},
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::config_v3::DEALER_CONFIG_SCHEMA_PREIMAGE_V3;
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID;
use solana_program::hash::hash;

use super::{
    DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
    v3_artifacts::{DealerEquityArtifactsErrorV3, authenticate_dealer_equity_artifacts_v3},
    v3_equity_operator::{
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3, DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3,
        DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3, DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
        DEALER_EQUITY_REDEEM_P1_SELECTOR_V3, DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
        DEALER_EQUITY_SELECTOR_OFFSET_V3,
    },
    v3_hot_artifact::{dealer_equity_identity_count_v3, dealer_equity_scalar_count_v3},
    v3_multi_lp::MultiLpActionV3,
};

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::vec;

#[cfg(not(target_os = "solana"))]
use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV4,
    v2::{AccountPrestateV2, LIFECYCLE_PRESTATE_ARTIFACT_PROFILE},
};
#[cfg(not(target_os = "solana"))]
use dclutch_effect_kernel::v3::ProgramV3 as EffectProgramV3;
#[cfg(not(target_os = "solana"))]
use dclutch_request_profile_contract::RequestProfileV1;
#[cfg(not(target_os = "solana"))]
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;

#[cfg(not(target_os = "solana"))]
use super::{
    v3_lp_artifacts::{
        DEALER_LP_IDENTITY_COUNT_V3, DEALER_LP_LIFECYCLE_BYTES_V3,
        DEALER_LP_REQUEST_PROFILE_BYTES_V3, DEALER_LP_SCALAR_COUNT_V3, DEALER_LP_STATE_ACCOUNT_V3,
        DealerLpArtifactErrorV3, LP_CURRENT_SLOT_SCALAR_V3, dealer_lp_account_count_v3,
        dealer_lp_effect_bytes_v3, dealer_lp_transition_bytes_v3, encode_dealer_lp_effect_v3,
        encode_dealer_lp_lifecycle_v3, encode_dealer_lp_request_profile_v3,
        encode_dealer_lp_transition_v3,
    },
    v3_operator::{DEALER_MULTI_LP_REQUEST_BYTES_V3, MultiLpRequestActionV3},
};

/// Canonical junior-equity request schema label.
pub const DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-junior-equity-request-v3";
/// Canonical LP lifecycle request schema label.
pub const DEALER_MULTI_LP_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-multi-lp-request-v3";
/// Canonical scenario exact-fill request schema label.
pub const DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-scenario-trade-request-v4";

/// First selector in the sole canonical Dealer space.
pub const DEALER_GLOBAL_SELECTOR_MIN_V3: u16 = DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3;
/// Last selector in the sole canonical Dealer space.
pub const DEALER_GLOBAL_SELECTOR_MAX_V3: u16 = 9;
/// Exact canonical selector count.
pub const DEALER_GLOBAL_SELECTOR_COUNT_V3: usize = 9;
/// Exact encoded width of the nine-entry ProgramSet.
pub const DEALER_GLOBAL_PROGRAM_SET_BYTES_V3: usize = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1
    + DEALER_GLOBAL_SELECTOR_COUNT_V3 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1;

const PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V1: usize = 12;
const PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V1: usize = 16;
const PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V1: usize = 17;
const PROGRAM_SET_ENTRY_COUNT_OFFSET_V1: usize = 18;
const PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V1: usize = 0;
const PROGRAM_SET_ENTRY_PROGRAM_OFFSET_V1: usize = 4;

/// Stable finalized-descriptor or ProgramSet refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerReleaseErrorV3 {
    /// Selector was outside the one global Dealer authority.
    Selector,
    /// One finalized artifact or content identity was absent or malformed.
    Artifact,
    /// Exact Account/Request/Transition/Effect geometry did not join.
    Geometry,
    /// The selected strategy was not the admitted successor for the transition.
    Strategy,
    /// Descriptor schemas or common Dealer identity differed.
    Descriptor,
    /// Canonical ProgramSet encoding or hostile decode refused.
    ProgramSet,
}

impl From<DealerEquityArtifactsErrorV3> for DealerReleaseErrorV3 {
    fn from(_: DealerEquityArtifactsErrorV3) -> Self {
        Self::Geometry
    }
}

#[cfg(not(target_os = "solana"))]
impl From<DealerLpArtifactErrorV3> for DealerReleaseErrorV3 {
    fn from(_: DealerLpArtifactErrorV3) -> Self {
        Self::Geometry
    }
}

/// Exact finalized bytes needed to construct one equity descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityFinalizedArtifactsV3<'a> {
    /// Contribution or redemption route shape.
    pub action: MultiLpActionV3,
    /// SignedDelta Position count P0/P1/P2.
    pub signed_position_count: u32,
    /// Exact AccountProfile5 bytes for this physical frame.
    pub account_profile: &'a [u8],
    /// Exact StateLifecycle/root-derivation policy bytes.
    pub derivation_policy: &'a [u8],
    /// Exact physical capacity-profile bytes.
    pub capacity_profile: &'a [u8],
    /// Exact action/P EffectProgram bytes.
    pub effect_program: &'a [u8],
    /// Exact action/P RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact admitted ExecutionStrategyProgramV2 bytes.
    pub execution_strategy: &'a [u8],
    /// Exact strategy-selected TransitionVM bytes.
    pub transition: &'a [u8],
}

/// Exact finalized bytes needed to construct one LP lifecycle descriptor.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpFinalizedArtifactsV3<'a> {
    /// Open or Close route.
    pub action: MultiLpRequestActionV3,
    /// Exact lifecycle-bound AccountProfile bytes.
    pub account_profile: &'a [u8],
    /// Exact successor StateLifecyclePolicy V4 bytes.
    pub derivation_policy: &'a [u8],
    /// Exact physical capacity-profile bytes.
    pub capacity_profile: &'a [u8],
    /// Exact local EffectProgram bytes.
    pub effect_program: &'a [u8],
    /// Exact fixed-width RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact admitted ExecutionStrategyProgramV2 bytes.
    pub execution_strategy: &'a [u8],
    /// Exact strategy-selected TransitionVM bytes.
    pub transition: &'a [u8],
}

/// Return the exact schema identity selected by one global Dealer selector.
pub fn dealer_request_schema_v3(selector: u16) -> Result<ContentId, DealerReleaseErrorV3> {
    let preimage = match selector {
        1..=6 => DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3,
        7..=8 => DEALER_MULTI_LP_REQUEST_SCHEMA_PREIMAGE_V3,
        9 => DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3,
        _ => return Err(DealerReleaseErrorV3::Selector),
    };
    content_id(hash(preimage).to_bytes())
}

/// Finalize one fully joined equity CapabilityProgramV3 descriptor.
pub fn finalize_dealer_equity_descriptor_v3(
    artifacts: DealerEquityFinalizedArtifactsV3<'_>,
) -> Result<[u8; CAPABILITY_PROGRAM_V3_BYTES], DealerReleaseErrorV3> {
    let selector = equity_selector(artifacts.action, artifacts.signed_position_count)?;
    if artifacts.account_profile.is_empty()
        || artifacts.derivation_policy.is_empty()
        || artifacts.capacity_profile.is_empty()
        || artifacts.effect_program.is_empty()
        || artifacts.request_profile.is_empty()
        || artifacts.execution_strategy.is_empty()
        || artifacts.transition.is_empty()
    {
        return Err(DealerReleaseErrorV3::Artifact);
    }
    let mut scalars = [0_u64; 35];
    let mut identities = [[0_u8; 32]; 52];
    let scalar_count = dealer_equity_scalar_count_v3(artifacts.action)
        .map_err(|_| DealerReleaseErrorV3::Geometry)?;
    let identity_count = dealer_equity_identity_count_v3(artifacts.action)
        .map_err(|_| DealerReleaseErrorV3::Geometry)?;
    let bundle = authenticate_dealer_equity_artifacts_v3(
        artifacts.action,
        artifacts.signed_position_count,
        artifacts.request_profile,
        artifacts.transition,
        artifacts.effect_program,
        scalars
            .get_mut(..scalar_count)
            .ok_or(DealerReleaseErrorV3::Geometry)?,
        identities
            .get_mut(..identity_count)
            .ok_or(DealerReleaseErrorV3::Geometry)?,
    )?;
    let profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    if usize::from(profile.fixed_account_count())
        != usize::from(bundle.effect.fixed_account_count())
        || profile.item_account_stride() != 0
        || usize::from(profile.common_scalar_count()) != scalar_count
        || profile.item_scalar_stride() != 0
        || usize::from(profile.common_identity_count()) != identity_count
        || profile.item_identity_stride() != 0
    {
        return Err(DealerReleaseErrorV3::Geometry);
    }
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.execution_strategy)
        .map_err(|_| DealerReleaseErrorV3::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || strategy.transition_program().to_bytes() != hash(artifacts.transition).to_bytes()
    {
        return Err(DealerReleaseErrorV3::Strategy);
    }
    let request_profile_schema = if artifacts.signed_position_count == 0 {
        content_id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?
    } else {
        content_id(REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID)?
    };
    let descriptor = CapabilityProgramV3::new(
        content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes())?,
        content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V3).to_bytes())?,
        dealer_request_schema_v3(selector)?,
        content_id(hash(DEALER_ROOT_SCHEMA_PREIMAGE_V2).to_bytes())?,
        content_id(hash(artifacts.account_profile).to_bytes())?,
        content_id(hash(artifacts.derivation_policy).to_bytes())?,
        content_id(hash(artifacts.capacity_profile).to_bytes())?,
        content_id(hash(artifacts.effect_program).to_bytes())?,
        request_profile_schema,
        content_id(hash(artifacts.request_profile).to_bytes())?,
        content_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
        content_id(hash(artifacts.execution_strategy).to_bytes())?,
        u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES)
            .map_err(|_| DealerReleaseErrorV3::Geometry)?,
    )
    .map_err(|_| DealerReleaseErrorV3::Descriptor)?;
    strategy
        .validate_descriptor_selection(
            content_id(hash(artifacts.execution_strategy).to_bytes())?,
            descriptor,
        )
        .map_err(|_| DealerReleaseErrorV3::Strategy)?;
    Ok(descriptor.encode())
}

/// Finalize one selector-7/8 LP descriptor after byte-for-byte artifact rederivation.
#[cfg(not(target_os = "solana"))]
pub fn finalize_dealer_lp_descriptor_v3(
    artifacts: DealerLpFinalizedArtifactsV3<'_>,
) -> Result<[u8; CAPABILITY_PROGRAM_V3_BYTES], DealerReleaseErrorV3> {
    let selector = artifacts.action.selector();
    if artifacts.account_profile.is_empty()
        || artifacts.derivation_policy.is_empty()
        || artifacts.capacity_profile.is_empty()
        || artifacts.effect_program.is_empty()
        || artifacts.request_profile.is_empty()
        || artifacts.execution_strategy.is_empty()
        || artifacts.transition.is_empty()
    {
        return Err(DealerReleaseErrorV3::Artifact);
    }

    let profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    if profile.artifact_profile() != LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
        || profile.fixed_account_count() != dealer_lp_account_count_v3(artifacts.action)
        || profile.item_account_stride() != 0
        || profile.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || profile.item_scalar_stride() != 0
        || profile.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || profile.item_identity_stride() != 0
        || profile.trusted_current_slot_scalar() != Some(LP_CURRENT_SLOT_SCALAR_V3)
        || profile
            .rule(false, DEALER_LP_STATE_ACCOUNT_V3)
            .map_err(|_| DealerReleaseErrorV3::Geometry)?
            .prestate()
            != AccountPrestateV2::LifecycleBound
    {
        return Err(DealerReleaseErrorV3::Geometry);
    }

    let lifecycle_id = hash(artifacts.derivation_policy).to_bytes();
    let lifecycle = StateLifecyclePolicyV4::decode_selected(
        lifecycle_id,
        lifecycle_id,
        artifacts.derivation_policy,
    )
    .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    lifecycle
        .validate_account_profile(profile)
        .map_err(|_| DealerReleaseErrorV3::Geometry)?;
    if lifecycle.action_plan_count(u32::from(selector)) != Ok(1) {
        return Err(DealerReleaseErrorV3::Geometry);
    }

    let mut expected_lifecycle_scratch = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
    let mut expected_lifecycle = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
    encode_dealer_lp_lifecycle_v3(&mut expected_lifecycle_scratch, &mut expected_lifecycle)?;
    require_exact(&expected_lifecycle, artifacts.derivation_policy)?;

    let mut expected_request_scratch = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
    let mut expected_request = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
    encode_dealer_lp_request_profile_v3(
        artifacts.action,
        &mut expected_request_scratch,
        &mut expected_request,
    )?;
    require_exact(&expected_request, artifacts.request_profile)?;
    let request = RequestProfileV1::decode(artifacts.request_profile)
        .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    if request.fixed_request_bytes()
        != u32::try_from(DEALER_MULTI_LP_REQUEST_BYTES_V3)
            .map_err(|_| DealerReleaseErrorV3::Geometry)?
        || request.item_request_bytes() != 0
        || request.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || request.item_scalar_stride() != 0
        || request.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || request.item_identity_stride() != 0
    {
        return Err(DealerReleaseErrorV3::Geometry);
    }

    let transition_bytes = dealer_lp_transition_bytes_v3(artifacts.action);
    let mut expected_transition_scratch = vec![0; transition_bytes];
    let mut expected_transition = vec![0; transition_bytes];
    encode_dealer_lp_transition_v3(
        artifacts.action,
        &mut expected_transition_scratch,
        &mut expected_transition,
    )?;
    require_exact(&expected_transition, artifacts.transition)?;
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    if transition.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || transition.item_scalar_stride() != 0
        || transition.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || transition.item_identity_stride() != 0
    {
        return Err(DealerReleaseErrorV3::Geometry);
    }

    let effect_bytes = dealer_lp_effect_bytes_v3(artifacts.action);
    let mut expected_effect_scratch = vec![0; effect_bytes];
    let mut expected_effect = vec![0; effect_bytes];
    encode_dealer_lp_effect_v3(
        artifacts.action,
        &mut expected_effect_scratch,
        &mut expected_effect,
    )?;
    require_exact(&expected_effect, artifacts.effect_program)?;
    let effect = EffectProgramV3::decode(artifacts.effect_program)
        .map_err(|_| DealerReleaseErrorV3::Artifact)?;
    if effect.fixed_account_count() != dealer_lp_account_count_v3(artifacts.action)
        || effect.item_account_stride() != 0
        || effect.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || effect.item_scalar_stride() != 0
        || effect.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || effect.item_identity_stride() != 0
        || effect.route_count() != 0
        || effect.receipt_dependency_count() != 0
    {
        return Err(DealerReleaseErrorV3::Geometry);
    }

    let strategy = ExecutionStrategyProgramV2::decode(artifacts.execution_strategy)
        .map_err(|_| DealerReleaseErrorV3::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || strategy.transition_program().to_bytes() != hash(artifacts.transition).to_bytes()
    {
        return Err(DealerReleaseErrorV3::Strategy);
    }
    let descriptor = CapabilityProgramV3::new(
        content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes())?,
        content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V3).to_bytes())?,
        dealer_request_schema_v3(selector)?,
        content_id(hash(DEALER_ROOT_SCHEMA_PREIMAGE_V2).to_bytes())?,
        content_id(hash(artifacts.account_profile).to_bytes())?,
        content_id(hash(artifacts.derivation_policy).to_bytes())?,
        content_id(hash(artifacts.capacity_profile).to_bytes())?,
        content_id(hash(artifacts.effect_program).to_bytes())?,
        content_id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        content_id(hash(artifacts.request_profile).to_bytes())?,
        content_id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
        content_id(hash(artifacts.execution_strategy).to_bytes())?,
        u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES)
            .map_err(|_| DealerReleaseErrorV3::Geometry)?,
    )
    .map_err(|_| DealerReleaseErrorV3::Descriptor)?;
    strategy
        .validate_descriptor_selection(
            content_id(hash(artifacts.execution_strategy).to_bytes())?,
            descriptor,
        )
        .map_err(|_| DealerReleaseErrorV3::Strategy)?;
    Ok(descriptor.encode())
}

/// Encode the exact global nine-entry Dealer CapabilityProgramSet.
///
/// `descriptors[0]` is selector 1 and `descriptors[8]` is selector 9. Every
/// descriptor must carry the common Dealer kind/config/root and the request
/// schema fixed by its selector.
pub fn encode_dealer_global_program_set_v3(
    descriptors: &[[u8; CAPABILITY_PROGRAM_V3_BYTES]; DEALER_GLOBAL_SELECTOR_COUNT_V3],
) -> Result<[u8; DEALER_GLOBAL_PROGRAM_SET_BYTES_V3], DealerReleaseErrorV3> {
    let expected_kind = content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes())?;
    let expected_config = content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V3).to_bytes())?;
    let expected_root = content_id(hash(DEALER_ROOT_SCHEMA_PREIMAGE_V2).to_bytes())?;
    let mut output = [0_u8; DEALER_GLOBAL_PROGRAM_SET_BYTES_V3];
    put(&mut output, 0, &CAPABILITY_PROGRAM_SET_MAGIC_V1)?;
    put(
        &mut output,
        8,
        &CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V1.to_le_bytes(),
    )?;
    put(
        &mut output,
        10,
        &CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V1.to_le_bytes(),
    )?;
    put(
        &mut output,
        PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V1,
        &DEALER_EQUITY_SELECTOR_OFFSET_V3.to_le_bytes(),
    )?;
    *output
        .get_mut(PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V1)
        .ok_or(DealerReleaseErrorV3::ProgramSet)? = SelectorWidthV1::U16.bytes();
    *output
        .get_mut(PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V1)
        .ok_or(DealerReleaseErrorV3::ProgramSet)? = 0;
    put(
        &mut output,
        PROGRAM_SET_ENTRY_COUNT_OFFSET_V1,
        &u16::try_from(DEALER_GLOBAL_SELECTOR_COUNT_V3)
            .map_err(|_| DealerReleaseErrorV3::ProgramSet)?
            .to_le_bytes(),
    )?;
    for (index, bytes) in descriptors.iter().enumerate() {
        let selector = u16::try_from(index + 1).map_err(|_| DealerReleaseErrorV3::ProgramSet)?;
        let descriptor =
            CapabilityProgramV3::decode(bytes).map_err(|_| DealerReleaseErrorV3::Descriptor)?;
        if descriptor.kind() != expected_kind
            || descriptor.config_schema() != expected_config
            || descriptor.root_schema() != expected_root
            || descriptor.request_schema() != dealer_request_schema_v3(selector)?
        {
            return Err(DealerReleaseErrorV3::Descriptor);
        }
        let entry = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1)
                    .ok_or(DealerReleaseErrorV3::ProgramSet)?,
            )
            .ok_or(DealerReleaseErrorV3::ProgramSet)?;
        put(
            &mut output,
            entry + PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V1,
            &u32::from(selector).to_le_bytes(),
        )?;
        put(
            &mut output,
            entry + PROGRAM_SET_ENTRY_PROGRAM_OFFSET_V1,
            &hash(bytes).to_bytes(),
        )?;
    }
    let set =
        CapabilityProgramSetV1::decode(&output).map_err(|_| DealerReleaseErrorV3::ProgramSet)?;
    if set.selector_offset() != DEALER_EQUITY_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U16
        || usize::from(set.entry_count()) != DEALER_GLOBAL_SELECTOR_COUNT_V3
    {
        return Err(DealerReleaseErrorV3::ProgramSet);
    }
    Ok(output)
}

fn equity_selector(
    action: MultiLpActionV3,
    signed_position_count: u32,
) -> Result<u16, DealerReleaseErrorV3> {
    match (action, signed_position_count) {
        (MultiLpActionV3::Add, 0) => Ok(DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3),
        (MultiLpActionV3::Add, 1) => Ok(DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3),
        (MultiLpActionV3::Add, 2) => Ok(DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3),
        (MultiLpActionV3::Remove, 0) => Ok(DEALER_EQUITY_REDEEM_P0_SELECTOR_V3),
        (MultiLpActionV3::Remove, 1) => Ok(DEALER_EQUITY_REDEEM_P1_SELECTOR_V3),
        (MultiLpActionV3::Remove, 2) => Ok(DEALER_EQUITY_REDEEM_P2_SELECTOR_V3),
        _ => Err(DealerReleaseErrorV3::Selector),
    }
}

fn content_id(bytes: [u8; 32]) -> Result<ContentId, DealerReleaseErrorV3> {
    ContentId::new(bytes).map_err(|_| DealerReleaseErrorV3::Artifact)
}

#[cfg(not(target_os = "solana"))]
fn require_exact(expected: &[u8], actual: &[u8]) -> Result<(), DealerReleaseErrorV3> {
    if actual != expected {
        return Err(DealerReleaseErrorV3::Geometry);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), DealerReleaseErrorV3> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or(DealerReleaseErrorV3::ProgramSet)?;
    output
        .get_mut(offset..end)
        .ok_or(DealerReleaseErrorV3::ProgramSet)?
        .copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    };

    use super::super::v3_lp_artifacts::{
        DealerLpAccountProfileInputV3, encode_dealer_lp_account_profile_v3,
    };

    fn id(value: u8) -> ContentId {
        content_id([value; 32]).expect("identity")
    }

    fn descriptor(selector: u16) -> [u8; CAPABILITY_PROGRAM_V3_BYTES] {
        CapabilityProgramV3::new(
            content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes()).expect("kind"),
            content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V3).to_bytes()).expect("config"),
            dealer_request_schema_v3(selector).expect("request schema"),
            content_id(hash(DEALER_ROOT_SCHEMA_PREIMAGE_V2).to_bytes()).expect("root"),
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2[0]),
            id(8),
            u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES).expect("root bytes"),
        )
        .expect("descriptor")
        .encode()
    }

    fn admitted_strategy(transition: &[u8]) -> Vec<u8> {
        ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            content_id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID).expect("transition schema"),
            content_id(hash(transition).to_bytes()).expect("transition program"),
            content_id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2).expect("certificate schema"),
            Some(id(0x71)),
            content_id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("admission schema"),
            Some(id(0x72)),
            content_id(ACCELERATOR_REQUEST_SCHEMA_ID_V2).expect("request schema"),
            content_id(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("ack schema"),
        )
        .expect("admitted strategy")
        .to_bytes()
        .to_vec()
    }

    fn lp_profile(action: MultiLpRequestActionV3) -> Vec<u8> {
        let lengths = match action {
            MultiLpRequestActionV3::Open => vec![0, 0, 0, 0, 0, 208, 256, 0, 48, 0],
            MultiLpRequestActionV3::Close => vec![0, 0, 0, 0, 0, 208, 256, 48, 0],
        };
        encode_dealer_lp_account_profile_v3(DealerLpAccountProfileInputV3 {
            action,
            logical_data_lengths: &lengths,
        })
        .expect("LP account profile")
    }

    #[test]
    fn lp_descriptors_rederive_every_successor_artifact() {
        let mut lifecycle_scratch = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
        let mut lifecycle = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
        encode_dealer_lp_lifecycle_v3(&mut lifecycle_scratch, &mut lifecycle)
            .expect("V4 lifecycle");
        StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], &lifecycle)
            .expect("successor lifecycle");

        for action in [MultiLpRequestActionV3::Open, MultiLpRequestActionV3::Close] {
            let profile = lp_profile(action);
            let mut request_scratch = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            let mut request = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            encode_dealer_lp_request_profile_v3(action, &mut request_scratch, &mut request)
                .expect("request");
            let mut transition_scratch = vec![0; dealer_lp_transition_bytes_v3(action)];
            let mut transition = vec![0; dealer_lp_transition_bytes_v3(action)];
            encode_dealer_lp_transition_v3(action, &mut transition_scratch, &mut transition)
                .expect("transition");
            let mut effect_scratch = vec![0; dealer_lp_effect_bytes_v3(action)];
            let mut effect = vec![0; dealer_lp_effect_bytes_v3(action)];
            encode_dealer_lp_effect_v3(action, &mut effect_scratch, &mut effect).expect("effect");
            let strategy = admitted_strategy(&transition);
            let descriptor = finalize_dealer_lp_descriptor_v3(DealerLpFinalizedArtifactsV3 {
                action,
                account_profile: &profile,
                derivation_policy: &lifecycle,
                capacity_profile: &[1],
                effect_program: &effect,
                request_profile: &request,
                execution_strategy: &strategy,
                transition: &transition,
            })
            .expect("finalized LP descriptor");
            let decoded = CapabilityProgramV3::decode(&descriptor).expect("descriptor");
            assert_eq!(
                decoded.request_schema(),
                dealer_request_schema_v3(action.selector()).expect("request schema")
            );

            let last = effect.len().checked_sub(1).expect("effect byte");
            *effect.get_mut(last).expect("effect byte") ^= 1;
            assert!(
                finalize_dealer_lp_descriptor_v3(DealerLpFinalizedArtifactsV3 {
                    action,
                    account_profile: &profile,
                    derivation_policy: &lifecycle,
                    capacity_profile: &[1],
                    effect_program: &effect,
                    request_profile: &request,
                    execution_strategy: &strategy,
                    transition: &transition,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn one_set_selects_every_global_dealer_action() {
        let descriptors =
            core::array::from_fn(|index| descriptor(u16::try_from(index + 1).expect("selector")));
        let bytes = encode_dealer_global_program_set_v3(&descriptors).expect("global set");
        let set = CapabilityProgramSetV1::decode(&bytes).expect("decode");
        assert_eq!(set.selector_offset(), 10);
        assert_eq!(set.selector_width(), SelectorWidthV1::U16);
        assert_eq!(set.entry_count(), 9);
        for selector in 1_u16..=9 {
            let mut request = [0_u8; 12];
            request[10..12].copy_from_slice(&selector.to_le_bytes());
            assert_eq!(
                set.select(&request).expect("selected").to_bytes(),
                hash(
                    descriptors
                        .get(usize::from(selector - 1))
                        .expect("descriptor")
                )
                .to_bytes()
            );
        }
    }

    #[test]
    fn selector_schema_substitution_and_legacy_aliases_refuse() {
        let mut descriptors =
            core::array::from_fn(|index| descriptor(u16::try_from(index + 1).expect("selector")));
        descriptors[6] = descriptor(1);
        assert_eq!(
            encode_dealer_global_program_set_v3(&descriptors),
            Err(DealerReleaseErrorV3::Descriptor)
        );
        assert_eq!(
            dealer_request_schema_v3(0),
            Err(DealerReleaseErrorV3::Selector)
        );
        assert_eq!(
            dealer_request_schema_v3(10),
            Err(DealerReleaseErrorV3::Selector)
        );
    }

    #[test]
    fn frozen_selector_assignments_do_not_collide() {
        assert_eq!(
            super::super::v3_operator::MultiLpRequestActionV3::Open.selector(),
            7
        );
        assert_eq!(
            super::super::v3_operator::MultiLpRequestActionV3::Close.selector(),
            8
        );
        assert_eq!(super::super::v3_trade::DEALER_SCENARIO_TRADE_ACTION_V3, 9);
        assert_eq!(DEALER_GLOBAL_SELECTOR_MIN_V3, 1);
        assert_eq!(DEALER_GLOBAL_SELECTOR_MAX_V3, 9);
        assert_eq!(
            super::super::v3_artifacts::DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3,
            240
        );
    }
}
