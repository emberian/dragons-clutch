//! Falsifiers for V1b, the moment-cone admission of the candidate price vector.
//!
//! The named subject is `DUAL_IS_THE_MEASURE.md` §7.4's refutation and §7.6's
//! gate: above degree one the exact simplex gate is strictly weaker than
//! no-arbitrage, so `S*e_j` passes V1 while no probability measure has it as a
//! moment vector.  Every test here is host-only and exact.

use super::*;
use crate::relation_v1_stream::{ClearWorkV1, FeedStatusV1, StreamCandidateV1};
use crate::{DustPolicy, PartialPolicy, Side};

extern crate std;
use std::boxed::Box;

const SCALE: u64 = PRICE_SCALE;

fn policy() -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
        transfer_phase: TransferPhaseV1::ActiveOrResolved,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

fn domain_of(outcomes: u8, scale: u64) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: outcomes,
        owner_count: 4,
        price_scale: scale,
        remainder_seed: 7,
        policy: policy(),
    }
}

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < values.len() {
        vector[i] = values[i];
        i += 1;
    }
    vector
}

fn coordinate(outcomes: usize, claim: usize, scale: u64) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    let _ = outcomes;
    vector[claim] = scale;
    vector
}

fn single(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn book_of(orders: &[OrderV1]) -> BookV1 {
    let mut book = BookV1::empty();
    let mut i = 0usize;
    while i < orders.len() {
        book.orders[i] = orders[i];
        i += 1;
    }
    book.len = orders.len() as u8;
    book
}

/// Drive the streaming twin with a bound basis and return the batch-shaped
/// verdict.
fn drive_with_basis(
    work: &mut ClearWorkV1,
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    basis: BasisDescriptorV1,
) -> Result<SummaryV1, ErrorV1> {
    let header = StreamCandidateV1 {
        order_len: candidate.order_len,
        prices: candidate.prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        claimed_score: candidate.claimed_score,
        canonical_candidate_digest: candidate.canonical_candidate_digest,
        declared_slices: None,
    };
    work.begin_with_basis(domain, &header, true, basis).unwrap();
    loop {
        match work.status() {
            FeedStatusV1::NeedOrders { .. } => {
                let mut j = 0usize;
                while j < book.len as usize {
                    if work.status() == FeedStatusV1::Complete {
                        break;
                    }
                    work.push_order(&book.orders[j], candidate.fills[j])
                        .unwrap();
                    j += 1;
                }
                if work.status() != FeedStatusV1::Complete {
                    work.end_pass().unwrap();
                }
            }
            FeedStatusV1::NeedSlices => unreachable!("recomputed-constructor policy"),
            FeedStatusV1::Complete => break,
        }
    }
    work.verdict()
        .expect("complete feed must have a verdict")
        .copied()
}

/// Batch and stream must agree at every bound basis, not only at the ungated
/// one.
fn assert_twins_agree(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    basis: BasisDescriptorV1,
) -> Result<SummaryV1, ErrorV1> {
    let batch = verify_with_basis(domain, book, candidate, None, basis);
    let mut work = Box::new(ClearWorkV1::new());
    let stream = drive_with_basis(&mut work, domain, book, candidate, basis);
    assert_eq!(
        batch, stream,
        "stream verdict diverged from the batch verifier at basis {basis:?}"
    );
    batch
}

/// A six-outcome book that clears with all price mass on claim three: one buy
/// at the top limit and one sell at the bottom, both bound to claim three.
fn crossing_book_on(claim: u8) -> BookV1 {
    book_of(&[
        single(1, 0, claim, Side::Buy, 4, SCALE),
        single(2, 1, claim, Side::Sell, 4, 0),
    ])
}

#[test]
fn deg2_simplex_vector_with_executable_arbitrage_is_refused_and_deg1_accepts_it() {
    // The §7.4 counterexample, as the relation sees it.  One book, one
    // candidate, one price vector: `p = S*e_3` on six claims.  At degree one
    // every simplex vector is a hat-moment vector (Theorem 7.1) and the
    // candidate clears; at degree two claim three can pay at most `3/4`, so
    // three complete sets short four units of claim three is a nonnegative
    // payoff with price `-S`, and the candidate is refused.
    let domain = domain_of(6, SCALE);
    let book = crossing_book_on(3);
    let peaked = coordinate(6, 3, SCALE);
    let candidate = canonical_candidate(&domain, &book, &peaked, 0, 0).unwrap();

    // Today's verdict, unchanged: no basis bound.
    assert!(verify(&domain, &book, &candidate, None).is_ok());
    assert!(assert_twins_agree(&domain, &book, &candidate, BasisDescriptorV1::UNGATED).is_ok());

    // The discriminating pair.
    let degree_one = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::One);
    let degree_two = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Two);
    let degree_three = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Three);
    assert!(assert_twins_agree(&domain, &book, &candidate, degree_one).is_ok());
    assert_eq!(
        assert_twins_agree(&domain, &book, &candidate, degree_two),
        Err(ErrorV1::PriceOutsideMomentCone { outcome: 3 })
    );
    assert_eq!(
        assert_twins_agree(&domain, &book, &candidate, degree_three),
        Err(ErrorV1::PriceOutsideMomentCone { outcome: 3 })
    );

    // The refusal is the price plane's, not a downstream stage's: the same
    // book clears at the same degree once the price vector re-enters the cone.
    let inside = prices(&[0, 0, 1_250, 7_500, 1_250, 0]);
    let cleared = canonical_candidate(&domain, &book, &inside, 0, 0).unwrap();
    assert!(assert_twins_agree(&domain, &book, &cleared, degree_two).is_ok());
}

