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
    AcceleratorAckV2, ExecutionCandidateV2, StrategyDispositionV2, resolve_execution_candidate_v2,
};
use dclutch_general_codec::{Action, SelectionPolicyV1};
use sha2::{Digest, Sha256};

use crate::{
    artifacts_v3::{
        GeneralArtifactBundleV3, GeneralArtifactBytesV3, GeneralArtifactSelectionV3,
        authenticate_general_artifacts_v3,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GeneralHotEnvironmentV3, general_hot_scalar_count_v3,
        project_general_hot_candidate_v3, project_general_initialize_candidate_v3,
        project_general_selection_candidate_v3,
    },
    runtime_manifest::SettlementManifestV2,
    runtime_selection::{
        RuntimeSelectionCursorV2, consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
    },
    runtime_width::VerifiedCandidateV2,
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

/// Authenticate the exact terminal selection-to-verification join required
/// before a settlement cursor may be initialized.
///
/// A complete verifier certificate is not selection authority by itself. The
/// frozen cursor must name its candidate, coordinate, revision, Product,
/// Batch, and exact record digest under the immutable selection policy.
pub fn authenticate_frozen_selection_v3<'a>(
    selection_policy_id: [u8; 32],
    product_id: [u8; 32],
    price_scale: u64,
    requested_candidate: Option<[u8; 32]>,
    tail_count: u32,
    frozen_selection: &[u8],
    verified: &'a [u8],
) -> Result<VerifiedCandidateV2<'a>> {
    let verified_value = VerifiedCandidateV2::decode(verified)
        .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    let verified_header = verified_value.header();
    let frozen = RuntimeSelectionCursorV2::decode(frozen_selection)
        .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    let frozen_header = frozen.header();
    let verified_digest: [u8; 32] = Sha256::digest(verified).into();
    if frozen_header.phase != crate::runtime_selection::RuntimeSelectionPhaseV2::Frozen
        || frozen_header.outcome_count != tail_count
        || frozen_header.policy_id != selection_policy_id
        || frozen_header.product_id != product_id
        || frozen_header.price_scale != price_scale
        || frozen_header.best_candidate_id != verified_header.candidate_id
        || frozen_header.best_candidate_coordinate != verified_header.candidate_coordinate
        || frozen_header.best_verified_revision != verified_header.revision
        || frozen_header.product_id != verified_header.product_id
        || frozen_header.batch_id != verified_header.batch_id
        || frozen_header.best_verified_digest != verified_digest
        || verified_header.outcome_count != tail_count
        || verified_header.product_id != product_id
        || verified_header.price_scale != price_scale
        || Some(verified_header.candidate_id) != requested_candidate
    {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    Ok(verified_value)
}

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

/// Readonly inputs for Consider or permissionless Freeze.
#[derive(Clone, Copy, Debug)]
pub struct GeneralAdmittedSelectionViewV3<'a> {
    /// Exact selection prestate, or all-zero vacant bytes for first Consider.
    pub selection_before: &'a [u8],
    /// Authenticated interpreted policy; required only for Consider.
    pub policy: Option<SelectionPolicyV1>,
    /// Exact submitted verified candidate; required only for Consider.
    pub submitted_verified: Option<&'a [u8]>,
}

/// Caller-owned selection and candidate-bank buffers.
pub struct GeneralAdmittedSelectionBuffersV3<'a> {
    /// Non-authoritative selection scratch.
    pub selection_scratch: &'a mut [u8],
    /// Complete selection successor candidate.
    pub selection_output: &'a mut [u8],
    /// Non-authoritative register-bank scratch.
    pub bank_scratch: &'a mut [u8],
    /// Complete register candidate.
    pub bank_output: &'a mut [u8],
}

/// Caller-owned initialization and candidate-bank buffers.
pub struct GeneralAdmittedInitializeBuffersV3<'a> {
    /// Exact runtime inventory scratch.
    pub inventory_scratch: &'a mut [u8],
    /// Non-authoritative SettlementCursor scratch.
    pub cursor_scratch: &'a mut [u8],
    /// Complete SettlementCursor successor candidate.
    pub cursor_output: &'a mut [u8],
    /// Non-authoritative register-bank scratch.
    pub bank_scratch: &'a mut [u8],
    /// Complete register candidate.
    pub bank_output: &'a mut [u8],
}

