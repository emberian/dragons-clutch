use core::cell::Cell;

use dclutch_market_core_codec::{
    Identity as CoreIdentity, SeriesCoreAckV1, SeriesCoreActionV1, SeriesCoreRequestV1,
};
use dclutch_series_codec::{
    Action, Error as SemanticError, InvocationV1, Limits, Phase, ReleaseReceiptV1, RequestV1,
    SeriesStateV1, TemplateV1, TicketFundsV1, TicketPhase, TicketV1, interpret,
};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use super::{
    BOOTSTRAP_BYTES_V1, SERIES_PDA_DOMAIN_V1, SeriesSbfError, TEMPLATE_PDA_DOMAIN_V1,
    TICKET_PDA_DOMAIN_V1, prepare_core_call, process_instruction, required_top_up,
    stage_and_encode, validate_initial, validate_template_schedule, validate_vacant_state_account,
};

fn id(value: u8) -> [u8; 32] {
    let mut output = [0; 32];
    output[0] = value;
    output
}

fn core_id(value: u8) -> CoreIdentity {
    CoreIdentity::new(id(value)).expect("nonzero fixture identity")
}

fn fixture() -> InvocationV1 {
    let template = TemplateV1 {
        template_id: id(1),
        realm_id: id(2),
        product_id: id(3),
        release_set_id: id(4),
        series_refund_owner: id(5),
        outcome_count: 3,
        occurrence_count: 3,
        first_occurrence_slot: 100,
        period_slots: 10,
        retry_window_slots: 5,
        seed_quantity: 10,
        market_rent_lamports: 20,
        capability_rent_lamports: 0,
        founding_work_lamports: 5,
        series_close_rent_lamports: 7,
    };
    let series = SeriesStateV1 {
        series_id: id(6),
        template_id: template.template_id,
        phase: Phase::Active,
        next_occurrence: 0,
        revision: 0,
        close_rent_lamports: 7,
    };
    let ticket = TicketV1 {
        ticket_id: id(7),
        template_id: template.template_id,
        founder: id(8),
        refund_owner: id(9),
        committed_market_id: id(10),
        occurrence: 0,
        revision: 0,
        phase: TicketPhase::Ready,
        funds: TicketFundsV1 {
            hoard_principal: 10,
            market_rent: 20,
            capability_rent: 0,
            founding_work: 5,
        },
    };
    InvocationV1 {
        template,
        series,
        ticket,
        release_receipt: ReleaseReceiptV1 {
            registry_program: id(11),
            release_set_id: template.release_set_id,
            observed_program: id(11),
            artifact_release: id(12),
            semantic_release: id(13),
        },
        request: RequestV1 {
            action: Action::Consume,
            now_slot: 100,
            expected_series_revision: 0,
            expected_ticket_revision: 0,
            work_recipient: id(14),
        },
        limits: Limits {
            slot_limit: 1000,
            lamport_limit: 10_000,
            revision_limit: 10_000,
        },
    }
}

#[test]
fn bootstrap_is_exact_concatenation_of_lean_owned_wires() {
    assert_eq!(
        BOOTSTRAP_BYTES_V1,
        dclutch_series_codec::TEMPLATE_BYTES
            + dclutch_series_codec::SERIES_STATE_BYTES
            + dclutch_series_codec::TICKET_BYTES
    );
    assert_eq!(BOOTSTRAP_BYTES_V1, 552);
}

#[test]
fn unknown_width_refuses_before_account_or_registry_reads() {
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &[0; 63]),
        Err(ProgramError::Custom(SeriesSbfError::Instruction as u32))
    );
}

#[test]
fn pda_domains_separate_template_series_and_each_occurrence() {
    let program = Pubkey::new_unique();
    let template_id = id(1);
    let template =
        Pubkey::find_program_address(&[TEMPLATE_PDA_DOMAIN_V1, &template_id], &program).0;
    let series = Pubkey::find_program_address(&[SERIES_PDA_DOMAIN_V1, &template_id], &program).0;
    let zero = 0_u32.to_le_bytes();
    let one = 1_u32.to_le_bytes();
    let ticket0 =
        Pubkey::find_program_address(&[TICKET_PDA_DOMAIN_V1, &template_id, &zero], &program).0;
    let ticket1 =
        Pubkey::find_program_address(&[TICKET_PDA_DOMAIN_V1, &template_id, &one], &program).0;
    assert_ne!(template, series);
    assert_ne!(series, ticket0);
    assert_ne!(ticket0, ticket1);
}

#[test]
fn prefunded_vacant_pda_is_not_dust_dosable() {
    assert_eq!(required_top_up(0, 100), 100);
    assert_eq!(required_top_up(99, 100), 1);
    assert_eq!(required_top_up(100, 100), 0);
    assert_eq!(required_top_up(101, 100), 0);

    let key = Pubkey::new_unique();
    let mut lamports = 101;
    let mut data = [];
    let account = AccountInfo::new(
        &key,
        false,
        true,
        &mut lamports,
        &mut data,
        &system_program::ID,
        false,
    );
    assert_eq!(validate_vacant_state_account(&account), Ok(()));
}

#[test]
fn occupied_or_host_program_owned_pda_refuses_as_vacant() {
    let key = Pubkey::new_unique();
    let hostile_owner = Pubkey::new_unique();
    let mut lamports = 1;
    let mut data = [];
    let account = AccountInfo::new(
        &key,
        false,
        true,
        &mut lamports,
        &mut data,
        &hostile_owner,
        false,
    );
    assert_eq!(
        validate_vacant_state_account(&account),
        Err(ProgramError::Custom(SeriesSbfError::AccountIdentity as u32))
    );
}