#[test]
fn the_moment_cone_refusal_survives_the_checkpoint_codec() {
    // The refusal carries the offending claim in the `outcome` payload lane,
    // so it has to round-trip through the encoded checkpoint — the lane the
    // decoder previously required to be canonical zero for every code but the
    // pairing refusal.
    let domain = domain_of(6, SCALE);
    let book = crossing_book_on(3);
    let peaked = coordinate(6, 3, SCALE);
    let candidate = canonical_candidate(&domain, &book, &peaked, 0, 0).unwrap();
    let basis = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Two);

    let mut work = Box::new(ClearWorkV1::new());
    let verdict = drive_with_basis(&mut work, &domain, &book, &candidate, basis);
    assert_eq!(verdict, Err(ErrorV1::PriceOutsideMomentCone { outcome: 3 }));

    let mut bytes = std::vec![0u8; ClearWorkV1::ENCODED_BYTES];
    work.encode_into(&mut bytes).unwrap();
    let mut decoded = Box::new(ClearWorkV1::new());
    decoded.decode_into(&bytes).unwrap();
    assert_eq!(
        decoded.verdict().map(|verdict| verdict.copied()),
        Some(Err(ErrorV1::PriceOutsideMomentCone { outcome: 3 }))
    );
    let mut round_trip = std::vec![0u8; ClearWorkV1::ENCODED_BYTES];
    decoded.encode_into(&mut round_trip).unwrap();
    assert_eq!(bytes, round_trip);
}

#[test]
fn moment_cone_refuses_every_interior_coordinate_vector_and_admits_the_clamped_ends() {
    // `e_j` is a moment vector exactly at the two open-clamped end claims,
    // which attain 1, and at no interior claim above degree one.
    for degree in [BasisDegreeV1::Two, BasisDegreeV1::Three] {
        let basis = BasisDescriptorV1::ClampedUniform(degree);
        let smallest = match degree {
            BasisDegreeV1::Two => 3usize,
            _ => 4usize,
        };
        for outcomes in smallest..=MAX_OUTCOMES {
            let domain = domain_of(outcomes as u8, SCALE);
            for claim in 0..outcomes {
                let vector = coordinate(outcomes, claim, SCALE);
                let verdict = validate_price_moment_cone(&domain, basis, &vector);
                if claim == 0 || claim + 1 == outcomes {
                    assert_eq!(verdict, Ok(()), "clamped end {claim} of {outcomes}");
                } else {
                    assert_eq!(
                        verdict,
                        Err(ErrorV1::PriceOutsideMomentCone {
                            outcome: claim as u8
                        }),
                        "interior claim {claim} of {outcomes} at {degree:?}"
                    );
                }
            }
        }
    }
}