/// Evaluate Consider or Freeze through the same admitted candidate transport.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_general_admitted_selection_v3<'a>(
    accelerator_request: &[u8],
    invocation_context: ContentId,
    selection: GeneralArtifactSelectionV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    family_request: &[u8],
    tail_count: u32,
    input_bank: &[u8],
    view: GeneralAdmittedSelectionViewV3<'_>,
    buffers: GeneralAdmittedSelectionBuffersV3<'a>,
) -> Result<AcceleratorAckV2<'a>> {
    let bundle =
        authenticate_general_artifacts_v3(selection, artifacts, family_request, tail_count)
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Artifacts)?;
    if bundle.strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || !matches!(bundle.request.action, Action::Consider | Action::Freeze)
    {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    let GeneralAdmittedSelectionBuffersV3 {
        selection_scratch,
        selection_output,
        bank_scratch,
        bank_output,
    } = buffers;
    match bundle.request.action {
        Action::Consider => {
            let policy = view
                .policy
                .ok_or(GeneralAdmittedAcceleratorErrorV3::Action)?;
            let submitted = view
                .submitted_verified
                .ok_or(GeneralAdmittedAcceleratorErrorV3::Action)?;
            let submitted_value = VerifiedCandidateV2::decode(submitted)
                .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
            let submitted_header = submitted_value.header();
            if policy.policy_id != bundle.config.selection_policy_id()
                || Some(submitted_header.candidate_id) != bundle.request.candidate_id
                || submitted_header.candidate_coordinate != bundle.request.page_index
                || submitted_header.outcome_count != tail_count
            {
                return Err(GeneralAdmittedAcceleratorErrorV3::Action);
            }
            consider_verified_candidate_v2(
                policy,
                view.selection_before,
                submitted,
                bundle.request.expected_revision,
                selection_scratch,
                selection_output,
            )
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
        }
        Action::Freeze => {
            if view.policy.is_some() || view.submitted_verified.is_some() {
                return Err(GeneralAdmittedAcceleratorErrorV3::Action);
            }
            freeze_selection_v2(
                view.selection_before,
                bundle.request.expected_revision,
                selection_scratch,
                selection_output,
            )
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
        }
        _ => return Err(GeneralAdmittedAcceleratorErrorV3::Action),
    }
    let selected = RuntimeSelectionCursorV2::decode(selection_output)
        .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    if selected.header().policy_id != bundle.config.selection_policy_id() {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    let accelerated = project_general_selection_candidate_v3(
        bundle.request.action,
        selection_output,
        tail_count,
        input_bank,
        bank_scratch,
        bank_output,
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Candidate)?;
    acknowledge_candidate(
        accelerator_request,
        invocation_context,
        artifacts,
        bundle,
        input_bank,
        tail_count,
        accelerated,
    )
}

/// Initialize the runtime-width settlement cursor from terminal verification.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_general_admitted_initialize_v3<'a>(
    accelerator_request: &[u8],
    invocation_context: ContentId,
    selection: GeneralArtifactSelectionV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    family_request: &[u8],
    tail_count: u32,
    input_bank: &[u8],
    environment: GeneralHotEnvironmentV3,
    frozen_selection: &[u8],
    verifier: &[u8],
    verified: &[u8],
    buffers: GeneralAdmittedInitializeBuffersV3<'a>,
) -> Result<AcceleratorAckV2<'a>> {
    let bundle =
        authenticate_general_artifacts_v3(selection, artifacts, family_request, tail_count)
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Artifacts)?;
    if bundle.strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || bundle.request.action != Action::InitializeSettlement
    {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    authenticate_frozen_selection_v3(
        bundle.config.selection_policy_id(),
        environment.product_record_digest,
        bundle.config.price_scale(),
        bundle.request.candidate_id,
        tail_count,
        frozen_selection,
        verified,
    )?;
    let GeneralAdmittedInitializeBuffersV3 {
        inventory_scratch,
        cursor_scratch,
        cursor_output,
        bank_scratch,
        bank_output,
    } = buffers;
    initialize_runtime_settlement_v2(
        verifier,
        verified,
        bundle.request.expected_revision,
        inventory_scratch,
        cursor_scratch,
        cursor_output,
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    let accelerated = project_general_initialize_candidate_v3(
        cursor_output,
        tail_count,
        environment,
        input_bank,
        bank_scratch,
        bank_output,
    )
    .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Candidate)?;
    acknowledge_candidate(
        accelerator_request,
        invocation_context,
        artifacts,
        bundle,
        input_bank,
        tail_count,
        accelerated,
    )
}

fn acknowledge_candidate<'a>(
    accelerator_request: &[u8],
    invocation_context: ContentId,
    artifacts: GeneralArtifactBytesV3<'_>,
    bundle: GeneralArtifactBundleV3<'_>,
    input_bank: &[u8],
    tail_count: u32,
    accelerated: ExecutionCandidateV2<'a>,
) -> Result<AcceleratorAckV2<'a>> {
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
    authenticate_settlement_request_v3(bundle, environment, &settlement)?;
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

