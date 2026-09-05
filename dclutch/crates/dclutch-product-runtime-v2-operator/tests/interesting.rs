//! The live Product compile path, driven with a partition that never varies.
//!
//! `compile_product_records_v2` is the entrance every production caller uses
//! (`tools/local-validator/bootstrap/successor/src/market.rs`,
//! `spline_product.rs`, `relayed.rs`). It takes cuts as caller authority and
//! validates their SHAPE — strictly increasing, canonical, exact widths — and
//! nothing about where they sit relative to the coordinate the source will
//! report. This file executes that gap rather than describing it, then shows
//! the gated entrance closing it, both against real record compilation.

use dclutch_product::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{
    BandProfileV1, Error, FoundingBandV1, FoundingBeliefV1, MAX_CELL_EX_ANTE_SHARE_BPS_V1,
    ProductCompilationInputV2, StatedPropositionV1, centred_cuts_v1,
    compile_interesting_product_records_v2, compile_product_records_v2,
};
use solana_program::pubkey::Pubkey;

/// Raw signed price atoms at exponent -8: the coordinate the committed local
/// Pyth fixture reports, so one SOL is a hundred million.
const SOL_USD_ANCHOR: i128 = 100_000_000;
const REGISTRY: Pubkey = Pubkey::new_from_array([0xa2; 32]);
fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

fn band() -> FoundingBandV1 {
    FoundingBandV1 {
        anchor: SOL_USD_ANCHOR,
        denominator: 1,
        volatility_bps: 200,
        window_slots: 10_000,
    }
}

fn belief() -> FoundingBeliefV1 {
    FoundingBeliefV1::SpotBand {
        band: band(),
        plausible_half_widths: 2,
    }
}

fn input<'a>(cuts: &'a [i128], coefficients: &'a [u64]) -> ProductCompilationInputV2<'a> {
    ProductCompilationInputV2 {
        product_id: id(1),
        coordinate_domain_id: id(2),
        result_unit_id: id(3),
        claim_basis_id: id(4),
        liability_basis_id: id(5),
        representation_release_id: id(6),
        mapping_release_id: id(7),
        cut_denominator: 1,
        cuts,
        portfolio_denominator: 1,
        coefficients,
    }
}

struct Buffers {
    product: [u8; PRODUCT_RECORD_BYTES_V2],
    domain: Vec<u8>,
    portfolio: Vec<u8>,
}

fn buffers(cuts: usize, coefficients: usize) -> Buffers {
    Buffers {
        product: [0; PRODUCT_RECORD_BYTES_V2],
        domain: vec![0; result_domain_record_bytes(cuts).expect("domain width")],
        portfolio: vec![0; portfolio_record_bytes(coefficients).expect("portfolio width")],
    }
}

#[test]
fn the_ungated_entrance_admits_a_partition_that_always_resolves_the_same_way() {
    // Cuts in USD cents per SOL against a source reporting price atoms: three
    // orders of magnitude below every observation this market will ever see.
    let cuts = [4_000_i128, 12_000, 25_000, 40_000];
    let coefficients = [0_u64, 1, 2, 3, 4, 0];
    let mut output = buffers(cuts.len(), coefficients.len());
    let compiled = compile_product_records_v2(
        REGISTRY,
        input(&cuts, &coefficients),
        &mut output.product,
        &mut output.domain,
        &mut output.portfolio,
    )
    .expect("the live entrance admits it today");
    assert_eq!(compiled.outcome_count, 6);

    // The gated entrance refuses the same input, by name, on the same buffers.
    let mut output = buffers(cuts.len(), coefficients.len());
    assert_eq!(
        compile_interesting_product_records_v2(
            REGISTRY,
            &belief(),
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
            input(&cuts, &coefficients),
            &mut output.product,
            &mut output.domain,
            &mut output.portfolio,
        )
        .err(),
        Some(Error::DegenerateOutcomePartition)
    );
    // A refusal that produced a partial record graph would be worse than none.
    assert_eq!(output.product, [0; PRODUCT_RECORD_BYTES_V2]);
    assert!(output.domain.iter().all(|byte| *byte == 0));
    assert!(output.portfolio.iter().all(|byte| *byte == 0));
}

