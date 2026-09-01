//! No dispatcher may hand an unauthored action an authored answer.
//!
//! `211079f6` grew `Action` by seven and wrote the new tags out one by one at
//! fourteen exhaustive matches, on the stated principle that the NEXT action
//! added must force a decision at each program rather than inherit one. Three
//! sites escaped that sweep, because none of them was an exhaustive match and so
//! the compiler had nothing to say:
//!
//! - `account_rules_v3::general_account_profile_operation_count_v3`, `_ => 5`,
//!   which handed an unauthored action the five settlement AccountProfile
//!   operations;
//! - `state_artifacts_v3::lifecycle_current_rent_quote_count`, `_ => 0`;
//! - `state_artifacts_v3::general_state_lifecycle_bytes`, which had no match at
//!   all: `lifecycle_counts` returns `(0, 0, 0)` for the seven, so the width
//!   computed as a bare header and `encode_general_state_lifecycle_v3_atomic`
//!   dispatched on `if action == Action::Close`, an if/else that is a catch-all
//!   wearing a condition. **It emitted a lifecycle artifact for an action with
//!   no triple.** Downstream the join refuses it — `action_plan_count` is
//!   zero — but a producer of an artifact for an unauthored action is exactly
//!   what writing the seven out one by one exists to prevent.
//!
//! None was reachable — every fallible entry point refused these actions by name
//! before the value is consumed — and none would have stayed unreachable,
//! because reachability is a property of the callers and a catch-all is a
//! promise about the callees. This file is the check that does not depend on
//! which of the two is true today: it asks every artifact dispatcher what it
//! says about the seven, and requires the answer to be a refusal or a
//! fail-closed zero.
//!
//! Deleting this file is part of authoring the last triple. When
//! `general_action_artifacts_authored_v3` returns true for all fourteen, every
//! assertion below is a statement about the empty set, and the test that
//! replaces it is the one that emits and joins fourteen artifact bundles.
//! Individual artifacts may become authored before the full quadruple: the
//! transition assertion below records that boundary explicitly while Effect
//! remains the full-bundle gate.

use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralAccountRuleErrorV3, general_account_profile_bytes_v3,
        general_account_profile_fixed_count_v3, general_account_profile_operation_count_v3,
        general_scratch_page_span_v3,
    },
    effect_artifacts_v3::{
        GeneralEffectArtifactErrorV3, general_action_artifacts_authored_v3,
        general_custody_callee_account_count_v3, general_effect_account_count_v3,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_program_bytes_v4, general_effect_route_count_v3,
        general_effect_route_frame_v3, general_effect_template_bytes_v3,
    },
    escrow_v1::{ActionCustodyTransferV1, general_action_custody_transfer_v1},
    specialization::general_request_profile_bytes_v1,
    state_artifacts_v3::{
        general_child_account_start_v3, general_readonly_evidence_count_v3,
        general_readonly_evidence_v3, general_state_lifecycle_bytes_v3,
    },
    transition_artifacts_v3::{
        GENERAL_VERIFY_CANDIDATE_ROW_TRANSITION_BYTES_V3, general_transition_instruction_count_v3,
        general_transition_program_bytes_v3,
    },
};
use dclutch_general_codec::Action;

/// The empty set: every accepted General action now owns a full artifact
/// quadruple. Keeping this typed partition makes the next enum addition force
/// an explicit decision at the same fail-closed dispatchers.
const UNAUTHORED: [Action; 0] = [];

/// All fourteen actions whose artifacts are authored, in catalogue order.
const AUTHORED: [Action; 14] = [
    Action::Consider,
    Action::Freeze,
    Action::InitializeSettlement,
    Action::Collect,
    Action::Materialize,
    Action::Distribute,
    Action::Close,
    Action::OpenBatch,
    Action::PlaceOrder,
    Action::CancelOrder,
    Action::CloseBatch,
    Action::SubmitCandidate,
    Action::VerifyCandidateRow,
    Action::ReleaseOrder,
];

#[test]
fn the_action_sets_partition_the_enum_and_the_gate_agrees_with_both() {
    for action in UNAUTHORED {
        assert!(
            !general_action_artifacts_authored_v3(action),
            "{action:?} is listed as unauthored and the gate disagrees",
        );
    }
    for action in AUTHORED {
        assert!(
            general_action_artifacts_authored_v3(action),
            "{action:?} is listed as authored and the gate disagrees",
        );
    }
    // Fourteen distinct tags, so the two lists are the whole enum.
    let mut tags: Vec<u8> = UNAUTHORED
        .iter()
        .chain(AUTHORED.iter())
        .map(|action| *action as u8)
        .collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), 14);
}

