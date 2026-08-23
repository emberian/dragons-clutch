//! Adversarial and property-style tests for the ScoreV2-Q kernel.

extern crate std;

use core::cmp::Ordering;

use crate::relation_v1::MAX_OUTCOMES;
use crate::score_v2::{
    certify_candidate_score_v2, derive_direct_flow_v2, score_candidate_v2,
    verify_candidate_score_v2, BestSubmittedScoreV2, CandidateDeltaV2, FlowFieldV2,
    NormalizationPolicyV2, RiskObjectiveV2, ScoreDomainV2, ScoreErrorV2, ScoreV2,
    SelectionUpdateV2,
};

const FROZEN_VECTORS: &str = include_str!("../fixtures/score_v2_q_vectors.txt");

fn active(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    assert!((2..=MAX_OUTCOMES).contains(&values.len()));
    let mut out = [0u64; MAX_OUTCOMES];
    out[..values.len()].copy_from_slice(values);
    out
}

fn candidate(
    direct: &[u64],
    virtual_split: u64,
    virtual_merge: u64,
    digest_byte: u8,
) -> CandidateDeltaV2 {
    let mut buys = [0u64; MAX_OUTCOMES];
    let mut sells = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < direct.len() {
        buys[outcome] = direct[outcome].checked_add(virtual_split).unwrap();
        sells[outcome] = direct[outcome].checked_add(virtual_merge).unwrap();
        outcome += 1;
    }
    CandidateDeltaV2 {
        normalization_policy: NormalizationPolicyV2::OwnerBlindAggregate,
        outcome_count: u8::try_from(direct.len()).unwrap(),
        aggregate_buy_flow: buys,
        aggregate_sell_flow: sells,
        claimed_direct_flow: active(direct),
        virtual_split,
        virtual_merge,
        candidate_digest: [digest_byte; 32],
    }
}

fn risk_of(direct: &[u64]) -> RiskObjectiveV2 {
    score_candidate_v2(&candidate(direct, 0, 0, 0))
        .unwrap()
        .risk
}

fn score_domain(outcomes: u8, market_byte: u8) -> ScoreDomainV2 {
    ScoreDomainV2::new(
        [market_byte; 32],
        [2u8; 32],
        [3u8; 32],
        outcomes,
    )
    .unwrap()
}

#[test]
fn frozen_score_v2_q_vectors_remain_exact() {
    let mut seen = 0usize;
    for line in FROZEN_VECTORS.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('|');
        let name = fields.next().unwrap();
        let outcomes = fields.next().unwrap().parse::<usize>().unwrap();
        let buys = parse_flow(fields.next().unwrap(), outcomes);
        let sells = parse_flow(fields.next().unwrap(), outcomes);
        let claimed = parse_flow(fields.next().unwrap(), outcomes);
        let split = fields.next().unwrap().parse::<u64>().unwrap();
        let merge = fields.next().unwrap().parse::<u64>().unwrap();
        let digest_byte = fields.next().unwrap().parse::<u8>().unwrap();
        let expected_risk = fields.next().unwrap().parse::<u64>().unwrap();
        let expected_cash = fields.next().unwrap().parse::<u64>().unwrap();
        let expected_churn = fields.next().unwrap().parse::<u64>().unwrap();
        assert!(fields.next().is_none(), "{name}: trailing frozen field");

        let delta = CandidateDeltaV2 {
            normalization_policy: NormalizationPolicyV2::OwnerBlindAggregate,
            outcome_count: u8::try_from(outcomes).unwrap(),
            aggregate_buy_flow: buys,
            aggregate_sell_flow: sells,
            claimed_direct_flow: claimed,
            virtual_split: split,
            virtual_merge: merge,
            candidate_digest: [digest_byte; 32],
        };
        let score = score_candidate_v2(&delta)
            .unwrap_or_else(|error| panic!("{name}: frozen vector refused with {error:?}"));
        assert_eq!(
            score.risk.certified_risk_flow_atoms, expected_risk,
            "{name}: risk moved"
        );
        assert_eq!(
            score.cash_equivalent_direct_flow_atoms, expected_cash,
            "{name}: cash layer moved"
        );
        assert_eq!(
            score.virtual_churn_atoms, expected_churn,
            "{name}: churn moved"
        );
        assert_eq!(score.digest, [digest_byte; 32], "{name}: digest moved");
        seen += 1;
    }
    assert_eq!(seen, 8);
}

