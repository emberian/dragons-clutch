//! Falsifiers for the coupled relation.
//!
//! Every named test in the design document's §15 list that this crate owns is
//! present under its exact name.  Names that belong to the settlement layer, the
//! kernel, or the vertical model are not stubbed here; they are reported as out
//! of this crate's scope.

use super::*;
use crate::{DustPolicy, PartialPolicy, Side};

const SCALE: u64 = PRICE_SCALE;

/// Every policy family named explicitly.  Tests that vary one family write
/// `FrozenPolicyV1 { family: ..., ..base_policy() }`, which still names every
/// remaining selector through this one construction site.
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
        remainder_seed: 7,
        policy,
    }
}

fn domain() -> RelationDomainV1 {
    domain_with(base_policy(), 2, 4)
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

fn aon(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: quantity,
        partial_policy: PartialPolicy::AllOrNone,
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

/// A two-outcome book with one strict buy and one strict sell on outcome 0,
/// bound to distinct owners.
fn crossing_book() -> BookV1 {
    book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ])
}

#[test]
fn relation_v1_simplex_sum_off_by_one_atom_refused() {
    let domain = domain();
    let book = crossing_book();
    let exact = prices(&[SCALE / 2, SCALE / 2]);
    assert!(canonical_candidate(&domain, &book, &exact, 0, 0).is_ok());

    let low = prices(&[SCALE / 2 - 1, SCALE / 2]);
    let high = prices(&[SCALE / 2 + 1, SCALE / 2]);
    assert_eq!(
        validate_prices(&domain, &low),
        Err(ErrorV1::SimplexSumMismatch)
    );
    assert_eq!(
        validate_prices(&domain, &high),
        Err(ErrorV1::SimplexSumMismatch)
    );
    let mut candidate = canonical_candidate(&domain, &book, &exact, 0, 0).unwrap();
    candidate.prices = low;
    assert_eq!(
        verify(&domain, &book, &candidate, None),
        Err(ErrorV1::SimplexSumMismatch)
    );
}

#[test]
fn relation_v1_noncanonical_inactive_price_refused() {
    let domain = domain();
    let book = crossing_book();
    let mut noncanonical = prices(&[SCALE / 2, SCALE / 2]);
    noncanonical[5] = 1;
    assert_eq!(
        validate_prices(&domain, &noncanonical),
        Err(ErrorV1::NonCanonicalPadding)
    );
    let mut candidate =
        canonical_candidate(&domain, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0).unwrap();
    candidate.prices = noncanonical;
    assert_eq!(
        verify(&domain, &book, &candidate, None),
        Err(ErrorV1::NonCanonicalPadding)
    );
    // Noncanonical padding in the fill vector is a distinct refusal.
    let mut forged =
        canonical_candidate(&domain, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0).unwrap();
    forged.fills[MAX_ORDERS - 1] = 1;
    assert_eq!(
        verify(&domain, &book, &forged, None),
        Err(ErrorV1::NonCanonicalPadding)
    );
}

#[test]
fn relation_v1_cross_outcome_pair_cannot_produce_matched_volume() {
    // The executed §P1-B counterexample: a buy bound to outcome 0 and a sell
    // bound to outcome 1.  The scalar relation matched one unit, charged a fee,
    // and then refused every settlement.  Here the conservation system has no
    // solution, so no candidate can claim the volume at all.
    let domain = domain_with(
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::FullProRata,
            ..base_policy()
        },
        2,
        4,
    );
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 1, 1, Side::Sell, 1, 0),
    ]);

    // No price vector and no imbalance admits any fee-eligible volume.
    let mut tick = 0u64;
    let mut seen_valid = 0u32;
    while tick <= SCALE {
        let vector = prices(&[tick, SCALE - tick]);
        let mut imbalance = -2i64;
        while imbalance <= 2 {
            if let Ok(candidate) = canonical_candidate(&domain, &book, &vector, imbalance, 0) {
                let summary = verify(&domain, &book, &candidate, None).unwrap();
                seen_valid += 1;
                assert_eq!(summary.buy_flow[0], 0);
                assert_eq!(summary.sell_flow[1], 0);
                assert_eq!(summary.buyer_consideration_price_units, 0);
                assert_eq!(summary.fee_price_units, 0);
                assert_eq!(summary.score.weighted_direct_volume, 0);
            }
            imbalance += 1;
        }
        tick += SCALE / 4;
    }
    assert!(
        seen_valid > 0,
        "the book must still clear as the empty candidate"
    );

    // A forged candidate that claims the cross-outcome match refuses on the
    // conservation identity, before any fee or liveness charge.
    let mut forged =
        canonical_candidate(&domain, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0).unwrap();
    forged.fills[0] = 1;
    forged.fills[1] = 1;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &forged, None),
        Err(ErrorV1::OutcomeConservationMismatch)
    );
}

#[test]
fn batch_bound_outcomes_and_owners_admit_complete_executable_pairing() {
    let domain = domain();
    // Same outcome, distinct owners: the fills admit a complete pairing.
    let book = crossing_book();
    let candidate =
        canonical_candidate(&domain, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    assert_eq!(summary.buy_flow[0], 4);
    assert_eq!(summary.sell_flow[0], 4);
    assert_eq!(candidate.fills[0], 4);
    assert_eq!(candidate.fills[1], 4);
    let witness = canonical_pairing(&domain, &book, &candidate).unwrap();
    assert_eq!(witness.len, 1);
    assert_eq!(witness.slices[0].quantity, 4);
    assert_eq!(witness.slices[0].outcome, 0);
    assert_eq!(witness.slices[0].buy_ref, LegRefV1::Order(0));
    assert_eq!(witness.slices[0].sell_ref, LegRefV1::Order(1));
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &witness),
        Ok(())
    );

    // Different outcomes: nothing crosses, and the relation says so.
    let split_book = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 1, Side::Sell, 4, 0),
    ]);
    let forged = CandidateV1 {
        order_len: 2,
        prices: prices(&[SCALE / 2, SCALE / 2]),
        virtual_split: 0,
        virtual_merge: 0,
        fills: {
            let mut fills = [0u64; MAX_ORDERS];
            fills[0] = 4;
            fills[1] = 4;
            fills
        },
        honored_aon_mask: 0,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
    };
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &split_book, &forged, None),
        Err(ErrorV1::OutcomeConservationMismatch)
    );
    assert_eq!(
        canonical_pairing(&domain, &split_book, &forged),
        Err(ErrorV1::OutcomeConservationMismatch)
    );
}

#[test]
fn relation_v1_invalid_owner_refused_before_any_charge() {
    let domain = domain_with(base_policy(), 2, 2);
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 9, 0, Side::Sell, 4, 0),
    ]);
    assert_eq!(book.validate(&domain), Err(ErrorV1::InvalidOwner));
    let candidate = CandidateV1::empty(2, prices(&[SCALE / 2, SCALE / 2]));
    assert_eq!(
        verify(&domain, &book, &candidate, None),
        Err(ErrorV1::InvalidOwner)
    );
    assert_eq!(
        propose_best_valid(
            &domain,
            &book,
            &SearchBoundsV1 {
                price_step: SCALE / 2,
                max_imbalance: 0,
                max_visits: 64,
            }
        ),
        Err(ErrorV1::InvalidOwner)
    );
}

#[test]
fn relation_v1_self_cross_only_book_refuses_per_frozen_variant() {
    // One owner standing on both sides of one outcome, and nobody else.
    let orders = [
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 0, 0, Side::Sell, 1, 0),
    ];
    let book = book_of(&orders);
    let vector = prices(&[SCALE / 2, SCALE / 2]);

    // N-a refuses at admission, before any charge.
    let refuse = domain_with(
        FrozenPolicyV1 {
            self_cross: SelfCrossPolicyV1::RefuseOverlap,
            ..base_policy()
        },
        2,
        2,
    );
    assert_eq!(book.validate(&refuse), Ok(()));
    assert_eq!(normalize(&refuse, &book), Err(ErrorV1::SelfCrossRefused));
    assert_eq!(
        canonical_candidate(&refuse, &book, &vector, 0, 0),
        Err(ErrorV1::SelfCrossRefused)
    );

    // N-b nets the overlap away and clears as the canonical empty candidate.
    let net = domain_with(
        FrozenPolicyV1 {
            self_cross: SelfCrossPolicyV1::NetAtAdmission,
            ..base_policy()
        },
        2,
        2,
    );
    let candidate = canonical_candidate(&net, &book, &vector, 0, 0).unwrap();
    let summary = verify(&net, &book, &candidate, None).unwrap();
    assert_eq!(candidate.fills[0], 0);
    assert_eq!(candidate.fills[1], 0);
    assert_eq!(summary.buy_flow[0], 0);
    assert_eq!(summary.netting_cancelled_egg[0], 1);
    assert_eq!(
        summary.score,
        ScoreV1 {
            digest: summary.candidate_digest,
            ..ScoreV1::ZERO
        }
    );

    // N-c admits the book and the V5 feasibility gate refuses the candidate.
    let allow = domain_with(base_policy(), 2, 2);
    assert_eq!(
        canonical_candidate(&allow, &book, &vector, 0, 0),
        Err(ErrorV1::PairingInfeasible {
            outcome: 0,
            owner: 0
        })
    );
}

