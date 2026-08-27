use super::relation_v1::{
    canonical_candidate, canonical_pairing, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1,
    FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1, PortfolioLotPolicyV1,
    PortfolioOrderV1, RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1,
    SelfCrossPolicyV1, SingleEggOrderV1, TransferPhaseV1, MAX_OUTCOMES, PRICE_SCALE,
    RELATION_VERSION_V1, TEST_COMPOSITE_LAB,
};
use super::relation_v1_stream::{ClearWorkV1, FeedErrorV1, StreamCandidateV1};
use super::relation_v1_stream_v2::*;
use super::{DustPolicy, PartialPolicy, Side};

extern crate std;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::vec;
use std::vec::Vec;

fn policy(self_cross: SelfCrossPolicyV1) -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement: ResidualSettlementV1::FullPairOnly,
        transfer_phase: TransferPhaseV1::ActiveOnly,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

fn domain(self_cross: SelfCrossPolicyV1) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: 2,
        owner_count: 3,
        price_scale: PRICE_SCALE,
        remainder_seed: 0xC0FFEE,
        policy: policy(self_cross),
    }
}

fn single(id: u64, owner: u16, outcome: u8, side: Side) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity: 2,
        limit_price: if side == Side::Buy { PRICE_SCALE } else { 0 },
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn single_with_obligation(
    id: u64,
    owner: u16,
    outcome: u8,
    side: Side,
    quantity: u64,
    minimum_fill: u64,
    partial_policy: PartialPolicy,
) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: if side == Side::Buy { PRICE_SCALE } else { 0 },
        minimum_fill,
        partial_policy,
        expiry_epoch: u64::MAX,
    })
}

