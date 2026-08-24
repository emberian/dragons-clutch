use super::*;

fn id(fill: u8) -> ContentId {
    ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test id")
}

fn config() -> GeneralConfigV1 {
    GeneralConfigV1::new(GeneralConfigV1Input {
        capacity_profile_id: id(1),
        market_identity_id: id(2),
        claim_basis_id: id(3),
        capability_release_id: id(4),
        settlement_asset_id: id(5),
        generation: 7,
        price_scale: 100,
        collection_slots: 10,
        selection_slots: 10,
        settlement_slots: 10,
        max_orders_per_candidate: 8,
        max_pages_per_candidate: 2,
        outcome_count: 2,
    })
    .expect("valid config")
}

fn order(
    order_fill: u8,
    owner_fill: u8,
    nonce: u64,
    coefficients: [i64; 2],
    limit: i128,
) -> PortfolioOrderV1<2> {
    PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: id(owner_fill),
        order_id: id(order_fill),
        generation: 7,
        batch_sequence: 0,
        nonce,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: limit,
        coefficients,
        outcome_count: 2,
    })
    .expect("valid order")
}

fn selecting_batch() -> BatchRootV1 {
    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch opens");
    batch.open_selection(10).expect("selection opens");
    batch
}

fn executions() -> [Option<ExecutionV1<2>>; MAX_EXECUTIONS_PER_PAGE_V1] {
    let first = order(10, 20, 0, [1, 0], 50);
    let second = order(11, 21, 0, [0, 1], 70);
    [
        Some(ExecutionV1 {
            order: first,
            order_state: OrderStateV1::open(first),
            fill_lots: 1,
        }),
        Some(ExecutionV1 {
            order: second,
            order_state: OrderStateV1::open(second),
            fill_lots: 1,
        }),
        None,
        None,
    ]
}

fn valid_candidate(batch: BatchRootV1, candidate_fill: u8) -> CandidateStateV1<2> {
    let prices = [40u64, 60];
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: id(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices,
        outcome_count: 2,
    };
    let mut candidate =
        CandidateStateV1::submit(id(candidate_fill), submission, config(), batch, 10)
            .expect("candidate submits");
    candidate
        .verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: executions(),
            },
            config(),
            batch,
            11,
        )
        .expect("page verifies");
    candidate
        .finish_verification(config())
        .expect("candidate conserves complete set");
    candidate
}

#[test]
fn golden_config_and_order_round_trip() {
    let config = config();
    let bytes = config.to_bytes();
    assert_eq!(bytes.len(), GENERAL_CONFIG_BYTES);
    assert_eq!(&bytes[..8], b"DCLTGEN1");
    assert_eq!(GeneralConfigV1::decode(&bytes), Ok(config));

    let order = order(10, 20, 9, [-2, 3], 77);
    let mut bytes = [0; 216];
    order.encode(&mut bytes).expect("order encodes");
    assert_eq!(
        bytes.len(),
        PortfolioOrderV1::<2>::encoded_len().expect("length")
    );
    assert_eq!(&bytes[..8], b"DCLTGOR1");
    assert_eq!(PortfolioOrderV1::<2>::decode(&bytes), Ok(order));
}