/// The incomplete full bundle refuses by name while its independently authored
/// transition remains available for byte-parity and execution tests.
#[test]
fn incomplete_bundle_refuses_while_its_transition_is_independently_authored() {
    for action in UNAUTHORED {
        assert_eq!(
            general_effect_account_count_v3(action),
            Err(GeneralEffectArtifactErrorV3::UnauthoredAction),
            "{action:?} was given an account width",
        );
        assert_eq!(
            general_effect_program_bytes_v3(action),
            Err(GeneralEffectArtifactErrorV3::UnauthoredAction),
            "{action:?} was given an effect width",
        );
        assert_eq!(
            general_effect_program_bytes_v4(action),
            Err(GeneralEffectArtifactErrorV3::UnauthoredAction),
            "{action:?} was given an envelope width",
        );
        assert_eq!(
            general_transition_program_bytes_v3(action),
            Ok(GENERAL_VERIFY_CANDIDATE_ROW_TRANSITION_BYTES_V3),
            "{action:?} lost its independently authored transition width",
        );
        assert_eq!(
            general_account_profile_fixed_count_v3(action),
            Err(GeneralAccountRuleErrorV3::Geometry),
            "{action:?} was given a fixed account count",
        );
        assert!(
            general_account_profile_bytes_v3(action).is_err(),
            "{action:?} was given an AccountProfile width",
        );
        assert!(
            general_scratch_page_span_v3(action).is_err(),
            "{action:?} was given a dynamic span",
        );
        assert!(
            general_state_lifecycle_bytes_v3(action).is_err(),
            "{action:?} was given a lifecycle policy width",
        );
        assert!(
            general_effect_route_frame_v3(action, 0).is_err(),
            "{action:?} was given a child frame",
        );
        assert!(
            general_readonly_evidence_v3(action, 0).is_err(),
            "{action:?} was given readonly evidence",
        );
    }
}

/// Every infallible count is the fail-closed value, and says so by being zero.
///
/// A zero here is not a shape. It is a value that cannot reach an encoder,
/// because the fallible siblings above refuse first — and the reason to require
/// it is that the alternative is a NONZERO value that looks like a shape, which
/// is exactly what the two catch-alls were producing.
#[test]
fn every_infallible_artifact_count_is_zero_for_an_unauthored_action() {
    for action in UNAUTHORED {
        assert_eq!(
            general_effect_route_count_v3(action),
            0,
            "{action:?} routes"
        );
        assert_eq!(
            general_effect_instruction_count_v3(action),
            (0, 0),
            "{action:?} effect instructions",
        );
        assert_eq!(
            general_effect_template_bytes_v3(action),
            0,
            "{action:?} templates",
        );
        assert_eq!(
            general_custody_callee_account_count_v3(action),
            0,
            "{action:?} custody callee",
        );
        assert_eq!(
            general_transition_instruction_count_v3(action),
            (0, 0, 0),
            "{action:?} transition instructions",
        );
        assert_eq!(
            general_readonly_evidence_count_v3(action),
            0,
            "{action:?} evidence count",
        );
        // THE ONE THAT WAS WRONG. A catch-all handed these the five settlement
        // AccountProfile operations. Zero is fail-closed twice over: it cannot
        // reach an encoder, and a profile projecting no operations leaves
        // `ROOT_LIFECYCLE_OBSERVATION` at zero, which is not `Active`.
        assert_eq!(
            general_account_profile_operation_count_v3(action),
            0,
            "{action:?} AccountProfile operations",
        );
    }
}

/// The RequestProfile is EMPTY rather than permissive.
///
/// An empty slice is not a usable profile: the artifact join compares against it
/// and `RequestProfileV1::decode` refuses an empty record, so an unauthored
/// action is admissible with NO profile rather than with one that projects
/// nothing and therefore constrains nothing.
#[test]
fn an_unauthored_action_has_no_request_profile_rather_than_a_permissive_one() {
    for action in UNAUTHORED {
        assert!(
            general_request_profile_bytes_v1(action).is_empty(),
            "{action:?} was given request-profile bytes",
        );
    }
    for action in AUTHORED {
        assert!(
            !general_request_profile_bytes_v1(action).is_empty(),
            "{action:?} lost its request profile",
        );
    }
}

/// The child frame prefix stays past the shared local-lifecycle accounts.
///
/// The seven have no child routes and no readonly evidence, so this coordinate
/// is not a frame they have — but it must not read as a NARROWER frame than the
/// local-lifecycle prefix every action occupies, because an arithmetic consumer
/// that reached it without a guard would then compute a window overlapping the
/// state, payer and rent-credit accounts rather than an empty one past them.
/// `Freeze` is the authored action that also selects no evidence, so it is the
/// exact comparison rather than a restated literal.
#[test]
fn the_unauthored_child_prefix_never_narrows_the_common_frame() {
    for action in UNAUTHORED {
        assert_eq!(general_readonly_evidence_count_v3(action), 0);
        assert_eq!(
            general_child_account_start_v3(action),
            general_child_account_start_v3(Action::Freeze),
            "{action:?} child prefix",
        );
    }
    assert_eq!(general_readonly_evidence_count_v3(Action::Freeze), 0);
}

/// The escrow table already names the three movements the escrow verbs will
/// perform, and names nothing for the four that move no collateral.
///
/// This is the one place an unauthored action legitimately has a non-empty
/// answer, because a compartment ruling is a protocol fact before it is an
/// artifact — the same argument by which their tags and ProgramSet coordinates
/// already exist. When their triples are authored, `build_action` reads these
/// rows exactly as the settlement actions' rows are read today.
#[test]
fn the_escrow_verbs_already_name_their_movements_and_the_rest_name_none() {
    for action in [
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::ReleaseOrder,
    ] {
        assert!(
            matches!(
                general_action_custody_transfer_v1(action),
                ActionCustodyTransferV1::Fixed(_)
            ),
            "{action:?} moves collateral and names no movement",
        );
    }
    for action in [
        Action::OpenBatch,
        Action::CloseBatch,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
    ] {
        assert_eq!(
            general_action_custody_transfer_v1(action),
            ActionCustodyTransferV1::None,
            "{action:?} names a Custody transfer it does not perform",
        );
    }
}