fn authenticate_settlement_request_v3(
    bundle: GeneralArtifactBundleV3<'_>,
    environment: GeneralHotEnvironmentV3,
    settlement: &RuntimeSettlementViewV2<'_>,
) -> Result<()> {
    let request = bundle.request;
    let verified = VerifiedCandidateV2::decode(settlement.verified)
        .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
    let verified_header = verified.header();
    if settlement.expected_revision != request.expected_revision
        || request.candidate_id != Some(verified_header.candidate_id)
        || verified_header.outcome_count != bundle.tail_count
        || verified_header.product_id != environment.product_record_digest
        || verified_header.price_scale != bundle.config.price_scale()
    {
        return Err(GeneralAdmittedAcceleratorErrorV3::Action);
    }
    match settlement.action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => {
            if settlement.manifest_order_index != u32::from(request.manifest_order_index) {
                return Err(GeneralAdmittedAcceleratorErrorV3::Action);
            }
            let manifest = SettlementManifestV2::decode(
                settlement
                    .manifest
                    .ok_or(GeneralAdmittedAcceleratorErrorV3::Action)?,
            )
            .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
            let row = manifest
                .order(settlement.manifest_order_index)
                .map_err(|_| GeneralAdmittedAcceleratorErrorV3::Settlement)?;
            if row.header().source_page_index != request.page_index
                || row.header().source_execution_index != u32::from(request.execution_index)
            {
                return Err(GeneralAdmittedAcceleratorErrorV3::Action);
            }
        }
        RuntimeSettlementActionV2::Materialize | RuntimeSettlementActionV2::Close => {
            if settlement.manifest.is_some()
                || settlement.manifest_order_index != 0
                || request.page_index != 0
                || request.execution_index != 0
                || request.manifest_order_index != 0
            {
                return Err(GeneralAdmittedAcceleratorErrorV3::Action);
            }
        }
    }
    Ok(())
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
    use dclutch_general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion};
    use std::{vec, vec::Vec};

    use super::*;
    use crate::{
        runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionPhaseV2},
        runtime_width::{VerifiedCandidateHeaderV2, VerifiedCandidateV2, verified_candidate_len},
    };

    const PRODUCT: [u8; 32] = [21; 32];
    const BATCH: [u8; 32] = [22; 32];
    const POLICY: [u8; 32] = [23; 32];
    const CANDIDATE: [u8; 32] = [24; 32];

    fn selection_policy() -> SelectionPolicyV1 {
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: POLICY,
            criterion_count: 3,
            criteria,
        }
    }

    fn verified_candidate(width: u32) -> Vec<u8> {
        let count = usize::try_from(width).expect("test width");
        let mut output = vec![0; verified_candidate_len(width).expect("verified width")];
        VerifiedCandidateV2::encode_into(
            VerifiedCandidateHeaderV2 {
                outcome_count: width,
                page_count: 1,
                candidate_coordinate: 7,
                revision: 9,
                candidate_id: CANDIDATE,
                product_id: PRODUCT,
                batch_id: BATCH,
                filled_lots: 11,
                quote_debit: 12,
                quote_credit: 1,
                price_scale: 10,
            },
            &vec![11; count],
            &vec![13; count],
            &mut output,
        )
        .expect("verified candidate");
        output
    }

    fn frozen_selection(width: u32, verified: &[u8]) -> [u8; RUNTIME_SELECTION_CURSOR_BYTES_V2] {
        let mut scratch = [0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let mut open = [0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        consider_verified_candidate_v2(
            selection_policy(),
            &[0; RUNTIME_SELECTION_CURSOR_BYTES_V2],
            verified,
            0,
            &mut scratch,
            &mut open,
        )
        .expect("consider selected candidate");
        let mut frozen = [0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        freeze_selection_v2(&open, 1, &mut scratch, &mut frozen).expect("freeze selection");
        let decoded = RuntimeSelectionCursorV2::decode(&frozen).expect("frozen cursor");
        assert_eq!(decoded.header().outcome_count, width);
        assert_eq!(decoded.header().phase, RuntimeSelectionPhaseV2::Frozen);
        frozen
    }

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

    #[test]
    fn initialize_requires_the_exact_frozen_best_valid_submitted_candidate() {
        for width in [1_u32, 258] {
            let verified = verified_candidate(width);
            let frozen = frozen_selection(width, &verified);
            let selected = authenticate_frozen_selection_v3(
                POLICY,
                PRODUCT,
                10,
                Some(CANDIDATE),
                width,
                &frozen,
                &verified,
            )
            .expect("exact frozen selection joins");
            assert_eq!(selected.header().candidate_id, CANDIDATE);

            let mut open = frozen;
            open[10] = RuntimeSelectionPhaseV2::Open.tag();
            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    PRODUCT,
                    10,
                    Some(CANDIDATE),
                    width,
                    &open,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );

            let mut substituted_digest = frozen;
            substituted_digest[176] ^= 1;
            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    PRODUCT,
                    10,
                    Some(CANDIDATE),
                    width,
                    &substituted_digest,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );

            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    PRODUCT,
                    10,
                    Some([25; 32]),
                    width,
                    &frozen,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );
            assert_eq!(
                authenticate_frozen_selection_v3(
                    [26; 32],
                    PRODUCT,
                    10,
                    Some(CANDIDATE),
                    width,
                    &frozen,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );

            let mut substituted_revision = verified.clone();
            substituted_revision[24] ^= 1;
            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    PRODUCT,
                    10,
                    Some(CANDIDATE),
                    width,
                    &frozen,
                    &substituted_revision,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );

            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    [27; 32],
                    10,
                    Some(CANDIDATE),
                    width,
                    &frozen,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );
            assert_eq!(
                authenticate_frozen_selection_v3(
                    POLICY,
                    PRODUCT,
                    11,
                    Some(CANDIDATE),
                    width,
                    &frozen,
                    &verified,
                ),
                Err(GeneralAdmittedAcceleratorErrorV3::Action)
            );
        }
    }
}
