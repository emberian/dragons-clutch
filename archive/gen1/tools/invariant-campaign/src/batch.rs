use std::boxed::Box;
use std::format;

use clutch_batch::relation_v1::{
    canonical_candidate, verify, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1, ErrorV1,
    FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1, PortfolioLotPolicyV1,
    RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1,
    SingleEggOrderV1, SummaryV1, TransferPhaseV1, MAX_OUTCOMES, MAX_PRICE_SCALE, PRICE_SCALE,
    RELATION_VERSION_V1,
};
use clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1, StreamCandidateV1};
use clutch_batch::{DustPolicy, PartialPolicy, Side, MAX_ORDERS};

use crate::digest::{Rng, Transcript};
use crate::Counts;

const MUTATIONS_PER_SEED: u64 = 128;

pub fn run(seeds: &[u64], transcript: &mut Transcript) -> Counts {
    let mut counts = Counts::default();
    for seed in seeds.iter().copied() {
        run_generated(seed, transcript, &mut counts);
    }
    run_boundaries(transcript, &mut counts);
    counts
}

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

fn run_generated(seed: u64, transcript: &mut Transcript, counts: &mut Counts) {
    let mut rng = Rng::new(seed ^ 0xba7c_15e5_5eed_0001);
    let outcomes = 2 + rng.below(4) as u8;
    let domain = RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: seed | 1,
        book_id: seed.rotate_left(5) | 1,
        epoch: rng.below(1_000_000),
        policy_id: 1,
        order_set_id: seed.rotate_right(9) | 1,
        outcome_count: outcomes,
        owner_count: u16::from(outcomes) * 2,
        price_scale: PRICE_SCALE,
        remainder_seed: seed,
        policy: policy(),
    };
    let mut book = BookV1::empty();
    let mut id = 1u64;
    for outcome in 0..outcomes {
        let quantity = 1 + rng.below(1_000_000);
        book.orders[(outcome as usize) * 2] = single(
            id,
            u16::from(outcome) * 2,
            outcome,
            Side::Buy,
            quantity,
            PRICE_SCALE,
        );
        id += 1;
        book.orders[(outcome as usize) * 2 + 1] = single(
            id,
            u16::from(outcome) * 2 + 1,
            outcome,
            Side::Sell,
            quantity,
            0,
        );
        id += 1;
    }
    book.len = outcomes * 2;
    let prices = simplex_prices(outcomes, seed);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0)
        .unwrap_or_else(|error| panic!("canonical candidate seed={seed:#x}: {error:?}"));
    compare(seed, 0, &domain, &book, &candidate, transcript, counts);

    for case in 0..MUTATIONS_PER_SEED {
        let mut mutated = candidate;
        match rng.below(9) {
            0 => {
                let slot = rng.below(u64::from(book.len)) as usize;
                mutated.fills[slot] = mutated.fills[slot].wrapping_add(1);
            }
            1 => {
                let outcome = rng.below(u64::from(outcomes)) as usize;
                mutated.prices[outcome] ^= 1;
            }
            2 => mutated.virtual_split = mutated.virtual_split.wrapping_add(1),
            3 => mutated.virtual_merge = mutated.virtual_merge.wrapping_add(1),
            4 => mutated.order_len ^= 1,
            5 => mutated.honored_aon_mask ^= 1u64 << rng.below(u64::from(book.len)),
            6 => mutated.claimed_score.digest ^= 1,
            7 => mutated.canonical_candidate_digest ^= 1,
            _ => mutated.claimed_score.weighted_direct_volume ^= 1,
        }
        compare(seed, case + 1, &domain, &book, &mutated, transcript, counts);
    }
}

