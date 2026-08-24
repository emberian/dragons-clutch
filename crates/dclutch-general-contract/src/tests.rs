use super::*;
use dclutch_capability_contract::{
    ActivationPolicy, CapabilityEntryV1, CapabilityManifestV1, FundingAmountsV1, FundingStateV1,
    FundingStatus, CAPABILITY_ENTRY_BYTES, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use sha2::{Digest, Sha256};

fn id(fill: u8) -> ContentId {
    ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test id")
}

fn owner(fill: u8) -> OwnerKeyV1 {
    OwnerKeyV1::new([fill; 32]).expect("nonzero test owner")
}

fn capability_id(fill: u8) -> dclutch_capability_contract::ContentId {
    dclutch_capability_contract::ContentId::new([fill; 32]).expect("capability ID")
}

fn capability_id_from(id: ContentId) -> dclutch_capability_contract::ContentId {
    dclutch_capability_contract::ContentId::new(id.to_bytes()).expect("capability ID")
}

fn capability_entry(config_id: ContentId, quote: FundingAmountsV1) -> CapabilityEntryV1 {
    CapabilityEntryV1::new(
        capability_id_from(GENERAL_CAPABILITY_KIND_ID_V1),
        capability_id_from(GENERAL_CAPABILITY_RELEASE_ID_V1),
        capability_id_from(config_id),
        capability_id_from(config().capacity_profile_id()),
        capability_id_from(GENERAL_CHILD_SCHEMA_ID_V1),
        capability_id_from(GENERAL_CHILD_DERIVATION_ID_V1),
        ActivationPolicy::PrepaidLazy,
        100,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .expect("General capability entry")
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn config() -> GeneralConfigV1 {
    GeneralConfigV1::new(GeneralConfigV1Input {
        capacity_profile_id: id(1),
        market_identity_id: id(2),
        claim_basis_id: id(3),
        capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V1,
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
        owner: owner(owner_fill),
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

fn round_trip_general_root(root: GeneralRootV1) {
    let mut bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut bytes).expect("root encodes");
    assert_eq!(&bytes[..8], b"DCLTGRR1");
    assert_eq!(GeneralRootV1::decode(&bytes), Ok(root));
}

fn round_trip_batch_root(batch: BatchRootV1) {
    let mut bytes = [0; BATCH_ROOT_BYTES];
    batch.encode(&mut bytes).expect("batch encodes");
    assert_eq!(&bytes[..8], b"DCLTGBR1");
    assert_eq!(BatchRootV1::decode(&bytes), Ok(batch));
}

fn round_trip_order_state(state: OrderStateV1) {
    let mut bytes = [0; ORDER_STATE_BYTES];
    state.encode(&mut bytes).expect("order state encodes");
    assert_eq!(&bytes[..8], b"DCLTGOS1");
    assert_eq!(OrderStateV1::decode(&bytes), Ok(state));
}

fn round_trip_funding(funding: GeneralFundingV1) {
    let mut bytes = [0; GENERAL_FUNDING_BYTES];
    funding.encode(&mut bytes).expect("funding encodes");
    assert_eq!(&bytes[..8], b"DCLTGFN1");
    assert_eq!(GeneralFundingV1::decode(&bytes), Ok(funding));
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
    assert_eq!(GENERAL_CONFIG_BYTES, 200);
    let mut config_bytes = config().to_bytes();
    config_bytes[199] = 1;
    assert_eq!(
        GeneralConfigV1::decode(&config_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut substituted_release = config().to_bytes();
    substituted_release[112] ^= 1;
    assert_eq!(
        GeneralConfigV1::decode(&substituted_release),
        Err(Error::UnrecognizedCapability)
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

    let mut zero_owner = encoded_order;
    zero_owner[80..112].fill(0);
    assert_eq!(
        PortfolioOrderV1::<2>::decode(&zero_owner),
        Err(Error::ZeroIdentifier)
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
        owner: owner(20),
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
        owner: owner(20),
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
    assert_eq!(state.cancel(owner(20), 9, 10), Ok(()));
    assert_eq!(
        state.validate_snapshot(order, 1),
        Err(Error::OrderUnavailable)
    );

    let mut locked = OrderStateV1::open(order);
    assert_eq!(locked.cancel(owner(20), 10, 10), Err(Error::OutsideWindow));
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

#[test]
fn general_root_codec_round_trips_every_phase_and_counter_transition() {
    let mut root = GeneralRootV1::founding(id(9), 7);
    round_trip_general_root(root);
    assert_eq!(root.open_batch(), Ok(0));
    assert_eq!(root.open_batch(), Ok(1));
    round_trip_general_root(root);

    root.request_quiescence().expect("quiescing");
    round_trip_general_root(root);
    root.close_batch().expect("one child retires");
    round_trip_general_root(root);
    root.close_batch().expect("last child retires");
    root.enter_terminal().expect("terminal");
    round_trip_general_root(root);
    root.retire(true).expect("retired");
    round_trip_general_root(root);
}

#[test]
fn batch_root_codec_round_trips_every_phase_and_winner_shape() {
    let mut empty = BatchRootV1::open(id(9), 1, 0, config()).expect("empty batch opens");
    empty.open_selection(10).expect("empty selection opens");
    assert_eq!(empty.close_selection(20), Ok(None));
    round_trip_batch_root(empty);
    empty.retire(true).expect("empty batch retires");
    round_trip_batch_root(empty);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch opens");
    round_trip_batch_root(batch);
    batch.open_selection(10).expect("selection opens");
    round_trip_batch_root(batch);

    let mut candidate = valid_candidate(batch, 40);
    batch
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    round_trip_batch_root(batch);
    batch.close_selection(20).expect("winner freezes");
    round_trip_batch_root(batch);

    let mut hoard = HoardLedgerV1::new(id(2), 0, 0).expect("empty hoard");
    let mut cursor = SettlementCursorV1::begin(candidate, &mut batch, &mut hoard, config(), 20)
        .expect("applying begins");
    round_trip_batch_root(batch);
    cursor
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
    cursor
        .finish(candidate, &mut batch)
        .expect("batch quiesces");
    round_trip_batch_root(batch);
    batch.retire(true).expect("batch retires");
    round_trip_batch_root(batch);
}

#[test]
fn order_state_codec_round_trips_open_cancelled_partial_and_consumed() {
    let signed = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 55,
        valid_until_slot: 30,
        max_lots: 2,
        max_quote_debit_per_lot_numerator: 0,
        coefficients: [1, -1],
        outcome_count: 2,
    })
    .expect("valid order");
    let mut partial = OrderStateV1::open(signed);
    round_trip_order_state(partial);
    partial.consume(signed, 1).expect("one lot consumes");
    round_trip_order_state(partial);
    partial.consume(signed, 1).expect("last lot consumes");
    round_trip_order_state(partial);

    let mut cancelled = OrderStateV1::open(signed);
    cancelled.cancel(owner(20), 9, 10).expect("owner cancels");
    round_trip_order_state(cancelled);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch");
    batch.open_selection(10).expect("selecting");
    batch.close_selection(20).expect("empty quiescent");
    let mut released = OrderStateV1::open(signed);
    released
        .release_after_batch(signed, batch)
        .expect("remainder releases");
    round_trip_order_state(released);
}

#[test]
fn general_funding_codec_round_trips_debits_and_atomic_terminal_refund() {
    let mut funding = GeneralFundingV1::founding(id(4), 10, 20, 30);
    round_trip_funding(funding);
    funding
        .debit(FundingCompartment::Liveness, 3, id(60))
        .expect("liveness debit");
    funding
        .debit(FundingCompartment::Work, 7, id(61))
        .expect("work debit");
    round_trip_funding(funding);
    assert_eq!(
        funding.refund_terminal(GeneralPhase::Terminal),
        Ok([7, 13, 30])
    );
    round_trip_funding(funding);
}

#[test]
fn mutable_state_codecs_reject_wrong_width_type_headers_and_reserved_bytes() {
    let root = GeneralRootV1::founding(id(9), 7);
    let mut root_bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut root_bytes).expect("root encodes");
    assert_eq!(
        GeneralRootV1::decode(&root_bytes[..GENERAL_ROOT_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut root_trailing = [0; GENERAL_ROOT_BYTES + 1];
    root_trailing[..GENERAL_ROOT_BYTES].copy_from_slice(&root_bytes);
    assert_eq!(
        GeneralRootV1::decode(&root_trailing),
        Err(Error::InvalidLength)
    );
    root_bytes[..8].copy_from_slice(b"DCLTGBR1");
    assert_eq!(GeneralRootV1::decode(&root_bytes), Err(Error::InvalidMagic));

    let mut funding_bytes = [0; GENERAL_FUNDING_BYTES];
    GeneralFundingV1::founding(id(4), 1, 2, 3)
        .encode(&mut funding_bytes)
        .expect("funding encodes");
    assert_eq!(
        GeneralFundingV1::decode(&funding_bytes[..GENERAL_FUNDING_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut funding_trailing = [0; GENERAL_FUNDING_BYTES + 1];
    funding_trailing[..GENERAL_FUNDING_BYTES].copy_from_slice(&funding_bytes);
    assert_eq!(
        GeneralFundingV1::decode(&funding_trailing),
        Err(Error::InvalidLength)
    );
    funding_bytes[12] = 1;
    assert_eq!(
        GeneralFundingV1::decode(&funding_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let mut batch_bytes = [0; BATCH_ROOT_BYTES];
    BatchRootV1::open(id(9), 0, 0, config())
        .expect("batch opens")
        .encode(&mut batch_bytes)
        .expect("batch encodes");
    assert_eq!(
        BatchRootV1::decode(&batch_bytes[..BATCH_ROOT_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut batch_trailing = [0; BATCH_ROOT_BYTES + 1];
    batch_trailing[..BATCH_ROOT_BYTES].copy_from_slice(&batch_bytes);
    assert_eq!(
        BatchRootV1::decode(&batch_trailing),
        Err(Error::InvalidLength)
    );
    batch_bytes[84] = 1;
    assert_eq!(
        BatchRootV1::decode(&batch_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );

    let signed = order(10, 20, 9, [1, -1], 0);
    let mut order_state_bytes = [0; ORDER_STATE_BYTES];
    OrderStateV1::open(signed)
        .encode(&mut order_state_bytes)
        .expect("order state encodes");
    assert_eq!(
        OrderStateV1::decode(&order_state_bytes[..ORDER_STATE_BYTES - 1]),
        Err(Error::InvalidLength)
    );
    let mut order_state_trailing = [0; ORDER_STATE_BYTES + 1];
    order_state_trailing[..ORDER_STATE_BYTES].copy_from_slice(&order_state_bytes);
    assert_eq!(
        OrderStateV1::decode(&order_state_trailing),
        Err(Error::InvalidLength)
    );
    order_state_bytes[13] = 1;
    assert_eq!(
        OrderStateV1::decode(&order_state_bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn general_root_decoder_rejects_unreachable_counts_terminal_children_and_tags() {
    let mut root = GeneralRootV1::founding(id(9), 7);
    root.open_batch().expect("batch reserves");
    let mut bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut bytes).expect("root encodes");

    bytes[64..68].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
    bytes[12] = general_phase_tag(GeneralPhase::Terminal);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = u8::MAX;
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::InvalidPhase));
    bytes[12] = general_phase_tag(GeneralPhase::Active);
    bytes[16..48].fill(0);
    assert_eq!(GeneralRootV1::decode(&bytes), Err(Error::ZeroIdentifier));
}

#[test]
fn batch_root_decoder_rejects_bad_option_shape_deadlines_and_phase_substitution() {
    let mut bytes = [0; BATCH_ROOT_BYTES];
    BatchRootV1::open(id(9), 0, 0, config())
        .expect("batch opens")
        .encode(&mut bytes)
        .expect("batch encodes");

    bytes[88] = 1;
    assert_eq!(
        BatchRootV1::decode(&bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
    bytes[88] = 0;
    bytes[13] = 2;
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[13] = 0;
    bytes[64..72].copy_from_slice(&10_u64.to_le_bytes());
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));

    let mut selecting = selecting_batch();
    let mut candidate = valid_candidate(selecting, 40);
    selecting
        .consider_candidate(&mut candidate, 12)
        .expect("candidate considered");
    selecting
        .encode(&mut bytes)
        .expect("winner-bearing selection encodes");
    bytes[12] = batch_phase_tag(BatchPhase::Collecting);
    assert_eq!(BatchRootV1::decode(&bytes), Err(Error::NonCanonicalState));
}

#[test]
fn order_state_decoder_rejects_phase_balance_and_identity_substitution() {
    let signed = order(10, 20, 9, [1, -1], 0);
    let state = OrderStateV1::open(signed);
    let mut bytes = [0; ORDER_STATE_BYTES];
    state.encode(&mut bytes).expect("state encodes");

    bytes[88..96].copy_from_slice(&2_u64.to_le_bytes());
    let inflated = OrderStateV1::decode(&bytes).expect("internally shaped state");
    assert_eq!(
        inflated.authenticate(signed),
        Err(Error::OrderBindingMismatch)
    );
    bytes[88..96].copy_from_slice(&1_u64.to_le_bytes());
    bytes[16..48].copy_from_slice(id(11).as_bytes());
    let substituted = OrderStateV1::decode(&bytes).expect("internally shaped state");
    assert_eq!(
        substituted.authenticate(signed),
        Err(Error::OrderBindingMismatch)
    );
    bytes[16..48].copy_from_slice(id(10).as_bytes());

    bytes[88..96].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = order_phase_tag(OrderPhase::Consumed);
    bytes[88..96].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::NonCanonicalState));
    bytes[12] = u8::MAX;
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::InvalidPhase));
    bytes[12] = order_phase_tag(OrderPhase::Open);
    bytes[16..48].fill(0);
    assert_eq!(OrderStateV1::decode(&bytes), Err(Error::ZeroIdentifier));
}

#[test]
fn funding_decoder_rejects_overflow_broken_conservation_and_partial_refund() {
    let mut bytes = [0; GENERAL_FUNDING_BYTES];
    GeneralFundingV1::founding(id(4), u64::MAX, 20, 30)
        .encode(&mut bytes)
        .expect("funding encodes");
    bytes[96..104].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::ArithmeticOverflow)
    );

    GeneralFundingV1::founding(id(4), 10, 20, 30)
        .encode(&mut bytes)
        .expect("funding encodes");
    bytes[72..80].copy_from_slice(&9_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::FundingConservationMismatch)
    );
    bytes[120..128].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        GeneralFundingV1::decode(&bytes),
        Err(Error::NonCanonicalState)
    );
}

#[test]
fn recognized_general_release_ids_are_derived_and_pda_domains_are_distinct() {
    assert_eq!(
        digest(GENERAL_CAPABILITY_KIND_PREIMAGE_V1),
        GENERAL_CAPABILITY_KIND_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CAPABILITY_RELEASE_PREIMAGE_V1),
        GENERAL_CAPABILITY_RELEASE_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CHILD_SCHEMA_PREIMAGE_V1),
        GENERAL_CHILD_SCHEMA_ID_V1.to_bytes()
    );
    assert_eq!(
        digest(GENERAL_CHILD_DERIVATION_PREIMAGE_V1),
        GENERAL_CHILD_DERIVATION_ID_V1.to_bytes()
    );

    let domains = [
        GENERAL_CONFIG_PDA_DOMAIN_V1,
        GENERAL_FUNDING_PDA_DOMAIN_V1,
        GENERAL_ROOT_PDA_DOMAIN_V1,
        GENERAL_BATCH_PDA_DOMAIN_V1,
        GENERAL_ORDER_STATE_PDA_DOMAIN_V1,
        GENERAL_ORDER_CUSTODY_PDA_DOMAIN_V1,
        GENERAL_QUOTE_ESCROW_PDA_DOMAIN_V1,
    ];
    for (index, domain) in domains.iter().enumerate() {
        assert!(!domain.is_empty());
        assert!(domain.len() <= 32);
        assert!(domains.iter().skip(index + 1).all(|other| other != domain));
    }
}

#[test]
fn capability_activation_maps_only_the_immutable_quote_into_general_funding() {
    let config_id = id(70);
    let manifest_id = id(71);
    let quote = FundingAmountsV1::new(11, 13, 17, 0, 19, 0, 23).expect("quote");
    let entry = capability_entry(config_id, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .expect("canonical manifest");
    validate_general_capability_entry_v1(entry, config_id, config()).expect("recognized entry");
    let capability_funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        manifest,
        0,
        quote.total_principal(),
    )
    .expect("prepaid generic funding");

    let activation = GeneralFundingV1::activate_from_capability(
        config_id,
        config(),
        manifest_id,
        manifest,
        capability_funding,
        quote.total_principal(),
        10,
    )
    .expect("exact General activation");
    assert_eq!(activation.rent_principal(), 11);
    assert_eq!(activation.creation_principal(), 13);
    assert_eq!(activation.general_principal(), 59);
    assert_eq!(
        activation.capability_funding_after().status(),
        FundingStatus::Active
    );
    assert_eq!(
        activation
            .capability_funding_after()
            .remaining()
            .total_principal(),
        0
    );
    assert_eq!(
        activation
            .capability_funding_after()
            .released()
            .total_principal(),
        quote.total_principal()
    );
    let general = activation.general_funding();
    assert_eq!(general.remaining(FundingCompartment::Liveness), Ok(23));
    assert_eq!(general.remaining(FundingCompartment::Work), Ok(17));
    assert_eq!(general.remaining(FundingCompartment::Bounty), Ok(19));
    assert_eq!(
        general.capability_release_id(),
        GENERAL_CAPABILITY_RELEASE_ID_V1
    );
}

#[test]
fn capability_activation_rejects_extra_compartments_release_substitution_and_deadline() {
    let config_id = id(70);
    let manifest_id = id(71);
    let extra = FundingAmountsV1::new(1, 1, 2, 1, 3, 0, 4).expect("quote");
    let extra_entry = capability_entry(config_id, extra);
    let mut extra_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let extra_manifest =
        CapabilityManifestV1::encode_into(&[extra_entry], &mut extra_bytes).expect("manifest");
    let extra_funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        extra_manifest,
        0,
        extra.total_principal(),
    )
    .expect("funding");
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            config_id,
            config(),
            manifest_id,
            extra_manifest,
            extra_funding,
            extra.total_principal(),
            10,
        ),
        Err(Error::ExtraneousCapabilityFunding)
    );

    let quote = FundingAmountsV1::new(1, 1, 2, 0, 3, 0, 4).expect("quote");
    let wrong_release = CapabilityEntryV1::new(
        capability_id_from(GENERAL_CAPABILITY_KIND_ID_V1),
        capability_id(88),
        capability_id_from(config_id),
        capability_id_from(config().capacity_profile_id()),
        capability_id_from(GENERAL_CHILD_SCHEMA_ID_V1),
        capability_id_from(GENERAL_CHILD_DERIVATION_ID_V1),
        ActivationPolicy::PrepaidLazy,
        100,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .expect("entry");
    assert_eq!(
        validate_general_capability_entry_v1(wrong_release, config_id, config()),
        Err(Error::UnrecognizedCapability)
    );

    let entry = capability_entry(config_id, quote);
    let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest =
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest");
    let funding = FundingStateV1::new(
        capability_id_from(manifest_id),
        manifest,
        0,
        quote.total_principal(),
    )
    .expect("funding");
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            config_id,
            config(),
            manifest_id,
            manifest,
            funding,
            quote.total_principal(),
            101,
        ),
        Err(Error::CapabilityFundingMismatch)
    );
    assert_eq!(
        GeneralFundingV1::activate_from_capability(
            id(72),
            config(),
            manifest_id,
            manifest,
            funding,
            quote.total_principal(),
            10,
        ),
        Err(Error::UnrecognizedCapability)
    );
}

#[test]
fn order_reserve_uses_one_checked_ceiling_and_exact_negative_coefficients() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    assert_eq!(order.max_lots(), 3);
    assert_eq!(order.max_quote_debit_per_lot_numerator(), 101);
    let reserve = order.worst_case_reserve(config()).expect("reserve");
    assert_eq!(reserve.quote_atoms(), 4);
    assert_eq!(reserve.claim_atoms(), &[6, 0]);

    let zero_quote = PortfolioOrderV1::new(PortfolioOrderV1Input {
        max_quote_debit_per_lot_numerator: -1,
        ..PortfolioOrderV1Input {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            owner: owner(20),
            order_id: id(11),
            generation: 7,
            batch_sequence: 0,
            nonce: 10,
            valid_until_slot: 30,
            max_lots: 3,
            max_quote_debit_per_lot_numerator: 0,
            coefficients: [-2, 3],
            outcome_count: 2,
        }
    })
    .expect("order");
    assert_eq!(
        zero_quote
            .worst_case_reserve(config())
            .expect("reserve")
            .quote_atoms(),
        0
    );

    let claim_overflow = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(12),
        generation: 7,
        batch_sequence: 0,
        nonce: 11,
        valid_until_slot: 30,
        max_lots: 2,
        max_quote_debit_per_lot_numerator: 0,
        coefficients: [i64::MIN, 1],
        outcome_count: 2,
    })
    .expect("order");
    assert_eq!(
        claim_overflow.worst_case_reserve(config()),
        Err(Error::TokenAmountOutOfRange)
    );

    let quote_overflow = PortfolioOrderV1::new(PortfolioOrderV1Input {
        max_lots: u64::MAX,
        max_quote_debit_per_lot_numerator: i128::MAX,
        coefficients: [-1, 1],
        order_id: id(13),
        nonce: 12,
        ..PortfolioOrderV1Input {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            owner: owner(20),
            order_id: id(13),
            generation: 7,
            batch_sequence: 0,
            nonce: 12,
            valid_until_slot: 30,
            max_lots: 1,
            max_quote_debit_per_lot_numerator: 0,
            coefficients: [-1, 1],
            outcome_count: 2,
        }
    })
    .expect("order");
    assert_eq!(
        quote_overflow.worst_case_reserve(config()),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn exact_n_order_custody_round_trips_and_rejects_hostile_geometry() {
    assert_eq!(GeneralOrderCustodyV1::<2>::encoded_len(), Ok(208));
    assert_eq!(GeneralOrderCustodyV1::<16>::encoded_len(), Ok(320));
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let mut bytes = [0; 208];
    admission
        .custody
        .encode(&mut bytes)
        .expect("custody encodes");
    assert_eq!(&bytes[..8], b"DCLTGOC1");
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&bytes),
        Ok(admission.custody)
    );
    assert_eq!(
        GeneralOrderCustodyV1::<16>::decode(&bytes),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&bytes[..207]),
        Err(Error::InvalidLength)
    );
    let mut trailing = [0; 209];
    trailing[..208].copy_from_slice(&bytes);
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&trailing),
        Err(Error::InvalidLength)
    );
    let mut changed = bytes;
    changed[14] = 1;
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut changed = bytes;
    changed[80..112].fill(0);
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::ZeroIdentifier)
    );
    let mut changed = bytes;
    changed[12..14].copy_from_slice(&16_u16.to_le_bytes());
    assert_eq!(
        GeneralOrderCustodyV1::<2>::decode(&changed),
        Err(Error::InvalidOutcomeCount)
    );
}

