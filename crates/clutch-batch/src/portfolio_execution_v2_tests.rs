use super::portfolio_execution_v2::{
    authenticate_exact_portfolio_pair_v2, authenticate_selected_portfolio_order_v2,
    portfolio_pair_transition_commitment_v2, prepare_portfolio_pair_execution_v2,
    AuthenticatedPortfolioPairV2, PortfolioAccountExpectationV2, PortfolioAccountRoleV2,
    PortfolioAdapterV2, PortfolioExecutionErrorV2, PortfolioPairExecutionInputV2,
    PortfolioPairPostSemanticIdsV2, PortfolioPositionPrestateV2,
    PortfolioReplayPrestateV2, PortfolioReservationLifecycleV2,
    PortfolioReservationPrestateV2, PortfolioSourceOrderKindV2,
    PortfolioSelectionMembershipExpectationV2, PortfolioSettlementReceiptV5Prestate,
    PortfolioSettlementReceiptV5TransitionExpectationV2, PortfolioTransitionExpectationV2,
    PortfolioValuationBoundaryV2,
    SettlementReceiptTransitionKindV2,
    SelectedPortfolioOrderRecordV2, PORTFOLIO_EXECUTION_VERSION_V2,
    PORTFOLIO_PAIR_RECEIPT_V2_BYTES, SELECTED_PORTFOLIO_ORDER_V2_BYTES,
};
use super::relation_v1::MAX_OUTCOMES;
use super::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2,
    EconomicCandidateV2, EconomicDomainV2, EconomicOrderV2, PricePreconditionV2,
    EMPTY_ECONOMIC_ORDER_V2, ECONOMIC_RELATION_VERSION_V2,
};
use super::{PartialPolicy, Side, MAX_ORDERS};
use std::cell::Cell;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[derive(Clone, Copy)]
struct TestAdapter {
    reject_account: Option<PortfolioAccountRoleV2>,
    reject_transition: Option<PortfolioAccountRoleV2>,
}

impl TestAdapter {
    const ACCEPT: Self = Self {
        reject_account: None,
        reject_transition: None,
    };
}

impl PortfolioAdapterV2 for TestAdapter {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool {
        self.reject_account != Some(expected.role)
    }

    fn authenticate_selection_membership(
        &self,
        _expected: &PortfolioSelectionMembershipExpectationV2,
        _relation_order: &EconomicOrderV2,
        _candidate: &EconomicCandidateV2,
    ) -> bool {
        self.reject_account != Some(PortfolioAccountRoleV2::OrderPage)
    }

    fn authenticate_transition(&self, expected: &PortfolioTransitionExpectationV2) -> bool {
        self.reject_transition != Some(expected.role)
    }

    fn derive_settlement_receipt_v5_post_data_id(
        &self,
        _expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[u8; 32]> {
        if self.reject_transition == Some(PortfolioAccountRoleV2::SettlementReceipt) {
            None
        } else {
            Some(id(66))
        }
    }
}

struct CapturingAdapter {
    commitment: Cell<[u8; 32]>,
}

impl PortfolioAdapterV2 for CapturingAdapter {
    fn authenticate_account(&self, _expected: &PortfolioAccountExpectationV2) -> bool {
        true
    }

    fn authenticate_selection_membership(
        &self,
        _expected: &PortfolioSelectionMembershipExpectationV2,
        _relation_order: &EconomicOrderV2,
        _candidate: &EconomicCandidateV2,
    ) -> bool {
        true
    }

    fn authenticate_transition(&self, _expected: &PortfolioTransitionExpectationV2) -> bool {
        true
    }