/// Exact basis vectors of the open-clamped uniform basis, as
/// `(degree, outcomes, price_scale, prices)`.  Each is `S` times the exact
/// rational value of the whole basis at one resolved coordinate, so each is the
/// moment vector of a point mass and must be admitted.  Sources: knots and pane
/// midpoints of the grids `K = 2, 3, 5` at degree two and `K = 2, 6` at degree
/// three.
const EXACT_ATOM_PRICES: &[(BasisDegreeV1, usize, u64, &[u64])] = &[
    // Single-span (Bernstein) grids: here the gate is exactly the cone.
    (BasisDegreeV1::Two, 3, 4_000, &[1_000, 2_000, 1_000]),
    (BasisDegreeV1::Two, 3, 4_000, &[4_000, 0, 0]),
    (BasisDegreeV1::Two, 3, 4_000, &[0, 0, 4_000]),
    (
        BasisDegreeV1::Three,
        4,
        8_000,
        &[1_000, 3_000, 3_000, 1_000],
    ),
    (BasisDegreeV1::Three, 4, 8_000, &[8_000, 0, 0, 0]),
    // Two spans, degree two.
    (BasisDegreeV1::Two, 4, 8_000, &[2_000, 5_000, 1_000, 0]),
    (BasisDegreeV1::Two, 4, 8_000, &[0, 4_000, 4_000, 0]),
    (BasisDegreeV1::Two, 4, 8_000, &[0, 1_000, 5_000, 2_000]),
    // Four spans, degree two: an interior claim at its peak `3/4`.
    (
        BasisDegreeV1::Two,
        6,
        8_000,
        &[2_000, 5_000, 1_000, 0, 0, 0],
    ),
    (BasisDegreeV1::Two, 6, 8_000, &[0, 4_000, 4_000, 0, 0, 0]),
    (
        BasisDegreeV1::Two,
        6,
        8_000,
        &[0, 1_000, 6_000, 1_000, 0, 0],
    ),
    (
        BasisDegreeV1::Two,
        6,
        8_000,
        &[0, 0, 1_000, 6_000, 1_000, 0],
    ),
    (
        BasisDegreeV1::Two,
        6,
        8_000,
        &[0, 0, 0, 1_000, 5_000, 2_000],
    ),
    // Five spans, degree three: an interior claim at its peak `2/3`.
    (
        BasisDegreeV1::Three,
        8,
        96_000,
        &[12_000, 57_000, 25_000, 2_000, 0, 0, 0, 0],
    ),
    (
        BasisDegreeV1::Three,
        8,
        96_000,
        &[0, 24_000, 56_000, 16_000, 0, 0, 0, 0],
    ),
    (
        BasisDegreeV1::Three,
        8,
        96_000,
        &[0, 3_000, 45_000, 46_000, 2_000, 0, 0, 0],
    ),
    (
        BasisDegreeV1::Three,
        8,
        96_000,
        &[0, 0, 16_000, 64_000, 16_000, 0, 0, 0],
    ),
    (
        BasisDegreeV1::Three,
        8,
        96_000,
        &[0, 0, 2_000, 46_000, 46_000, 2_000, 0, 0],
    ),
];

#[test]
fn moment_cone_admits_every_exact_atom_price_vector() {
    for (degree, outcomes, scale, values) in EXACT_ATOM_PRICES {
        let domain = domain_of(*outcomes as u8, *scale);
        let vector = prices(values);
        assert_eq!(validate_prices(&domain, &vector), Ok(()));
        assert_eq!(
            validate_price_moment_cone(
                &domain,
                BasisDescriptorV1::ClampedUniform(*degree),
                &vector
            ),
            Ok(()),
            "a point mass's own moment vector was refused: {degree:?} {values:?}"
        );
    }
}