fn portfolio(id: u64, owner: u16, side: Side, coefficients: &[u64], lots: u64) -> OrderV1 {
    let mut active = [0u64; MAX_OUTCOMES];
    active[..coefficients.len()].copy_from_slice(coefficients);
    OrderV1::Portfolio(PortfolioOrderV1 {
        canonical_order_id: id,
        owner,
        side,
        coefficients: active,
        active_len: coefficients.len() as u8,
        lots,
        limit_collateral_per_lot: PRICE_SCALE,
        minimum_fill_lots: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn candidate_mutations(candidate: &CandidateV1) -> [CandidateV1; 6] {
    let mut fill = *candidate;
    fill.fills[0] = fill.fills[0].wrapping_add(1);
    let mut split = *candidate;
    split.virtual_split = split.virtual_split.wrapping_add(1);
    let mut merge = *candidate;
    merge.virtual_merge = merge.virtual_merge.wrapping_add(1);
    let mut mask = *candidate;
    mask.honored_aon_mask ^= 1;
    let mut score = *candidate;
    score.claimed_score.churn = score.claimed_score.churn.wrapping_add(1);
    let mut digest = *candidate;
    digest.canonical_candidate_digest ^= 1;
    [fill, split, merge, mask, score, digest]
}

fn fixture(self_cross: SelfCrossPolicyV1) -> (RelationDomainV1, BookV1, [u64; MAX_OUTCOMES]) {
    let domain = domain(self_cross);
    let mut book = BookV1::empty();
    book.len = 4;
    book.orders[0] = single(1, 0, 0, Side::Buy);
    book.orders[1] = single(2, 1, 0, Side::Sell);
    book.orders[2] = single(3, 2, 1, Side::Buy);
    book.orders[3] = single(4, 1, 1, Side::Sell);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[0] = PRICE_SCALE / 2;
    prices[1] = PRICE_SCALE / 2;
    (domain, book, prices)
}

fn header(candidate: &CandidateV1) -> StreamCandidateV1 {
    StreamCandidateV1 {
        order_len: candidate.order_len,
        prices: candidate.prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        claimed_score: candidate.claimed_score,
        canonical_candidate_digest: candidate.canonical_candidate_digest,
        declared_slices: None,
    }
}

fn encoded(work: &ClearWorkV1) -> Vec<u8> {
    let mut out = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
    work.encode_into(&mut out).unwrap();
    out
}

fn reachable_snapshots(self_cross: SelfCrossPolicyV1) -> Vec<Vec<u8>> {
    let (domain, book, prices) = fixture(self_cross);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let mut work = ClearWorkV1::new();
    let mut snapshots = vec![encoded(&work)];
    work.begin(&domain, &header(&candidate), true).unwrap();
    snapshots.push(encoded(&work));
    let passes = if self_cross == SelfCrossPolicyV1::NetAtAdmission {
        3
    } else {
        2
    };
    let mut pass = 0;
    while pass < passes {
        let mut index = 0usize;
        while index < book.len as usize {
            work.push_order(&book.orders[index], candidate.fills[index])
                .unwrap();
            snapshots.push(encoded(&work));
            index += 1;
        }
        work.end_pass().unwrap();
        snapshots.push(encoded(&work));
        pass += 1;
    }
    snapshots
}

fn edge_snapshots() -> Vec<(ClearWorkWidthsV2, Vec<u8>)> {
    let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut states = Vec::new();

    let mut empty_domain = domain;
    empty_domain.owner_count = 0;
    let empty_candidate = CandidateV1::empty(0, prices);
    let mut empty_work = ClearWorkV1::new();
    empty_work
        .begin(&empty_domain, &header(&empty_candidate), true)
        .unwrap();
    states.push((ClearWorkWidthsV2::new(2, 0, 0), encoded(&empty_work)));

    let mut invalid_domain = domain;
    invalid_domain.relation_version = 99;
    let mut work = ClearWorkV1::new();
    work.begin(&invalid_domain, &header(&candidate), true)
        .unwrap();
    states.push((widths, encoded(&work)));

    let mut poisoned = ClearWorkV1::new();
    poisoned.begin(&domain, &header(&candidate), true).unwrap();
    let mut index = 0usize;
    while index < book.len as usize {
        poisoned
            .push_order(&book.orders[index], candidate.fills[index])
            .unwrap();
        index += 1;
    }
    poisoned.end_pass().unwrap();
    index = 0;
    while index < book.len as usize {
        let fill = if index == 0 {
            candidate.fills[index].wrapping_add(1)
        } else {
            candidate.fills[index]
        };
        poisoned.push_order(&book.orders[index], fill).unwrap();
        index += 1;
    }
    assert_eq!(poisoned.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    states.push((widths, encoded(&poisoned)));

    let mut unchecked = ClearWorkV1::new();
    unchecked
        .begin(&domain, &header(&candidate), false)
        .unwrap();
    states.push((widths, encoded(&unchecked)));

    let mut explicit_domain = domain;
    explicit_domain.owner_count = 2;
    explicit_domain.policy.pairing_witness = PairingWitnessPolicyV1::ExplicitSlices;
    explicit_domain.policy.residual_settlement = ResidualSettlementV1::UniqueSliceReceipts;
    let mut cross = BookV1::empty();
    cross.len = 2;
    cross.orders[0] = single(1, 0, 0, Side::Buy);
    cross.orders[1] = single(2, 1, 0, Side::Sell);
    let sliced = canonical_candidate(&explicit_domain, &cross, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit_domain, &cross, &sliced).unwrap();
    let sliced_header = StreamCandidateV1 {
        declared_slices: Some(witness.len),
        ..header(&sliced)
    };
    let slice_widths = ClearWorkWidthsV2::new(2, 2, 2);
    let mut sliced_work = ClearWorkV1::new();
    sliced_work
        .begin(&explicit_domain, &sliced_header, true)
        .unwrap();
    index = 0;
    while index < cross.len as usize {
        sliced_work
            .push_order(&cross.orders[index], sliced.fills[index])
            .unwrap();
        index += 1;
    }
    sliced_work.end_pass().unwrap();
    states.push((slice_widths, encoded(&sliced_work)));
    index = 0;
    while index < witness.len as usize {
        sliced_work.push_slice(&witness.slices[index]).unwrap();
        states.push((slice_widths, encoded(&sliced_work)));
        index += 1;
    }
    sliced_work.end_pass().unwrap();
    states.push((slice_widths, encoded(&sliced_work)));
    index = 0;
    while index < cross.len as usize {
        sliced_work
            .push_order(&cross.orders[index], sliced.fills[index])
            .unwrap();
        index += 1;
    }
    sliced_work.end_pass().unwrap();
    states.push((slice_widths, encoded(&sliced_work)));
    states
}

fn compact(v1: &[u8], widths: ClearWorkWidthsV2) -> Vec<u8> {
    let mut out = vec![0u8; clear_work_v2_body_len(widths)];
    let mut scratch = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
    project_clear_work_v1_wire_into_v2(v1, widths, &mut out, &mut scratch).unwrap();
    out
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn v2_verdict(
    body: &mut [u8],
    widths: ClearWorkWidthsV2,
) -> Option<Result<super::relation_v1::SummaryV1, super::relation_v1::ErrorV1>> {
    let work = ClearWorkFeedV2::open(body, widths).unwrap();
    work.verdict().map(|result| result.copied())
}

fn v1_verdict(
    work: &ClearWorkV1,
) -> Option<Result<super::relation_v1::SummaryV1, super::relation_v1::ErrorV1>> {
    work.verdict().map(|result| result.copied())
}

fn assert_lockstep_checkpoint(v1: &ClearWorkV1, v2: &mut [u8], widths: ClearWorkWidthsV2) {
    assert_eq!(v2, compact(&encoded(v1), widths));
    let reopened = ClearWorkFeedV2::open(v2, widths).unwrap();
    assert_eq!(reopened.status(), v1.status());
    assert_eq!(reopened.orders_consumed(), v1.orders_consumed());
    assert_eq!(reopened.slices_consumed(), v1.slices_consumed());
    assert_eq!(reopened.consumed_fold(), v1.consumed_fold());
    assert_eq!(reopened.is_idle(), v1.is_idle());
    assert_eq!(reopened.is_poisoned(), v1.is_poisoned());
    assert_eq!(
        reopened.verdict().map(|result| result.copied()),
        v1_verdict(v1)
    );
}

fn lockstep_orders(self_cross: SelfCrossPolicyV1) {
    let (domain, book, prices) = fixture(self_cross);
    let candidate_full = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let candidate = header(&candidate_full);
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut v1 = ClearWorkV1::new();
    let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut v2, widths).unwrap();
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    let expected = v1.begin(&domain, &candidate, true);
    let actual = ClearWorkFeedV2::open(&mut v2, widths)
        .unwrap()
        .begin(&domain, &candidate, true);
    assert_eq!(actual, expected);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    let passes = if self_cross == SelfCrossPolicyV1::NetAtAdmission {
        3
    } else {
        2
    };
    let mut pass = 0usize;
    while pass < passes {
        let mut index = 0usize;
        while index < book.len as usize {
            let fill = candidate_full.fills[index];
            let expected = v1.push_order(&book.orders[index], fill);
            let actual = ClearWorkFeedV2::open(&mut v2, widths)
                .unwrap()
                .push_order(&book.orders[index], fill);
            assert_eq!(actual, expected, "pass {pass} order {index}");
            assert_lockstep_checkpoint(&v1, &mut v2, widths);
            index += 1;
        }
        let expected = v1.end_pass();
        let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
        assert_eq!(actual, expected, "pass {pass} end");
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
        pass += 1;
    }
    assert_eq!(v2_verdict(&mut v2, widths), v1_verdict(&v1));
}

fn drive_lockstep_book(domain: &RelationDomainV1, book: &BookV1, candidate_full: &CandidateV1) {
    let candidate = header(candidate_full);
    let widths = ClearWorkWidthsV2::new(domain.outcome_count, book.len, domain.owner_count as u8);
    let mut v1 = ClearWorkV1::new();
    let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut v2, widths).unwrap();
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .begin(domain, &candidate, true),
        v1.begin(domain, &candidate, true)
    );
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    let mut transitions = 0usize;
    while v1.status() != super::relation_v1_stream::FeedStatusV1::Complete {
        for index in 0..book.len as usize {
            let expected = v1.push_order(&book.orders[index], candidate_full.fills[index]);
            let actual = ClearWorkFeedV2::open(&mut v2, widths)
                .unwrap()
                .push_order(&book.orders[index], candidate_full.fills[index]);
            assert_eq!(actual, expected);
            assert_lockstep_checkpoint(&v1, &mut v2, widths);
            transitions += 1;
        }
        let expected = v1.end_pass();
        let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
        assert_eq!(actual, expected);
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
        transitions += 1;
        assert!(
            transitions <= 4 + 3 * (book.len as usize + 1),
            "unexpected feed cycle"
        );
    }
    let batch = super::relation_v1::verify(domain, book, candidate_full, None);
    assert_eq!(v1_verdict(&v1), Some(batch));
    assert_eq!(v2_verdict(&mut v2, widths), v1_verdict(&v1));
}

#[test]
fn native_engine_matches_v1_after_every_order_transition() {
    lockstep_orders(SelfCrossPolicyV1::AllowGateAtPairing);
    lockstep_orders(SelfCrossPolicyV1::NetAtAdmission);
}

#[test]
fn bounded_books_match_batch_v1_and_v2_at_every_transition() {
    let mut cases = 0usize;
    for self_cross in [
        SelfCrossPolicyV1::AllowGateAtPairing,
        SelfCrossPolicyV1::NetAtAdmission,
    ] {
        for len in [2usize, 4, 6] {
            for bits in 0..64u64 {
                let mut domain = domain(self_cross);
                domain.owner_count = 2;
                domain.remainder_seed = bits.wrapping_mul(0x9e37_79b9);
                let mut book = BookV1::empty();
                book.len = len as u8;
                for index in 0..len {
                    let pair = index / 2;
                    let side = if index & 1 == 0 {
                        Side::Buy
                    } else {
                        Side::Sell
                    };
                    let outcome = ((bits >> pair) & 1) as u8;
                    let owner = if side == Side::Buy { 0 } else { 1 };
                    let mut order = single(index as u64 + 1, owner, outcome, side);
                    let OrderV1::SingleEgg(ref mut single) = order else {
                        unreachable!();
                    };
                    single.quantity = 1 + ((bits >> (pair + 3)) % 3);
                    book.orders[index] = order;
                }
                let mut prices = [0u64; MAX_OUTCOMES];
                prices[0] = PRICE_SCALE / 2;
                prices[1] = PRICE_SCALE / 2;
                let Ok(candidate) = canonical_candidate(&domain, &book, &prices, 0, 0) else {
                    continue;
                };
                drive_lockstep_book(&domain, &book, &candidate);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 384);
}

#[test]
fn minimum_and_maximum_active_widths_execute_without_v1_expansion() {
    let mut empty_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
    empty_domain.owner_count = 0;
    let empty_book = BookV1::empty();
    let mut two_prices = [0u64; MAX_OUTCOMES];
    two_prices[0] = PRICE_SCALE / 2;
    two_prices[1] = PRICE_SCALE / 2;
    let empty_candidate = CandidateV1::empty(0, two_prices);
    drive_lockstep_book(&empty_domain, &empty_book, &empty_candidate);

    let mut maximum_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
    maximum_domain.outcome_count = MAX_OUTCOMES as u8;
    maximum_domain.owner_count = 64;
    let mut maximum_book = BookV1::empty();
    maximum_book.len = 64;
    for index in 0..64usize {
        let side = if index & 1 == 0 {
            Side::Buy
        } else {
            Side::Sell
        };
        maximum_book.orders[index] =
            single(index as u64 + 1, index as u16, (index / 4) as u8, side);
    }
    let mut maximum_prices = [PRICE_SCALE / MAX_OUTCOMES as u64; MAX_OUTCOMES];
    maximum_prices[MAX_OUTCOMES - 1] += PRICE_SCALE - maximum_prices[0] * MAX_OUTCOMES as u64;
    let maximum_candidate =
        canonical_candidate(&maximum_domain, &maximum_book, &maximum_prices, 0, 0)
            .expect("maximum active dimensions have a canonical candidate");
    drive_lockstep_book(&maximum_domain, &maximum_book, &maximum_candidate);
}

#[test]
fn portfolio_aon_dust_self_cross_and_composite_hooks_are_v1_exact() {
    let mut compared = 0usize;

    let mut portfolio_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
    portfolio_domain.outcome_count = 3;
    portfolio_domain.owner_count = 3;
    let mut portfolio_book = BookV1::empty();
    portfolio_book.len = 4;
    portfolio_book.orders[0] = portfolio(1, 0, Side::Buy, &[1, 1, 1], 2);
    portfolio_book.orders[1] =
        single_with_obligation(2, 1, 0, Side::Sell, 2, 1, PartialPolicy::Allow);
    portfolio_book.orders[2] =
        single_with_obligation(3, 1, 1, Side::Sell, 2, 1, PartialPolicy::Allow);
    portfolio_book.orders[3] =
        single_with_obligation(4, 2, 2, Side::Sell, 2, 1, PartialPolicy::Allow);
    let mut portfolio_prices = [0u64; MAX_OUTCOMES];
    portfolio_prices[0] = PRICE_SCALE / 4;
    portfolio_prices[1] = PRICE_SCALE / 4;
    portfolio_prices[2] = PRICE_SCALE / 2;
    let portfolio_candidate =
        canonical_candidate(&portfolio_domain, &portfolio_book, &portfolio_prices, 0, 0).unwrap();
    drive_lockstep_book(&portfolio_domain, &portfolio_book, &portfolio_candidate);
    for mutation in candidate_mutations(&portfolio_candidate) {
        drive_lockstep_book(&portfolio_domain, &portfolio_book, &mutation);
        compared += 1;
    }

    let mut aon_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
    aon_domain.owner_count = 4;
    aon_domain.policy.aon = AonPolicyV1::WitnessedHonoredMask;
    let mut aon_book = BookV1::empty();
    aon_book.len = 4;
    aon_book.orders[0] = single_with_obligation(1, 0, 0, Side::Buy, 4, 4, PartialPolicy::AllOrNone);
    aon_book.orders[1] =
        single_with_obligation(2, 1, 0, Side::Sell, 4, 4, PartialPolicy::AllOrNone);
    aon_book.orders[2] = single_with_obligation(3, 2, 1, Side::Buy, 4, 4, PartialPolicy::AllOrNone);
    aon_book.orders[3] =
        single_with_obligation(4, 3, 1, Side::Sell, 4, 4, PartialPolicy::AllOrNone);
    let mut midpoint = [0u64; MAX_OUTCOMES];
    midpoint[0] = PRICE_SCALE / 2;
    midpoint[1] = PRICE_SCALE / 2;
    for mask in 0..16u64 {
        let candidate = canonical_candidate(&aon_domain, &aon_book, &midpoint, 0, mask)
            .unwrap_or_else(|_| {
                let mut refused = CandidateV1::empty(4, midpoint);
                refused.honored_aon_mask = mask;
                refused
            });
        drive_lockstep_book(&aon_domain, &aon_book, &candidate);
        compared += 1;
    }

    let mut variant_book = BookV1::empty();
    variant_book.len = 4;
    variant_book.orders[0] = single_with_obligation(1, 0, 0, Side::Buy, 4, 1, PartialPolicy::Allow);
    variant_book.orders[1] =
        single_with_obligation(2, 1, 0, Side::Sell, 2, 1, PartialPolicy::Allow);
    variant_book.orders[2] =
        single_with_obligation(3, 2, 0, Side::Sell, 1, 1, PartialPolicy::Allow);
    variant_book.orders[3] =
        single_with_obligation(4, 3, 0, Side::Sell, 3, 1, PartialPolicy::Allow);
    for allocation in [
        AllocationPolicyV1::PricePriorityMarginalProRata,
        AllocationPolicyV1::FullProRata,
    ] {
        for dust in [DustPolicy::AssignCanonical, DustPolicy::Reject] {
            let mut variant_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
            variant_domain.owner_count = 4;
            variant_domain.policy.allocation = allocation;
            variant_domain.policy.dust = dust;
            if let Ok(candidate) =
                canonical_candidate(&variant_domain, &variant_book, &midpoint, 0, 0)
            {
                drive_lockstep_book(&variant_domain, &variant_book, &candidate);
                compared += 1;
            }
        }
    }

    let mut overlap_book = BookV1::empty();
    overlap_book.len = 3;
    overlap_book.orders[0] = single_with_obligation(1, 0, 0, Side::Buy, 3, 1, PartialPolicy::Allow);
    overlap_book.orders[1] =
        single_with_obligation(2, 0, 0, Side::Sell, 2, 1, PartialPolicy::Allow);
    overlap_book.orders[2] =
        single_with_obligation(3, 1, 0, Side::Sell, 2, 1, PartialPolicy::Allow);
    for self_cross in [
        SelfCrossPolicyV1::RefuseOverlap,
        SelfCrossPolicyV1::NetAtAdmission,
        SelfCrossPolicyV1::AllowGateAtPairing,
    ] {
        let mut overlap_domain = domain(self_cross);
        overlap_domain.owner_count = 2;
        let candidate = canonical_candidate(&overlap_domain, &overlap_book, &midpoint, 0, 0)
            .unwrap_or_else(|_| CandidateV1::empty(3, midpoint));
        drive_lockstep_book(&overlap_domain, &overlap_book, &candidate);
        compared += 1;
    }

    let mut fee_domain = domain(SelfCrossPolicyV1::AllowGateAtPairing);
    fee_domain.owner_count = 2;
    fee_domain.policy.fee_base = TEST_COMPOSITE_LAB;
    fee_domain.policy.rounding = RoundingBoundaryV1::ReceiptFloor;
    let mut fee_book = BookV1::empty();
    fee_book.len = 2;
    fee_book.orders[0] =
        single_with_obligation(1, 0, 0, Side::Buy, 40_000, 1, PartialPolicy::Allow);
    fee_book.orders[1] =
        single_with_obligation(2, 1, 0, Side::Sell, 40_000, 1, PartialPolicy::Allow);
    let fee_candidate = canonical_candidate(&fee_domain, &fee_book, &midpoint, 0, 0).unwrap();
    let fee_summary = super::relation_v1::verify(&fee_domain, &fee_book, &fee_candidate, None)
        .expect("fee fixture must accept");
    assert!(fee_summary.fee_price_units > 0);
    drive_lockstep_book(&fee_domain, &fee_book, &fee_candidate);
    for mutation in candidate_mutations(&fee_candidate) {
        drive_lockstep_book(&fee_domain, &fee_book, &mutation);
        compared += 1;
    }

    assert_eq!(compared, 32, "policy corpus moved; re-audit it");
}

#[test]
fn native_engine_matches_v1_after_every_explicit_slice_transition() {
    let (mut domain, _, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    domain.owner_count = 2;
    domain.policy.pairing_witness = PairingWitnessPolicyV1::ExplicitSlices;
    domain.policy.residual_settlement = ResidualSettlementV1::UniqueSliceReceipts;
    let mut book = BookV1::empty();
    book.len = 2;
    book.orders[0] = single(1, 0, 0, Side::Buy);
    book.orders[1] = single(2, 1, 0, Side::Sell);
    let candidate_full = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&domain, &book, &candidate_full).unwrap();
    let candidate = StreamCandidateV1 {
        declared_slices: Some(witness.len),
        ..header(&candidate_full)
    };
    let widths = ClearWorkWidthsV2::new(2, 2, 2);
    let mut v1 = ClearWorkV1::new();
    let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut v2, widths).unwrap();

    let expected = v1.begin(&domain, &candidate, true);
    let actual = ClearWorkFeedV2::open(&mut v2, widths)
        .unwrap()
        .begin(&domain, &candidate, true);
    assert_eq!(actual, expected);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    for index in 0..book.len as usize {
        let expected = v1.push_order(&book.orders[index], candidate_full.fills[index]);
        let actual = ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_order(&book.orders[index], candidate_full.fills[index]);
        assert_eq!(actual, expected);
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
    }
    let expected = v1.end_pass();
    let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
    assert_eq!(actual, expected);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    for index in 0..witness.len as usize {
        let expected = v1.push_slice(&witness.slices[index]);
        let actual = ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_slice(&witness.slices[index]);
        assert_eq!(actual, expected);
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
    }
    let expected = v1.end_pass();
    let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
    assert_eq!(actual, expected);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    for index in 0..book.len as usize {
        let expected = v1.push_order(&book.orders[index], candidate_full.fills[index]);
        let actual = ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_order(&book.orders[index], candidate_full.fills[index]);
        assert_eq!(actual, expected);
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
    }
    let expected = v1.end_pass();
    let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
    assert_eq!(actual, expected);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);
    assert!(v1_verdict(&v1).unwrap().is_ok());
}

#[test]
fn native_engine_poison_and_protocol_errors_are_v1_exact_and_atomic() {
    let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    let candidate_full = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let candidate = header(&candidate_full);
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut v1 = ClearWorkV1::new();
    let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut v2, widths).unwrap();

    let idle_before = v2.clone();
    assert_eq!(v1.end_pass(), Err(FeedErrorV1::NotInProgress));
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass(),
        Err(FeedErrorV1::NotInProgress)
    );
    assert_eq!(v2, idle_before);

    v1.begin(&domain, &candidate, true).unwrap();
    ClearWorkFeedV2::open(&mut v2, widths)
        .unwrap()
        .begin(&domain, &candidate, true)
        .unwrap();
    let wrong_phase_before = v2.clone();
    let slice = super::relation_v1::PairingSliceV1 {
        buy_ref: super::relation_v1::LegRefV1::Order(0),
        sell_ref: super::relation_v1::LegRefV1::Order(1),
        outcome: 0,
        quantity: 1,
    };
    assert_eq!(v1.push_slice(&slice), Err(FeedErrorV1::WrongPhase));
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_slice(&slice),
        Err(FeedErrorV1::WrongPhase)
    );
    assert_eq!(v2, wrong_phase_before);

    for index in 0..book.len as usize {
        v1.push_order(&book.orders[index], candidate_full.fills[index])
            .unwrap();
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_order(&book.orders[index], candidate_full.fills[index])
            .unwrap();
    }
    v1.end_pass().unwrap();
    ClearWorkFeedV2::open(&mut v2, widths)
        .unwrap()
        .end_pass()
        .unwrap();
    for index in 0..book.len as usize {
        let fill = if index == 0 {
            candidate_full.fills[index].wrapping_add(1)
        } else {
            candidate_full.fills[index]
        };
        v1.push_order(&book.orders[index], fill).unwrap();
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_order(&book.orders[index], fill)
            .unwrap();
    }
    let before_poison = v2.clone();
    assert_eq!(v1.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass(),
        Err(FeedErrorV1::ResumeFoldMismatch)
    );
    assert_ne!(v2, before_poison);
    assert_lockstep_checkpoint(&v1, &mut v2, widths);

    let poisoned_before = v2.clone();
    assert_eq!(
        v1.push_order(&book.orders[0], 0),
        Err(FeedErrorV1::NotInProgress)
    );
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .push_order(&book.orders[0], 0),
        Err(FeedErrorV1::NotInProgress)
    );
    assert_eq!(v2, poisoned_before);
}

