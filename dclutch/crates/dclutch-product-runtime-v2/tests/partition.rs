//! Exhaustive-and-disjoint, proven by sweep rather than asserted.
//!
//! `ResultDomainV2::select_ordinary` is a scan that returns exactly one `u32`,
//! so "one cell per coordinate" is true of the function's type and says
//! nothing about the partition. What has to be shown is that the selector it
//! returns is the region whose *declared interval* contains the coordinate,
//! and that no other region's declared interval does. This file states those
//! intervals independently — from the record's documented sentence, not from
//! the scan — and sweeps.
//!
//! Both controls run in the same pass as the claim: the sweep asserts it
//! reached every region, and it asserts that a one-boundary error (the
//! opposite half-open convention) is something it can actually see.
//!
//! The sweep also carries the declared source-to-result scale, because a
//! partition test at the identity is exactly the test that missed cohort-14
//! market B. The oracle below applies the factor to the *numerator* while
//! `select_ordinary` applies it to a *denominator*; the two routes to the same
//! cell are what make agreement evidence rather than a function compared with
//! itself.

#![allow(clippy::indexing_slicing)]

use dclutch_product_runtime_v2::{
    ContentId, Error, MAX_SOURCE_SCALE_EXPONENT, ResultDomainInputV2, ResultDomainV2,
    compile_result_domain_v2, result_domain_record_bytes, validate_source_scale_exponent,
};

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

fn domain_bytes(cut_denominator: u64, cuts: &[i128]) -> Vec<u8> {
    let mut bytes = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator,
            cuts,
        },
        &mut bytes,
    )
    .expect("runtime domain compiles");
    bytes
}

/// How many regions declare they own this coordinate, and the lowest such.
///
/// Region `0` is `x < c[0]/d`, interior region `i` is
/// `c[i-1]/d <= x < c[i]/d`, and region `R-1` is `x >= c[R-2]/d`. This
/// predicate is written from that sentence. It does not call
/// `select_ordinary`, so agreement between the two is evidence and not a
/// function compared with itself. `boundary_below` flips to the opposite
/// half-open convention and exists only as this file's control.
///
/// `x` here is the observation *on the cuts' scale*: the raw ratio times ten
/// to the declared `scale`. The oracle multiplies whichever numerator the sign
/// selects, which is a different arithmetic route from the one the record's
/// decoder takes.
fn declared_owners(
    cuts: &[i128],
    cut_denominator: u64,
    numerator: i128,
    denominator: u64,
    scale: i32,
    boundary_below: bool,
) -> (u32, u32) {
    let factor = i128::from(10_u64.pow(scale.unsigned_abs()));
    let at_least = |cut: i128| {
        let left = numerator
            .checked_mul(i128::from(cut_denominator))
            .and_then(|value| {
                if scale < 0 {
                    Some(value)
                } else {
                    value.checked_mul(factor)
                }
            })
            .expect("fixture stays inside i128");
        let right = cut
            .checked_mul(i128::from(denominator))
            .and_then(|value| {
                if scale < 0 {
                    value.checked_mul(factor)
                } else {
                    Some(value)
                }
            })
            .expect("fixture stays inside i128");
        if boundary_below {
            left > right
        } else {
            left >= right
        }
    };
    let region_count = cuts.len().saturating_add(1);
    let mut owners = 0;
    let mut first = 0;
    for region in 0..region_count {
        let lower_ok = region == 0 || at_least(cuts[region.saturating_sub(1)]);
        let upper_ok = region == region_count - 1 || !at_least(cuts[region]);
        if lower_ok && upper_ok {
            if owners == 0 {
                first = u32::try_from(region).expect("fixture width");
            }
            owners += 1;
        }
    }
    (owners, first)
}

struct Sweep {
    swept: u32,
    reached: u32,
    boundary_disagreements: u32,
}

