//! The streaming/batch equivalence gate.
//!
//! Every test here asserts *verdict identity* between
//! [`crate::relation_v1::verify`] and the streaming feed: same `SummaryV1` on
//! acceptance, same `ErrorV1` — payloads included — on refusal.  The domains
//! are the bounded enumerations the batch oracle already trusts (the
//! 2,592-book domain and the pairing-feasibility flow tables), plus the policy
//! fixtures and the P-BATCH-03 resumption obligation.  A divergence is a
//! finding, never a tune.

use super::*;
use crate::relation_v1::{
    canonical_candidate, canonical_pairing, verify, AllocationPolicyV1, AonPolicyV1, BookV1,
    CandidateV1, ErrorV1, FeeBaseV1, FrozenPolicyV1, LegRefV1, OrderV1, PairingSliceV1,
    PairingWitnessPolicyV1, PairingWitnessV1, PortfolioLotPolicyV1, PortfolioOrderV1,
    RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1, ScoreV1,
    SelfCrossPolicyV1, SingleEggOrderV1, SummaryV1, TransferPhaseV1, MAX_OUTCOMES, PRICE_SCALE,
    RELATION_VERSION_V1,
};
use crate::{DustPolicy, PartialPolicy, Side, MAX_ORDERS};

extern crate std;
use std::boxed::Box;
use std::vec::Vec;

const SCALE: u64 = PRICE_SCALE;

fn base_policy() -> FrozenPolicyV1 {
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

fn domain_with(policy: FrozenPolicyV1, outcomes: u8, owners: u16) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: outcomes,
        owner_count: owners,
        price_scale: SCALE,
        remainder_seed: 0x00C0_FFEE,
        policy,
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

#[allow(clippy::too_many_arguments)]
fn single_min(
    id: u64,
    owner: u16,
    outcome: u8,
    side: Side,
    quantity: u64,
    limit: u64,
    minimum: u64,
    partial: PartialPolicy,
) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: minimum,
        partial_policy: partial,
        expiry_epoch: u64::MAX,
    })
}

fn portfolio(
    id: u64,
    owner: u16,
    side: Side,
    coefficients: &[u64],
    lots: u64,
    limit_per_lot: u64,
) -> OrderV1 {
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < coefficients.len() {
        vector[i] = coefficients[i];
        i += 1;
    }
    OrderV1::Portfolio(PortfolioOrderV1 {
        canonical_order_id: id,
        owner,
        side,
        coefficients: vector,
        active_len: coefficients.len() as u8,
        lots,
        limit_collateral_per_lot: limit_per_lot,
        minimum_fill_lots: 1,
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

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < values.len() {
        vector[i] = values[i];
        i += 1;
    }
    vector
}

fn crossing_book() -> BookV1 {
    book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ])
}

fn header_of(candidate: &CandidateV1, pairing: Option<&PairingWitnessV1>) -> StreamCandidateV1 {
    StreamCandidateV1 {
        order_len: candidate.order_len,
        prices: candidate.prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        claimed_score: candidate.claimed_score,
        canonical_candidate_digest: candidate.canonical_candidate_digest,
        declared_slices: pairing.map(|witness| witness.len),
    }
}

/// Drive one whole feed and return the batch-shaped verdict.
fn drive(
    work: &mut ClearWorkV1,
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<SummaryV1, ErrorV1> {
    let header = header_of(candidate, pairing);
    work.begin(domain, &header, true).unwrap();
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
            FeedStatusV1::NeedSlices => {
                let witness = pairing.expect("slices demanded without a witness");
                let mut k = 0usize;
                while k < witness.len as usize {
                    work.push_slice(&witness.slices[k]).unwrap();
                    k += 1;
                }
                work.end_pass().unwrap();
            }
            FeedStatusV1::Complete => break,
        }
    }
    work.verdict()
        .expect("complete feed must have a verdict")
        .copied()
}

/// The gate: batch and stream verdicts must be identical.
fn assert_stream_matches(
    work: &mut ClearWorkV1,
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
    pairing: Option<&PairingWitnessV1>,
) {
    let batch = verify(domain, book, candidate, pairing);
    let stream = drive(work, domain, book, candidate, pairing);
    assert_eq!(
        batch, stream,
        "stream verdict diverged from the batch verifier: domain {:?} book {:?} candidate {:?}",
        domain, book, candidate
    );
}

/// The seven mutation families applied to every accepted candidate.
fn mutations(candidate: &CandidateV1) -> Vec<CandidateV1> {
    let mut out = Vec::new();
    let mut bumped = *candidate;
    bumped.fills[0] = bumped.fills[0].wrapping_add(1);
    out.push(bumped);
    let mut zeroed = *candidate;
    let mut j = 0usize;
    while j < candidate.order_len as usize {
        if zeroed.fills[j] != 0 {
            zeroed.fills[j] = 0;
            break;
        }
        j += 1;
    }
    out.push(zeroed);
    let mut split = *candidate;
    split.virtual_split += 1;
    out.push(split);
    let mut merge = *candidate;
    merge.virtual_merge += 1;
    out.push(merge);
    let mut score = *candidate;
    score.claimed_score.churn = score.claimed_score.churn.wrapping_add(1);
    out.push(score);
    let mut digest = *candidate;
    digest.canonical_candidate_digest ^= 1;
    out.push(digest);
    let mut swapped = *candidate;
    swapped.prices.swap(0, 1);
    out.push(swapped);
    out
}

