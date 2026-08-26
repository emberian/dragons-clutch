//! Read-only admitted-AOT execution for General's runtime-width settlement.
//!
//! This is the semantic body consumed by a narrow SBF accelerator adapter. It
//! authenticates the complete action artifact chain, evaluates one settlement
//! transition from readonly records into caller-owned candidate buffers, and
//! returns only the generic chunk acknowledgement. It owns no account, invokes
//! no child, and never commits a cursor or effect; common Trading remains the
//! sole writer and CPI authority.

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AcceleratorAckV2, StrategyDispositionV2, resolve_execution_candidate_v2,
};
use dclutch_general_codec::Action;
use sha2::{Digest, Sha256};

use crate::{
    artifacts_v3::{
        GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GeneralHotEnvironmentV3, general_hot_scalar_count_v3,
        project_general_hot_candidate_v3,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_v2,
    },
    shadow_accelerator_v3::{GeneralAcceleratorBindingV3, general_accelerator_ack_v3},
};

/// Stable refusal from the complete read-only accelerator path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAdmittedAcceleratorErrorV3 {
    /// The descriptor-selected artifact chain refused.
    Artifacts,
    /// The request action and semantic settlement action differed.
    Action,
    /// Runtime-width settlement evaluation refused.
    Settlement,
    /// Candidate-bank projection refused.
    Candidate,
    /// The generic admitted strategy resolver refused.
    Strategy,
    /// The exact accelerator request/ack binding refused.
    Transport,
    /// A required content identity was zero.
    Identity,
}

/// Result alias for General's admitted accelerator.
pub type Result<T> = core::result::Result<T, GeneralAdmittedAcceleratorErrorV3>;

/// Complete caller-owned scratch and candidate buffers for one evaluation.
pub struct GeneralAdmittedSettlementBuffersV3<'a> {
    /// Non-authoritative successor-cursor scratch.
    pub cursor_scratch: &'a mut [u8],
    /// Complete successor cursor; unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Exact runtime inventory scratch.
    pub inventory_scratch: &'a mut [u8],
    /// Non-authoritative semantic-effect scratch.
    pub effect_scratch: &'a mut [u8],
    /// Complete semantic effect plan; unchanged on refusal.
    pub effect_output: &'a mut [u8],
    /// Non-authoritative register-bank scratch.
    pub bank_scratch: &'a mut [u8],
    /// Complete register candidate; unchanged on refusal.
    pub bank_output: &'a mut [u8],
}

/// Authenticate, evaluate, and acknowledge one General settlement chunk.
///
/// `input_bank` is the exact Account/Request-profile projection assembled by
/// Trading. The evaluator derives the semantic effect plan from readonly
/// cursor, certificate, and manifest records before it may project a candidate
/// bank. A caller-supplied effect or candidate can never bypass that derivation.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_general_admitted_settlement_v3<'a>(
    accelerator_request: &[u8],
    invocation_context: ContentId,
    selection: GeneralArtifactSelectionV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    family_request: &[u8],
    tail_count: u32,
    input_bank: &[u8],
    environment: GeneralHotEnvironmentV3,
    settlement: RuntimeSettlementViewV2<'_>,
    buffers: GeneralAdmittedSettlementBuffersV3<'a>,
) -> Result<AcceleratorAckV2<'a>> {
    let bundle =
        authenticate_general_artifacts_v3(selection, artifacts, family_request, tail_count)
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Artifacts)?;
    if bundle.strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || !action_matches(bundle.request.action, settlement.action)
    {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    let GeneralAdmittedSettlementBuffersV3 {
        cursor_scratch,
        cursor_output,
        inventory_scratch,
        effect_scratch,
        effect_output,
        bank_scratch,
        bank_output,
    } = buffers;
    evaluate_runtime_settlement_v2(
        settlement,
        RuntimeSettlementBuffersV2 {
            cursor_scratch: &mut *cursor_scratch,
            cursor_output: &mut *cursor_output,
            inventory_scratch: &mut *inventory_scratch,
            effect_scratch: &mut *effect_scratch,
            effect_output: &mut *effect_output,
        },
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    let accelerated = project_general_hot_candidate_v3(
        effect_output,
        cursor_output,
        tail_count,
        environment,
        input_bank,
        bank_scratch,
        bank_output,
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Candidate)?;
    let candidate = resolve_execution_candidate_v2(
        StrategyDispositionV2::AdmittedAot,
        None,
        Some(accelerated),
        Some(bundle.admitted_aot),
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Strategy)?;
    let certificate = bundle
        .strategy
        .certificate_program()
        .ok_or(GeneralAdmittedAcceleratorErrorV3::Artifacts)?;
    general_accelerator_ack_v3(
        accelerator_request,
        input_bank,
        candidate,
        GeneralAcceleratorBindingV3 {
            strategy: content(artifacts.strategy)?,
            certificate,
            capability_program: content(artifacts.descriptor)?,
            invocation_context,
            tail_count,
            scalar_count: general_hot_scalar_count_v3(tail_count)
                .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Candidate)?,
            identity_count: GENERAL_HOT_COMMON_IDENTITIES_V3,
        },
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Transport)
}

const fn action_matches(action: Action, settlement: RuntimeSettlementActionV2) -> bool {
    matches!(
        (action, settlement),
        (Action::Collect, RuntimeSettlementActionV2::Collect)
            | (Action::Materialize, RuntimeSettlementActionV2::Materialize)
            | (Action::Distribute, RuntimeSettlementActionV2::Distribute)
            | (Action::Close, RuntimeSettlementActionV2::Close)
    )
}

fn content(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(Sha256::digest(bytes).into())
        .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Identity)
}

#[cfg(test)]
mod tests {
    use dclutch_execution_strategy_contract::v2::ExecutionCandidateV2;

    use super::*;

    #[test]
    fn only_the_four_runtime_settlement_actions_join() {
        assert!(action_matches(
            Action::Collect,
            RuntimeSettlementActionV2::Collect
        ));
        assert!(action_matches(
            Action::Materialize,
            RuntimeSettlementActionV2::Materialize
        ));
        assert!(action_matches(
            Action::Distribute,
            RuntimeSettlementActionV2::Distribute
        ));
        assert!(action_matches(
            Action::Close,
            RuntimeSettlementActionV2::Close
        ));
        assert!(!action_matches(
            Action::Freeze,
            RuntimeSettlementActionV2::Close
        ));
    }

    #[test]
    fn content_identity_is_exact_and_deterministic() {
        let first = content(b"general admitted artifact").expect("content identity");
        let second = content(b"general admitted artifact").expect("same identity");
        assert_eq!(first, second);
        assert_ne!(
            first,
            content(b"substituted artifact").expect("other identity")
        );
    }

    #[test]
    fn candidate_type_carries_no_account_authority() {
        let refused = ExecutionCandidateV2::Refused;
        assert_eq!(refused, ExecutionCandidateV2::Refused);
    }
}