fn sweep(
    cut_denominator: u64,
    cuts: &[i128],
    numerators: core::ops::RangeInclusive<i128>,
    denominators: &[u64],
    scale: i32,
) -> Sweep {
    let bytes = domain_bytes(cut_denominator, cuts);
    let domain = ResultDomainV2::decode(&bytes).expect("runtime domain decodes");
    let region_count = u32::try_from(cuts.len().saturating_add(1)).expect("fixture width");
    assert_eq!(domain.region_count(), region_count);
    let mut reached = vec![false; cuts.len().saturating_add(1)];
    let mut boundary_disagreements = 0;
    let mut swept = 0;
    for denominator in denominators.iter().copied() {
        for numerator in numerators.clone() {
            let honest =
                declared_owners(cuts, cut_denominator, numerator, denominator, scale, false);
            let (owners, owner) = honest;
            assert_eq!(
                owners, 1,
                "{numerator}/{denominator} is owned by {owners} declared regions, not exactly one"
            );
            assert_eq!(
                domain.select_ordinary(numerator, denominator, scale),
                Ok(owner),
                "select_ordinary disagreed with the declared interval \
                 at {numerator}/{denominator} times ten to the {scale}"
            );
            assert!(owner < region_count);
            assert_ne!(owner, domain.failure_selector());
            reached[usize::try_from(owner).expect("fixture width")] = true;
            if declared_owners(cuts, cut_denominator, numerator, denominator, scale, true) != honest
            {
                boundary_disagreements += 1;
            }
            swept += 1;
        }
    }
    Sweep {
        swept,
        reached: u32::try_from(reached.iter().filter(|hit| **hit).count()).expect("fixture width"),
        boundary_disagreements,
    }
}

#[test]
fn narrow_domain_sweep_is_exhaustive_disjoint_and_boundary_sensitive() {
    // Cuts -10, 0 and 25 over a denominator of ten: coordinates -1, 0 and 2.5.
    // Denominators 1, 3 and 7 never land on 2.5; denominator 10 does.
    let cuts = [-10_i128, 0, 25];
    let result = sweep(10, &cuts, -400..=400, &[1, 3, 7, 10], 0);
    assert_eq!(result.swept, 3_204);
    assert_eq!(result.reached, 4);
    assert_eq!(result.boundary_disagreements, 9);
}

#[test]
fn runtime_width_three_hundred_sweep_reaches_every_region() {
    // The same 300-cut domain the width test uses, swept rather than probed at
    // five points: 301 regions, every one of them selected by some coordinate.
    let cuts: Vec<i128> = (-150_i128..150).collect();
    let result = sweep(3, &cuts, -460..=460, &[1, 3], 0);
    assert_eq!(result.swept, 1_842);
    assert_eq!(result.reached, 301);
    // Over denominator 3 every cut numerator is hit exactly; over denominator 1
    // only the multiples of three are. 300 + 100 = 400.
    assert_eq!(result.boundary_disagreements, 400);
}

#[test]
fn single_region_domain_owns_the_whole_line() {
    // No cuts at all: one ordinary region plus the explicit failure outcome.
    // The sweep still has to show the region owns every coordinate and that
    // failure is never reachable by an observation.
    let result = sweep(1, &[], -50..=50, &[1, 2, 3], 0);
    assert_eq!(result.swept, 303);
    assert_eq!(result.reached, 1);
    assert_eq!(result.boundary_disagreements, 0);
}

#[test]
fn a_declared_scale_moves_the_whole_partition_and_still_partitions() {
    // The same narrow domain, swept under a negative and a positive declared
    // factor. Under a shift the reachable numerators move by two decades, so
    // the ranges move with them; what must not move is that every coordinate
    // is owned by exactly one region and that `select_ordinary` names it.
    let cuts = [-10_i128, 0, 25];

    let shrunk = sweep(10, &cuts, -40_000..=40_000, &[1, 3, 7, 10], -2);
    assert_eq!(shrunk.swept, 320_004);
    assert_eq!(shrunk.reached, 4);
    assert_eq!(shrunk.boundary_disagreements, 12);

    // The stretched sweep needs a denominator fine enough to land inside the
    // one-hundredth-wide preimage of the interior cell; without one it reaches
    // three regions and the sweep says so rather than passing quietly.
    let stretched = sweep(10, &cuts, -4..=4, &[1, 3, 7, 10, 1_000], 2);
    assert_eq!(stretched.swept, 45);
    assert_eq!(stretched.reached, 4);
    assert_eq!(stretched.boundary_disagreements, 5);
}