#[test]
fn native_engine_preserves_candidate_length_and_padding_refusals() {
    let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    let candidate_full = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let widths = ClearWorkWidthsV2::new(2, 4, 3);

    for forged_len in [3u8, 5] {
        let mut candidate = header(&candidate_full);
        candidate.order_len = forged_len;
        let mut v1 = ClearWorkV1::new();
        let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
        initialize_clear_work_v2_idle(&mut v2, widths).unwrap();
        assert_eq!(
            ClearWorkFeedV2::open(&mut v2, widths)
                .unwrap()
                .begin(&domain, &candidate, true),
            v1.begin(&domain, &candidate, true)
        );
        assert_lockstep_checkpoint(&v1, &mut v2, widths);
        for pass in 0..2 {
            for index in 0..book.len as usize {
                let expected = v1.push_order(&book.orders[index], candidate_full.fills[index]);
                let actual = ClearWorkFeedV2::open(&mut v2, widths)
                    .unwrap()
                    .push_order(&book.orders[index], candidate_full.fills[index]);
                assert_eq!(
                    actual, expected,
                    "len {forged_len} pass {pass} order {index}"
                );
            }
            let expected = v1.end_pass();
            let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
            assert_eq!(actual, expected, "len {forged_len} pass {pass} end");
            assert_lockstep_checkpoint(&v1, &mut v2, widths);
            if v1.status() == super::relation_v1_stream::FeedStatusV1::Complete {
                break;
            }
        }
        assert_eq!(
            v1_verdict(&v1),
            Some(Err(super::relation_v1::ErrorV1::CandidateMismatch))
        );
    }

    // Inactive candidate prices are intentionally absent from compact
    // storage, but their begin-time refusal and digest contribution must
    // survive every reopen and produce the same semantic verdict.
    let mut candidate = header(&candidate_full);
    candidate.prices[2] = 1;
    let mut v1 = ClearWorkV1::new();
    let mut v2 = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut v2, widths).unwrap();
    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths)
            .unwrap()
            .begin(&domain, &candidate, true),
        v1.begin(&domain, &candidate, true)
    );
    while v1.status() != super::relation_v1_stream::FeedStatusV1::Complete {
        for index in 0..book.len as usize {
            let expected = v1.push_order(&book.orders[index], candidate_full.fills[index]);
            let actual = ClearWorkFeedV2::open(&mut v2, widths)
                .unwrap()
                .push_order(&book.orders[index], candidate_full.fills[index]);
            assert_eq!(actual, expected);
        }
        let expected = v1.end_pass();
        let actual = ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass();
        assert_eq!(actual, expected);
    }
    assert_eq!(v2_verdict(&mut v2, widths), v1_verdict(&v1));
    assert_eq!(
        v1_verdict(&v1),
        Some(Err(super::relation_v1::ErrorV1::NonCanonicalPadding))
    );
}

