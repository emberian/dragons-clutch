//! Executable diagnosis of occurrence facts retained in an immutable release.
//!
//! This is a native compiler/VM control. No account creation, CPI, validator
//! execution, or accepted second Prepare is claimed by its scratch bank.

use super::tests::{compiler_input_at_occurrence, prepared_founder};
use super::{compile_series_found_prepare_selection_v1, derive_series_found_prepare_preprofile_v1};
use dclutch_core_contract::ContentId;
use dclutch_custody::ProjectedCustodyRequestV1;
use dclutch_market::capability_program::{set_v2::CapabilityProgramSetV2, v4::CapabilityProgramV4};
use dclutch_trading::series::{TemplateV3, request::SeriesActionRequestV3};
use dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_ROOT_KEY_IDENTITY_V5;
use dclutch_vm::{
    account_profile::{v2::derive_effect_permissions, v3::AccountProfileV3},
    effect::{
        v2::{AccountInput, AccountPermission},
        v3::{ProjectionV3, project_atomic},
        v5::ProgramV5,
    },
    request_profile::{ProjectionRegistersV1, RequestProfileV1},
    v3::{ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic},
};
use solana_program::hash::hash;
use solana_sdk::pubkey::Pubkey;

fn artifact(selected: &crate::model::SelectedCapabilityV1, label: &str) -> Vec<u8> {
    crate::runtime::decode_hex(
        &selected
            .records
            .iter()
            .find(|record| record.label == label)
            .expect("compiler published the action artifact")
            .body_hex,
    )
    .expect("artifact bytes")
}

/// Execute the actual RequestProfile, TransitionVM and Effect request writer.
/// The only account-projected register used by the current Prepare requests
/// is the parent root. Account lengths here permit the unrelated Ticket data
/// writes; they are an Effect component fixture, not account-authentication
/// evidence. Permissions and alias coordinates come from the compiled profile.
fn projected_initialize(
    selected: &crate::model::SelectedCapabilityV1,
    family: &[u8],
    root: Pubkey,
) -> ProjectedCustodyRequestV1 {
    let profile_bytes = artifact(selected, "series_prepare_account_profile_record");
    let profile = AccountProfileV3::decode(&profile_bytes)
        .expect("compiled profile")
        .base();
    let request_bytes = artifact(selected, "series_prepare_request_profile_record");
    let request = RequestProfileV1::decode(&request_bytes).expect("compiled request profile");
    let transition_bytes = artifact(selected, "series_prepare_transition_record");
    let transition = ProgramV3::decode(&transition_bytes).expect("compiled transition");
    let effect_bytes = artifact(selected, "series_prepare_effect_record");
    let effect = ProgramV5::decode(&effect_bytes)
        .expect("compiled effect")
        .base()
        .base();
    let scalars = usize::from(request.common_scalar_count());
    let identities = usize::from(request.common_identity_count());
    let input_scalars = vec![0; scalars];
    let mut input_identities = vec![[0; 32]; identities];
    input_identities[usize::from(SERIES_PREPARE_ROOT_KEY_IDENTITY_V5)] = root.to_bytes();
    let mut scratch_scalars = vec![0; scalars];
    let mut scratch_identities = vec![[0; 32]; identities];
    let mut output_scalars = vec![0; scalars];
    let mut output_identities = vec![[0; 32]; identities];
    dclutch_vm::request_profile::project_atomic(
        request,
        0,
        &family[..128],
        ProjectionRegistersV1 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
    )
    .expect("actual second Prepare request projects");
    let mut candidate_scalars = vec![0; scalars];
    let mut candidate_identities = vec![[0; 32]; identities];
    execute_fold_atomic(
        transition,
        0,
        RegisterInput {
            scalars: &output_scalars,
            identities: &output_identities,
        },
        RegisterOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        RegisterOutput {
            scalars: &mut candidate_scalars,
            identities: &mut candidate_identities,
        },
    )
    .expect("actual Prepare transition executes");
    let count = effect.account_count(0).expect("account count");
    let aliases = (0..count)
        .map(|coordinate| profile.representative(0, coordinate).expect("alias"))
        .collect::<Vec<_>>();
    let accounts = vec![
        AccountInput {
            lamports: 1,
            data_len: dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3
        };
        count
    ];
    let mut permissions = vec![AccountPermission::default(); count];
    derive_effect_permissions(profile, 0, &mut permissions).expect("profile-owned permissions");
    let mut scratch_lamports = vec![0; count];
    let mut output_lamports = vec![0; count];
    let mut requests = vec![0; effect.request_bytes(0).expect("request bank width")];
    project_atomic(
        effect,
        0,
        ProjectionV3 {
            scalars: &candidate_scalars,
            identities: &candidate_identities,
            aliases: &aliases,
            accounts: &accounts,
            permissions: &permissions,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: &mut output_lamports,
            requests: &mut requests,
        },
    )
    .expect("actual retained Effect projects its child bank");
    let width = usize::try_from(
        effect
            .route(0)
            .expect("Initialize route")
            .fixed_request_bytes(),
    )
    .expect("request width");
    ProjectedCustodyRequestV1::decode(&requests[..width]).expect("native Initialize decoder")
}

