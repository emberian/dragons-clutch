//! Schema-bound release authority for Dealer selector 9.
//!
//! Selector 9 is finalized only as `CapabilityProgramV4`, binding every
//! interpreter artifact by both schema and exact content.  The global Dealer
//! selector remains one `CapabilityProgramSetV2`; during the finite migration
//! of selectors 1..=8, each entry explicitly names either the V3 or V4
//! descriptor schema.  There is never a second selector table or legacy alias.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_RELEASE_ID_V5,
        HEADER_BYTES as LIFECYCLE_HEADER_BYTES_V5, StateLifecyclePolicyV5,
        encode::encode_lifecycle_policy_v5_atomic,
    },
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2},
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1,
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v3::{CapabilityProgramV3, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3},
    v4::{
        ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4,
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::config_v4::DEALER_CONFIG_SCHEMA_PREIMAGE_V4;
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::v3::REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID;
use solana_program::hash::hash;

use super::{
    DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
    v3_equity_operator::DEALER_EQUITY_SELECTOR_OFFSET_V3,
    v3_multi_lp::MultiLpCustodyRequestV3,
    v3_release::{
        DEALER_GLOBAL_SELECTOR_COUNT_V3, DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3,
        dealer_request_schema_v3,
    },
    v3_trade::DEALER_SCENARIO_TRADE_ACTION_V3,
    v3_trade_artifacts::{
        DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
        DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4, DEALER_SCENARIO_TRANSITION_BYTES_V4,
        authenticate_dealer_scenario_artifacts_v4, dealer_scenario_base_effect_program_bytes_v4,
        dealer_scenario_effect_program_bytes_v4, encode_dealer_scenario_base_effect_program_v4,
        encode_dealer_scenario_effect_program_v4, encode_dealer_scenario_request_profile_v4,
        encode_dealer_scenario_transition_v4,
    },
    v3_trade_profile::{
        DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DealerScenarioAccountProfileInputV4,
        encode_dealer_scenario_account_profile_v4_atomic,
    },
};

/// Exact width of the sole nine-entry Dealer ProgramSet V2.
pub const DEALER_GLOBAL_PROGRAM_SET_BYTES_V4: usize = 680;
/// Exact canonical empty Lifecycle V5 artifact width for selector 9.
pub const DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5: usize = LIFECYCLE_HEADER_BYTES_V5;

const _: () = assert!(DEALER_GLOBAL_SELECTOR_COUNT_V3 == 9);

/// Stable release-construction or migration refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioReleaseErrorV4 {
    /// Selector or schema did not belong to the one canonical Dealer table.
    Selector,
    /// An artifact was absent, substituted, or hostile-decode invalid.
    Artifact,
    /// The exact generated Account/Request/Transition/Effect geometry differed.
    Geometry,
    /// Strategy disposition or Transition selection differed.
    Strategy,
    /// Descriptor semantic identities or artifact schemas differed.
    Descriptor,
    /// ProgramSet width, ordering, schema, content, or selection differed.
    ProgramSet,
}

/// Encode selector 9's canonical empty Lifecycle V5 artifact.
///
/// Scenario execution authenticates only already-live accounts and therefore
/// owns no creation, closure, protected output, immutable binding, or current
/// Rent quote. The generic V5 empty form expresses that fact without an
/// unreachable dummy plan.
pub fn encode_dealer_scenario_empty_lifecycle_v5(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioReleaseErrorV4> {
    if scratch.len() != DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5
        || output.len() != DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5
    {
        return Err(DealerScenarioReleaseErrorV4::Geometry);
    }
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], scratch, output)
        .map_err(|_| DealerScenarioReleaseErrorV4::Artifact)?;
    let id = digest(output);
    let policy = StateLifecyclePolicyV5::decode_selected(id, id, output)
        .map_err(|_| DealerScenarioReleaseErrorV4::Artifact)?;
    if !policy.is_empty() {
        return Err(DealerScenarioReleaseErrorV4::Geometry);
    }
    Ok(())
}

