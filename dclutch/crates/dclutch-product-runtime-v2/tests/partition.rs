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

#![allow(clippy::indexing_slicing)]

use dclutch_product_runtime_v2::{
    ContentId, ResultDomainInputV2, ResultDomainV2, compile_result_domain_v2,
    result_domain_record_bytes,
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
fn declared_owners(
    cuts: &[i128],
    cut_denominator: u64,
    numerator: i128,
    denominator: u64,
    boundary_below: bool,
) -> (u32, u32) {
    let at_least = |cut: i128| {
        let left = numerator
            .checked_mul(i128::from(cut_denominator))
            .expect("fixture stays inside i128");
        let right = cut
            .checked_mul(i128::from(denominator))
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
            let honest = declared_owners(cuts, cut_denominator, numerator, denominator, false);
            let (owners, owner) = honest;
            assert_eq!(
                owners, 1,
                "{numerator}/{denominator} is owned by {owners} declared regions, not exactly one"
            );
            assert_eq!(
                domain.select_ordinary(numerator, denominator),
                Ok(owner),
                "select_ordinary disagreed with the declared interval \
                 at {numerator}/{denominator}"
            );
            assert!(owner < region_count);
            assert_ne!(owner, domain.failure_selector());
            reached[usize::try_from(owner).expect("fixture width")] = true;
            if declared_owners(cuts, cut_denominator, numerator, denominator, true) != honest {
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
    let result = sweep(10, &cuts, -400..=400, &[1, 3, 7, 10]);
    assert_eq!(result.swept, 3_204);
    assert_eq!(result.reached, 4);
    assert_eq!(result.boundary_disagreements, 9);
}

#[test]
fn runtime_width_three_hundred_sweep_reaches_every_region() {
    // The same 300-cut domain the width test uses, swept rather than probed at
    // five points: 301 regions, every one of them selected by some coordinate.
    let cuts: Vec<i128> = (-150_i128..150).collect();
    let result = sweep(3, &cuts, -460..=460, &[1, 3]);
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
    let result = sweep(1, &[], -50..=50, &[1, 2, 3]);
    assert_eq!(result.swept, 303);
    assert_eq!(result.reached, 1);
    assert_eq!(result.boundary_disagreements, 0);
}