#[test]
fn hostile_decoders_reject_reserved_and_unused_words() {
    let mut config_bytes = config().to_bytes();
    config_bytes[231] = 1;
    assert_eq!(
        GeneralConfigV1::decode(&config_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let mut encoded_order = [0; 216];
    order(10, 20, 9, [1, -1], 10)
        .encode(&mut encoded_order)
        .expect("order encodes");
    let mut order_bytes = [0; 224];
    order_bytes[..216].copy_from_slice(&encoded_order);
    order_bytes[216] = 1;
    assert_eq!(
        PortfolioOrderV1::<2>::decode(&order_bytes),
        Err(Error::InvalidLength)
    );
}

#[test]
fn outcome_vector_codecs_scale_exactly_with_selected_width() {
    assert_eq!(PortfolioOrderV1::<2>::encoded_len(), Ok(216));
    assert_eq!(PortfolioOrderV1::<16>::encoded_len(), Ok(328));
    assert_eq!(SettlementReceiptV1::<2>::encoded_len(), Ok(192));
    assert_eq!(SettlementReceiptV1::<16>::encoded_len(), Ok(304));

    let order = order(10, 20, 9, [1, -1], 10);
    let mut short = [0; 216];
    order.encode(&mut short).expect("two-outcome order encodes");
    assert_eq!(
        PortfolioOrderV1::<16>::decode(&short),
        Err(Error::InvalidLength)
    );

    let bad_width = PortfolioOrderV1::<2>::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: id(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: 10,
        coefficients: [1, -1],
        outcome_count: 16,
    });
    assert_eq!(bad_width, Err(Error::InvalidOutcomeCount));

    let receipt = SettlementReceiptV1::<2> {
        candidate_id: id(40),
        order_id: id(10),
        owner: id(20),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        fill_lots: 1,
        remaining_lots: 0,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [1, -1],
        outcome_count: 2,
    };
    let mut receipt_bytes = [0; 192];
    receipt
        .encode(&mut receipt_bytes)
        .expect("two-outcome receipt encodes");
    assert_eq!(
        SettlementReceiptV1::<16>::decode(&receipt_bytes),
        Err(Error::InvalidLength)
    );
    receipt_bytes[12..14].copy_from_slice(&16_u16.to_le_bytes());
    assert_eq!(
        SettlementReceiptV1::<2>::decode(&receipt_bytes),
        Err(Error::InvalidOutcomeCount)
    );
}

#[test]
fn simplex_is_exact_and_has_one_canonical_width() {
    let batch = selecting_batch();
    let mut prices = [40u64, 59];
    let submission = CandidateSubmissionV1 {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        submitter: id(30),
        initial_transcript_id: id(50),
        generation: 7,
        batch_sequence: 0,
        valid_until_slot: 20,
        claimed_execution_count: 2,
        claimed_score: 20,
        prices,
        outcome_count: 2,
    };
    assert_eq!(
        CandidateStateV1::submit(id(40), submission, config(), batch, 10),
        Err(Error::InvalidSimplexPrice)
    );

    prices[1] = 60;
    assert!(CandidateStateV1::submit(
        id(40),
        CandidateSubmissionV1 {
            prices,
            ..submission
        },
        config(),
        batch,
        10
    )
    .is_ok());
}

#[test]
fn paginated_failure_does_not_advance_cursor() {
    let batch = selecting_batch();
    let prices = [40u64, 60];
    let mut candidate = CandidateStateV1::submit(
        id(40),
        CandidateSubmissionV1 {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            submitter: id(30),
            initial_transcript_id: id(50),
            generation: 7,
            batch_sequence: 0,
            valid_until_slot: 20,
            claimed_execution_count: 2,
            claimed_score: 20,
            prices,
            outcome_count: 2,
        },
        config(),
        batch,
        10,
    )
    .expect("submits");
    let before = candidate;
    let mut bad = executions();
    let mut second = bad[1].expect("second");
    second.order.order_id = id(9);
    second.order_state = OrderStateV1::open(second.order);
    bad[1] = Some(second);
    assert_eq!(
        candidate.verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: bad,
            },
            config(),
            batch,
            11,
        ),
        Err(Error::NonCanonicalOrder)
    );
    assert_eq!(candidate, before);
}

#[test]
fn imbalance_cannot_reach_valid_candidate() {
    let batch = selecting_batch();
    let prices = [40u64, 60];
    let first = order(10, 20, 0, [1, 0], 50);
    let mut candidate = CandidateStateV1::submit(
        id(40),
        CandidateSubmissionV1 {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            submitter: id(30),
            initial_transcript_id: id(50),
            generation: 7,
            batch_sequence: 0,
            valid_until_slot: 20,
            claimed_execution_count: 1,
            claimed_score: 10,
            prices,
            outcome_count: 2,
        },
        config(),
        batch,
        10,
    )
    .expect("submits");
    candidate
        .verify_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 1,
                executions: [
                    Some(ExecutionV1 {
                        order: first,
                        order_state: OrderStateV1::open(first),
                        fill_lots: 1,
                    }),
                    None,
                    None,
                    None,
                ],
            },
            config(),
            batch,
            11,
        )
        .expect("page arithmetic is locally valid");
    assert_eq!(
        candidate.finish_verification(config()),
        Err(Error::IncompleteSetImbalance)
    );
}

#[test]
fn deterministic_best_valid_submitted_candidate_tie_breaks_by_id() {
    let mut batch = selecting_batch();
    let mut high_id = valid_candidate(batch, 42);
    let mut low_id = valid_candidate(batch, 41);
    batch
        .consider_candidate(&mut high_id, 12)
        .expect("consider high");
    assert_eq!(
        batch.consider_candidate(&mut high_id, 12),
        Err(Error::CandidateClaimMismatch)
    );
    batch
        .consider_candidate(&mut low_id, 13)
        .expect("consider low");
    assert_eq!(batch.close_selection(20), Ok(Some(id(41))));
}

