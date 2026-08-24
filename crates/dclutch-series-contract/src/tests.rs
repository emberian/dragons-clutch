use super::*;

fn id(value: u8) -> IdentityV1 {
    IdentityV1::new([value; IDENTITY_BYTES]).expect("nonzero identity")
}

fn vacant(lamports: u64) -> VacantAccountFactsV1 {
    VacantAccountFactsV1 {
        lamports,
        owner: SYSTEM_PROGRAM_ID,
        data_len: 0,
        is_executable: false,
    }
}

fn recipe() -> SeriesRecipeV1 {
    SeriesRecipeV1 {
        realm_id: id(1),
        terms_id: id(2),
        claim_basis_id: id(3),
        capacity_profile_id: id(4),
        compiler_release_id: id(5),
        occurrence_schedule_id: id(6),
        source_schedule_id: id(7),
        capability_template_id: id(8),
        occurrence_derivation_release_id: id(9),
        source_derivation_release_id: id(10),
        capability_derivation_release_id: id(11),
        market_derivation_release_id: id(12),
        capitalization_schedule_id: id(13),
        first_occurrence_time: 1_800_000_000,
        cadence_seconds: 3_600,
        occurrence_count: 3,
        first_generation: 90,
        outcome_count: 4,
    }
}

fn aggregate(recipe_id: IdentityV1) -> CapitalizationAggregateV1 {
    CapitalizationAggregateV1 {
        recipe_id,
        capitalization_schedule_id: recipe().capitalization_schedule_id,
        occurrence_count: 3,
        total_principal: 60,
    }
}

fn capitalization(recipe_id: IdentityV1, index: u64) -> OccurrenceCapitalizationV1 {
    OccurrenceCapitalizationV1 {
        recipe_id,
        capitalization_schedule_id: recipe().capitalization_schedule_id,
        occurrence_index: index,
        market_principal: 15,
        ticket_rent: 5,
        total_principal: 20,
    }
}

fn derived(recipe_id: IdentityV1, index: u64) -> DerivedOccurrenceV1 {
    let recipe = recipe();
    let index_byte = u8::try_from(index).expect("small fixture index");
    DerivedOccurrenceV1 {
        recipe_id,
        occurrence_index: index,
        occurrence_time: recipe.time_at(index).expect("fixture time"),
        generation: recipe.generation_at(index).expect("fixture generation"),
        occurrence_artifact_id: id(20 + index_byte),
        occurrence_id: id(30 + index_byte),
        product_instance_id: id(40 + index_byte),
        source_spec_id: id(50 + index_byte),
        source_window_id: id(60 + index_byte),
        statistic_id: id(70 + index_byte),
        resolution_policy_id: id(80 + index_byte),
        capability_manifest_id: id(90 + index_byte),
        market_identity_id: id(100 + index_byte),
        capitalization_id: id(110 + index_byte),
    }
}

fn created() -> (
    IdentityV1,
    IdentityV1,
    IdentityV1,
    SeriesRootV1,
    SeriesEscrowV1,
    SeriesReplayGuardV1,
) {
    let root_address = id(200);
    let recipe_id = id(201);
    let aggregate_id = id(202);
    let plan = plan_create_series_v1(
        root_address,
        recipe_id,
        aggregate_id,
        &recipe(),
        &aggregate(recipe_id),
        CreateSeriesV1 {
            refund_authority: id(203),
            root_bump: 7,
            escrow_bump: 8,
            replay_guard_bump: 9,
        },
        1_000,
        vacant(0),
        vacant(0),
        vacant(0),
        10,
        20,
        10,
    )
    .expect("valid creation");
    assert_eq!(plan.payer_after, 900);
    assert_eq!(plan.root_after, 10);
    assert_eq!(plan.escrow_after, 80);
    assert_eq!(plan.replay_guard_after, 10);
    (
        root_address,
        recipe_id,
        aggregate_id,
        plan.root,
        plan.escrow,
        plan.replay_guard,
    )
}