#[test]
fn known_defect_same_bundle_second_prepare_retains_first_occurrence_market() {
    let prepared = prepared_founder();
    let root = Pubkey::new_unique();
    let first_input =
        compiler_input_at_occurrence(&prepared, root, &prepared.admitted.tickets()[0], 0);
    let template = TemplateV3::decode(prepared.admitted.template()).expect("canonical Template");
    let first_state = first_input.lifecycle.series;
    let compiled = compile_series_found_prepare_selection_v1(
        first_input,
        ContentId::new([62; 32]).expect("component certificate identity"),
    )
    .expect("actual five-action compiler accepts occurrence zero");
    let first_native = ProjectedCustodyRequestV1::decode(
        compiled
            .prepare_children
            .prepare_requests()
            .projected_initialize,
    )
    .expect("first native child");
    assert_eq!(
        projected_initialize(&compiled.selected, &compiled.parents.prepare_request, root),
        first_native,
        "positive control: the immutable artifacts reproduce the first native child exactly"
    );

    let next_state = first_state
        .prepare_ticket(first_state.revision())
        .expect("Prepare transition");
    let next_state = next_state
        .settle_current(next_state.revision(), template.occurrence_count())
        .expect("settlement transition");
    let next_state = next_state
        .retire_ticket(next_state.revision())
        .expect("Retire transition");
    assert_eq!(next_state.next_occurrence(), 1);
    let mut second_input =
        compiler_input_at_occurrence(&prepared, root, &prepared.admitted.tickets()[1], 1);
    second_input.lifecycle.series = next_state;
    second_input.lifecycle.now_slot = template.scheduled_slot(1).expect("second schedule");
    let second = derive_series_found_prepare_preprofile_v1(&mut second_input)
        .expect("actual codecs admit occurrence one after real replay transitions");
    let second_native = ProjectedCustodyRequestV1::decode(
        second
            .prepare_children
            .prepare_requests()
            .projected_initialize,
    )
    .expect("second native child");
    assert_ne!(
        second_native.market, first_native.market,
        "different canonical occurrence Markets"
    );
    assert_ne!(
        second.parents.prepare_digest,
        compiled.parents.prepare_digest
    );
    assert_eq!(
        SeriesActionRequestV3::decode(&second.parents.prepare_request)
            .expect("second family")
            .expected_series_revision(),
        next_state.revision()
    );

    let set_bytes = crate::runtime::decode_hex(&compiled.selected.program_set_hex)
        .expect("retained ProgramSet");
    let set = CapabilityProgramSetV2::decode(&set_bytes).expect("canonical ProgramSet");
    let selected = set
        .select_entry(&second.parents.prepare_request)
        .expect("second Prepare selects from the retained set");
    let descriptor_bytes = artifact(&compiled.selected, "series_prepare_descriptor_record");
    assert_eq!(
        selected.descriptor().program().to_bytes(),
        hash(&descriptor_bytes).to_bytes()
    );
    let descriptor = CapabilityProgramV4::decode(&descriptor_bytes).expect("retained descriptor");
    assert_eq!(
        descriptor.effect().program().to_bytes(),
        hash(&artifact(
            &compiled.selected,
            "series_prepare_effect_record"
        ))
        .to_bytes()
    );
    let actual = projected_initialize(&compiled.selected, &second.parents.prepare_request, root);
    assert_eq!(
        actual.market, first_native.market,
        "known defect: retained Effect still names the first Market"
    );
    assert_ne!(
        actual.market, second_native.market,
        "the admitted second occurrence cannot use this child request"
    );
    assert_eq!(
        actual.context_digest, first_native.context_digest,
        "known defect also retains the first Ticket-derived context"
    );
}