#[test]
fn quotient_shift_complement_relabel_and_scale_properties_hold() {
    let mut state = 0xC1A0_5EED_D15C_A11Du64;
    let mut outcomes = 2usize;
    while outcomes <= 8 {
        let mut case = 0usize;
        while case < 512 {
            let mut flow = [0u64; MAX_OUTCOMES];
            let mut outcome = 0usize;
            while outcome < outcomes {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                flow[outcome] = (state >> 32) % 1_000_000;
                outcome += 1;
            }
            let base = score_candidate_v2(&candidate(&flow[..outcomes], 0, 0, 0)).unwrap();

            let shift = u64::try_from(case).unwrap();
            let mut shifted = flow;
            let mut complement = flow;
            let mut relabeled = [0u64; MAX_OUTCOMES];
            let mut scaled = flow;
            let highest = *flow[..outcomes].iter().max().unwrap();
            let mut i = 0usize;
            while i < outcomes {
                shifted[i] += shift;
                complement[i] = highest - flow[i];
                relabeled[(i + 1) % outcomes] = flow[i];
                scaled[i] *= 7;
                i += 1;
            }
            assert_eq!(risk_of(&shifted[..outcomes]), base.risk);
            assert_eq!(risk_of(&complement[..outcomes]), base.risk);
            assert_eq!(risk_of(&relabeled[..outcomes]), base.risk);
            assert_eq!(
                risk_of(&scaled[..outcomes]).certified_risk_flow_atoms,
                base.risk.certified_risk_flow_atoms * 7
            );
            case += 1;
        }
        outcomes += 1;
    }
}

#[test]
fn payoff_preserving_refinement_repeats_a_coordinate_without_reward() {
    for flow in [
        &[7u64, 7][..],
        &[0, 7][..],
        &[3, 8, 1][..],
        &[9, 9, 2, 4][..],
    ] {
        let expected = risk_of(flow);
        let mut at = 0usize;
        while at < flow.len() {
            let mut refined = [0u64; MAX_OUTCOMES];
            let mut source = 0usize;
            let mut target = 0usize;
            while source < flow.len() {
                refined[target] = flow[source];
                target += 1;
                if source == at {
                    refined[target] = flow[source];
                    target += 1;
                }
                source += 1;
            }
            assert_eq!(risk_of(&refined[..flow.len() + 1]), expected);
            at += 1;
        }
    }
}

#[test]
fn pure_complete_set_wash_scores_zero_and_loses_before_digest() {
    let empty = score_candidate_v2(&candidate(&[0, 0], 0, 0, 255)).unwrap();
    let wash = score_candidate_v2(&candidate(&[7, 7], 0, 0, 0)).unwrap();
    assert_eq!(empty.risk.certified_risk_flow_atoms, 0);
    assert_eq!(wash.risk.certified_risk_flow_atoms, 0);
    assert_eq!(wash.cash_equivalent_direct_flow_atoms, 7);
    assert!(empty.is_better_than(&wash));
}

#[test]
fn virtual_complete_set_translations_preserve_risk_and_name_churn() {
    let direct = [9, 0, 4];
    let canonical = score_candidate_v2(&candidate(&direct, 0, 0, 255)).unwrap();
    let split = score_candidate_v2(&candidate(&direct, 5, 0, 0)).unwrap();
    let merge = score_candidate_v2(&candidate(&direct, 0, 5, 0)).unwrap();
    assert_eq!(split.risk, canonical.risk);
    assert_eq!(merge.risk, canonical.risk);
    assert_eq!(split.cash_equivalent_direct_flow_atoms, 0);
    assert_eq!(merge.cash_equivalent_direct_flow_atoms, 0);
    assert_eq!(split.virtual_churn_atoms, 5);
    assert_eq!(merge.virtual_churn_atoms, 5);
    assert!(canonical.is_better_than(&split));
    assert!(canonical.is_better_than(&merge));
}

