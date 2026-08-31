//! Runtime-width exact liability-basis successor.
//!
//! This module removes V2's fixed knot and term arrays. A canonical borrowed
//! record defines either the categorical `Q = 1` basis or a runtime-width set
//! of nonnegative rational graded curves followed by one exact complement.
//! Every graded term is evaluated with the parent module's exact signed-rational
//! arithmetic and sole final-floor boundary. A checked cell-by-cell envelope
//! over every Product-owned knot bounds simultaneous primary payouts by `Q`,
//! so the final claim is always `Q - sum(primary)`. Disjoint curves may each
//! use the full scale; they are not rejected by a global amplitude surrogate.
//!
//! Product and result-domain links are authenticated by the full raw-record
//! digest. They are deliberately omitted from [`semantic_basis_preimage_v3`]
//! so a Product result-domain may commit the semantic basis identity without a
//! hash cycle. The raw digest, Product identity, result-domain identity, and
//! semantic identity therefore remain four distinct joins at admission.

use core::convert::{TryFrom, TryInto};

use super::{interpolation_floor, rational_compare};
use crate::spline_admission_v3::{
    BasisSelectionV3, SPLINE_EVALUATOR_RELEASED_V3, admit_basis_selection_v3, spline_basis_width_v3,
};
use crate::spline_eval_v3::{
    SplineKnotsV3, apportion_cumulative_v3, evaluate_spline_weights_v3,
    spline_arithmetic_envelope_v3,
};

#[allow(missing_docs)]
mod generated {
    include!("generated_runtime_v3.rs");
}

pub use generated::*;

// Every offset, width and discriminant above is Lean-owned. `specialize` in
// `DClutchSemantics/ProductBasisV3Abi.lean` is the sole author of a field
// offset in this record, and `basisHeaderCoordinates` freezes all eighteen of
// them against a literal witness read off the ABI already deployed. The
// aliases below keep the decoder's established vocabulary, so moving the
// specification here costs the evaluator no change at all.
const HEADER_BYTES_OFFSET: usize = BASIS_HEADER_BYTES_OFFSET_V3;
const RECORD_BYTES_OFFSET: usize = BASIS_RECORD_BYTES_OFFSET_V3;
const KIND_OFFSET: usize = BASIS_KIND_OFFSET_V3;
const ROUNDING_OFFSET: usize = BASIS_ROUNDING_OFFSET_V3;
const KNOT_COUNT_OFFSET: usize = BASIS_KNOT_COUNT_OFFSET_V3;
const TERM_COUNT_OFFSET: usize = BASIS_TERM_COUNT_OFFSET_V3;
const PRODUCT_ID_OFFSET: usize = BASIS_PRODUCT_ID_OFFSET_V3;
const RESULT_DOMAIN_ID_OFFSET: usize = BASIS_RESULT_DOMAIN_ID_OFFSET_V3;
const COORDINATE_DOMAIN_ID_OFFSET: usize = BASIS_COORDINATE_DOMAIN_ID_OFFSET_V3;
const RESULT_UNIT_ID_OFFSET: usize = BASIS_RESULT_UNIT_ID_OFFSET_V3;
const PAYOUT_SCALE_OFFSET: usize = BASIS_PAYOUT_SCALE_OFFSET_V3;
const KNOT_DENOMINATOR_OFFSET: usize = BASIS_KNOT_DENOMINATOR_OFFSET_V3;
const EVALUATOR_RELEASE_ID_OFFSET: usize = BASIS_EVALUATOR_RELEASE_ID_OFFSET_V3;
const HEADER_TAIL_RESERVED_OFFSET: usize = BASIS_HEADER_TAIL_RESERVED_OFFSET_V3;
const PRICE_GATE_DIGEST_OFFSET: usize = BASIS_PRICE_GATE_DIGEST_OFFSET_V3;
const PRODUCT_LINK_END: usize = RESULT_DOMAIN_ID_OFFSET + 32;

const CATEGORICAL_KIND: u8 = BASIS_CATEGORICAL_KIND_V3;
const GRADED_COMPLEMENT_KIND: u8 = BASIS_GRADED_COMPLEMENT_KIND_V3;
const SPLINE_KIND: u8 = BASIS_SPLINE_DEGREE_2_TO_3_KIND_V3;

/// Refusal from hostile decoding, construction, or exact evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A record or caller buffer did not have its exact derived width.
    InvalidLength,
    /// Magic selected another record family.
    InvalidMagic,
    /// Schema or fixed-header width selected another layout.
    UnsupportedSchema,
    /// Kind or named rounding boundary was unsupported.
    UnsupportedKind,
    /// Reserved or kind-inactive fields were nonzero.
    NonCanonicalReserved,
    /// A persisted content identity was all zero.
    ZeroIdentifier,
    /// Runtime basis width, knot count, or term count was invalid.
    InvalidCount,
    /// The payout scale was zero.
    ZeroScale,
    /// An exact rational denominator was zero.
    ZeroDenominator,
    /// Active knots were not strictly increasing.
    UnorderedKnots,
    /// A term referenced another claim or malformed shape.
    InvalidTerm,
    /// Terms were not in unique canonical `(claim, shape)` order.
    NonCanonicalTermOrder,
    /// Payouts were not an exact nonnegative partition of `Q`.
    NonPartition,
    /// A caller-selected terminal coordinate did not match this basis kind.
    UnsupportedCoordinate,
    /// A categorical selector was outside runtime width.
    SelectorOutOfRange,
    /// Checked record sizing or exact arithmetic overflowed.
    ArithmeticOverflow,
    /// A degree-2-to-3 spline basis was selected and this build has no
    /// evaluator for one.
    ///
    /// The kind is allocated on the wire and refused on every route. This is
    /// the outermost conjunct of [`admit_basis_selection_v3`] and it fires
    /// last, so the refusals below stay individually observable.
    SplineEvaluatorAbsent,
    /// A spline selection named a degree outside the closed interval
    /// [`BASIS_SPLINE_MINIMUM_DEGREE_V3`]..=[`BASIS_SPLINE_MAXIMUM_DEGREE_V3`].
    SplineDegreeOutOfProfile,
    /// A spline selection's knot vector does not derive its declared width:
    /// `knot_count - degree - 1 != basis_width`.
    SplineWidthDerivationMismatch,
    /// A spline coordinate located no non-degenerate knot span, or the located
    /// span left the basis domain.
    ///
    /// Distinct from [`Error::UnorderedKnots`] on purpose. The two shipping
    /// kinds refuse any knot that fails to strictly increase; the spline family
    /// *admits* repeated interior knots — that is how a spline lowers
    /// continuity — and refuses only when no usable span survives the
    /// collapse. Reporting that as unordered knots would name a rule this
    /// family does not have.
    SplineDegenerateSpan,
    /// A spline selection carried no price-gate certificate digest.
    ///
    /// Degree 2 and 3 are both strictly above the gate's exempt degree, so a
    /// certificate is required for the whole interval with no comparison —
    /// see `exempt_degree_below_spline_interval` in
    /// `DClutchSemantics/ProductBasisV3Abi.lean`.
    PriceGateCertificateRequired,
    /// A categorical or graded selection carried a price-gate certificate
    /// digest. Degree 0 and 1 are exempt by proof; an input that is present is
    /// never silently ignored.
    PriceGateCertificateUnexpected,
    /// A spline basis could overflow the `u128` de Boor triangle at some
    /// coordinate it is required to accept, and so was refused at admission.
    ///
    /// This is the refusal that keeps an arithmetic failure from becoming a
    /// *settlement* failure. Every operation in the evaluator is checked, so no
    /// wrong number is reachable either way — but a basis that traps at
    /// settlement traps with the principal already in, and the founding-time
    /// refusal costs nobody anything. See
    /// [`spline_arithmetic_envelope_v3`](crate::spline_eval_v3::spline_arithmetic_envelope_v3).
    SplineEnvelopeExceeded,
    /// A spline record carried a repeated **interior** knot without declaring
    /// interior multiplicity in its flags byte.
    ///
    /// An interior repeat lowers continuity and changes what the Market pays,
    /// so it must be declared and carried in the record's digest rather than
    /// inferred. End clamping is structural and needs no declaration. The two
    /// shipping kinds never reach this: they keep their strictly-increasing
    /// rule and refuse any repeat with [`Error::UnorderedKnots`].
    SplineInteriorMultiplicityUndeclared,
    /// The certificate declared a profile this decoder does not implement.
    PriceGateUnsupportedProfile,
    /// The certificate's atom weights summed to a zero common mass.
    PriceGateZeroMass,
    /// The certificate's width was not above its degree, or exceeded the
    /// affine-Caratheodory capacity.
    PriceGateWidthOutOfRange,
    /// The certificate carried no hull atoms, or more than the
    /// affine-Caratheodory capacity permits.
    PriceGateCapacity,
    /// Bytes past a declared width or atom count were nonzero. One certificate
    /// has exactly one encoding, so padding is not free space.
    PriceGateNonCanonicalPadding,
    /// A hull atom carried a zero weight, which asserts nothing and would let a
    /// support be padded without changing what the certificate claims.
    PriceGateZeroAtomWeight,
    /// Hull atom coordinates were not strictly increasing. One support has one
    /// encoding, and a repeated coordinate breaks that.
    PriceGateNonCanonicalAtomOrder,
    /// The atom weights did not sum to the declared mass, so they are not a
    /// convex combination.
    PriceGateWeightMassMismatch,
    /// The weights and mass shared a common factor. Scaling both leaves the
    /// hull identity unchanged, so only the primitive representative is
    /// canonical.
    PriceGateNonPrimitiveWeightScale,
    /// The certified prices did not partition the declared scale.
    PriceGatePriceNotPartition,
    /// The certificate was issued against different founding-fixed quantities
    /// than the basis carries -- a different scale, degree or width.
    PriceGateBasisMismatch,
    /// **The hull identity failed.** `price * mass != sum(weight * payout)` at
    /// some claim, with every payout recomputed through the production
    /// evaluator rather than read from the certificate. This is the refusal
    /// that makes a forged certificate useless.
    PriceGateHullRefused,
    /// A coordinate denominator above
    /// [`SPLINE_COORDINATE_DENOMINATOR_CEILING_V3`](crate::spline_eval_v3::SPLINE_COORDINATE_DENOMINATOR_CEILING_V3).
    ///
    /// The admission envelope quantifies over every denominator up to that
    /// ceiling, so a basis that founds evaluates at all of them. This names the
    /// boundary rather than letting it arrive as a generic overflow, and it is
    /// the residue a `SignedU256` accumulation would retire.
    SplineCoordinateOutOfEnvelope,
}