#[test]
fn relation_v1_pairing_feasibility_inequality_is_necessary() {
    // The gate is checked directly against a forged participation table: an
    // owner whose participation exceeds the outcome's total flow can never be
    // completely paired, whatever else the candidate claims.
    let domain = domain();
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 0, 0, Side::Sell, 1, 0),
        single(3, 1, 0, Side::Sell, 1, 0),
    ]);
    let normalized = normalize(&domain, &book).unwrap();
    let mut fills = [0u64; MAX_ORDERS];
    fills[0] = 1;
    fills[1] = 1;
    let flows = FlowsV1 {
        buy: [0u64; MAX_OUTCOMES],
        sell: [0u64; MAX_OUTCOMES],
    };
    let mut table = ParticipationV1::zeroed();
    participation_from_fills(&domain, &normalized, &fills, &mut table).unwrap();
    let mut feasible = flows;
    feasible.buy[0] = 2;
    assert_eq!(
        check_pairing_feasibility(&domain, &normalized, &table, &feasible, 0),
        Ok(())
    );
    let mut infeasible = flows;
    infeasible.buy[0] = 1;
    assert_eq!(
        check_pairing_feasibility(&domain, &normalized, &table, &infeasible, 0),
        Err(ErrorV1::PairingInfeasible {
            outcome: 0,
            owner: 0
        })
    );
    // And the constructor cannot decompose what the inequality refuses.
    let forged = CandidateV1 {
        order_len: 3,
        prices: prices(&[SCALE / 2, SCALE / 2]),
        virtual_split: 0,
        virtual_merge: 0,
        fills,
        honored_aon_mask: 0,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
    };
    assert_eq!(
        canonical_pairing(&domain, &book, &forged),
        Err(ErrorV1::ConstructorStalled)
    );
}

#[test]
fn relation_v1_virtual_split_merge_imbalance_in_one_outcome_refused() {
    let domain = domain();
    // Strict buys on both outcomes, marginal sells on both: a churn candidate
    // with sigma = 1 is valid, and the same fills with the imbalance carried on
    // only one outcome are not.
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 0, 1, Side::Buy, 2, SCALE),
        single(3, 1, 0, Side::Sell, 2, SCALE / 2),
        single(4, 2, 1, Side::Sell, 2, SCALE / 2),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let churn = canonical_candidate(&domain, &book, &vector, 1, 0).unwrap();
    assert_eq!(churn.virtual_split, 1);
    assert_eq!(churn.fills[0], 2);
    assert_eq!(churn.fills[1], 2);
    assert_eq!(churn.fills[2], 1);
    assert_eq!(churn.fills[3], 1);

    let mut lopsided = churn;
    lopsided.fills[3] = 2;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &lopsided, None),
        Err(ErrorV1::OutcomeConservationMismatch)
    );

    // A candidate that claims both a split and a merge is not canonical.
    let mut both = churn;
    both.virtual_merge = 1;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &both, None),
        Err(ErrorV1::ChurnNotCanonical)
    );

    // More conversion than the book supports refuses at derivation.
    assert_eq!(
        canonical_candidate(&domain, &book, &vector, 3, 0),
        Err(ErrorV1::InfeasibleVirtualLeg)
    );
}

#[test]
fn relation_v1_churn_candidate_not_canonical_and_scores_below_churnless() {
    let domain = domain();
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 0, 1, Side::Buy, 2, SCALE),
        single(3, 1, 0, Side::Sell, 2, SCALE / 2),
        single(4, 2, 1, Side::Sell, 2, SCALE / 2),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let churnless = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let churn = canonical_candidate(&domain, &book, &vector, 1, 0).unwrap();
    let churnless_summary = verify(&domain, &book, &churnless, None).unwrap();
    let churn_summary = verify(&domain, &book, &churn, None).unwrap();
    assert_eq!(churnless_summary.direct_flow[0], 2);
    assert_eq!(churn_summary.direct_flow[0], 1);
    assert!(churnless.claimed_score.is_better_than(&churn.claimed_score));
    assert!(!churn.claimed_score.is_better_than(&churnless.claimed_score));
    assert_eq!(churn_summary.score.churn, 1);
    assert_eq!(churnless_summary.score.churn, 0);

    // The best valid submitted candidate of the bounded search is churnless.
    let best = propose_best_valid(
        &domain,
        &book,
        &SearchBoundsV1 {
            price_step: SCALE / 4,
            max_imbalance: 2,
            max_visits: 4096,
        },
    )
    .unwrap();
    assert_eq!(best.virtual_split, 0);
    assert_eq!(best.virtual_merge, 0);
}

#[test]
fn relation_v1_strict_underfill_refused_under_price_priority() {
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 10, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let price_priority = domain();
    assert_eq!(
        canonical_candidate(&price_priority, &book, &vector, 0, 0),
        Err(ErrorV1::StrictUnderfill)
    );
    let pro_rata = domain_with(
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::FullProRata,
            ..base_policy()
        },
        2,
        4,
    );
    let candidate = canonical_candidate(&pro_rata, &book, &vector, 0, 0).unwrap();
    assert_eq!(candidate.fills[0], 4);
    assert_eq!(candidate.fills[1], 4);
}

#[test]
fn relation_v1_canonical_fills_are_exact_equality_not_aggregates() {
    let domain = domain();
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 10, SCALE / 2),
        single(2, 1, 0, Side::Buy, 10, SCALE / 2),
        single(3, 2, 0, Side::Sell, 10, 0),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let canonical = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    assert_eq!(canonical.fills[0], 5);
    assert_eq!(canonical.fills[1], 5);
    assert_eq!(canonical.fills[2], 10);

    // Both side totals and every conservation identity survive this forgery;
    // only the exact canonical vector kills it.
    let mut forged = canonical;
    forged.fills[0] = 10;
    forged.fills[1] = 0;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &forged, None),
        Err(ErrorV1::CandidateMismatch)
    );
    let flows =
        flows_from_fills(&domain, &normalize(&domain, &book).unwrap(), &forged.fills).unwrap();
    assert_eq!(flows.buy[0], 10);
    assert_eq!(flows.sell[0], 10);
}

#[test]
fn batch_rejects_noncanonical_fill_reallocation() {
    // The same forgery as above, stated under the review's §6 name and extended
    // to the marginal-remainder path: moving the leftover atom is a forgery too.
    let domain = domain();
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE / 2),
        single(2, 1, 0, Side::Buy, 3, SCALE / 2),
        single(3, 2, 0, Side::Sell, 5, 0),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let canonical = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    assert_eq!(canonical.fills[0] + canonical.fills[1], 5);
    let mut forged = canonical;
    forged.fills[0] = canonical.fills[1];
    forged.fills[1] = canonical.fills[0];
    assert_ne!(forged.fills, canonical.fills);
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &forged, None),
        Err(ErrorV1::CandidateMismatch)
    );

    // The same allocation under `DustPolicy::Reject` refuses the leftover atom
    // instead of assigning it.
    let strict_dust = domain_with(
        FrozenPolicyV1 {
            dust: DustPolicy::Reject,
            ..base_policy()
        },
        2,
        3,
    );
    assert_eq!(
        canonical_candidate(&strict_dust, &book, &vector, 0, 0),
        Err(ErrorV1::DustRejected)
    );
}

