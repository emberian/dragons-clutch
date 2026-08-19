use clutch_kernel::{
    BasisMode, Error, MarketState, PayoutSet, PayoutVector, Phase, Position, TransferPhasePolicy,
    MAX_OUTCOMES, MAX_PAYOUTS,
};

use crate::digest::{Rng, Transcript};
use crate::Counts;

const POSITIONS: usize = 4;
const STEPS_PER_SEED: u64 = 2_048;

pub fn run(seeds: &[u64], transcript: &mut Transcript) -> Counts {
    let mut counts = Counts::default();
    for (seed_index, seed) in seeds.iter().copied().enumerate() {
        run_trace(seed_index, seed, transcript, &mut counts);
        run_closure(seed_index, seed, transcript, &mut counts);
    }
    run_dust(transcript, &mut counts);
    run_boundaries(transcript, &mut counts);
    counts
}

fn payout_set() -> PayoutSet {
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 7;
    vectors[0] = PayoutVector::new(7, left);
    let mut middle = [0; MAX_OUTCOMES];
    middle[1] = 7;
    vectors[1] = PayoutVector::new(7, middle);
    let mut fractional = [0; MAX_OUTCOMES];
    fractional[..3].copy_from_slice(&[1, 2, 4]);
    vectors[2] = PayoutVector::new(7, fractional);
    PayoutSet::new(3, 3, vectors)
}

fn fractional_vector() -> PayoutVector {
    let mut weights = [0; MAX_OUTCOMES];
    weights[..3].copy_from_slice(&[1, 2, 4]);
    PayoutVector::new(7, weights)
}

fn run_trace(seed_index: usize, seed: u64, transcript: &mut Transcript, counts: &mut Counts) {
    let mode = if seed_index.is_multiple_of(2) {
        BasisMode::FinitePreset
    } else {
        BasisMode::DerivedBasis
    };
    let mut market = MarketState::new(3, mode, payout_set(), 0).expect("valid campaign market");
    let mut positions = [Position::EMPTY; POSITIONS];
    let mut rng = Rng::new(seed);

    transcript.text("kernel-trace");
    transcript.u64(seed);
    transcript.byte(mode as u8);

    for step in 0..STEPS_PER_SEED {
        let op = rng.below(10) as u8;
        let a = rng.below(POSITIONS as u64) as usize;
        let mut b = rng.below((POSITIONS - 1) as u64) as usize;
        if b >= a {
            b += 1;
        }
        let outcome = rng.below(5) as u8;
        let quantity = match rng.below(32) {
            0 => 0,
            1 => u64::MAX,
            _ => 1 + rng.below(31),
        };
        let before_market = market;
        let before_positions = positions;

        let result: Result<u64, Error> = match op {
            0 => market.split(&mut positions[a], quantity).map(|()| 0),
            1 => market.merge(&mut positions[a], quantity).map(|()| 0),
            2 => market
                .materialize(&mut positions[a], outcome, quantity)
                .map(|()| 0),
            3 => market
                .dematerialize(&mut positions[a], outcome, quantity)
                .map(|()| 0),
            4 => {
                let (from, to) = two_mut(&mut positions, a, b);
                market
                    .transfer_internal(
                        from,
                        to,
                        outcome,
                        quantity,
                        if rng.below(2) == 0 {
                            TransferPhasePolicy::ActiveOnly
                        } else {
                            TransferPhasePolicy::ActiveOrResolved
                        },
                    )
                    .map(|()| 0)
            }
            5 => market.resolve(rng.below(5) as u8).map(|()| 0),
            6 => market
                .resolve_with_vector(if rng.below(4) == 0 {
                    let mut invalid = fractional_vector();
                    invalid.weights[0] += 1;
                    invalid
                } else {
                    fractional_vector()
                })
                .map(|()| 0),
            7 => market.redeem_internal(&mut positions[a], outcome, quantity),
            8 => market.redeem_external(&mut positions[a], outcome, quantity),
            _ => market.redeem_complete_set(&mut positions[a], quantity),
        };

        counts.cases += 1;
        transcript.byte(op);
        transcript.u64(step);
        transcript.u64(quantity);
        transcript.byte(outcome);
        match result {
            Ok(payout) => {
                counts.accepted += 1;
                transcript.byte(1);
                transcript.u64(payout);
                assert_eq!(
                    market.check_invariants(),
                    Ok(()),
                    "seed={seed:#x} step={step}"
                );
                assert_aggregate_closure(&market, &positions, seed, step);
            }
            Err(error) => {
                counts.refused += 1;
                transcript.byte(0);
                transcript.byte(error as u8);
                assert_eq!(
                    market, before_market,
                    "market changed on refusal seed={seed:#x} step={step} op={op} error={error:?}"
                );
                assert_eq!(positions, before_positions, "positions changed on refusal seed={seed:#x} step={step} op={op} error={error:?}");
            }
        }
        fold_market(transcript, &market, &positions);
    }
}