    fn derive_settlement_receipt_v5_post_data_id(
        &self,
        expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[u8; 32]> {
        if expected.prestate.transition_kind
            != SettlementReceiptTransitionKindV2::PortfolioPairV2
            || expected.prestate.transition_commitment != [0; 32]
            || expected.prestate.accounted_end_mask != expected.prestate.expected_end_mask
            || expected.prestate.delivered_end_mask != 0
            || expected.post_transition_kind
                != SettlementReceiptTransitionKindV2::PortfolioPairV2
            || expected.transition_commitment == [0; 32]
        {
            return None;
        }
        self.commitment.set(expected.transition_commitment);
        Some(id(66))
    }
}

#[derive(Clone, Copy)]
struct PairFixture {
    domain: EconomicDomainV2,
    book: EconomicBookV2,
    price: PricePreconditionV2,
    candidate: EconomicCandidateV2,
    buyer_record: SelectedPortfolioOrderRecordV2,
    seller_record: SelectedPortfolioOrderRecordV2,
    candidate_digest: [u8; 32],
    payoff: [u64; MAX_OUTCOMES],
}

fn pair_fixture(quantity: u64) -> PairFixture {
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: id(1),
        epoch_semantics_digest: id(2),
        relation_policy_digest: id(3),
        price_policy_digest: id(4),
        epoch_index: 7,
        outcome_count: u8::try_from(MAX_OUTCOMES).unwrap(),
        price_scale: 10_000,
    };
    let prices = [625u64; MAX_OUTCOMES];
    let price = PricePreconditionV2 {
        policy_digest: domain.price_policy_digest,
        semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
        prices,
    };
    let mut coefficients = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        coefficients[outcome] = u64::try_from(outcome).unwrap() + 1;
        outcome += 1;
    }
    let buy = EconomicOrderV2 {
        order_id: id(10),
        side: Side::Buy,
        coefficients,
        quantity,
        minimum_fill: quantity,
        partial_policy: PartialPolicy::AllOrNone,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: 85_000,
    };
    let sell = EconomicOrderV2 {
        order_id: id(11),
        side: Side::Sell,
        coefficients,
        quantity,
        minimum_fill: quantity,
        partial_policy: PartialPolicy::AllOrNone,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: 85_000,
    };
    let mut orders = [EMPTY_ECONOMIC_ORDER_V2; MAX_ORDERS];
    orders[0] = buy;
    orders[1] = sell;
    let book = EconomicBookV2 { orders, len: 2 };
    let mut fills = [0u64; MAX_ORDERS];
    fills[0] = quantity;
    fills[1] = quantity;
    let candidate = EconomicCandidateV2 {
        fills,
        honored_aon_mask: 0b11,
        virtual_split: 0,
        virtual_merge: 0,
    };
    let verified = verify_economic_candidate_v2(&domain, &book, &price, &candidate).unwrap();
    let candidate_digest = verified.economic_candidate_digest;
    let common = SelectedPortfolioOrderRecordV2 {
        version: PORTFOLIO_EXECUTION_VERSION_V2,
        outcome_count: u8::try_from(MAX_OUTCOMES).unwrap(),
        source_kind: PortfolioSourceOrderKindV2::Portfolio,
        side: Side::Buy,
        order_index: 0,
        page_slot: 3,
        traversal_index: 10,
        page_index: 9,
        settlement_root_epoch_generation: 12,
        position_generation: 4,
        selected_fill_units: quantity,
        market_semantics_digest: domain.market_semantics_digest,
        epoch_semantics_digest: domain.epoch_semantics_digest,
        economic_candidate_digest: candidate_digest,
        order_set_digest: id(20),
        settlement_root_account_id: id(21),
        settlement_root_pre_semantic_id: id(22),
        settlement_candidate_id: id(23),
        retained_feed_account_id: id(24),
        retained_feed_semantic_id: id(25),
        settlement_witness_id: id(26),
        order_page_account_id: id(27),
        order_page_semantic_id: id(28),
        position_account_id: id(30),
        position_pre_semantic_id: id(31),
        order_id: buy.order_id,
        owner_id: id(32),
    };
    let seller_record = SelectedPortfolioOrderRecordV2 {
        side: Side::Sell,
        order_index: 1,
        page_slot: 4,
        position_generation: 8,
        position_account_id: id(33),
        position_pre_semantic_id: id(34),
        order_id: sell.order_id,
        owner_id: id(35),
        ..common
    };
    let mut payoff = [0u64; MAX_OUTCOMES];
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        payoff[outcome] = coefficients[outcome] * quantity;
        outcome += 1;
    }
    PairFixture {
        domain,
        book,
        price,
        candidate,
        buyer_record: common,
        seller_record,
        candidate_digest,
        payoff,
    }
}