/// Independently evaluate the feasibility inequality `part_i(O) <= F_i` for a
/// fill vector, without consulting the constructor.
fn feasibility_holds(domain: &RelationDomainV1, book: &BookV1, candidate: &CandidateV1) -> bool {
    let normalized = normalize(domain, book).unwrap();
    let flows = flows_from_fills(domain, &normalized, &candidate.fills).unwrap();
    let mut table = ParticipationV1::zeroed();
    participation_from_fills(domain, &normalized, &candidate.fills, &mut table).unwrap();
    check_pairing_feasibility(domain, &normalized, &table, &flows, candidate.virtual_merge).is_ok()
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

/// Build a book whose fills realize an exact `(owner, outcome, side)` flow
/// table, so the pairing constructor can be exercised over aggregates directly.
fn flow_book(buys: &[[u64; 3]; 2], sells: &[[u64; 3]; 2]) -> (BookV1, [u64; MAX_ORDERS], u8) {
    let mut orders = [empty_order_v1(); MAX_ORDERS];
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

#[test]
fn pairing_constructor_completes_iff_feasibility_inequality_holds() {
    // Bounded exhaustive oracle.  For every owner/side flow table within the
    // searched box, and every canonical `(sigma, mu)`, the constructor must
    // complete exactly when the feasibility inequality holds, and its output
    // must be a complete executable pairing whenever it completes.
    let domain = domain_with(base_policy(), 2, 3);
    let mut checked = 0u32;
    let mut completed = 0u32;
    let mut refused = 0u32;
    let mut widest_slice_count = 0u16;

    let mut code = 0u32;
    while code < 4096 {
        // Three owners, buy and sell quantities in 0..=3 on outcome 0.
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
                if sign == 1 && conversion == 0 {
                    sign += 1;
                    continue;
                }
                // Per-outcome conservation on outcome 0, and a sink leg on
                // outcome 1 that absorbs the same global conversion.
                if buy_total + merge != sell_total + split {
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
                let feasible = feasibility_holds(&domain, &book, &candidate);
                let constructed = canonical_pairing(&domain, &book, &candidate);
                checked += 1;
                match constructed {
                    Ok(witness) => {
                        completed += 1;
                        assert!(
                            feasible,
                            "constructor completed on an infeasible table: buys {:?} sells {:?} \
                             split {} merge {}",
                            buys, sells, split, merge
                        );
                        assert_eq!(
                            verify_pairing_witness(&domain, &book, &candidate, &witness),
                            Ok(()),
                            "constructor emitted a decomposition that is not a complete \
                             executable pairing: buys {:?} sells {:?} split {} merge {}",
                            buys,
                            sells,
                            split,
                            merge
                        );
                        assert_eq!(
                            canonical_pairing(&domain, &book, &candidate),
                            Ok(witness),
                            "the constructor is not deterministic"
                        );
                        if witness.len > widest_slice_count {
                            widest_slice_count = witness.len;
                        }
                    }
                    Err(error) => {
                        refused += 1;
                        assert!(
                            !feasible,
                            "constructor refused a feasible table with {:?}: buys {:?} \
                             sells {:?} split {} merge {}",
                            error, buys, sells, split, merge
                        );
                        assert_eq!(error, ErrorV1::ConstructorStalled);
                    }
                }
                sign += 1;
            }
            conversion += 1;
        }
        code += 1;
    }
    assert!(
        checked > 1000,
        "the oracle must actually search: {}",
        checked
    );
    assert!(
        completed > 0 && refused > 0,
        "both branches must be exercised"
    );
    assert!(
        widest_slice_count as usize <= MAX_SLICES,
        "slice capacity bound violated"
    );
}

#[test]
fn pairing_constructor_completes_on_two_coupled_outcomes() {
    // The same oracle with both outcomes carrying owner flow and one shared
    // global conversion, so the cross-outcome coupling is exercised too.
    let domain = domain_with(base_policy(), 2, 3);
    let mut checked = 0u32;
    let mut code = 0u32;
    while code < 6561 {
        let mut digits = [0u64; 8];
        let mut value = code;
        let mut i = 0usize;
        while i < 8 {
            digits[i] = (value % 3) as u64;
            value /= 3;
            i += 1;
        }
        let mut buys = [[0u64; 3]; 2];
        let mut sells = [[0u64; 3]; 2];
        buys[0][0] = digits[0];
        buys[0][1] = digits[1];
        sells[0][0] = digits[2];
        sells[0][1] = digits[3];
        buys[1][0] = digits[4];
        buys[1][1] = digits[5];
        sells[1][0] = digits[6];
        sells[1][1] = digits[7];
        let mut conversion = 0u64;
        while conversion <= 2 {
            let mut sign = 0usize;
            while sign < 2 {
                let split = if sign == 0 { conversion } else { 0 };
                let merge = if sign == 0 { 0 } else { conversion };
                if sign == 1 && conversion == 0 {
                    sign += 1;
                    continue;
                }
                let mut consistent = true;
                let mut outcome = 0usize;
                while outcome < 2 {
                    let buy_total: u64 = buys[outcome].iter().sum();
                    let sell_total: u64 = sells[outcome].iter().sum();
                    if buy_total + merge != sell_total + split {
                        consistent = false;
                    }
                    outcome += 1;
                }
                if consistent {
                    let (book, fills, len) = flow_book(&buys, &sells);
                    if len != 0 {
                        let candidate = candidate_for_fills(len, split, merge, fills);
                        let feasible = feasibility_holds(&domain, &book, &candidate);
                        let constructed = canonical_pairing(&domain, &book, &candidate);
                        checked += 1;
                        match constructed {
                            Ok(witness) => {
                                assert!(
                                    feasible,
                                    "completed on infeasible: {:?} {:?}",
                                    buys, sells
                                );
                                assert_eq!(
                                    verify_pairing_witness(&domain, &book, &candidate, &witness),
                                    Ok(())
                                );
                            }
                            Err(error) => {
                                assert!(
                                    !feasible,
                                    "refused a feasible coupled table with {:?}: {:?} {:?} \
                                     split {} merge {}",
                                    error, buys, sells, split, merge
                                );
                            }
                        }
                    }
                }
                sign += 1;
            }
            conversion += 1;
        }
        code += 1;
    }
    assert!(checked > 500, "the coupled oracle must actually search");
}

#[test]
fn pairing_constructor_invariant_under_shard_and_seed_permutation() {
    // Three claims, each separately checked:
    //   (a) the constructor is deterministic;
    //   (b) relabeling order identifiers so the seeded ranks keep their order
    //       leaves the decomposition byte-identical;
    //   (c) a different remainder seed may reorder tie-broken slices, but the
    //       decomposition still covers exactly the same fills.
    let domain = domain_with(base_policy(), 2, 3);
    let base_ids = [1u64, 2u64, 3u64, 4u64];
    let build = |ids: [u64; 4], seed: u64| {
        let mut local = domain;
        local.remainder_seed = seed;
        let book = book_of(&[
            single(ids[0], 0, 0, Side::Buy, 2, SCALE),
            single(ids[1], 1, 0, Side::Buy, 2, SCALE),
            single(ids[2], 2, 0, Side::Sell, 2, SCALE / 2),
            single(ids[3], 0, 0, Side::Sell, 2, SCALE / 2),
        ]);
        let candidate =
            canonical_candidate(&local, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0).unwrap();
        let witness = canonical_pairing(&local, &book, &candidate).unwrap();
        (local, book, candidate, witness)
    };

    let (domain_a, book_a, candidate_a, witness_a) = build(base_ids, 7);
    let (_, _, _, repeat) = build(base_ids, 7);
    assert_eq!(witness_a, repeat);

    // Find a rank-order preserving relabeling and require an identical output.
    let mut relabeled: Option<[u64; 4]> = None;
    let mut base = 100u64;
    while base < 400 && relabeled.is_none() {
        let ids = [base, base + 1, base + 2, base + 3];
        let mut preserved = true;
        let mut i = 0usize;
        while i < 4 {
            let mut j = 0usize;
            while j < 4 {
                let original = seeded_rank(base_ids[i], 7) < seeded_rank(base_ids[j], 7);
                let mapped = seeded_rank(ids[i], 7) < seeded_rank(ids[j], 7);
                if original != mapped {
                    preserved = false;
                }
                j += 1;
            }
            i += 1;
        }
        if preserved {
            relabeled = Some(ids);
        }
        base += 1;
    }
    let relabeled = relabeled.expect("a rank-order preserving relabeling must exist");
    let (_, _, _, witness_b) = build(relabeled, 7);
    assert_eq!(witness_a, witness_b);

    // A different seed keeps the covered fills exactly, whatever the tie order.
    let mut seed = 1u64;
    while seed < 12 {
        let (local, book, candidate, witness) = build(base_ids, seed);
        assert_eq!(
            verify_pairing_witness(&local, &book, &candidate, &witness),
            Ok(())
        );
        assert_eq!(candidate.fills, candidate_a.fills);
        seed += 1;
    }
    assert_eq!(
        verify_pairing_witness(&domain_a, &book_a, &candidate_a, &witness_a),
        Ok(())
    );
}

fn two_cycle_book() -> BookV1 {
    book_of(&[
        aon(1, 0, 0, Side::Buy, 10, SCALE / 2),
        aon(2, 1, 0, Side::Buy, 10, SCALE / 2),
        single(3, 2, 0, Side::Sell, 15, SCALE / 2),
    ])
}

fn masked_domain() -> RelationDomainV1 {
    domain_with(
        FrozenPolicyV1 {
            aon: AonPolicyV1::WitnessedHonoredMask,
            ..base_policy()
        },
        2,
        3,
    )
}

#[test]
fn aon_two_cycle_book_has_no_unique_honorable_subset() {
    // T({}) = {X, Y} and T({X, Y}) = {}: the honorable subsets are exactly
    // {}, {X}, {Y}, with no unique maximum.  A verifier that iterated its own
    // fixed point could oscillate, so the verifier only ever checks a submitted
    // mask.
    let domain = masked_domain();
    let book = two_cycle_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);

    let empty = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    assert_eq!(empty.fills[0], 0);
    assert_eq!(empty.fills[1], 0);
    assert_eq!(empty.fills[2], 0);

    let only_x = canonical_candidate(&domain, &book, &vector, 0, 0b001).unwrap();
    assert_eq!(only_x.fills[0], 10);
    assert_eq!(only_x.fills[1], 0);
    assert_eq!(only_x.fills[2], 10);

    let only_y = canonical_candidate(&domain, &book, &vector, 0, 0b010).unwrap();
    assert_eq!(only_y.fills[0], 0);
    assert_eq!(only_y.fills[1], 10);
    assert_eq!(only_y.fills[2], 10);

    assert_eq!(
        canonical_candidate(&domain, &book, &vector, 0, 0b011),
        Err(ErrorV1::AonMaskDishonored)
    );

    // The two maximal honorable subsets are score-equivalent except for the
    // digest, and the digest still makes the order total.
    assert_eq!(
        only_x.claimed_score.weighted_direct_volume,
        only_y.claimed_score.weighted_direct_volume
    );
    assert_ne!(only_x.claimed_score.digest, only_y.claimed_score.digest);
    assert_ne!(
        only_x.claimed_score.total_order(&only_y.claimed_score),
        core::cmp::Ordering::Equal
    );
    // Maximality of the mask is never verified; the accepted candidate is the
    // best valid *submitted* candidate.
    let best = propose_best_valid(
        &domain,
        &book,
        &SearchBoundsV1 {
            price_step: SCALE / 2,
            max_imbalance: 0,
            max_visits: 4096,
        },
    )
    .unwrap();
    assert!(best.honored_aon_mask == 0b001 || best.honored_aon_mask == 0b010);
}

