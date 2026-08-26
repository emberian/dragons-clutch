//! Differential and hostile tests for the Lean-owned liability-basis profile.

use dclutch_liability_basis_v2_kernel::{
    AGREEMENT_CASES_V2, Error, OperationV2, RAMP_REQUEST_BYTES_V2, REFUSAL_CASES_V2, RampRequestV2,
    TRANSITION_CASES_V2, TransitionRequestV2, categorical_payout_at, liability,
    maximum_liability_v2, peak_supply, plan_claim_transfer_v2, plan_complete_set_split,
    plan_transition_v2, validate_partition,
};

#[test]
fn handwritten_ramp_exactly_matches_the_lean_corpus() {
    for case in AGREEMENT_CASES_V2 {
        let request = RampRequestV2::decode(&case.request).expect("Lean-admitted request");
        let observed = request.evaluate().expect("profile arithmetic");
        assert_eq!(observed, case.expected);
        validate_partition(&observed, u64::from(request.scale())).expect("partition of Q");
    }
}

#[test]
fn hostile_decoder_exactly_matches_the_lean_refusal_corpus() {
    for case in REFUSAL_CASES_V2 {
        let error = RampRequestV2::decode(&case.request).expect_err("hostile request");
        assert_eq!(error.tag(), case.error_tag);
    }
}

