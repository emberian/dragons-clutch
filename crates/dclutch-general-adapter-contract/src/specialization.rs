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
    use dclutch_general_codec::{CONTROLLER_REQUEST_BYTES, ControllerRequestV1};
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
    ];

    fn request(action: Action) -> [u8; CONTROLLER_REQUEST_BYTES] {
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
        ControllerRequestV1 {
            action,
            expected_revision: 7,
            candidate_id,
            page_index,
            execution_index,
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
            assert_eq!(output_scalars.first(), Some(&7));
            if action == Action::Freeze {
                assert_eq!(identity_count, TEST_IDENTITIES);
                assert_eq!(output_identities.first(), Some(&[0x99; 32]));
            } else {
                assert_eq!(output_identities.first(), Some(&[0x31; 32]));
            }
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
