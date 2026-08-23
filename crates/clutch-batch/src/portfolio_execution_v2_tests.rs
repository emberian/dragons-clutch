use super::portfolio_execution_v2::{
    authenticate_exact_portfolio_pair_v2, authenticate_portfolio_receipt_sibling_set_v2,
    authenticate_selected_portfolio_order_for_materialization_v2,
    authenticate_selected_portfolio_order_v2, portfolio_pair_transition_commitment_v2,
    prepare_portfolio_pair_execution_borrowed_v2, prepare_portfolio_pair_execution_v2,
    AuthenticatedPortfolioPairV2, PortfolioAccountExpectationV2, PortfolioAccountRoleV2,
    PortfolioAdapterV2, PortfolioExecutionErrorV2, PortfolioPairExecutionInputV2,
    PortfolioPairPostSemanticIdsV2, PortfolioPositionPrestateV2,
    PortfolioReceiptSiblingTraversalSetV2, PortfolioReceiptSiblingTraversalV2,
    PortfolioReplayPrestateV2, PortfolioReservationLifecycleV2,
    PortfolioReservationPrestateV2, PortfolioSourceOrderKindV2,
    PortfolioSelectionMembershipExpectationV2, PortfolioSettlementReceiptV5Prestate,
    PortfolioSettlementReceiptV5SetPrestate,
    PortfolioSettlementReceiptV5TransitionExpectationV2, PortfolioTransitionExpectationV2,
    PortfolioValuationBoundaryV2,
    SettlementReceiptTransitionKindV2,
    SelectedPortfolioOrderRecordV2, PORTFOLIO_EXECUTION_VERSION_V2,
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2, PORTFOLIO_PAIR_RECEIPT_V2_BYTES,
    SELECTED_PORTFOLIO_ORDER_V2_BYTES,
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

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        _expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[[u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        if self.reject_transition == Some(PortfolioAccountRoleV2::SettlementReceipt) {
            None
        } else {
            let mut ids = [[0u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
            let mut index = 0usize;
            while index < usize::from(_expected.prestate.receipt_count) {
                ids[index] = id(66u8.checked_add(u8::try_from(index).ok()?).ok()?);
                index += 1;
            }
            Some(ids)
        }
    }
}

#[derive(Clone, Copy)]
struct AccessContractAdapter {
    materialization: bool,
}

impl PortfolioAdapterV2 for AccessContractAdapter {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool {
        match expected.role {
            PortfolioAccountRoleV2::SettlementRoot => {
                expected.writable == self.materialization
            }
            PortfolioAccountRoleV2::Position => {
                expected.writable != self.materialization
            }
            PortfolioAccountRoleV2::RetainedFeed | PortfolioAccountRoleV2::OrderPage => {
                !expected.writable
            }
            PortfolioAccountRoleV2::Reservation
            | PortfolioAccountRoleV2::Replay
            | PortfolioAccountRoleV2::SettlementReceipt => false,
        }
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
        false
    }

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        _expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[[u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        None
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

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[[u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        let entry = expected.prestate.receipts[0];
        if entry.transition_kind != SettlementReceiptTransitionKindV2::PortfolioPairV2
            || entry.transition_commitment != [0; 32]
            || entry.accounted_end_mask != entry.expected_end_mask
            || entry.delivered_end_mask != 0
            || expected.post_transition_kind
                != SettlementReceiptTransitionKindV2::PortfolioPairV2
            || expected.transition_commitment == [0; 32]
        {
            return None;
        }
        self.commitment.set(expected.transition_commitment);
        let mut ids = [[0u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
        let mut index = 0usize;
        while index < usize::from(expected.prestate.receipt_count) {
            ids[index] = id(66u8.checked_add(u8::try_from(index).ok()?).ok()?);
            index += 1;
        }
        Some(ids)
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
    let mut receipts =
        [PortfolioSettlementReceiptV5Prestate::EMPTY; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
    let mut receipt_count = 0usize;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        if fixture.payoff[outcome] != 0 {
            let at = receipt_count;
            let slice_index = 10u16.checked_add(u16::try_from(at).unwrap()).unwrap();
            receipts[at] = PortfolioSettlementReceiptV5Prestate {
                account_id: id(80u8.checked_add(u8::try_from(at).unwrap()).unwrap()),
                pre_data_id: id(100u8.checked_add(u8::try_from(at).unwrap()).unwrap()),
                slice_index,
                sequence: u64::from(slice_index).checked_add(1).unwrap(),
                outcome: u8::try_from(outcome).unwrap(),
                quantity: fixture.payoff[outcome],
                price: fixture.price.prices[outcome],
                accounted_end_mask: 3,
                delivered_end_mask: 0,
                expected_end_mask: 3,
                transition_kind: SettlementReceiptTransitionKindV2::PortfolioPairV2,
                transition_commitment: [0; 32],
                rent_owner_id: id(82),
                rent_principal_lamports: 2_000_000,
                rent_donation_floor_lamports: 17,
            };
            receipt_count += 1;
        }
        outcome += 1;
    }
    PortfolioPairExecutionInputV2 {
        settlement_receipts: PortfolioSettlementReceiptV5SetPrestate {
            receipt_count: u8::try_from(receipt_count).unwrap(),
            receipts,
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
            entitled_units: fixture.buyer_record.selected_fill_units,
            consumed_units: 0,
            paid_units: 0,
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
            entitled_units: fixture.seller_record.selected_fill_units,
            consumed_units: 0,
            paid_units: 0,
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
            settlement_receipts: [[0; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
        },
    }
}

fn sibling_traversal(
    fixture: &PairFixture,
    input: &PortfolioPairExecutionInputV2,
) -> PortfolioReceiptSiblingTraversalSetV2 {
    let mut siblings =
        [PortfolioReceiptSiblingTraversalV2::EMPTY; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
    let mut index = 0usize;
    while index < usize::from(input.settlement_receipts.receipt_count) {
        let receipt = input.settlement_receipts.receipts[index];
        siblings[index] = PortfolioReceiptSiblingTraversalV2 {
            slice_index: receipt.slice_index,
            sequence: receipt.sequence,
            buy_order_index: fixture.buyer_record.order_index,
            sell_order_index: fixture.seller_record.order_index,
            outcome: receipt.outcome,
            quantity: receipt.quantity,
            price: receipt.price,
        };
        index += 1;
    }
    PortfolioReceiptSiblingTraversalSetV2 {
        sibling_count: input.settlement_receipts.receipt_count,
        siblings,
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
    let borrowed_input = execution_input(&fixture);
    let borrowed = prepare_portfolio_pair_execution_borrowed_v2(
        &TestAdapter::ACCEPT,
        id(200),
        &pair,
        &borrowed_input,
    )
    .unwrap();
    assert_eq!(borrowed, prepared);
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
    assert_eq!(prepared.post_semantic_ids().settlement_receipts[0], id(66));
    assert_eq!(prepared.receipt().receipt_count(), 16);
}

#[test]
fn receipt_v5_post_data_identity_is_never_a_caller_fact() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let mut input = execution_input(&fixture);
    input.post_semantic_ids.settlement_receipts[0] = id(66);
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
    receipt[14] = 1;
    assert_eq!(
        super::portfolio_execution_v2::PortfolioPairReceiptV2::decode(&receipt),
        Err(PortfolioExecutionErrorV2::NonCanonicalPadding)
    );
}

#[test]
fn complete_receipt_set_refuses_missing_duplicate_reordered_and_nonzero_tail() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let base = execution_input(&fixture);

    let mut missing = base;
    missing.settlement_receipts.receipt_count = 15;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, missing),
        Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch)
    );

    let mut duplicate = base;
    duplicate.settlement_receipts.receipts[1].account_id =
        duplicate.settlement_receipts.receipts[0].account_id;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, duplicate),
        Err(PortfolioExecutionErrorV2::AliasedAccount)
    );

    let mut reordered = base;
    reordered.settlement_receipts.receipts.swap(0, 1);
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, reordered),
        Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch)
    );

    let sparse = zero_consideration_fixture(20);
    let sparse_pair = authenticated_pair(&sparse);
    let mut nonzero_tail = execution_input(&sparse);
    nonzero_tail.settlement_receipts.receipts[1] =
        nonzero_tail.settlement_receipts.receipts[0];
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            sparse_pair,
            nonzero_tail,
        ),
        Err(PortfolioExecutionErrorV2::NonCanonicalPadding)
    );
}

#[test]
fn retained_feed_sibling_capability_is_exhaustive_and_not_count_authority() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let input = execution_input(&fixture);
    let traversal = sibling_traversal(&fixture, &input);
    let capability =
        authenticate_portfolio_receipt_sibling_set_v2(pair, traversal).unwrap();
    assert_eq!(capability.sibling_count(), input.settlement_receipts.receipt_count);
    assert_eq!(capability.sibling(0).unwrap().slice_index, 10);
    assert_eq!(capability.pair(), pair);

    let mut missing = traversal;
    missing.sibling_count -= 1;
    assert_eq!(
        authenticate_portfolio_receipt_sibling_set_v2(pair, missing),
        Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch)
    );

    let mut duplicate = traversal;
    duplicate.siblings[1].slice_index = duplicate.siblings[0].slice_index;
    duplicate.siblings[1].sequence = duplicate.siblings[0].sequence;
    assert_eq!(
        authenticate_portfolio_receipt_sibling_set_v2(pair, duplicate),
        Err(PortfolioExecutionErrorV2::FeedTraversalMismatch)
    );

    let sparse = zero_consideration_fixture(20);
    let sparse_pair = authenticated_pair(&sparse);
    let sparse_input = execution_input(&sparse);
    let mut extra_tail = sibling_traversal(&sparse, &sparse_input);
    extra_tail.siblings[1] = extra_tail.siblings[0];
    assert_eq!(
        authenticate_portfolio_receipt_sibling_set_v2(sparse_pair, extra_tail),
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

    let mut wrong_stamp = base;
    wrong_stamp.buyer_reservation.entitled_units -= 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, wrong_stamp),
        Err(PortfolioExecutionErrorV2::ReservationMismatch)
    );

    let mut partially_consumed = base;
    partially_consumed.seller_reservation.consumed_units = 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            pair,
            partially_consumed,
        ),
        Err(PortfolioExecutionErrorV2::ReservationMismatch)
    );

    let mut partially_paid = base;
    partially_paid.seller_reservation.paid_units = 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(
            &TestAdapter::ACCEPT,
            id(200),
            pair,
            partially_paid,
        ),
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
    sequence.settlement_receipts.receipts[0].sequence += 1;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, sequence),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut wrong_kind = base;
    wrong_kind.settlement_receipts.receipts[0].transition_kind =
        SettlementReceiptTransitionKindV2::None;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, wrong_kind),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut unaccounted = base;
    unaccounted.settlement_receipts.receipts[0].accounted_end_mask = 0;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, unaccounted),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut delivered = base;
    delivered.settlement_receipts.receipts[0].delivered_end_mask = 3;
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, delivered),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut replay = base;
    replay.settlement_receipts.receipts[0].transition_commitment = id(99);
    assert_eq!(
        prepare_portfolio_pair_execution_v2(&TestAdapter::ACCEPT, id(200), pair, replay),
        Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch)
    );

    let mut wrong_traversal = base;
    wrong_traversal.settlement_receipts.receipts[0].slice_index = 9;
    wrong_traversal.settlement_receipts.receipts[0].sequence = 10;
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
fn materialization_selection_has_a_disjoint_root_and_position_access_contract() {
    let fixture = pair_fixture(20);
    let materialization = AccessContractAdapter {
        materialization: true,
    };
    assert!(authenticate_selected_portfolio_order_for_materialization_v2(
        &materialization,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .is_ok());
    assert_eq!(
        authenticate_selected_portfolio_order_v2(
            &materialization,
            id(200),
            &fixture.domain,
            &fixture.book,
            &fixture.candidate,
            fixture.candidate_digest,
            fixture.buyer_record,
        ),
        Err(PortfolioExecutionErrorV2::AuthenticationFailed {
            role: PortfolioAccountRoleV2::SettlementRoot,
        })
    );

    let delivery = AccessContractAdapter {
        materialization: false,
    };
    assert!(authenticate_selected_portfolio_order_v2(
        &delivery,
        id(200),
        &fixture.domain,
        &fixture.book,
        &fixture.candidate,
        fixture.candidate_digest,
        fixture.buyer_record,
    )
    .is_ok());
    assert_eq!(
        authenticate_selected_portfolio_order_for_materialization_v2(
            &delivery,
            id(200),
            &fixture.domain,
            &fixture.book,
            &fixture.candidate,
            fixture.candidate_digest,
            fixture.buyer_record,
        ),
        Err(PortfolioExecutionErrorV2::AuthenticationFailed {
            role: PortfolioAccountRoleV2::SettlementRoot,
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
fn every_sibling_preimage_is_in_the_shared_transition_commitment() {
    let fixture = pair_fixture(20);
    let pair = authenticated_pair(&fixture);
    let first = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        execution_input(&fixture),
    )
    .unwrap();

    let mut changed = execution_input(&fixture);
    changed.settlement_receipts.receipts[MAX_OUTCOMES - 1].pre_data_id = id(199);
    let second = prepare_portfolio_pair_execution_v2(
        &TestAdapter::ACCEPT,
        id(200),
        pair,
        changed,
    )
    .unwrap();
    assert_ne!(
        first.receipt().settlement_receipt_set_digest(),
        second.receipt().settlement_receipt_set_digest()
    );
    assert_ne!(first.transition_commitment(), second.transition_commitment());
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