#[test]
fn stream_matches_batch_on_the_bounded_exhaustive_book_domain() {
    // The 2,592-book domain of the batch oracle (1,296 shapes x 2 owner
    // layouts), all 9 `(p, c)` coordinates each: every canonical candidate
    // verified through both paths, and every accepted candidate re-verified
    // under seven mutation families.
    let quantities = [1u64, 2u64];
    let limits = [0u64, SCALE / 2, SCALE];
    let price_ticks = [SCALE / 4, SCALE / 2, 3 * SCALE / 4];
    let domain = domain_with(base_policy(), 2, 3);
    let mut work = Box::new(ClearWorkV1::new());
    let mut accepted = 0u32;
    let mut refused_mutations = 0u32;
    let mut compared = 0u32;

    let mut shape = 0usize;
    while shape < 1296 {
        let mut digits = [0usize; 4];
        let mut value = shape;
        let mut d = 0usize;
        while d < 4 {
            digits[d] = value % 6;
            value /= 6;
            d += 1;
        }
        let mut owner_layout = 0usize;
        while owner_layout < 2 {
            let sell_owner = if owner_layout == 0 { 1u16 } else { 0u16 };
            let book = book_of(&[
                single(
                    1,
                    0,
                    0,
                    Side::Buy,
                    quantities[digits[0] % 2],
                    limits[digits[0] / 2],
                ),
                single(
                    2,
                    sell_owner,
                    0,
                    Side::Sell,
                    quantities[digits[1] % 2],
                    limits[digits[1] / 2],
                ),
                single(
                    3,
                    2,
                    1,
                    Side::Buy,
                    quantities[digits[2] % 2],
                    limits[digits[2] / 2],
                ),
                single(
                    4,
                    1,
                    1,
                    Side::Sell,
                    quantities[digits[3] % 2],
                    limits[digits[3] / 2],
                ),
            ]);
            let mut tick = 0usize;
            while tick < price_ticks.len() {
                let vector = prices(&[price_ticks[tick], SCALE - price_ticks[tick]]);
                let mut imbalance = -1i64;
                while imbalance <= 1 {
                    if let Ok(candidate) =
                        canonical_candidate(&domain, &book, &vector, imbalance, 0)
                    {
                        accepted += 1;
                        assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                        compared += 1;
                        for mutated in mutations(&candidate) {
                            if verify(&domain, &book, &mutated, None).is_err() {
                                refused_mutations += 1;
                            }
                            assert_stream_matches(&mut work, &domain, &book, &mutated, None);
                            compared += 1;
                        }
                    }
                    imbalance += 1;
                }
                tick += 1;
            }
            owner_layout += 1;
        }
        shape += 1;
    }
    assert!(
        accepted > 1000,
        "the oracle must accept something: {}",
        accepted
    );
    assert!(
        refused_mutations > 1000,
        "the mutations must refuse something: {}",
        refused_mutations
    );
    assert!(
        compared > 8000,
        "the gate must actually compare: {}",
        compared
    );
}

fn flow_book(buys: &[[u64; 3]; 2], sells: &[[u64; 3]; 2]) -> (BookV1, [u64; MAX_ORDERS], u8) {
    let mut orders = [crate::relation_v1::empty_order_v1(); MAX_ORDERS];
    let mut fills = [0u64; MAX_ORDERS];
    let mut count = 0usize;
    let mut identifier = 1u64;
    let mut outcome = 0usize;
    while outcome < 2 {
        let mut owner = 0usize;
        while owner < 3 {
            if buys[outcome][owner] != 0 {
                orders[count] = single(
                    identifier,
                    owner as u16,
                    outcome as u8,
                    Side::Buy,
                    buys[outcome][owner],
                    SCALE,
                );
                fills[count] = buys[outcome][owner];
                count += 1;
                identifier += 1;
            }
            if sells[outcome][owner] != 0 {
                orders[count] = single(
                    identifier,
                    owner as u16,
                    outcome as u8,
                    Side::Sell,
                    sells[outcome][owner],
                    0,
                );
                fills[count] = sells[outcome][owner];
                count += 1;
                identifier += 1;
            }
            owner += 1;
        }
        outcome += 1;
    }
    let mut book = BookV1::empty();
    book.orders = orders;
    book.len = count as u8;
    (book, fills, count as u8)
}

fn candidate_for_fills(
    order_len: u8,
    split: u64,
    merge: u64,
    fills: [u64; MAX_ORDERS],
) -> CandidateV1 {
    CandidateV1 {
        order_len,
        prices: prices(&[SCALE / 2, SCALE / 2]),
        virtual_split: split,
        virtual_merge: merge,
        fills,
        honored_aon_mask: 0,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
    }
}

