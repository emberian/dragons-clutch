//! Data-defined General action request specialization.
//!
//! [`crate::plan`] remains a stateless accelerator/differential oracle. These
//! finalized artifacts are the generic Trading interpreter's request boundary:
//! CapabilityProgramSetV1 selects one profile by action, then the selected
//! profile independently revalidates that action and its exact coordinate
//! grammar before projecting caller-owned registers.

use dclutch_general_codec::Action;
use dclutch_request_profile_contract::{Error, RequestProfileV1};

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_request_profiles_v1.rs"]
mod generated;

pub use generated::*;

/// Borrow the exact Lean-generated RequestProfile for one General action.
#[must_use]
pub const fn general_request_profile_bytes_v1(action: Action) -> &'static [u8] {
    match action {
        // No Lean-emitted RequestProfile exists for the collection and
        // candidate actions. The empty slice is not a usable profile: the
        // artifact join compares the supplied profile against this value, and
        // `RequestProfileV1::decode` refuses an empty record, so an unauthored
        // action cannot be admitted with any profile at all.
        Action::VerifyCandidateRow => &GENERAL_VERIFY_CANDIDATE_ROW_REQUEST_PROFILE_V1,
        Action::OpenBatch => &GENERAL_OPEN_BATCH_REQUEST_PROFILE_V1,
        Action::CloseBatch => &GENERAL_CLOSE_BATCH_REQUEST_PROFILE_V1,
        Action::PlaceOrder => &GENERAL_PLACE_ORDER_REQUEST_PROFILE_V1,
        Action::CancelOrder => &GENERAL_CANCEL_ORDER_REQUEST_PROFILE_V1,
        Action::ReleaseOrder => &GENERAL_RELEASE_ORDER_REQUEST_PROFILE_V1,
        Action::CloseCandidate => &GENERAL_CLOSE_CANDIDATE_REQUEST_PROFILE_V1,
        Action::SubmitCandidate => &GENERAL_SUBMIT_CANDIDATE_REQUEST_PROFILE_V1,
        Action::Consider => &GENERAL_CONSIDER_REQUEST_PROFILE_V1,
        Action::Freeze => &GENERAL_FREEZE_REQUEST_PROFILE_V1,
        Action::InitializeSettlement => &GENERAL_INITIALIZE_REQUEST_PROFILE_V1,
        Action::Collect => &GENERAL_COLLECT_REQUEST_PROFILE_V1,
        Action::Materialize => &GENERAL_MATERIALIZE_REQUEST_PROFILE_V1,
        Action::Distribute => &GENERAL_DISTRIBUTE_REQUEST_PROFILE_V1,
        Action::Close => &GENERAL_CLOSE_REQUEST_PROFILE_V1,
    }
}

/// Hostile-decode the exact Lean-generated profile for one General action.
pub fn general_request_profile_v1(action: Action) -> Result<RequestProfileV1<'static>, Error> {
    RequestProfileV1::decode(general_request_profile_bytes_v1(action))
}

#[cfg(test)]
mod tests {
    use dclutch_general_codec::{
        successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
        successor_request_v3::{ControllerActionV3, ControllerRequestV3},
    };
    use dclutch_request_profile_contract::{ProjectionRegistersV1, project_atomic};