#[test]
fn the_identity_scale_is_exactly_the_selector_that_had_no_scale() {
    // The migration statement, as a sweep: a record that declares no factor
    // reads as zero, and zero reproduces every cell the pre-factor selector
    // chose. Nothing founded before the factor changes hands.
    let cuts = [-10_i128, 0, 25];
    let bytes = domain_bytes(10, &cuts);
    let domain = ResultDomainV2::decode(&bytes).expect("runtime domain decodes");
    for denominator in [1_u64, 3, 7, 10] {
        for numerator in -400_i128..=400 {
            let owner = declared_owners(&cuts, 10, numerator, denominator, 0, false).1;
            assert_eq!(domain.select_ordinary(numerator, denominator, 0), Ok(owner));
        }
    }
}

/// Cohort-14 market B, `DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A`,
/// settled 2026-09-03 at slot 492,412,657.
///
/// Cuts `9900, 10300` over `100` are dollars authored in cents; the
/// certificate's observation `10062091764 / 1` is a raw Pyth SOL/USD mantissa
/// at exponent -8, which is $100.62 — inside the $99-$103 band. The market
/// carried a `[1,0,1,0]` `CentredRangeProtection`, which pays *outside* the
/// band, so the cell decided who was paid.
///
/// Both selectors below are honest arithmetic. They differ only in whether a
/// factor was declared, and that is the whole finding.
#[test]
fn cohort14_market_b_names_cell_one_once_the_factor_is_declared() {
    let cuts = [9_900_i128, 10_300];
    let bytes = domain_bytes(100, &cuts);
    let domain = ResultDomainV2::decode(&bytes).expect("runtime domain decodes");
    let observation = 10_062_091_764_i128;

    // What the deployed program computed, and what the chain committed.
    assert_eq!(domain.select_ordinary(observation, 1, 0), Ok(2));
    // What the feed's own exponent says the reading is.
    assert_eq!(domain.select_ordinary(observation, 1, -8), Ok(1));

    // And the independent interval oracle agrees with both, so neither number
    // is this file reading its own answer back.
    assert_eq!(
        declared_owners(&cuts, 100, observation, 1, 0, false),
        (1, 2)
    );
    assert_eq!(
        declared_owners(&cuts, 100, observation, 1, -8, false),
        (1, 1)
    );
}

#[test]
fn an_unadmitted_or_overflowing_scale_refuses_rather_than_selecting() {
    let bytes = domain_bytes(100, &[9_900_i128, 10_300]);
    let domain = ResultDomainV2::decode(&bytes).expect("runtime domain decodes");
    // One decade past the emitted bound, in both directions.
    assert_eq!(
        domain.select_ordinary(1, 1, MAX_SOURCE_SCALE_EXPONENT + 1),
        Err(Error::UnsupportedScale)
    );
    assert_eq!(
        domain.select_ordinary(1, 1, -(MAX_SOURCE_SCALE_EXPONENT + 1)),
        Err(Error::UnsupportedScale)
    );
    assert_eq!(
        domain.select_ordinary(1, 1, i32::MIN),
        Err(Error::UnsupportedScale)
    );
    // Admitted, but the shifted denominator leaves the physical integer.
    assert_eq!(
        domain.select_ordinary(1, u64::MAX, -1),
        Err(Error::ArithmeticOverflow)
    );
    // Zero denominators still refuse before the scale is even consulted.
    assert_eq!(
        domain.select_ordinary(1, 0, i32::MIN),
        Err(Error::ZeroDenominator)
    );
    assert_eq!(validate_source_scale_exponent(-8), Ok(()));
    assert_eq!(
        validate_source_scale_exponent(MAX_SOURCE_SCALE_EXPONENT + 1),
        Err(Error::UnsupportedScale)
    );
}