fn instantiate(
    root: SeriesRootV1,
    root_address: IdentityV1,
    recipe_id: IdentityV1,
    aggregate_id: IdentityV1,
    escrow: SeriesEscrowV1,
    escrow_balance: u64,
) -> InstantiationPlanV1 {
    let index = root.next_occurrence_index;
    let occurrence = derived(recipe_id, index);
    let cap = capitalization(recipe_id, index);
    plan_instantiate_next_v1(
        root,
        root_address,
        escrow,
        recipe_id,
        &recipe(),
        aggregate_id,
        &aggregate(recipe_id),
        id(150 + u8::try_from(index).expect("small fixture index")),
        &occurrence,
        occurrence.capitalization_id,
        &cap,
        InstantiateNextV1 {
            expected_index: index,
            expected_time: occurrence.occurrence_time,
            ticket_bump: 9,
        },
        occurrence.occurrence_time,
        20,
        5,
        escrow_balance,
        0,
    )
    .expect("valid exact-next instantiation")
}

#[test]
fn all_persistent_preimages_round_trip_at_exact_widths() {
    let recipe_id = id(201);
    let aggregate_id = id(202);
    let recipe = recipe();
    assert_eq!(SeriesRecipeV1::decode(&recipe.to_bytes()), Ok(recipe));

    let derived_record = derived(recipe_id, 1);
    assert_eq!(
        DerivedOccurrenceV1::decode(&derived_record.to_bytes()),
        Ok(derived_record)
    );
    let aggregate = aggregate(recipe_id);
    assert_eq!(
        CapitalizationAggregateV1::decode(&aggregate.to_bytes()),
        Ok(aggregate)
    );
    let capitalization = capitalization(recipe_id, 1);
    assert_eq!(
        OccurrenceCapitalizationV1::decode(&capitalization.to_bytes()),
        Ok(capitalization)
    );

    let (root_address, _, _, root, escrow, replay_guard) = created();
    assert_eq!(SeriesRootV1::decode(&root.to_bytes()), Ok(root));
    assert_eq!(SeriesEscrowV1::decode(&escrow.to_bytes()), Ok(escrow));
    assert_eq!(
        SeriesReplayGuardV1::decode(&replay_guard.to_bytes()),
        Ok(replay_guard)
    );
    let plan = instantiate(root, root_address, recipe_id, aggregate_id, escrow, 80);
    assert_eq!(plan.derived_occurrence, derived(recipe_id, 0));
    assert_eq!(
        OccurrenceTicketV1::decode(&plan.ticket.to_bytes()),
        Ok(plan.ticket)
    );
}

#[test]
fn instructions_are_exact_and_reserved_bytes_are_hostile() {
    let create = CreateSeriesV1 {
        refund_authority: id(1),
        root_bump: 2,
        escrow_bump: 3,
        replay_guard_bump: 4,
    };
    assert_eq!(CreateSeriesV1::decode(&create.to_bytes()), Ok(create));
    assert_eq!(
        SeriesInstructionV1::decode(&create.to_bytes()),
        Ok(SeriesInstructionV1::CreateSeries(create))
    );
    let instantiate = InstantiateNextV1 {
        expected_index: 7,
        expected_time: -4,
        ticket_bump: 6,
    };
    assert_eq!(
        InstantiateNextV1::decode(&instantiate.to_bytes()),
        Ok(instantiate)
    );
    let consume = ConsumeTicketV1 { expected_index: 7 };
    assert_eq!(ConsumeTicketV1::decode(&consume.to_bytes()), Ok(consume));
    let close = CloseExhaustedV1 {
        expected_released_allocations: 3,
    };
    assert_eq!(CloseExhaustedV1::decode(&close.to_bytes()), Ok(close));

    let mut noncanonical = instantiate.to_bytes();
    *noncanonical.get_mut(39).expect("fixed offset") = 1;
    assert_eq!(
        InstantiateNextV1::decode(&noncanonical),
        Err(Error::NonCanonicalReservedBytes)
    );
    assert_eq!(
        InstantiateNextV1::decode(
            instantiate
                .to_bytes()
                .get(..39)
                .expect("fixed short instruction"),
        ),
        Err(Error::InvalidLength)
    );
}

