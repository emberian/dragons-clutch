//! Recurrence control for native requests materialized by one immutable release.
//!
//! This is a native compiler/VM control. No account creation, CPI, validator
//! execution, or accepted second Prepare is claimed by its scratch bank.

use super::tests::{compiler_input_with_plan_at_occurrence, prepared_founder_with_plan};
use super::{compile_series_found_prepare_selection_v1, derive_series_found_prepare_preprofile_v1};
use dclutch_core_contract::ContentId;
use dclutch_custody::ProjectedCustodyRequestV1;
use dclutch_market::capability_program::{set_v2::CapabilityProgramSetV2, v4::CapabilityProgramV4};
use dclutch_trading::series::{TemplateV3, request::SeriesActionRequestV3};
use dclutch_trading_sbf::series::{
    derived_prepare_v1::{SeriesPrepareDerivedRequestErrorV1, SeriesPrepareDerivedRequestsV1},
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_DERIVED_REQUEST_SCALAR_START_V1,
        SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1, SERIES_PREPARE_ROOT_KEY_IDENTITY_V5,
    },
};
use dclutch_vm::{
    account_profile::{v2::derive_effect_permissions_with_dynamic_spans, v3::AccountProfileV3},
    effect::{
        v2::{AccountInput, AccountPermission},
        v3::{ProjectionV3, ResolvedEffectV3},
        v4::{ResolvedWriteRangeV4, project_atomic_visiting},
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
fn projected_requests(
    selected: &crate::model::SelectedCapabilityV1,
    family: &[u8],
    root: Pubkey,
    derived: &SeriesPrepareDerivedRequestsV1,
) -> ActionProjection {
    project_action(selected, "prepare", family, &[], |scalars, identities| {
        derived.write_scalar_words(family, root.to_bytes(),
            &mut scalars[usize::from(SERIES_PREPARE_DERIVED_REQUEST_SCALAR_START_V1)..usize::from(dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_END_V1)])
            .expect("private native bank is bound to this family and root");
        derived.write_replay_scalars(family, root.to_bytes(),
            &mut scalars[usize::from(dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_END_V1)..])
            .expect("native root successor is bound to this family and root");
        identities[usize::from(SERIES_PREPARE_ROOT_KEY_IDENTITY_V5)] = root.to_bytes();
    })
}

struct ActionProjection {
    requests: Vec<u8>,
    scalars: Vec<u64>,
    writes: Vec<ResolvedEffectV3>,
}

fn project_action(
    selected: &crate::model::SelectedCapabilityV1,
    action: &str,
    family: &[u8],
    span_counts: &[u32],
    seed: impl FnOnce(&mut [u64], &mut [[u8; 32]]),
) -> ActionProjection {
    let profile_bytes = artifact(selected, &format!("series_{action}_account_profile_record"));
    let profile = AccountProfileV3::decode(&profile_bytes)
        .expect("compiled profile")
        .base();
    let request_bytes = artifact(selected, &format!("series_{action}_request_profile_record"));
    let request = RequestProfileV1::decode(&request_bytes).expect("compiled request profile");
    let transition_bytes = artifact(selected, &format!("series_{action}_transition_record"));
    let transition = ProgramV3::decode(&transition_bytes).expect("compiled transition");
    let effect_bytes = artifact(selected, &format!("series_{action}_effect_record"));
    let effect = ProgramV5::decode(&effect_bytes)
        .expect("compiled effect")
        .base();
    let scalars = usize::from(request.common_scalar_count());
    let identities = usize::from(request.common_identity_count());
    let mut input_scalars = vec![0; scalars];
    let mut input_identities = vec![[0; 32]; identities];
    seed(&mut input_scalars, &mut input_identities);
    let mut scratch_scalars = vec![0; scalars];
    let mut scratch_identities = vec![[0; 32]; identities];
    let mut output_scalars = vec![0; scalars];
    let mut output_identities = vec![[0; 32]; identities];
    dclutch_vm::request_profile::project_atomic(
        request,
        0,
        &family[..request.request_bytes(0).expect("native request prefix")],
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
    .unwrap_or_else(|error| panic!("actual {action} transition: {error:?}"));
    let count = effect
        .account_count(0, &candidate_scalars)
        .expect("account count");
    let aliases = (0..count)
        .map(|coordinate| {
            profile
                .representative_with_dynamic_spans(0, span_counts, coordinate)
                .expect("alias")
        })
        .collect::<Vec<_>>();
    let accounts = vec![
        AccountInput {
            lamports: 1,
            data_len: dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5
        };
        count
    ];
    let mut permissions = vec![AccountPermission::default(); count];
    derive_effect_permissions_with_dynamic_spans(profile, 0, span_counts, &mut permissions)
        .expect("profile-owned permissions");
    let mut scratch_lamports = vec![0; count];
    let mut output_lamports = vec![0; count];
    let mut requests = vec![0; effect.base().request_bytes(0).expect("request bank width")];
    let mut ranges = vec![
        ResolvedWriteRangeV4::vacant();
        effect.data_write_operation_count(0).expect("data writes")
    ];
    let mut writes = Vec::new();
    project_atomic_visiting(
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
        &mut ranges,
        &mut |operation| {
            writes.push(operation);
            Ok(())
        },
    )
    .expect("actual retained Effect projects its child bank");
    ActionProjection {
        requests,
        scalars: candidate_scalars,
        writes,
    }
}

/// Apply already-resolved VM data writes to component account memory. Semantic
/// fields and offsets are never decoded here; the retained Effect owns them.
fn apply_replay_writes(writes: &[ResolvedEffectV3], root: &mut [u8], ticket: &mut [u8]) {
    for write in writes {
        let mut bytes = [0_u8; 32];
        let (account, offset, width) = match *write {
            ResolvedEffectV3::WriteScalar {
                account,
                offset,
                value,
            } => {
                bytes[..8].copy_from_slice(&value.to_le_bytes());
                (account, offset, 8)
            }
            ResolvedEffectV3::WriteIdentity {
                account,
                offset,
                value,
            } => {
                bytes = value;
                (account, offset, 32)
            }
            ResolvedEffectV3::WriteU8 {
                account,
                offset,
                value,
            } => {
                bytes[0] = value;
                (account, offset, 1)
            }
            ResolvedEffectV3::WriteU16 {
                account,
                offset,
                value,
            } => {
                bytes[..2].copy_from_slice(&value.to_le_bytes());
                (account, offset, 2)
            }
            ResolvedEffectV3::WriteU32 {
                account,
                offset,
                value,
            } => {
                bytes[..4].copy_from_slice(&value.to_le_bytes());
                (account, offset, 4)
            }
            _ => continue,
        };
        let destination = match account {
            0 => &mut *root,
            5 => &mut *ticket,
            _ => panic!("unexpected replay writer coordinate {account}"),
        };
        let offset = usize::try_from(offset).expect("resolved offset");
        destination
            .get_mut(offset..offset + width)
            .expect("resolved native data range")
            .copy_from_slice(&bytes[..width]);
    }
}

fn native_requests(requests: SeriesPrepareChildRequestsV4<'_>) -> Vec<u8> {
    [
        requests.projected_initialize.as_slice(),
        requests.projected_open.as_slice(),
        requests.replay_initialize.as_slice(),
        requests.escrow_open.as_slice(),
        requests.escrow_lock.as_slice(),
    ]
    .concat()
}

#[test]
fn same_bundle_native_consume_then_expire_preserves_all_replay_poststates() {
    let (prepared, plan) = prepared_founder_with_plan();
    let root = Pubkey::new_unique();
    let first_input = compiler_input_with_plan_at_occurrence(
        &prepared,
        &plan,
        root,
        &prepared.admitted.tickets()[0],
        0,
    );
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
    let prepare_projection = projected_requests(
        &compiled.selected,
        &compiled.parents.prepare_request,
        root,
        &compiled.derived_prepare,
    );
    assert_eq!(
        prepare_projection.requests,
        native_requests(compiled.prepare_children.prepare_requests())
    );
    let header = super::tests::compiled_root_header(&prepared, &compiled).to_bytes();
    let mut root_bytes = [
        header.as_slice(),
        first_state
            .encode(template.occurrence_count())
            .expect("native initial state")
            .as_slice(),
    ]
    .concat();
    let mut ticket_bytes = vec![0; dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3];
    apply_replay_writes(
        &prepare_projection.writes,
        &mut root_bytes,
        &mut ticket_bytes,
    );
    let expected_prepared = first_state
        .prepare_ticket(first_state.revision())
        .expect("native Prepare successor");
    assert_eq!(
        &root_bytes[..header.len()],
        header.as_slice(),
        "immutable header preserved"
    );
    assert_eq!(
        &root_bytes[header.len()..],
        expected_prepared
            .encode(template.occurrence_count())
            .expect("native prepared state")
    );
    let ticket = dclutch_trading::series::admit_ticket(&prepared.admitted.tickets()[0])
        .expect("first Ticket");
    assert_eq!(
        ticket_bytes,
        dclutch_trading::series::replay::TicketStateV3::prepared(ticket.content_id()).encode()
    );

    let consume = project_action(
        &compiled.selected,
        "consume",
        &compiled.consume_request,
        &[1],
        |scalars, _| {
            use dclutch_trading_sbf::series::consume_artifacts_v4::SERIES_CONSUME_DERIVED_REQUEST_SCALAR_START_V1;
            scalars[usize::from(
                dclutch_trading_sbf::series::effect_v4::SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4,
            )] = 1;
            compiled
                .derived_consume
                .write_scalar_words(
                    &compiled.consume_request,
                    root.to_bytes(),
                    &mut scalars[usize::from(SERIES_CONSUME_DERIVED_REQUEST_SCALAR_START_V1)..],
                )
                .expect("current native Consume words");
        },
    );
    let native = compiled.prepare_children.consume_requests();
    let claims = dclutch_claims::series_founding_transport_v1::SeriesClaimsFoundingTransportV1::from_canonical_v5(
        compiled.physical.permit.to_bytes(), dclutch_claims::founding_v5::ClaimsFoundingRequestV5::decode(native.claims)
            .expect("native Claims request")).expect("native Claims transport").to_bytes();
    assert_eq!(
        consume.requests,
        [
            native.lock.as_slice(),
            native.core.as_slice(),
            native.realize.as_slice(),
            claims.as_slice(),
            native.core.as_slice()
        ]
        .concat()
    );
    let lock = ProjectedCustodyRequestV1::decode(&consume.requests[..native.lock.len()])
        .expect("emitted Lock");
    let (locked, lock_receipt) = compiled
        .prepared_projected_state
        .lock_hoard_and_close_source(
            lock,
            hash(&consume.requests[..native.lock.len()]).to_bytes(),
            compiled.physical.normal_replay.to_bytes(),
            compiled.prepared_source_replay,
            lock.amount,
            0,
            0,
            lock.amount,
            compiled.physical.projected.escrow_vault_rent_lamports,
            compiled.physical.projected.escrow_replay_rent_lamports,
            compiled.physical.projected.rent_credit,
            true,
        )
        .expect("native Lock accepts materialized request");
    assert_eq!(lock_receipt.amount, lock.amount);
    assert_eq!(locked.locked_amount, lock.amount);
    let realize_start = native.lock.len() + native.core.len();
    let realize_bytes = &consume.requests[realize_start..realize_start + native.realize.len()];
    let realize = ProjectedCustodyRequestV1::decode(realize_bytes).expect("emitted Realize");
    let realization = locked
        .realize_and_close_ref(
            &realize,
            hash(realize_bytes).to_bytes(),
            &compiled.predicted_core,
            hash(&compiled.predicted_core.encode().expect("native Core state")).to_bytes(),
            lock.amount,
            compiled.physical.projected.rent_credit,
        )
        .expect("native Realize accepts materialized request");
    let normal = dclutch_custody::normal_replay_from_realization_v1(
        locked,
        realization,
        hash(&realization.encode().expect("native Realize receipt")).to_bytes(),
    )
    .expect("realized projected-state account becomes the normal Hoard replay");
    assert_eq!(normal.market, first_native.market);
    assert_eq!(normal.open_vault_count, 1);
    assert_eq!(normal.next_revision, 1);
    assert_ne!(
        compiled.physical.realized_hoard_replay,
        compiled.physical.normal_replay
    );

    let next_state = first_state
        .prepare_ticket(first_state.revision())
        .expect("Prepare transition");
    let next_state = next_state
        .settle_current(next_state.revision(), template.occurrence_count())
        .expect("settlement transition");
    assert_eq!(next_state.next_occurrence(), 1);
    let mut second_input = compiler_input_with_plan_at_occurrence(
        &prepared,
        &plan,
        root,
        &prepared.admitted.tickets()[1],
        1,
    );
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
    let independently_derived = compile_series_found_prepare_selection_v1(
        second_input,
        ContentId::new([62; 32]).expect("same certificate identity"),
    )
    .expect("same source compiler admits occurrence one");
    super::require_series_selection_invariance_v1(&compiled, &independently_derived)
        .expect("all five action artifacts are byte-identical across occurrences");
    let actual = projected_requests(
        &compiled.selected,
        &second.parents.prepare_request,
        root,
        &second.derived_prepare,
    );
    assert_eq!(
        actual.requests,
        native_requests(second.prepare_children.prepare_requests()),
        "the retained Effect emits every native occurrence-one request byte"
    );
    let actual_first = ProjectedCustodyRequestV1::decode(
        &actual.requests[..dclutch_custody::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    )
    .expect("materialized native Initialize request");
    assert_eq!(actual_first, second_native);
    assert_ne!(actual_first.context_digest, first_native.context_digest);
    let mut hostile_output = vec![u64::MAX; SERIES_PREPARE_DERIVED_REQUEST_WORD_COUNT_V1];
    assert_eq!(
        compiled.derived_prepare.write_scalar_words(
            &second.parents.prepare_request,
            root.to_bytes(),
            &mut hostile_output
        ),
        Err(SeriesPrepareDerivedRequestErrorV1::FamilyRequest)
    );
    assert!(
        hostile_output.iter().all(|word| *word == u64::MAX),
        "refusal leaves the destination unchanged"
    );
    use dclutch_trading::series::replay::{TicketPhaseV3, TicketStateV3};
    use dclutch_trading_sbf::series::expire_funding_artifacts_v5::*;
    let after_second_prepare = next_state
        .prepare_ticket(next_state.revision())
        .expect("native second Prepare");
    let second_ticket = dclutch_trading::series::admit_ticket(&prepared.admitted.tickets()[1])
        .expect("second Ticket");
    let prepared_ticket = TicketStateV3::prepared(second_ticket.content_id());
    let mut second_root_bytes = [
        header.as_slice(),
        next_state
            .encode(template.occurrence_count())
            .expect("native occurrence-one root")
            .as_slice(),
    ]
    .concat();
    let mut second_ticket_bytes =
        vec![0; dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3];
    apply_replay_writes(
        &actual.writes,
        &mut second_root_bytes,
        &mut second_ticket_bytes,
    );
    assert_eq!(&second_root_bytes[..header.len()], header.as_slice());
    assert_eq!(
        &second_root_bytes[header.len()..],
        after_second_prepare
            .encode(template.occurrence_count())
            .expect("native second prepared root")
    );
    assert_eq!(second_ticket_bytes, prepared_ticket.encode());
    let expiry = project_action(
        &compiled.selected,
        "expire",
        &second.parents.expire_request,
        &[],
        |scalars, identities| {
            second
                .derived_expire
                .write_scalar_words(
                    &second.parents.expire_request,
                    root.to_bytes(),
                    &mut scalars[usize::from(SERIES_EXPIRE_DERIVED_REQUEST_SCALAR_START_V1)..],
                )
                .expect("current native Expire words");
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_ROOT_REVISION_SCALAR_V5)] =
                after_second_prepare.revision();
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_TICKET_REVISION_SCALAR_V5)] =
                prepared_ticket.revision();
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_NEXT_OCCURRENCE_SCALAR_V5)] =
                u64::from(after_second_prepare.next_occurrence());
            scalars[usize::from(SERIES_EXPIRE_OCCURRENCE_COUNT_SCALAR_V5)] =
                u64::from(template.occurrence_count());
            scalars[usize::from(SERIES_EXPIRE_OCCURRENCE_INDEX_SCALAR_V5)] =
                u64::from(second_ticket.ticket().occurrence());
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_ROOT_PHASE_SCALAR_V5)] =
                after_second_prepare.phase() as u64;
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_PREPARED_SCALAR_V5)] = 1;
            scalars[usize::from(SERIES_EXPIRE_OBSERVED_TICKET_PHASE_SCALAR_V5)] =
                prepared_ticket.phase() as u64;
            identities[usize::from(SERIES_EXPIRE_OBSERVED_TICKET_IDENTITY_V5)] =
                second_ticket.content_id().to_bytes();
            identities[usize::from(SERIES_EXPIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5)] =
                template.refund_owner().to_bytes();
            identities[usize::from(SERIES_EXPIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5)] =
                template.refund_owner().to_bytes();
        },
    );
    let native_expire = second.prepare_children.expire_requests();
    let permit = dclutch_market::SeriesUnallocatedPermitExpiryRequestV1::new(
        after_second_prepare.revision(),
        prepared_ticket.revision(),
    )
    .encode();
    assert_eq!(
        expiry.requests,
        [
            native_expire.refund.as_slice(),
            native_expire.close_vault.as_slice(),
            native_expire.close_replay.as_slice(),
            native_expire.projected_abort.as_slice(),
            permit.as_slice()
        ]
        .concat()
    );
    let abort_start = native_expire.refund.len()
        + native_expire.close_vault.len()
        + native_expire.close_replay.len();
    let abort_bytes =
        &expiry.requests[abort_start..abort_start + native_expire.projected_abort.len()];
    let abort = ProjectedCustodyRequestV1::decode(abort_bytes).expect("materialized second Abort");
    let aborted = second
        .prepared_projected_state
        .abort_open_and_close(
            abort,
            hash(abort_bytes).to_bytes(),
            template.retry_through(1).expect("retry") + 1,
            0,
            second.physical().projected.rent_credit,
            true,
        )
        .expect("native second projected state accepts materialized expiry cleanup");
    assert!(aborted.aborted_open);
    assert_eq!(aborted.market, second_native.market);
    let settled = after_second_prepare
        .settle_current(after_second_prepare.revision(), template.occurrence_count())
        .expect("native second expiry settles root");
    let expired_ticket = prepared_ticket
        .settle(prepared_ticket.revision(), TicketPhaseV3::Expired)
        .expect("native second Ticket expires");
    assert_eq!(
        expiry.scalars[usize::from(SERIES_EXPIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5)],
        settled.revision()
    );
    assert_eq!(
        expiry.scalars[usize::from(SERIES_EXPIRE_CANDIDATE_NEXT_OCCURRENCE_SCALAR_V5)],
        u64::from(settled.next_occurrence())
    );
    assert_eq!(
        expiry.scalars[usize::from(SERIES_EXPIRE_CANDIDATE_ROOT_PHASE_SCALAR_V5)],
        settled.phase() as u64
    );
    assert_eq!(
        expiry.scalars[usize::from(SERIES_EXPIRE_CANDIDATE_TICKET_REVISION_SCALAR_V5)],
        expired_ticket.revision()
    );
    apply_replay_writes(
        &expiry.writes,
        &mut second_root_bytes,
        &mut second_ticket_bytes,
    );
    assert_eq!(&second_root_bytes[..header.len()], header.as_slice());
    assert_eq!(
        &second_root_bytes[header.len()..],
        settled
            .encode(template.occurrence_count())
            .expect("native terminal root")
    );
    assert_eq!(second_ticket_bytes, expired_ticket.encode());
    use dclutch_market::rent::lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2};
    use dclutch_trading_sbf::series::operator::{
        SeriesCloseSnapshotV3, SeriesRetireSnapshotV3, build_close_v3, build_retire_v3,
    };
    let rent_credit = LifecycleRentCreditV2::new(
        dclutch_market::rent::RefundAuthority::new(template.refund_owner().to_bytes())
            .expect("refund authority"),
        LifecycleAccountIdV2::new(first_native.market).expect("root Market"),
        LifecycleAccountIdV2::new(template.release_set().to_bytes()).expect("root release"),
        1,
        1,
    )
    .expect("typed lifecycle credit fixture");
    let rent_sink = dclutch_trading::series::terminal::SeriesLifecycleRentSinkV3::admit(
        dclutch_trading::series::AccountKeyV3::new(compiled.physical.projected.rent_credit)
            .expect("credit key"),
        &rent_credit.to_bytes(),
        dclutch_trading::series::AccountKeyV3::new(first_native.market).expect("root Market"),
        template.release_set(),
        1,
        template.refund_owner(),
    )
    .expect("native lifecycle credit binding");
    let first_consumed_ticket =
        dclutch_trading::series::replay::TicketStateV3::prepared(ticket.content_id())
            .settle(0, TicketPhaseV3::Consumed)
            .expect("native first Ticket consumption");
    let mut terminal = settled;
    assert_eq!(
        terminal.outstanding_ticket_accounts(),
        2,
        "both Tickets remain until the Series is terminal"
    );
    for (record, ticket_state) in [
        (&prepared.admitted.tickets()[0], first_consumed_ticket),
        (&prepared.admitted.tickets()[1], expired_ticket),
    ] {
        let retire = build_retire_v3(SeriesRetireSnapshotV3 {
            template_bytes: prepared.admitted.template(),
            ticket_bytes: record,
            series: terminal,
            ticket_state,
            observed_ticket_lamports: 1,
            exact_ticket_rent: 1,
            rent_sink,
        })
        .expect("native Retire action admits terminal Series only");
        let projection = project_action(
            &compiled.selected,
            "retire",
            retire.as_bytes(),
            &[],
            |scalars, identities| {
                use dclutch_trading_sbf::series::retire_funding_artifacts_v5::*;
                scalars[usize::from(SERIES_RETIRE_OBSERVED_ROOT_REVISION_SCALAR_V5)] =
                    terminal.revision();
                scalars[usize::from(SERIES_RETIRE_OBSERVED_TICKET_REVISION_SCALAR_V5)] =
                    ticket_state.revision();
                scalars[usize::from(SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5)] =
                    u64::from(terminal.outstanding_ticket_accounts());
                scalars[usize::from(SERIES_RETIRE_TICKET_LAMPORTS_SCALAR_V5)] = 1;
                scalars[usize::from(SERIES_RETIRE_ROOT_PHASE_SCALAR_V5)] = terminal.phase() as u64;
                scalars[usize::from(SERIES_RETIRE_TICKET_PHASE_SCALAR_V5)] =
                    ticket_state.phase() as u64;
                identities[usize::from(SERIES_RETIRE_OBSERVED_TICKET_IDENTITY_V5)] =
                    ticket_state.ticket_record_id().to_bytes();
                identities[usize::from(SERIES_RETIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5)] =
                    template.refund_owner().to_bytes();
                identities[usize::from(SERIES_RETIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5)] =
                    template.refund_owner().to_bytes();
            },
        );
        terminal = terminal
            .retire_ticket(terminal.revision())
            .expect("native Ticket retirement");
        apply_replay_writes(
            &projection.writes,
            &mut second_root_bytes,
            &mut second_ticket_bytes,
        );
        assert_eq!(&second_root_bytes[..header.len()], header.as_slice());
        assert_eq!(
            &second_root_bytes[header.len()..],
            terminal
                .encode(template.occurrence_count())
                .expect("native retired root")
        );
        assert!(projection.requests.is_empty());
    }
    terminal
        .admit_close(terminal.revision())
        .expect("native terminal root closes after both Ticket retirements");
    let close = build_close_v3(SeriesCloseSnapshotV3 {
        template_bytes: prepared.admitted.template(),
        series: terminal,
        observed_root_lamports: 1 + terminal.close_rent_remaining(),
        exact_root_rent: 1,
        rent_sink,
    })
    .expect("native Close action");
    let close_projection = project_action(
        &compiled.selected,
        "close",
        close.as_bytes(),
        &[],
        |scalars, identities| {
            use dclutch_trading_sbf::series::funding_artifacts_v5::*;
            scalars[usize::from(SERIES_CLOSE_OBSERVED_ROOT_REVISION_SCALAR_V5)] =
                terminal.revision();
            identities[usize::from(SERIES_CLOSE_TEMPLATE_REFUND_OWNER_IDENTITY_V5)] =
                template.refund_owner().to_bytes();
            identities[usize::from(dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CLOSE_BENEFICIARY_IDENTITY_V5)] = template.refund_owner().to_bytes();
        },
    );
    assert!(close_projection.requests.is_empty());
    use dclutch_trading_sbf::series::derived_terminal_v1::SeriesTerminalDerivedRequestErrorV1;
    let mut stale_consume = vec![u64::MAX; compiled.derived_consume.request_bytes().len() / 8];
    assert_eq!(
        compiled.derived_consume.write_scalar_words(
            &second.consume_request,
            root.to_bytes(),
            &mut stale_consume
        ),
        Err(SeriesTerminalDerivedRequestErrorV1::FamilyRequest)
    );
    assert_eq!(
        compiled.derived_consume.write_scalar_words(
            &compiled.consume_request,
            Pubkey::new_unique().to_bytes(),
            &mut stale_consume
        ),
        Err(SeriesTerminalDerivedRequestErrorV1::Physical)
    );
    assert!(stale_consume.iter().all(|word| *word == u64::MAX));
    let mut stale_expire = vec![u64::MAX; compiled.derived_expire.request_bytes().len() / 8];
    assert_eq!(
        compiled.derived_expire.write_scalar_words(
            &second.parents.expire_request,
            root.to_bytes(),
            &mut stale_expire
        ),
        Err(SeriesTerminalDerivedRequestErrorV1::FamilyRequest)
    );
    assert_eq!(
        compiled.derived_expire.write_scalar_words(
            &compiled.parents.expire_request,
            Pubkey::new_unique().to_bytes(),
            &mut stale_expire
        ),
        Err(SeriesTerminalDerivedRequestErrorV1::Physical)
    );
    assert!(stale_expire.iter().all(|word| *word == u64::MAX));
}
