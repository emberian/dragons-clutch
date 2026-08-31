//! Admission for the degree-2-to-3 spline basis kind, and the seam an
//! evaluator plugs into.
//!
//! # What this module is
//!
//! [`BasisKindV3::SplineDegree2To3`] is allocated on the wire and evaluated by
//! nothing. This module is the single place that decides whether a selection
//! of that kind may be admitted, and today its answer is always no. It is
//! written as a cascade rather than a single refusal so that each conjunct is
//! individually observable: a caller that supplies a malformed degree learns
//! that, and a caller that supplies a well-formed degree with no price-gate
//! certificate learns *that*, rather than both collapsing into one opaque
//! "unsupported".
//!
//! # The order of the conjuncts, and why it is that order
//!
//! 1. the degree lies in the profile's closed interval,
//! 2. the knot vector derives the declared width,
//! 3. a price-gate certificate digest is present,
//! 4. an evaluator exists.
//!
//! Conjunct 4 fails unconditionally today. If it were checked first it would
//! mask the other three and they could rot unnoticed until the day someone
//! wrote an evaluator and found out which of them had never run. Checking it
//! last means conjuncts 1 through 3 are exercised now, on real inputs, by
//! tests that will still be meaningful after the evaluator lands.
//!
//! # Why conjunct 3 compares nothing
//!
//! The price gate exempts degree `<= 1` by proof and refuses everything above
//! it without a certificate. This kind's interval starts at 2. So a
//! certificate is required for the whole of it and no runtime degree
//! comparison is performed — `exempt_degree_below_spline_interval` and
//! `spline_degrees_require_a_certificate` in
//! `formal/dclutch-semantics/DClutchSemantics/ProductBasisV3Abi.lean` are
//! where that shortcut is discharged.
//!
//! The two shipping kinds take the converse rule: they are exempt, so a
//! certificate digest offered alongside one is a refusal rather than a
//! harmless extra. An input that is present is never silently ignored.

use crate::runtime_v3::{
    BASIS_SPLINE_MAXIMUM_DEGREE_V3, BASIS_SPLINE_MINIMUM_DEGREE_V3, BasisKindV3, Error, Result,
};

/// Whether this build carries an evaluator for the degree-2-to-3 spline
/// family.
///
/// **This is the seam.** It is the whole of what separates the tree from
/// curved payoffs on the admission side. The lane that ports an evaluator
/// flips this constant and gives [`crate::runtime_v3::ProductBasisV3`] the
/// three things the two shipping kinds already have — a decode arm, a
/// validation arm and an evaluation arm — and every refusal in this module
/// stops firing in the order the cascade names. Nothing else in the tree needs
/// to move: the kind byte, the degree interval, the width derivation and the
/// certificate conjunct are all already here and already tested.
///
/// It is a constant rather than a feature flag deliberately. A feature flag
/// would let a build exist in which the kind is admitted and no evaluator is
/// linked; a constant cannot.
pub const SPLINE_EVALUATOR_RELEASED_V3: bool = false;

/// Basis width a knot vector of this length derives at this degree.
///
/// `width = knot_count - degree - 1` is the standard B-spline count, and it is
/// the binding between the degree carried in [`BasisKindV3::SplineDegree2To3`]
/// and the knot vector carried in the record's tail. A knot vector too short
/// to derive any width refuses rather than saturating.
pub fn spline_basis_width_v3(knot_count: u32, degree: u8) -> Result<u32> {
    knot_count
        .checked_sub(u32::from(degree))
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value > 0)
        .ok_or(Error::SplineWidthDerivationMismatch)
}

/// One basis selection offered for admission, before any evaluation.
///
/// `knot_count` and `basis_width` are read from the record or the compiler
/// input; `price_gate_certificate_digest` is the 32-byte digest of the
/// `DCLTPGT1` certificate, all-zero when none was offered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisSelectionV3 {
    /// Basis family, carrying the degree when it is a spline.
    pub kind: BasisKindV3,
    /// Knot vector length declared by the record.
    pub knot_count: u32,
    /// Basis width declared by the record.
    pub basis_width: u32,
    /// Digest of the offered price-gate certificate; all-zero when absent.
    pub price_gate_certificate_digest: [u8; 32],
}