#[test]
fn a_centred_band_compiles_the_identical_records_and_carries_its_report() {
    let cuts = centred_cuts_v1(&band(), 5, BandProfileV1::Uniform).expect("centred band");
    let coefficients = [0_u64, 1, 2, 3, 4, 0];
    let mut gated = buffers(cuts.len(), coefficients.len());
    let (compiled, report) = compile_interesting_product_records_v2(
        REGISTRY,
        &belief(),
        MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        input(&cuts, &coefficients),
        &mut gated.product,
        &mut gated.domain,
        &mut gated.portfolio,
    )
    .expect("a centred band is a real question");
    assert_eq!(report.cell_share_bps, vec![3_612, 900, 975, 900, 3_612]);
    assert_eq!(report.characteristic_displacement, Some(2_000_000));
    assert_eq!(report.plausible_half_width, Some(4_000_000));
    assert_eq!(report.unresolved_share_bps, 0);

    // The gate adds a refusal and changes no byte of the record graph.
    let mut plain = buffers(cuts.len(), coefficients.len());
    let direct = compile_product_records_v2(
        REGISTRY,
        input(&cuts, &coefficients),
        &mut plain.product,
        &mut plain.domain,
        &mut plain.portfolio,
    )
    .expect("ungated");
    assert_eq!(compiled, direct);
    assert_eq!(gated.product, plain.product);
    assert_eq!(gated.domain, plain.domain);
    assert_eq!(gated.portfolio, plain.portfolio);
}

#[test]
fn a_band_over_another_denominator_refuses_before_any_compilation() {
    let cuts = centred_cuts_v1(&band(), 5, BandProfileV1::Uniform).expect("centred band");
    let coefficients = [0_u64, 1, 2, 3, 4, 0];
    let mut output = buffers(cuts.len(), coefficients.len());
    assert_eq!(
        compile_interesting_product_records_v2(
            REGISTRY,
            &FoundingBeliefV1::SpotBand {
                band: FoundingBandV1 {
                    denominator: 100,
                    ..band()
                },
                plausible_half_widths: 2,
            },
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
            input(&cuts, &coefficients),
            &mut output.product,
            &mut output.domain,
            &mut output.portfolio,
        )
        .err(),
        Some(Error::FoundingBand)
    );
    assert_eq!(output.product, [0; PRODUCT_RECORD_BYTES_V2]);
}

/// The zero-cut market, through the LIVE record compiler rather than through
/// the measure alone: the narrowest partition the protocol emits is refused
/// under a spot band and compiles under a stated prior, on real buffers.
#[test]
fn a_zero_cut_proposition_compiles_records_that_a_spot_band_refuses() {
    let cuts: [i128; 0] = [];
    let coefficients = [1_u64, 0];
    let mut refused = buffers(cuts.len(), coefficients.len());
    assert_eq!(
        compile_interesting_product_records_v2(
            REGISTRY,
            &belief(),
            MAX_CELL_EX_ANTE_SHARE_BPS_V1,
            input(&cuts, &coefficients),
            &mut refused.product,
            &mut refused.domain,
            &mut refused.portfolio,
        )
        .err(),
        Some(Error::DegenerateOutcomePartition),
        "one ordinary cell takes the whole plausible band under every spot band"
    );
    assert_eq!(refused.product, [0; PRODUCT_RECORD_BYTES_V2]);

    let prior = FoundingBeliefV1::StatedProposition(StatedPropositionV1 {
        denominator: 1,
        cell_probability_bps: vec![3_500],
    });
    let mut gated = buffers(cuts.len(), coefficients.len());
    let (compiled, report) = compile_interesting_product_records_v2(
        REGISTRY,
        &prior,
        MAX_CELL_EX_ANTE_SHARE_BPS_V1,
        input(&cuts, &coefficients),
        &mut gated.product,
        &mut gated.domain,
        &mut gated.portfolio,
    )
    .expect("a stated proposition is a question the compiler will emit");
    assert_eq!(compiled.outcome_count, 2);
    assert_eq!(report.cell_share_bps, vec![3_500]);
    assert_eq!(report.unresolved_share_bps, 6_500);

    // And the gate still changes no byte of the graph it admits.
    let mut plain = buffers(cuts.len(), coefficients.len());
    let direct = compile_product_records_v2(
        REGISTRY,
        input(&cuts, &coefficients),
        &mut plain.product,
        &mut plain.domain,
        &mut plain.portfolio,
    )
    .expect("ungated");
    assert_eq!(compiled, direct);
    assert_eq!(gated.product, plain.product);
    assert_eq!(gated.domain, plain.domain);
    assert_eq!(gated.portfolio, plain.portfolio);
}
