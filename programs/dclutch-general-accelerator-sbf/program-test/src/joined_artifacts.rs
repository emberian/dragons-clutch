//! Reproducible seven-action General artifact graph for joined real-SBF tests.
//!
//! The fixture uses only public semantic-owner encoders. It deliberately does
//! not cache generated identities: every content edge is re-derived from the
//! exact bytes which ProgramTest installs as finalized Registry records.

use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3,
    },
    artifacts_v3::{
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3, GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3,
        GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
    },
    effect_artifacts_v3::{
        GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v3_atomic,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_template_bytes_v3,
    },
    release_v3::GENERAL_ACTIONS_V3,
    state_artifacts_v3::{
        GeneralChildRentWidthsV5, encode_general_state_lifecycle_v5_atomic,
        general_state_lifecycle_bytes_v5,
    },
    transition_artifacts_v3::{
        GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3, encode_general_transition_program_v3_atomic,
        general_transition_instruction_count_v3, general_transition_program_bytes_v3,
    },
};
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::v3::{
    GENERAL_CONFIG_SCHEMA_ID_V3, GeneralConfigV3, GeneralConfigV3Input,
};
use dclutch_general_config_contract::{GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_SCHEMA_ID_V2};
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use solana_program::hash::hash;

/// Immutable inputs which are common to all seven generated action bundles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JoinedGeneralArtifactInputV5 {
    /// Selected release/capacity identity.
    pub capacity_profile: [u8; 32],
    /// Registry-authenticated real General accelerator ArtifactRelease ID.
    pub accelerator_artifact_release: [u8; 32],
    /// Product-authenticated runtime outcome width.
    pub outcome_count: u32,
    /// Release-selected external account widths used by Profile13.
    pub external_widths: GeneralExternalAccountWidthsV3,
    /// Exact selected collateral token-account byte width.
    pub token_account_bytes: u32,
    /// Immutable General price scale.
    pub price_scale: u64,
    /// Immutable selection policy identity.
    pub selection_policy: [u8; 32],
    /// Immutable quote-surplus beneficiary.
    pub quote_surplus_beneficiary: [u8; 32],
}

/// Owned bytes for one action-selected descriptor graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JoinedGeneralActionArtifactsV5 {
    /// Selected action.
    pub action: Action,
    /// CapabilityProgramV4 bytes.
    pub descriptor: Vec<u8>,
    /// Profile13 bytes.
    pub account_profile: Vec<u8>,
    /// LifecycleV5 bytes.
    pub lifecycle_policy: Vec<u8>,
    /// RequestProfile bytes.
    pub request_profile: Vec<u8>,
    /// Admitted-AOT strategy bytes.
    pub strategy: Vec<u8>,
    /// Translation certificate bytes.
    pub certificate: Vec<u8>,
    /// Registry admission bytes.
    pub admission: Vec<u8>,
    /// TransitionProgramV3 bytes.
    pub transition: Vec<u8>,
    /// DCE5 EffectProgram bytes.
    pub effect: Vec<u8>,
}

/// Complete seven-action ProgramSet/config/artifact graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JoinedGeneralArtifactsV5 {
    /// Exact seven-entry CapabilityProgramSetV2.
    pub program_set: Vec<u8>,
    /// Exact immutable GeneralConfigV3.
    pub config: Vec<u8>,
    /// Action bundles in canonical action-tag order.
    pub actions: Vec<JoinedGeneralActionArtifactsV5>,
    /// Registry-authenticated accelerator ArtifactRelease identity.
    pub accelerator_artifact_release: [u8; 32],
}

/// Stable refusal from exact artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoinedGeneralArtifactErrorV5 {
    /// An identity was zero or an integer geometry overflowed.
    Input,
    /// A semantic-owner encoder refused the requested artifact.
    Encoding,
    /// The complete cross-artifact admission join refused.
    Admission,
}

impl JoinedGeneralArtifactsV5 {
    /// Return one action bundle by exact tag.
    pub fn action(
        &self,
        action: Action,
    ) -> Result<&JoinedGeneralActionArtifactsV5, JoinedGeneralArtifactErrorV5> {
        self.actions
            .get(usize::from(action as u8))
            .filter(|value| value.action == action)
            .ok_or(JoinedGeneralArtifactErrorV5::Input)
    }