fn zero_consideration_fixture(quantity: u64) -> PairFixture {
    let mut fixture = pair_fixture(quantity);
    let mut coefficients = [0u64; MAX_OUTCOMES];
    coefficients[1] = 1;
    fixture.book.orders[0].coefficients = coefficients;
    fixture.book.orders[1].coefficients = coefficients;
    fixture.book.orders[0].limit_value_price_units_per_unit = 0;
    fixture.book.orders[1].limit_value_price_units_per_unit = 0;
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[0] = fixture.domain.price_scale;
    fixture.price = PricePreconditionV2 {
        policy_digest: fixture.domain.price_policy_digest,
        semantic_price_digest: price_semantics_digest_v2(&fixture.domain, &prices).unwrap(),
        prices,
    };
    let verified = verify_economic_candidate_v2(
        &fixture.domain,
        &fixture.book,
        &fixture.price,
        &fixture.candidate,
    )
    .unwrap();
    fixture.candidate_digest = verified.economic_candidate_digest;
    fixture.buyer_record.economic_candidate_digest = verified.economic_candidate_digest;
    fixture.seller_record.economic_candidate_digest = verified.economic_candidate_digest;
    fixture.payoff = [0; MAX_OUTCOMES];
    fixture.payoff[1] = quantity;
    fixture
}

fn authenticated_pair(fixture: &PairFixture) -> AuthenticatedPortfolioPairV2 {
    let buyer = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .unwrap();
    let seller = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.seller_record,
    )
    .unwrap();
    authenticate_exact_portfolio_pair_v2(
        &fixture.domain,
        &fixture.book,
        &fixture.price,
        &fixture.candidate,
        PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
        buyer,
        seller,
    )
    .unwrap()
}

fn execution_input(fixture: &PairFixture) -> PortfolioPairExecutionInputV2 {
    PortfolioPairExecutionInputV2 {
        settlement_receipt: PortfolioSettlementReceiptV5Prestate {
            account_id: id(80),
            pre_data_id: id(81),
            slice_index: 10,
            sequence: 11,
            accounted_end_mask: 3,
            delivered_end_mask: 0,
            expected_end_mask: 3,
            transition_kind: SettlementReceiptTransitionKindV2::PortfolioPairV2,
            transition_commitment: [0; 32],
            rent_owner_id: id(82),
            rent_principal_lamports: 2_000_000,
            rent_donation_floor_lamports: 17,
        },
        buyer_reservation: PortfolioReservationPrestateV2 {
            account_id: id(40),
            semantic_id: id(41),
            generation: 3,
            lifecycle: PortfolioReservationLifecycleV2::Entitled,
            owner_id: fixture.buyer_record.owner_id,
            order_id: fixture.buyer_record.order_id,
            position_account_id: fixture.buyer_record.position_account_id,
            position_generation: fixture.buyer_record.position_generation,
            remaining_cash_atoms: 200,
            remaining_claim_atoms: [0; MAX_OUTCOMES],
            maximum_fee_atoms: 0,
        },
        seller_reservation: PortfolioReservationPrestateV2 {
            account_id: id(42),
            semantic_id: id(43),
            generation: 6,
            lifecycle: PortfolioReservationLifecycleV2::Entitled,
            owner_id: fixture.seller_record.owner_id,
            order_id: fixture.seller_record.order_id,
            position_account_id: fixture.seller_record.position_account_id,
            position_generation: fixture.seller_record.position_generation,
            remaining_cash_atoms: 0,
            remaining_claim_atoms: fixture.payoff,
            maximum_fee_atoms: 0,
        },
        buyer_position: PortfolioPositionPrestateV2 {
            account_id: fixture.buyer_record.position_account_id,
            semantic_id: fixture.buyer_record.position_pre_semantic_id,
            owner_id: fixture.buyer_record.owner_id,
            generation: fixture.buyer_record.position_generation,
            cash_atoms: 500,
            reserved_cash_atoms: 200,
            native_eggs: [0; MAX_OUTCOMES],
            outstanding_reservations: 1,
        },
        seller_position: PortfolioPositionPrestateV2 {
            account_id: fixture.seller_record.position_account_id,
            semantic_id: fixture.seller_record.position_pre_semantic_id,
            owner_id: fixture.seller_record.owner_id,
            generation: fixture.seller_record.position_generation,
            cash_atoms: 10,
            reserved_cash_atoms: 0,
            native_eggs: [0; MAX_OUTCOMES],
            outstanding_reservations: 1,
        },
        buyer_replay: PortfolioReplayPrestateV2 {
            account_id: id(50),
            semantic_id: id(51),
            ordinal: 4,
        },
        seller_replay: PortfolioReplayPrestateV2 {
            account_id: id(52),
            semantic_id: id(53),
            ordinal: 8,
        },
        post_semantic_ids: PortfolioPairPostSemanticIdsV2 {
            buyer_reservation: id(60),
            seller_reservation: id(61),
            buyer_position: id(62),
            seller_position: id(63),
            buyer_replay: id(64),
            seller_replay: id(65),
            settlement_receipt: [0; 32],
        },
    }
}