#[test]
fn comparison_order_is_total_and_each_direction_is_frozen() {
    let high_risk = score_candidate_v2(&candidate(&[8, 0], 9, 0, 255)).unwrap();
    let low_risk = score_candidate_v2(&candidate(&[7, 0], 0, 0, 0)).unwrap();
    assert!(high_risk.is_better_than(&low_risk));

    let min_zero = score_candidate_v2(&candidate(&[8, 0], 0, 0, 255)).unwrap();
    let shifted = score_candidate_v2(&candidate(&[13, 5], 0, 0, 0)).unwrap();
    assert_eq!(min_zero.risk, shifted.risk);
    assert!(min_zero.is_better_than(&shifted));

    let low_churn = score_candidate_v2(&candidate(&[3, 0], 1, 0, 255)).unwrap();
    let high_churn = score_candidate_v2(&candidate(&[3, 0], 2, 0, 0)).unwrap();
    assert!(low_churn.is_better_than(&high_churn));

    let small_digest = score_candidate_v2(&candidate(&[3, 0], 0, 0, 4)).unwrap();
    let large_digest = score_candidate_v2(&candidate(&[3, 0], 0, 0, 5)).unwrap();
    assert!(small_digest.is_better_than(&large_digest));
    assert_eq!(small_digest.total_order(&small_digest), Ordering::Equal);
    assert_eq!(
        small_digest.total_order(&large_digest),
        large_digest.total_order(&small_digest).reverse()
    );

    let scores = [
        high_risk,
        low_risk,
        min_zero,
        shifted,
        low_churn,
        high_churn,
        small_digest,
        large_digest,
    ];
    for left in scores {
        for right in scores {
            assert_eq!(left.total_order(&right), right.total_order(&left).reverse());
            for tail in scores {
                if left.total_order(&right) != Ordering::Less
                    && right.total_order(&tail) != Ordering::Less
                {
                    assert_ne!(left.total_order(&tail), Ordering::Less);
                }
            }
        }
    }
}

#[test]
fn inactive_padding_is_never_part_of_the_quotient() {
    for (field, expected) in [
        (
            FlowFieldV2::AggregateBuy,
            ScoreErrorV2::NonCanonicalPadding {
                field: FlowFieldV2::AggregateBuy,
                outcome: 2,
            },
        ),
        (
            FlowFieldV2::AggregateSell,
            ScoreErrorV2::NonCanonicalPadding {
                field: FlowFieldV2::AggregateSell,
                outcome: 2,
            },
        ),
        (
            FlowFieldV2::ClaimedDirect,
            ScoreErrorV2::NonCanonicalPadding {
                field: FlowFieldV2::ClaimedDirect,
                outcome: 2,
            },
        ),
    ] {
        let mut delta = candidate(&[7, 7], 0, 0, 0);
        match field {
            FlowFieldV2::AggregateBuy => delta.aggregate_buy_flow[2] = 1,
            FlowFieldV2::AggregateSell => delta.aggregate_sell_flow[2] = 1,
            FlowFieldV2::ClaimedDirect => delta.claimed_direct_flow[2] = 1,
        }
        assert_eq!(score_candidate_v2(&delta), Err(expected));
    }
}

#[test]
fn every_owner_tagged_normalization_policy_refuses() {
    for policy in [
        NormalizationPolicyV2::OwnerTaggedRefuseOverlap,
        NormalizationPolicyV2::OwnerTaggedNetAtAdmission,
        NormalizationPolicyV2::OwnerTaggedGateAtPairing,
    ] {
        assert!(!policy.is_representation_neutral());
        let mut delta = candidate(&[7, 0], 0, 0, 0);
        delta.normalization_policy = policy;
        assert_eq!(
            score_candidate_v2(&delta),
            Err(ScoreErrorV2::NormalizationNotRepresentationNeutral)
        );
    }
    assert!(NormalizationPolicyV2::OwnerBlindAggregate.is_representation_neutral());
}

