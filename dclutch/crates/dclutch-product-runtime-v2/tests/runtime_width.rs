//! Runtime-width agreement and caller-buffer tests.

use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, PortfolioV2, ResultDomainInputV2, ResultDomainV2,
    compile_portfolio_v2, compile_result_domain_v2, join_product_v2, portfolio_record_bytes,
    result_domain_record_bytes,
};

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

#[test]
fn runtime_width_three_hundred_has_no_const_generic_or_byte_ceiling() {
    let cuts: Vec<i128> = (-150_i128..150).collect();
    let mut domain_bytes =
        vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: 3,
            cuts: &cuts,
        },
        &mut domain_bytes,
    )
    .expect("runtime domain compiles");
    let domain = ResultDomainV2::decode(&domain_bytes).expect("runtime domain decodes");
    assert_eq!(domain.region_count(), 301);
    assert_eq!(domain.outcome_count(), Ok(302));
    assert_eq!(domain.failure_selector(), 301);
    assert_eq!(domain.cuts().count(), 300);
    assert_eq!(domain.select_ordinary(-151, 3), Ok(0));
    assert_eq!(domain.select_ordinary(-150, 3), Ok(1));
    assert_eq!(domain.select_ordinary(149, 3), Ok(300));
    assert_eq!(domain.select_ordinary(i128::MIN, u64::MAX), Ok(0));
    assert_eq!(domain.select_ordinary(i128::MAX, 1), Ok(300));

    let coefficients: Vec<u64> = (1_u64..=302).map(|value| value * 2).collect();
    let mut portfolio_bytes =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(7),
            claim_basis_id: id(8),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 6,
            coefficients: &coefficients,
        },
        &mut portfolio_bytes,
    )
    .expect("runtime portfolio compiles");
    let portfolio = PortfolioV2::decode(&portfolio_bytes).expect("runtime portfolio decodes");
    assert_eq!(portfolio.coefficient_count(), 302);
    assert_eq!(portfolio.denominator(), 3);
    assert_eq!(portfolio.coefficients().next(), Some(1));
    let joined = join_product_v2(id(7), id(9), domain, portfolio).expect("content identities join");
    assert_eq!(joined.product_id, id(1));
    assert_eq!(joined.result_domain_id, id(7));
    assert_eq!(joined.liability_basis_id, id(4));
    assert_eq!(joined.representation_id, id(9));
    assert_eq!(joined.outcome_count, 302);
}

#[test]
fn exact_representation_has_one_final_floor_and_rechecks_it() {
    let coefficients = [1_u64, 2, 4];
    let mut bytes = vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(2),
            claim_basis_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 3,
            coefficients: &coefficients,
        },
        &mut bytes,
    )
    .expect("portfolio compiles");
    let portfolio = PortfolioV2::decode(&bytes).expect("portfolio decodes");
    let mut quantities = [99_u64; 3];
    portfolio
        .materialize_floor(10, &mut quantities)
        .expect("floor materializes");
    assert_eq!(quantities, [3, 6, 13]);
    assert_eq!(portfolio.recheck_materialization(10, &quantities), Ok(()));
    quantities[1] = 7;
    assert!(portfolio.recheck_materialization(10, &quantities).is_err());
}

#[test]
fn output_buffers_remain_unchanged_on_every_caller_refusal() {
    let mut domain_output = [0xa5_u8; 272];
    let before_domain = domain_output;
    let unordered = [10_i128, 10];
    assert!(
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id: id(1),
                coordinate_domain_id: id(2),
                result_unit_id: id(3),
                liability_basis_id: id(4),
                representation_release_id: id(5),
                mapping_release_id: id(6),
                cut_denominator: 1,
                cuts: &unordered,
            },
            &mut domain_output,
        )
        .is_err()
    );
    assert_eq!(domain_output, before_domain);

    let coefficients = [u64::MAX];
    let mut bytes = vec![0_u8; portfolio_record_bytes(1).expect("width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(2),
            claim_basis_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut bytes,
    )
    .expect("portfolio compiles");
    let portfolio = PortfolioV2::decode(&bytes).expect("portfolio decodes");
    let mut output = [77_u64];
    assert!(portfolio.materialize_floor(u64::MAX, &mut output).is_err());
    assert_eq!(output, [77]);
}