#[test]
fn moment_cone_refuses_one_atom_above_the_peak() {
    // The peak vectors above sit exactly on the cone boundary — both the
    // ceiling and the butterfly certificate are tight there — so moving one
    // price atom from a wing onto the peak leaves the cone.
    let cases: &[(BasisDegreeV1, usize, u64, &[u64])] = &[
        (BasisDegreeV1::Two, 6, 8_000, &[0, 0, 999, 6_001, 1_000, 0]),
        (BasisDegreeV1::Two, 3, 4_000, &[999, 2_002, 999]),
        (
            BasisDegreeV1::Three,
            8,
            96_000,
            &[0, 0, 15_999, 64_001, 16_000, 0, 0, 0],
        ),
        (BasisDegreeV1::Three, 4, 8_000, &[999, 3_002, 3_000, 999]),
    ];
    for (degree, outcomes, scale, values) in cases {
        let domain = domain_of(*outcomes as u8, *scale);
        let vector = prices(values);
        assert_eq!(validate_prices(&domain, &vector), Ok(()));
        assert!(
            validate_price_moment_cone(
                &domain,
                BasisDescriptorV1::ClampedUniform(*degree),
                &vector
            )
            .is_err(),
            "one atom above the peak stayed admitted: {degree:?} {values:?}"
        );
    }
}

#[test]
fn moment_cone_is_the_constant_true_below_degree_two() {
    // The regression anchor.  At degrees zero and one every simplex vector is a
    // basis-moment vector (§7.1, §7.2), so V1b must admit everything V1 admits
    // and no landed verdict can move.
    let scale = 24u64;
    for outcomes in 2usize..=5 {
        let domain = domain_of(outcomes as u8, scale);
        let mut vector = [0u64; MAX_OUTCOMES];
        let mut seen = 0u32;
        // Enumerate the whole scaled simplex at a small scale.
        fn walk(
            domain: &RelationDomainV1,
            vector: &mut [u64; MAX_OUTCOMES],
            index: usize,
            left: u64,
            seen: &mut u32,
        ) {
            let outcomes = domain.outcomes();
            if index + 1 == outcomes {
                vector[index] = left;
                assert_eq!(validate_prices(domain, vector), Ok(()));
                for basis in [
                    BasisDescriptorV1::UNGATED,
                    BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Zero),
                    BasisDescriptorV1::ClampedUniform(BasisDegreeV1::One),
                ] {
                    assert_eq!(
                        validate_price_moment_cone(domain, basis, vector),
                        Ok(()),
                        "V1b refused a simplex vector below degree two: {vector:?}"
                    );
                }
                *seen += 1;
                vector[index] = 0;
                return;
            }
            let mut take = 0u64;
            while take <= left {
                vector[index] = take;
                walk(domain, vector, index + 1, left - take, seen);
                take += 1;
            }
            vector[index] = 0;
        }
        walk(&domain, &mut vector, 0, scale, &mut seen);
        assert!(seen > 20, "corpus too small at {outcomes} outcomes");
    }
}

#[test]
fn moment_cone_accept_set_is_convex_and_contains_the_uniform_vector() {
    // Every family is a convex condition, so the admitted set is convex; and
    // the uniform vector is the moment vector of the measure that spreads mass
    // by basis integral, so it is always admitted.
    let scale = 96u64;
    for degree in [BasisDegreeV1::Two, BasisDegreeV1::Three] {
        let basis = BasisDescriptorV1::ClampedUniform(degree);
        for outcomes in 4usize..=8 {
            if outcomes % 2 != 0 {
                continue;
            }
            let domain = domain_of(outcomes as u8, scale);
            let mut uniform = [0u64; MAX_OUTCOMES];
            let share = scale / outcomes as u64;
            let mut i = 0usize;
            while i < outcomes {
                uniform[i] = share;
                i += 1;
            }
            assert_eq!(validate_prices(&domain, &uniform), Ok(()));
            assert_eq!(
                validate_price_moment_cone(&domain, basis, &uniform),
                Ok(()),
                "the uniform price vector was refused at {degree:?}, {outcomes} outcomes"
            );
            // Midpoint of the uniform vector and an admitted end coordinate.
            let end = coordinate(outcomes, outcomes - 1, scale);
            let mut mid = [0u64; MAX_OUTCOMES];
            let mut j = 0usize;
            while j < outcomes {
                mid[j] = (uniform[j] + end[j]) / 2;
                j += 1;
            }
            assert_eq!(validate_prices(&domain, &mid), Ok(()));
            assert_eq!(validate_price_moment_cone(&domain, basis, &end), Ok(()));
            assert_eq!(
                validate_price_moment_cone(&domain, basis, &mid),
                Ok(()),
                "convexity broken at {degree:?}, {outcomes} outcomes"
            );
        }
    }
}