#[test]
fn stream_matches_batch_on_the_pairing_feasibility_tables() {
    // The 4,096-code owner/side flow-table enumeration behind the batch
    // pairing oracle, driven through `verify` on both paths.  On infeasible
    // tables the identity includes the `PairingInfeasible { outcome, owner }`
    // payload.
    let domain = domain_with(base_policy(), 2, 3);
    let mut work = Box::new(ClearWorkV1::new());
    let mut checked = 0u32;
    let mut infeasible = 0u32;

    let mut code = 0u32;
    while code < 4096 {
        let mut buy = [0u64; 3];
        let mut sell = [0u64; 3];
        let mut digit = 0usize;
        while digit < 3 {
            buy[digit] = ((code >> (2 * digit)) & 3) as u64;
            sell[digit] = ((code >> (6 + 2 * digit)) & 3) as u64;
            digit += 1;
        }
        let buy_total: u64 = buy[0] + buy[1] + buy[2];
        let sell_total: u64 = sell[0] + sell[1] + sell[2];
        let mut conversion = 0i64;
        while conversion <= 3 {
            let mut sign = 0usize;
            while sign < 2 {
                let split = if sign == 0 { conversion as u64 } else { 0 };
                let merge = if sign == 0 { 0 } else { conversion as u64 };
                if (sign == 1 && conversion == 0) || buy_total + merge != sell_total + split {
                    sign += 1;
                    continue;
                }
                let mut buys = [[0u64; 3]; 2];
                let mut sells = [[0u64; 3]; 2];
                buys[0] = buy;
                sells[0] = sell;
                if split != 0 {
                    buys[1][0] = split;
                }
                if merge != 0 {
                    sells[1][0] = merge;
                }
                let (book, fills, len) = flow_book(&buys, &sells);
                if len == 0 {
                    sign += 1;
                    continue;
                }
                let candidate = candidate_for_fills(len, split, merge, fills);
                let batch = verify(&domain, &book, &candidate, None);
                if matches!(batch, Err(ErrorV1::PairingInfeasible { .. })) {
                    infeasible += 1;
                }
                assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                checked += 1;
                sign += 1;
            }
            conversion += 1;
        }
        code += 1;
    }
    assert!(
        checked > 3000,
        "the table oracle must actually search: {}",
        checked
    );
    assert!(
        infeasible > 100,
        "the payload branch must be exercised: {}",
        infeasible
    );
}

#[test]
fn stream_matches_batch_on_the_coupled_outcome_tables() {
    // The coupled-outcome variant: both outcomes carry owner flow and one
    // shared global conversion.
    let domain = domain_with(base_policy(), 2, 3);
    let mut work = Box::new(ClearWorkV1::new());
    let mut checked = 0u32;
    let mut code = 0u32;
    while code < 4096 {
        let mut buys = [[0u64; 3]; 2];
        let mut sells = [[0u64; 3]; 2];
        let mut digit = 0usize;
        while digit < 3 {
            buys[0][digit] = ((code >> (2 * digit)) & 1) as u64;
            sells[0][digit] = ((code >> (3 + 2 * digit)) & 1) as u64;
            buys[1][digit] = ((code >> (6 + 2 * digit)) & 1) as u64;
            sells[1][digit] = ((code >> (9 + 2 * digit)) & 1) as u64;
            digit += 1;
        }
        let mut conversion = -1i64;
        while conversion <= 1 {
            let split = if conversion > 0 { conversion as u64 } else { 0 };
            let merge = if conversion < 0 {
                (-conversion) as u64
            } else {
                0
            };
            let mut balanced = true;
            let mut outcome = 0usize;
            while outcome < 2 {
                let b: u64 = buys[outcome].iter().sum();
                let e: u64 = sells[outcome].iter().sum();
                if b + merge != e + split {
                    balanced = false;
                }
                outcome += 1;
            }
            if balanced {
                let (book, fills, len) = flow_book(&buys, &sells);
                if len != 0 {
                    let candidate = candidate_for_fills(len, split, merge, fills);
                    assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                    checked += 1;
                }
            }
            conversion += 1;
        }
        code += 1;
    }
    assert!(
        checked > 500,
        "the coupled oracle must actually search: {}",
        checked
    );
}

