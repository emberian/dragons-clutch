use super::portfolio_book_v2::{
    authenticate_complete_portfolio_book_for_root_transition_v2,
    authenticate_complete_portfolio_book_ref_v2, authenticate_complete_portfolio_book_v2,
    PortfolioBookAccountExpectationV2,
    PortfolioBookAccountRoleV2, PortfolioBookAdapterV2, PortfolioBookAuthorityErrorV2,
    PortfolioBookPageSetRecordV2, PortfolioCompleteBookProjectionExpectationV2,
    PORTFOLIO_BOOK_AUTHORITY_VERSION_V2, PORTFOLIO_BOOK_MAX_PAGES_V2,
    PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES,
};
use super::relation_v1::MAX_OUTCOMES;
use super::relation_v2::{
    EconomicBookV2, EconomicDomainV2, EconomicOrderV2, EMPTY_ECONOMIC_ORDER_V2,
    ECONOMIC_RELATION_VERSION_V2,
};
use super::{PartialPolicy, Side, MAX_ORDERS};
use std::cell::Cell;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn order_id(index: usize) -> [u8; 32] {
    let mut value = [0u8; 32];
    value[31] = u8::try_from(index.checked_add(1).unwrap()).unwrap();
    value
}

fn domain() -> EconomicDomainV2 {
    EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: id(70),
        epoch_semantics_digest: id(71),
        relation_policy_digest: id(72),
        price_policy_digest: id(73),
        epoch_index: 9,
        outcome_count: u8::try_from(MAX_OUTCOMES).unwrap(),
        price_scale: 10_000,
    }
}

fn economic_book(order_count: u8) -> EconomicBookV2 {
    let mut orders = [EMPTY_ECONOMIC_ORDER_V2; MAX_ORDERS];
    let mut index = 0usize;
    while index < usize::from(order_count) {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[index % MAX_OUTCOMES] = 1;
        let side = if index % 2 == 0 {
            Side::Buy
        } else {
            Side::Sell
        };
        orders[index] = EconomicOrderV2 {
            order_id: order_id(index),
            side,
            coefficients,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 9,
            limit_value_price_units_per_unit: 10_000,
        };
        index += 1;
    }
    EconomicBookV2 {
        orders,
        len: order_count,
    }
}

fn page_set(order_count: u8) -> PortfolioBookPageSetRecordV2 {
    let count = usize::from(order_count);
    let page_count = (count.checked_sub(1).unwrap() / 16)
        .checked_add(1)
        .unwrap();
    let mut page_account_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    let mut page_semantic_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    let mut page = 0usize;
    while page < page_count {
        let page_byte = u8::try_from(page).unwrap();
        page_account_ids[page] = id(80u8.checked_add(page_byte).unwrap());
        page_semantic_ids[page] = id(90u8.checked_add(page_byte).unwrap());
        page += 1;
    }
    PortfolioBookPageSetRecordV2 {
        version: PORTFOLIO_BOOK_AUTHORITY_VERSION_V2,
        outcome_count: u8::try_from(MAX_OUTCOMES).unwrap(),
        page_count: u8::try_from(page_count).unwrap(),
        order_count,
        traversal_index: 12,
        settlement_root_epoch_generation: 4,
        market_semantics_digest: domain().market_semantics_digest,
        epoch_semantics_digest: domain().epoch_semantics_digest,
        order_set_digest: id(20),
        settlement_root_account_id: id(21),
        settlement_root_pre_semantic_id: id(22),
        retained_feed_account_id: id(23),
        retained_feed_semantic_id: id(24),
        settlement_candidate_id: id(25),
        settlement_witness_id: id(26),
        page_account_ids,
        page_semantic_ids,
    }
}

struct BookAdapter {
    projected_book: Option<EconomicBookV2>,
    reject_role: Option<PortfolioBookAccountRoleV2>,
    root_writable: bool,
    observed_pages: Cell<u8>,
}

impl PortfolioBookAdapterV2 for BookAdapter {
    fn authenticate_book_account(&self, expected: &PortfolioBookAccountExpectationV2) -> bool {
        if self.reject_role == Some(expected.role)
            || (expected.role == PortfolioBookAccountRoleV2::SettlementRoot
                && expected.writable != self.root_writable)
            || (expected.role != PortfolioBookAccountRoleV2::SettlementRoot && expected.writable)
        {
            return false;
        }
        if expected.role == PortfolioBookAccountRoleV2::OrderPage {
            if expected.page_index != Some(self.observed_pages.get()) {
                return false;
            }
            self.observed_pages
                .set(self.observed_pages.get().checked_add(1).unwrap());
        }
        true
    }

    fn project_complete_economic_book(
        &self,
        _expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<EconomicBookV2> {
        self.projected_book
    }

    fn project_complete_economic_book_ref<'a>(
        &'a self,
        _expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<&'a EconomicBookV2> {
        self.projected_book.as_ref()
    }
}