#[test]
fn full_sixteen_outcome_pair_values_once_and_freezes_exact_effects() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    assert_eq!(pair.unit_value_price_units(), 85_000);
    assert_eq!(pair.total_value_price_units(), 1_700_000);
    assert_eq!(pair.consideration_atoms(), 170);
    assert_eq!(pair.payoff(), &fixture.payoff);

    let prepared = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        execution_input(&fixture),
    )
    .unwrap();
    assert_eq!(prepared.effects().buyer_cash_debit_atoms(), 170);
    assert_eq!(prepared.effects().buyer_cash_refund_atoms(), 30);
    assert_eq!(prepared.effects().seller_cash_credit_atoms(), 170);
    assert_eq!(prepared.effects().claim_debits(), &fixture.payoff);
    assert_eq!(prepared.effects().claim_credits(), &fixture.payoff);
    assert_eq!(prepared.buyer_position_after().cash_atoms(), 330);
    assert_eq!(prepared.buyer_position_after().reserved_cash_atoms(), 0);
    assert_eq!(prepared.buyer_position_after().native_eggs(), &fixture.payoff);
    assert_eq!(prepared.buyer_position_after().generation(), 4);
    assert_eq!(prepared.buyer_position_after().outstanding_reservations(), 1);
    assert_eq!(prepared.seller_position_after().cash_atoms(), 180);
    assert_eq!(prepared.seller_position_after().generation(), 8);
    assert_eq!(prepared.receipt().consideration_atoms(), 170);
    assert_eq!(prepared.receipt().slice_index(), 10);
    assert_eq!(prepared.receipt().sequence(), 11);
}

#[test]
fn receipt_v5_sets_one_typed_commitment_in_the_authenticated_postimage() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let adapter = CapturingAdapter {
        commitment: Cell::new([0; 32]),
    };
    let prepared = prepare_portfolio_pair_execution_v2(
        &adapter,
        id(200),
        pair,
        execution_input(&fixture),
    )
    .unwrap();
    assert_eq!(adapter.commitment.get(), prepared.transition_commitment());
    assert_eq!(prepared.post_semantic_ids().settlement_receipt, id(66));
}

#[test]
fn receipt_v5_post_data_identity_is_never_a_caller_fact() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let mut input = execution_input(&fixture);
    input.post_semantic_ids.settlement_receipt = id(66);
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, input),
        Err(PortfolioExecutionErrorV2::PostSemanticMismatch)
    );
}