#[test]
fn custody_admission_receipt_cancel_and_post_batch_close_are_atomic() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    assert_eq!(admission.reserve.quote_atoms(), 4);
    assert_eq!(admission.reserve.claim_atoms(), &[6, 0]);
    assert_eq!(admission.order_state.phase(), OrderPhase::Open);

    let mut state = admission.order_state;
    let mut custody = admission.custody;
    let receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: order.order_id(),
        owner: order.owner(),
        generation: order.generation(),
        batch_sequence: order.batch_sequence(),
        nonce: order.nonce(),
        fill_lots: 1,
        remaining_lots: 2,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    let effects = custody
        .apply_receipt(&mut state, order, receipt, config())
        .expect("receipt applies");
    assert_eq!(effects.quote_debit_from_escrow(), 1);
    assert_eq!(effects.quote_credit_to_owner(), 0);
    assert_eq!(effects.claim_debits_from_custody(), &[2, 0]);
    assert_eq!(effects.claim_credits_to_owner(), &[0, 3]);
    assert_eq!(state.remaining_lots(), 2);
    assert_eq!(custody.reserved_quote_atoms(), 3);
    assert_eq!(custody.reserved_claim_atoms(), &[4, 0]);

    let mut batch = BatchRootV1::open(id(9), 0, 0, config()).expect("batch");
    batch.open_selection(10).expect("selecting");
    batch.close_selection(20).expect("quiescent");
    let release = custody
        .close_after_batch(&mut state, order, batch, config())
        .expect("partial remainder closes");
    assert_eq!(state.phase(), OrderPhase::Released);
    assert_eq!(release.quote_atoms, 3);
    assert_eq!(release.claim_atoms, [4, 0]);
    assert_eq!(release.owner, owner(20));
    assert_eq!(release.rent_beneficiary, [70; 32]);
    assert_eq!(release.quote_escrow, [71; 32]);

    let cancelled_admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let mut cancelled_state = cancelled_admission.order_state;
    let before = cancelled_state;
    assert_eq!(
        cancelled_admission.custody.cancel_and_release(
            &mut cancelled_state,
            order,
            owner(21),
            9,
            10,
            config(),
        ),
        Err(Error::AuthorityMismatch)
    );
    assert_eq!(cancelled_state, before);
    let cancel_release = cancelled_admission
        .custody
        .cancel_and_release(&mut cancelled_state, order, owner(20), 9, 10, config())
        .expect("cancel releases");
    assert_eq!(cancelled_state.phase(), OrderPhase::Cancelled);
    assert_eq!(cancel_release.quote_atoms, 4);
    assert_eq!(cancel_release.claim_atoms, [6, 0]);

    let consumed_order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(11),
        generation: 7,
        batch_sequence: 0,
        nonce: 10,
        valid_until_slot: 30,
        max_lots: 1,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("one-lot order");
    let consumed_admission =
        GeneralOrderCustodyV1::admit(consumed_order, config(), [70; 32], [71; 32])
            .expect("admission");
    let mut consumed_state = consumed_admission.order_state;
    let mut consumed_custody = consumed_admission.custody;
    let consumed_receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: consumed_order.order_id(),
        owner: consumed_order.owner(),
        generation: consumed_order.generation(),
        batch_sequence: consumed_order.batch_sequence(),
        nonce: consumed_order.nonce(),
        fill_lots: 1,
        remaining_lots: 0,
        quote_delta_atoms: -1,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    consumed_custody
        .apply_receipt(
            &mut consumed_state,
            consumed_order,
            consumed_receipt,
            config(),
        )
        .expect("full receipt applies");
    assert_eq!(consumed_state.phase(), OrderPhase::Consumed);
    let consumed_release = consumed_custody
        .close_after_batch(&mut consumed_state, consumed_order, batch, config())
        .expect("fully consumed custody closes");
    assert_eq!(consumed_state.phase(), OrderPhase::Consumed);
    assert_eq!(consumed_release.quote_atoms, 1);
    assert_eq!(consumed_release.claim_atoms, [0, 0]);
}

