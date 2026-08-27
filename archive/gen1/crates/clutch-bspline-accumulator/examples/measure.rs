use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS};
use clutch_bspline_accumulator::{
    BasisDomain, FinalWeights, FinalizationMode, Summary, GRID_IDENTITY_BYTES, SPEC_DIGEST_BYTES,
};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..3].copy_from_slice(&[0, 8, 16]);
    let spec = BasisSpec {
        outcome_count: 5,
        degree: 3,
        knot_count: 3,
        uniform_log2_spacing: 3,
        denominator: 65_537,
        domain_max: 16,
        edge_policy: EdgePolicy::Clamp,
        knots,
    };
    let domain = BasisDomain::new(
        [0x5a; SPEC_DIGEST_BYTES],
        [0x47; GRID_IDENTITY_BYTES],
        60,
        spec,
    )
    .unwrap();

    const ITERATIONS: u64 = 100_000;
    let started = Instant::now();
    for iteration in 0..ITERATIONS {
        let summary = Summary::accepted(domain, iteration, u128::from(iteration & 15)).unwrap();
        black_box(summary);
    }
    let accepted_elapsed = started.elapsed();

    let left = Summary::accepted(domain, 0, 3).unwrap();
    let right = Summary::accepted(domain, 1, 13).unwrap();
    let started = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(left.combine(right).unwrap());
    }
    let combine_elapsed = started.elapsed();

    let summary = left.combine(right).unwrap();
    let started = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(
            summary
                .finalize(FinalizationMode::LargestRemainderV1)
                .unwrap(),
        );
    }
    let finalize_elapsed = started.elapsed();

    println!("BasisSpec bytes: {}", std::mem::size_of::<BasisSpec>());
    println!("BasisDomain bytes: {}", std::mem::size_of::<BasisDomain>());
    println!("Summary bytes: {}", std::mem::size_of::<Summary>());
    println!(
        "FinalWeights bytes: {}",
        std::mem::size_of::<FinalWeights>()
    );
    println!("iterations: {ITERATIONS}");
    println!("accepted cubic: {accepted_elapsed:?}");
    println!("combine: {combine_elapsed:?}");
    println!("largest-remainder finalize: {finalize_elapsed:?}");
}