#[test]
fn unchanged_position_endpoint_keeps_its_exact_semantic_identity() {
    let fixture = zero_consideration_fixture(20);
    let pair = authenticated_pair(&fixture);
    assert_eq!(pair.consideration_atoms(), 0);
    let mut input = execution_input(&fixture);
    input.post_semantic_ids.seller_position = input.seller_position.semantic_id;
    prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, input).unwrap();

    let mut invented_post = input;
    invented_post.post_semantic_ids.seller_position = id(63);
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            pair,
            invented_post,
        ),
        Err(PortfolioExecutionErrorV2::PostSemanticMismatch)
    );
}

#[test]
fn selected_membership_and_receipt_codecs_refuse_noncanonical_padding() {
    let fixture = pair_fixture(20);
    let mut selected = [0u8; SELECTED_PORTFOLIO_ORDER_V2_BYTES];
    fixture.buyer_record.encode_into(&mut selected).unwrap();
    assert_eq!(
        SelectedPortfolioOrderRecordV2::decode(&selected).unwrap(),
        fixture.buyer_record
    );
    selected[18] = 1;
    assert_eq!(
        SelectedPortfolioOrderRecordV2::decode(&selected),
        Err(PortfolioExecutionErrorV2::NonCanonicalPadding)
    );

    let pair = authenticated_pair(&fixture);
    let prepared = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        execution_input(&fixture),
    )
    .unwrap();
    let mut receipt = [0u8; PORTFOLIO_PAIR_RECEIPT_V2_BYTES];
    prepared.receipt().encode_into(&mut receipt).unwrap();
    let decoded = super::portfolio_execution_v2::PortfolioPairReceiptV2::decode(&receipt).unwrap();
    assert_eq!(
        portfolio_pair_transition_commitment_v2(&decoded).unwrap(),
        prepared.transition_commitment()
    );
    receipt[11] = 1;
    assert_eq!(
        super::portfolio_execution_v2::PortfolioPairReceiptV2::decode(&receipt),
        Err(PortfolioExecutionErrorV2::NonCanonicalPadding)
    );
}

#[test]
fn valuation_refuses_a_remainder_instead_of_rounding_per_leg_or_per_order() {
    let fixture = pair_fixture(1);
    let buyer = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .unwrap();
    let seller = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.seller_record,
    )
    .unwrap();
    assert_eq!(
        authenticate_exact_portfolio_pair_v2(
            &fixture.domain,
            &fixture.book,
            &fixture.price,
            &fixture.candidate,
            PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
            buyer,
            seller,
        ),
        Err(PortfolioExecutionErrorV2::InexactValuation)
    );
}

#[test]
fn every_coefficient_cell_is_pair_authority_not_a_display_projection() {
    let fixture = pair_fixture(20);
    let buyer = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .unwrap();
    let seller = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.seller_record,
    )
    .unwrap();
    let mut mutated_book = fixture.book;
    mutated_book.orders[1].coefficients[MAX_OUTCOMES - 1] += 1;
    assert_eq!(
        authenticate_exact_portfolio_pair_v2(
            &fixture.domain,
            &mutated_book,
            &fixture.price,
            &fixture.candidate,
            PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
            buyer,
            seller,
        ),
        Err(PortfolioExecutionErrorV2::CoefficientMismatch)
    );
}

#[test]
fn reservation_funding_fee_and_claim_mutants_refuse_before_authority() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let base = execution_input(&fixture);

    let mut fee = base;
    fee.buyer_reservation.maximum_fee_atoms = 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, fee),
        Err(PortfolioExecutionErrorV2::ReservationFeeUnsupported)
    );

    let mut claim = base;
    claim.seller_reservation.remaining_claim_atoms[MAX_OUTCOMES - 1] -= 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, claim),
        Err(PortfolioExecutionErrorV2::ReservationMismatch)
    );

    let mut underfunded = base;
    underfunded.buyer_reservation.remaining_cash_atoms = 169;
    underfunded.buyer_position.reserved_cash_atoms = 169;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            pair,
            underfunded,
        ),
        Err(PortfolioExecutionErrorV2::BuyerReservationUnderfunded)
    );
}