#[test]
fn hostile_receipt_and_underfunded_custody_leave_replay_and_reserves_unchanged() {
    let order = PortfolioOrderV1::new(PortfolioOrderV1Input {
        market_identity_id: id(2),
        claim_basis_id: id(3),
        owner: owner(20),
        order_id: id(10),
        generation: 7,
        batch_sequence: 0,
        nonce: 9,
        valid_until_slot: 30,
        max_lots: 3,
        max_quote_debit_per_lot_numerator: 101,
        coefficients: [-2, 3],
        outcome_count: 2,
    })
    .expect("order");
    let admission =
        GeneralOrderCustodyV1::admit(order, config(), [70; 32], [71; 32]).expect("admission");
    let receipt = SettlementReceiptV1 {
        candidate_id: id(40),
        order_id: order.order_id(),
        owner: order.owner(),
        generation: order.generation(),
        batch_sequence: order.batch_sequence(),
        nonce: order.nonce(),
        fill_lots: 1,
        remaining_lots: 2,
        quote_delta_atoms: -5,
        carry_before: 0,
        carry_after: 0,
        outcome_deltas: [-2, 3],
        outcome_count: 2,
    };
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    let before_state = state;
    let before_custody = custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, receipt, config()),
        Err(Error::InsufficientCustody)
    );
    assert_eq!(state, before_state);
    assert_eq!(custody, before_custody);

    let mut bytes = [0; 208];
    admission
        .custody
        .encode(&mut bytes)
        .expect("custody encodes");
    bytes[192..200].copy_from_slice(&1_u64.to_le_bytes());
    let mut underfunded = GeneralOrderCustodyV1::<2>::decode(&bytes).expect("shaped custody");
    let mut state = admission.order_state;
    let claim_receipt = SettlementReceiptV1 {
        quote_delta_atoms: -1,
        ..receipt
    };
    let before_state = state;
    let before_custody = underfunded;
    assert_eq!(
        underfunded.apply_receipt(&mut state, order, claim_receipt, config()),
        Err(Error::InsufficientCustody)
    );
    assert_eq!(state, before_state);
    assert_eq!(underfunded, before_custody);

    let mut substituted = receipt;
    substituted.owner = owner(21);
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, substituted, config()),
        Err(Error::CustodyMismatch)
    );
    assert_eq!(state, admission.order_state);
    assert_eq!(custody, admission.custody);

    let mut wrong_width = receipt;
    wrong_width.outcome_count = 16;
    let mut state = admission.order_state;
    let mut custody = admission.custody;
    assert_eq!(
        custody.apply_receipt(&mut state, order, wrong_width, config()),
        Err(Error::CustodyMismatch)
    );
    assert_eq!(state, admission.order_state);
    assert_eq!(custody, admission.custody);
}