/// Result alias for the runtime basis successor.
pub type Result<T> = core::result::Result<T, Error>;

/// Runtime basis evaluator family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisKindV3 {
    /// Runtime-width one-hot basis with `Q = 1`.
    CategoricalQ1,
    /// Nonnegative rational graded curves plus one exact complement.
    GradedExactComplement,
    /// Degree-2-to-3 B-spline over the record's own knot vector.
    ///
    /// **The degree is on the wire.** It is the byte at
    /// [`BASIS_SPLINE_DEGREE_OFFSET_V3`], which was zero-enforced reserved
    /// space until the commit that made this family evaluable, and the flags
    /// byte beside it at [`BASIS_SPLINE_FLAGS_OFFSET_V3`] carries the interior
    /// multiplicity permission. Both are forced zero for the two shipping
    /// kinds, so a deployed decoder confronted with a curved record refuses it
    /// rather than misreading one — which is the property the spans were
    /// reserved for, and why the schema identity bumped to `…-graded-basis-v4`
    /// in the same commit.
    ///
    /// The knot vector this degree interprets is the record's existing one —
    /// `knot_count`, `knots` and `knot_denominator`. The binding between the
    /// two is the width derivation `knot_count - degree - 1 == basis_width`.
    SplineDegree2To3 {
        /// Spline degree, required to lie in
        /// [`BASIS_SPLINE_MINIMUM_DEGREE_V3`]..=[`BASIS_SPLINE_MAXIMUM_DEGREE_V3`].
        degree: u8,
        /// Whether repeated **interior** knots are permitted.
        ///
        /// Interior multiplicity is how a spline lowers continuity: a knot of
        /// multiplicity `r` collapses `r - 1` spans and puts a corner inside an
        /// otherwise smooth basis. It is a permission the *record declares*
        /// rather than a fact an evaluator infers, so the relaxation is visible
        /// in the record's digest. End clamping is structural and never needs
        /// it.
        interior_multiplicity: bool,
    },
}

impl BasisKindV3 {
    fn tag(self) -> u8 {
        match self {
            Self::CategoricalQ1 => CATEGORICAL_KIND,
            Self::GradedExactComplement => GRADED_COMPLEMENT_KIND,
            Self::SplineDegree2To3 { .. } => SPLINE_KIND,
        }
    }

    /// Decode the family from the header, reading the degree and flags bytes
    /// only for the kind that owns them.
    ///
    /// The two shipping kinds do not read bytes 18 and 19 at all; the decoder
    /// requires them zero for those kinds separately, which is the same
    /// kind-inactive-fields-forced-canonical discipline `payout_scale == 1`
    /// already takes for a categorical record.
    fn decode_header(bytes: &[u8]) -> Result<Self> {
        match read_byte(bytes, KIND_OFFSET)? {
            CATEGORICAL_KIND => Ok(Self::CategoricalQ1),
            GRADED_COMPLEMENT_KIND => Ok(Self::GradedExactComplement),
            SPLINE_KIND => {
                if !SPLINE_EVALUATOR_RELEASED_V3 {
                    return Err(Error::SplineEvaluatorAbsent);
                }
                let degree = read_byte(bytes, BASIS_SPLINE_DEGREE_OFFSET_V3)?;
                if !(BASIS_SPLINE_MINIMUM_DEGREE_V3..=BASIS_SPLINE_MAXIMUM_DEGREE_V3)
                    .contains(&degree)
                {
                    return Err(Error::SplineDegreeOutOfProfile);
                }
                let flags = read_byte(bytes, BASIS_SPLINE_FLAGS_OFFSET_V3)?;
                // Bits 1..7 are unallocated and required zero, so the byte keeps
                // refusing what it does not understand.
                if flags & !BASIS_SPLINE_INTERIOR_MULTIPLICITY_FLAG_V3 != 0 {
                    return Err(Error::NonCanonicalReserved);
                }
                Ok(Self::SplineDegree2To3 {
                    degree,
                    interior_multiplicity: flags & BASIS_SPLINE_INTERIOR_MULTIPLICITY_FLAG_V3 != 0,
                })
            }
            _ => Err(Error::UnsupportedKind),
        }
    }

    /// The flags byte this family writes at [`BASIS_SPLINE_FLAGS_OFFSET_V3`].
    fn flags(self) -> u8 {
        match self {
            Self::SplineDegree2To3 {
                interior_multiplicity: true,
                ..
            } => BASIS_SPLINE_INTERIOR_MULTIPLICITY_FLAG_V3,
            _ => 0,
        }
    }

    /// The degree byte this family writes at [`BASIS_SPLINE_DEGREE_OFFSET_V3`].
    fn degree_byte(self) -> u8 {
        match self {
            Self::SplineDegree2To3 { degree, .. } => degree,
            _ => 0,
        }
    }
}

/// One exact runtime graded shape with `u32` Product-owned knot indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisShapeV3 {
    /// Constant payout over the full signed-rational line.
    Constant,
    /// Increasing ramp with exact clamped tails.
    RampUp {
        /// Left Product-owned knot index.
        left: u32,
        /// Right Product-owned knot index.
        right: u32,
    },
    /// Decreasing ramp with exact clamped tails.
    RampDown {
        /// Left Product-owned knot index.
        left: u32,
        /// Right Product-owned knot index.
        right: u32,
    },
    /// Tent with zero outer tails and one exact peak.
    Tent {
        /// Left Product-owned knot index.
        left: u32,
        /// Peak Product-owned knot index.
        peak: u32,
        /// Right Product-owned knot index.
        right: u32,
    },
}

/// One canonical runtime graded term assigned to a primary basis claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisTermV3 {
    /// Zero-based claim index. The final complement claim is not term-defined.
    pub claim_index: u32,
    /// Exact nonnegative term shape.
    pub shape: BasisShapeV3,
    /// Positive amplitude in payout-scale atoms.
    pub amplitude: u64,
}

/// Caller-owned construction inputs for one canonical runtime basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisInputV3<'a> {
    /// Evaluator family.
    pub kind: BasisKindV3,
    /// Stable semantic Product identity. Omitted from semantic-basis hashing.
    pub product_id: [u8; 32],
    /// Exact Product-owned result-domain record identity. Omitted from semantic hashing.
    pub result_domain_id: [u8; 32],
    /// Exact coordinate-domain identity.
    pub coordinate_domain_id: [u8; 32],
    /// Exact result-unit identity.
    pub result_unit_id: [u8; 32],
    /// Immutable evaluator semantic release.
    pub evaluator_release_id: [u8; 32],
    /// Runtime number of basis claims.
    pub basis_width: u32,
    /// Positive exact payout scale `Q`.
    pub payout_scale: u64,
    /// Positive common denominator for all knot numerators.
    pub knot_denominator: u64,
    /// Strictly increasing Product-owned knot numerators.
    pub knots: &'a [i128],
    /// Unique ordered terms for primary claims.
    pub terms: &'a [BasisTermV3],
    /// Exact failure payout vector. Empty for categorical; width-sized for the
    /// two curve-bearing families.
    pub failure_payouts: &'a [u64],
    /// Digest of the `DCLTPGT1` price-gate certificate this basis is admitted
    /// against. All-zero for the two kinds the gate exempts by proof, and
    /// required nonzero for the spline family.
    pub price_gate_certificate_digest: [u8; 32],
}

/// Canonical semantic preimage fragments excluding only the two acyclic links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticBasisPreimageV3<'a> {
    prefix: &'a [u8],
    suffix: &'a [u8],
}

impl<'a> SemanticBasisPreimageV3<'a> {
    /// Bytes before the omitted Product and result-domain links.
    pub const fn prefix(self) -> &'a [u8] {
        self.prefix
    }

    /// Bytes after the omitted Product and result-domain links.
    pub const fn suffix(self) -> &'a [u8] {
        self.suffix
    }
}

/// Borrowed hostile-decoded runtime liability basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBasisV3<'a> {
    bytes: &'a [u8],
    kind: BasisKindV3,
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    evaluator_release_id: [u8; 32],
    basis_width: u32,
    payout_scale: u64,
    knot_denominator: u64,
    knot_count: u32,
    term_count: u32,
}

/// The record's own knot vector, read by index straight out of the
/// authenticated account body.
///
/// This is what lets the founding-time arithmetic envelope run on chain with
/// no allocator: the knots are never collected anywhere, they are read where
/// they already are.
impl SplineKnotsV3 for ProductBasisV3<'_> {
    fn knot_at(&self, index: usize) -> Option<i128> {
        self.knot_at_index(index)
    }

    fn knot_count(&self) -> usize {
        usize::try_from(self.knot_count).unwrap_or(0)
    }
}

