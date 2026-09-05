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
//! 3. the arithmetic envelope holds at every admissible coordinate,
//! 4. a price-gate certificate digest is present,
//! 5. an evaluator exists.
//!
//! Conjunct 5 is the seam. If it were checked first it would mask the other
//! four and they could rot unnoticed until the day someone wrote an evaluator
//! and found out which of them had never run. Checking it last means conjuncts
//! 1 through 4 are exercised on real inputs by tests that stay meaningful
//! after the evaluator lands.
//!
//! # Why conjunct 3 is here rather than at evaluation
//!
//! Because a refusal at evaluation time is a refusal after the money is in.
//! Every operation in the ported evaluator is checked, so an out-of-envelope
//! basis cannot produce a *wrong* payout — it produces
//! [`Error::ArithmeticOverflow`] at settlement, on a Market that has already
//! taken deposits, which is principal stranding by a different route. Conjunct
//! 3 moves that discovery to founding by quantifying over every coordinate
//! denominator the family accepts. See
//! [`spline_arithmetic_envelope_v3`] for the bound and for the measured
//! fixed-width bound. Degree 3 became foundable when the evaluator and this
//! envelope moved together to the same 256-bit accumulator; neither side is
//! permitted to promise a range the other cannot execute.
//!
//! # Why conjunct 4 compares nothing
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

use crate::payoff::runtime_v3::{
    BASIS_SPLINE_MAXIMUM_DEGREE_V3, BASIS_SPLINE_MINIMUM_DEGREE_V3, BasisKindV3, Error, Result,
};
use crate::payoff::spline_eval_v3::{SplineKnotsV3, spline_arithmetic_envelope_v3};

/// Whether this build carries an evaluator for the degree-2-to-3 spline
/// family.
///
/// **This is the seam.** It is the whole of what separates the tree from
/// curved payoffs on the admission side. The lane that ports an evaluator
/// flips this constant and gives [`crate::payoff::runtime_v3::ProductBasisV3`] the
/// three things the two shipping kinds already have — a decode arm, a
/// validation arm and an evaluation arm — and every refusal in this module
/// stops firing in the order the cascade names. Nothing else in the tree needs
/// to move: the kind byte, the degree interval, the width derivation and the
/// certificate conjunct are all already here and already tested.
///
/// It is a constant rather than a feature flag deliberately. A feature flag
/// would let a build exist in which the kind is admitted and no evaluator is
/// linked; a constant cannot.
///
/// # It is `true`, and here is everything that had to be true first
///
/// This flipped **last**, after every conjunct it gates was already in place
/// and already exercised:
///
/// - the de Boor port reproduces the Lean-emitted corpus exactly, and the
///   apportionment is cumulative-floor, the rule WAVE `76e2ca3f` blessed;
/// - the arithmetic envelope refuses at *founding* any basis that could
///   overflow at settlement, so an admitted basis evaluates at every
///   coordinate the family accepts;
/// - the degree, the multiplicity flag and the certificate digest are on the
///   wire in spans a deployed decoder already refuses on nonzero, and the
///   schema identity bumped to `…-graded-basis-v4` so a record finalized under
///   the old body language cannot be reinterpreted;
/// - `DCLTPGT1` is decodable from this crate, and Core verifies the hull
///   identity at founding against a certificate the *authenticated basis*
///   names -- never one the caller does;
/// - the paired settlement match and its off-chain twin moved together.
///
/// Flipping this first would have been the same edit and none of the safety.
pub const SPLINE_EVALUATOR_RELEASED_V3: bool = true;

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
///
/// `knots` and `payout_scale` are here because the arithmetic envelope
/// (conjunct 3) is a statement about the actual numbers a Market is founded
/// with, not about their declared counts. A selection that carried only the
/// counts could not tell an admissible basis from one that overflows at
/// settlement, which is the whole point of checking it at founding.
///
/// `knots` is generic over [`SplineKnotsV3`] rather than being a slice, because
/// the production caller is founding — on chain, over a borrowed account body,
/// with no allocator to collect a slice into.
pub struct BasisSelectionV3<'a, K: SplineKnotsV3 + ?Sized> {
    /// Basis family, carrying the degree when it is a spline.
    pub kind: BasisKindV3,
    /// Knot vector length declared by the record.
    pub knot_count: u32,
    /// Basis width declared by the record.
    pub basis_width: u32,
    /// Exact knot numerators the record carries, over its knot denominator.
    ///
    /// Must be exactly `knot_count` long: a declared count that disagrees with
    /// the vector it describes is refused rather than reconciled.
    pub knots: &'a K,
    /// Payout scale `Q` the basis apportions.
    pub payout_scale: u64,
    /// Digest of the offered price-gate certificate; all-zero when absent.
    pub price_gate_certificate_digest: [u8; 32],
}

