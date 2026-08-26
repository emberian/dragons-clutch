//! Hostile Product runtime-tail decoding and identity-substitution tests.

use dclutch_product_runtime_v2::{
    ContentId, DOMAIN_CUT_COUNT_OFFSET, DOMAIN_HEADER_BYTES, DOMAIN_LIABILITY_BASIS_ID_OFFSET,
    DOMAIN_REGION_COUNT_OFFSET, PORTFOLIO_DENOMINATOR_OFFSET, PORTFOLIO_HEADER_BYTES,
    PORTFOLIO_PRODUCT_ID_OFFSET, PORTFOLIO_REPRESENTATION_RELEASE_ID_OFFSET, PortfolioInputV2,
    PortfolioV2, ResultDomainInputV2, ResultDomainV2, compile_portfolio_v2,
    compile_result_domain_v2, join_product_v2, portfolio_record_bytes, result_domain_record_bytes,
};

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

fn domain_bytes() -> Vec<u8> {
    let cuts = [-10_i128, 0, 10];
    let mut bytes = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: 10,
            cuts: &cuts,
        },
        &mut bytes,
    )
    .expect("domain compiles");
    bytes
}

fn portfolio_bytes(width: usize) -> Vec<u8> {
    let coefficients = vec![1_u64; width];
    let mut bytes = vec![0_u8; portfolio_record_bytes(width).expect("width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(7),
            claim_basis_id: id(8),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 3,
            coefficients: &coefficients,
        },
        &mut bytes,
    )
    .expect("portfolio compiles");
    bytes
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes
        .get_mut(offset..offset + 4)
        .expect("fixture offset")
        .copy_from_slice(&value.to_le_bytes());
}

#[test]
fn domain_hostile_lengths_counts_order_and_reserved_bytes_refuse() {
    let canonical = domain_bytes();
    let truncated = canonical
        .get(..canonical.len() - 1)
        .expect("nonempty fixture");
    assert!(ResultDomainV2::decode(truncated).is_err());

    let mut bad_count = canonical.clone();
    put_u32(&mut bad_count, DOMAIN_REGION_COUNT_OFFSET, 9);
    assert!(ResultDomainV2::decode(&bad_count).is_err());

    let mut bad_tail_count = canonical.clone();
    put_u32(&mut bad_tail_count, DOMAIN_CUT_COUNT_OFFSET, 2);
    assert!(ResultDomainV2::decode(&bad_tail_count).is_err());

    let mut unordered = canonical.clone();
    let second = DOMAIN_HEADER_BYTES + 16;
    unordered
        .get_mut(second..second + 16)
        .expect("second cut")
        .copy_from_slice(&(-10_i128).to_le_bytes());
    assert!(ResultDomainV2::decode(&unordered).is_err());

    let mut reserved = canonical.clone();
    *reserved.get_mut(28).expect("reserved byte") = 1;
    assert!(ResultDomainV2::decode(&reserved).is_err());

    let mut zero_identity = canonical.clone();
    zero_identity
        .get_mut(DOMAIN_LIABILITY_BASIS_ID_OFFSET..DOMAIN_LIABILITY_BASIS_ID_OFFSET + 32)
        .expect("identity")
        .fill(0);
    assert!(ResultDomainV2::decode(&zero_identity).is_err());
}

#[test]
fn portfolio_hostile_canonicality_rounding_and_identity_substitution_refuse() {
    let canonical_domain = domain_bytes();
    let domain = ResultDomainV2::decode(&canonical_domain).expect("domain");
    let canonical_portfolio = portfolio_bytes(5);
    let portfolio = PortfolioV2::decode(&canonical_portfolio).expect("portfolio");
    assert!(join_product_v2(id(7), id(9), domain, portfolio).is_ok());
    assert!(join_product_v2(id(10), id(9), domain, portfolio).is_err());

    let mut wrong_product = canonical_portfolio.clone();
    wrong_product
        .get_mut(PORTFOLIO_PRODUCT_ID_OFFSET..PORTFOLIO_PRODUCT_ID_OFFSET + 32)
        .expect("product id")
        .fill(11);
    let wrong_product =
        PortfolioV2::decode(&wrong_product).expect("structurally valid substitution");
    assert!(join_product_v2(id(7), id(9), domain, wrong_product).is_err());

    let mut wrong_release = canonical_portfolio.clone();
    wrong_release
        .get_mut(
            PORTFOLIO_REPRESENTATION_RELEASE_ID_OFFSET
                ..PORTFOLIO_REPRESENTATION_RELEASE_ID_OFFSET + 32,
        )
        .expect("release id")
        .fill(12);
    let wrong_release =
        PortfolioV2::decode(&wrong_release).expect("structurally valid substitution");
    assert!(join_product_v2(id(7), id(9), domain, wrong_release).is_err());

    let different_width_bytes = portfolio_bytes(4);
    let different_width =
        PortfolioV2::decode(&different_width_bytes).expect("different valid width");
    assert!(join_product_v2(id(7), id(9), domain, different_width).is_err());
    let mut wrong_width = canonical_portfolio.clone();

    *wrong_width.get_mut(20).expect("rounding tag") = 2;
    assert!(PortfolioV2::decode(&wrong_width).is_err());

    let mut reducible = canonical_portfolio.clone();
    reducible
        .get_mut(PORTFOLIO_DENOMINATOR_OFFSET..PORTFOLIO_DENOMINATOR_OFFSET + 8)
        .expect("denominator")
        .copy_from_slice(&6_u64.to_le_bytes());
    for chunk in reducible
        .get_mut(PORTFOLIO_HEADER_BYTES..)
        .expect("coefficient tail")
        .chunks_exact_mut(8)
    {
        chunk.copy_from_slice(&2_u64.to_le_bytes());
    }
    assert!(PortfolioV2::decode(&reducible).is_err());
}