#[test]
fn aon_witness_mask_cannot_claim_unhonorable_order() {
    let domain = masked_domain();
    let book = two_cycle_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let honored = canonical_candidate(&domain, &book, &vector, 0, 0b001).unwrap();

    // A honored order that is not filled to full size.
    let mut partial = honored;
    partial.fills[0] = 5;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &partial, None),
        Err(ErrorV1::AonMaskDishonored)
    );

    // An unhonored obligation carrying a nonzero fill.
    let mut leaked = honored;
    leaked.honored_aon_mask = 0;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &leaked, None),
        Err(ErrorV1::AonMaskLeak)
    );

    // A mask bit on an order that carries no minimum-fill obligation.
    let mut misapplied = honored;
    misapplied.honored_aon_mask = 0b101;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &misapplied, None),
        Err(ErrorV1::AonMaskNotApplicable)
    );

    // A mask bit beyond the book, and a mask under a policy that has no mask.
    let mut beyond = honored;
    beyond.honored_aon_mask = 1 << 40;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &beyond, None),
        Err(ErrorV1::AonMaskNotApplicable)
    );
    let counting = domain_with(
        FrozenPolicyV1 {
            aon: AonPolicyV1::FullSizeCounting,
            ..base_policy()
        },
        2,
        3,
    );
    assert_eq!(
        verify_ignoring_claimed_aggregates(&counting, &book, &honored, None),
        Err(ErrorV1::AonMaskNotApplicable)
    );

    // A mask bit on an ineligible order can never be honored.
    let skewed = prices(&[SCALE / 4, SCALE - SCALE / 4]);
    assert_eq!(
        canonical_candidate(&domain, &book, &skewed, 0, 0b001),
        Err(ErrorV1::AonMaskDishonored)
    );
}

#[test]
fn batch_aon_and_minimum_fill_poisoning_matches_frozen_policy() {
    // One all-or-none buy that the canonical allocator cannot make whole, one
    // ordinary buy, and one sell.  The three frozen variants answer differently,
    // and each answer is pinned.
    let orders = [
        aon(1, 0, 0, Side::Buy, 10, SCALE / 2),
        single(2, 1, 0, Side::Buy, 10, SCALE / 2),
        single(3, 2, 0, Side::Sell, 15, SCALE / 2),
    ];
    let book = book_of(&orders);
    let vector = prices(&[SCALE / 2, SCALE / 2]);

    // 2a: refused at admission, before any fee or liveness charge.
    let refuse = domain_with(
        FrozenPolicyV1 {
            aon: AonPolicyV1::RefuseAdmission,
            ..base_policy()
        },
        2,
        3,
    );
    assert_eq!(book.validate(&refuse), Err(ErrorV1::AonNotAdmitted));
    let minimum_only = book_of(&[
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: 1,
            owner: 0,
            outcome: 0,
            side: Side::Buy,
            quantity: 10,
            limit_price: SCALE,
            minimum_fill: 4,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        }),
        single(2, 1, 0, Side::Sell, 10, 0),
    ]);
    assert_eq!(
        minimum_only.validate(&refuse),
        Err(ErrorV1::MinimumFillNotAdmitted)
    );

    // 2b: poisoning is structurally impossible.  The unhonorable order simply
    // stays unhonored at zero and the rest of the book still clears.
    let masked = masked_domain();
    let unhonored = canonical_candidate(&masked, &book, &vector, 0, 0).unwrap();
    let unhonored_summary = verify(&masked, &book, &unhonored, None).unwrap();
    assert_eq!(unhonored.fills[0], 0);
    assert_eq!(unhonored_summary.buy_flow[0], 10);
    // Honoring it is also valid here and scores higher, so the search prefers it.
    let honored = canonical_candidate(&masked, &book, &vector, 0, 0b001).unwrap();
    let honored_summary = verify(&masked, &book, &honored, None).unwrap();
    assert_eq!(honored.fills[0], 10);
    assert_eq!(honored_summary.buy_flow[0], 15);
    assert!(honored
        .claimed_score
        .is_better_than(&unhonored.claimed_score));

    // 2c: full-size counting refuses the whole candidate at this price vector.
    let counting = domain_with(
        FrozenPolicyV1 {
            aon: AonPolicyV1::FullSizeCounting,
            ..base_policy()
        },
        2,
        3,
    );
    assert_eq!(
        canonical_candidate(&counting, &book, &vector, 0, 0),
        Err(ErrorV1::AllOrNoneViolation)
    );
    // A manually supplied all-or-none fill is not a licence to bypass that
    // refusal (the scalar lab's gate, lifted to the coupled relation).
    let forged = CandidateV1 {
        order_len: 3,
        prices: vector,
        virtual_split: 0,
        virtual_merge: 0,
        fills: {
            let mut fills = [0u64; MAX_ORDERS];
            fills[0] = 10;
            fills[1] = 5;
            fills[2] = 15;
            fills
        },
        honored_aon_mask: 0,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
    };
    // The canonical derivation refuses before the comparison can even happen,
    // so both construction and verification refuse for the same reason.
    assert_eq!(
        verify_ignoring_claimed_aggregates(&counting, &book, &forged, None),
        Err(ErrorV1::AllOrNoneViolation)
    );
}

#[test]
fn batch_fragmentation_and_seed_permutation_oracle() {
    // Splitting one order into k orders of the same owner must not move score
    // components 1 (weighted direct volume), 3 (limit surplus), 4 (distinct
    // owners), or 5 (churn); nor may permuting the frozen remainder seed.
    let domain = domain();
    let whole = book_of(&[
        single(1, 0, 0, Side::Buy, 10, SCALE / 2),
        single(2, 1, 0, Side::Buy, 5, SCALE / 2),
        single(3, 2, 0, Side::Sell, 10, 0),
    ]);
    let fragmented = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE / 2),
        single(2, 0, 0, Side::Buy, 6, SCALE / 2),
        single(3, 1, 0, Side::Buy, 5, SCALE / 2),
        single(4, 2, 0, Side::Sell, 10, 0),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let whole_candidate = canonical_candidate(&domain, &whole, &vector, 0, 0).unwrap();
    let fragmented_candidate = canonical_candidate(&domain, &fragmented, &vector, 0, 0).unwrap();
    let whole_summary = verify(&domain, &whole, &whole_candidate, None).unwrap();
    let fragmented_summary = verify(&domain, &fragmented, &fragmented_candidate, None).unwrap();

    assert_eq!(
        whole_summary.score.weighted_direct_volume,
        fragmented_summary.score.weighted_direct_volume
    );
    assert_eq!(
        whole_summary.score.limit_surplus_price_units,
        fragmented_summary.score.limit_surplus_price_units
    );
    assert_eq!(
        whole_summary.score.distinct_owners,
        fragmented_summary.score.distinct_owners
    );
    assert_eq!(whole_summary.score.churn, fragmented_summary.score.churn);
    assert_eq!(
        whole_summary.self_overlap_volume,
        fragmented_summary.self_overlap_volume
    );
    // The digest is the only component that may move: it binds the order set.
    assert_ne!(
        whole_summary.candidate_digest,
        fragmented_summary.candidate_digest
    );

    // Seed permutation may move which order receives a leftover atom, and may
    // never move the aggregate score components.
    let mut seed = 1u64;
    while seed < 16 {
        let mut permuted = domain;
        permuted.remainder_seed = seed;
        let candidate = canonical_candidate(&permuted, &whole, &vector, 0, 0).unwrap();
        let summary = verify(&permuted, &whole, &candidate, None).unwrap();
        assert_eq!(
            summary.score.weighted_direct_volume,
            whole_summary.score.weighted_direct_volume
        );
        assert_eq!(
            summary.score.limit_surplus_price_units,
            whole_summary.score.limit_surplus_price_units
        );
        assert_eq!(
            summary.score.distinct_owners,
            whole_summary.score.distinct_owners
        );
        assert_eq!(summary.buy_flow, whole_summary.buy_flow);
        assert_eq!(summary.sell_flow, whole_summary.sell_flow);
        seed += 1;
    }
}