/// Exact finalized inputs for the selector-9 V4 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioFinalizedArtifactsV4<'a> {
    /// Inputs selecting the five finalized common account data widths.
    pub account_profile_input: DealerScenarioAccountProfileInputV4,
    /// Exact Profile13 artifact.
    pub account_profile: &'a [u8],
    /// Exact successor lifecycle artifact. Selector 9 must select no plans.
    pub lifecycle_policy: &'a [u8],
    /// Exact immutable physical capacity profile.
    pub capacity_profile: &'a [u8],
    /// Exact borrowed-witness RequestProfile V3 artifact.
    pub request_profile: &'a [u8],
    /// Exact admitted-AOT ExecutionStrategy V2 artifact.
    pub execution_strategy: &'a [u8],
    /// Exact underlying TransitionVM V3 artifact.
    pub transition: &'a [u8],
    /// Exact Effect V4 artifact around the typed base program.
    pub effect: &'a [u8],
    /// Six exact typed Custody request templates in route order.
    pub custody_templates: &'a [MultiLpCustodyRequestV3; 6],
}

/// One finalized descriptor record in the sole global SetV2 authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerDescriptorRecordV4<'a> {
    selector: u16,
    schema: [u8; 32],
    bytes: &'a [u8],
}

impl<'a> DealerDescriptorRecordV4<'a> {
    /// Admit one exact schema-bound Dealer descriptor for `selector`.
    pub fn new(
        selector: u16,
        schema: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self, DealerScenarioReleaseErrorV4> {
        validate_descriptor_record(selector, schema, bytes)?;
        Ok(Self {
            selector,
            schema,
            bytes,
        })
    }

    /// Canonical selector carried by this typed record.
    pub const fn selector(self) -> u16 {
        self.selector
    }

    /// Exact finalized descriptor schema.
    pub const fn schema(self) -> [u8; 32] {
        self.schema
    }

    /// Exact finalized descriptor bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Finalize the exact selector-9 `CapabilityProgramV4`.
pub fn finalize_dealer_scenario_descriptor_v4(
    artifacts: DealerScenarioFinalizedArtifactsV4<'_>,
) -> Result<[u8; CAPABILITY_PROGRAM_V4_BYTES], DealerScenarioReleaseErrorV4> {
    if artifacts.lifecycle_policy.is_empty()
        || artifacts.capacity_profile.is_empty()
        || artifacts.execution_strategy.is_empty()
    {
        return Err(DealerScenarioReleaseErrorV4::Artifact);
    }
    validate_generated_artifacts(artifacts)?;

    let profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DealerScenarioReleaseErrorV4::Artifact)?;
    let lifecycle_id = digest(artifacts.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        artifacts.lifecycle_policy,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::Artifact)?;
    lifecycle
        .validate_account_profile(profile)
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    if !lifecycle.is_empty()
        || lifecycle
            .action_plan_count(u32::from(DEALER_SCENARIO_TRADE_ACTION_V3))
            .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?
            != 0
    {
        return Err(DealerScenarioReleaseErrorV4::Geometry);
    }

    let strategy = ExecutionStrategyProgramV2::decode(artifacts.execution_strategy)
        .map_err(|_| DealerScenarioReleaseErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || strategy.transition_program().to_bytes() != digest(artifacts.transition)
    {
        return Err(DealerScenarioReleaseErrorV4::Strategy);
    }

    let descriptor = CapabilityProgramV4::new(
        content(digest(DEALER_KIND_PREIMAGE_V2))?,
        content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))?,
        content(digest(DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3))?,
        content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2))?,
        content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)?,
        content(digest(artifacts.capacity_profile))?,
        CapabilityArtifactsV4 {
            account_profile: reference(
                ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2,
                artifacts.account_profile,
            )?,
            request_profile: reference(
                REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID,
                artifacts.request_profile,
            )?,
            lifecycle: reference(LIFECYCLE_SCHEMA_RELEASE_ID_V5, artifacts.lifecycle_policy)?,
            strategy: reference(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                artifacts.execution_strategy,
            )?,
            transition: reference(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                artifacts.transition,
            )?,
            effect: reference(
                dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
                artifacts.effect,
            )?,
        },
        u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES)
            .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::Descriptor)?;
    strategy
        .validate_descriptor_selection_v4(
            content(digest(artifacts.execution_strategy))?,
            descriptor,
        )
        .map_err(|_| DealerScenarioReleaseErrorV4::Strategy)?;
    validate_v4_semantics(DEALER_SCENARIO_TRADE_ACTION_V3, descriptor)?;
    Ok(descriptor.encode())
}