#[test]
fn initial_projection_refuses_wrong_compartment_or_revision() {
    let input = fixture();
    assert_eq!(
        validate_initial(input.template, input.series, input.ticket),
        Ok(())
    );
    let mut under = input.ticket;
    under.funds.hoard_principal = 9;
    assert_eq!(
        validate_initial(input.template, input.series, under),
        Err(ProgramError::Custom(SeriesSbfError::Semantic as u32))
    );
    let mut stale = input.series;
    stale.revision = 1;
    assert_eq!(
        validate_initial(input.template, stale, input.ticket),
        Err(ProgramError::Custom(SeriesSbfError::Semantic as u32))
    );
}

#[test]
fn physical_schedule_overflow_refuses_before_account_creation() {
    let input = fixture();
    assert_eq!(validate_template_schedule(input.template), Ok(()));
    assert_eq!(
        validate_template_schedule(TemplateV1 {
            first_occurrence_slot: u64::MAX - 2,
            ..input.template
        }),
        Err(ProgramError::Custom(SeriesSbfError::Semantic as u32))
    );
    assert_eq!(
        validate_template_schedule(TemplateV1 {
            occurrence_count: 0,
            ..input.template
        }),
        Err(ProgramError::Custom(SeriesSbfError::Semantic as u32))
    );
    assert_eq!(
        validate_template_schedule(TemplateV1 {
            occurrence_count: 3,
            first_occurrence_slot: 1,
            period_slots: u64::MAX,
            retry_window_slots: 0,
            ..input.template
        }),
        Err(ProgramError::Custom(SeriesSbfError::Semantic as u32))
    );
}

#[test]
fn canonical_core_request_binds_all_ticket_compartments_and_zero_rent() {
    let input = fixture();
    let program = Pubkey::new_unique();
    let call = prepare_core_call(
        &program,
        input.template,
        input.series,
        input.ticket,
        SeriesCoreActionV1::Prepare,
        input.ticket.refund_owner,
    )
    .expect("canonical Core call");
    let request = SeriesCoreRequestV1::decode(&call.request_bytes).expect("Core request");
    assert_eq!(request.action(), SeriesCoreActionV1::Prepare);
    assert_eq!(request.hoard_principal(), 10);
    assert_eq!(request.market_rent(), 20);
    assert_eq!(request.capability_rent(), 0);
    assert_eq!(request.work(), 5);
    assert_eq!(call.request.action(), SeriesCoreActionV1::Prepare);
    assert_eq!(call.request.market_generation(), Some(1));
    assert_eq!(
        call.request_digest.to_bytes(),
        solana_program::hash::hash(&call.request_bytes).to_bytes()
    );
    let consume = prepare_core_call(
        &program,
        input.template,
        input.series,
        input.ticket,
        SeriesCoreActionV1::Consume,
        input.request.work_recipient,
    )
    .expect("canonical consume call");
    assert_ne!(call.caller_authority, consume.caller_authority);
}

#[test]
fn canonical_core_ack_binds_exact_request_and_current_program() {
    let input = fixture();
    let call = prepare_core_call(
        &Pubkey::new_unique(),
        input.template,
        input.series,
        input.ticket,
        SeriesCoreActionV1::Consume,
        input.request.work_recipient,
    );
    let call = call.expect("canonical Core call");
    let core_program = core_id(20);
    let ack = SeriesCoreAckV1::new(call.request, core_program, call.request_digest, core_id(21));
    let encoded = ack.encode().expect("ack encodes");
    assert_eq!(SeriesCoreAckV1::decode(&encoded), Ok(ack));
    let mut hostile = encoded;
    hostile[11] = 1;
    assert!(SeriesCoreAckV1::decode(&hostile).is_err());
    assert_eq!(
        ack.validate_for(call.request, core_program, call.request_digest, core_id(21),),
        Ok(())
    );
    assert!(
        ack.validate_for(call.request, core_program, call.request_digest, core_id(22),)
            .is_err()
    );
}

#[test]
fn stage_failure_exposes_no_candidate_bytes_for_commit() {
    let input = fixture();
    let candidate = interpret(input).expect("semantic candidate");
    let called = Cell::new(false);
    let result = stage_and_encode(candidate, || {
        called.set(true);
        Err(ProgramError::Custom(SeriesSbfError::RoleCpi as u32))
    });
    assert!(called.get());
    assert_eq!(
        result,
        Err(ProgramError::Custom(SeriesSbfError::RoleCpi as u32))
    );
}

#[test]
fn successful_staging_encodes_only_the_exact_semantic_candidate() {
    let input = fixture();
    let candidate = interpret(input).expect("semantic candidate");
    let (series, ticket) = stage_and_encode(candidate, || Ok(())).expect("staged");
    assert_eq!(
        SeriesStateV1::decode(&series).expect("post Series"),
        candidate.series
    );
    assert_eq!(
        TicketV1::decode(&ticket).expect("post Ticket"),
        candidate.ticket
    );
}

#[test]
fn semantic_release_substitution_and_replay_stay_refusals() {
    let input = fixture();
    let mut receipt = input.release_receipt;
    receipt.release_set_id[0] ^= 1;
    assert_eq!(
        interpret(InvocationV1 {
            release_receipt: receipt,
            ..input
        }),
        Err(SemanticError::ReleaseAdmission)
    );
    assert_eq!(
        interpret(InvocationV1 {
            request: RequestV1 {
                expected_series_revision: 1,
                ..input.request
            },
            ..input
        }),
        Err(SemanticError::RevisionMismatch)
    );
}
