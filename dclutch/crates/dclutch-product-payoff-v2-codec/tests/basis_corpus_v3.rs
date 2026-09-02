//! Conformance of the LIVE runtime basis evaluator against its specification.
//!
//! Every expectation in this file is Lean-emitted. Nothing is read back out of
//! the evaluator and asserted against itself: `BASIS_AGREEMENT_CASES_V3` and
//! `BASIS_CATEGORICAL_CASES_V3` carry complete `DCLTPAY3` records built by
//! `DClutchSemantics.ProductBasisV3`'s own encoder, and the payouts beside them
//! were computed by that module's model of the evaluator, not by this crate.
//!
//! This is the check the on-chain evaluator did not have. A round-trip test
//! cannot supply it: when one encoder and one decoder share a moved offset,
//! they move together and agree with each other about the wrong bytes. Fixed
//! record bytes with independently derived payouts are what catches that, and
//! the corpus exists because a one-byte offset perturbation during this lane
//! turned only a single reserved-zero check red while every payout assertion
//! stayed green.

#![allow(clippy::indexing_slicing, clippy::panic)]

use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_PAYOUT_SCALE_OFFSET_V3, BASIS_WIDTH_OFFSET_V3, ProductBasisV3,
};

/// One Lean-emitted accepted runtime basis record and its exact partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisAgreementCaseV3 {
    /// Complete canonical record bytes, as the chain would see them.
    pub record: &'static [u8],
    /// Exact rational coordinate numerator.
    pub coordinate_numerator: i128,
    /// Exact rational coordinate denominator.
    pub coordinate_denominator: u64,
    /// Exact payouts at that coordinate, width-sized.
    pub expected: &'static [u64],
    /// Exact payouts for the Product's resolution-failure terminal.
    pub expected_failure: &'static [u64],
}

/// One Lean-emitted categorical record and its one-hot embedding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisCategoricalCaseV3 {
    /// Complete canonical record bytes.
    pub record: &'static [u8],
    /// Categorical selector this case evaluates.
    pub selector: u32,
    /// Exact one-hot payouts, width-sized.
    pub expected: &'static [u64],
}

#[allow(missing_docs)]
mod corpus {
    include!("generated/basis_corpus_v3.rs");
}

use corpus::{BASIS_AGREEMENT_CASES_V3, BASIS_CATEGORICAL_CASES_V3};

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

#[test]
fn live_evaluator_matches_the_lean_agreement_corpus() {
    for (index, case) in BASIS_AGREEMENT_CASES_V3.iter().enumerate() {
        let basis = ProductBasisV3::decode(case.record)
            .unwrap_or_else(|error| panic!("case {index}: Lean-admitted record refused: {error:?}"));
        let mut payouts = vec![0_u64; case.expected.len()];
        basis
            .evaluate_rational(
                case.coordinate_numerator,
                case.coordinate_denominator,
                &mut payouts,
            )
            .unwrap_or_else(|error| panic!("case {index}: evaluation refused: {error:?}"));
        assert_eq!(
            payouts, case.expected,
            "case {index}: live evaluator disagrees with its specification at {}/{}",
            case.coordinate_numerator, case.coordinate_denominator
        );
    }
    assert_eq!(
        BASIS_AGREEMENT_CASES_V3.len(),
        22,
        "a silently shrunk corpus must fail rather than vacuously pass"
    );
}

#[test]
fn live_evaluator_matches_the_lean_failure_corpus() {
    for (index, case) in BASIS_AGREEMENT_CASES_V3.iter().enumerate() {
        let basis = ProductBasisV3::decode(case.record).expect("Lean-admitted record");
        let mut payouts = vec![0_u64; case.expected_failure.len()];
        basis
            .evaluate_failure(&mut payouts)
            .unwrap_or_else(|error| panic!("case {index}: failure evaluation refused: {error:?}"));
        assert_eq!(payouts, case.expected_failure, "case {index}");
    }
}

#[test]
fn live_evaluator_matches_the_lean_categorical_corpus() {
    for (index, case) in BASIS_CATEGORICAL_CASES_V3.iter().enumerate() {
        let basis = ProductBasisV3::decode(case.record)
            .unwrap_or_else(|error| panic!("case {index}: record refused: {error:?}"));
        let mut payouts = vec![0_u64; case.expected.len()];
        basis
            .evaluate_categorical(case.selector, &mut payouts)
            .unwrap_or_else(|error| panic!("case {index}: evaluation refused: {error:?}"));
        assert_eq!(payouts, case.expected, "case {index}");
    }
    assert_eq!(BASIS_CATEGORICAL_CASES_V3.len(), 3);
}