fn portfolio_book() -> BookV1 {
    book_of(&[
        portfolio(1, 0, Side::Buy, &[1, 1], 3, 2),
        single(2, 1, 0, Side::Sell, 3, SCALE / 2),
        single(3, 2, 1, Side::Sell, 3, SCALE / 2),
    ])
}

#[test]
fn portfolio_lot_coupling_conserves_every_outcome_simultaneously() {
    let domain = domain_with(base_policy(), 2, 3);
    let book = portfolio_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    assert_eq!(candidate.fills[0], 3);
    assert_eq!(candidate.fills[1], 3);
    assert_eq!(candidate.fills[2], 3);
    assert_eq!(summary.buy_flow[0], 3);
    assert_eq!(summary.buy_flow[1], 3);
    assert_eq!(summary.sell_flow[0], 3);
    assert_eq!(summary.sell_flow[1], 3);
    assert_eq!(summary.buyer_consideration_price_units, 30_000);
    assert_eq!(summary.seller_credit_price_units, 30_000);

    // One lot fewer on the portfolio moves both outcomes at once, so no
    // per-outcome patch can rescue it.
    let mut forged = candidate;
    forged.fills[0] = 2;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &forged, None),
        Err(ErrorV1::OutcomeConservationMismatch)
    );
    // Filling only one of the two sells is likewise unbalanced.
    let mut half = candidate;
    half.fills[1] = 2;
    assert_eq!(
        verify_ignoring_claimed_aggregates(&domain, &book, &half, None),
        Err(ErrorV1::OutcomeConservationMismatch)
    );
    // A marginal portfolio fills zero under P-a, and the constructor says so
    // rather than rationing lots.
    let marginal = book_of(&[
        portfolio(1, 0, Side::Buy, &[1, 1], 3, 1),
        single(2, 1, 0, Side::Sell, 3, SCALE / 2),
        single(3, 2, 1, Side::Sell, 3, SCALE / 2),
    ]);
    let quiet = canonical_candidate(&domain, &marginal, &vector, 0, 0).unwrap();
    assert_eq!(quiet.fills[0], 0);
    assert_eq!(quiet.fills[1], 0);
    // P-b is named but not implemented, and says so instead of guessing.
    let lot_rationing = domain_with(
        FrozenPolicyV1 {
            portfolio_lots: PortfolioLotPolicyV1::MarginalProRataLots,
            ..base_policy()
        },
        2,
        3,
    );
    assert_eq!(
        lot_rationing.validate(),
        Err(ErrorV1::PolicyVariantUnimplemented)
    );
}

#[test]
fn portfolio_dot_product_rounds_once_at_named_boundary() {
    // Eligibility is an exact cross-multiplied integer comparison: there is no
    // division anywhere in it, so no per-leg rounding can exist.  Mutating a
    // per-leg truncation into the comparison changes the answer, which is
    // exactly what the named-boundary rule forbids.
    let domain = domain_with(base_policy(), 2, 3);
    let vector = prices(&[3333, 6667]);
    let order = portfolio(1, 0, Side::Buy, &[1, 2], 1, 1);
    assert_eq!(
        classify_order(&domain, &order, &vector),
        Ok(EligibilityV1::Ineligible)
    );
    let coefficients: [u128; 2] = [1, 2];
    let quoted: [u128; 2] = [3333, 6667];
    let exact_value: u128 = coefficients[0] * quoted[0] + coefficients[1] * quoted[1];
    assert_eq!(exact_value, 16_667);
    let truncated: u128 = coefficients[0] * quoted[0] / (SCALE as u128)
        + coefficients[1] * quoted[1] / (SCALE as u128);
    assert_eq!(truncated, 1);
    let limit_per_lot: u128 = 1;
    let limit: u128 = limit_per_lot * (SCALE as u128);
    assert!(limit < exact_value, "the exact comparison refuses");
    assert!(
        limit >= truncated * (SCALE as u128),
        "a per-leg truncation would have admitted it"
    );

    // And the one named boundary is the only place a remainder can appear: with
    // `RoundingBoundary::None` the relation is exact or it refuses.
    let exact_domain = domain_with(
        FrozenPolicyV1 {
            rounding: RoundingBoundaryV1::None,
            ..base_policy()
        },
        2,
        3,
    );
    let book = portfolio_book();
    assert_eq!(
        canonical_candidate(&exact_domain, &book, &prices(&[SCALE / 2, SCALE / 2]), 0, 0),
        Err(ErrorV1::RemainderRequired)
    );
}

#[test]
fn consideration_remainder_has_exactly_one_owner_per_frozen_variant() {
    // Three one-atom buys from one owner against one sell: the per-owner and
    // per-receipt boundaries disagree, and both conserve every remainder atom.
    let orders = [
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 0, 0, Side::Buy, 1, SCALE),
        single(3, 0, 0, Side::Buy, 1, SCALE),
        single(4, 1, 0, Side::Sell, 3, 0),
    ];
    let book = book_of(&orders);
    let vector = prices(&[SCALE / 2, SCALE / 2]);

    let owner_floor = domain_with(base_policy(), 2, 2);
    let candidate = canonical_candidate(&owner_floor, &book, &vector, 0, 0).unwrap();
    let summary = verify(&owner_floor, &book, &candidate, None).unwrap();
    assert_eq!(summary.buyer_consideration_price_units, 15_000);
    assert_eq!(summary.debit_atoms, 2);
    assert_eq!(summary.credit_atoms, 1);
    assert_eq!(summary.rounding_pot_price_units, 10_000);
    let debit_remainder =
        summary.debit_atoms * (SCALE as u128) - summary.buyer_consideration_price_units;
    let credit_remainder =
        summary.seller_credit_price_units - summary.credit_atoms * (SCALE as u128);
    assert_eq!(
        debit_remainder + credit_remainder,
        summary.rounding_pot_price_units
    );

    let receipt_floor = domain_with(
        FrozenPolicyV1 {
            rounding: RoundingBoundaryV1::ReceiptFloor,
            ..base_policy()
        },
        2,
        2,
    );
    let per_receipt = canonical_candidate(&receipt_floor, &book, &vector, 0, 0).unwrap();
    let receipt_summary = verify(&receipt_floor, &book, &per_receipt, None).unwrap();
    assert_eq!(receipt_summary.debit_atoms, 3);
    assert_eq!(receipt_summary.credit_atoms, 1);
    assert_eq!(receipt_summary.rounding_pot_price_units, 20_000);
    assert!(
        receipt_summary.rounding_pot_price_units >= summary.rounding_pot_price_units,
        "more rounding events can never mean fewer remainder atoms"
    );

    let exact = domain_with(
        FrozenPolicyV1 {
            rounding: RoundingBoundaryV1::None,
            ..base_policy()
        },
        2,
        2,
    );
    assert_eq!(
        canonical_candidate(&exact, &book, &vector, 0, 0),
        Err(ErrorV1::RemainderRequired)
    );
    // Lot-admitted quantities make the same book exact under R-a.
    let lots = book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 1, 0, Side::Sell, 2, 0),
    ]);
    let exact_candidate = canonical_candidate(&exact, &lots, &vector, 0, 0).unwrap();
    let exact_summary = verify(&exact, &lots, &exact_candidate, None).unwrap();
    assert_eq!(exact_summary.rounding_pot_price_units, 0);
    assert_eq!(exact_summary.debit_atoms, 1);
    assert_eq!(exact_summary.credit_atoms, 1);
}