    use super::*;
    use crate::hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
    };

    const TEST_SCALARS: usize = GENERAL_HOT_COMMON_SCALARS_V3 as usize;
    const TEST_IDENTITIES: usize = GENERAL_HOT_COMMON_IDENTITIES_V3 as usize;

    const ACTIONS: [Action; GENERAL_REQUEST_PROFILE_ACTION_COUNT_V1] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
        Action::OpenBatch,
        Action::CloseBatch,
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
        Action::ReleaseOrder,
        Action::CloseCandidate,
    ];

    const fn speaks_v3(action: Action) -> bool {
        matches!(
            action,
            Action::OpenBatch
                | Action::CloseBatch
                | Action::PlaceOrder
                | Action::CancelOrder
                | Action::VerifyCandidateRow
                | Action::ReleaseOrder
                | Action::SubmitCandidate
                | Action::CloseCandidate
        )
    }

    fn request(action: Action) -> [u8; CONTROLLER_REQUEST_BYTES_V2] {
        if speaks_v3(action) {
            return ControllerRequestV3 {
                action: ControllerActionV3::from(action),
                // ReleaseOrder carries no optimistic revision at all: an order
                // state has no revision counter, and its grammar requires the
                // coordinate zero.
                expected_revision: if matches!(
                    action,
                    Action::PlaceOrder
                        | Action::CancelOrder
                        | Action::SubmitCandidate
                        | Action::ReleaseOrder
                        | Action::CloseCandidate
                ) {
                    0
                } else {
                    7
                },
                subject_id: Some([0x31; 32]),
                page_index: if action == Action::VerifyCandidateRow {
                    2
                } else {
                    0
                },
                execution_index: if action == Action::VerifyCandidateRow {
                    3
                } else {
                    0
                },
                manifest_order_index: 0,
                primary_state_bump: 42,
                secondary_state_bump: if matches!(
                    action,
                    Action::PlaceOrder | Action::CancelOrder | Action::VerifyCandidateRow
                ) {
                    43
                } else {
                    0
                },
                result_state_bump: if action == Action::VerifyCandidateRow {
                    44
                } else {
                    0
                },
            }
            .to_bytes()
            .expect("canonical V3 request");
        }
        let candidate_id = if action == Action::Freeze {
            None
        } else {
            Some([0x31; 32])
        };
        let (page_index, execution_index) = if matches!(
            action,
            Action::Consider | Action::Collect | Action::Distribute
        ) {
            (2, if action == Action::Consider { 0 } else { 3 })
        } else {
            (0, 0)
        };
        ControllerRequestV2 {
            action,
            expected_revision: 7,
            candidate_id,
            page_index,
            execution_index,
            manifest_order_index: u8::from(matches!(action, Action::Collect | Action::Distribute)),
            state_bump: 42,
            terminal_record_bump: if action == Action::Close { 43 } else { 0 },
        }
        .to_bytes()
        .expect("canonical request")
    }

    #[test]
    fn every_action_profile_revalidates_and_projects_its_exact_request() {
        for action in ACTIONS {
            let profile = general_request_profile_v1(action).expect("generated profile");
            let scalar_count = profile.scalar_count(0).expect("scalar width");
            let identity_count = profile.identity_count(0).expect("identity width");
            let input_scalars = [99_u64; TEST_SCALARS];
            let input_identities = [[0x99; 32]; TEST_IDENTITIES];
            let mut scratch_scalars = [0_u64; TEST_SCALARS];
            let mut scratch_identities = [[0_u8; 32]; TEST_IDENTITIES];
            let mut output_scalars = [0_u64; TEST_SCALARS];
            let mut output_identities = [[0_u8; 32]; TEST_IDENTITIES];
            project_atomic(
                profile,
                0,
                &request(action),
                ProjectionRegistersV1 {
                    input_scalars: input_scalars.get(..scalar_count).expect("scalar input"),
                    input_identities: input_identities
                        .get(..identity_count)
                        .expect("identity input"),
                    scratch_scalars: scratch_scalars
                        .get_mut(..scalar_count)
                        .expect("scalar scratch"),
                    scratch_identities: scratch_identities
                        .get_mut(..identity_count)
                        .expect("identity scratch"),
                    output_scalars: output_scalars
                        .get_mut(..scalar_count)
                        .expect("scalar output"),
                    output_identities: output_identities
                        .get_mut(..identity_count)
                        .expect("identity output"),
                },
            )
            .expect("selected profile accepts");
            if action == Action::VerifyCandidateRow {
                assert_eq!(output_scalars.first(), Some(&99));
                assert_eq!(output_scalars.get(1), Some(&2));
                assert_eq!(output_scalars.get(2), Some(&3));
                assert_eq!(output_scalars.get(94), Some(&7));
                assert_eq!(output_scalars.get(69), Some(&42));
                assert_eq!(output_scalars.get(70), Some(&43));
                assert_eq!(output_scalars.get(145), Some(&44));
                assert_eq!(output_identities.first(), Some(&[0x31; 32]));
            } else if action == Action::SubmitCandidate {
                assert_eq!(output_scalars.first(), Some(&99));
                assert_eq!(output_scalars.get(94), Some(&99));
                assert_eq!(output_identities.first(), Some(&[0x31; 32]));
                assert_eq!(output_identities.get(29), Some(&[0x99; 32]));
            } else if action == Action::CloseCandidate {
                // Close names the Candidate directly, carries no optimistic
                // revision, and has no secondary or result state.
                assert_eq!(output_scalars.first(), Some(&99));
                assert_eq!(output_scalars.get(94), Some(&99));
                assert_eq!(output_identities.first(), Some(&[0x31; 32]));
                assert_eq!(output_identities.get(29), Some(&[0x99; 32]));
            } else if matches!(
                action,
                Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder
            ) {
                // The order grammars name their subject in the ORDER register
                // the state PDA is keyed by, and project NO revision: the
                // replay-guard register passes through untouched.
                assert_eq!(output_scalars.first(), Some(&99));
                assert_eq!(output_scalars.get(94), Some(&99));
                assert_eq!(output_identities.get(3), Some(&[0x31; 32]));
                assert_eq!(output_identities.get(29), Some(&[0x99; 32]));
            } else if speaks_v3(action) {
                // The V3 grammar lands the optimistic root revision in the
                // replay-guard register and the subject in the batch identity
                // register; the settlement coordinates pass through untouched.
                assert_eq!(output_scalars.first(), Some(&99));
                assert_eq!(output_scalars.get(94), Some(&7));
                assert_eq!(output_identities.first(), Some(&[0x99; 32]));
                assert_eq!(output_identities.get(29), Some(&[0x31; 32]));
            } else {
                assert_eq!(output_scalars.first(), Some(&7));
                if action == Action::Freeze {
                    assert_eq!(identity_count, TEST_IDENTITIES);
                    assert_eq!(output_identities.first(), Some(&[0x99; 32]));
                } else {
                    assert_eq!(output_identities.first(), Some(&[0x31; 32]));
                }
            }
            assert_eq!(output_scalars.get(69), Some(&42));
            assert_eq!(
                output_scalars.get(70),
                Some(&if matches!(
                    action,
                    Action::Close
                        | Action::PlaceOrder
                        | Action::CancelOrder
                        | Action::VerifyCandidateRow
                ) {
                    43
                } else {
                    99
                })
            );
        }
    }

    #[test]
    fn substituted_action_and_noncanonical_coordinates_refuse_atomically() {
        for (index, action) in ACTIONS.into_iter().enumerate() {
            let profile = general_request_profile_v1(action).expect("generated profile");
            let scalar_count = profile.scalar_count(0).expect("scalar width");
            let identity_count = profile.identity_count(0).expect("identity width");
            let mut hostile = request(action);
            *hostile.get_mut(10).expect("action byte") =
                u8::try_from((index + 1) % ACTIONS.len()).expect("bounded action");
            let input_scalars = [1_u64; TEST_SCALARS];
            let input_identities = [[1_u8; 32]; TEST_IDENTITIES];
            let mut scratch_scalars = [0_u64; TEST_SCALARS];
            let mut scratch_identities = [[0_u8; 32]; TEST_IDENTITIES];
            let mut output_scalars = [0x55_u64; TEST_SCALARS];
            let mut output_identities = [[0x55_u8; 32]; TEST_IDENTITIES];
            let before_scalars = output_scalars;
            let before_identities = output_identities;
            assert!(
                project_atomic(
                    profile,
                    0,
                    &hostile,
                    ProjectionRegistersV1 {
                        input_scalars: input_scalars.get(..scalar_count).expect("scalar input"),
                        input_identities: input_identities
                            .get(..identity_count)
                            .expect("identity input"),
                        scratch_scalars: scratch_scalars
                            .get_mut(..scalar_count)
                            .expect("scalar scratch"),
                        scratch_identities: scratch_identities
                            .get_mut(..identity_count)
                            .expect("identity scratch"),
                        output_scalars: output_scalars
                            .get_mut(..scalar_count)
                            .expect("scalar output"),
                        output_identities: output_identities
                            .get_mut(..identity_count)
                            .expect("identity output"),
                    },
                )
                .is_err()
            );
            assert_eq!(output_scalars, before_scalars);
            assert_eq!(output_identities, before_identities);
        }
    }
}