#[test]
fn invalid_and_noncanonical_candidate_deltas_refuse_exactly() {
    let mut invalid_width = candidate(&[1, 0], 0, 0, 0);
    invalid_width.outcome_count = 1;
    assert_eq!(
        score_candidate_v2(&invalid_width),
        Err(ScoreErrorV2::InvalidOutcomeCount)
    );

    let both = CandidateDeltaV2 {
        virtual_split: 1,
        virtual_merge: 1,
        ..candidate(&[1, 0], 0, 0, 0)
    };
    assert_eq!(
        score_candidate_v2(&both),
        Err(ScoreErrorV2::NonCanonicalVirtualConversion)
    );

    let mut split_exceeds = candidate(&[0, 1], 0, 0, 0);
    split_exceeds.virtual_split = 1;
    assert_eq!(
        score_candidate_v2(&split_exceeds),
        Err(ScoreErrorV2::VirtualSplitExceedsBuyFlow { outcome: 0 })
    );

    let mut merge_exceeds = candidate(&[0, 1], 0, 0, 0);
    merge_exceeds.virtual_merge = 1;
    assert_eq!(
        score_candidate_v2(&merge_exceeds),
        Err(ScoreErrorV2::VirtualMergeExceedsSellFlow { outcome: 0 })
    );

    let mut conservation = candidate(&[1, 0], 0, 0, 0);
    conservation.aggregate_sell_flow[0] = 0;
    assert_eq!(
        score_candidate_v2(&conservation),
        Err(ScoreErrorV2::OutcomeConservationMismatch { outcome: 0 })
    );

    let mut direct = candidate(&[1, 0], 0, 0, 0);
    direct.claimed_direct_flow[0] = 0;
    assert_eq!(
        score_candidate_v2(&direct),
        Err(ScoreErrorV2::DirectFlowMismatch { outcome: 0 })
    );
}

#[test]
fn u64_boundaries_are_exact_and_impossible_aggregates_refuse_overflow() {
    let boundary = score_candidate_v2(&candidate(&[u64::MAX, 0], 0, 0, 0)).unwrap();
    assert_eq!(boundary.risk.certified_risk_flow_atoms, u64::MAX);

    let mut merge_overflow = candidate(&[0, 0], 0, 0, 0);
    merge_overflow.virtual_merge = 1;
    merge_overflow.aggregate_buy_flow[0] = u64::MAX;
    merge_overflow.aggregate_sell_flow[0] = u64::MAX;
    merge_overflow.claimed_direct_flow[0] = u64::MAX - 1;
    merge_overflow.aggregate_sell_flow[1] = 1;
    assert_eq!(
        score_candidate_v2(&merge_overflow),
        Err(ScoreErrorV2::ArithmeticOverflow { outcome: 0 })
    );

    let mut split_overflow = candidate(&[0, 0], 0, 0, 0);
    split_overflow.virtual_split = 1;
    split_overflow.aggregate_buy_flow[0] = u64::MAX;
    split_overflow.aggregate_sell_flow[0] = u64::MAX;
    split_overflow.claimed_direct_flow[0] = u64::MAX - 1;
    split_overflow.aggregate_buy_flow[1] = 1;
    assert_eq!(
        score_candidate_v2(&split_overflow),
        Err(ScoreErrorV2::ArithmeticOverflow { outcome: 0 })
    );
}

#[test]
fn claimed_scores_are_recomputed_not_trusted() {
    let delta = candidate(&[11, 2, 7], 0, 0, 3);
    let score = score_candidate_v2(&delta).unwrap();
    assert_eq!(verify_candidate_score_v2(&delta, &score), Ok(score));

    let lie = ScoreV2 {
        risk: RiskObjectiveV2 {
            certified_risk_flow_atoms: score.risk.certified_risk_flow_atoms + 1,
        },
        ..score
    };
    assert_eq!(
        verify_candidate_score_v2(&delta, &lie),
        Err(ScoreErrorV2::ScoreMismatch)
    );
}

#[test]
fn buy_and_sell_derivations_are_byte_identical() {
    let delta = candidate(&[13, 0, 8, 1], 4, 0, 7);
    assert_eq!(derive_direct_flow_v2(&delta), Ok(active(&[13, 0, 8, 1])));
}