impl<'a> ProductBasisV3<'a> {
    /// Hostile-decode and completely validate a canonical runtime basis.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < BASIS_HEADER_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != BASIS_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != BASIS_SCHEMA_V3
            || usize::from(read_u16(bytes, HEADER_BYTES_OFFSET)?) != BASIS_HEADER_BYTES_V3
        {
            return Err(Error::UnsupportedSchema);
        }
        let encoded_bytes = usize::try_from(read_u32(bytes, RECORD_BYTES_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        if encoded_bytes != bytes.len() {
            return Err(Error::InvalidLength);
        }
        let kind = BasisKindV3::decode_header(bytes)?;
        // **The reserved spans, now per-kind.** Sixteen bytes at 240 stay
        // reserved for everyone. The degree/flags pair at 18 and the
        // certificate digest at 208 belong to the spline family alone, and are
        // required zero for the two shipping kinds -- so a record cannot smuggle
        // a degree past a categorical kind byte, and a record cannot claim a
        // certificate it has no use for.
        require_zero(bytes, HEADER_TAIL_RESERVED_OFFSET, 16)?;
        match kind {
            BasisKindV3::CategoricalQ1 | BasisKindV3::GradedExactComplement => {
                require_zero(bytes, BASIS_SPLINE_DEGREE_OFFSET_V3, 2)?;
                // Hostile 16, one direction: degree 0 and 1 are exempt from the
                // price gate by proof, so a digest offered alongside one is an
                // input nobody will check. Refused rather than ignored.
                if read_array::<32>(bytes, PRICE_GATE_DIGEST_OFFSET)? != [0_u8; 32] {
                    return Err(Error::PriceGateCertificateUnexpected);
                }
            }
            BasisKindV3::SplineDegree2To3 { .. } => {
                // Hostile 16, the other direction: the whole of this family's
                // degree interval is above the gate's exempt degree, so a
                // certificate is mandatory and its absence is refused here
                // rather than at settlement.
                if read_array::<32>(bytes, PRICE_GATE_DIGEST_OFFSET)? == [0_u8; 32] {
                    return Err(Error::PriceGateCertificateRequired);
                }
            }
        }
        let value = Self {
            bytes,
            kind,
            product_id: read_nonzero_id(bytes, PRODUCT_ID_OFFSET)?,
            result_domain_id: read_nonzero_id(bytes, RESULT_DOMAIN_ID_OFFSET)?,
            coordinate_domain_id: read_nonzero_id(bytes, COORDINATE_DOMAIN_ID_OFFSET)?,
            result_unit_id: read_nonzero_id(bytes, RESULT_UNIT_ID_OFFSET)?,
            evaluator_release_id: read_nonzero_id(bytes, EVALUATOR_RELEASE_ID_OFFSET)?,
            basis_width: read_u32(bytes, BASIS_WIDTH_OFFSET_V3)?,
            payout_scale: read_u64(bytes, PAYOUT_SCALE_OFFSET)?,
            knot_denominator: read_u64(bytes, KNOT_DENOMINATOR_OFFSET)?,
            knot_count: read_u32(bytes, KNOT_COUNT_OFFSET)?,
            term_count: read_u32(bytes, TERM_COUNT_OFFSET)?,
        };
        value.validate()
    }

    fn validate(self) -> Result<Self> {
        if self.basis_width == 0 {
            return Err(Error::InvalidCount);
        }
        if self.payout_scale == 0 {
            return Err(Error::ZeroScale);
        }
        let expected = basis_record_bytes_v3(
            self.kind,
            usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)?,
            usize::try_from(self.knot_count).map_err(|_| Error::InvalidLength)?,
            usize::try_from(self.term_count).map_err(|_| Error::InvalidLength)?,
        )?;
        if expected != self.bytes.len() {
            return Err(Error::InvalidLength);
        }
        match self.kind {
            BasisKindV3::CategoricalQ1 => {
                if read_byte(self.bytes, ROUNDING_OFFSET)? != EXACT_CATEGORICAL_BOUNDARY_V3 {
                    return Err(Error::UnsupportedKind);
                }
                if self.payout_scale != 1
                    || self.knot_denominator != 1
                    || self.knot_count != 0
                    || self.term_count != 0
                {
                    return Err(Error::NonCanonicalReserved);
                }
            }
            BasisKindV3::GradedExactComplement => {
                if read_byte(self.bytes, ROUNDING_OFFSET)?
                    != TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3
                {
                    return Err(Error::UnsupportedKind);
                }
                if self.basis_width < 2 || self.knot_denominator == 0 || self.term_count == 0 {
                    return Err(Error::InvalidCount);
                }
                self.validate_knots()?;
                self.validate_terms()?;
                validate_partition(self.failure_payouts(), self.payout_scale)?;
            }
            BasisKindV3::SplineDegree2To3 {
                degree,
                interior_multiplicity,
            } => {
                // The rounding byte must AGREE with the kind, not merely be in
                // range. Cumulative-floor has its own tag because it is a
                // different function from the graded family's rule, and the
                // wire should answer "which rounding did this record use".
                if read_byte(self.bytes, ROUNDING_OFFSET)? != CUMULATIVE_FLOOR_BOUNDARY_V3 {
                    return Err(Error::UnsupportedKind);
                }
                // Terms are kind-inactive here: a spline's weights are
                // structural, induced by knots and degree together, not a sum
                // of independently-amplitude-scaled terms. Forced canonical
                // rather than left free.
                if self.term_count != 0 {
                    return Err(Error::NonCanonicalReserved);
                }
                if self.knot_denominator == 0 {
                    return Err(Error::ZeroDenominator);
                }
                if spline_basis_width_v3(self.knot_count, degree)? != self.basis_width {
                    return Err(Error::SplineWidthDerivationMismatch);
                }
                self.validate_spline_knots(degree, interior_multiplicity)?;
                // The founding-time arithmetic envelope, at decode. A record
                // that could overflow the triangle at some coordinate it must
                // accept never becomes a decoded basis at all.
                spline_arithmetic_envelope_v3(&self, degree, self.basis_width, self.payout_scale)?;
                validate_partition(self.failure_payouts(), self.payout_scale)?;
            }
        }
        Ok(self)
    }

    fn validate_knots(self) -> Result<()> {
        let mut prior = None;
        for knot in self.knots() {
            if prior.is_some_and(|value| knot <= value) {
                return Err(Error::UnorderedKnots);
            }
            prior = Some(knot);
        }
        Ok(())
    }

    /// The spline family's knot rule: non-decreasing, with repeats governed by
    /// where they sit.
    ///
    /// **The relaxation is per-kind and it does not leak.** The two shipping
    /// kinds keep [`Self::validate_knots`]'s strictly-increasing rule
    /// unconditionally; this function is reached only from the spline arm.
    ///
    /// Within it, a repeat is either structural or declared:
    ///
    /// - **End clamping** — up to `degree + 1` equal knots at each end — is how
    ///   a B-spline is pinned to its domain boundary. It is structural and
    ///   always admitted.
    /// - **An interior repeat** lowers continuity, putting a corner inside an
    ///   otherwise smooth basis. That is a real change to what the Market pays,
    ///   so the record must *declare* it in the flags byte and carry the
    ///   declaration in its digest. Undeclared, it refuses.
    ///
    /// A strict decrease is [`Error::UnorderedKnots`] for every kind.
    fn validate_spline_knots(self, degree: u8, interior_multiplicity: bool) -> Result<()> {
        validate_spline_knot_slice(&self, degree, interior_multiplicity)
    }

    fn validate_terms(self) -> Result<()> {
        let primary_count = self.basis_width.checked_sub(1).ok_or(Error::InvalidCount)?;
        let mut prior = None;
        let mut last_claim = None;
        for term in self.terms() {
            if term.claim_index >= primary_count {
                return Err(Error::InvalidTerm);
            }
            validate_shape(term.shape, self.knot_count)?;
            if term.amplitude == 0 {
                return Err(Error::InvalidTerm);
            }
            let key = (term.claim_index, shape_key(term.shape));
            if prior.is_some_and(|value| key <= value) {
                return Err(Error::NonCanonicalTermOrder);
            }
            if let Some(claim) = last_claim {
                if term.claim_index != claim
                    && term.claim_index != claim.checked_add(1).ok_or(Error::InvalidCount)?
                {
                    return Err(Error::InvalidTerm);
                }
            } else if term.claim_index != 0 {
                return Err(Error::InvalidTerm);
            }
            last_claim = Some(term.claim_index);
            prior = Some(key);
        }
        if last_claim
            .and_then(|claim| claim.checked_add(1))
            .ok_or(Error::InvalidTerm)?
            != primary_count
        {
            return Err(Error::InvalidTerm);
        }
        self.validate_envelope()
    }

    fn validate_envelope(self) -> Result<()> {
        let mut knots = self.knots();
        let Some(mut left) = knots.next() else {
            return self.validate_envelope_cell(0, 1, 0, 1);
        };
        let mut had_cell = false;
        for right in knots {
            self.validate_envelope_cell(left, self.knot_denominator, right, self.knot_denominator)?;
            left = right;
            had_cell = true;
        }
        if !had_cell {
            self.validate_envelope_cell(left, self.knot_denominator, left, self.knot_denominator)?;
        }
        Ok(())
    }

    fn validate_envelope_cell(
        self,
        left_numerator: i128,
        left_denominator: u64,
        right_numerator: i128,
        right_denominator: u64,
    ) -> Result<()> {
        let mut bound = 0_u64;
        for term in self.terms() {
            let left = evaluate_term(self, term, left_numerator, left_denominator)?;
            let right = evaluate_term(self, term, right_numerator, right_denominator)?;
            bound = bound
                .checked_add(left.max(right))
                .ok_or(Error::ArithmeticOverflow)?;
        }
        if bound > self.payout_scale {
            return Err(Error::NonPartition);
        }
        Ok(())
    }

    /// Basis evaluator family.
    pub const fn kind(self) -> BasisKindV3 {
        self.kind
    }

    /// Stable Product identity from the full linked record.
    pub const fn product_id(self) -> [u8; 32] {
        self.product_id
    }

    /// Exact Product-owned result-domain identity from the full linked record.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }

