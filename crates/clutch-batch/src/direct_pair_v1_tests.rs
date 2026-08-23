use super::direct_pair_v1::{
    authenticate_selected_direct_pair_v1, AuthenticatedDirectSelectionAuthorityV1,
    DirectCashBoundaryV1, DirectPairErrorV1, NoDirectSelectionAuthorityV1,
};
use super::relation_v1::MAX_OUTCOMES;
use super::relation_v2::{
    price_semantics_digest_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    EconomicOrderV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
    EMPTY_ECONOMIC_ORDER_V2,
};
use super::{PartialPolicy, Side, MAX_ORDERS};

#[derive(Clone, Copy, Debug)]
struct ExactAuthority {
    transcript: [u8; 32],
    digest: [u8; 32],
}

impl AuthenticatedDirectSelectionAuthorityV1 for ExactAuthority {
    fn authenticate_selected_pair(
        &self,
        selection_transcript_id: [u8; 32],
        _domain: &EconomicDomainV2,
        _book: &EconomicBookV2,
        _price: &PricePreconditionV2,
        _candidate: &EconomicCandidateV2,
        economics: &super::relation_v2::VerifiedEconomicsV2,
    ) -> Result<(), DirectPairErrorV1> {
        if selection_transcript_id == self.transcript
            && economics.economic_candidate_digest == self.digest
        {
            Ok(())
        } else {
            Err(DirectPairErrorV1::UnauthenticatedSelection)
        }
    }
}

fn domain(outcome_count: u8, scale: u64) -> EconomicDomainV2 {
    EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: [1; 32],
        epoch_semantics_digest: [2; 32],
        relation_policy_digest: [3; 32],
        price_policy_digest: [4; 32],
        epoch_index: 7,
        outcome_count,
        price_scale: scale,
    }
}

fn price(domain: &EconomicDomainV2, selected: usize, selected_price: u64) -> PricePreconditionV2 {
    let active = usize::from(domain.outcome_count);
    let other = (domain.price_scale - selected_price)
        / u64::try_from(active - 1).unwrap();
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < active {
        prices[outcome] = if outcome == selected {
            selected_price
        } else {
            other
        };
        outcome += 1;
    }
    let mut sum = 0u64;
    outcome = 0;
    while outcome < active {
        sum += prices[outcome];
        outcome += 1;
    }
    prices[0] += domain.price_scale - sum;
    PricePreconditionV2 {
        policy_digest: domain.price_policy_digest,
        semantic_price_digest: price_semantics_digest_v2(domain, &prices).unwrap(),
        prices,
    }
}

fn order(id: u8, side: Side, outcome: usize, quantity: u64, limit: u128) -> EconomicOrderV2 {
    let mut coefficients = [0u64; MAX_OUTCOMES];
    coefficients[outcome] = 1;
    EconomicOrderV2 {
        order_id: [id; 32],
        side,
        coefficients,
        quantity,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: limit,
    }
}

fn book(buy: EconomicOrderV2, sell: EconomicOrderV2) -> EconomicBookV2 {
    let mut orders = [EMPTY_ECONOMIC_ORDER_V2; MAX_ORDERS];
    orders[0] = buy;
    orders[1] = sell;
    EconomicBookV2 { orders, len: 2 }
}

fn candidate(quantity: u64) -> EconomicCandidateV2 {
    let mut fills = [0u64; MAX_ORDERS];
    fills[0] = quantity;
    fills[1] = quantity;
    EconomicCandidateV2 {
        fills,
        honored_aon_mask: 0,
        virtual_split: 0,
        virtual_merge: 0,
    }
}