/// The partition is asserted against the scale read out of each record's OWN
/// bytes rather than against `expected`, so this still catches a corpus that
/// agreed with an evaluator losing atoms to a remainder.
#[test]
fn every_case_partitions_the_scale_read_from_its_own_bytes() {
    for (index, case) in BASIS_AGREEMENT_CASES_V3.iter().enumerate() {
        let scale = read_u64(case.record, BASIS_PAYOUT_SCALE_OFFSET_V3);
        let total: u64 = case.expected.iter().sum();
        assert_eq!(total, scale, "case {index}: payouts do not sum to Q");
        let failure_total: u64 = case.expected_failure.iter().sum();
        assert_eq!(
            failure_total, scale,
            "case {index}: failure payouts do not sum to Q"
        );
    }
}

/// The corpus must stay inside the envelope its own records declare. This is
/// what stops a case drifting into a shape the evaluator never sees.
#[test]
fn the_corpus_stays_inside_its_declared_envelope() {
    for (index, case) in BASIS_AGREEMENT_CASES_V3.iter().enumerate() {
        let width = read_u32(case.record, BASIS_WIDTH_OFFSET_V3) as usize;
        assert_eq!(width, case.expected.len(), "case {index}: width disagrees");
        assert!(width >= 2, "case {index}: graded width must exceed one");
        assert_eq!(
            read_u32(case.record, 12) as usize,
            case.record.len(),
            "case {index}: declared record_bytes must equal the real length"
        );
    }
    for (index, case) in BASIS_CATEGORICAL_CASES_V3.iter().enumerate() {
        let width = read_u32(case.record, BASIS_WIDTH_OFFSET_V3) as usize;
        assert_eq!(width, case.expected.len(), "categorical {index}");
        assert_eq!(
            read_u64(case.record, BASIS_PAYOUT_SCALE_OFFSET_V3),
            1,
            "categorical {index}: Q must be one"
        );
    }
}

/// Coverage, asserted rather than described. A corpus of twenty-two cases that
/// all exercised the same branch would pass every test above.
#[test]
fn the_corpus_reaches_both_clamps_and_the_interior() {
    let mut clamped_low = 0;
    let mut clamped_high = 0;
    let mut interior = 0;
    for case in BASIS_AGREEMENT_CASES_V3.iter() {
        let scale = read_u64(case.record, BASIS_PAYOUT_SCALE_OFFSET_V3);
        let primary: u64 = case.expected[..case.expected.len() - 1].iter().sum();
        if primary == 0 {
            clamped_low += 1;
        } else if primary == scale {
            clamped_high += 1;
        } else {
            interior += 1;
        }
    }
    assert!(clamped_low >= 3, "corpus must reach the zero clamp");
    assert!(clamped_high >= 3, "corpus must reach the full-amplitude clamp");
    assert!(
        interior >= 6,
        "corpus must reach the interior, which is the only place rounding happens"
    );
}

/// **The founding gate runs on every specification record.**
///
/// `admit_selection_v3` is the conjunct `authenticate_product_basis_v3` calls
/// before Core commits a founding permit. Today it is a total `Ok` for both
/// shipping kinds — the wire cannot yet carry a kind that needs a certificate,
/// and the tail the digest will live in is zero-enforced. That is exactly why
/// this test matters: it says the gate is on the route and admits everything
/// the specification says is admissible, so the commit that first accepts
/// curvature changes what the gate *decides* rather than introducing the gate.
///
/// A gate whose first execution is also the first execution of the thing it
/// guards has never been observed to pass. The cohort-9 review found this
/// cascade green in tests and called by nothing; this is the other half of
/// closing that.
#[test]
fn every_specification_record_passes_the_founding_admission_gate() {
    let mut admitted = 0_usize;
    for (index, case) in BASIS_AGREEMENT_CASES_V3.iter().enumerate() {
        let basis = ProductBasisV3::decode(case.record)
            .unwrap_or_else(|error| panic!("case {index}: record refused: {error:?}"));
        basis
            .admit_selection_v3()
            .unwrap_or_else(|error| panic!("graded case {index} refused at founding: {error:?}"));
        admitted += 1;
    }
    for (index, case) in BASIS_CATEGORICAL_CASES_V3.iter().enumerate() {
        let basis = ProductBasisV3::decode(case.record)
            .unwrap_or_else(|error| panic!("categorical case {index}: record refused: {error:?}"));
        basis.admit_selection_v3().unwrap_or_else(|error| {
            panic!("categorical case {index} refused at founding: {error:?}")
        });
        admitted += 1;
    }
    assert_eq!(
        admitted,
        BASIS_AGREEMENT_CASES_V3.len() + BASIS_CATEGORICAL_CASES_V3.len(),
        "every emitted record must reach the gate"
    );
    assert!(admitted >= 25, "the corpus link has gone vacuous");
}