    /// Semantic coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> [u8; 32] {
        self.coordinate_domain_id
    }

    /// Exact result-unit identity.
    pub const fn result_unit_id(self) -> [u8; 32] {
        self.result_unit_id
    }

    /// Immutable evaluator semantic release.
    pub const fn evaluator_release_id(self) -> [u8; 32] {
        self.evaluator_release_id
    }

    /// Runtime basis width.
    pub const fn basis_width(self) -> u32 {
        self.basis_width
    }

    /// Exact positive payout scale `Q`.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }

    /// Positive common knot denominator.
    pub const fn knot_denominator(self) -> u64 {
        self.knot_denominator
    }

    /// Runtime knot count.
    pub const fn knot_count(self) -> u32 {
        self.knot_count
    }

    /// Runtime term count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Evaluate one canonical primary-claim term at an exact rational coordinate.
    ///
    /// This narrow projection lets offline compilers derive independently
    /// checkable categorical error bounds without reimplementing evaluator
    /// arithmetic. It never evaluates or allocates the complement claim.
    pub fn evaluate_term_rational(
        self,
        term_index: u32,
        numerator: i128,
        denominator: u64,
    ) -> Result<(u32, u64)> {
        if self.kind != BasisKindV3::GradedExactComplement {
            return Err(Error::UnsupportedCoordinate);
        }
        if denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let index = usize::try_from(term_index).map_err(|_| Error::InvalidTerm)?;
        let term = self.terms().nth(index).ok_or(Error::InvalidTerm)?;
        Ok((
            term.claim_index,
            evaluate_term(self, term, numerator, denominator)?,
        ))
    }

    /// Borrow all exact Product-owned knot numerators.
    pub fn knots(self) -> KnotIterV3<'a> {
        let start = self.knots_offset().unwrap_or(self.bytes.len());
        KnotIterV3 {
            bytes: self.bytes.get(start..).unwrap_or(&[]),
            next: 0,
            count: self.knot_count,
        }
    }

    /// Borrow all unique ordered primary-claim terms.
    pub fn terms(self) -> TermIterV3<'a> {
        let start = self.terms_offset().unwrap_or(self.bytes.len());
        TermIterV3 {
            bytes: self.bytes.get(start..).unwrap_or(&[]),
            next: 0,
            count: self.term_count,
        }
    }

    /// Borrow the exact failure payout vector for a graded basis.
    pub fn failure_payouts(self) -> PayoutIterV3<'a> {
        let count = if self.carries_failure_payouts() {
            self.basis_width
        } else {
            0
        };
        PayoutIterV3 {
            bytes: self.bytes.get(BASIS_HEADER_BYTES_V3..).unwrap_or(&[]),
            next: 0,
            count,
        }
    }

    /// Evaluate an ordinary exact rational coordinate into an exact partition.
    ///
    /// Every possible refusal is preflighted before `output` changes.
    pub fn evaluate_rational(
        self,
        numerator: i128,
        denominator: u64,
        output: &mut [u64],
    ) -> Result<()> {
        // **The curvature arm.** The weights are exact rational de Boor and the
        // apportionment is cumulative-floor, the rule WAVE `76e2ca3f` blessed
        // and the rule this record's own rounding byte names. Every refusal
        // reachable here was already refused at `decode` -- the degree, the
        // width derivation and the arithmetic envelope are all record
        // properties -- so a decoded spline basis evaluates at every coordinate
        // the family accepts.
        if let BasisKindV3::SplineDegree2To3 { degree, .. } = self.kind {
            self.require_output(output)?;
            if denominator == 0 {
                return Err(Error::ZeroDenominator);
            }
            let weights = evaluate_spline_weights_v3(
                &self,
                self.knot_denominator,
                numerator,
                denominator,
                degree,
                self.basis_width,
            )?;
            return apportion_cumulative_v3(&weights, self.payout_scale, output);
        }
        if self.kind != BasisKindV3::GradedExactComplement {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        if denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        for term in self.terms() {
            evaluate_term(self, term, numerator, denominator)?;
        }
        output.fill(0);
        let mut total = 0_u64;
        for term in self.terms() {
            let payout = evaluate_term(self, term, numerator, denominator)?;
            let index = usize::try_from(term.claim_index).map_err(|_| Error::InvalidTerm)?;
            let claim = output.get_mut(index).ok_or(Error::InvalidTerm)?;
            *claim = claim.checked_add(payout).ok_or(Error::ArithmeticOverflow)?;
            total = total.checked_add(payout).ok_or(Error::ArithmeticOverflow)?;
        }
        let complement = self
            .payout_scale
            .checked_sub(total)
            .ok_or(Error::NonPartition)?;
        let last = output.last_mut().ok_or(Error::InvalidLength)?;
        *last = complement;
        Ok(())
    }

    /// Evaluate the Product's explicit resolution-failure terminal result.
    pub fn evaluate_failure(self, output: &mut [u64]) -> Result<()> {
        if !self.carries_failure_payouts() {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        for (destination, payout) in output.iter_mut().zip(self.failure_payouts()) {
            *destination = payout;
        }
        Ok(())
    }

    /// Evaluate a categorical selector. This is the exact `Q = 1` embedding.
    pub fn evaluate_categorical(self, selector: u32, output: &mut [u64]) -> Result<()> {
        if self.kind != BasisKindV3::CategoricalQ1 {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        if selector >= self.basis_width {
            return Err(Error::SelectorOutOfRange);
        }
        let index = usize::try_from(selector).map_err(|_| Error::SelectorOutOfRange)?;
        output.fill(0);
        *output.get_mut(index).ok_or(Error::SelectorOutOfRange)? = 1;
        Ok(())
    }

    fn require_output(self, output: &[u64]) -> Result<()> {
        if output.len() != usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)? {
            return Err(Error::InvalidLength);
        }
        Ok(())
    }

    /// Admit this record's basis selection, or refuse at the failing conjunct.
    ///
    /// **This is the founding-time gate**, and it is the method a founding
    /// route calls rather than reaching for
    /// [`admit_basis_selection_v3`](crate::spline_admission_v3::admit_basis_selection_v3)
    /// with a hand-built selection. Building the selection from the record
    /// itself is what keeps the thing admitted and the thing authenticated the
    /// same object: every field below is read off the authenticated bytes, and
    /// none of them is supplied by the caller.
    ///
    /// `price_gate_certificate_digest` is read from the record's reserved tail,
    /// which is zero-enforced today — so for the two shipping kinds this is a
    /// total function returning `Ok`, and the call exists to put the gate on
    /// the route *before* the wire can carry a kind that needs it. A gate
    /// introduced in the same commit that first accepts curvature is a gate
    /// nobody has ever seen run.
    pub fn admit_selection_v3(self) -> Result<()> {
        admit_basis_selection_v3(BasisSelectionV3 {
            kind: self.kind,
            knot_count: self.knot_count,
            basis_width: self.basis_width,
            knots: &self,
            payout_scale: self.payout_scale,
            price_gate_certificate_digest: self.price_gate_certificate_digest_v3(),
        })
    }

    /// The price-gate certificate digest carried in the record's reserved tail.
    ///
    /// Nonzero exactly for the spline family: [`Self::decode`] requires the
    /// span zero for the two exempt kinds and nonzero for the curved one, so
    /// the digest and the degree agree in both directions.
    pub fn price_gate_certificate_digest_v3(self) -> [u8; 32] {
        read_array::<32>(self.bytes, PRICE_GATE_DIGEST_OFFSET).unwrap_or([0_u8; 32])
    }

    fn knot_at_index(self, index: usize) -> Option<i128> {
        if index >= usize::try_from(self.knot_count).ok()? {
            return None;
        }
        let offset = self
            .knots_offset()
            .ok()?
            .checked_add(index.checked_mul(KNOT_BYTES_V3)?)?;
        Some(i128::from_le_bytes(
            read_array::<16>(self.bytes, offset).ok()?,
        ))
    }

    /// Whether this family carries an explicit resolution-failure payout
    /// vector ahead of its knots.
    ///
    /// The spline family carries one for the same reason the graded family
    /// does: resolution failure is a real terminal and the basis has to say
    /// what it pays. Reusing the existing tail slot rather than inventing a
    /// spline-specific one is what lets `evaluate_failure` and the paired
    /// settlement match answer `(kind 3, ResolutionFailure)` with the arm they
    /// already have.
    fn carries_failure_payouts(self) -> bool {
        matches!(
            self.kind,
            BasisKindV3::GradedExactComplement | BasisKindV3::SplineDegree2To3 { .. }
        )
    }

    fn knots_offset(self) -> Result<usize> {
        let failures = if self.carries_failure_payouts() {
            usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)?
        } else {
            0
        };
        BASIS_HEADER_BYTES_V3
            .checked_add(failures.checked_mul(8).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)
    }

    fn terms_offset(self) -> Result<usize> {
        self.knots_offset()?
            .checked_add(
                usize::try_from(self.knot_count)
                    .map_err(|_| Error::InvalidLength)?
                    .checked_mul(KNOT_BYTES_V3)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)
    }
}

/// Exact record width for caller-owned allocation.
pub fn basis_record_bytes_v3(
    kind: BasisKindV3,
    basis_width: usize,
    knot_count: usize,
    term_count: usize,
) -> Result<usize> {
    let failure_count = match kind {
        BasisKindV3::CategoricalQ1 => 0,
        // Both curve-bearing families carry a width-sized failure vector.
        BasisKindV3::GradedExactComplement | BasisKindV3::SplineDegree2To3 { .. } => basis_width,
    };
    BASIS_HEADER_BYTES_V3
        .checked_add(failure_count.checked_mul(8).ok_or(Error::InvalidLength)?)
        .and_then(|value| value.checked_add(knot_count.checked_mul(KNOT_BYTES_V3)?))
        .and_then(|value| value.checked_add(term_count.checked_mul(TERM_BYTES_V3)?))
        .ok_or(Error::InvalidLength)
}