#[test]
fn adapter_privately_projects_all_active_pages_into_one_book() {
    let adapter = BookAdapter {
        projected_book: Some(economic_book(17)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    let authenticated = authenticate_complete_portfolio_book_v2(
        &adapter,
        id(200),
        &domain(),
        page_set(17),
    )
    .unwrap();
    assert_eq!(adapter.observed_pages.get(), 2);
    assert_eq!(authenticated.page_count(), 2);
    assert_eq!(authenticated.order_count(), 17);
    assert_eq!(authenticated.economic_book(), &economic_book(17));
    assert_eq!(authenticated.order_set_digest(), id(20));
    assert_eq!(authenticated.settlement_root_account_id(), id(21));
    assert_eq!(authenticated.retained_feed_account_id(), id(23));
    assert_eq!(authenticated.settlement_candidate_id(), id(25));
    assert_eq!(authenticated.settlement_witness_id(), id(26));
    assert_eq!(authenticated.traversal_index(), 12);
}

#[test]
fn borrowed_capability_keeps_the_maximum_book_in_adapter_storage() {
    let maximum = u8::try_from(MAX_ORDERS).unwrap();
    let adapter = BookAdapter {
        projected_book: Some(economic_book(maximum)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    let authenticated = authenticate_complete_portfolio_book_ref_v2(
        &adapter,
        id(200),
        &domain(),
        page_set(maximum),
    )
    .unwrap();
    assert_eq!(authenticated.page_count(), 4);
    assert_eq!(authenticated.order_count(), maximum);
    assert_eq!(
        authenticated.economic_book(),
        adapter.projected_book.as_ref().unwrap()
    );
}

#[test]
fn fixed_page_set_codec_refuses_padding_and_noncanonical_geometry() {
    let record = page_set(17);
    let mut bytes = [0u8; PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES];
    record.encode_into(&mut bytes).unwrap();
    assert_eq!(PortfolioBookPageSetRecordV2::decode(&bytes), Ok(record));

    bytes[14] = 1;
    assert_eq!(
        PortfolioBookPageSetRecordV2::decode(&bytes),
        Err(PortfolioBookAuthorityErrorV2::NonCanonicalPadding)
    );

    let mut inactive_page = record;
    inactive_page.page_account_ids[2] = id(99);
    assert_eq!(
        inactive_page.encode_into(&mut bytes),
        Err(PortfolioBookAuthorityErrorV2::NonCanonicalPadding)
    );

    let mut wrong_page_count = record;
    wrong_page_count.page_count = 3;
    assert_eq!(
        wrong_page_count.encode_into(&mut bytes),
        Err(PortfolioBookAuthorityErrorV2::InvalidPageGeometry)
    );
}

#[test]
fn all_four_pages_are_bounded_and_authenticated() {
    let maximum = u8::try_from(MAX_ORDERS).unwrap();
    let adapter = BookAdapter {
        projected_book: Some(economic_book(maximum)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    let authenticated = authenticate_complete_portfolio_book_v2(
        &adapter,
        id(200),
        &domain(),
        page_set(maximum),
    )
    .unwrap();
    assert_eq!(authenticated.page_count(), 4);
    assert_eq!(authenticated.order_count(), maximum);
    assert_eq!(adapter.observed_pages.get(), 4);
}

#[test]
fn adapter_refusal_and_wrong_projection_never_mint_capability() {
    let rejected = BookAdapter {
        projected_book: Some(economic_book(17)),
        reject_role: Some(PortfolioBookAccountRoleV2::OrderPage),
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    assert_eq!(
        authenticate_complete_portfolio_book_v2(&rejected, id(200), &domain(), page_set(17)),
        Err(PortfolioBookAuthorityErrorV2::AuthenticationFailed {
            role: PortfolioBookAccountRoleV2::OrderPage,
        })
    );

    let wrong_count = BookAdapter {
        projected_book: Some(economic_book(16)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    assert_eq!(
        authenticate_complete_portfolio_book_v2(&wrong_count, id(200), &domain(), page_set(17)),
        Err(PortfolioBookAuthorityErrorV2::ProjectedOrderCountMismatch)
    );

    let absent = BookAdapter {
        projected_book: None,
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    assert_eq!(
        authenticate_complete_portfolio_book_v2(&absent, id(200), &domain(), page_set(17)),
        Err(PortfolioBookAuthorityErrorV2::ProjectionAuthenticationFailed)
    );
}

#[test]
fn root_and_domain_substitution_are_refused_before_projection() {
    let adapter = BookAdapter {
        projected_book: Some(economic_book(17)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    let mut alias = page_set(17);
    alias.settlement_root_account_id = alias.retained_feed_account_id;
    assert_eq!(
        authenticate_complete_portfolio_book_v2(&adapter, id(200), &domain(), alias),
        Err(PortfolioBookAuthorityErrorV2::AliasedAccount)
    );

    let mut wrong_domain = page_set(17);
    wrong_domain.market_semantics_digest = id(250);
    assert_eq!(
        authenticate_complete_portfolio_book_v2(&adapter, id(200), &domain(), wrong_domain),
        Err(PortfolioBookAuthorityErrorV2::DomainMismatch)
    );
}

#[test]
fn root_transition_constructor_requires_only_root_writable() {
    let adapter = BookAdapter {
        projected_book: Some(economic_book(17)),
        reject_role: None,
        root_writable: true,
        observed_pages: Cell::new(0),
    };
    let authenticated = authenticate_complete_portfolio_book_for_root_transition_v2(
        &adapter,
        id(200),
        &domain(),
        page_set(17),
    )
    .unwrap();
    assert_eq!(authenticated.page_count(), 2);
    assert_eq!(adapter.observed_pages.get(), 2);

    let readonly_adapter = BookAdapter {
        projected_book: Some(economic_book(17)),
        reject_role: None,
        root_writable: false,
        observed_pages: Cell::new(0),
    };
    assert_eq!(
        authenticate_complete_portfolio_book_for_root_transition_v2(
            &readonly_adapter,
            id(200),
            &domain(),
            page_set(17),
        ),
        Err(PortfolioBookAuthorityErrorV2::AuthenticationFailed {
            role: PortfolioBookAccountRoleV2::SettlementRoot,
        })
    );
}