#[test]
fn fee_payer_is_debited_and_fee_allocation_conserves() {
    let domain = domain_with(
        FrozenPolicyV1 {
            fee_base: FeeBaseV1::FlatNotional { bps: 100 },
            ..base_policy()
        },
        2,
        2,
    );
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    assert_eq!(summary.buyer_consideration_price_units, 20_000);
    assert_eq!(summary.fee_price_units, 200);
    assert_eq!(summary.fee_carry_bps_units, 0);
    // The seller is not the payer: its credit is the whole consideration.
    assert_eq!(summary.seller_credit_price_units, 20_000);
    // The payer's reservation funds consideration, fee, and refund exactly.
    assert_eq!(summary.opening_reserved_cash_price_units, 40_000);
    assert_eq!(summary.cash_refund_price_units, 19_800);
    assert_eq!(
        summary.opening_reserved_cash_price_units,
        summary.buyer_consideration_price_units
            + summary.fee_price_units
            + summary.cash_refund_price_units
    );
    // A payer that reserved exactly its consideration cannot fund a fee.
    let tight = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE / 2),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    assert_eq!(
        canonical_candidate(&domain, &tight, &vector, 0, 0),
        Err(ErrorV1::FeePayerUnfunded)
    );
}

#[test]
fn fee_carry_survives_order_fragmentation() {
    let domain = domain_with(
        FrozenPolicyV1 {
            fee_base: FeeBaseV1::FlatNotional { bps: 1 },
            ..base_policy()
        },
        2,
        2,
    );
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let whole = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single(2, 1, 0, Side::Sell, 3, 0),
    ]);
    let fragmented = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 0, 0, Side::Buy, 1, SCALE),
        single(3, 0, 0, Side::Buy, 1, SCALE),
        single(4, 1, 0, Side::Sell, 3, 0),
    ]);
    let whole_summary = verify(
        &domain,
        &whole,
        &canonical_candidate(&domain, &whole, &vector, 0, 0).unwrap(),
        None,
    )
    .unwrap();
    let fragmented_summary = verify(
        &domain,
        &fragmented,
        &canonical_candidate(&domain, &fragmented, &vector, 0, 0).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(whole_summary.fee_price_units, 1);
    assert_eq!(whole_summary.fee_carry_bps_units, 5_000);
    assert_eq!(
        whole_summary.fee_price_units,
        fragmented_summary.fee_price_units
    );
    assert_eq!(
        whole_summary.fee_carry_bps_units,
        fragmented_summary.fee_carry_bps_units
    );
    // A per-order floor would have collected nothing from the fragments; the
    // carry is keyed to the canonical owner, so fragmentation cannot reset it.
    let fragment_units: u128 = 5_000;
    let rate_bps: u128 = 1;
    let per_order_floor: u128 = 3 * (fragment_units * rate_bps / (FEE_BPS_DENOMINATOR as u128));
    assert_eq!(per_order_floor, 0);
    assert_ne!(fragmented_summary.fee_price_units, per_order_floor);
}

#[test]
fn score_components_are_exact_and_ordering_is_total() {
    let domain = domain();
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    // Component 1: dispersion-weighted direct volume, exact scaled integers.
    assert_eq!(
        summary.score.weighted_direct_volume,
        4 * (SCALE as i128 / 2) * (SCALE as i128 / 2)
    );
    // Component 3: exact limit surplus in price units.
    assert_eq!(summary.score.limit_surplus_price_units, 40_000);
    // Component 4: distinct owners, not orders.  Component 5: churn.
    assert_eq!(summary.score.distinct_owners, 2);
    assert_eq!(summary.score.churn, 0);
    assert_eq!(summary.self_overlap_volume, 0);

    // The order is total: every pair of distinct valid candidates compares
    // strictly, and the comparison is antisymmetric and transitive.
    let churn_book = book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 0, 1, Side::Buy, 2, SCALE),
        single(3, 1, 0, Side::Sell, 2, SCALE / 2),
        single(4, 2, 1, Side::Sell, 2, SCALE / 2),
    ]);
    let mut scores = [ScoreV1::ZERO; 3];
    scores[0] = canonical_candidate(&domain, &churn_book, &vector, 0, 0)
        .unwrap()
        .claimed_score;
    scores[1] = canonical_candidate(&domain, &churn_book, &vector, 1, 0)
        .unwrap()
        .claimed_score;
    scores[2] = canonical_candidate(&domain, &churn_book, &vector, 2, 0)
        .unwrap()
        .claimed_score;
    let mut i = 0usize;
    while i < scores.len() {
        assert_eq!(
            scores[i].total_order(&scores[i]),
            core::cmp::Ordering::Equal
        );
        let mut j = 0usize;
        while j < scores.len() {
            if i != j {
                assert_ne!(
                    scores[i].total_order(&scores[j]),
                    core::cmp::Ordering::Equal,
                    "distinct candidates must not tie"
                );
                assert_eq!(
                    scores[i].total_order(&scores[j]).reverse(),
                    scores[j].total_order(&scores[i])
                );
            }
            j += 1;
        }
        i += 1;
    }
    assert!(scores[0].is_better_than(&scores[1]));
    assert!(scores[1].is_better_than(&scores[2]));
    assert!(scores[0].is_better_than(&scores[2]));
}

#[test]
fn settlement_slice_universe_matches_frozen_variant() {
    // The batch-side half of the §13 variants: the slice universe that
    // settlement consumes is exactly the constructor's frozen decomposition,
    // it sums to the fills, and freezing it is a real choice because other
    // complete pairings of the same fills exist.
    let domain = domain_with(base_policy(), 2, 4);
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE / 2),
        single(2, 1, 0, Side::Buy, 1, SCALE / 2),
        single(3, 2, 0, Side::Sell, 1, SCALE / 2),
        single(4, 3, 0, Side::Sell, 1, SCALE / 2),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let frozen = canonical_pairing(&domain, &book, &candidate).unwrap();
    assert_eq!(frozen.len, 2);
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &frozen),
        Ok(())
    );
    let mut covered = [0u64; MAX_ORDERS];
    let mut i = 0usize;
    while i < frozen.len as usize {
        let slice = frozen.slices[i];
        assert_eq!(slice.outcome, 0);
        if let LegRefV1::Order(index) = slice.buy_ref {
            covered[index as usize] += slice.quantity;
        }
        if let LegRefV1::Order(index) = slice.sell_ref {
            covered[index as usize] += slice.quantity;
        }
        i += 1;
    }
    let mut j = 0usize;
    while j < 4 {
        assert_eq!(covered[j], candidate.fills[j]);
        j += 1;
    }

    // A different complete pairing of the same fills also verifies, which is
    // why the decomposition must be frozen rather than re-derived per receipt.
    let mut alternative = PairingWitnessV1::empty();
    alternative.slices[0] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(0),
        sell_ref: LegRefV1::Order(2),
        outcome: 0,
        quantity: 1,
    };
    alternative.slices[1] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(1),
        sell_ref: LegRefV1::Order(3),
        outcome: 0,
        quantity: 1,
    };
    alternative.len = 2;
    let mut swapped = PairingWitnessV1::empty();
    swapped.slices[0] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(0),
        sell_ref: LegRefV1::Order(3),
        outcome: 0,
        quantity: 1,
    };
    swapped.slices[1] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(1),
        sell_ref: LegRefV1::Order(2),
        outcome: 0,
        quantity: 1,
    };
    swapped.len = 2;
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &alternative),
        Ok(())
    );
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &swapped),
        Ok(())
    );
    assert_ne!(alternative, swapped);
    assert!(frozen == alternative || frozen == swapped);

    // A receipt universe that does not cover the fills is refused.
    let mut short = alternative;
    short.len = 1;
    short.slices[1] = PairingWitnessV1::empty().slices[0];
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &short),
        Err(ErrorV1::SliceSumMismatch)
    );
}

#[test]
fn settlement_strand_fixture_matches_frozen_terminal_policy() {
    // Buys {A, C} against sells {B, C}: settling A-B first would strand the
    // forbidden C-C residue.  The frozen universe never contains that move.
    let domain = domain_with(base_policy(), 2, 3);
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE / 2),
        single(2, 2, 0, Side::Buy, 1, SCALE / 2),
        single(3, 1, 0, Side::Sell, 1, SCALE / 2),
        single(4, 2, 0, Side::Sell, 1, SCALE / 2),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    // Under N-c the wash participation of owner C is neutralized in score
    // component 1 rather than counted as executed risk mass.
    assert_eq!(summary.self_overlap_volume, 1);
    assert_eq!(summary.direct_flow[0], 2);
    assert_eq!(
        summary.score.weighted_direct_volume,
        (SCALE as i128 / 2) * (SCALE as i128 / 2)
    );
    let frozen = canonical_pairing(&domain, &book, &candidate).unwrap();
    assert_eq!(frozen.len, 2);
    assert_eq!(
        frozen.slices[0],
        PairingSliceV1 {
            buy_ref: LegRefV1::Order(1),
            sell_ref: LegRefV1::Order(2),
            outcome: 0,
            quantity: 1,
        }
    );
    assert_eq!(
        frozen.slices[1],
        PairingSliceV1 {
            buy_ref: LegRefV1::Order(0),
            sell_ref: LegRefV1::Order(3),
            outcome: 0,
            quantity: 1,
        }
    );
    let mut k = 0usize;
    while k < frozen.len as usize {
        assert!(
            !(frozen.slices[k].buy_ref == LegRefV1::Order(0)
                && frozen.slices[k].sell_ref == LegRefV1::Order(2)),
            "the strand-inducing pair must never be frozen"
        );
        k += 1;
    }
    // The strand-prone decomposition cannot be submitted either: its residue is
    // one owner against itself, which is not an executable transfer.
    let mut stranded = PairingWitnessV1::empty();
    stranded.slices[0] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(0),
        sell_ref: LegRefV1::Order(2),
        outcome: 0,
        quantity: 1,
    };
    stranded.slices[1] = PairingSliceV1 {
        buy_ref: LegRefV1::Order(1),
        sell_ref: LegRefV1::Order(3),
        outcome: 0,
        quantity: 1,
    };
    stranded.len = 2;
    assert_eq!(
        verify_pairing_witness(&domain, &book, &candidate, &stranded),
        Err(ErrorV1::SliceNotExecutable)
    );
}