fn run_closure(seed_index: usize, seed: u64, transcript: &mut Transcript, counts: &mut Counts) {
    let mode = if seed_index.is_multiple_of(2) {
        BasisMode::FinitePreset
    } else {
        BasisMode::DerivedBasis
    };
    let quantity = 64 + (seed & 63);
    let mut market = MarketState::new(3, mode, payout_set(), 0).unwrap();
    let mut positions = [Position::EMPTY; POSITIONS];
    market.split(&mut positions[0], quantity).unwrap();

    for outcome in 0..3u8 {
        let first = 1 + ((seed.rotate_left(u32::from(outcome)) % (quantity - 2)) / 2);
        let second = 1 + ((seed.rotate_right(u32::from(outcome) + 1) % (quantity - first - 1)) / 2);
        let (zero, one) = two_mut(&mut positions, 0, 1);
        market
            .transfer_internal(zero, one, outcome, first, TransferPhasePolicy::ActiveOnly)
            .unwrap();
        let (zero, two) = two_mut(&mut positions, 0, 2);
        market
            .transfer_internal(zero, two, outcome, second, TransferPhasePolicy::ActiveOnly)
            .unwrap();
        market
            .materialize(&mut positions[1], outcome, first)
            .unwrap();
        market
            .dematerialize(&mut positions[1], outcome, first)
            .unwrap();
        let (one, zero) = two_mut(&mut positions, 1, 0);
        market
            .transfer_internal(one, zero, outcome, first, TransferPhasePolicy::ActiveOnly)
            .unwrap();
        let (two, zero) = two_mut(&mut positions, 2, 0);
        market
            .transfer_internal(two, zero, outcome, second, TransferPhasePolicy::ActiveOnly)
            .unwrap();
    }
    assert_aggregate_closure(&market, &positions, seed, STEPS_PER_SEED);
    match mode {
        BasisMode::FinitePreset => market.resolve(2).unwrap(),
        BasisMode::DerivedBasis => market.resolve_with_vector(fractional_vector()).unwrap(),
    }
    assert_eq!(
        market.redeem_complete_set(&mut positions[0], quantity),
        Ok(quantity)
    );
    assert_eq!(market.collateral, 0);
    assert_eq!(market.total_supply, [0; MAX_OUTCOMES]);
    assert_aggregate_closure(&market, &positions, seed, STEPS_PER_SEED + 1);

    counts.cases += 1;
    counts.accepted += 1;
    transcript.text("kernel-closure");
    transcript.u64(seed);
    transcript.u64(quantity);
    fold_market(transcript, &market, &positions);
}