#[test]
fn stream_matches_batch_on_self_cross_policy_variants() {
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single(2, 0, 0, Side::Sell, 2, 0),
        single(3, 1, 0, Side::Sell, 2, 0),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    for self_cross in [
        SelfCrossPolicyV1::RefuseOverlap,
        SelfCrossPolicyV1::NetAtAdmission,
        SelfCrossPolicyV1::AllowGateAtPairing,
    ] {
        let domain = domain_with(
            FrozenPolicyV1 {
                self_cross,
                ..base_policy()
            },
            2,
            2,
        );
        // The canonical candidate at the coordinates, when one exists.
        match canonical_candidate(&domain, &book, &vector, 0, 0) {
            Ok(candidate) => {
                assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                for mutated in mutations(&candidate) {
                    assert_stream_matches(&mut work, &domain, &book, &mutated, None);
                }
            }
            Err(_) => {
                // Hand candidates still exercise the refusal identity.
                let mut fills = [0u64; MAX_ORDERS];
                fills[0] = 2;
                fills[1] = 2;
                let mut candidate = CandidateV1::empty(3, vector);
                candidate.fills = fills;
                assert_stream_matches(&mut work, &domain, &book, &candidate, None);
            }
        }
        // The all-zero candidate and a full-overlap candidate on every variant.
        let empty = CandidateV1::empty(3, vector);
        assert_stream_matches(&mut work, &domain, &book, &empty, None);
        let mut full = CandidateV1::empty(3, vector);
        full.fills[0] = 3;
        full.fills[1] = 2;
        full.fills[2] = 1;
        assert_stream_matches(&mut work, &domain, &book, &full, None);
    }
    // N-b with an all-or-none order in the overlap refuses at netting; the
    // stream must agree from the same coordinates.
    let aon_overlap = book_of(&[
        single_min(1, 0, 0, Side::Buy, 3, SCALE, 3, PartialPolicy::AllOrNone),
        single(2, 0, 0, Side::Sell, 2, 0),
        single(3, 1, 0, Side::Sell, 2, 0),
    ]);
    let netting = domain_with(
        FrozenPolicyV1 {
            self_cross: SelfCrossPolicyV1::NetAtAdmission,
            aon: AonPolicyV1::FullSizeCounting,
            ..base_policy()
        },
        2,
        2,
    );
    let empty = CandidateV1::empty(3, vector);
    assert_stream_matches(&mut work, &netting, &aon_overlap, &empty, None);
    // N-b with a portfolio in the overlap refuses.
    let portfolio_overlap = book_of(&[
        portfolio(1, 0, Side::Buy, &[1, 1], 2, SCALE),
        single(2, 0, 0, Side::Sell, 2, 0),
        single(3, 1, 0, Side::Sell, 2, 0),
    ]);
    assert_stream_matches(&mut work, &netting, &portfolio_overlap, &empty, None);
}

#[test]
fn stream_matches_batch_on_the_masked_aon_domain() {
    // The two-cycle AON book under the witnessed-mask policy: all 16 masks,
    // canonical candidates where they exist, and the poisoned masks.
    let masked = domain_with(
        FrozenPolicyV1 {
            aon: AonPolicyV1::WitnessedHonoredMask,
            ..base_policy()
        },
        2,
        4,
    );
    let book = book_of(&[
        single_min(1, 0, 0, Side::Buy, 4, SCALE, 4, PartialPolicy::AllOrNone),
        single_min(2, 1, 0, Side::Sell, 4, 0, 4, PartialPolicy::AllOrNone),
        single_min(3, 2, 1, Side::Buy, 4, SCALE, 4, PartialPolicy::AllOrNone),
        single_min(4, 3, 1, Side::Sell, 4, 0, 4, PartialPolicy::AllOrNone),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    let mut compared = 0u32;
    let mut mask = 0u64;
    while mask < 16 {
        match canonical_candidate(&masked, &book, &vector, 0, mask) {
            Ok(candidate) => {
                assert_stream_matches(&mut work, &masked, &book, &candidate, None);
                compared += 1;
                // A mask bit flipped after finalization is a poisoned witness.
                let mut poisoned = candidate;
                poisoned.honored_aon_mask ^= 1;
                assert_stream_matches(&mut work, &masked, &book, &poisoned, None);
                compared += 1;
            }
            Err(_) => {
                let mut candidate = CandidateV1::empty(4, vector);
                candidate.honored_aon_mask = mask;
                assert_stream_matches(&mut work, &masked, &book, &candidate, None);
                compared += 1;
            }
        }
        mask += 1;
    }
    // A mask claiming an order with no minimum obligation.
    let plain = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    let candidate = canonical_candidate(&masked, &plain, &vector, 0, 0).unwrap();
    let mut claimed = candidate;
    claimed.honored_aon_mask = 1;
    assert_stream_matches(&mut work, &masked, &plain, &claimed, None);
    // A nonzero mask under a policy with no mask.
    let unmasked = domain_with(base_policy(), 2, 4);
    let candidate = canonical_candidate(&unmasked, &plain, &vector, 0, 0).unwrap();
    let mut claimed = candidate;
    claimed.honored_aon_mask = 2;
    assert_stream_matches(&mut work, &unmasked, &plain, &claimed, None);
    assert!(compared >= 16);
}

#[test]
fn stream_matches_batch_on_the_derived_vector_obligation_corner() {
    // Design §5: under AON policy 2c a marginal all-or-none or minimum-fill
    // order can make the *canonical* fills violate their own obligation, and
    // the batch verdict is then a fact about the derived vector even when the
    // submitted fills differ.  The stream must reproduce it exactly.
    let policy = FrozenPolicyV1 {
        aon: AonPolicyV1::FullSizeCounting,
        ..base_policy()
    };
    let domain = domain_with(policy, 2, 3);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    // Marginal AON sell whose pro-rata share is a partial fill.
    let aon_book = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single_min(
            2,
            1,
            0,
            Side::Sell,
            2,
            SCALE / 2,
            2,
            PartialPolicy::AllOrNone,
        ),
        single(3, 2, 0, Side::Sell, 2, SCALE / 2),
    ]);
    // Marginal minimum-fill sell in the same shape.
    let min_book = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single_min(2, 1, 0, Side::Sell, 2, SCALE / 2, 2, PartialPolicy::Allow),
        single(3, 2, 0, Side::Sell, 2, SCALE / 2),
    ]);
    for book in [&aon_book, &min_book] {
        let mut imbalance = -2i64;
        while imbalance <= 2 {
            match canonical_candidate(&domain, book, &vector, imbalance, 0) {
                Ok(candidate) => {
                    assert_stream_matches(&mut work, &domain, book, &candidate, None);
                    for mutated in mutations(&candidate) {
                        assert_stream_matches(&mut work, &domain, book, &mutated, None);
                    }
                }
                Err(_) => {
                    // Submit non-canonical fills so the batch path reaches the
                    // derived-vector walk.
                    let mut candidate = CandidateV1::empty(3, vector);
                    candidate.virtual_split = if imbalance > 0 { imbalance as u64 } else { 0 };
                    candidate.virtual_merge = if imbalance < 0 {
                        imbalance.unsigned_abs()
                    } else {
                        0
                    };
                    candidate.fills[0] = 3;
                    candidate.fills[1] = 0;
                    candidate.fills[2] = 2;
                    assert_stream_matches(&mut work, &domain, book, &candidate, None);
                }
            }
            imbalance += 1;
        }
    }
}