#[test]
fn relation_v1_explicit_slice_witness_variant_refuses_the_same_books() {
    let recomputed = domain_with(base_policy(), 2, 2);
    let explicit = domain_with(
        FrozenPolicyV1 {
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            ..base_policy()
        },
        2,
        2,
    );
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&explicit, &book, &vector, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit, &book, &candidate).unwrap();
    assert!(verify(&explicit, &book, &candidate, Some(&witness)).is_ok());
    assert_eq!(
        verify(&explicit, &book, &candidate, None),
        Err(ErrorV1::PairingWitnessMissing)
    );
    let recomputed_candidate = canonical_candidate(&recomputed, &book, &vector, 0, 0).unwrap();
    assert_eq!(
        verify(&recomputed, &book, &recomputed_candidate, Some(&witness)),
        Err(ErrorV1::PairingWitnessNotAdmitted)
    );
    // The two variants accept the same fills and refuse the same books.
    assert_eq!(recomputed_candidate.fills, candidate.fills);
    let self_cross = book_of(&[
        single(1, 0, 0, Side::Buy, 1, SCALE),
        single(2, 0, 0, Side::Sell, 1, 0),
    ]);
    assert_eq!(
        canonical_candidate(&recomputed, &self_cross, &vector, 0, 0),
        Err(ErrorV1::PairingInfeasible {
            outcome: 0,
            owner: 0
        })
    );
    assert_eq!(
        canonical_candidate(&explicit, &self_cross, &vector, 0, 0),
        Err(ErrorV1::PairingInfeasible {
            outcome: 0,
            owner: 0
        })
    );
    // Forged slices are refused term by term.
    let mut short = witness;
    short.slices[0].quantity = 3;
    assert_eq!(
        verify(&explicit, &book, &candidate, Some(&short)),
        Err(ErrorV1::SliceSumMismatch)
    );
    let mut reversed = witness;
    reversed.slices[0].buy_ref = LegRefV1::Order(1);
    assert_eq!(
        verify(&explicit, &book, &candidate, Some(&reversed)),
        Err(ErrorV1::SliceNotExecutable)
    );
    let mut padded = witness;
    padded.slices[MAX_SLICES - 1].quantity = 1;
    assert_eq!(
        verify(&explicit, &book, &candidate, Some(&padded)),
        Err(ErrorV1::NonCanonicalPadding)
    );
    // Both a split and a merge in one slice is a virtual self-pair.
    let mut virtual_self = PairingWitnessV1::empty();
    virtual_self.slices[0] = PairingSliceV1 {
        buy_ref: LegRefV1::Merge,
        sell_ref: LegRefV1::Split,
        outcome: 0,
        quantity: 1,
    };
    virtual_self.len = 1;
    assert_eq!(
        verify(&explicit, &book, &candidate, Some(&virtual_self)),
        Err(ErrorV1::SliceNotExecutable)
    );
}

#[test]
fn narrow_direct_book_is_not_policy_invariant_selection_authority() {
    // This is the exact economic shape admitted by SubmitDirectPage: two
    // distinct owners, equal 5_000 limits, equal full quantities, no minimum
    // fill, no virtual flow, and one direct pairing. Structural determinism of
    // that proposal does not make an opaque Epoch policy irrelevant.
    let book = book_of(&[
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: 1,
            owner: 0,
            outcome: 0,
            side: Side::Buy,
            quantity: 4,
            limit_price: 5_000,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        }),
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: 2,
            owner: 1,
            outcome: 0,
            side: Side::Sell,
            quantity: 4,
            limit_price: 5_000,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        }),
    ]);
    let vector = prices(&[5_000, 5_000]);

    let no_fee = domain_with(base_policy(), 2, 2);
    let candidate = canonical_candidate(&no_fee, &book, &vector, 0, 0).unwrap();
    let summary = verify(&no_fee, &book, &candidate, None).unwrap();
    assert_eq!(summary.fee_price_units, 0);
    assert_eq!(summary.debit_atoms, 2);

    // The book reserves exactly its two-atom limit consideration. A one-percent
    // frozen fee policy needs a third debit atom at the named rounding boundary,
    // so the same candidate is invalid rather than merely differently scored.
    let with_fee = domain_with(
        FrozenPolicyV1 {
            fee_base: FeeBaseV1::FlatNotional { bps: 100 },
            ..base_policy()
        },
        2,
        2,
    );
    assert_eq!(
        canonical_candidate(&with_fee, &book, &vector, 0, 0),
        Err(ErrorV1::FeePayerUnfunded)
    );

    // The witness selector is independently consensus-relevant. It changes
    // both the admitted proof shape and the relation digest even though fills
    // and prices are identical.
    let explicit = domain_with(
        FrozenPolicyV1 {
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            ..base_policy()
        },
        2,
        2,
    );
    let explicit_candidate = canonical_candidate(&explicit, &book, &vector, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit, &book, &explicit_candidate).unwrap();
    assert_eq!(candidate.fills, explicit_candidate.fills);
    assert_ne!(
        candidate.canonical_candidate_digest,
        explicit_candidate.canonical_candidate_digest
    );
    assert_eq!(
        verify(&explicit, &book, &explicit_candidate, None),
        Err(ErrorV1::PairingWitnessMissing)
    );
    assert!(verify(&explicit, &book, &explicit_candidate, Some(&witness)).is_ok());
}

#[test]
fn relation_v1_epoch_lapse_refunds_all_reservations() {
    // A book whose sides never cross clears as the canonical empty candidate,
    // and every reserved atom is refunded to the owner that reserved it.
    let domain = domain();
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 5, 3_000),
        single(2, 1, 0, Side::Sell, 7, 7_000),
    ]);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    assert_eq!(candidate.fills[0], 0);
    assert_eq!(candidate.fills[1], 0);
    assert_eq!(summary.buy_flow[0], 0);
    assert_eq!(summary.sell_flow[0], 0);
    assert_eq!(summary.opening_reserved_egg[0], 7);
    assert_eq!(summary.unfilled_refund_egg[0], 7);
    assert_eq!(summary.netting_cancelled_egg[0], 0);
    assert_eq!(summary.opening_reserved_cash_price_units, 15_000);
    assert_eq!(summary.cash_refund_price_units, 15_000);
    assert_eq!(summary.fee_price_units, 0);
    assert_eq!(summary.rounding_pot_price_units, 0);
    assert_eq!(summary.score.weighted_direct_volume, 0);
    assert_eq!(summary.score.distinct_owners, 0);

    // The bounded search agrees: the best valid submitted candidate is empty.
    let best = propose_best_valid(
        &domain,
        &book,
        &SearchBoundsV1 {
            price_step: SCALE / 4,
            max_imbalance: 1,
            max_visits: 4096,
        },
    )
    .unwrap();
    assert_eq!(best.fills, [0u64; MAX_ORDERS]);
    assert_eq!(best.virtual_split, 0);
    assert_eq!(best.virtual_merge, 0);
}