#[test]
fn receipt_v5_requires_pending_kind_accounting_and_zero_precommitment() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let base = execution_input(&fixture);

    let mut sequence = base;
    sequence.settlement_receipt.sequence += 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, sequence),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut wrong_kind = base;
    wrong_kind.settlement_receipt.transition_kind = SettlementReceiptTransitionKindV2::None;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, wrong_kind),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut unaccounted = base;
    unaccounted.settlement_receipt.accounted_end_mask = 0;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, unaccounted),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut delivered = base;
    delivered.settlement_receipt.delivered_end_mask = 3;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, delivered),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut replay = base;
    replay.settlement_receipt.transition_commitment = id(99);
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, replay),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut wrong_traversal = base;
    wrong_traversal.settlement_receipt.slice_index = 9;
    wrong_traversal.settlement_receipt.sequence = 10;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            pair,
            wrong_traversal,
        ),
        Err(PortfolioExecutionErrorV2::FeedTraversalMismatch)
    );
}

#[test]
fn adapter_refusal_cannot_be_repackaged_as_a_private_capability() {
    let fixture = pair_fixture(20);
    let rejecting_selection = TestAdapter {
        reject_account: Some(PortfolioAccountRoleV2::OrderPage),
        reject_transition: None,
    };
    assert_eq!(
        authenticate_selected_portfolio_order_v2(
            &rejecting_selection,
            id(200),
            &fixture.domain,
            &fixture.book,
            &fixture.candidate,
            fixture.candidate_digest,
            fixture.buyer_record,
        ),
        Err(PortfolioExecutionErrorV2::AuthenticationFailed {
            role: PortfolioAccountRoleV2::OrderPage,
        })
    );

    let pair = authenticated_pair(&fixture);
    let rejecting_transition = TestAdapter {
        reject_account: None,
        reject_transition: Some(PortfolioAccountRoleV2::Replay),
    };
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &rejecting_transition,
            id(200),
            pair,
            execution_input(&fixture),
        ),
        Err(PortfolioExecutionErrorV2::TransitionAuthenticationFailed {
            role: PortfolioAccountRoleV2::Replay,
        })
    );

    let rejecting_receipt = TestAdapter {
        reject_account: None,
        reject_transition: Some(PortfolioAccountRoleV2::SettlementReceipt),
    };
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &rejecting_receipt,
            id(200),
            pair,
            execution_input(&fixture),
        ),
        Err(PortfolioExecutionErrorV2::TransitionAuthenticationFailed {
            role: PortfolioAccountRoleV2::SettlementReceipt,
        })
    );
}

#[test]
fn replay_prestate_is_in_the_transition_and_receipt_identity() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let first = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        execution_input(&fixture),
    )
    .unwrap();

    let mut replayed = execution_input(&fixture);
    replayed.buyer_replay.ordinal += 1;
    replayed.buyer_replay.semantic_id = id(90);
    replayed.post_semantic_ids.buyer_replay = id(91);
    let second = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        replayed,
    )
    .unwrap();
    assert_ne!(first.receipt().transition_id(), second.receipt().transition_id());
    assert_ne!(first.transition_commitment(), second.transition_commitment());

    let mut overflow = execution_input(&fixture);
    overflow.buyer_replay.ordinal = u64::MAX;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, overflow),
        Err(PortfolioExecutionErrorV2::ReplayOverflow)
    );
}

#[test]
fn inactive_candidate_fills_and_nonexclusive_active_pairs_refuse() {
    let fixture = pair_fixture(20);
    let buyer = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .unwrap();
    let seller = authenticate_selected_portfolio_order_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.seller_record,
    )
    .unwrap();
    let mut padded = fixture.candidate;
    padded.fills[MAX_ORDERS - 1] = 1;
    assert!(matches!(
        authenticate_exact_portfolio_pair_v2(
            &fixture.domain,
            &fixture.book,
            &fixture.price,
            &padded,
            PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
            buyer,
            seller,
        ),
        Err(PortfolioExecutionErrorV2::Economic(_))
    ));
}