#[test]
fn stream_matches_batch_on_allocation_and_dust_variants() {
    // Allocation B and the dust machinery: marginal pools with nonzero dust,
    // both dust policies, and the dust-atom transfer forgery that conserves
    // every flow and violates only largest-remainder canonicality — the
    // falsifier aimed straight at the key table of design §5.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    let mut compared = 0u32;
    for allocation in [
        AllocationPolicyV1::PricePriorityMarginalProRata,
        AllocationPolicyV1::FullProRata,
    ] {
        for dust in [DustPolicy::AssignCanonical, DustPolicy::Reject] {
            let domain = domain_with(
                FrozenPolicyV1 {
                    allocation,
                    dust,
                    ..base_policy()
                },
                2,
                4,
            );
            let mut buy_quantity = 1u64;
            while buy_quantity <= 4 {
                let book = book_of(&[
                    single(1, 0, 0, Side::Buy, buy_quantity, SCALE),
                    single(2, 1, 0, Side::Sell, 2, SCALE / 2),
                    single(3, 2, 0, Side::Sell, 1, SCALE / 2),
                    single(4, 3, 0, Side::Sell, 3, SCALE / 2),
                ]);
                let mut imbalance = -1i64;
                while imbalance <= 1 {
                    if let Ok(candidate) =
                        canonical_candidate(&domain, &book, &vector, imbalance, 0)
                    {
                        assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                        compared += 1;
                        for mutated in mutations(&candidate) {
                            assert_stream_matches(&mut work, &domain, &book, &mutated, None);
                            compared += 1;
                        }
                        // Transfer one dust atom between two pool members.
                        let mut donor = usize::MAX;
                        let mut receiver = usize::MAX;
                        let mut j = 1usize;
                        while j < 4 {
                            if candidate.fills[j] != 0 && donor == usize::MAX {
                                donor = j;
                            } else if candidate.fills[j] < book_quantity(&book, j)
                                && receiver == usize::MAX
                                && j != donor
                            {
                                receiver = j;
                            }
                            j += 1;
                        }
                        if donor != usize::MAX && receiver != usize::MAX {
                            let mut moved = candidate;
                            moved.fills[donor] -= 1;
                            moved.fills[receiver] += 1;
                            assert_stream_matches(&mut work, &domain, &book, &moved, None);
                            compared += 1;
                        }
                    }
                    imbalance += 1;
                }
                buy_quantity += 1;
            }
        }
    }
    assert!(
        compared > 50,
        "the variant oracle must compare: {}",
        compared
    );
}

fn book_quantity(book: &BookV1, index: usize) -> u64 {
    book.orders[index].quantity()
}