#[test]
fn relation_v1_single_atom_conservation_mutations_are_refused() {
    let domain = domain();
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    assert!(verify(&domain, &book, &candidate, None).is_ok());

    let mut mutations = [candidate; 8];
    mutations[0].fills[0] -= 1;
    mutations[1].fills[0] += 1;
    mutations[2].fills[1] -= 1;
    mutations[3].fills[1] += 1;
    mutations[4].virtual_split += 1;
    mutations[5].virtual_merge += 1;
    mutations[6].fills[0] -= 1;
    mutations[6].fills[1] -= 1;
    mutations[7].prices[0] += 1;

    let expected = [
        ErrorV1::OutcomeConservationMismatch,
        ErrorV1::FillExceedsQuantity,
        ErrorV1::OutcomeConservationMismatch,
        ErrorV1::FillExceedsQuantity,
        ErrorV1::OutcomeConservationMismatch,
        ErrorV1::OutcomeConservationMismatch,
        ErrorV1::CandidateMismatch,
        ErrorV1::SimplexSumMismatch,
    ];
    let mut i = 0usize;
    while i < mutations.len() {
        assert_eq!(
            verify_ignoring_claimed_aggregates(&domain, &book, &mutations[i], None),
            Err(expected[i]),
            "mutation {} was not refused for its own reason",
            i
        );
        // The claimed aggregates bind too: no mutation survives full verification.
        assert!(
            verify(&domain, &book, &mutations[i], None).is_err(),
            "mutation {} survived verification",
            i
        );
        i += 1;
    }
    // A stale claimed score or digest alone is refused.
    let mut stale_score = candidate;
    stale_score.claimed_score.distinct_owners += 1;
    assert_eq!(
        verify(&domain, &book, &stale_score, None),
        Err(ErrorV1::ScoreMismatch)
    );
    let mut stale_digest = candidate;
    stale_digest.canonical_candidate_digest ^= 1;
    assert_eq!(
        verify(&domain, &book, &stale_digest, None),
        Err(ErrorV1::DigestMismatch)
    );
}

#[test]
fn relation_v1_admission_refusals_precede_every_charge() {
    let domain = domain();
    let good = single(1, 0, 0, Side::Buy, 4, SCALE);

    let expired = book_of(&[
        OrderV1::SingleEgg(SingleEggOrderV1 {
            expiry_epoch: domain.epoch - 1,
            ..match good {
                OrderV1::SingleEgg(order) => order,
                OrderV1::Portfolio(_) => unreachable!(),
            }
        }),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    assert_eq!(expired.validate(&domain), Err(ErrorV1::ExpiredOrder));

    let unordered = book_of(&[
        single(2, 0, 0, Side::Buy, 4, SCALE),
        single(1, 1, 0, Side::Sell, 4, 0),
    ]);
    assert_eq!(
        unordered.validate(&domain),
        Err(ErrorV1::NonCanonicalOrderOrder)
    );

    let zero = book_of(&[single(1, 0, 0, Side::Buy, 0, SCALE)]);
    assert_eq!(zero.validate(&domain), Err(ErrorV1::InvalidQuantity));

    let over_limit = book_of(&[single(1, 0, 0, Side::Buy, 4, SCALE + 1)]);
    assert_eq!(over_limit.validate(&domain), Err(ErrorV1::PriceOutOfRange));

    let bad_outcome = book_of(&[single(1, 0, 5, Side::Buy, 4, SCALE)]);
    assert_eq!(bad_outcome.validate(&domain), Err(ErrorV1::InvalidOutcome));

    let mut padded = book_of(&[single(1, 0, 0, Side::Buy, 4, SCALE)]);
    padded.orders[7] = single(9, 0, 0, Side::Buy, 1, SCALE);
    assert_eq!(padded.validate(&domain), Err(ErrorV1::NonCanonicalPadding));

    let mut noncanonical_coefficients = portfolio(1, 0, Side::Buy, &[1, 1], 2, 2);
    if let OrderV1::Portfolio(order) = &mut noncanonical_coefficients {
        order.coefficients[9] = 1;
    }
    let portfolio_book = book_of(&[noncanonical_coefficients]);
    assert_eq!(
        portfolio_book.validate(&domain),
        Err(ErrorV1::NonCanonicalPadding)
    );

    let mut too_many = BookV1::empty();
    let mut i = 0usize;
    while i < MAX_PORTFOLIO_ORDERS + 1 {
        too_many.orders[i] = portfolio(i as u64 + 1, 0, Side::Buy, &[1, 1], 1, 2);
        i += 1;
    }
    too_many.len = (MAX_PORTFOLIO_ORDERS + 1) as u8;
    assert_eq!(too_many.validate(&domain), Err(ErrorV1::TooManyPortfolios));

    let mut wrong_version = domain;
    wrong_version.relation_version = 2;
    assert_eq!(
        wrong_version.validate(),
        Err(ErrorV1::UnknownRelationVersion)
    );
    let mut wrong_scale = domain;
    wrong_scale.price_scale = 0;
    assert_eq!(wrong_scale.validate(), Err(ErrorV1::InvalidPriceScale));
    let mut wrong_outcomes = domain;
    wrong_outcomes.outcome_count = 1;
    assert_eq!(wrong_outcomes.validate(), Err(ErrorV1::InvalidOutcome));
    let mut wrong_owners = domain;
    wrong_owners.owner_count = 0;
    assert_eq!(wrong_owners.validate(), Err(ErrorV1::InvalidOwner));
}

#[test]
fn relation_v1_golden_trace_is_stable() {
    // A pinned end-to-end trace of one frozen fixture.  Any change to the
    // policy code, the derivation, the ledger, or the digest moves it, and
    // moving it is a deliberate act.
    let domain = domain();
    let book = crossing_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &book, &candidate, None).unwrap();
    assert_eq!(candidate.fills[0], 4);
    assert_eq!(candidate.fills[1], 4);
    assert_eq!(summary.buy_flow[0], 4);
    assert_eq!(summary.sell_flow[0], 4);
    assert_eq!(summary.total_flow[0], 4);
    assert_eq!(summary.direct_flow[0], 4);
    assert_eq!(summary.buyer_consideration_price_units, 20_000);
    assert_eq!(summary.seller_credit_price_units, 20_000);
    assert_eq!(summary.opening_reserved_cash_price_units, 40_000);
    assert_eq!(summary.cash_refund_price_units, 20_000);
    assert_eq!(summary.debit_atoms, 2);
    assert_eq!(summary.credit_atoms, 2);
    assert_eq!(summary.rounding_pot_price_units, 0);
    assert_eq!(summary.score.weighted_direct_volume, 100_000_000);
    assert_eq!(summary.score.limit_surplus_price_units, 40_000);
    assert_eq!(summary.score.distinct_owners, 2);
    assert_eq!(summary.score.churn, 0);
    assert_eq!(domain.policy.code(), GOLDEN_POLICY_CODE);
    assert_eq!(summary.candidate_digest, GOLDEN_DIGEST);
    let witness = canonical_pairing(&domain, &book, &candidate).unwrap();
    assert_eq!(witness.len, 1);
    assert_eq!(witness.slices[0].quantity, 4);
}

const GOLDEN_POLICY_CODE: u64 = 2_222_914_011_136;
const GOLDEN_DIGEST: u128 = 217_262_973_876_000_398_979_303_838_267_102_196_092;

#[test]
fn relation_v1_bounded_exhaustive_books_agree_with_the_constructor() {
    // Bounded exhaustive over tiny two-outcome books and all `(p, c)` in the
    // searched box.  Every candidate the relation accepts must also admit a
    // complete executable pairing from the canonical constructor, and every
    // conservation identity must close when recomputed outside `verify`.
    let quantities = [1u64, 2u64];
    let limits = [0u64, SCALE / 2, SCALE];
    let price_ticks = [SCALE / 4, SCALE / 2, 3 * SCALE / 4];
    let mut accepted = 0u32;
    let mut refused = 0u32;

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
            let domain = domain_with(base_policy(), 2, 3);
            let mut tick = 0usize;
            while tick < price_ticks.len() {
                let vector = prices(&[price_ticks[tick], SCALE - price_ticks[tick]]);
                let mut imbalance = -1i64;
                while imbalance <= 1 {
                    match canonical_candidate(&domain, &book, &vector, imbalance, 0) {
                        Ok(candidate) => {
                            accepted += 1;
                            let summary = verify(&domain, &book, &candidate, None).unwrap();
                            let mut outcome = 0usize;
                            while outcome < 2 {
                                assert_eq!(
                                    summary.buy_flow[outcome] + summary.virtual_merge,
                                    summary.sell_flow[outcome] + summary.virtual_split,
                                    "conservation must close for {:?} at tick {}",
                                    book.orders,
                                    tick
                                );
                                assert_eq!(
                                    summary.opening_reserved_egg[outcome],
                                    summary.sell_flow[outcome]
                                        + summary.unfilled_refund_egg[outcome]
                                        + summary.netting_cancelled_egg[outcome]
                                );
                                outcome += 1;
                            }
                            assert_eq!(
                                summary.buyer_consideration_price_units
                                    + summary.merge_proceeds_price_units,
                                summary.seller_credit_price_units + summary.split_cost_price_units
                            );
                            let witness = canonical_pairing(&domain, &book, &candidate).expect(
                                "V5 accepted a candidate the canonical constructor cannot pair",
                            );
                            assert_eq!(
                                verify_pairing_witness(&domain, &book, &candidate, &witness),
                                Ok(())
                            );
                        }
                        Err(_) => refused += 1,
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
        refused > 100,
        "the oracle must refuse something: {}",
        refused
    );
}