#[test]
fn exact_width_reserved_tail_and_boundary_refusals_are_total() {
    let base = AGREEMENT_CASES_V2[2].request;
    assert_eq!(
        RampRequestV2::decode(&base[..RAMP_REQUEST_BYTES_V2 - 1]),
        Err(Error::InvalidLength)
    );
    let mut extended = [0_u8; RAMP_REQUEST_BYTES_V2 + 1];
    extended[..RAMP_REQUEST_BYTES_V2].copy_from_slice(&base);
    assert_eq!(RampRequestV2::decode(&extended), Err(Error::InvalidLength));
    for offset in 48..64 {
        let mut hostile = base;
        let byte = hostile.get_mut(offset);
        assert!(byte.is_some());
        if let Some(byte) = byte {
            *byte = 1;
        }
        assert_eq!(
            RampRequestV2::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }
}

#[test]
fn runtime_width_split_matches_the_generic_liability_identity() {
    let payouts = [2_u64, 3, 5, 7];
    let supplies = [11_u64, 13, 17, 19];
    let scale = 17;
    let before = liability(&supplies, &payouts).expect("liability");
    let plan = plan_complete_set_split(&supplies, &payouts, scale, 23, before + 9)
        .expect("solvent complete-set split");
    assert_eq!(plan.collateral_delta(), 23 * scale);
    assert_eq!(plan.liability_after(), before + 23 * scale);
    assert_eq!(plan.hoard_after(), before + 9 + 23 * scale);
    for supply in supplies {
        assert_eq!(plan.candidate_supply(supply), Ok(supply + 23));
    }
}

#[test]
fn categorical_is_runtime_width_q_one_one_hot() {
    for width in [1_usize, 2, 17, 257] {
        for winner in 0..width {
            let mut sum = 0_u64;
            for claim in 0..width {
                sum += categorical_payout_at(width, winner, claim).expect("in range");
            }
            assert_eq!(sum, 1);
        }
    }
    assert_eq!(categorical_payout_at(0, 0, 0), Err(Error::EmptyBasis));
    assert_eq!(
        categorical_payout_at(2, 2, 0),
        Err(Error::OutcomeOutOfRange)
    );
}

#[test]
fn malformed_partitions_insolvency_and_overflow_refuse() {
    assert_eq!(validate_partition(&[], 1), Err(Error::EmptyBasis));
    assert_eq!(validate_partition(&[1], 0), Err(Error::ZeroScale));
    assert_eq!(validate_partition(&[2, 2], 3), Err(Error::NonPartition));
    assert_eq!(liability(&[1], &[1, 0]), Err(Error::WidthMismatch));
    assert_eq!(
        liability(&[u64::MAX, u64::MAX], &[u64::MAX, u64::MAX]),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        plan_complete_set_split(&[2, 3], &[1, 0], 1, 1, 1),
        Err(Error::Insolvent)
    );
    assert_eq!(
        plan_complete_set_split(&[u64::MAX], &[1], 1, 1, u64::MAX),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn the_lean_transition_corpus_matches_the_handwritten_planner() {
    for case in TRANSITION_CASES_V2 {
        let supplies = case.supplies.get(..case.width).expect("corpus width");
        let payouts = case
            .payouts
            .get(..case.payout_width)
            .expect("corpus payout width");
        let operation = OperationV2::from_tag(case.operation).expect("corpus operation tag");
        let request = TransitionRequestV2 {
            supplies,
            payouts,
            scale: case.scale,
            quantity: case.quantity,
            claim_index: case.claim_index,
            hoard: case.hoard,
            operation,
        };
        match plan_transition_v2(request) {
            Ok(outcome) => {
                assert!(
                    case.accepted,
                    "the kernel admitted a Lean-refused transition"
                );
                assert_eq!(outcome.hoard_after(), case.hoard_after);
                assert_eq!(outcome.liability_before(), case.liability_before);
                assert_eq!(outcome.liability_after(), case.liability_after);
            }
            Err(error) => {
                assert!(
                    !case.accepted,
                    "the kernel refused a Lean-admitted transition"
                );
                assert_eq!(error.tag(), case.error_tag);
            }
        }
    }
}

#[test]
fn the_transition_corpus_reaches_every_named_refusal_and_operation() {
    let refusals = [
        Error::ZeroScale,
        Error::EmptyBasis,
        Error::WidthMismatch,
        Error::NonPartition,
        Error::ArithmeticOverflow,
        Error::Insolvent,
        Error::OutcomeOutOfRange,
        Error::InsufficientSupply,
    ];
    for error in refusals {
        assert!(
            TRANSITION_CASES_V2
                .iter()
                .any(|case| !case.accepted && case.error_tag == error.tag()),
            "the generated transition corpus never reaches a named refusal"
        );
    }
    for operation in [
        OperationV2::Split,
        OperationV2::Merge,
        OperationV2::TerminalRedeem,
    ] {
        assert!(
            TRANSITION_CASES_V2
                .iter()
                .any(|case| case.accepted && case.operation == operation.tag()),
            "the generated transition corpus never admits a named operation"
        );
    }
}

#[test]
fn admitted_transitions_move_liability_and_collateral_by_the_same_exact_amount() {
    for case in TRANSITION_CASES_V2 {
        if !case.accepted {
            continue;
        }
        let liability_delta = case.liability_after.abs_diff(case.liability_before);
        let hoard_delta = case.hoard_after.abs_diff(case.hoard);
        assert_eq!(liability_delta, hoard_delta);
        assert!(case.liability_after <= case.hoard_after);
        if case.operation == OperationV2::Split.tag() {
            assert_eq!(liability_delta, case.quantity * case.scale);
        }
        if case.operation == OperationV2::Merge.tag() {
            assert_eq!(liability_delta, case.quantity * case.scale);
            assert!(case.liability_after <= case.liability_before);
        }
    }
}

#[test]
fn the_agreement_corpus_covers_both_caps_and_the_strict_interior() {
    let mut lower_cap = false;
    let mut upper_cap = false;
    let mut interior = false;
    for case in AGREEMENT_CASES_V2 {
        let request = RampRequestV2::decode(&case.request).expect("Lean-admitted request");
        let scale = u64::from(request.scale());
        let primary = case.expected.first().copied().expect("primary payout");
        let complement = case.expected.get(1).copied().expect("complement payout");
        assert_eq!(primary + complement, scale);
        if primary == 0 {
            lower_cap = true;
        }
        if primary == scale {
            upper_cap = true;
        }
        if primary > 0 && primary < scale {
            interior = true;
        }
    }
    assert!(lower_cap, "no generated case reaches the zero cap");
    assert!(upper_cap, "no generated case reaches the scale cap");
    assert!(
        interior,
        "no generated case lands strictly between the caps"
    );
}

#[test]
fn the_peak_bound_is_attained_at_both_ramp_caps() {
    let scale = 10_u64;
    for (primary_supply, complement_supply) in [(7_u64, 11_u64), (11, 7), (0, 0), (5, 5)] {
        let supplies = [primary_supply, complement_supply];
        let bound = maximum_liability_v2(&supplies, scale).expect("peak bound");
        let at_lower_cap = liability(&supplies, &[0, scale]).expect("lower cap liability");
        let at_upper_cap = liability(&supplies, &[scale, 0]).expect("upper cap liability");
        assert_eq!(bound, at_lower_cap.max(at_upper_cap));
        assert!(at_lower_cap <= bound);
        assert!(at_upper_cap <= bound);
    }
    assert_eq!(peak_supply(&[]), Err(Error::EmptyBasis));
    assert_eq!(maximum_liability_v2(&[1], 0), Err(Error::ZeroScale));
    assert_eq!(
        maximum_liability_v2(&[u64::MAX], 2),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn a_backed_claim_transfer_preserves_aggregate_supply() {
    let seller = [7_u64, 11];
    let buyer = [2_u64, 3];
    let plan = plan_claim_transfer_v2(&seller, &buyer, 1, 4).expect("backed transfer");
    assert_eq!(plan.claim_index(), 1);
    assert_eq!(plan.quantity(), 4);
    assert_eq!(plan.seller_after(), 7);
    assert_eq!(plan.buyer_after(), 7);
    let payouts = [3_u64, 7];
    let aggregate_before = liability(&[seller[0] + buyer[0], seller[1] + buyer[1]], &payouts)
        .expect("aggregate before");
    let aggregate_after = liability(
        &[
            seller[0] + buyer[0],
            plan.seller_after() + plan.buyer_after(),
        ],
        &payouts,
    )
    .expect("aggregate after");
    assert_eq!(aggregate_before, aggregate_after);

    assert_eq!(
        plan_claim_transfer_v2(&seller, &buyer, 1, 12),
        Err(Error::InsufficientSupply)
    );
    assert_eq!(
        plan_claim_transfer_v2(&seller, &buyer, 2, 1),
        Err(Error::OutcomeOutOfRange)
    );
    assert_eq!(
        plan_claim_transfer_v2(&seller, &[1], 0, 1),
        Err(Error::WidthMismatch)
    );
    assert_eq!(
        plan_claim_transfer_v2(&[1], &[u64::MAX], 0, 1),
        Err(Error::ArithmeticOverflow)
    );
}