#[test]
fn a_weakened_ceiling_admits_the_counterexample_the_gate_refuses() {
    // The mutant: the same stage with the ceiling relaxed to the simplex bound
    // `1` (equivalently, the pre-2026-08-21 relation) accepts `S*e_j`, which
    // the landed stage refuses.  If this ever stops discriminating, the gate
    // has been weakened into the thing it replaced.
    fn weakened(domain: &RelationDomainV1, prices: &[u64; MAX_OUTCOMES]) -> Result<(), ErrorV1> {
        let outcomes = domain.outcomes();
        let mut claim = 1usize;
        while claim + 1 < outcomes {
            if prices[claim] > domain.price_scale {
                return Err(ErrorV1::PriceOutsideMomentCone {
                    outcome: claim as u8,
                });
            }
            claim += 1;
        }
        Ok(())
    }
    let domain = domain_of(6, SCALE);
    let basis = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Two);
    for claim in 1..5usize {
        let vector = coordinate(6, claim, SCALE);
        assert_eq!(weakened(&domain, &vector), Ok(()));
        assert_eq!(
            validate_price_moment_cone(&domain, basis, &vector),
            Err(ErrorV1::PriceOutsideMomentCone {
                outcome: claim as u8
            })
        );
    }
}

#[test]
fn single_span_gate_is_the_exact_hankel_condition() {
    // At `outcome_count == degree + 1` the basis is the Bernstein basis of one
    // span and the implemented conditions are exactly moment-cone membership
    // (§7.6.3, Corollary 7.6.6).  Both quadrics are tight at every point mass:
    // `p_1^2 = 4 p_0 p_2` at degree two, and `p_1^2 = 3 p_0 p_2` with
    // `p_2^2 = 3 p_1 p_3` at degree three.
    let scale = 4_096u64;
    let domain = domain_of(3, scale);
    let basis = BasisDescriptorV1::ClampedUniform(BasisDegreeV1::Two);
    let mut atoms = 0u32;
    let mut denominator = 2u64;
    while denominator <= 64 {
        let mut numerator = 0u64;
        while numerator <= denominator {
            // The exact Bernstein moment vector of the point mass at
            // `u = numerator/denominator`, scaled by `denominator^2`.
            let a = denominator - numerator;
            let b = numerator;
            let square = denominator * denominator;
            if scale.is_multiple_of(square) {
                let unit = scale / square;
                let vector = prices(&[a * a * unit, 2 * a * b * unit, b * b * unit]);
                assert_eq!(validate_prices(&domain, &vector), Ok(()));
                assert_eq!(
                    validate_price_moment_cone(&domain, basis, &vector),
                    Ok(()),
                    "a Bernstein point mass was refused: {vector:?}"
                );
                atoms += 1;
                // One atom above the middle leaves the cone.
                if b > 0 && a > 0 {
                    let raised = prices(&[a * a * unit - 1, 2 * a * b * unit + 1, b * b * unit]);
                    assert_eq!(
                        validate_price_moment_cone(&domain, basis, &raised),
                        Err(ErrorV1::PriceOutsideMomentCone { outcome: 1 }),
                        "raising the middle stayed admitted: {raised:?}"
                    );
                }
            }
            numerator += 1;
        }
        denominator *= 2;
    }
    assert!(atoms > 20, "single-span corpus too small");
}
