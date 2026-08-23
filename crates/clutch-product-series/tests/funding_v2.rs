use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV2, ComponentDebitV1, ContentId, Error,
    MarketFoundationAccountGraphV1, MarketFoundationScheduleV1, MarketFoundationSlotV1,
    MarketInstanceV2Id, RecoveryAttemptFundingV1, SeriesFundingComponentV2, SeriesFundingPhaseV2,
    SeriesFundingQuoteV2, SeriesFundingReservationV2, SeriesFundingStateV2, SeriesFundingTermsV2Id,
    SeriesMarketDispositionV1, SeriesMarketLinkV1Id, SeriesOrdinalFulfillmentV2, SeriesPlanV5Id,
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V1, MARKET_FOUNDATION_MAX_OUTCOMES_V1,
    MARKET_FOUNDATION_SLOT_COUNT_V1, MAX_RECOVERY_ATTEMPTS, SERIES_FUNDING_COMPONENT_COUNT_V2,
};

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn debit(lamports: u64) -> ComponentDebitV1 {
    ComponentDebitV1 {
        lamports,
        collateral_atoms: 0,
    }
}

fn quote() -> SeriesFundingQuoteV2 {
    let mut slots = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V1];
    for amount in &mut slots[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + 2] {
        *amount = 1;
    }
    let custody = MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + MARKET_FOUNDATION_MAX_OUTCOMES_V1;
    slots[custody] = 1;
    slots[custody + 1] = 1;
    let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    attempts[0] = RecoveryAttemptFundingV1 {
        max_progress_units: 2,
        lamports_per_progress_unit: 3,
    };
    SeriesFundingQuoteV2 {
        evidence_only_recovery_policy_id: id(1),
        components: [debit(17), debit(7), debit(10), debit(3), debit(4), debit(5)],
        foundation: MarketFoundationScheduleV1 {
            outcome_count: 2,
            slot_principal_lamports: slots,
            founding_timeout_buckets: 8,
        },
        recovery_attempt_count: 1,
        recovery_attempt_funding: attempts,
        recovery_rent_principal_lamports: 4,
    }
}

#[derive(Debug)]
struct Authority;

impl AuthenticatedSeriesFundingAuthorityV2 for Authority {
    fn authenticate_activation(
        &self,
        _: &SeriesFundingQuoteV2,
        _: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> Result<(), Error> {
        Ok(())
    }

    fn authenticate_reservation(
        &self,
        _: &SeriesFundingStateV2,
        _: &SeriesFundingQuoteV2,
        _: &SeriesFundingReservationV2,
        _: &SeriesOrdinalFulfillmentV2,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn authenticate_commit(
        &self,
        _: &SeriesFundingStateV2,
        _: &SeriesFundingReservationV2,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn authenticate_abort(
        &self,
        _: &SeriesFundingStateV2,
        _: &SeriesFundingQuoteV2,
        _: &SeriesFundingReservationV2,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn authenticate_lapse(&self, _: &SeriesFundingStateV2, _: u32) -> Result<(), Error> {
        Ok(())
    }
}

fn active_state() -> SeriesFundingStateV2 {
    let quote = quote();
    let mut principal = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    for (index, value) in principal.iter_mut().enumerate() {
        *value = ComponentDebitV1 {
            lamports: quote.components[index].lamports * 2,
            collateral_atoms: quote.components[index].collateral_atoms * 2,
        };
    }
    SeriesFundingStateV2::activate(
        &Authority,
        SeriesPlanV5Id::from_bytes([11; 32]),
        SeriesFundingTermsV2Id::from_bytes([12; 32]),
        2,
        &quote,
        principal,
        [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
    )
    .unwrap()
}

#[test]
fn reservation_blocks_lapse_and_abort_restores_exact_principal() {
    let quote = quote();
    let mut state = active_state();
    let before = state;
    let reservation = state
        .reserve(
            &Authority,
            &quote,
            SeriesMarketDispositionV1::Founder,
            SeriesOrdinalFulfillmentV2 {
                market_instance_id: MarketInstanceV2Id::from_bytes([21; 32]),
                series_market_link_id: SeriesMarketLinkV1Id::from_bytes([22; 32]),
                debit_component: [true; SERIES_FUNDING_COMPONENT_COUNT_V2],
            },
            id(23),
        )
        .unwrap();
    assert_eq!(reservation.ordinal, 0);
    assert_eq!(state.phase().unwrap(), SeriesFundingPhaseV2::Pending);
    assert_eq!(state.lapse(&Authority, &quote), Err(Error::SeriesNotActive));
    assert_eq!(state.abort_pending(&Authority, &quote).unwrap(), 0);
    assert_eq!(state.components, before.components);
    assert_eq!(state.next_ordinal, 0);
}

#[test]
fn converger_cannot_debit_shared_market_or_recovery() {
    let quote = quote();
    let mut state = active_state();
    let result = state.reserve(
        &Authority,
        &quote,
        SeriesMarketDispositionV1::Converger,
        SeriesOrdinalFulfillmentV2 {
            market_instance_id: MarketInstanceV2Id::from_bytes([31; 32]),
            series_market_link_id: SeriesMarketLinkV1Id::from_bytes([32; 32]),
            debit_component: [true; SERIES_FUNDING_COMPONENT_COUNT_V2],
        },
        id(33),
    );
    assert_eq!(result, Err(Error::InvalidComponentStatus));
}

#[test]
fn series_admission_is_mandatory_even_for_exact_convergence() {
    let quote = quote();
    let mut state = active_state();
    let mut debit_component = [false; SERIES_FUNDING_COMPONENT_COUNT_V2];
    debit_component[SeriesFundingComponentV2::SourceWork.index()] = true;
    let result = state.reserve(
        &Authority,
        &quote,
        SeriesMarketDispositionV1::Converger,
        SeriesOrdinalFulfillmentV2 {
            market_instance_id: MarketInstanceV2Id::from_bytes([41; 32]),
            series_market_link_id: SeriesMarketLinkV1Id::from_bytes([42; 32]),
            debit_component,
        },
        id(43),
    );
    assert_eq!(result, Err(Error::InvalidComponentStatus));
}

#[test]
fn failure_admission_and_runtime_accounts_cannot_alias() {
    let schedule = quote().foundation;
    let mut accounts = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V1];
    for (index, account) in accounts.iter_mut().enumerate() {
        if schedule.slot_principal_lamports[index] != 0 {
            *account = id(u8::try_from(index + 60).unwrap());
        }
    }
    let mut graph = MarketFoundationAccountGraphV1 {
        market_instance_id: MarketInstanceV2Id::from_bytes([51; 32]),
        generation: 1,
        foundation_schedule_id: schedule.id().unwrap(),
        account_ids: accounts,
    };
    assert!(graph.validate(schedule).is_ok());
    let admission = MarketFoundationSlotV1::FailureAdmissionRoot
        .index()
        .unwrap();
    let runtime = MarketFoundationSlotV1::FailureRuntimeRoot.index().unwrap();
    graph.account_ids[runtime] = graph.account_ids[admission];
    assert_eq!(graph.validate(schedule), Err(Error::MismatchedArtifact));
}
