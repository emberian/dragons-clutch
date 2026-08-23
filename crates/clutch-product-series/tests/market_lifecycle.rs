use clutch_product_series::{
    ContentId, Error, FixedCodec, MarketInstanceV2Id, SeriesFundingQuoteV2Id,
    SeriesFundingTermsV2Id, SeriesLinkObligationConfigurationV1, SeriesLinkObligationDispositionV1,
    SeriesLinkObligationStatusV1, SeriesLinkObligationTerminalProjectionV1, SeriesLinkObligationV1,
    SeriesMarketDispositionV1, SeriesMarketLinkBindingV1, SeriesMarketLinkV1, SeriesPlanV5Id,
    SourceOccurrenceV1Id, SERIES_MARKET_LINK_BYTES_V1,
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
            SeriesLinkObligationStatusV1::Live,
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
        funding_quote_id: SeriesFundingQuoteV2Id::from_bytes([6; 32]),
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

#[test]
fn configuration_refuses_caller_shaped_terminal_initial_state() {
    let mut configuration = configuration();
    configuration.initial_statuses[0] = SeriesLinkObligationStatusV1::Terminal;
    assert_eq!(configuration.validate(), Err(Error::InvalidParameter));
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
    let released = active_link()
        .pin_failure_session(id(27))
        .unwrap()
        .release_failure_session(id(28))
        .unwrap();
    assert_eq!(released.active_failure_sessions(), 0);
    assert_ne!(released.failure_session_transcript_id(), ContentId::ZERO);
}
