use super::relation_v1::{
    canonical_candidate, canonical_pairing, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1,
    FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1, PortfolioLotPolicyV1,
    RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1,
    SingleEggOrderV1, TransferPhaseV1, MAX_OUTCOMES, PRICE_SCALE, RELATION_VERSION_V1,
};
use super::relation_v1_stream::{ClearWorkV1, FeedErrorV1, StreamCandidateV1};
use super::relation_v1_stream_v2::*;
use super::{DustPolicy, PartialPolicy, Side};

extern crate std;
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
    for at in [126usize, 127, 160] {
        let mut forged = active.clone();
        forged[at] = forged[at].wrapping_add(1);
        assert_eq!(
            validate_clear_work_v2(&forged, widths),
            Err(ClearWorkFaultV2::WidthBindingMismatch)
        );
    }
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
    assert!(core::mem::size_of::<ClearWorkViewV2<'_>>() * 20 < body.len());
}