#[test]
fn settlement_prefix_carry_conserves_exact_complete_set() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    assert_eq!(batch.close_selection(20), Ok(Some(id(40))));
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let mut cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("settlement begins");
    assert_eq!(batch.phase(), BatchPhase::Applying);
    assert_eq!(hoard.principal_atoms(), 1);
    let result = cursor
        .settle_page(
            VerificationPageV1 {
                page_index: 0,
                prior_transcript_id: id(50),
                next_transcript_id: id(51),
                execution_count: 2,
                executions: executions(),
            },
            candidate,
            config(),
            batch,
        )
        .expect("page settles");
    let first = result.receipts[0].expect("first receipt");
    let second = result.receipts[1].expect("second receipt");
    assert_eq!(
        (
            first.quote_delta_atoms,
            first.carry_before,
            first.carry_after
        ),
        (-1, 0, 60)
    );
    assert_eq!(
        (
            second.quote_delta_atoms,
            second.carry_before,
            second.carry_after
        ),
        (0, 60, 0)
    );
    assert_eq!(first.quote_delta_atoms + second.quote_delta_atoms, -1);
    assert_eq!(
        result.order_states[0].expect("first state").phase(),
        OrderPhase::Consumed
    );

    let mut bytes = [0; 192];
    first.encode(&mut bytes).expect("receipt encodes");
    assert_eq!(SettlementReceiptV1::<2>::decode(&bytes), Ok(first));
    cursor
        .finish(candidate, &mut batch)
        .expect("exact settlement finishes");
    assert_eq!(hoard.principal_atoms(), 1);
    assert_eq!(hoard.liability_units_per_outcome(), 1);
    assert_eq!(batch.phase(), BatchPhase::Quiescent);
}

#[test]
fn applying_batch_cannot_expire_after_hoard_conversion() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    batch.close_selection(20).expect("winner freezes");
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let _cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("application commits");
    assert_eq!(batch.expire_unsettled(31), Err(Error::OutsideWindow));
    assert_eq!(batch.phase(), BatchPhase::Applying);
    assert_eq!(hoard.principal_atoms(), 1);
}

#[test]
fn failed_complete_set_burn_leaves_batch_and_hoard_unchanged() {
    let mut batch = selecting_batch();
    let mut candidate = valid_candidate(batch, 40);
    candidate.complete_set_delta = -1;
    candidate.total_quote_debit_numerator = -100;
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    batch.close_selection(20).expect("winner freezes");
    let before_batch = batch;
    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let before_hoard = hoard;
    assert_eq!(
        SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20),
        Err(Error::InsufficientHoardPrincipal)
    );
    assert_eq!(batch, before_batch);
    assert_eq!(hoard, before_hoard);
}

#[test]
fn cancellation_replay_and_expiry_are_refused() {
    let order = order(10, 20, 5, [1, -1], 0);
    let mut state = OrderStateV1::open(order);
    assert_eq!(state.cancel(id(20), 9, 10), Ok(()));
    assert_eq!(
        state.validate_snapshot(order, 1),
        Err(Error::OrderUnavailable)
    );

    let mut locked = OrderStateV1::open(order);
    assert_eq!(locked.cancel(id(20), 10, 10), Err(Error::OutsideWindow));
    assert_eq!(locked.phase(), OrderPhase::Open);

    let mut expired = order;
    expired.valid_until_slot = 29;
    let execution = ExecutionV1 {
        order: expired,
        order_state: OrderStateV1::open(expired),
        fill_lots: 1,
    };
    assert_eq!(
        validate_execution_binding(execution, config(), selecting_batch()),
        Err(Error::OrderExpired)
    );
}

#[test]
fn funding_is_prepaid_segregated_and_conserved() {
    let mut funding = GeneralFundingV1::founding(id(4), 10, 20, 30);
    assert_eq!(
        funding.debit(FundingCompartment::Work, 21, id(60)),
        Err(Error::InsufficientFunding)
    );
    let debit = funding
        .debit(FundingCompartment::Work, 7, id(60))
        .expect("prepaid work");
    assert_eq!(debit.amount, 7);
    funding.validate().expect("still conserved");
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Quiescing),
        Err(Error::NotQuiescent)
    );
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Terminal),
        Ok([10, 13, 30])
    );
    assert!(funding.is_discharged());
    funding.validate().expect("refund conserved");
}

#[test]
fn root_requires_real_quiescence_before_retirement() {
    let mut root = GeneralRootV1::founding(id(9), 7);
    assert_eq!(root.open_batch(), Ok(0));
    root.request_quiescence().expect("quiescing");
    assert_eq!(root.enter_terminal(), Err(Error::NotQuiescent));
    root.close_batch().expect("last batch closes");
    root.enter_terminal().expect("terminal");
    assert_eq!(root.retire(false), Err(Error::NotQuiescent));
    root.retire(true).expect("funding discharged");
    assert_eq!(root.phase(), GeneralPhase::Retired);
}