#[test]
fn stream_matches_batch_on_portfolio_books() {
    let domain = domain_with(base_policy(), 3, 3);
    let book = book_of(&[
        portfolio(1, 0, Side::Buy, &[1, 1, 1], 2, SCALE),
        single(2, 1, 0, Side::Sell, 2, 0),
        single(3, 1, 1, Side::Sell, 2, 0),
        single(4, 2, 2, Side::Sell, 2, 0),
    ]);
    let vector = prices(&[SCALE / 4, SCALE / 4, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    let mut imbalance = -2i64;
    while imbalance <= 2 {
        if let Ok(candidate) = canonical_candidate(&domain, &book, &vector, imbalance, 0) {
            assert_stream_matches(&mut work, &domain, &book, &candidate, None);
            for mutated in mutations(&candidate) {
                assert_stream_matches(&mut work, &domain, &book, &mutated, None);
            }
        }
        imbalance += 1;
    }
    // The all-zero candidate and a lot-coupling forgery.
    let empty = CandidateV1::empty(4, vector);
    assert_stream_matches(&mut work, &domain, &book, &empty, None);
    let mut forged = CandidateV1::empty(4, vector);
    forged.fills[0] = 1;
    forged.fills[1] = 1;
    assert_stream_matches(&mut work, &domain, &book, &forged, None);
}

#[test]
fn stream_matches_batch_on_fee_and_rounding_variants() {
    let book = crossing_book();
    let vector = prices(&[SCALE / 3, SCALE - SCALE / 3]);
    let mut work = Box::new(ClearWorkV1::new());
    for rounding in [
        RoundingBoundaryV1::None,
        RoundingBoundaryV1::TerminalOwnerFloor,
        RoundingBoundaryV1::ReceiptFloor,
    ] {
        for fee_base in [FeeBaseV1::None, FeeBaseV1::FlatNotional { bps: 30 }] {
            let domain = domain_with(
                FrozenPolicyV1 {
                    rounding,
                    fee_base,
                    ..base_policy()
                },
                2,
                2,
            );
            match canonical_candidate(&domain, &book, &vector, 0, 0) {
                Ok(candidate) => {
                    assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                    for mutated in mutations(&candidate) {
                        assert_stream_matches(&mut work, &domain, &book, &mutated, None);
                    }
                }
                Err(_) => {
                    let mut candidate = CandidateV1::empty(2, vector);
                    candidate.fills[0] = 4;
                    candidate.fills[1] = 4;
                    assert_stream_matches(&mut work, &domain, &book, &candidate, None);
                }
            }
        }
    }
}

#[test]
fn stream_matches_batch_on_explicit_slice_witnesses() {
    let explicit = domain_with(
        FrozenPolicyV1 {
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            ..base_policy()
        },
        2,
        2,
    );
    let recomputed = domain_with(base_policy(), 2, 2);
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    let candidate = canonical_candidate(&explicit, &book, &vector, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit, &book, &candidate).unwrap();
    // The verbatim witness accepts identically.
    assert_stream_matches(&mut work, &explicit, &book, &candidate, Some(&witness));
    // A missing witness under the explicit policy.
    assert_stream_matches(&mut work, &explicit, &book, &candidate, None);
    // A witness under the recomputed policy.
    let plain = canonical_candidate(&recomputed, &book, &vector, 0, 0).unwrap();
    assert_stream_matches(&mut work, &recomputed, &book, &plain, Some(&witness));
    // Forged slices: short quantity, reversed reference, virtual self-pair.
    let mut short = witness;
    short.slices[0].quantity = 3;
    assert_stream_matches(&mut work, &explicit, &book, &candidate, Some(&short));
    let mut reversed = witness;
    reversed.slices[0].buy_ref = LegRefV1::Order(1);
    assert_stream_matches(&mut work, &explicit, &book, &candidate, Some(&reversed));
    let mut virtual_self = PairingWitnessV1::empty();
    virtual_self.slices[0] = PairingSliceV1 {
        buy_ref: LegRefV1::Merge,
        sell_ref: LegRefV1::Split,
        outcome: 0,
        quantity: 1,
    };
    virtual_self.len = 1;
    assert_stream_matches(&mut work, &explicit, &book, &candidate, Some(&virtual_self));
    // A witness with churn: split and merge legs on a two-outcome book.
    let churned = canonical_candidate(&explicit, &book, &vector, 1, 0);
    if let Ok(churned) = churned {
        let churn_witness = canonical_pairing(&explicit, &book, &churned).unwrap();
        assert_stream_matches(&mut work, &explicit, &book, &churned, Some(&churn_witness));
    }
}

#[test]
fn stream_matches_batch_on_admission_and_domain_refusals() {
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut work = Box::new(ClearWorkV1::new());
    // A bad domain refuses before any order is read.
    let mut bad_domain = domain_with(base_policy(), 2, 2);
    bad_domain.relation_version = 9;
    let candidate = CandidateV1::empty(2, vector);
    assert_stream_matches(&mut work, &bad_domain, &crossing_book(), &candidate, None);
    // Per-order admission refusals, each at its own index.
    let domain = domain_with(base_policy(), 2, 2);
    let books = [
        book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(1, 1, 0, Side::Sell, 4, 0),
        ]),
        book_of(&[single(1, 5, 0, Side::Buy, 4, SCALE)]),
        book_of(&[single(1, 0, 5, Side::Buy, 4, SCALE)]),
        book_of(&[single(1, 0, 0, Side::Buy, 0, SCALE)]),
        book_of(&[single(1, 0, 0, Side::Buy, 4, SCALE + 1)]),
        book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single_min(2, 1, 0, Side::Sell, 4, 0, 5, PartialPolicy::Allow),
        ]),
        book_of(&[single_min(
            1,
            0,
            0,
            Side::Buy,
            4,
            SCALE,
            4,
            PartialPolicy::AllOrNone,
        )]),
        book_of(&[{
            let mut expired = single(1, 0, 0, Side::Buy, 4, SCALE);
            if let OrderV1::SingleEgg(ref mut o) = expired {
                o.expiry_epoch = 1;
            }
            expired
        }]),
    ];
    for book in &books {
        let candidate = CandidateV1::empty(book.len, vector);
        assert_stream_matches(&mut work, &domain, book, &candidate, None);
    }
    // The order-length gate: a candidate binding a different count.
    let book = crossing_book();
    let mut mislengthed = CandidateV1::empty(3, vector);
    mislengthed.prices = prices(&[SCALE, SCALE]);
    assert_stream_matches(&mut work, &domain, &book, &mislengthed, None);
    // The epoch-lapse empty book.
    let empty_book = BookV1::empty();
    let lapse = canonical_candidate(&domain, &empty_book, &vector, 0, 0).unwrap();
    assert_stream_matches(&mut work, &domain, &empty_book, &lapse, None);
}