/// Admit or refuse one basis selection.
///
/// Refuses every [`BasisKindV3::SplineDegree2To3`] selection, and does so at
/// the most specific conjunct that fails. The two shipping kinds are admitted
/// exactly when they carry no price-gate certificate.
pub fn admit_basis_selection_v3(selection: BasisSelectionV3) -> Result<()> {
    let offered_certificate = selection
        .price_gate_certificate_digest
        .iter()
        .any(|byte| *byte != 0);
    match selection.kind {
        // Degree 0 and degree 1. Exempt from the price gate by proof, so a
        // certificate offered here is an input nobody asked for and nobody
        // will check.
        BasisKindV3::CategoricalQ1 | BasisKindV3::GradedExactComplement => {
            if offered_certificate {
                return Err(Error::PriceGateCertificateUnexpected);
            }
            Ok(())
        }
        BasisKindV3::SplineDegree2To3 { degree } => {
            if !(BASIS_SPLINE_MINIMUM_DEGREE_V3..=BASIS_SPLINE_MAXIMUM_DEGREE_V3).contains(&degree)
            {
                return Err(Error::SplineDegreeOutOfProfile);
            }
            if spline_basis_width_v3(selection.knot_count, degree)? != selection.basis_width {
                return Err(Error::SplineWidthDerivationMismatch);
            }
            if !offered_certificate {
                return Err(Error::PriceGateCertificateRequired);
            }
            if !SPLINE_EVALUATOR_RELEASED_V3 {
                return Err(Error::SplineEvaluatorAbsent);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: [u8; 32] = [7_u8; 32];
    const NONE: [u8; 32] = [0_u8; 32];

    fn spline(degree: u8, knot_count: u32, basis_width: u32, digest: [u8; 32]) -> BasisSelectionV3 {
        BasisSelectionV3 {
            kind: BasisKindV3::SplineDegree2To3 { degree },
            knot_count,
            basis_width,
            price_gate_certificate_digest: digest,
        }
    }

    /// Hostiles 2 and 3: degree 0, 1, 4 and 255 are not this kind's business.
    /// Degree 1 is refused here even though it is a real spline degree,
    /// because it is the graded family's degree and is reached through
    /// `BasisShapeV3` instead.
    #[test]
    fn degree_outside_the_profile_refuses() {
        for degree in [0_u8, 1, 4, 5, 255] {
            assert_eq!(
                admit_basis_selection_v3(spline(degree, 8, 5, DIGEST)),
                Err(Error::SplineDegreeOutOfProfile),
                "degree {degree}"
            );
        }
    }

    /// Hostile 14: the knot vector must derive the declared width.
    #[test]
    fn width_that_the_knot_vector_does_not_derive_refuses() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 4, DIGEST)),
            Err(Error::SplineWidthDerivationMismatch)
        );
        assert_eq!(
            admit_basis_selection_v3(spline(3, 3, 1, DIGEST)),
            Err(Error::SplineWidthDerivationMismatch),
            "a knot vector too short to derive any width"
        );
    }

    #[test]
    fn width_derivation_is_the_standard_count() {
        assert_eq!(spline_basis_width_v3(8, 2), Ok(5));
        assert_eq!(spline_basis_width_v3(8, 3), Ok(4));
        assert_eq!(
            spline_basis_width_v3(3, 3),
            Err(Error::SplineWidthDerivationMismatch)
        );
        assert_eq!(
            spline_basis_width_v3(0, 2),
            Err(Error::SplineWidthDerivationMismatch)
        );
    }

    /// **Hostile 4, the conjunct this lane exists to prove.** A well-formed
    /// degree-2 selection with no price-gate certificate is refused for that
    /// reason and no other. If the evaluator-absent guard ever moves ahead of
    /// this conjunct, this test goes red rather than silently passing on the
    /// wrong refusal.
    #[test]
    fn degree_two_without_a_certificate_refuses_for_the_certificate() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, NONE)),
            Err(Error::PriceGateCertificateRequired)
        );
        assert_eq!(
            admit_basis_selection_v3(spline(3, 8, 4, NONE)),
            Err(Error::PriceGateCertificateRequired)
        );
    }

    /// And with a certificate it is still refused — by the seam, not by the
    /// gate. This is the pair that shows the cascade is layered rather than
    /// collapsed: the same input differs in exactly one field and produces two
    /// different refusals.
    #[test]
    fn degree_two_with_a_certificate_refuses_for_the_absent_evaluator() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, DIGEST)),
            Err(Error::SplineEvaluatorAbsent)
        );
        assert!(!SPLINE_EVALUATOR_RELEASED_V3);
    }

    /// Hostile 16, the other direction: degree 0 and 1 are exempt by proof, so
    /// a certificate offered alongside one is refused rather than ignored.
    #[test]
    fn a_certificate_on_an_exempt_kind_refuses() {
        for kind in [
            BasisKindV3::CategoricalQ1,
            BasisKindV3::GradedExactComplement,
        ] {
            assert_eq!(
                admit_basis_selection_v3(BasisSelectionV3 {
                    kind,
                    knot_count: 0,
                    basis_width: 4,
                    price_gate_certificate_digest: DIGEST,
                }),
                Err(Error::PriceGateCertificateUnexpected),
                "{kind:?}"
            );
        }
    }

    /// The control: the two shipping kinds still admit. A refusal cascade that
    /// refused everything would pass every test above and be worthless.
    #[test]
    fn the_shipping_kinds_still_admit() {
        for kind in [
            BasisKindV3::CategoricalQ1,
            BasisKindV3::GradedExactComplement,
        ] {
            assert_eq!(
                admit_basis_selection_v3(BasisSelectionV3 {
                    kind,
                    knot_count: 0,
                    basis_width: 4,
                    price_gate_certificate_digest: NONE,
                }),
                Ok(()),
                "{kind:?}"
            );
        }
    }
}
