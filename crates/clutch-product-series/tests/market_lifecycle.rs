use clutch_product_series::{
    ContentId, Error, FixedCodec, MarketFoundationAccountGraphV2, MarketFoundationScheduleV2,
    MarketFoundationSlotV2, MarketInstanceV2Id, SeriesFundingQuoteV4Id, SeriesFundingTermsV2Id,
    SeriesLinkObligationAdmissionProjectionV1, SeriesLinkObligationConfigurationV1,
    SeriesLinkObligationDispositionV1, SeriesLinkObligationStatusV1,
    SeriesLinkObligationTerminalProjectionV1, SeriesLinkObligationV1, SeriesMarketDispositionV1,
    SeriesMarketLinkBindingV1, SeriesMarketLinkV1, SeriesPlanV5Id, SourceOccurrenceV1Id,
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V2, MARKET_FOUNDATION_MAX_OUTCOMES_V2,
    MARKET_FOUNDATION_SLOT_COUNT_V2, SERIES_MARKET_LINK_BYTES_V1,
};

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn configuration() -> SeriesLinkObligationConfigurationV1 {
    SeriesLinkObligationConfigurationV1 {
        capability_profile_id: id(8),
        attachment_plan_id: id(7),
        initial_statuses: [
            SeriesLinkObligationStatusV1::CapabilityDisabled,
            SeriesLinkObligationStatusV1::EnabledNeverFounded,
            SeriesLinkObligationStatusV1::EnabledNeverFounded,
            SeriesLinkObligationStatusV1::CapabilityDisabled,
        ],
    }
}

fn binding() -> SeriesMarketLinkBindingV1 {
    let configuration = configuration();
    SeriesMarketLinkBindingV1 {
        series_plan_id: SeriesPlanV5Id::from_bytes([1; 32]),
        ordinal: 0,
        market_instance_id: MarketInstanceV2Id::from_bytes([2; 32]),
        market_root_account_id: id(3),
        market_binding_id: id(4),
        disposition: SeriesMarketDispositionV1::Founder,
        funding_terms_id: SeriesFundingTermsV2Id::from_bytes([5; 32]),
        funding_quote_id: SeriesFundingQuoteV4Id::from_bytes([6; 32]),
        attachment_plan_id: id(7),
        capability_profile_id: id(8),
        obligation_configuration_id: configuration.id().unwrap(),
        compiler_output_id: id(9),
        source_occurrence_id: SourceOccurrenceV1Id::from_bytes([10; 32]),
        source_occurrence_account_id: id(11),
        source_occurrence_account_authentication_id: id(12),
        source_occurrence_receipt_id: id(13),
        source_release_id: id(14),
        source_route_id: id(15),
        clock_policy_id: id(16),
        source_plane_contract_id: id(17),
        source_spec_id: id(18),
        window_spec_id: id(19),
        statistic_key_id: id(20),
        funding_state_account_id: id(21),
        funding_debit_receipt_id: id(22),
        rent_refund_owner: id(23),
        neutral_lamport_sink: id(24),
        generation: 1,
        source_repair_generation: 1,
        funding_transition_sequence: 1,
    }
}

fn active_link() -> SeriesMarketLinkV1 {
    SeriesMarketLinkV1::initialize_pending(binding(), configuration(), 1, 0)
        .unwrap()
        .activate(1, id(25))
        .unwrap()
}

fn schedule() -> MarketFoundationScheduleV2 {
    let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V2];
    for principal in &mut slot_principal_lamports[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V2 + 2] {
        *principal = 1;
    }
    let custody_start = MARKET_FOUNDATION_CORE_SLOT_COUNT_V2 + MARKET_FOUNDATION_MAX_OUTCOMES_V2;
    for principal in &mut slot_principal_lamports[custody_start..custody_start + 2] {
        *principal = 1;
    }
    MarketFoundationScheduleV2 {
        outcome_count: 2,
        slot_principal_lamports,
        founding_timeout_buckets: 10,
    }
}

fn account_graph() -> MarketFoundationAccountGraphV2 {
    let schedule = schedule();
    let mut account_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V2];
    let mut index = 0usize;
    while index < MARKET_FOUNDATION_SLOT_COUNT_V2 {
        if schedule.slot_principal_lamports[index] != 0 {
            account_ids[index] = ContentId::from_bytes([u8::try_from(index + 1).unwrap(); 32]);
        }
        index += 1;
    }
    MarketFoundationAccountGraphV2 {
        market_instance_id: MarketInstanceV2Id::from_bytes([201; 32]),
        generation: 1,
        foundation_schedule_id: schedule.id().unwrap(),
        account_ids,
    }
}

#[test]
fn configuration_refuses_caller_shaped_terminal_initial_state() {
    let mut configuration = configuration();
    configuration.initial_statuses[0] = SeriesLinkObligationStatusV1::Terminal;
    assert_eq!(configuration.validate(), Err(Error::InvalidParameter));
}