/// Compile one canonical runtime basis into an exact caller-owned buffer.
///
/// All input validation completes before the first output mutation.
pub fn compile_basis_v3(input: BasisInputV3<'_>, output: &mut [u8]) -> Result<()> {
    validate_input(input)?;
    let expected = basis_record_bytes_v3(
        input.kind,
        usize::try_from(input.basis_width).map_err(|_| Error::InvalidLength)?,
        input.knots.len(),
        input.terms.len(),
    )?;
    if output.len() != expected {
        return Err(Error::InvalidLength);
    }
    let record_bytes = u32::try_from(expected).map_err(|_| Error::InvalidLength)?;
    let knot_count = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
    let term_count = u32::try_from(input.terms.len()).map_err(|_| Error::InvalidCount)?;
    output.fill(0);
    put(output, 0, &BASIS_MAGIC_V3)?;
    put(output, 8, &BASIS_SCHEMA_V3.to_le_bytes())?;
    put(
        output,
        HEADER_BYTES_OFFSET,
        &u16::try_from(BASIS_HEADER_BYTES_V3)
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    put(output, RECORD_BYTES_OFFSET, &record_bytes.to_le_bytes())?;
    put(output, KIND_OFFSET, &[input.kind.tag()])?;
    let rounding = match input.kind {
        BasisKindV3::CategoricalQ1 => EXACT_CATEGORICAL_BOUNDARY_V3,
        BasisKindV3::GradedExactComplement => TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3,
        // Cumulative-floor, WAVE 76e2ca3f's ruling, named on the wire.
        BasisKindV3::SplineDegree2To3 { .. } => CUMULATIVE_FLOOR_BOUNDARY_V3,
    };
    put(output, ROUNDING_OFFSET, &[rounding])?;
    // The three spans this family spends. Zero for the two exempt kinds, which
    // is what `decode` requires of them, so `output.fill(0)` above is already
    // the whole story there.
    put(
        output,
        BASIS_SPLINE_DEGREE_OFFSET_V3,
        &[input.kind.degree_byte()],
    )?;
    put(output, BASIS_SPLINE_FLAGS_OFFSET_V3, &[input.kind.flags()])?;
    put(
        output,
        PRICE_GATE_DIGEST_OFFSET,
        &input.price_gate_certificate_digest,
    )?;
    put(
        output,
        BASIS_WIDTH_OFFSET_V3,
        &input.basis_width.to_le_bytes(),
    )?;
    put(output, KNOT_COUNT_OFFSET, &knot_count.to_le_bytes())?;
    put(output, TERM_COUNT_OFFSET, &term_count.to_le_bytes())?;
    put(output, PRODUCT_ID_OFFSET, &input.product_id)?;
    put(output, RESULT_DOMAIN_ID_OFFSET, &input.result_domain_id)?;
    put(
        output,
        COORDINATE_DOMAIN_ID_OFFSET,
        &input.coordinate_domain_id,
    )?;
    put(output, RESULT_UNIT_ID_OFFSET, &input.result_unit_id)?;
    put(
        output,
        PAYOUT_SCALE_OFFSET,
        &input.payout_scale.to_le_bytes(),
    )?;
    put(
        output,
        KNOT_DENOMINATOR_OFFSET,
        &input.knot_denominator.to_le_bytes(),
    )?;
    put(
        output,
        EVALUATOR_RELEASE_ID_OFFSET,
        &input.evaluator_release_id,
    )?;
    let mut offset = BASIS_HEADER_BYTES_V3;
    for payout in input.failure_payouts {
        put(output, offset, &payout.to_le_bytes())?;
        offset = offset.checked_add(8).ok_or(Error::InvalidLength)?;
    }
    for knot in input.knots {
        put(output, offset, &knot.to_le_bytes())?;
        offset = offset
            .checked_add(KNOT_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
    }
    for term in input.terms {
        encode_term(output, offset, *term)?;
        offset = offset
            .checked_add(TERM_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
    }
    let _ = ProductBasisV3::decode(output)?;
    Ok(())
}

/// Validate one record and expose its acyclic semantic-identity fragments.
pub fn semantic_basis_preimage_v3(bytes: &[u8]) -> Result<SemanticBasisPreimageV3<'_>> {
    let _ = ProductBasisV3::decode(bytes)?;
    Ok(SemanticBasisPreimageV3 {
        prefix: bytes.get(..PRODUCT_ID_OFFSET).ok_or(Error::InvalidLength)?,
        suffix: bytes.get(PRODUCT_LINK_END..).ok_or(Error::InvalidLength)?,
    })
}

fn validate_input(input: BasisInputV3<'_>) -> Result<()> {
    for id in [
        input.product_id,
        input.result_domain_id,
        input.coordinate_domain_id,
        input.result_unit_id,
        input.evaluator_release_id,
    ] {
        if id.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentifier);
        }
    }
    if input.basis_width == 0 {
        return Err(Error::InvalidCount);
    }
    if input.payout_scale == 0 {
        return Err(Error::ZeroScale);
    }
    let _ = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
    let _ = u32::try_from(input.terms.len()).map_err(|_| Error::InvalidCount)?;
    let offered_certificate = input
        .price_gate_certificate_digest
        .iter()
        .any(|byte| *byte != 0);
    match input.kind {
        BasisKindV3::CategoricalQ1 => {
            if offered_certificate {
                return Err(Error::PriceGateCertificateUnexpected);
            }
            if input.payout_scale != 1
                || input.knot_denominator != 1
                || !input.knots.is_empty()
                || !input.terms.is_empty()
                || !input.failure_payouts.is_empty()
            {
                return Err(Error::NonCanonicalReserved);
            }
        }
        BasisKindV3::SplineDegree2To3 {
            degree,
            interior_multiplicity,
        } => {
            if !SPLINE_EVALUATOR_RELEASED_V3 {
                return Err(Error::SplineEvaluatorAbsent);
            }
            if !offered_certificate {
                return Err(Error::PriceGateCertificateRequired);
            }
            if !input.terms.is_empty() || input.knot_denominator == 0 {
                return Err(Error::NonCanonicalReserved);
            }
            let knot_count = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
            if spline_basis_width_v3(knot_count, degree)? != input.basis_width {
                return Err(Error::SplineWidthDerivationMismatch);
            }
            if input.failure_payouts.len()
                != usize::try_from(input.basis_width).map_err(|_| Error::InvalidLength)?
            {
                return Err(Error::InvalidLength);
            }
            validate_spline_knot_slice(input.knots, degree, interior_multiplicity)?;
            spline_arithmetic_envelope_v3(
                input.knots,
                degree,
                input.basis_width,
                input.payout_scale,
            )?;
            validate_partition(input.failure_payouts.iter().copied(), input.payout_scale)?;
        }
        BasisKindV3::GradedExactComplement => {
            if offered_certificate {
                return Err(Error::PriceGateCertificateUnexpected);
            }
            if input.basis_width < 2 || input.knot_denominator == 0 || input.terms.is_empty() {
                return Err(Error::InvalidCount);
            }
            if input.failure_payouts.len()
                != usize::try_from(input.basis_width).map_err(|_| Error::InvalidLength)?
            {
                return Err(Error::InvalidLength);
            }
            let mut prior_knot = None;
            for knot in input.knots {
                if prior_knot.is_some_and(|value| *knot <= value) {
                    return Err(Error::UnorderedKnots);
                }
                prior_knot = Some(*knot);
            }
            let primary_count = input
                .basis_width
                .checked_sub(1)
                .ok_or(Error::InvalidCount)?;
            let knot_count = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
            let mut prior_term = None;
            let mut last_claim = None;
            for term in input.terms {
                if term.claim_index >= primary_count || term.amplitude == 0 {
                    return Err(Error::InvalidTerm);
                }
                validate_shape(term.shape, knot_count)?;
                let key = (term.claim_index, shape_key(term.shape));
                if prior_term.is_some_and(|prior| key <= prior) {
                    return Err(Error::NonCanonicalTermOrder);
                }
                if let Some(claim) = last_claim {
                    if term.claim_index != claim
                        && term.claim_index != claim.checked_add(1).ok_or(Error::InvalidCount)?
                    {
                        return Err(Error::InvalidTerm);
                    }
                } else if term.claim_index != 0 {
                    return Err(Error::InvalidTerm);
                }
                last_claim = Some(term.claim_index);
                prior_term = Some(key);
            }
            if last_claim
                .and_then(|claim| claim.checked_add(1))
                .ok_or(Error::InvalidTerm)?
                != primary_count
            {
                return Err(Error::InvalidTerm);
            }
            validate_input_envelope(input)?;
            validate_partition(input.failure_payouts.iter().copied(), input.payout_scale)?;
        }
    }
    Ok(())
}

/// The spline family's knot rule, over any knot source.
///
/// **The relaxation is per-kind and it does not leak.** The two shipping kinds
/// keep the strictly-increasing rule of `validate_knots` unconditionally; this
/// function is reached only from the spline arms of `validate` and
/// `validate_input`. Within it a repeat is either structural or declared:
///
/// - **End clamping** — up to `degree + 1` equal knots at each end — is how a
///   B-spline pins itself to its domain boundary. Structural, always admitted.
/// - **An interior repeat** lowers continuity, putting a corner inside an
///   otherwise smooth basis. That changes what the Market pays, so the record
///   must declare it in the flags byte and carry the declaration in its digest.
///   Undeclared, it refuses.
///
/// A strict decrease is [`Error::UnorderedKnots`] for every kind.
fn validate_spline_knot_slice<K: SplineKnotsV3 + ?Sized>(
    knots: &K,
    degree: u8,
    interior_multiplicity: bool,
) -> Result<()> {
    let degree = usize::from(degree);
    let count = knots.knot_count();
    let last_clamp_start = count
        .checked_sub(degree)
        .and_then(|value| value.checked_sub(1))
        .ok_or(Error::SplineWidthDerivationMismatch)?;
    let mut prior: Option<i128> = None;
    for index in 0..count {
        let knot = knots.knot_at(index).ok_or(Error::InvalidCount)?;
        if let Some(previous) = prior {
            if knot < previous {
                return Err(Error::UnorderedKnots);
            }
            if knot == previous {
                let leading = index <= degree;
                let trailing = index
                    .checked_sub(1)
                    .is_some_and(|left| left >= last_clamp_start);
                if !(leading || trailing) && !interior_multiplicity {
                    return Err(Error::SplineInteriorMultiplicityUndeclared);
                }
            }
        }
        prior = Some(knot);
    }
    Ok(())
}

fn validate_partition(payouts: impl Iterator<Item = u64>, scale: u64) -> Result<()> {
    let mut count = 0_u32;
    let mut total = 0_u64;
    for payout in payouts {
        count = count.checked_add(1).ok_or(Error::InvalidCount)?;
        if payout > scale {
            return Err(Error::NonPartition);
        }
        total = total.checked_add(payout).ok_or(Error::ArithmeticOverflow)?;
    }
    if count == 0 || total != scale {
        return Err(Error::NonPartition);
    }
    Ok(())
}