fn run_dust(transcript: &mut Transcript, counts: &mut Counts) {
    for outcome in 0..3u8 {
        for quantity in 1..=256u64 {
            let mut market = MarketState::new(3, BasisMode::DerivedBasis, payout_set(), 0).unwrap();
            let mut position = Position::EMPTY;
            market.split(&mut position, quantity).unwrap();
            market.resolve_with_vector(fractional_vector()).unwrap();
            let before_market = market;
            let before_position = position;
            let weight = [1u64, 2, 4][usize::from(outcome)];
            let result = market.redeem_internal(&mut position, outcome, quantity);
            counts.cases += 1;
            transcript.text("kernel-dust");
            transcript.byte(outcome);
            transcript.u64(quantity);
            if (quantity * weight).is_multiple_of(7) {
                assert_eq!(result, Ok(quantity * weight / 7));
                counts.accepted += 1;
                transcript.byte(1);
            } else {
                assert_eq!(result, Err(Error::RemainderRequired));
                assert_eq!(market, before_market);
                assert_eq!(position, before_position);
                counts.refused += 1;
                transcript.byte(0);
            }

            let mut complete =
                MarketState::new(3, BasisMode::DerivedBasis, payout_set(), 0).unwrap();
            let mut complete_position = Position::EMPTY;
            complete.split(&mut complete_position, quantity).unwrap();
            complete.resolve_with_vector(fractional_vector()).unwrap();
            assert_eq!(
                complete.redeem_complete_set(&mut complete_position, quantity),
                Ok(quantity)
            );
            assert_eq!(complete.collateral, 0);
        }
    }
}

fn run_boundaries(transcript: &mut Transcript, counts: &mut Counts) {
    let mut market = MarketState::new(3, BasisMode::DerivedBasis, payout_set(), u64::MAX).unwrap();
    let mut position = Position::EMPTY;
    let before_market = market;
    let before_position = position;
    assert_eq!(
        market.split(&mut position, 1),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(market, before_market);
    assert_eq!(position, before_position);
    counts.cases += 1;
    counts.refused += 1;

    let mut market = MarketState::new(3, BasisMode::FinitePreset, payout_set(), 0).unwrap();
    let mut from = Position::EMPTY;
    market.split(&mut from, 1).unwrap();
    let mut to = Position::EMPTY;
    to.internal[0] = u64::MAX;
    let before_from = from;
    let before_to = to;
    assert_eq!(
        market.transfer_internal(&mut from, &mut to, 0, 1, TransferPhasePolicy::ActiveOnly),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(from, before_from);
    assert_eq!(to, before_to);
    counts.cases += 1;
    counts.refused += 1;
    transcript.text("kernel-boundaries");
    transcript.u64(u64::MAX);
}

fn assert_aggregate_closure(
    market: &MarketState,
    positions: &[Position; POSITIONS],
    seed: u64,
    step: u64,
) {
    for outcome in 0..usize::from(market.outcomes) {
        let sum: u128 = positions
            .iter()
            .map(|position| {
                u128::from(position.internal[outcome]) + u128::from(position.external[outcome])
            })
            .sum();
        assert_eq!(
            u128::from(market.total_supply[outcome]),
            sum,
            "aggregate closure seed={seed:#x} step={step} outcome={outcome}"
        );
    }
    assert!(market.total_supply[usize::from(market.outcomes)..]
        .iter()
        .all(|value| *value == 0));
}

fn fold_market(
    transcript: &mut Transcript,
    market: &MarketState,
    positions: &[Position; POSITIONS],
) {
    transcript.byte(market.phase as u8);
    transcript.byte(market.resolved_payout);
    transcript.u64(market.collateral);
    for value in market.total_supply {
        transcript.u64(value);
    }
    for position in positions {
        for value in position.internal {
            transcript.u64(value);
        }
        for value in position.external {
            transcript.u64(value);
        }
    }
    assert!(matches!(market.phase, Phase::Active | Phase::Resolved));
}

fn two_mut<T>(values: &mut [T; POSITIONS], a: usize, b: usize) -> (&mut T, &mut T) {
    assert_ne!(a, b);
    if a < b {
        let (left, right) = values.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = values.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}