fn run_boundaries(transcript: &mut Transcript, counts: &mut Counts) {
    let domain = RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: u64::MAX,
        book_id: u64::MAX - 1,
        epoch: u64::MAX,
        policy_id: u64::MAX - 2,
        order_set_id: u64::MAX - 3,
        outcome_count: MAX_OUTCOMES as u8,
        owner_count: 2,
        price_scale: MAX_PRICE_SCALE,
        remainder_seed: u64::MAX,
        policy: policy(),
    };
    let mut book = BookV1::empty();
    book.orders[0] = single(1, 0, 0, Side::Buy, u64::MAX, MAX_PRICE_SCALE);
    book.orders[1] = single(2, 1, 0, Side::Sell, u64::MAX, 0);
    book.len = 2;
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[0] = MAX_PRICE_SCALE;
    let canonical = canonical_candidate(&domain, &book, &prices, 0, 0)
        .expect("u128 accumulators admit the declared maximum relation scale");
    compare(u64::MAX, 0, &domain, &book, &canonical, transcript, counts);

    let mut overflow_probe = canonical;
    overflow_probe.virtual_split = u64::MAX;
    compare(
        u64::MAX,
        1,
        &domain,
        &book,
        &overflow_probe,
        transcript,
        counts,
    );
}

fn compare(
    seed: u64,
    case: u64,
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    transcript: &mut Transcript,
    counts: &mut Counts,
) {
    let batch = verify(domain, book, candidate, None);
    let stream = drive(domain, book, candidate);
    assert_eq!(
        stream, batch,
        "batch/stream divergence seed={seed:#x} case={case}"
    );
    counts.cases += 1;
    transcript.text("batch-stream");
    transcript.u64(seed);
    transcript.u64(case);
    match batch {
        Ok(summary) => {
            counts.accepted += 1;
            transcript.byte(1);
            transcript.u128(summary.candidate_digest);
            assert_summary_closure(domain, &summary);
        }
        Err(error) => {
            counts.refused += 1;
            transcript.byte(0);
            transcript.text(&format!("{error:?}"));
        }
    }
}

fn drive(
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
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
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(domain, &header, true)
        .expect("a complete in-memory feed starts in protocol order");
    loop {
        match work.status() {
            FeedStatusV1::NeedOrders { .. } => {
                for index in 0..book.len as usize {
                    work.push_order(&book.orders[index], candidate.fills[index])
                        .expect("ordered bounded feed");
                }
                work.end_pass().expect("pass boundary");
            }
            FeedStatusV1::NeedSlices => panic!("recomputed-constructor policy requested slices"),
            FeedStatusV1::Complete => {
                return work.verdict().expect("complete feed has verdict").copied();
            }
        }
    }
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

fn simplex_prices(outcomes: u8, seed: u64) -> [u64; MAX_OUTCOMES] {
    let mut prices = [0u64; MAX_OUTCOMES];
    let count = u64::from(outcomes);
    let base = PRICE_SCALE / count;
    let remainder = PRICE_SCALE % count;
    let offset = seed % count;
    for index in 0..count {
        prices[index as usize] = base + u64::from((index + offset) % count < remainder);
    }
    assert_eq!(prices.iter().copied().sum::<u64>(), PRICE_SCALE);
    prices
}

fn assert_summary_closure(domain: &RelationDomainV1, summary: &SummaryV1) {
    for outcome in 0..usize::from(summary.outcome_count) {
        assert_eq!(
            summary.buy_flow[outcome] + summary.virtual_merge,
            summary.total_flow[outcome]
        );
        assert_eq!(
            summary.sell_flow[outcome] + summary.virtual_split,
            summary.total_flow[outcome]
        );
        assert_eq!(
            summary.opening_reserved_egg[outcome],
            summary.sell_flow[outcome]
                + summary.unfilled_refund_egg[outcome]
                + summary.netting_cancelled_egg[outcome]
        );
    }
    assert_eq!(
        summary.buyer_consideration_price_units + summary.merge_proceeds_price_units,
        summary.seller_credit_price_units + summary.split_cost_price_units
    );
    let debit_remainder = summary.debit_atoms * u128::from(domain.price_scale)
        - summary.buyer_consideration_price_units
        - summary.fee_price_units;
    let credit_remainder =
        summary.seller_credit_price_units - summary.credit_atoms * u128::from(domain.price_scale);
    assert_eq!(
        debit_remainder + credit_remainder,
        summary.rounding_pot_price_units
    );
}

const _: () = assert!(MAX_ORDERS >= 2 * MAX_OUTCOMES);