/// Encode the sole nine-entry global Dealer ProgramSet V2.
///
/// Each input is already typed by its selector.  Selector 9 must be V4; the
/// explicit per-entry schema permits selectors 1..=8 to migrate without a
/// parallel ProgramSet or magic-based decoder choice.
pub fn encode_dealer_global_program_set_v4(
    records: &[DealerDescriptorRecordV4<'_>; DEALER_GLOBAL_SELECTOR_COUNT_V3],
    output: &mut [u8],
) -> Result<(), DealerScenarioReleaseErrorV4> {
    if output.len() != DEALER_GLOBAL_PROGRAM_SET_BYTES_V4
        || encoded_program_set_bytes_v2(records.len())
            .map_err(|_| DealerScenarioReleaseErrorV4::ProgramSet)?
            != DEALER_GLOBAL_PROGRAM_SET_BYTES_V4
    {
        return Err(DealerScenarioReleaseErrorV4::ProgramSet);
    }
    let mut entries = Vec::with_capacity(records.len());
    for (index, record) in records.iter().copied().enumerate() {
        let selector =
            u16::try_from(index + 1).map_err(|_| DealerScenarioReleaseErrorV4::ProgramSet)?;
        if record.selector != selector {
            return Err(DealerScenarioReleaseErrorV4::ProgramSet);
        }
        validate_descriptor_record(record.selector, record.schema, record.bytes)?;
        entries.push(CapabilityProgramSetEntryV2::new(
            u32::from(record.selector),
            CapabilityDescriptorReferenceV2::new(
                content(record.schema)?,
                content(digest(record.bytes))?,
            ),
        ));
    }
    encode_program_set_v2(
        DEALER_EQUITY_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U16,
        &entries,
        output,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::ProgramSet)?;
    let set = CapabilityProgramSetV2::decode(output)
        .map_err(|_| DealerScenarioReleaseErrorV4::ProgramSet)?;
    if set.selector_offset() != DEALER_EQUITY_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U16
        || usize::from(set.entry_count()) != DEALER_GLOBAL_SELECTOR_COUNT_V3
    {
        return Err(DealerScenarioReleaseErrorV4::ProgramSet);
    }
    for record in records.iter().copied() {
        let mut request = [0_u8; 12];
        request
            .get_mut(10..12)
            .ok_or(DealerScenarioReleaseErrorV4::ProgramSet)?
            .copy_from_slice(&record.selector.to_le_bytes());
        set.require_descriptor(
            &request,
            content(record.schema)?,
            content(digest(record.bytes))?,
        )
        .map_err(|_| DealerScenarioReleaseErrorV4::ProgramSet)?;
    }
    Ok(())
}

fn validate_generated_artifacts(
    artifacts: DealerScenarioFinalizedArtifactsV4<'_>,
) -> Result<(), DealerScenarioReleaseErrorV4> {
    if artifacts.account_profile.len() != DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4
        || artifacts.request_profile.len() != DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4
        || artifacts.transition.len() != DEALER_SCENARIO_TRANSITION_BYTES_V4
    {
        return Err(DealerScenarioReleaseErrorV4::Geometry);
    }
    let mut profile_scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    let mut profile = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    encode_dealer_scenario_account_profile_v4_atomic(
        artifacts.account_profile_input,
        &mut profile_scratch,
        &mut profile,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    require_exact(&profile, artifacts.account_profile)?;

    let mut request_scratch = [0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
    let mut request = [0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
    encode_dealer_scenario_request_profile_v4(&mut request_scratch, &mut request)
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    require_exact(&request, artifacts.request_profile)?;

    let mut transition_scratch = [0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
    let mut transition = [0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
    encode_dealer_scenario_transition_v4(&mut transition_scratch, &mut transition)
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    require_exact(&transition, artifacts.transition)?;

    let base_bytes = dealer_scenario_base_effect_program_bytes_v4()
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    let mut base_scratch = vec![0; base_bytes];
    let mut base = vec![0; base_bytes];
    encode_dealer_scenario_base_effect_program_v4(
        artifacts.custody_templates,
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    let effect_bytes = dealer_scenario_effect_program_bytes_v4(base_bytes)
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    if artifacts.effect.len() != effect_bytes {
        return Err(DealerScenarioReleaseErrorV4::Geometry);
    }
    let mut effect_scratch = vec![0; effect_bytes];
    let mut effect = vec![0; effect_bytes];
    encode_dealer_scenario_effect_program_v4(&base, &mut effect_scratch, &mut effect)
        .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    require_exact(&effect, artifacts.effect)?;

    let mut scalars = vec![0_u64; usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)];
    let mut identities = vec![[0_u8; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
    authenticate_dealer_scenario_artifacts_v4(
        artifacts.request_profile,
        artifacts.transition,
        artifacts.effect,
        &mut scalars,
        &mut identities,
    )
    .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?;
    Ok(())
}

fn validate_descriptor_record(
    selector: u16,
    schema: [u8; 32],
    bytes: &[u8],
) -> Result<(), DealerScenarioReleaseErrorV4> {
    if !(1..=9).contains(&selector) {
        return Err(DealerScenarioReleaseErrorV4::Selector);
    }
    if schema == CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3 && selector != 9 {
        let descriptor = CapabilityProgramV3::decode(bytes)
            .map_err(|_| DealerScenarioReleaseErrorV4::Descriptor)?;
        if descriptor.kind() != content(digest(DEALER_KIND_PREIMAGE_V2))?
            || descriptor.config_schema() != content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))?
            || descriptor.request_schema()
                != dealer_request_schema_v3(selector)
                    .map_err(|_| DealerScenarioReleaseErrorV4::Descriptor)?
            || descriptor.root_schema() != content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2))?
            || usize::try_from(descriptor.root_state_bytes())
                .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?
                != dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES
        {
            return Err(DealerScenarioReleaseErrorV4::Descriptor);
        }
        return Ok(());
    }
    if schema != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4 {
        return Err(DealerScenarioReleaseErrorV4::Descriptor);
    }
    let descriptor =
        CapabilityProgramV4::decode(bytes).map_err(|_| DealerScenarioReleaseErrorV4::Descriptor)?;
    validate_v4_semantics(selector, descriptor)
}

fn validate_v4_semantics(
    selector: u16,
    descriptor: CapabilityProgramV4,
) -> Result<(), DealerScenarioReleaseErrorV4> {
    if descriptor.kind() != content(digest(DEALER_KIND_PREIMAGE_V2))?
        || descriptor.config_schema() != content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))?
        || descriptor.request_schema()
            != dealer_request_schema_v3(selector)
                .map_err(|_| DealerScenarioReleaseErrorV4::Descriptor)?
        || descriptor.root_schema() != content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2))?
        || descriptor.derivation_policy() != content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)?
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| DealerScenarioReleaseErrorV4::Geometry)?
            != dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES
        || descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2
        || descriptor.lifecycle().schema().to_bytes() != LIFECYCLE_SCHEMA_RELEASE_ID_V5
        || descriptor.strategy().schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition().schema().to_bytes()
            != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
    {
        return Err(DealerScenarioReleaseErrorV4::Descriptor);
    }
    if selector == DEALER_SCENARIO_TRADE_ACTION_V3
        && (descriptor.request_profile().schema().to_bytes()
            != REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
            || descriptor.effect().schema().to_bytes()
                != dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4)
    {
        return Err(DealerScenarioReleaseErrorV4::Descriptor);
    }
    Ok(())
}