    /// Borrow one complete action selection for canonical onchain admission.
    pub fn selected<'a>(
        &'a self,
        action: Action,
    ) -> Result<
        (GeneralArtifactSelectionV3, GeneralArtifactBytesV3<'a>),
        JoinedGeneralArtifactErrorV5,
    > {
        let selected = self.action(action)?;
        Ok((
            GeneralArtifactSelectionV3 {
                program_set: digest(&self.program_set),
                config: digest(&self.config),
                artifact_release: self.accelerator_artifact_release,
            },
            GeneralArtifactBytesV3 {
                program_set: &self.program_set,
                descriptor: &selected.descriptor,
                config: &self.config,
                account_profile: &selected.account_profile,
                lifecycle_policy: &selected.lifecycle_policy,
                request_profile: &selected.request_profile,
                strategy: &selected.strategy,
                certificate: &selected.certificate,
                admission: &selected.admission,
                transition: &selected.transition,
                effect: &selected.effect,
            },
        ))
    }
}

/// Generate and reauthenticate the exact seven-action successor graph.
pub fn build_joined_general_artifacts_v5(
    input: JoinedGeneralArtifactInputV5,
) -> Result<JoinedGeneralArtifactsV5, JoinedGeneralArtifactErrorV5> {
    validate_input(input)?;
    let mut actions = Vec::with_capacity(GENERAL_ACTIONS_V3.len());
    for action in GENERAL_ACTIONS_V3 {
        actions.push(action_artifacts(input, action)?);
    }
    let entries = actions
        .iter()
        .map(|action| {
            Ok(CapabilityProgramSetEntryV2::new(
                u32::from(action.action as u8),
                CapabilityDescriptorReferenceV2::new(
                    content(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)?,
                    content(digest(&action.descriptor))?,
                ),
            ))
        })
        .collect::<Result<Vec<_>, JoinedGeneralArtifactErrorV5>>()?;
    let mut program_set = vec![
        0_u8;
        encoded_program_set_bytes_v2(entries.len())
            .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?
    ];
    encode_program_set_v2(
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    let config = GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: input.capacity_profile,
        claim_basis_id: [0x61; 32],
        program_set_id: digest(&program_set),
        generation: 1,
        price_scale: input.price_scale,
        collection_slots: 16,
        selection_slots: 16,
        settlement_slots: 64,
        max_orders_per_candidate: 32,
        max_pages_per_candidate: 32,
        continuation_reward_lamports: 1,
        selection_policy_id: input.selection_policy,
        quote_surplus_beneficiary: input.quote_surplus_beneficiary,
    })
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?
    .to_bytes()
    .to_vec();
    let output = JoinedGeneralArtifactsV5 {
        program_set,
        config,
        actions,
        accelerator_artifact_release: input.accelerator_artifact_release,
    };
    for action in GENERAL_ACTIONS_V3 {
        let (selection, bytes) = output.selected(action)?;
        let request = canonical_request(action)?;
        authenticate_general_artifacts_v3(selection, bytes, &request, input.outcome_count)
            .map_err(|_| JoinedGeneralArtifactErrorV5::Admission)?;
    }
    Ok(output)
}