#[test]
fn checked_certificate_binds_domain_width_flow_and_score() {
    let domain = score_domain(3, 1);
    let delta = candidate(&[9, 0, 4], 5, 0, 7);
    let certificate = certify_candidate_score_v2(domain, &delta).unwrap();

    assert_eq!(certificate.domain(), domain);
    assert_eq!(certificate.candidate_delta(), &delta);
    assert_eq!(&certificate.direct_flow()[..3], &[9, 0, 4]);
    assert_eq!(certificate.score().risk.certified_risk_flow_atoms, 9);
    assert_eq!(certificate.score().cash_equivalent_direct_flow_atoms, 0);
    assert_eq!(certificate.score().virtual_churn_atoms, 5);
    assert_eq!(certificate.score().digest, [7u8; 32]);

    assert_eq!(
        ScoreDomainV2::new([0u8; 32], [2u8; 32], [3u8; 32], 3),
        Err(ScoreErrorV2::ZeroBindingIdentity)
    );
    assert_eq!(
        certify_candidate_score_v2(score_domain(2, 1), &delta),
        Err(ScoreErrorV2::ScoreDomainWidthMismatch)
    );
}

#[test]
fn checked_selection_rejects_cross_domain_and_preserves_state_on_refusal() {
    let first = certify_candidate_score_v2(
        score_domain(2, 1),
        &candidate(&[4, 0], 0, 0, 9),
    )
    .unwrap();
    let foreign = certify_candidate_score_v2(
        score_domain(2, 8),
        &candidate(&[99, 0], 0, 0, 1),
    )
    .unwrap();
    assert_eq!(
        foreign.total_order_same_domain(&first),
        Err(ScoreErrorV2::MismatchedScoreDomain)
    );

    let mut selection = BestSubmittedScoreV2::begin(first);
    let before = selection;
    assert_eq!(
        selection.consider(foreign),
        Err(ScoreErrorV2::MismatchedScoreDomain)
    );
    assert_eq!(selection, before);
}

#[test]
fn checked_selection_penalizes_complete_set_wash_and_freezes_ties() {
    let domain = score_domain(4, 1);
    let wash = certify_candidate_score_v2(
        domain,
        &candidate(&[7, 7, 7, 7], 0, 0, 1),
    )
    .unwrap();
    let empty = certify_candidate_score_v2(
        domain,
        &candidate(&[0, 0, 0, 0], 0, 0, 255),
    )
    .unwrap();
    let risk_large_digest = certify_candidate_score_v2(
        domain,
        &candidate(&[8, 0, 3, 4], 0, 0, 9),
    )
    .unwrap();
    let risk_small_digest = certify_candidate_score_v2(
        domain,
        &candidate(&[8, 0, 3, 4], 0, 0, 2),
    )
    .unwrap();

    let mut selection = BestSubmittedScoreV2::begin(wash);
    assert_eq!(
        selection.consider(empty),
        Ok(SelectionUpdateV2::ReplacedBest)
    );
    assert_eq!(selection.best().score().digest, [255u8; 32]);
    assert_eq!(
        selection.consider(risk_large_digest),
        Ok(SelectionUpdateV2::ReplacedBest)
    );
    assert_eq!(
        selection.consider(risk_small_digest),
        Ok(SelectionUpdateV2::ReplacedBest)
    );
    assert_eq!(selection.best().score().digest, [2u8; 32]);
    assert_eq!(
        selection.consider(risk_small_digest),
        Ok(SelectionUpdateV2::RetainedBest)
    );
    assert_eq!(selection.checked_submission_count(), 5);
}

#[test]
fn checked_certificate_and_selection_support_all_sixteen_eggs() {
    let mut direct = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        direct[outcome] = u64::try_from(outcome).unwrap();
        outcome += 1;
    }
    let certificate = certify_candidate_score_v2(
        score_domain(16, 1),
        &candidate(&direct, 0, 0, 1),
    )
    .unwrap();
    assert_eq!(certificate.score().risk.certified_risk_flow_atoms, 15);

    let mut selection = BestSubmittedScoreV2::begin(certificate);
    selection.set_checked_submission_count_for_test(u64::MAX);
    let before = selection;
    assert_eq!(
        selection.consider(certificate),
        Err(ScoreErrorV2::CheckedSubmissionCountOverflow)
    );
    assert_eq!(selection, before);
}

fn parse_flow(text: &str, outcomes: usize) -> [u64; MAX_OUTCOMES] {
    let mut out = [0u64; MAX_OUTCOMES];
    let mut count = 0usize;
    for value in text.split(',') {
        assert!(count < outcomes, "too many active frozen flow cells");
        out[count] = value.parse::<u64>().unwrap();
        count += 1;
    }
    assert_eq!(count, outcomes, "wrong frozen flow width");
    out
}