#[test]
// The fresh `Box` per split is the point: resumption must depend only on the
// checkpoint *value*, so every split moves it to new storage.
#[allow(clippy::replace_box)]
fn stream_resumption_equals_one_ordered_fold() {
    // P-BATCH-03: for every split of the feed, saving and restoring the
    // checkpoint object between chunks yields the identical verdict and the
    // identical final state as the uninterrupted fold.  Save = clone; resume =
    // continue on the clone, discarding the original.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let explicit = domain_with(
        FrozenPolicyV1 {
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            ..base_policy()
        },
        2,
        2,
    );
    let netting = domain_with(
        FrozenPolicyV1 {
            self_cross: SelfCrossPolicyV1::NetAtAdmission,
            ..base_policy()
        },
        2,
        2,
    );
    let plain = domain_with(base_policy(), 2, 3);
    let cross = crossing_book();
    let self_cross_book = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single(2, 0, 0, Side::Sell, 2, 0),
        single(3, 1, 0, Side::Sell, 1, 0),
    ]);
    let four = book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 1, 0, Side::Sell, 2, 0),
        single(3, 2, 1, Side::Buy, 1, SCALE),
        single(4, 1, 1, Side::Sell, 1, 0),
    ]);

    let mut cases: Vec<(
        RelationDomainV1,
        BookV1,
        CandidateV1,
        Option<PairingWitnessV1>,
    )> = Vec::new();
    let accepted = canonical_candidate(&plain, &four, &vector, 0, 0).unwrap();
    cases.push((plain, four, accepted, None));
    for mutated in mutations(&accepted) {
        cases.push((plain, four, mutated, None));
    }
    // After netting, one effective buy stands against one strict sell.
    let netted = canonical_candidate(&netting, &self_cross_book, &vector, 0, 0).unwrap();
    cases.push((netting, self_cross_book, netted, None));
    for mutated in mutations(&netted) {
        cases.push((netting, self_cross_book, mutated, None));
    }
    let sliced = canonical_candidate(&explicit, &cross, &vector, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit, &cross, &sliced).unwrap();
    cases.push((explicit, cross, sliced, Some(witness)));
    let mut short = witness;
    short.slices[0].quantity = 3;
    cases.push((explicit, cross, sliced, Some(short)));

    let mut splits_exercised = 0u32;
    for (domain, book, candidate, pairing) in &cases {
        let pairing = pairing.as_ref();
        // The uninterrupted fold.
        let mut whole = Box::new(ClearWorkV1::new());
        let whole_verdict = drive(&mut whole, domain, book, candidate, pairing);
        assert_eq!(whole_verdict, verify(domain, book, candidate, pairing));
        // The maximally interrupted fold: a checkpoint copy before every step.
        let header = header_of(candidate, pairing);
        let mut work = Box::new(ClearWorkV1::new());
        work.begin(domain, &header, true).unwrap();
        loop {
            work = Box::new((*work).clone());
            splits_exercised += 1;
            match work.status() {
                FeedStatusV1::NeedOrders { .. } => {
                    let mut done = false;
                    let mut j = 0usize;
                    while j < book.len as usize {
                        work = Box::new((*work).clone());
                        splits_exercised += 1;
                        if work.status() == FeedStatusV1::Complete {
                            done = true;
                            break;
                        }
                        work.push_order(&book.orders[j], candidate.fills[j])
                            .unwrap();
                        j += 1;
                    }
                    if !done && work.status() != FeedStatusV1::Complete {
                        work.end_pass().unwrap();
                    }
                }
                FeedStatusV1::NeedSlices => {
                    let witness = pairing.unwrap();
                    let mut k = 0usize;
                    while k < witness.len as usize {
                        work = Box::new((*work).clone());
                        splits_exercised += 1;
                        work.push_slice(&witness.slices[k]).unwrap();
                        k += 1;
                    }
                    work.end_pass().unwrap();
                }
                FeedStatusV1::Complete => break,
            }
        }
        let split_verdict = work.verdict().unwrap().copied();
        assert_eq!(split_verdict, whole_verdict, "a split changed the verdict");
        assert_eq!(*work, *whole, "a split changed the checkpoint state");
    }
    assert!(
        splits_exercised > 100,
        "the splits must be exercised: {}",
        splits_exercised
    );
    assert!(cases.len() >= 18, "the case set collapsed: {}", cases.len());
}