fn validate_input_envelope(input: BasisInputV3<'_>) -> Result<()> {
    let mut knots = input.knots.iter().copied();
    let Some(mut left) = knots.next() else {
        return validate_input_envelope_cell(input, 0, 1, 0, 1);
    };
    let mut had_cell = false;
    for right in knots {
        validate_input_envelope_cell(
            input,
            left,
            input.knot_denominator,
            right,
            input.knot_denominator,
        )?;
        left = right;
        had_cell = true;
    }
    if !had_cell {
        validate_input_envelope_cell(
            input,
            left,
            input.knot_denominator,
            left,
            input.knot_denominator,
        )?;
    }
    Ok(())
}

fn validate_input_envelope_cell(
    input: BasisInputV3<'_>,
    left_numerator: i128,
    left_denominator: u64,
    right_numerator: i128,
    right_denominator: u64,
) -> Result<()> {
    let mut bound = 0_u64;
    for term in input.terms {
        let left = evaluate_input_term(input, *term, left_numerator, left_denominator)?;
        let right = evaluate_input_term(input, *term, right_numerator, right_denominator)?;
        bound = bound
            .checked_add(left.max(right))
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if bound > input.payout_scale {
        return Err(Error::NonPartition);
    }
    Ok(())
}

fn evaluate_term(
    basis: ProductBasisV3<'_>,
    term: BasisTermV3,
    numerator: i128,
    denominator: u64,
) -> Result<u64> {
    evaluate_term_with_knots(
        term,
        numerator,
        denominator,
        basis.knot_denominator,
        |index| {
            basis
                .knots()
                .nth(usize::try_from(index).map_err(|_| Error::InvalidTerm)?)
                .ok_or(Error::InvalidTerm)
        },
    )
}

fn evaluate_input_term(
    input: BasisInputV3<'_>,
    term: BasisTermV3,
    numerator: i128,
    denominator: u64,
) -> Result<u64> {
    evaluate_term_with_knots(
        term,
        numerator,
        denominator,
        input.knot_denominator,
        |index| {
            input
                .knots
                .get(usize::try_from(index).map_err(|_| Error::InvalidTerm)?)
                .copied()
                .ok_or(Error::InvalidTerm)
        },
    )
}

fn evaluate_term_with_knots(
    term: BasisTermV3,
    numerator: i128,
    denominator: u64,
    knot_denominator: u64,
    mut knot: impl FnMut(u32) -> Result<i128>,
) -> Result<u64> {
    match term.shape {
        BasisShapeV3::Constant => Ok(term.amplitude),
        BasisShapeV3::RampUp { left, right } => ramp(
            term.amplitude,
            knot(left)?,
            knot(right)?,
            knot_denominator,
            numerator,
            denominator,
            true,
        ),
        BasisShapeV3::RampDown { left, right } => ramp(
            term.amplitude,
            knot(left)?,
            knot(right)?,
            knot_denominator,
            numerator,
            denominator,
            false,
        ),
        BasisShapeV3::Tent { left, peak, right } => {
            let rising = ramp(
                term.amplitude,
                knot(left)?,
                knot(peak)?,
                knot_denominator,
                numerator,
                denominator,
                true,
            )?;
            let falling = ramp(
                term.amplitude,
                knot(peak)?,
                knot(right)?,
                knot_denominator,
                numerator,
                denominator,
                false,
            )?;
            Ok(rising.min(falling))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ramp(
    amplitude: u64,
    left: i128,
    right: i128,
    knot_denominator: u64,
    numerator: i128,
    denominator: u64,
    rising: bool,
) -> Result<u64> {
    let left_cmp = rational_compare(numerator, denominator, left, knot_denominator)
        .map_err(|_| Error::ArithmeticOverflow)?;
    let right_cmp = rational_compare(numerator, denominator, right, knot_denominator)
        .map_err(|_| Error::ArithmeticOverflow)?;
    if rising {
        if left_cmp != core::cmp::Ordering::Greater {
            Ok(0)
        } else if right_cmp != core::cmp::Ordering::Less {
            Ok(amplitude)
        } else {
            interpolation_floor(
                amplitude,
                numerator,
                denominator,
                left,
                right,
                knot_denominator,
                true,
            )
            .map_err(|_| Error::ArithmeticOverflow)
        }
    } else if left_cmp != core::cmp::Ordering::Greater {
        Ok(amplitude)
    } else if right_cmp != core::cmp::Ordering::Less {
        Ok(0)
    } else {
        interpolation_floor(
            amplitude,
            numerator,
            denominator,
            left,
            right,
            knot_denominator,
            false,
        )
        .map_err(|_| Error::ArithmeticOverflow)
    }
}

fn validate_shape(shape: BasisShapeV3, knot_count: u32) -> Result<()> {
    match shape {
        BasisShapeV3::Constant => Ok(()),
        BasisShapeV3::RampUp { left, right } | BasisShapeV3::RampDown { left, right }
            if left < right && right < knot_count =>
        {
            Ok(())
        }
        BasisShapeV3::Tent { left, peak, right }
            if left < peak && peak < right && right < knot_count =>
        {
            Ok(())
        }
        _ => Err(Error::InvalidTerm),
    }
}

fn shape_key(shape: BasisShapeV3) -> (u8, u32, u32, u32) {
    match shape {
        BasisShapeV3::Constant => (TERM_CONSTANT_SHAPE_V3, 0, 0, 0),
        BasisShapeV3::RampUp { left, right } => (TERM_RAMP_UP_SHAPE_V3, left, 0, right),
        BasisShapeV3::RampDown { left, right } => (TERM_RAMP_DOWN_SHAPE_V3, left, 0, right),
        BasisShapeV3::Tent { left, peak, right } => (TERM_TENT_SHAPE_V3, left, peak, right),
    }
}

fn encode_term(output: &mut [u8], offset: usize, term: BasisTermV3) -> Result<()> {
    put(
        output,
        offset
            .checked_add(TERM_CLAIM_INDEX_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &term.claim_index.to_le_bytes(),
    )?;
    let (tag, left, peak, right) = shape_key(term.shape);
    put(
        output,
        offset
            .checked_add(TERM_SHAPE_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &[tag],
    )?;
    put(
        output,
        offset
            .checked_add(TERM_LEFT_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &left.to_le_bytes(),
    )?;
    put(
        output,
        offset
            .checked_add(TERM_PEAK_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &peak.to_le_bytes(),
    )?;
    put(
        output,
        offset
            .checked_add(TERM_RIGHT_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &right.to_le_bytes(),
    )?;
    put(
        output,
        offset
            .checked_add(TERM_AMPLITUDE_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        &term.amplitude.to_le_bytes(),
    )
}

fn decode_term(input: &[u8], offset: usize) -> Result<BasisTermV3> {
    require_zero(
        input,
        offset
            .checked_add(TERM_SHAPE_RESERVED_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        3,
    )?;
    require_zero(
        input,
        offset
            .checked_add(TERM_TAIL_RESERVED_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
        4,
    )?;
    let claim_index = read_u32(
        input,
        offset
            .checked_add(TERM_CLAIM_INDEX_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
    )?;
    let tag = read_byte(
        input,
        offset
            .checked_add(TERM_SHAPE_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
    )?;
    let left = read_u32(
        input,
        offset
            .checked_add(TERM_LEFT_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
    )?;
    let peak = read_u32(
        input,
        offset
            .checked_add(TERM_PEAK_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
    )?;
    let right = read_u32(
        input,
        offset
            .checked_add(TERM_RIGHT_OFFSET_V3)
            .ok_or(Error::InvalidLength)?,
    )?;
    let shape = match tag {
        TERM_CONSTANT_SHAPE_V3 if left == 0 && peak == 0 && right == 0 => BasisShapeV3::Constant,
        TERM_RAMP_UP_SHAPE_V3 if peak == 0 => BasisShapeV3::RampUp { left, right },
        TERM_RAMP_DOWN_SHAPE_V3 if peak == 0 => BasisShapeV3::RampDown { left, right },
        TERM_TENT_SHAPE_V3 => BasisShapeV3::Tent { left, peak, right },
        _ => return Err(Error::InvalidTerm),
    };
    Ok(BasisTermV3 {
        claim_index,
        shape,
        amplitude: read_u64(
            input,
            offset
                .checked_add(TERM_AMPLITUDE_OFFSET_V3)
                .ok_or(Error::InvalidLength)?,
        )?,
    })
}

/// Exact-size borrowed knot iterator.
#[derive(Clone, Debug)]
pub struct KnotIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for KnotIterV3<'_> {
    type Item = i128;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(KNOT_BYTES_V3)?;
        let value = i128::from_le_bytes(read_array::<16>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for KnotIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

/// Exact-size borrowed graded-term iterator.
#[derive(Clone, Debug)]
pub struct TermIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for TermIterV3<'_> {
    type Item = BasisTermV3;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(TERM_BYTES_V3)?;
        let value = decode_term(self.bytes, offset).ok()?;
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for TermIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

/// Exact-size borrowed payout iterator.
#[derive(Clone, Debug)]
pub struct PayoutIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for PayoutIterV3<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(8)?;
        let value = u64::from_le_bytes(read_array::<8>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for PayoutIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

fn read_nonzero_id(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = read_array(input, offset)?;
    if value.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(value)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn id(fill: u8) -> [u8; 32] {
        [fill; 32]
    }

    /// A canonical degree-2 spline input: clamped uniform knots, no terms, a
    /// width-sized failure vector, and a certificate digest.
    fn spline_input<'a>(knots: &'a [i128], failure: &'a [u64], degree: u8) -> BasisInputV3<'a> {
        BasisInputV3 {
            kind: BasisKindV3::SplineDegree2To3 {
                degree,
                interior_multiplicity: false,
            },
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: u32::try_from(failure.len()).expect("width"),
            payout_scale: 1_000_000,
            knot_denominator: 1,
            knots,
            terms: &[],
            failure_payouts: failure,
            price_gate_certificate_digest: [3_u8; 32],
        }
    }

    fn graded_input<'a>(
        knots: &'a [i128],
        terms: &'a [BasisTermV3],
        failure: &'a [u64],
    ) -> BasisInputV3<'a> {
        BasisInputV3 {
            kind: BasisKindV3::GradedExactComplement,
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: 3,
            payout_scale: 100,
            knot_denominator: 2,
            knots,
            terms,
            failure_payouts: failure,
            // Exempt by proof: degree 0 and 1 need no price gate,
            // and a digest offered alongside one is refused.
            price_gate_certificate_digest: [0_u8; 32],
        }
    }

    fn compile(input: BasisInputV3<'_>) -> Vec<u8> {
        let width = basis_record_bytes_v3(
            input.kind,
            usize::try_from(input.basis_width).expect("width"),
            input.knots.len(),
            input.terms.len(),
        )
        .expect("record width");
        let mut output = vec![0; width];
        compile_basis_v3(input, &mut output).expect("compile");
        output
    }

    /// **The lane's central claim, asserted rather than argued.** The set of
    /// byte strings this codec accepts is exactly what it was before the third
    /// kind was allocated: a valid record whose kind byte is overwritten with
    /// 3 is refused, and the refusal happens at the kind byte rather than
    /// somewhere downstream where a partially-decoded record could do damage.
    ///
    /// Hostile 1 of `docs/design/BASIS_ABI_UNIFICATION_V1.md` §7. It refused
    /// before this lane too, as `UnsupportedKind`; what changed is that the
    /// refusal now names an allocated tag instead of an unrecognised one.
    #[test]
    fn a_record_whose_kind_byte_is_three_is_refused() {
        let knots = [0_i128, 2, 4];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 1 },
                amplitude: 40,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::RampUp { left: 1, right: 2 },
                amplitude: 60,
            },
        ];
        let failure = [10_u64, 20, 70];
        let mut bytes = compile(graded_input(&knots, &terms, &failure));
        ProductBasisV3::decode(&bytes).expect("the unmodified record still decodes");

        // **Hostile 1, and the flip this cut is about.** Flipping ONLY the
        // kind byte of a graded record no longer refuses for "no evaluator" --
        // it refuses because the degree span at offset 18 is still zero, which
        // is outside the family's closed interval. That is a stronger refusal
        // than the old one: it says a curved record must actually BE curved,
        // not merely claim the tag. A graded record reinterpreted as a spline
        // is exactly the migration hazard the reserved span existed to catch,
        // and this is the byte that catches it.
        bytes[KIND_OFFSET] = SPLINE_KIND;
        assert_eq!(
            ProductBasisV3::decode(&bytes),
            Err(Error::SplineDegreeOutOfProfile),
            "kind byte 3 with a zeroed degree span is not a spline record"
        );
        // Give it a degree in the profile and it still refuses -- now for the
        // certificate, which a graded record has no digest for. The cascade is
        // layered, not collapsed.
        bytes[BASIS_SPLINE_DEGREE_OFFSET_V3] = BASIS_SPLINE_MINIMUM_DEGREE_V3;
        assert_eq!(
            ProductBasisV3::decode(&bytes),
            Err(Error::PriceGateCertificateRequired),
            "a curved record with no certificate digest is refused at founding"
        );
        bytes[BASIS_SPLINE_DEGREE_OFFSET_V3] = 0;

        // And the byte above it is still unclaimed, so the allocation took
        // exactly one value out of circulation and not a range.
        bytes[KIND_OFFSET] = SPLINE_KIND + 1;
        assert_eq!(
            ProductBasisV3::decode(&bytes),
            Err(Error::UnsupportedKind),
            "byte 4 is still merely unsupported"
        );
    }

    /// The degree's wire slot was not spent. Both reserved spans are still
    /// zero-enforced for every kind, which is what lets a decoder that predates
    /// this lane refuse rather than misread anything a later one writes.
    #[test]
    fn the_reserved_spans_are_still_refused_on_nonzero() {
        let knots = [0_i128, 2, 4];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 1 },
                amplitude: 40,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::RampUp { left: 1, right: 2 },
                amplitude: 60,
            },
        ];
        let failure = [10_u64, 20, 70];
        for offset in [BASIS_SPLINE_DEGREE_OFFSET_V3, HEADER_TAIL_RESERVED_OFFSET] {
            let mut bytes = compile(graded_input(&knots, &terms, &failure));
            bytes[offset] = 2;
            assert_eq!(
                ProductBasisV3::decode(&bytes),
                Err(Error::NonCanonicalReserved),
                "offset {offset}"
            );
        }
    }

    /// **Sizing a buffer is not admitting a record**, and the two came apart
    /// when the family gained a wire form.
    ///
    /// `basis_record_bytes_v3` used to refuse the spline kind, on the grounds
    /// that no buffer should be sized for a record that could never be
    /// written. Now one can be: the family has a canonical tail -- a
    /// width-sized failure vector, then knots, and no terms -- so the width is
    /// a fact about the layout rather than a guess. Admission is a separate
    /// question, asked by `compile_basis_v3` and `decode`.
    #[test]
    fn a_spline_record_has_a_derivable_width_at_every_degree_in_the_profile() {
        for degree in [2_u8, 3] {
            let kind = BasisKindV3::SplineDegree2To3 {
                degree,
                interior_multiplicity: false,
            };
            assert_eq!(
                basis_record_bytes_v3(kind, 4, 8, 0),
                Ok(BASIS_HEADER_BYTES_V3 + 4 * 8 + 8 * KNOT_BYTES_V3),
                "degree {degree}: header, a width-sized failure vector, and the knots"
            );
        }
    }

    /// The compiler refuses a spline input while the seam is closed, and
    /// refuses one with no certificate whatever the seam says.
    ///
    /// The order matters: the seam is the outermost conjunct, so while it is
    /// shut this is the refusal every spline input gets. When it opens, the
    /// certificate conjunct is what remains -- and that is a refusal about the
    /// *input*, which no build flag can rescue.
    #[test]
    fn the_compiler_refuses_a_spline_input_without_a_certificate() {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        let expected = if SPLINE_EVALUATOR_RELEASED_V3 {
            Error::PriceGateCertificateRequired
        } else {
            Error::SplineEvaluatorAbsent
        };
        assert_eq!(
            compile_basis_v3(
                BasisInputV3 {
                    kind: BasisKindV3::SplineDegree2To3 {
                        degree: 3,
                        interior_multiplicity: false
                    },
                    ..graded_input(&[], &[], &[])
                },
                &mut output,
            ),
            Err(expected)
        );
        assert!(
            output.iter().all(|byte| *byte == 0),
            "a refused compile writes nothing"
        );
    }

    /// **Hostile 16, encode side, both directions.** A certificate digest on a
    /// kind the price gate exempts by proof is refused rather than ignored --
    /// an input that is present is never silently dropped.
    #[test]
    fn a_certificate_on_an_exempt_kind_refuses_at_compile() {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        assert_eq!(
            compile_basis_v3(
                BasisInputV3 {
                    price_gate_certificate_digest: [9_u8; 32],
                    ..graded_input(&[], &[], &[])
                },
                &mut output,
            ),
            Err(Error::PriceGateCertificateUnexpected)
        );
    }

    /// **Negative control, hostile 13.** The knot-ordering rule was not
    /// relaxed. Interior multiplicity is what a spline needs and what this
    /// lane did not grant: repeated knots still refuse on the two shipping
    /// kinds. If a later lane relaxes the check globally instead of per-kind,
    /// this goes red.
    #[test]
    fn repeated_knots_still_refuse_on_the_shipping_kinds() {
        let knots = [0_i128, 2, 2];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 1 },
                amplitude: 40,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::RampUp { left: 1, right: 2 },
                amplitude: 60,
            },
        ];
        let failure = [10_u64, 20, 70];
        assert_eq!(
            validate_input(graded_input(&knots, &terms, &failure)),
            Err(Error::UnorderedKnots)
        );
    }

    #[test]
    fn categorical_q1_embeds_at_runtime_width_258() {
        let input = BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: 258,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
            // Exempt by proof: degree 0 and 1 need no price gate,
            // and a digest offered alongside one is refused.
            price_gate_certificate_digest: [0_u8; 32],
        };
        let bytes = compile(input);
        assert_eq!(bytes.len(), BASIS_HEADER_BYTES_V3);
        assert_eq!(
            bytes.get(BASIS_WIDTH_OFFSET_V3..BASIS_WIDTH_OFFSET_V3 + 4),
            Some(258_u32.to_le_bytes().as_slice())
        );
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        assert_eq!(basis.basis_width(), 258);
        let mut output = vec![9; 258];
        basis
            .evaluate_categorical(257, &mut output)
            .expect("one hot");
        assert_eq!(output.iter().sum::<u64>(), 1);
        assert_eq!(output.get(257), Some(&1));
    }

    #[test]
    fn graded_curves_and_exact_complement_partition_every_terminal() {
        let knots = [-20, 0, 20, 40];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 2 },
                amplitude: 40,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::Tent {
                    left: 1,
                    peak: 2,
                    right: 3,
                },
                amplitude: 30,
            },
        ];
        let bytes = compile(graded_input(&knots, &terms, &[7, 11, 82]));
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        for (numerator, denominator) in [
            (i128::MIN, u64::MAX),
            (-1, 1),
            (7, 3),
            (i128::MAX, u64::MAX),
        ] {
            let mut payout = [99; 3];
            basis
                .evaluate_rational(numerator, denominator, &mut payout)
                .expect("total exact evaluation");
            assert_eq!(payout.iter().sum::<u64>(), 100);
        }
        let mut failure = [0; 3];
        basis.evaluate_failure(&mut failure).expect("failure");
        assert_eq!(failure, [7, 11, 82]);
    }

    #[test]
    fn runtime_tails_lift_sixteen_item_prototype_caps() {
        let knots: Vec<i128> = (-150..=150).map(i128::from).collect();
        let terms: Vec<BasisTermV3> = (0_u32..32)
            .map(|claim_index| BasisTermV3 {
                claim_index,
                shape: BasisShapeV3::RampUp {
                    left: claim_index + 260,
                    right: claim_index + 261,
                },
                amplitude: 1,
            })
            .collect();
        let mut failure = vec![1; 33];
        *failure.last_mut().expect("last") = 68;
        let input = BasisInputV3 {
            basis_width: 33,
            payout_scale: 100,
            knot_denominator: 1,
            knots: &knots,
            terms: &terms,
            failure_payouts: &failure,
            ..graded_input(&[], &[], &[])
        };
        let bytes = compile(input);
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        assert_eq!(basis.knot_count(), 301);
        assert_eq!(basis.term_count(), 32);
        let mut output = vec![0; 33];
        basis
            .evaluate_rational(0, 1, &mut output)
            .expect("runtime width");
        assert_eq!(output.iter().sum::<u64>(), 100);
    }

    #[test]
    fn semantic_identity_omits_links_but_not_payoff_changes() {
        let knots = [0, 10];
        let terms = [BasisTermV3 {
            claim_index: 0,
            shape: BasisShapeV3::RampUp { left: 0, right: 1 },
            amplitude: 100,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &terms, &[0, 100])
        };
        let original = compile(input);
        let relinked = compile(BasisInputV3 {
            product_id: id(91),
            result_domain_id: id(92),
            ..input
        });
        let first = semantic_basis_preimage_v3(&original).expect("semantic");
        let second = semantic_basis_preimage_v3(&relinked).expect("semantic");
        assert_eq!(first.prefix(), second.prefix());
        assert_eq!(first.suffix(), second.suffix());
        assert_ne!(original, relinked);

        let changed_terms = [BasisTermV3 {
            amplitude: 99,
            ..terms[0]
        }];
        let changed = compile(BasisInputV3 {
            terms: &changed_terms,
            ..input
        });
        assert_ne!(
            first.suffix(),
            semantic_basis_preimage_v3(&changed)
                .expect("changed semantic")
                .suffix()
        );
    }

    #[test]
    fn hostile_refusals_preserve_outputs() {
        let knots = [0, 10];
        let terms = [BasisTermV3 {
            claim_index: 0,
            shape: BasisShapeV3::RampUp { left: 0, right: 1 },
            amplitude: 60,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &terms, &[1, 99])
        };
        let bytes = compile(input);
        for width in 0..bytes.len() {
            assert_eq!(
                ProductBasisV3::decode(bytes.get(..width).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        // Byte 208 now opens the price-gate digest, so flipping it on a
        // graded record claims a certificate the family is exempt from -- a
        // different, more specific refusal than the reserved one.
        let mut claimed = bytes.clone();
        *claimed.get_mut(208).expect("digest") = 1;
        assert_eq!(
            ProductBasisV3::decode(&claimed),
            Err(Error::PriceGateCertificateUnexpected)
        );
        // The sixteen bytes that stayed reserved still refuse on nonzero.
        let mut reserved = bytes.clone();
        *reserved.get_mut(240).expect("reserved") = 1;
        assert_eq!(
            ProductBasisV3::decode(&reserved),
            Err(Error::NonCanonicalReserved)
        );
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        let mut output = [7, 8];
        assert_eq!(
            basis.evaluate_rational(1, 0, &mut output),
            Err(Error::ZeroDenominator)
        );
        assert_eq!(output, [7, 8]);
        assert_eq!(
            basis.evaluate_rational(1, 1, &mut [0]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn excess_amplitudes_and_nonpartitioned_failure_refuse_before_write() {
        let knots = [0, 10];
        let excessive = [BasisTermV3 {
            claim_index: 0,
            shape: BasisShapeV3::RampUp { left: 0, right: 1 },
            amplitude: 101,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &excessive, &[0, 100])
        };
        let width = basis_record_bytes_v3(input.kind, 2, 2, 1).expect("width");
        let mut output = vec![0xa5; width];
        assert_eq!(
            compile_basis_v3(input, &mut output),
            Err(Error::NonPartition)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));

        let valid_term = [BasisTermV3 {
            amplitude: 100,
            ..excessive[0]
        }];
        let invalid_failure = BasisInputV3 {
            terms: &valid_term,
            failure_payouts: &[0, 99],
            ..input
        };
        assert_eq!(
            compile_basis_v3(invalid_failure, &mut output),
            Err(Error::NonPartition)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }

    #[test]
    fn exact_cell_envelope_accepts_disjoint_full_scale_curves_and_refuses_claim_gaps() {
        let knots = [0, 10, 20, 30, 40, 50];
        let disjoint = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::Tent {
                    left: 0,
                    peak: 1,
                    right: 2,
                },
                amplitude: 100,
            },
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::Tent {
                    left: 3,
                    peak: 4,
                    right: 5,
                },
                amplitude: 100,
            },
        ];
        let input = BasisInputV3 {
            basis_width: 2,
            knot_denominator: 1,
            knots: &knots,
            terms: &disjoint,
            failure_payouts: &[0, 100],
            ..graded_input(&[], &[], &[])
        };
        let bytes = compile(input);
        let basis = ProductBasisV3::decode(&bytes).expect("disjoint full-scale curves");
        for coordinate in [-1, 5, 10, 15, 25, 35, 40, 45, 60] {
            let mut payout = [0; 2];
            basis
                .evaluate_rational(coordinate, 1, &mut payout)
                .expect("partition");
            assert_eq!(payout.iter().sum::<u64>(), 100);
        }

        let missing_claim_zero = [BasisTermV3 {
            claim_index: 1,
            shape: BasisShapeV3::Constant,
            amplitude: 1,
        }];
        let hostile = BasisInputV3 {
            basis_width: 3,
            knots: &[],
            terms: &missing_claim_zero,
            failure_payouts: &[0, 0, 100],
            ..graded_input(&[], &[], &[])
        };
        let width = basis_record_bytes_v3(hostile.kind, 3, 0, 1).expect("width");
        let mut output = vec![0x5a; width];
        assert_eq!(
            compile_basis_v3(hostile, &mut output),
            Err(Error::InvalidTerm)
        );
        assert!(output.iter().all(|byte| *byte == 0x5a));
    }

    /// **Curvature, end to end through the codec.**
    ///
    /// A degree-2 basis is compiled to `DCLTPAY3` bytes, decoded back through
    /// the hostile decoder, and evaluated at a rational coordinate -- the same
    /// three steps the two shipping kinds have always taken. This is the
    /// assertion the whole cut exists to make true, and every conjunct it
    /// passes through is one that refused before the seam flipped.
    #[test]
    fn a_degree_two_market_compiles_decodes_and_evaluates() {
        let knots = [0_i128, 0, 0, 1, 2, 3, 3, 3];
        let failure = [200_000_u64, 200_000, 200_000, 200_000, 200_000];
        let input = spline_input(&knots, &failure, 2);
        let bytes = compile(input);

        // The wire says what it is: kind 3, degree 2, cumulative-floor
        // rounding, and a certificate digest in the spent tail.
        assert_eq!(bytes[KIND_OFFSET], SPLINE_KIND);
        assert_eq!(bytes[BASIS_SPLINE_DEGREE_OFFSET_V3], 2);
        assert_eq!(bytes[BASIS_SPLINE_FLAGS_OFFSET_V3], 0);
        assert_eq!(bytes[ROUNDING_OFFSET], CUMULATIVE_FLOOR_BOUNDARY_V3);
        assert_eq!(
            read_array::<32>(&bytes, PRICE_GATE_DIGEST_OFFSET).expect("digest"),
            [3_u8; 32]
        );

        let basis = ProductBasisV3::decode(&bytes).expect("a curved record decodes");
        assert_eq!(
            basis.kind(),
            BasisKindV3::SplineDegree2To3 {
                degree: 2,
                interior_multiplicity: false
            }
        );
        assert_eq!(basis.basis_width(), 5);

        // It evaluates, at the blessed rounding, to an exact partition -- at
        // every coordinate, including well outside the knot domain, because
        // the admission envelope promised exactly that.
        for (numerator, denominator) in [
            (0_i128, 1_u64),
            (1, 1),
            (3, 2),
            (2, 1),
            (3, 1),
            (-9_999, 1),
            (i128::MAX, 1),
        ] {
            let mut output = vec![0_u64; 5];
            basis
                .evaluate_rational(numerator, denominator, &mut output)
                .unwrap_or_else(|error| panic!("at {numerator}/{denominator}: {error:?}"));
            assert_eq!(
                output.iter().sum::<u64>(),
                1_000_000,
                "the partition is exact at {numerator}/{denominator}"
            );
        }

        // And the explicit failure terminal pays the record's own vector.
        let mut output = vec![0_u64; 5];
        basis.evaluate_failure(&mut output).expect("failure pays");
        assert_eq!(output.as_slice(), failure.as_slice());
    }

    /// A degree-2 record that declares no interior multiplicity is refused
    /// when its knots repeat inside the domain -- and admitted when it
    /// declares it. The relaxation is a permission the record carries, not a
    /// property an evaluator infers.
    #[test]
    fn an_interior_repeat_needs_the_declaration() {
        let knots = [0_i128, 0, 0, 2, 2, 4, 4, 4];
        let failure = [200_000_u64, 200_000, 200_000, 200_000, 200_000];
        let mut output = [0_u8; 512];

        let undeclared = spline_input(&knots, &failure, 2);
        let sized = basis_record_bytes_v3(undeclared.kind, 5, 8, 0).expect("width");
        assert_eq!(
            compile_basis_v3(undeclared, output.get_mut(..sized).expect("buffer")),
            Err(Error::SplineInteriorMultiplicityUndeclared)
        );

        let declared = BasisInputV3 {
            kind: BasisKindV3::SplineDegree2To3 {
                degree: 2,
                interior_multiplicity: true,
            },
            ..spline_input(&knots, &failure, 2)
        };
        compile_basis_v3(declared, output.get_mut(..sized).expect("buffer"))
            .expect("a declared interior repeat compiles");
        let basis =
            ProductBasisV3::decode(output.get(..sized).expect("buffer")).expect("and decodes");
        assert_eq!(
            basis.kind(),
            BasisKindV3::SplineDegree2To3 {
                degree: 2,
                interior_multiplicity: true
            }
        );
        assert_eq!(
            basis.bytes[BASIS_SPLINE_FLAGS_OFFSET_V3],
            BASIS_SPLINE_INTERIOR_MULTIPLICITY_FLAG_V3
        );
    }
}
