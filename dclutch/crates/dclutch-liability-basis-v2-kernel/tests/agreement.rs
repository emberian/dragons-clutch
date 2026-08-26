//! Differential and hostile tests for the Lean-owned liability-basis profile.

use dclutch_liability_basis_v2_kernel::{
    AGREEMENT_CASES_V2, Error, RAMP_REQUEST_BYTES_V2, REFUSAL_CASES_V2, RampRequestV2,
    categorical_payout_at, liability, plan_complete_set_split, validate_partition,
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