impl<K: SplineKnotsV3 + ?Sized> Clone for BasisSelectionV3<'_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: SplineKnotsV3 + ?Sized> Copy for BasisSelectionV3<'_, K> {}

/// Admit or refuse one basis selection.
///
/// Refuses every [`BasisKindV3::SplineDegree2To3`] selection, and does so at
/// the most specific conjunct that fails. The two shipping kinds are admitted
/// exactly when they carry no price-gate certificate.
pub fn admit_basis_selection_v3<K: SplineKnotsV3 + ?Sized>(
    selection: BasisSelectionV3<'_, K>,
) -> Result<()> {
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
        BasisKindV3::SplineDegree2To3 { degree, .. } => {
            if !(BASIS_SPLINE_MINIMUM_DEGREE_V3..=BASIS_SPLINE_MAXIMUM_DEGREE_V3).contains(&degree)
            {
                return Err(Error::SplineDegreeOutOfProfile);
            }
            if spline_basis_width_v3(selection.knot_count, degree)? != selection.basis_width {
                return Err(Error::SplineWidthDerivationMismatch);
            }
            // The declared count and the vector it describes must be the same
            // thing before the envelope reads either.
            if selection.knots.knot_count()
                != usize::try_from(selection.knot_count).unwrap_or(usize::MAX)
            {
                return Err(Error::SplineWidthDerivationMismatch);
            }
            spline_arithmetic_envelope_v3(
                selection.knots,
                degree,
                selection.basis_width,
                selection.payout_scale,
            )?;
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

    /// Clamped uniform degree-2 knot vector: eight knots, width five.
    const KNOTS_D2: [i128; 8] = [0, 0, 0, 1, 2, 3, 3, 3];
    /// Clamped uniform degree-3 knot vector: eight knots, width four.
    const KNOTS_D3: [i128; 8] = [0, 0, 0, 0, 1, 1, 1, 1];
    /// Degree-2 knots whose three envelope factors leave the fixed 256-bit
    /// accumulator before any position can be admitted.
    const KNOTS_WIDE: [i128; 8] = [
        0,
        0,
        0,
        1_i128 << 80,
        2_i128 << 80,
        3_i128 << 80,
        3_i128 << 80,
        3_i128 << 80,
    ];
    /// Degree-2 knots whose triangle fits with the ordinary scale but whose
    /// product with `u64::MAX` does not.
    const KNOTS_MID: [i128; 8] = [
        0,
        0,
        0,
        1_i128 << 56,
        2_i128 << 56,
        3_i128 << 56,
        3_i128 << 56,
        3_i128 << 56,
    ];
    /// A payout scale a degree-2 basis apportions comfortably.
    const SCALE: u64 = 1_000_000;

    fn spline<'a>(
        degree: u8,
        knot_count: u32,
        basis_width: u32,
        knots: &'a [i128],
        payout_scale: u64,
        digest: [u8; 32],
    ) -> BasisSelectionV3<'a, [i128]> {
        BasisSelectionV3 {
            kind: BasisKindV3::SplineDegree2To3 {
                degree,
                interior_multiplicity: false,
            },
            knot_count,
            basis_width,
            knots,
            payout_scale,
            price_gate_certificate_digest: digest,
        }
    }

    /// The ordinary well-formed degree-2 selection every cascade test varies
    /// one field of.
    fn well_formed(digest: [u8; 32]) -> BasisSelectionV3<'static, [i128]> {
        spline(2, 8, 5, &KNOTS_D2, SCALE, digest)
    }

    fn exempt(kind: BasisKindV3, digest: [u8; 32]) -> BasisSelectionV3<'static, [i128]> {
        BasisSelectionV3 {
            kind,
            knot_count: 0,
            basis_width: 4,
            knots: &[],
            payout_scale: 1,
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
                admit_basis_selection_v3(spline(degree, 8, 5, &KNOTS_D2, SCALE, DIGEST)),
                Err(Error::SplineDegreeOutOfProfile),
                "degree {degree}"
            );
        }
    }

    /// Hostile 14: the knot vector must derive the declared width.
    #[test]
    fn width_that_the_knot_vector_does_not_derive_refuses() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 4, &KNOTS_D2, SCALE, DIGEST)),
            Err(Error::SplineWidthDerivationMismatch)
        );
        assert_eq!(
            admit_basis_selection_v3(spline(3, 3, 1, &KNOTS_D2[..3], SCALE, DIGEST)),
            Err(Error::SplineWidthDerivationMismatch),
            "a knot vector too short to derive any width"
        );
    }

    /// A declared `knot_count` that disagrees with the vector it describes is
    /// refused rather than reconciled — otherwise the envelope would bound a
    /// different basis than the one the record carries.
    #[test]
    fn a_knot_count_that_disagrees_with_its_vector_refuses() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &KNOTS_D2[..7], SCALE, DIGEST)),
            Err(Error::SplineWidthDerivationMismatch)
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
            admit_basis_selection_v3(well_formed(NONE)),
            Err(Error::PriceGateCertificateRequired)
        );
        assert_eq!(
            admit_basis_selection_v3(spline(3, 8, 4, &KNOTS_D3, 1, NONE)),
            Err(Error::PriceGateCertificateRequired)
        );
    }

    /// **And with a certificate it is ADMITTED.** This is the assertion the
    /// whole cut exists to make true, and it is the exact input that refused
    /// for the absent evaluator until the seam flipped: same degree, same
    /// knots, same scale, one certificate digest.
    ///
    /// The pair with the test above still shows the cascade is layered rather
    /// than collapsed -- two inputs differing in exactly one field, one
    /// admitted and one refused for that field.
    #[test]
    fn a_well_formed_degree_two_selection_with_a_certificate_is_admitted() {
        assert_eq!(admit_basis_selection_v3(well_formed(DIGEST)), Ok(()));
        const _: () = assert!(SPLINE_EVALUATOR_RELEASED_V3);
    }

    /// **The envelope conjunct, and the thing it exists to stop.** The same
    /// well-formed degree-2 basis, founded against a payout scale whose
    /// apportionment cannot fit the triangle, is refused at admission rather
    /// than at settlement. Without this the Market founds, takes deposits, and
    /// refuses `ArithmeticOverflow` when it is time to pay — fail-closed
    /// arithmetically, principal stranding operationally.
    #[test]
    fn a_basis_that_could_overflow_at_settlement_refuses_at_admission() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &KNOTS_WIDE, SCALE, DIGEST)),
            Err(Error::SplineEnvelopeExceeded)
        );
        // The same shape at a scale large enough to overflow the apportionment
        // rather than the triangle. Both products are the envelope's business,
        // because both happen after the money is in.
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &KNOTS_MID, u64::MAX, DIGEST)),
            Err(Error::SplineEnvelopeExceeded)
        );

        // Translation leaves the mathematical weights unchanged, but the live
        // evaluator still has to pre-scale signed knot numerators to locate a
        // span. A narrow vector translated near i128::MAX is therefore caught
        // here rather than at terminal evaluation.
        let base = i128::MAX / 2;
        let translated = [
            base,
            base,
            base,
            base + 1,
            base + 2,
            base + 3,
            base + 3,
            base + 3,
        ];
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &translated, SCALE, DIGEST)),
            Err(Error::SplineEnvelopeExceeded)
        );
    }

    /// The envelope is checked *before* the certificate, because a basis that
    /// cannot be evaluated is unfoundable whatever certificate accompanies it —
    /// telling the founder to go get one would be advice that cannot help.
    #[test]
    fn the_envelope_outranks_the_certificate_conjunct() {
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &KNOTS_WIDE, SCALE, NONE)),
            Err(Error::SplineEnvelopeExceeded)
        );
    }

    /// A knot vector with a degenerate span the coordinate could still land in
    /// is refused at admission, for the same reason: it divides by zero at
    /// settlement otherwise.
    #[test]
    fn a_selectable_degenerate_span_refuses_at_admission() {
        let flat: [i128; 8] = [0; 8];
        assert_eq!(
            admit_basis_selection_v3(spline(2, 8, 5, &flat, SCALE, DIGEST)),
            Err(Error::SplineDegenerateSpan)
        );
    }

    /// The former `u128` blocker is retired: this realistic cubic is admitted
    /// under the same fixed 256-bit envelope the live evaluator executes.
    #[test]
    fn degree_three_fits_the_live_fixed_width_envelope() {
        assert_eq!(
            admit_basis_selection_v3(spline(3, 8, 4, &KNOTS_D3, SCALE, DIGEST)),
            Ok(())
        );
        assert_eq!(admit_basis_selection_v3(well_formed(DIGEST)), Ok(()));
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
                admit_basis_selection_v3(exempt(kind, DIGEST)),
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
                admit_basis_selection_v3(exempt(kind, NONE)),
                Ok(()),
                "{kind:?}"
            );
        }
    }
}