#[test]
fn exact_geometry_and_native_idle_are_pinned() {
    let rows = [
        (ClearWorkWidthsV2::new(2, 0, 0), 1_350),
        (ClearWorkWidthsV2::new(2, 1, 1), 1_555),
        (ClearWorkWidthsV2::new(2, 4, 3), 2_070),
        (ClearWorkWidthsV2::new(4, 16, 8), 5_270),
        (ClearWorkWidthsV2::new(8, 32, 16), 12_934),
        (ClearWorkWidthsV2::new(16, 64, 64), 47_846),
    ];
    for (widths, expected) in rows {
        widths.validate().unwrap();
        assert_eq!(clear_work_v2_body_len(widths), expected);
        let mut cursor = 0usize;
        for region in [
            ClearWorkRegionV2::Control,
            ClearWorkRegionV2::Orders,
            ClearWorkRegionV2::Scratch,
            ClearWorkRegionV2::Flows,
            ClearWorkRegionV2::Pools,
            ClearWorkRegionV2::Ledger,
            ClearWorkRegionV2::Slices,
            ClearWorkRegionV2::Summary,
        ] {
            let span = clear_work_v2_region_span(widths, region);
            assert_eq!(span.offset, cursor);
            cursor = span.end();
        }
        assert_eq!(cursor, expected);
    }

    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut native = vec![0xA5; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut native, widths).unwrap();
    assert_eq!(
        validate_clear_work_v2(&native, widths).unwrap().phase,
        ClearWorkPhaseV2::Idle
    );
    assert_eq!(native, compact(&encoded(&ClearWorkV1::new()), widths));
}