fn action_artifacts(
    input: JoinedGeneralArtifactInputV5,
    action: Action,
) -> Result<JoinedGeneralActionArtifactsV5, JoinedGeneralArtifactErrorV5> {
    let account_profile = account_profile(input.external_widths, action)?;
    let lifecycle_policy = lifecycle(input, action)?;
    let request_profile =
        dclutch_general_adapter_contract::specialization::general_request_profile_bytes_v1(action)
            .to_vec();
    let transition = transition(action)?;
    let effect = effect(action)?;
    let certificate = ExecutionStrategyCertificateV2::new(
        content(digest(&account_profile))?,
        content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        content(digest(&request_profile))?,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(digest(&effect))?,
        ArtifactReleaseIdV1::new(input.accelerator_artifact_release)
            .map_err(|_| JoinedGeneralArtifactErrorV5::Input)?,
        content([0x71; 32])?,
        content([0x72; 32])?,
        content([0x73; 32])?,
    )
    .to_bytes()
    .to_vec();
    let admission = ExecutionStrategyAdmissionV2::new(content(digest(&certificate))?)
        .to_bytes()
        .to_vec();
    let strategy = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::AdmittedAot,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        Some(content(digest(&certificate))?),
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        Some(content(digest(&admission))?),
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?
    .to_bytes()
    .to_vec();
    let descriptor = CapabilityProgramV4::new(
        content(GENERAL_CAPABILITY_KIND_ID_V1)?,
        content(GENERAL_CONFIG_SCHEMA_ID_V3)?,
        content(GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3)?,
        content(GENERAL_ROOT_SCHEMA_ID_V2)?,
        content(digest(&lifecycle_policy))?,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: ArtifactReferenceV4::new(
                content(dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID)?,
                content(digest(&account_profile))?,
            ),
            request_profile: ArtifactReferenceV4::new(
                content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
                content(digest(&request_profile))?,
            ),
            lifecycle: ArtifactReferenceV4::new(
                content(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)?,
                content(digest(&lifecycle_policy))?,
            ),
            strategy: ArtifactReferenceV4::new(
                content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
                content(digest(&strategy))?,
            ),
            transition: ArtifactReferenceV4::new(
                content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
                content(digest(&transition))?,
            ),
            effect: ArtifactReferenceV4::new(
                content(dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID)?,
                content(digest(&effect))?,
            ),
        },
        u32::try_from(dclutch_general_config_contract::GENERAL_ROOT_BYTES_V2)
            .map_err(|_| JoinedGeneralArtifactErrorV5::Input)?,
    )
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?
    .encode()
    .to_vec();
    Ok(JoinedGeneralActionArtifactsV5 {
        action,
        descriptor,
        account_profile,
        lifecycle_policy,
        request_profile,
        strategy,
        certificate,
        admission,
        transition,
        effect,
    })
}

fn account_profile(
    widths: GeneralExternalAccountWidthsV3,
    action: Action,
) -> Result<Vec<u8>, JoinedGeneralArtifactErrorV5> {
    // The whole artifact has one author, not just its operation list. The
    // trusted-environment declaration, the scratch-page span, the extra page
    // rule and the register geometry used to be restated here AND in a
    // contract-side test fixture, with nothing able to compare the two -- the
    // same shape `2e890d4` had to undo one level down.
    let bytes = general_account_profile_bytes_v3(action)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(action, widths, &mut scratch, &mut output)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    Ok(output)
}

fn lifecycle(
    input: JoinedGeneralArtifactInputV5,
    action: Action,
) -> Result<Vec<u8>, JoinedGeneralArtifactErrorV5> {
    let bytes = general_state_lifecycle_bytes_v5(action)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    let child_widths = if action == Action::InitializeSettlement {
        Some(
            GeneralChildRentWidthsV5::new(input.outcome_count, input.token_account_bytes)
                .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?,
        )
    } else {
        None
    };
    encode_general_state_lifecycle_v5_atomic(action, child_widths, &mut scratch, &mut output)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    Ok(output)
}

fn transition(action: Action) -> Result<Vec<u8>, JoinedGeneralArtifactErrorV5> {
    let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
    let count = prelude
        .checked_add(item)
        .and_then(|value| value.checked_add(epilogue))
        .ok_or(JoinedGeneralArtifactErrorV5::Input)?;
    let mut instructions = vec![GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3; count];
    let bytes = general_transition_program_bytes_v3(action)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_transition_program_v3_atomic(
        action,
        &mut instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    Ok(output)
}

fn effect(action: Action) -> Result<Vec<u8>, JoinedGeneralArtifactErrorV5> {
    let (fixed, item) = general_effect_instruction_count_v3(action);
    let count = fixed
        .checked_add(item)
        .ok_or(JoinedGeneralArtifactErrorV5::Input)?;
    let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; count];
    let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
    let bytes = general_effect_program_bytes_v3(action)
        .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_effect_program_v3_atomic(
        action,
        &mut instructions,
        &mut templates,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)?;
    Ok(output)
}

fn canonical_request(
    action: Action,
) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V2], JoinedGeneralArtifactErrorV5> {
    ControllerRequestV2 {
        action,
        expected_revision: 0,
        candidate_id: (!matches!(action, Action::Freeze)).then_some([0x81; 32]),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 1,
        terminal_record_bump: u8::from(action == Action::Close) * 2,
    }
    .to_bytes()
    .map_err(|_| JoinedGeneralArtifactErrorV5::Encoding)
}