#[test]
fn configuration_refuses_caller_shaped_live_initial_state() {
    let mut configuration = configuration();
    configuration.initial_statuses[0] = SeriesLinkObligationStatusV1::Live;
    assert_eq!(configuration.validate(), Err(Error::InvalidParameter));
}

#[test]
fn obligation_admission_is_exact_and_replay_sequenced() {
    let link = active_link();
    let projection = SeriesLinkObligationAdmissionProjectionV1 {
        link_semantic_id: link.semantic_id().unwrap(),
        obligation: SeriesLinkObligationV1::Dealer,
        link_transition_sequence: 2,
        owner_admission_receipt_id: id(29),
    };
    let live = link.admit_obligation(projection).unwrap();
    assert_eq!(
        live.obligation_status(SeriesLinkObligationV1::Dealer),
        SeriesLinkObligationStatusV1::Live
    );
    assert_eq!(
        live.admit_obligation(projection),
        Err(Error::UnauthenticatedAuthority)
    );
}

#[test]
fn disabled_and_enabled_unfounded_require_authenticated_absence() {
    let link = active_link();
    let wrong = SeriesLinkObligationTerminalProjectionV1 {
        link_semantic_id: link.semantic_id().unwrap(),
        obligation: SeriesLinkObligationV1::Dealer,
        disposition: SeriesLinkObligationDispositionV1::Terminal,
        link_transition_sequence: 2,
        owner_terminal_receipt_id: id(26),
    };
    assert_eq!(
        link.consume_obligation(wrong),
        Err(Error::WorkStateMismatch)
    );

    let absent = SeriesLinkObligationTerminalProjectionV1 {
        disposition: SeriesLinkObligationDispositionV1::Absent,
        ..wrong
    };
    let next = link.consume_obligation(absent).unwrap();
    assert_eq!(
        next.obligation_status(SeriesLinkObligationV1::Dealer),
        SeriesLinkObligationStatusV1::Terminal
    );
}

#[test]
fn hostile_active_failure_session_cannot_erase_transcript() {
    let pinned = active_link().pin_failure_session(id(27)).unwrap();
    assert_eq!(
        pinned.pin_failure_session(id(28)),
        Err(Error::WorkStateMismatch)
    );
    let mut body = [0_u8; SERIES_MARKET_LINK_BYTES_V1];
    pinned.encode_into(&mut body).unwrap();
    body[SERIES_MARKET_LINK_BYTES_V1 - 32..].fill(0);
    assert_eq!(
        SeriesMarketLinkV1::decode(&body),
        Err(Error::WorkStateMismatch)
    );
}

#[test]
fn failure_transcript_survives_session_release() {
    let pinned = active_link().pin_failure_session(id(27)).unwrap();
    let pinned_transcript = pinned.failure_session_transcript_id();
    let released = pinned.release_failure_session(id(28)).unwrap();
    assert_eq!(released.active_failure_sessions(), 0);
    assert_eq!(released.failure_sessions_started(), 1);
    assert_ne!(released.failure_session_transcript_id(), ContentId::ZERO);
    assert_ne!(released.failure_session_transcript_id(), pinned_transcript);
    assert_eq!(
        released.release_failure_session(id(28)),
        Err(Error::WorkStateMismatch)
    );

    let mut body = [0_u8; SERIES_MARKET_LINK_BYTES_V1];
    released.encode_into(&mut body).unwrap();
    body[SERIES_MARKET_LINK_BYTES_V1 - 32..].fill(0);
    assert_eq!(
        SeriesMarketLinkV1::decode(&body),
        Err(Error::WorkStateMismatch)
    );
}

#[test]
fn foundation_graph_separates_failure_admission_and_runtime_roots() {
    let graph = account_graph();
    let schedule = schedule();
    assert_ne!(
        graph
            .account(MarketFoundationSlotV2::FailureAdmissionRoot)
            .unwrap(),
        graph
            .account(MarketFoundationSlotV2::FailureRuntimeRoot)
            .unwrap()
    );
    assert!(graph.validate(schedule).is_ok());
}

#[test]
fn foundation_graph_refuses_role_alias_and_noncanonical_tail() {
    let schedule = schedule();
    let mut aliased = account_graph();
    aliased.account_ids[MarketFoundationSlotV2::FailureRuntimeRoot.index().unwrap()] = aliased
        .account(MarketFoundationSlotV2::FailureAdmissionRoot)
        .unwrap();
    assert_eq!(aliased.validate(schedule), Err(Error::MismatchedArtifact));

    let mut tailed = account_graph();
    tailed.account_ids[MARKET_FOUNDATION_CORE_SLOT_COUNT_V2 + 3] = id(250);
    assert_eq!(tailed.validate(schedule), Err(Error::NonCanonicalPadding));
}