#[test]
fn frozen_lifecycle_corpus_is_v1_byte_exact() {
    let common_widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut snapshots = Vec::new();
    for policy in [
        SelfCrossPolicyV1::AllowGateAtPairing,
        SelfCrossPolicyV1::NetAtAdmission,
    ] {
        for v1 in reachable_snapshots(policy) {
            snapshots.push((common_widths, v1));
        }
    }
    snapshots.extend(edge_snapshots());
    assert_eq!(snapshots.len(), 37);

    let mut corpus_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (widths, v1) in snapshots {
        let v2 = compact(&v1, widths);
        let mut rebuilt = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
        expand_clear_work_v2_into_v1_wire(&v2, widths, &mut rebuilt).unwrap();
        assert_eq!(rebuilt, v1);
        let mut decoded = ClearWorkV1::new();
        decoded.decode_into(&rebuilt).unwrap();
        assert_eq!(encoded(&decoded), v1);
        corpus_fingerprint ^= fingerprint(&v2);
        corpus_fingerprint = corpus_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
    assert_eq!(corpus_fingerprint, 9_044_710_648_940_933_722);
}

#[test]
fn hostile_bytes_are_total_and_accepted_images_are_closed() {
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let snapshots = reachable_snapshots(SelfCrossPolicyV1::AllowGateAtPairing);
    let base = compact(snapshots.last().unwrap(), widths);
    for at in 0..base.len() {
        let mut changed = base.clone();
        changed[at] ^= 0xff;
        if validate_clear_work_v2(&changed, widths).is_ok() {
            let mut v1 = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
            expand_clear_work_v2_into_v1_wire(&changed, widths, &mut v1).unwrap();
            let mut decoded = ClearWorkV1::new();
            decoded.decode_into(&v1).unwrap();
            let round_trip = compact(&encoded(&decoded), widths);
            assert_eq!(round_trip, changed);
        }
    }
    let mut long = base.clone();
    long.push(0);
    assert_eq!(
        validate_clear_work_v2(&base[..base.len() - 1], widths),
        Err(ClearWorkFaultV2::WrongLength)
    );
    assert_eq!(
        validate_clear_work_v2(&long, widths),
        Err(ClearWorkFaultV2::WrongLength)
    );
}

#[test]
fn every_accepted_single_byte_mutation_has_v1_exact_continuation() {
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let snapshots = reachable_snapshots(SelfCrossPolicyV1::AllowGateAtPairing);

    let mut accepted = 0usize;
    for source in snapshots {
        let base = compact(&source, widths);
        for at in 0..base.len() {
            let mut v2 = base.clone();
            v2[at] ^= 0xff;
            if validate_clear_work_v2(&v2, widths).is_err() {
                continue;
            }
            accepted += 1;

            let mut v1_wire = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
            expand_clear_work_v2_into_v1_wire(&v2, widths, &mut v1_wire).unwrap();
            let mut v1 = ClearWorkV1::new();
            v1.decode_into(&v1_wire).unwrap();

            let actual = catch_unwind(AssertUnwindSafe(|| {
                let mut work = ClearWorkFeedV2::open(&mut v2, widths).unwrap();
                match work.status() {
                    super::relation_v1_stream::FeedStatusV1::NeedOrders { .. }
                        if (work.orders_consumed() as usize) < book.len as usize =>
                    {
                        let index = work.orders_consumed() as usize;
                        work.push_order(&book.orders[index], candidate.fills[index])
                    }
                    _ => work.end_pass(),
                }
            }))
            .unwrap_or_else(|_| panic!("V2 panicked after snapshot mutation at byte {at}"));
            let expected = catch_unwind(AssertUnwindSafe(|| match v1.status() {
                super::relation_v1_stream::FeedStatusV1::NeedOrders { .. }
                    if (v1.orders_consumed() as usize) < book.len as usize =>
                {
                    let index = v1.orders_consumed() as usize;
                    v1.push_order(&book.orders[index], candidate.fills[index])
                }
                _ => v1.end_pass(),
            }))
            .unwrap_or_else(|_| panic!("V1 panicked after snapshot mutation at byte {at}"));
            assert_eq!(actual, expected, "snapshot mutation at byte {at}");
            assert_lockstep_checkpoint(&v1, &mut v2, widths);
        }
    }
    assert_eq!(accepted, 23_339, "accepted corpus moved; re-audit it");
}

#[test]
fn hostile_resumed_price_overflow_cannot_wrap_score() {
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let snapshots = reachable_snapshots(SelfCrossPolicyV1::AllowGateAtPairing);
    let mut v2 = compact(&snapshots[10], widths);

    // The high byte of the first persisted price is structurally valid, but
    // this corruption makes the dispersion score overflow. Resumption must be
    // total in debug and release builds, preserve the earlier economic refusal,
    // and produce the same non-wrapping checkpoint in both engines.
    v2[168] ^= 0xff;
    validate_clear_work_v2(&v2, widths).unwrap();

    let mut v1_wire = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
    expand_clear_work_v2_into_v1_wire(&v2, widths, &mut v1_wire).unwrap();
    let mut v1 = ClearWorkV1::new();
    v1.decode_into(&v1_wire).unwrap();

    assert_eq!(
        ClearWorkFeedV2::open(&mut v2, widths).unwrap().end_pass(),
        Ok(super::relation_v1_stream::FeedStatusV1::Complete)
    );
    assert_eq!(
        v1.end_pass(),
        Ok(super::relation_v1_stream::FeedStatusV1::Complete)
    );
    assert_eq!(
        v2_verdict(&mut v2, widths),
        Some(Err(super::relation_v1::ErrorV1::ConsiderationMismatch))
    );
    assert_eq!(v1_verdict(&v1), v2_verdict(&mut v2, widths));
    assert_lockstep_checkpoint(&v1, &mut v2, widths);
}

#[test]
fn omitted_v1_padding_and_active_width_forgery_are_refused() {
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let idle = encoded(&ClearWorkV1::new());
    let omitted = [
        161 + 2 * 8,
        390 + 3 * 2,
        520 + 4 * 2,
        5_200 + 2 * 8,
        46_672 + 1 + 2 * 8,
    ];
    for at in omitted {
        let mut changed = idle.clone();
        changed[at] ^= 1;
        let mut out = vec![0u8; clear_work_v2_body_len(widths)];
        let mut scratch = vec![0u8; CLEAR_WORK_V1_BODY_BYTES];
        assert_eq!(
            project_clear_work_v1_wire_into_v2(&changed, widths, &mut out, &mut scratch),
            Err(ClearWorkFaultV2::NonCanonicalPadding),
            "omitted V1 byte {at}"
        );
    }

    let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let mut work = ClearWorkV1::new();
    work.begin(&domain, &header(&candidate), true).unwrap();
    let active = compact(&encoded(&work), widths);
    for at in [126usize, 127] {
        let mut forged = active.clone();
        forged[at] = forged[at].wrapping_add(1);
        assert_eq!(
            validate_clear_work_v2(&forged, widths),
            Err(ClearWorkFaultV2::WidthBindingMismatch)
        );
    }
    let mut mismatched_candidate_len = active.clone();
    mismatched_candidate_len[160] = mismatched_candidate_len[160].wrapping_add(1);
    assert!(validate_clear_work_v2(&mismatched_candidate_len, widths).is_ok());
}

#[test]
fn native_region_primitives_are_bounded_and_do_not_expand() {
    let widths = ClearWorkWidthsV2::new(2, 4, 3);
    let mut body = vec![0u8; clear_work_v2_body_len(widths)];
    initialize_clear_work_v2_idle(&mut body, widths).unwrap();
    {
        let mut view = ClearWorkViewMutV2::open(&mut body, widths).unwrap();
        view.set_matrix_u64(MatrixU64V2::ScratchBuy, 3, 1, 91)
            .unwrap();
        view.set_matrix_u64(MatrixU64V2::ParticipationSell, 2, 0, 92)
            .unwrap();
        view.set_owner_units(OwnerUnitsV2::Debit, 2, 93).unwrap();
        view.set_outcome_flow(OutcomeFlowV2::Buy, 1, 94).unwrap();
        view.set_outcome_aggregate(OutcomeAggregateV2::StrictSell, 0, 95)
            .unwrap();
        assert_eq!(
            view.set_matrix_u64(MatrixU64V2::ScratchBuy, 4, 0, 1),
            Err(ClearWorkFaultV2::InvalidIndex)
        );
        assert_eq!(
            view.set_owner_units(OwnerUnitsV2::Debit, 3, 1),
            Err(ClearWorkFaultV2::InvalidIndex)
        );
    }
    let view = ClearWorkViewV2::open(&body, widths).unwrap();
    assert_eq!(view.matrix_u64(MatrixU64V2::ScratchBuy, 3, 1), Ok(91));
    assert_eq!(
        view.matrix_u64(MatrixU64V2::ParticipationSell, 2, 0),
        Ok(92)
    );
    assert_eq!(view.owner_units(OwnerUnitsV2::Debit, 2), Ok(93));
    assert_eq!(view.outcome_flow(OutcomeFlowV2::Buy, 1), Ok(94));
    assert_eq!(
        view.outcome_aggregate(OutcomeAggregateV2::StrictSell, 0),
        Ok(95)
    );

    assert_eq!(core::mem::size_of::<ClearWorkViewV2<'_>>(), 32);
    assert_eq!(core::mem::size_of::<ClearWorkViewMutV2<'_>>(), 32);
    assert_eq!(core::mem::size_of::<ClearWorkFeedV2<'_>>(), 1_776);
    assert!(core::mem::size_of::<ClearWorkViewV2<'_>>() * 20 < body.len());
}