fn validate_input(input: JoinedGeneralArtifactInputV5) -> Result<(), JoinedGeneralArtifactErrorV5> {
    if input.capacity_profile == [0; 32]
        || input.accelerator_artifact_release == [0; 32]
        || input.outcome_count == 0
        || input.token_account_bytes == 0
        || input.price_scale == 0
        || input.selection_policy == [0; 32]
        || input.quote_surplus_beneficiary == [0; 32]
    {
        return Err(JoinedGeneralArtifactErrorV5::Input);
    }
    Ok(())
}

fn content(value: [u8; 32]) -> Result<ContentId, JoinedGeneralArtifactErrorV5> {
    ContentId::new(value).map_err(|_| JoinedGeneralArtifactErrorV5::Input)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_custody_contract::OperationV1;
    use dclutch_general_adapter_contract::effect_artifacts_v3::{
        GeneralChildFrameV3, general_effect_route_count_v3, general_effect_route_frame_v3,
    };

    const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
        linked_basis_prefix: 256,
        result_domain: 192,
        rent_sysvar: 17,
        core_market: 320,
        activation_cache: 160,
        upgradeable_program: 36,
        trading_programdata_prefix: 45,
        claims_programdata_prefix: 45,
        core_programdata_prefix: 45,
        realm_record: 112,
        rent_credit: 48,
    };

    fn input(outcome_count: u32) -> JoinedGeneralArtifactInputV5 {
        JoinedGeneralArtifactInputV5 {
            capacity_profile: [0x41; 32],
            accelerator_artifact_release: [0x42; 32],
            outcome_count,
            external_widths: WIDTHS,
            token_account_bytes: 165,
            price_scale: 1_000,
            selection_policy: [0x43; 32],
            quote_surplus_beneficiary: [0x44; 32],
        }
    }

    #[test]
    fn all_seven_actions_join_at_runtime_widths_one_and_258() {
        for outcome_count in [1, 258] {
            let fixture = build_joined_general_artifacts_v5(input(outcome_count))
                .expect("complete General graph");
            assert_eq!(fixture.actions.len(), 7);
            for action in GENERAL_ACTIONS_V3 {
                let (selection, bytes) = fixture.selected(action).expect("selected action");
                authenticate_general_artifacts_v3(
                    selection,
                    bytes,
                    &canonical_request(action).expect("request"),
                    outcome_count,
                )
                .expect("joined artifacts");
            }
        }
    }

    #[test]
    fn child_routes_and_ordered_receipt_edges_are_exact() {
        assert_eq!(
            general_effect_route_count_v3(Action::InitializeSettlement),
            3
        );
        assert_eq!(general_effect_route_count_v3(Action::Close), 4);
        assert!(matches!(
            general_effect_route_frame_v3(Action::InitializeSettlement, 1)
                .expect("Initialize replay")
                .frame,
            GeneralChildFrameV3::Custody(OperationV1::InitializeReplay)
        ));
        assert!(matches!(
            general_effect_route_frame_v3(Action::InitializeSettlement, 2)
                .expect("Initialize vault")
                .frame,
            GeneralChildFrameV3::Custody(OperationV1::OpenVault)
        ));
        assert!(matches!(
            general_effect_route_frame_v3(Action::Close, 2)
                .expect("Close vault")
                .frame,
            GeneralChildFrameV3::Custody(OperationV1::CloseVault)
        ));
        assert!(matches!(
            general_effect_route_frame_v3(Action::Close, 3)
                .expect("Close replay")
                .frame,
            GeneralChildFrameV3::Custody(OperationV1::CloseReplay)
        ));
    }

    #[test]
    fn stale_release_and_zero_width_refuse() {
        let fixture = build_joined_general_artifacts_v5(input(1)).expect("fixture");
        let (mut selection, bytes) = fixture.selected(Action::Collect).expect("Collect");
        selection.artifact_release = [0x55; 32];
        assert!(
            authenticate_general_artifacts_v3(
                selection,
                bytes,
                &canonical_request(Action::Collect).expect("request"),
                1,
            )
            .is_err()
        );
        assert_eq!(
            build_joined_general_artifacts_v5(input(0)),
            Err(JoinedGeneralArtifactErrorV5::Input)
        );
    }
}
