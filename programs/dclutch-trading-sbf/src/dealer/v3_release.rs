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
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID;
use solana_program::hash::hash;

use super::{
    DEALER_CONFIG_SCHEMA_PREIMAGE_V2, DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
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

/// Canonical junior-equity request schema label.
pub const DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-junior-equity-request-v3";
/// Canonical LP lifecycle request schema label.
pub const DEALER_MULTI_LP_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-multi-lp-request-v3";
/// Canonical scenario exact-fill request schema label.
pub const DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/dealer-scenario-trade-request-v3";

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
        content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V2).to_bytes())?,
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

/// Encode the exact global nine-entry Dealer CapabilityProgramSet.
///
/// `descriptors[0]` is selector 1 and `descriptors[8]` is selector 9. Every
/// descriptor must carry the common Dealer kind/config/root and the request
/// schema fixed by its selector.
pub fn encode_dealer_global_program_set_v3(
    descriptors: &[[u8; CAPABILITY_PROGRAM_V3_BYTES]; DEALER_GLOBAL_SELECTOR_COUNT_V3],
) -> Result<[u8; DEALER_GLOBAL_PROGRAM_SET_BYTES_V3], DealerReleaseErrorV3> {
    let expected_kind = content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes())?;
    let expected_config = content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V2).to_bytes())?;
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

    fn id(value: u8) -> ContentId {
        content_id([value; 32]).expect("identity")
    }

    fn descriptor(selector: u16) -> [u8; CAPABILITY_PROGRAM_V3_BYTES] {
        CapabilityProgramV3::new(
            content_id(hash(DEALER_KIND_PREIMAGE_V2).to_bytes()).expect("kind"),
            content_id(hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V2).to_bytes()).expect("config"),
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
            super::super::v3_operator::MultiLpRequestActionV3::Open as u16,
            7
        );
        assert_eq!(
            super::super::v3_operator::MultiLpRequestActionV3::Close as u16,
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