#[test]
fn stream_refuses_a_tampered_resumption() {
    // Refusal-on-tamper: a later pass that is not the pass-1 sequence is a
    // feed-protocol refusal, not a verdict.
    let domain = domain_with(base_policy(), 2, 2);
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let header = header_of(&candidate, None);

    // A changed fill in pass 2.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header, true).unwrap();
    for j in 0..2 {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
    }
    work.end_pass().unwrap();
    work.push_order(&book.orders[0], candidate.fills[0].wrapping_add(1))
        .unwrap();
    work.push_order(&book.orders[1], candidate.fills[1])
        .unwrap();
    assert_eq!(work.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    assert_eq!(work.verdict(), None);
    assert_eq!(
        work.push_order(&book.orders[0], 0),
        Err(FeedErrorV1::NotInProgress)
    );

    // A changed order in pass 2.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header, true).unwrap();
    for j in 0..2 {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
    }
    work.end_pass().unwrap();
    work.push_order(&single(1, 0, 0, Side::Buy, 5, SCALE), candidate.fills[0])
        .unwrap();
    work.push_order(&book.orders[1], candidate.fills[1])
        .unwrap();
    assert_eq!(work.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));

    // A short pass 2.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header, true).unwrap();
    for j in 0..2 {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
    }
    work.end_pass().unwrap();
    work.push_order(&book.orders[0], candidate.fills[0])
        .unwrap();
    assert_eq!(work.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));

    // An over-long pass 2.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header, true).unwrap();
    for j in 0..2 {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
    }
    work.end_pass().unwrap();
    for j in 0..2 {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
    }
    assert_eq!(
        work.push_order(&book.orders[0], candidate.fills[0]),
        Err(FeedErrorV1::TooManyPushes)
    );
}

#[test]
fn stream_feed_protocol_misuse_is_refused() {
    let domain = domain_with(base_policy(), 2, 2);
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let header = header_of(&candidate, None);
    let mut work = Box::new(ClearWorkV1::new());
    assert_eq!(
        work.push_order(&book.orders[0], 0),
        Err(FeedErrorV1::NotInProgress)
    );
    assert_eq!(work.end_pass(), Err(FeedErrorV1::NotInProgress));
    assert_eq!(work.verdict(), None);
    work.begin(&domain, &header, true).unwrap();
    let slice = PairingSliceV1 {
        buy_ref: LegRefV1::Order(0),
        sell_ref: LegRefV1::Order(1),
        outcome: 0,
        quantity: 1,
    };
    assert_eq!(work.push_slice(&slice), Err(FeedErrorV1::WrongPhase));
    let verdict = drive(&mut work, &domain, &book, &candidate, None);
    assert!(verdict.is_ok());
    assert_eq!(
        work.push_order(&book.orders[0], 0),
        Err(FeedErrorV1::FeedComplete)
    );
    assert_eq!(work.end_pass(), Err(FeedErrorV1::FeedComplete));
}

#[test]
fn stream_verdict_binds_the_consumed_fold() {
    // The continuation digest is stable across identical feeds and moves when
    // any consumed coordinate moves.
    let domain = domain_with(base_policy(), 2, 2);
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut a = Box::new(ClearWorkV1::new());
    let mut b = Box::new(ClearWorkV1::new());
    drive(&mut a, &domain, &book, &candidate, None).unwrap();
    drive(&mut b, &domain, &book, &candidate, None).unwrap();
    assert_eq!(a.consumed_fold(), b.consumed_fold());
    let mut changed = book;
    if let OrderV1::SingleEgg(ref mut o) = changed.orders[0] {
        o.quantity = 5;
    }
    let mut c = Box::new(ClearWorkV1::new());
    let _ = drive(&mut c, &domain, &changed, &candidate, None);
    assert_ne!(a.consumed_fold(), c.consumed_fold());
}

#[test]
fn stream_unchecked_claims_mode_matches_the_batch_entry_point() {
    // `begin(.., strict_claims: false)` mirrors
    // `verify_ignoring_claimed_aggregates`: same acceptance on a candidate
    // whose claimed aggregates are stale, same refusals everywhere else.
    let domain = domain_with(base_policy(), 2, 2);
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let mut candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    candidate.claimed_score = ScoreV1::ZERO;
    candidate.canonical_candidate_digest = 0;
    let batch =
        crate::relation_v1::verify_ignoring_claimed_aggregates(&domain, &book, &candidate, None);
    assert!(batch.is_ok(), "the fixture must accept without claims");
    assert_eq!(
        verify(&domain, &book, &candidate, None),
        Err(ErrorV1::ScoreMismatch)
    );
    let header = header_of(&candidate, None);
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header, false).unwrap();
    loop {
        match work.status() {
            FeedStatusV1::NeedOrders { .. } => {
                for j in 0..book.len as usize {
                    work.push_order(&book.orders[j], candidate.fills[j])
                        .unwrap();
                }
                work.end_pass().unwrap();
            }
            FeedStatusV1::NeedSlices => unreachable!(),
            FeedStatusV1::Complete => break,
        }
    }
    let stream = work.verdict().unwrap().copied();
    assert_eq!(stream, batch);
    // The strict mode still refuses the same stale claims.
    let mut strict = Box::new(ClearWorkV1::new());
    assert_eq!(
        drive(&mut strict, &domain, &book, &candidate, None),
        Err(ErrorV1::ScoreMismatch)
    );
}

#[test]
fn clear_work_size_is_pinned() {
    // The checkpoint object's size is a design quantity (STREAMING_RELATION
    // design §7): a silent doubling would break the on-chain account budget.
    let size = core::mem::size_of::<ClearWorkV1>();
    assert_eq!(
        size, 48_592,
        "the checkpoint layout moved; re-measure and update the design doc"
    );
}