#[test]
fn recipe_bounds_are_explicit_and_checked() {
    let mut invalid = recipe();
    invalid.outcome_count = 17;
    assert_eq!(invalid.validate(), Err(Error::UnsupportedOutcomeCount));
    invalid = recipe();
    invalid.cadence_seconds = 0;
    assert_eq!(invalid.validate(), Err(Error::InvalidCadence));
    invalid = recipe();
    invalid.occurrence_count = 0;
    assert_eq!(invalid.validate(), Err(Error::EmptySeries));
    invalid = recipe();
    invalid.first_occurrence_time = i64::MAX;
    assert_eq!(invalid.validate(), Err(Error::ScheduleOverflow));
    invalid = recipe();
    invalid.first_generation = u64::MAX;
    assert_eq!(invalid.validate(), Err(Error::GenerationOverflow));

    let mut full_clock_span = recipe();
    full_clock_span.first_occurrence_time = i64::MIN;
    full_clock_span.cadence_seconds = u64::MAX;
    full_clock_span.occurrence_count = 2;
    assert!(full_clock_span.validate().is_ok());
    assert_eq!(
        full_clock_span.time_at(1),
        Ok(i64::MAX),
        "cadence has no accidental i64 intermediate bound"
    );
}

#[test]
fn instantiation_is_gap_free_conservative_and_exhaustive() {
    let (root_address, recipe_id, aggregate_id, mut root, escrow, _guard) = created();
    let mut escrow_balance = 85;
    let mut tickets = [None, None, None];
    for slot in &mut tickets {
        let plan = instantiate(
            root,
            root_address,
            recipe_id,
            aggregate_id,
            escrow,
            escrow_balance,
        );
        escrow_balance -= 20;
        assert_eq!(plan.escrow_lamports_after, escrow_balance);
        assert_eq!(plan.ticket_lamports_after, 20);
        assert_eq!(
            plan.root_after.remaining_principal + plan.root_after.released_principal,
            60
        );
        *slot = Some(plan.ticket);
        root = plan.root_after;
    }
    assert_eq!(root.phase, SeriesPhaseV1::Exhausted);
    assert_eq!(root.remaining_allocations, 0);
    assert_eq!(root.released_allocations, 3);
    assert_eq!(root.outstanding_tickets, 3);
    assert_eq!(escrow_balance, 25, "five donated lamports remain untouched");
    assert_eq!(
        root.next_occurrence_time,
        recipe().time_at(2).expect("fixture occurrence exists")
    );

    let occurrence = derived(recipe_id, 2);
    assert_eq!(
        plan_instantiate_next_v1(
            root,
            root_address,
            escrow,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            id(152),
            &occurrence,
            occurrence.capitalization_id,
            &capitalization(recipe_id, 2),
            InstantiateNextV1 {
                expected_index: 3,
                expected_time: root.next_occurrence_time,
                ticket_bump: 9,
            },
            root.next_occurrence_time,
            20,
            5,
            20,
            0,
        ),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn stale_gap_time_derivation_and_funding_substitutions_refuse_without_mutation() {
    let (root_address, recipe_id, aggregate_id, root, escrow, _guard) = created();
    let occurrence = derived(recipe_id, 0);
    let cap = capitalization(recipe_id, 0);
    let call = |instruction, observed, derived_value: &DerivedOccurrenceV1, cap_value| {
        plan_instantiate_next_v1(
            root,
            root_address,
            escrow,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            id(150),
            derived_value,
            occurrence.capitalization_id,
            &cap_value,
            instruction,
            occurrence.occurrence_time,
            20,
            5,
            observed,
            0,
        )
    };
    assert_eq!(
        call(
            InstantiateNextV1 {
                expected_index: 1,
                expected_time: occurrence.occurrence_time,
                ticket_bump: 9,
            },
            80,
            &occurrence,
            cap,
        ),
        Err(Error::IndexMismatch)
    );
    assert_eq!(
        call(
            InstantiateNextV1 {
                expected_index: 0,
                expected_time: occurrence.occurrence_time + 1,
                ticket_bump: 9,
            },
            80,
            &occurrence,
            cap,
        ),
        Err(Error::TimeMismatch)
    );
    let mut wrong_derived = occurrence;
    wrong_derived.market_identity_id = id(222);
    wrong_derived.occurrence_index = 1;
    assert_eq!(
        call(
            InstantiateNextV1 {
                expected_index: 0,
                expected_time: occurrence.occurrence_time,
                ticket_bump: 9,
            },
            80,
            &wrong_derived,
            cap,
        ),
        Err(Error::DerivationMismatch)
    );
    assert_eq!(
        call(
            InstantiateNextV1 {
                expected_index: 0,
                expected_time: occurrence.occurrence_time,
                ticket_bump: 9,
            },
            79,
            &occurrence,
            cap,
        ),
        Err(Error::PresentPrincipalMismatch)
    );
    let too_large = OccurrenceCapitalizationV1 {
        market_principal: 60,
        ticket_rent: 1,
        total_principal: 61,
        ..cap
    };
    assert_eq!(
        call(
            InstantiateNextV1 {
                expected_index: 0,
                expected_time: occurrence.occurrence_time,
                ticket_bump: 9,
            },
            80,
            &occurrence,
            too_large,
        ),
        Err(Error::Underfunded)
    );
    assert_eq!(root.remaining_principal, 60);
    assert_eq!(root.next_occurrence_index, 0);
}

#[test]
fn successful_release_cannot_be_replayed_after_root_advances() {
    let (root_address, recipe_id, aggregate_id, root, escrow, _guard) = created();
    let first = instantiate(root, root_address, recipe_id, aggregate_id, escrow, 80);
    let occurrence = derived(recipe_id, 0);
    let cap = capitalization(recipe_id, 0);
    assert_eq!(
        plan_instantiate_next_v1(
            first.root_after,
            root_address,
            escrow,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            first.derived_occurrence_id,
            &occurrence,
            occurrence.capitalization_id,
            &cap,
            InstantiateNextV1 {
                expected_index: 0,
                expected_time: occurrence.occurrence_time,
                ticket_bump: first.ticket.pda_bump,
            },
            occurrence.occurrence_time,
            20,
            5,
            first.escrow_lamports_after,
            first.ticket_lamports_after,
        ),
        Err(Error::IndexMismatch)
    );
    assert_eq!(first.root_after.next_occurrence_index, 1);
    assert_eq!(first.root_after.remaining_principal, 40);
}

#[test]
fn ticket_consumption_routes_market_principal_and_all_surplus_to_rent_credit() {
    let (root_address, recipe_id, aggregate_id, root, escrow, _guard) = created();
    let first = instantiate(root, root_address, recipe_id, aggregate_id, escrow, 80);
    let occurrence = derived(recipe_id, 0);
    let capitalization = capitalization(recipe_id, 0);
    let wrong_ticket = OccurrenceTicketV1 {
        market_identity_id: id(250),
        ..first.ticket
    };
    assert_eq!(
        plan_consume_ticket_v1(
            first.root_after,
            root_address,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            first.ticket.derived_occurrence_id,
            &occurrence,
            occurrence.capitalization_id,
            &capitalization,
            wrong_ticket,
            ConsumeTicketV1 { expected_index: 0 },
            23,
            100,
        ),
        Err(Error::TicketMismatch)
    );
    assert_eq!(first.root_after.outstanding_tickets, 1);
    let plan = plan_consume_ticket_v1(
        first.root_after,
        root_address,
        recipe_id,
        &recipe(),
        aggregate_id,
        &aggregate(recipe_id),
        first.ticket.derived_occurrence_id,
        &occurrence,
        occurrence.capitalization_id,
        &capitalization,
        first.ticket,
        ConsumeTicketV1 { expected_index: 0 },
        23,
        100,
    )
    .expect("one-use ticket consumption");
    assert_eq!(plan.market_principal, 15);
    assert_eq!(plan.found_obligations.realm_id, recipe().realm_id);
    assert_eq!(
        plan.found_obligations.product_instance_id,
        occurrence.product_instance_id
    );
    assert_eq!(
        plan.found_obligations.capability_manifest_id,
        occurrence.capability_manifest_id
    );
    assert_eq!(
        plan.found_obligations.market_identity_id,
        occurrence.market_identity_id
    );
    assert_eq!(plan.found_obligations.generation, occurrence.generation);
    assert_eq!(plan.found_obligations.market_principal, 15);
    assert_eq!(plan.rent_credit_after, 108);
    assert_eq!(plan.ticket_lamports_after, 0);
    assert_eq!(plan.root_after.outstanding_tickets, 0);

    assert_eq!(
        plan_consume_ticket_v1(
            plan.root_after,
            root_address,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            first.ticket.derived_occurrence_id,
            &occurrence,
            occurrence.capitalization_id,
            &capitalization,
            first.ticket,
            ConsumeTicketV1 { expected_index: 0 },
            20,
            plan.rent_credit_after,
        ),
        Err(Error::OutstandingTickets)
    );
}

#[test]
fn close_requires_exhaustion_zero_principal_and_zero_tickets() {
    let (root_address, recipe_id, aggregate_id, mut root, escrow, replay_guard) = created();
    assert_eq!(
        plan_close_exhausted_v1(
            root,
            root_address,
            escrow,
            replay_guard,
            CloseExhaustedV1 {
                expected_released_allocations: 0,
            },
            10,
            80,
            10,
            10,
            100,
        ),
        Err(Error::SeriesNotExhausted)
    );
    let mut tickets = [None, None, None];
    let mut escrow_balance = 80;
    for slot in &mut tickets {
        let plan = instantiate(
            root,
            root_address,
            recipe_id,
            aggregate_id,
            escrow,
            escrow_balance,
        );
        escrow_balance = plan.escrow_lamports_after;
        root = plan.root_after;
        *slot = Some(plan.ticket);
    }
    assert_eq!(
        plan_close_exhausted_v1(
            root,
            root_address,
            escrow,
            replay_guard,
            CloseExhaustedV1 {
                expected_released_allocations: 3,
            },
            10,
            20,
            10,
            10,
            100,
        ),
        Err(Error::OutstandingTickets)
    );
    for (index, slot) in tickets.iter().enumerate() {
        let ticket = slot.expect("created ticket");
        let index = u64::try_from(index).expect("small fixture index");
        let occurrence = derived(recipe_id, index);
        root = plan_consume_ticket_v1(
            root,
            root_address,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            ticket.derived_occurrence_id,
            &occurrence,
            occurrence.capitalization_id,
            &capitalization(recipe_id, index),
            ticket,
            ConsumeTicketV1 {
                expected_index: index,
            },
            20,
            100,
        )
        .expect("consume each ticket")
        .root_after;
    }
    assert_eq!(
        plan_close_exhausted_v1(
            root,
            root_address,
            escrow,
            replay_guard,
            CloseExhaustedV1 {
                expected_released_allocations: 3,
            },
            10,
            20,
            9,
            10,
            100,
        ),
        Err(Error::ReplayGuardUnderfunded)
    );
    let close = plan_close_exhausted_v1(
        root,
        root_address,
        escrow,
        replay_guard,
        CloseExhaustedV1 {
            expected_released_allocations: 3,
        },
        10,
        20,
        12,
        10,
        100,
    )
    .expect("fully exhausted close");
    assert_eq!(close.rent_credit_after, 132);
    assert_eq!(close.root_lamports_after, 0);
    assert_eq!(close.escrow_lamports_after, 0);
    assert_eq!(close.replay_guard_lamports_after, 10);
    assert_eq!(replay_guard.plan_close(), Err(Error::PermanentReplayGuard));
}

#[test]
fn exact_frames_reject_aliasing_and_privilege_escalation() {
    let meta = |key, signer, writable, executable| AccountMetaV1 {
        key,
        is_signer: signer,
        is_writable: writable,
        is_executable: executable,
    };
    let accounts = [
        meta([1; 32], true, false, false),
        meta([2; 32], false, true, false),
        meta([3; 32], false, false, false),
        meta([4; 32], false, false, false),
        meta([5; 32], false, false, false),
        meta([6; 32], false, true, false),
        meta([7; 32], false, true, false),
        meta(SYSTEM_PROGRAM_ID, false, false, true),
        meta(RENT_SYSVAR_ID, false, false, false),
    ];
    assert!(InstantiateNextFrameV1::validate(&accounts).is_ok());
    let mut aliased = accounts;
    aliased.get_mut(6).expect("fixed role").key = [6; 32];
    assert_eq!(
        InstantiateNextFrameV1::validate(&aliased),
        Err(Error::AccountAlias)
    );
    let mut escalated = accounts;
    escalated.get_mut(2).expect("fixed role").is_writable = true;
    assert_eq!(
        InstantiateNextFrameV1::validate(&escalated),
        Err(Error::InvalidAccountPrivilege)
    );

    let create_accounts = [
        meta([11; 32], true, true, false),
        meta([12; 32], false, false, false),
        meta([13; 32], false, false, false),
        meta([14; 32], false, true, false),
        meta([15; 32], false, true, false),
        meta([16; 32], false, true, false),
        meta([17; 32], false, false, false),
        meta(SYSTEM_PROGRAM_ID, false, false, true),
        meta(RENT_SYSVAR_ID, false, false, false),
    ];
    assert!(CreateSeriesFrameV1::validate(&create_accounts).is_ok());

    let close_accounts = [
        meta([21; 32], true, false, false),
        meta([22; 32], false, true, false),
        meta([23; 32], false, true, false),
        meta([24; 32], false, true, false),
        meta([25; 32], false, true, false),
        meta(RENT_SYSVAR_ID, false, false, false),
    ];
    assert!(CloseExhaustedFrameV1::validate(&close_accounts).is_ok());
}

#[test]
fn pda_domains_obey_chain_seed_component_bound() {
    assert!(SERIES_ROOT_PDA_DOMAIN_V1.len() <= 32);
    assert!(SERIES_ESCROW_PDA_DOMAIN_V1.len() <= 32);
    assert!(SERIES_TICKET_PDA_DOMAIN_V1.len() <= 32);
    assert!(SERIES_REPLAY_GUARD_PDA_DOMAIN_V1.len() <= 32);

    let root = SeriesRootPdaPreimageV1 {
        recipe_id: id(1),
        aggregate_id: id(2),
        refund_authority: id(3),
        bump: 4,
    }
    .to_bytes();
    assert_eq!(root.get(..22), Some(SERIES_ROOT_PDA_DOMAIN_V1));
    assert_eq!(root.get(22..54), Some(id(1).to_bytes().as_slice()));
    assert_eq!(root.get(54..86), Some(id(2).to_bytes().as_slice()));
    assert_eq!(root.get(86..118), Some(id(3).to_bytes().as_slice()));
    assert_eq!(root.get(118), Some(&4));

    let ticket = OccurrenceTicketPdaPreimageV1 {
        series_root_address: id(9),
        occurrence_index: 0x0102_0304_0506_0708,
        bump: 10,
    }
    .to_bytes();
    assert_eq!(ticket.get(..24), Some(SERIES_TICKET_PDA_DOMAIN_V1));
    assert_eq!(
        ticket.get(56..64),
        Some(0x0102_0304_0506_0708_u64.to_le_bytes().as_slice())
    );
    assert_eq!(ticket.get(64), Some(&10));

    let guard = SeriesReplayGuardPdaPreimageV1 {
        series_root_address: id(9),
        bump: 11,
    }
    .to_bytes();
    assert_eq!(guard.get(..23), Some(SERIES_REPLAY_GUARD_PDA_DOMAIN_V1));
    assert_eq!(guard.get(23..55), Some(id(9).to_bytes().as_slice()));
    assert_eq!(guard.get(55), Some(&11));
}

#[test]
fn permanent_guard_blocks_root_resurrection_after_close() {
    let root_address = id(200);
    let recipe_id = id(201);
    let aggregate_id = id(202);
    let instruction = CreateSeriesV1 {
        refund_authority: id(203),
        root_bump: 7,
        escrow_bump: 8,
        replay_guard_bump: 9,
    };
    assert_eq!(
        plan_create_series_v1(
            root_address,
            recipe_id,
            aggregate_id,
            &recipe(),
            &aggregate(recipe_id),
            instruction,
            1_000,
            vacant(0),
            vacant(0),
            VacantAccountFactsV1 {
                lamports: 10,
                owner: [77; IDENTITY_BYTES],
                data_len: u64::try_from(SERIES_REPLAY_GUARD_BYTES_V1)
                    .expect("guard width fits u64"),
                is_executable: false,
            },
            10,
            20,
            10,
        ),
        Err(Error::AccountNotVacant)
    );
    let dusted = plan_create_series_v1(
        root_address,
        recipe_id,
        aggregate_id,
        &recipe(),
        &aggregate(recipe_id),
        instruction,
        1_000,
        vacant(12),
        vacant(85),
        vacant(14),
        10,
        20,
        10,
    )
    .expect("System-owned empty dusting is not occupation");
    assert_eq!(dusted.payer_after, 1_000);
    assert_eq!(dusted.root_after, 12);
    assert_eq!(dusted.escrow_after, 85);
    assert_eq!(dusted.replay_guard_after, 14);
    let guard = SeriesReplayGuardV1 {
        series_root_address: root_address,
        pda_bump: instruction.replay_guard_bump,
    };
    assert_eq!(guard.plan_close(), Err(Error::PermanentReplayGuard));
}

#[test]
fn future_release_and_insufficient_current_ticket_rent_refuse() {
    let (root_address, recipe_id, aggregate_id, root, escrow, _guard) = created();
    let occurrence = derived(recipe_id, 0);
    let cap = capitalization(recipe_id, 0);
    let instruction = InstantiateNextV1 {
        expected_index: 0,
        expected_time: occurrence.occurrence_time,
        ticket_bump: 9,
    };
    assert_eq!(
        plan_instantiate_next_v1(
            root,
            root_address,
            escrow,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            id(150),
            &occurrence,
            occurrence.capitalization_id,
            &cap,
            instruction,
            occurrence.occurrence_time - 1,
            20,
            5,
            80,
            0,
        ),
        Err(Error::OccurrenceNotDue)
    );
    assert_eq!(
        plan_instantiate_next_v1(
            root,
            root_address,
            escrow,
            recipe_id,
            &recipe(),
            aggregate_id,
            &aggregate(recipe_id),
            id(150),
            &occurrence,
            occurrence.capitalization_id,
            &cap,
            instruction,
            occurrence.occurrence_time,
            20,
            6,
            80,
            0,
        ),
        Err(Error::Underfunded)
    );
}