fn reference(
    schema: [u8; 32],
    bytes: &[u8],
) -> Result<ArtifactReferenceV4, DealerScenarioReleaseErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(digest(bytes))?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DealerScenarioReleaseErrorV4> {
    ContentId::new(bytes).map_err(|_| DealerScenarioReleaseErrorV4::Artifact)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

fn require_exact(expected: &[u8], actual: &[u8]) -> Result<(), DealerScenarioReleaseErrorV4> {
    if expected == actual {
        Ok(())
    } else {
        Err(DealerScenarioReleaseErrorV4::Geometry)
    }
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES;

    use super::*;

    fn byte_id(value: u8) -> ContentId {
        content([value; 32]).expect("content")
    }

    fn v3_descriptor(selector: u16, tag: u8) -> [u8; CAPABILITY_PROGRAM_V3_BYTES] {
        CapabilityProgramV3::new(
            content(digest(DEALER_KIND_PREIMAGE_V2)).expect("kind"),
            content(digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4)).expect("config"),
            dealer_request_schema_v3(selector).expect("request"),
            content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2)).expect("root"),
            byte_id(tag),
            byte_id(tag + 1),
            byte_id(tag + 2),
            byte_id(tag + 3),
            byte_id(tag + 4),
            byte_id(tag + 5),
            byte_id(tag + 6),
            byte_id(tag + 7),
            u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES).expect("width"),
        )
        .expect("V3")
        .encode()
    }

    fn scenario_descriptor(effect_schema: [u8; 32]) -> [u8; CAPABILITY_PROGRAM_V4_BYTES] {
        scenario_descriptor_with_config_schema(
            effect_schema,
            digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4),
        )
    }

    fn scenario_descriptor_with_config_schema(
        effect_schema: [u8; 32],
        config_schema: [u8; 32],
    ) -> [u8; CAPABILITY_PROGRAM_V4_BYTES] {
        CapabilityProgramV4::new(
            content(digest(DEALER_KIND_PREIMAGE_V2)).expect("kind"),
            content(config_schema).expect("config"),
            content(digest(DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3)).expect("request"),
            content(digest(DEALER_ROOT_SCHEMA_PREIMAGE_V2)).expect("root"),
            content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1).expect("derivation"),
            byte_id(30),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(
                    content(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2).expect("schema"),
                    byte_id(31),
                ),
                request_profile: ArtifactReferenceV4::new(
                    content(REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID).expect("schema"),
                    byte_id(32),
                ),
                lifecycle: ArtifactReferenceV4::new(
                    content(LIFECYCLE_SCHEMA_RELEASE_ID_V5).expect("schema"),
                    byte_id(33),
                ),
                strategy: ArtifactReferenceV4::new(
                    content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2).expect("schema"),
                    byte_id(34),
                ),
                transition: ArtifactReferenceV4::new(
                    content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID).expect("schema"),
                    byte_id(35),
                ),
                effect: ArtifactReferenceV4::new(
                    content(effect_schema).expect("schema"),
                    byte_id(36),
                ),
            },
            u32::try_from(dclutch_dealer_codec::root_tail::ROOT_TAIL_BYTES).expect("width"),
        )
        .expect("V4")
        .encode()
    }

    #[test]
    fn selector_nine_requires_v4_and_every_successor_schema() {
        let exact = scenario_descriptor(dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4);
        assert!(
            DealerDescriptorRecordV4::new(9, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4, &exact,)
                .is_ok()
        );
        assert_eq!(
            DealerDescriptorRecordV4::new(
                9,
                CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3,
                &v3_descriptor(9, 40),
            ),
            Err(DealerScenarioReleaseErrorV4::Descriptor)
        );
        let stale = scenario_descriptor(dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID);
        assert_eq!(
            DealerDescriptorRecordV4::new(9, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4, &stale,),
            Err(DealerScenarioReleaseErrorV4::Descriptor)
        );
        let legacy_config = scenario_descriptor_with_config_schema(
            dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
            digest(super::super::DEALER_CONFIG_SCHEMA_PREIMAGE_V2),
        );
        assert_eq!(
            DealerDescriptorRecordV4::new(
                9,
                CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
                &legacy_config,
            ),
            Err(DealerScenarioReleaseErrorV4::Descriptor)
        );
    }

    #[test]
    fn selector_nine_lifecycle_is_the_canonical_empty_v5_policy() {
        let mut scratch = [0xa5_u8; DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5];
        let mut output = [0x5a_u8; DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5];
        encode_dealer_scenario_empty_lifecycle_v5(&mut scratch, &mut output).expect("empty V5");
        let id = digest(&output);
        let policy = StateLifecyclePolicyV5::decode_selected(id, id, &output).expect("decode");
        assert!(policy.is_empty());
        assert_eq!(policy.action_plan_count(9), Ok(0));
        assert_eq!(policy.current_rent_quote_count(), 0);

        let mut substituted = output;
        *substituted.get_mut(12).expect("recipe count") = 1;
        assert!(
            StateLifecyclePolicyV5::decode_selected(
                digest(&substituted),
                digest(&substituted),
                &substituted,
            )
            .is_err()
        );
    }

    #[test]
    fn global_set_has_one_schema_bound_entry_per_selector() {
        let legacy = core::array::from_fn::<_, 8, _>(|index| {
            v3_descriptor(
                u16::try_from(index + 1).expect("selector"),
                40 + index as u8,
            )
        });
        let scenario = scenario_descriptor(dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4);
        let records = [
            DealerDescriptorRecordV4::new(1, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[0])
                .expect("1"),
            DealerDescriptorRecordV4::new(2, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[1])
                .expect("2"),
            DealerDescriptorRecordV4::new(3, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[2])
                .expect("3"),
            DealerDescriptorRecordV4::new(4, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[3])
                .expect("4"),
            DealerDescriptorRecordV4::new(5, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[4])
                .expect("5"),
            DealerDescriptorRecordV4::new(6, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[5])
                .expect("6"),
            DealerDescriptorRecordV4::new(7, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[6])
                .expect("7"),
            DealerDescriptorRecordV4::new(8, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &legacy[7])
                .expect("8"),
            DealerDescriptorRecordV4::new(9, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4, &scenario)
                .expect("9"),
        ];
        let mut set_bytes = [0_u8; DEALER_GLOBAL_PROGRAM_SET_BYTES_V4];
        encode_dealer_global_program_set_v4(&records, &mut set_bytes).expect("set");
        let set = CapabilityProgramSetV2::decode(&set_bytes).expect("decode");
        assert_eq!(set.entry_count(), 9);
        assert_eq!(
            set.entry(8)
                .expect("selector 9")
                .descriptor()
                .schema()
                .to_bytes(),
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4
        );
    }

    #[test]
    fn global_set_refuses_cross_family_or_schema_substituted_records() {
        let one = v3_descriptor(1, 60);
        let seven = v3_descriptor(7, 70);
        assert_eq!(
            DealerDescriptorRecordV4::new(1, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3, &seven),
            Err(DealerScenarioReleaseErrorV4::Descriptor)
        );
        assert_eq!(
            DealerDescriptorRecordV4::new(2, CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4, &one),
            Err(DealerScenarioReleaseErrorV4::Descriptor)
        );
    }
}