fn exact_authority(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> ExactAuthority {
    let economics =
        super::relation_v2::verify_economic_candidate_v2(domain, book, price, candidate).unwrap();
    ExactAuthority {
        transcript: [9; 32],
        digest: economics.economic_candidate_digest,
    }
}

#[test]
fn full_width_direct_pair_is_exact_and_private() {
    let domain = domain(u8::try_from(MAX_OUTCOMES).unwrap(), 1_000);
    let price = price(&domain, MAX_OUTCOMES - 1, 625);
    let book = book(
        order(10, Side::Buy, MAX_OUTCOMES - 1, 16, 625),
        order(11, Side::Sell, MAX_OUTCOMES - 1, 16, 625),
    );
    let candidate = candidate(8);
    let authority = exact_authority(&domain, &book, &price, &candidate);
    let selected = authenticate_selected_direct_pair_v1(
        &authority,
        authority.transcript,
        &domain,
        &book,
        &price,
        &candidate,
    )
    .unwrap();
    assert_eq!(selected.outcome(), 15);
    assert_eq!(selected.quantity(), 8);
    assert_eq!(selected.consideration_price_units(), 5_000);
    assert_eq!(selected.consideration_cash_atoms(), 5);
    assert_eq!(selected.boundary(), DirectCashBoundaryV1::ExactOnly);
    assert_eq!(selected.canonical_transcript().len(), 253);
}

#[test]
fn default_deny_cannot_promote_valid_economics() {
    let domain = domain(2, 100);
    let price = price(&domain, 1, 50);
    let book = book(
        order(10, Side::Buy, 1, 10, 50),
        order(11, Side::Sell, 1, 10, 50),
    );
    assert!(matches!(
        authenticate_selected_direct_pair_v1(
            &NoDirectSelectionAuthorityV1,
            [9; 32],
            &domain,
            &book,
            &price,
            &candidate(2),
        ),
        Err(DirectPairErrorV1::UnauthenticatedSelection)
    ));
}

#[test]
fn refuses_portfolio_coefficients_virtual_legs_and_inexact_cash() {
    let domain = domain(2, 100);
    let price = price(&domain, 1, 50);
    let mut book = book(
        order(10, Side::Buy, 1, 10, 50),
        order(11, Side::Sell, 1, 10, 50),
    );
    let exact = candidate(2);
    let authority = exact_authority(&domain, &book, &price, &exact);

    book.orders[0].coefficients[0] = 1;
    assert!(matches!(
        authenticate_selected_direct_pair_v1(
            &authority,
            authority.transcript,
            &domain,
            &book,
            &price,
            &exact,
        ),
        Err(DirectPairErrorV1::Economic(_))
    ));

    let book = book(
        order(10, Side::Buy, 1, 10, 50),
        order(11, Side::Sell, 1, 10, 50),
    );
    let mut virtualized = exact;
    virtualized.virtual_split = 1;
    assert!(matches!(
        authenticate_selected_direct_pair_v1(
            &authority,
            authority.transcript,
            &domain,
            &book,
            &price,
            &virtualized,
        ),
        Err(DirectPairErrorV1::Economic(_)) | Err(DirectPairErrorV1::VirtualConversion)
    ));

    let candidate = candidate(1);
    let authority = exact_authority(&domain, &book, &price, &candidate);
    assert_eq!(
        authenticate_selected_direct_pair_v1(
            &authority,
            authority.transcript,
            &domain,
            &book,
            &price,
            &candidate,
        ),
        Err(DirectPairErrorV1::InexactCashConversion)
    );
}

#[test]
fn refuses_detached_selection_digest_and_unequal_fills() {
    let domain = domain(2, 100);
    let price = price(&domain, 1, 50);
    let book = book(
        order(10, Side::Buy, 1, 10, 50),
        order(11, Side::Sell, 1, 10, 50),
    );
    let candidate = candidate(2);
    let mut authority = exact_authority(&domain, &book, &price, &candidate);
    authority.digest = [77; 32];
    assert_eq!(
        authenticate_selected_direct_pair_v1(
            &authority,
            authority.transcript,
            &domain,
            &book,
            &price,
            &candidate,
        ),
        Err(DirectPairErrorV1::UnauthenticatedSelection)
    );

    let mut unequal = candidate;
    unequal.fills[1] = 1;
    assert!(matches!(
        authenticate_selected_direct_pair_v1(
            &authority,
            authority.transcript,
            &domain,
            &book,
            &price,
            &unequal,
        ),
        Err(DirectPairErrorV1::Economic(_)) | Err(DirectPairErrorV1::FillMismatch)
    ));
}

#[test]
fn exact_zero_price_is_not_mislabeled_as_rounding() {
    let domain = domain(2, 100);
    let price = price(&domain, 1, 0);
    let book = book(
        order(10, Side::Buy, 1, 10, 0),
        order(11, Side::Sell, 1, 10, 0),
    );
    let candidate = candidate(7);
    let authority = exact_authority(&domain, &book, &price, &candidate);
    let selected = authenticate_selected_direct_pair_v1(
        &authority,
        authority.transcript,
        &domain,
        &book,
        &price,
        &candidate,
    )
    .unwrap();
    assert_eq!(selected.quantity(), 7);
    assert_eq!(selected.price_units_per_egg(), 0);
    assert_eq!(selected.consideration_price_units(), 0);
    assert_eq!(selected.consideration_cash_atoms(), 0);
    assert_eq!(selected.boundary(), DirectCashBoundaryV1::ExactOnly);
}
