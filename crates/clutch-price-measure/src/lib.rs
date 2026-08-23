#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact fixed-capacity certificates that a degree-two or degree-three
//! open-clamped uniform B-spline price vector comes from a nonnegative measure.
//!
//! The continuous checker consumes per-span Bernstein moments. The quantized
//! checker consumes a bounded mixture of integer resolved coordinates and
//! recomputes their production payout vectors. Both reconstruct every
//! simplex-price coordinate with exact integer arithmetic. This crate does not
//! parse Solana accounts, compute cryptographic digests, select candidates,
//! judge price quality beyond measure coherence, or determine fees, bonds, and
//! solver compensation.

use clutch_bspline::{BasisSpec, ValidatedBasisSpec};

/// Semantic version of the certificate and checker interface.
pub const PRICE_MEASURE_WITNESS_VERSION_V2: u8 = 2;
/// Version of the generated open-clamped uniform transfer tables.
pub const TRANSFER_TABLE_VERSION_V1: u8 = 1;
/// Largest admitted outcome width.
pub const MAX_OUTCOMES: usize = 16;
/// Largest span count: degree two at the maximum outcome width.
pub const MAX_SPANS: usize = MAX_OUTCOMES - 2;
/// Fixed stride for one degree-three Bernstein row.
pub const MOMENTS_PER_SPAN: usize = 4;
/// Fixed witness-body capacity.
pub const MAX_MOMENTS: usize = MAX_SPANS * MOMENTS_PER_SPAN;
/// Caratheodory support bound in the affine payout simplex.
pub const MAX_QUANTIZED_ATOMS: usize = MAX_OUTCOMES;
/// Largest generated transfer denominator.
pub const MAX_TRANSFER_DENOMINATOR: u64 = 12;
/// Largest representable common witness denominator.
///
/// Reconstruction compares reduced rational pairs rather than cross-products,
/// so every `u64` denominator is admitted. The finite width still makes this a
/// sufficient inner certificate until a denominator bound is proved complete.
pub const MAX_COMMON_DENOMINATOR: u64 = u64::MAX;

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_MOMENTS == 56);
const MAX_U64_AS_U128: u128 = (1_u128 << 64) - 1;
const _: () = assert!(12_u128 * MAX_U64_AS_U128 < u128::MAX);

/// Frozen basis semantics supported by the two certificate families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisSemanticsV2 {
    /// Continuous exact basis before any payout-weight quantization.
    ContinuousOpenClampedUniformV1,
    /// Integer-coordinate basis evaluated with frozen largest-remainder payouts.
    QuantizedIntegerGridV1,
    /// A caller-supplied transfer matrix is never trusted by this checker.
    CallerSuppliedTransferForbidden,
}

/// The sole price rounding boundary recognized by this checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriceRoundingBoundaryV2 {
    /// Prices were already quantized upstream and form an exact integer simplex.
    UpstreamExactSimplexV1,
    /// Rounding during certificate verification is forbidden.
    VerifierSideRoundingForbidden,
}

/// Payout-weight semantics against which price coherence is certified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayoutRoundingBoundaryV2 {
    /// Exact rational B-spline values with no payout-weight quantization.
    ExactUnquantizedV1,
    /// `clutch-bspline` largest remainder, exact ties to the lowest outcome.
    LargestRemainderLowestIndexV1,
    /// Any adapter-local or caller-selected rounding rule is forbidden.
    CallerDefinedForbidden,
}

/// An externally authenticated binding expected by the adapter.
///
/// The adapter derives these values from owner-checked accounts and computes
/// `observed_body_digest` over canonical certificate bytes excluding the digest
/// field. This arithmetic crate only compares the bytes; it deliberately
/// contains no hash primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterBindingsV2 {
    /// Candidate-feed account identity.
    pub candidate_feed: [u8; 32],
    /// Digest of immutable relation-domain semantics.
    pub relation_domain_digest: [u8; 32],
    /// Digest of the exact immutable basis, including knots, spacing, domain,
    /// edge policy, and payout denominator.
    pub basis_digest: [u8; 32],
    /// Digest of the exact candidate price vector.
    pub candidate_price_digest: [u8; 32],
    /// Digest recomputed over canonical witness bytes excluding the digest field.
    pub observed_body_digest: [u8; 32],
}

/// Exact already-quantized candidate prices checked by the certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceVectorV2 {
    /// B-spline basis degree; exactly two or three.
    pub basis_degree: u8,
    /// Active price prefix.
    pub outcome_count: u8,
    /// Positive integer simplex scale.
    pub price_scale: u64,
    /// Active prices summing exactly to `price_scale`, then zero padding.
    pub prices: [u64; MAX_OUTCOMES],
}

/// Fixed-capacity certificate body supplied by a candidate sidecar.
///
/// Moment row `k` starts at `4*k`. Degree two uses columns zero through two and
/// requires column three to be zero. All rows after `span_count` are zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuousPriceMeasureWitnessV2 {
    /// Exact [`PRICE_MEASURE_WITNESS_VERSION_V2`].
    pub schema_version: u8,
    /// Exact [`TRANSFER_TABLE_VERSION_V1`].
    pub transfer_table_version: u8,
    /// Frozen basis family; caller-supplied tables refuse.
    pub basis_semantics: BasisSemanticsV2,
    /// Named upstream-only price rounding boundary.
    pub price_rounding_boundary: PriceRoundingBoundaryV2,
    /// Exact/unquantized payout semantics required by the Bernstein witness.
    pub payout_rounding_boundary: PayoutRoundingBoundaryV2,
    /// Must repeat the authenticated candidate-feed identity.
    pub candidate_feed: [u8; 32],
    /// Must repeat the authenticated relation-domain digest.
    pub relation_domain_digest: [u8; 32],
    /// Must repeat the authenticated exact-basis digest.
    pub basis_digest: [u8; 32],
    /// Must repeat the authenticated exact-price digest.
    pub candidate_price_digest: [u8; 32],
    /// Digest of canonical body bytes excluding this digest field.
    pub body_digest: [u8; 32],
    /// Must repeat [`PriceVectorV2::basis_degree`].
    pub basis_degree: u8,
    /// Must repeat [`PriceVectorV2::outcome_count`].
    pub outcome_count: u8,
    /// Must equal `outcome_count - basis_degree`.
    pub span_count: u8,
    /// Positive primitive common denominator in the full `u64` range.
    pub common_denominator: u64,
    /// Per-span Bernstein moments in fixed stride-four layout.
    pub moments: [u64; MAX_MOMENTS],
}

/// Finite mixture certificate for the current integer-coordinate quantized
/// payout semantics.
///
/// The active atoms are strictly coordinate-sorted, have positive masses, and
/// are followed by zero padding. At most `outcome_count` atoms are required by
/// Caratheodory's theorem because every payout vector lies in the affine
/// `sum(weights) = payout_denominator` hyperplane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomWitnessV2 {
    /// Exact [`PRICE_MEASURE_WITNESS_VERSION_V2`].
    pub schema_version: u8,
    /// Current integer-coordinate quantized basis semantics.
    pub basis_semantics: BasisSemanticsV2,
    /// Named upstream-only candidate-price rounding boundary.
    pub price_rounding_boundary: PriceRoundingBoundaryV2,
    /// Frozen production payout quantizer.
    pub payout_rounding_boundary: PayoutRoundingBoundaryV2,
    /// Must repeat the authenticated candidate-feed identity.
    pub candidate_feed: [u8; 32],
    /// Must repeat the authenticated relation-domain digest.
    pub relation_domain_digest: [u8; 32],
    /// Must repeat the authenticated exact-basis digest.
    pub basis_digest: [u8; 32],
    /// Must repeat the authenticated exact-price digest.
    pub candidate_price_digest: [u8; 32],
    /// Digest of canonical body bytes excluding this digest field.
    pub body_digest: [u8; 32],
    /// Number of active sorted atoms in `1..=outcome_count`.
    pub atom_count: u8,
    /// Primitive positive common denominator of the atom masses.
    pub common_denominator: u64,
    /// Integer resolved coordinates in strictly ascending active order.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Positive active masses summing to `common_denominator`.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

/// One generated local B-spline-to-Bernstein transfer matrix.
///
/// Rows are the `degree + 1` outcome coordinates beginning at
/// `first_outcome`; columns are the local Bernstein moments. Inactive rows and
/// columns are canonical zeroes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferSpanV2 {
    /// First globally indexed outcome active on this span.
    pub first_outcome: u8,
    /// Common positive denominator of every numerator.
    pub denominator: u8,
    /// Nonnegative transfer numerators.
    pub numerators: [[u8; MOMENTS_PER_SPAN]; MOMENTS_PER_SPAN],
}

/// Successful checked-certificate summary suitable for an adapter checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedPriceMeasureV2 {
    /// Checked basis degree.
    pub basis_degree: u8,
    /// Checked active outcome width.
    pub outcome_count: u8,
    /// Checked active span count.
    pub span_count: u8,
    /// Primitive checked witness denominator.
    pub common_denominator: u64,
    /// Adapter-authenticated canonical body digest.
    pub body_digest: [u8; 32],
}

/// Validated append-only verifier for one quantized atom certificate.
///
/// [`Self::begin`] validates the complete immutable basis, candidate price,
/// certificate header, atom order, coordinate bounds, mass, primitive scale,
/// and canonical padding exactly once. Each [`Self::accumulate_atom`] then
/// evaluates one exact atom through the captured [`ValidatedBasisSpec`].
/// [`Self::finish`] succeeds only after the exact active atom prefix has been
/// consumed and the accumulated mixture reconstructs every candidate price.
///
/// Fields are private so a caller cannot forge a cursor, partial sum, or
/// validated-basis capability. This value is an in-memory arithmetic state,
/// not a canonical account layout or persistence codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuantizedPriceMeasureAccumulatorV2 {
    basis: ValidatedBasisSpec,
    prices: PriceVectorV2,
    witness: QuantizedAtomWitnessV2,
    atom_cursor: u8,
    accumulators: [u128; MAX_OUTCOMES],
}

/// Binding coordinate that did not match adapter-authenticated truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingFieldV2 {
    /// Candidate-feed identity.
    CandidateFeed,
    /// Relation-domain digest.
    RelationDomainDigest,
    /// Exact immutable basis digest.
    BasisDigest,
    /// Exact-price digest.
    CandidatePriceDigest,
    /// Canonical witness-body digest.
    BodyDigest,
}

/// Side of the cubic Hausdorff system that failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CubicConstraintV2 {
    /// `w1^2 <= 3*w0*w2`.
    Left,
    /// `w2^2 <= 3*w1*w3`.
    Right,
}

/// Total hostile-input refusal set for certificate checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV2 {
    /// Certificate schema version differs from the frozen interface.
    UnsupportedSchemaVersion,
    /// Transfer-table version differs from the generated table family.
    UnsupportedTransferTableVersion,
    /// Basis semantics are not the generated open-clamped uniform family.
    UnsupportedBasisSemantics,
    /// The checker was asked to round rather than compare exact integers.
    UnsupportedPriceRoundingBoundary,
    /// Payout quantization semantics do not match the selected checker.
    UnsupportedPayoutRoundingBoundary,
    /// An adapter-authenticated identity or digest did not match.
    BindingMismatch {
        /// Mismatching authenticated coordinate.
        field: BindingFieldV2,
    },
    /// Basis degree was not two or three.
    InvalidDegree,
    /// Outcome width was outside `degree + 1 ..= 16`.
    InvalidOutcomeCount,
    /// A continuous witness repeated a different degree or outcome width.
    ContinuousWitnessShapeMismatch,
    /// The immutable basis failed its own total hostile-input validation.
    InvalidBasis,
    /// Certificate span count did not equal `outcome_count - degree`.
    InvalidSpanCount,
    /// Price scale was zero.
    InvalidPriceScale,
    /// An active price exceeded the simplex scale.
    PriceExceedsScale {
        /// First active outcome exceeding the scale.
        outcome: u8,
    },
    /// Active prices did not sum exactly to the simplex scale.
    PriceSimplexMismatch,
    /// An inactive price was nonzero.
    NonCanonicalPricePadding {
        /// First nonzero inactive outcome.
        outcome: u8,
    },
    /// Common witness denominator was zero.
    InvalidCommonDenominator,
    /// Active atom count was zero or exceeded the affine support bound.
    InvalidAtomCount,
    /// An active atom coordinate lay outside the canonical knot span.
    AtomCoordinateOutOfRange {
        /// First out-of-range atom.
        atom: u8,
    },
    /// Active atom coordinates were not strictly increasing.
    NonCanonicalAtomOrder {
        /// First atom not strictly above its predecessor.
        atom: u8,
    },
    /// An active atom mass was zero.
    ZeroAtomMass {
        /// First active zero-mass atom.
        atom: u8,
    },
    /// An inactive atom coordinate or mass was nonzero.
    NonCanonicalAtomPadding {
        /// First nonzero inactive atom slot.
        atom: u8,
    },
    /// Active atom mass did not equal the common denominator.
    AtomMassMismatch,
    /// Denominator and atom masses shared a nontrivial common divisor.
    NonPrimitiveAtomScale,
    /// A staged atom did not equal the exact next active atom.
    AtomCursorMismatch {
        /// Exact next atom required by the accumulator.
        expected: u8,
        /// Atom cursor supplied by the caller.
        provided: u8,
    },
    /// A staged append was attempted after every active atom was consumed.
    AtomCursorExhausted,
    /// Staged reconstruction was requested before every active atom was consumed.
    IncompleteAtomAccumulation {
        /// Exact next atom that remained to be consumed.
        cursor: u8,
        /// Total active atom count committed by the witness.
        atom_count: u8,
    },
    /// An inactive moment cell was nonzero.
    NonCanonicalMomentPadding {
        /// First nonzero inactive moment cell.
        cell: usize,
    },
    /// Active moment mass did not sum exactly to the common denominator.
    MomentMassMismatch,
    /// Denominator and active moments shared a nontrivial common divisor.
    NonPrimitiveMomentScale,
    /// A degree-two span failed `w1^2 <= 4*w0*w2`.
    QuadraticMomentOutsideCone {
        /// First failing span.
        span: u8,
    },
    /// A degree-three span failed one truncated Hausdorff constraint.
    CubicMomentOutsideCone {
        /// Failing span.
        span: u8,
        /// Failing left or right constraint.
        constraint: CubicConstraintV2,
    },
    /// Exact reconstruction disagreed with a candidate price.
    PriceReconstructionMismatch {
        /// First mismatching outcome.
        outcome: u8,
    },
    /// A checked integer operation overflowed despite the validated envelope.
    ArithmeticOverflow,
}

/// Result alias for certificate operations.
pub type Result<T> = core::result::Result<T, ErrorV2>;

impl QuantizedPriceMeasureAccumulatorV2 {
    /// Validate one complete quantized certificate and open its empty staged sum.
    ///
    /// Refusal order through this constructor is version and policy, bindings,
    /// immutable basis and shape, price simplex and padding, then atom count,
    /// denominator, coordinate range and order, mass, padding, and primitive
    /// scale. No atom is evaluated until every structural check succeeds.
    pub fn begin(
        expected: &AdapterBindingsV2,
        basis: &BasisSpec,
        prices: &PriceVectorV2,
        witness: &QuantizedAtomWitnessV2,
    ) -> Result<Self> {
        let basis = validate_quantized_header(expected, basis, prices, witness)?;
        let outcomes = usize::from(prices.outcome_count);
        validate_prices(prices, outcomes)?;
        validate_atoms(&basis.spec(), prices.outcome_count, witness)?;
        Ok(Self {
            basis,
            prices: *prices,
            witness: *witness,
            atom_cursor: 0,
            accumulators: [0; MAX_OUTCOMES],
        })
    }

    /// Exact next active atom required by [`Self::accumulate_atom`].
    pub const fn atom_cursor(&self) -> u8 {
        self.atom_cursor
    }

    /// Total active atom count committed by the validated witness.
    pub const fn atom_count(&self) -> u8 {
        self.witness.atom_count
    }

    /// Evaluate and accumulate exactly the next certified atom.
    ///
    /// `atom` must equal [`Self::atom_cursor`]. A skipped, replayed, or
    /// post-completion cursor refuses. Every checked evaluation and component
    /// addition is staged locally, so a refusal leaves `self` unchanged.
    pub fn accumulate_atom(&mut self, atom: u8) -> Result<()> {
        if atom != self.atom_cursor {
            return Err(ErrorV2::AtomCursorMismatch {
                expected: self.atom_cursor,
                provided: atom,
            });
        }
        if atom >= self.witness.atom_count {
            return Err(ErrorV2::AtomCursorExhausted);
        }
        let index = usize::from(atom);
        let weights = self
            .basis
            .evaluate_point(self.witness.atom_coordinates[index])
            .map_err(|_| ErrorV2::InvalidBasis)?;
        let mass = u128::from(self.witness.atom_masses[index]);
        let mut accumulators = self.accumulators;
        let mut outcome = 0_usize;
        while outcome < usize::from(self.prices.outcome_count) {
            let term = mass
                .checked_mul(u128::from(weights.weights[outcome]))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            accumulators[outcome] = accumulators[outcome]
                .checked_add(term)
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            outcome += 1;
        }
        let next = atom.checked_add(1).ok_or(ErrorV2::ArithmeticOverflow)?;
        self.accumulators = accumulators;
        self.atom_cursor = next;
        Ok(())
    }

    /// Finish exact reconstruction after consuming every active atom.
    ///
    /// This consumes the accumulator. An incomplete walk refuses before any
    /// price comparison; a complete walk compares reduced exact rational pairs
    /// in ascending outcome order and performs no rounding.
    pub fn finish(self) -> Result<VerifiedPriceMeasureV2> {
        if self.atom_cursor != self.witness.atom_count {
            return Err(ErrorV2::IncompleteAtomAccumulation {
                cursor: self.atom_cursor,
                atom_count: self.witness.atom_count,
            });
        }
        let spec = self.basis.spec();
        let witness_scale = u128::from(spec.denominator)
            .checked_mul(u128::from(self.witness.common_denominator))
            .ok_or(ErrorV2::ArithmeticOverflow)?;
        let mut outcome = 0_u8;
        while outcome < self.prices.outcome_count {
            let index = usize::from(outcome);
            if !ratios_equal(
                u128::from(self.prices.prices[index]),
                u128::from(self.prices.price_scale),
                self.accumulators[index],
                witness_scale,
            ) {
                return Err(ErrorV2::PriceReconstructionMismatch { outcome });
            }
            outcome += 1;
        }
        Ok(VerifiedPriceMeasureV2 {
            basis_degree: self.prices.basis_degree,
            outcome_count: self.prices.outcome_count,
            span_count: self.prices.outcome_count - self.prices.basis_degree,
            common_denominator: self.witness.common_denominator,
            body_digest: self.witness.body_digest,
        })
    }
}

/// Verify the complete adapter-bound price-measure certificate.
///
/// Refusal order is version and policy, bindings, shape, price simplex and
/// padding, denominator and moment padding/mass/primitive scale, per-span
/// Hausdorff constraints, then exact reconstruction in outcome order.
pub fn verify_continuous_price_measure_v2(
    expected: &AdapterBindingsV2,
    prices: &PriceVectorV2,
    witness: &ContinuousPriceMeasureWitnessV2,
) -> Result<VerifiedPriceMeasureV2> {
    validate_header(expected, prices, witness)?;
    let degree = usize::from(prices.basis_degree);
    let outcomes = usize::from(prices.outcome_count);
    let spans = outcomes - degree;
    validate_prices(prices, outcomes)?;
    validate_moments(witness, degree, spans)?;
    validate_span_constraints(witness, degree, spans)?;
    reconstruct_prices(prices, witness, degree, outcomes, spans)?;
    Ok(VerifiedPriceMeasureV2 {
        basis_degree: prices.basis_degree,
        outcome_count: prices.outcome_count,
        span_count: witness.span_count,
        common_denominator: witness.common_denominator,
        body_digest: witness.body_digest,
    })
}

/// Verify a support-bounded measure over the current production payout vectors.
///
/// `basis` must be the owner-checked immutable `BasisSpec` whose canonical byte
/// digest equals `expected.basis_digest`. This function delegates to
/// [`QuantizedPriceMeasureAccumulatorV2`], validating the basis once and then
/// evaluating every certified integer coordinate through `clutch-bspline`.
pub fn verify_quantized_price_measure_v2(
    expected: &AdapterBindingsV2,
    basis: &BasisSpec,
    prices: &PriceVectorV2,
    witness: &QuantizedAtomWitnessV2,
) -> Result<VerifiedPriceMeasureV2> {
    let mut accumulator =
        QuantizedPriceMeasureAccumulatorV2::begin(expected, basis, prices, witness)?;
    while accumulator.atom_cursor() < accumulator.atom_count() {
        let atom = accumulator.atom_cursor();
        accumulator.accumulate_atom(atom)?;
    }
    accumulator.finish()
}

/// Return the generated exact transfer matrix for one span.
///
/// The table is derived from the canonical open-clamped uniform B-spline
/// recurrence. Degree two uses denominator 2; degree three uses denominator
/// 12. No caller-provided coefficient is accepted.
pub fn transfer_span_v2(degree: u8, outcome_count: u8, span: u8) -> Result<TransferSpanV2> {
    validate_shape(degree, outcome_count)?;
    let spans = outcome_count - degree;
    if span >= spans {
        return Err(ErrorV2::InvalidSpanCount);
    }
    let numerators = match degree {
        2 => degree_two_table(spans, span),
        3 => degree_three_table(spans, span),
        _ => return Err(ErrorV2::InvalidDegree),
    };
    Ok(TransferSpanV2 {
        first_outcome: span,
        denominator: transfer_denominator(degree)?,
        numerators,
    })
}

fn validate_header(
    expected: &AdapterBindingsV2,
    prices: &PriceVectorV2,
    witness: &ContinuousPriceMeasureWitnessV2,
) -> Result<()> {
    if witness.schema_version != PRICE_MEASURE_WITNESS_VERSION_V2 {
        return Err(ErrorV2::UnsupportedSchemaVersion);
    }
    if witness.transfer_table_version != TRANSFER_TABLE_VERSION_V1 {
        return Err(ErrorV2::UnsupportedTransferTableVersion);
    }
    if witness.basis_semantics != BasisSemanticsV2::ContinuousOpenClampedUniformV1 {
        return Err(ErrorV2::UnsupportedBasisSemantics);
    }
    if witness.price_rounding_boundary != PriceRoundingBoundaryV2::UpstreamExactSimplexV1 {
        return Err(ErrorV2::UnsupportedPriceRoundingBoundary);
    }
    if witness.payout_rounding_boundary != PayoutRoundingBoundaryV2::ExactUnquantizedV1 {
        return Err(ErrorV2::UnsupportedPayoutRoundingBoundary);
    }
    for (matches, field) in [
        (
            witness.candidate_feed == expected.candidate_feed,
            BindingFieldV2::CandidateFeed,
        ),
        (
            witness.relation_domain_digest == expected.relation_domain_digest,
            BindingFieldV2::RelationDomainDigest,
        ),
        (
            witness.basis_digest == expected.basis_digest,
            BindingFieldV2::BasisDigest,
        ),
        (
            witness.candidate_price_digest == expected.candidate_price_digest,
            BindingFieldV2::CandidatePriceDigest,
        ),
        (
            witness.body_digest == expected.observed_body_digest,
            BindingFieldV2::BodyDigest,
        ),
    ] {
        if !matches {
            return Err(ErrorV2::BindingMismatch { field });
        }
    }
    validate_shape(prices.basis_degree, prices.outcome_count)?;
    if witness.basis_degree != prices.basis_degree || witness.outcome_count != prices.outcome_count
    {
        return Err(ErrorV2::ContinuousWitnessShapeMismatch);
    }
    let expected_spans = prices.outcome_count - prices.basis_degree;
    if witness.span_count != expected_spans {
        return Err(ErrorV2::InvalidSpanCount);
    }
    Ok(())
}

fn validate_quantized_header(
    expected: &AdapterBindingsV2,
    basis: &BasisSpec,
    prices: &PriceVectorV2,
    witness: &QuantizedAtomWitnessV2,
) -> Result<ValidatedBasisSpec> {
    if witness.schema_version != PRICE_MEASURE_WITNESS_VERSION_V2 {
        return Err(ErrorV2::UnsupportedSchemaVersion);
    }
    if witness.basis_semantics != BasisSemanticsV2::QuantizedIntegerGridV1 {
        return Err(ErrorV2::UnsupportedBasisSemantics);
    }
    if witness.price_rounding_boundary != PriceRoundingBoundaryV2::UpstreamExactSimplexV1 {
        return Err(ErrorV2::UnsupportedPriceRoundingBoundary);
    }
    if witness.payout_rounding_boundary != PayoutRoundingBoundaryV2::LargestRemainderLowestIndexV1 {
        return Err(ErrorV2::UnsupportedPayoutRoundingBoundary);
    }
    for (matches, field) in [
        (
            witness.candidate_feed == expected.candidate_feed,
            BindingFieldV2::CandidateFeed,
        ),
        (
            witness.relation_domain_digest == expected.relation_domain_digest,
            BindingFieldV2::RelationDomainDigest,
        ),
        (
            witness.basis_digest == expected.basis_digest,
            BindingFieldV2::BasisDigest,
        ),
        (
            witness.candidate_price_digest == expected.candidate_price_digest,
            BindingFieldV2::CandidatePriceDigest,
        ),
        (
            witness.body_digest == expected.observed_body_digest,
            BindingFieldV2::BodyDigest,
        ),
    ] {
        if !matches {
            return Err(ErrorV2::BindingMismatch { field });
        }
    }
    let validated_basis = basis.validated().map_err(|_| ErrorV2::InvalidBasis)?;
    validate_shape(prices.basis_degree, prices.outcome_count)?;
    if basis.degree != prices.basis_degree || basis.outcome_count != prices.outcome_count {
        return Err(ErrorV2::InvalidBasis);
    }
    Ok(validated_basis)
}

fn validate_shape(degree: u8, outcome_count: u8) -> Result<()> {
    if degree != 2 && degree != 3 {
        return Err(ErrorV2::InvalidDegree);
    }
    if outcome_count < degree + 1 || usize::from(outcome_count) > MAX_OUTCOMES {
        return Err(ErrorV2::InvalidOutcomeCount);
    }
    Ok(())
}

fn validate_prices(prices: &PriceVectorV2, outcomes: usize) -> Result<()> {
    if prices.price_scale == 0 {
        return Err(ErrorV2::InvalidPriceScale);
    }
    let mut sum = 0_u128;
    let mut outcome = 0_u8;
    while usize::from(outcome) < MAX_OUTCOMES {
        let price = prices.prices[usize::from(outcome)];
        if usize::from(outcome) < outcomes {
            if price > prices.price_scale {
                return Err(ErrorV2::PriceExceedsScale { outcome });
            }
            sum = sum
                .checked_add(u128::from(price))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
        } else if price != 0 {
            return Err(ErrorV2::NonCanonicalPricePadding { outcome });
        }
        outcome += 1;
    }
    if sum != u128::from(prices.price_scale) {
        return Err(ErrorV2::PriceSimplexMismatch);
    }
    Ok(())
}

fn validate_moments(
    witness: &ContinuousPriceMeasureWitnessV2,
    degree: usize,
    spans: usize,
) -> Result<()> {
    let denominator = witness.common_denominator;
    if denominator == 0 {
        return Err(ErrorV2::InvalidCommonDenominator);
    }
    let mut total = 0_u128;
    let mut divisor = denominator;
    let mut cell = 0_usize;
    while cell < MAX_MOMENTS {
        let span = cell / MOMENTS_PER_SPAN;
        let local = cell % MOMENTS_PER_SPAN;
        let value = witness.moments[cell];
        if span < spans && local <= degree {
            total = total
                .checked_add(u128::from(value))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            divisor = gcd(divisor, value);
        } else if value != 0 {
            return Err(ErrorV2::NonCanonicalMomentPadding { cell });
        }
        cell += 1;
    }
    if total != u128::from(denominator) {
        return Err(ErrorV2::MomentMassMismatch);
    }
    if divisor != 1 {
        return Err(ErrorV2::NonPrimitiveMomentScale);
    }
    Ok(())
}

fn validate_atoms(
    basis: &BasisSpec,
    outcome_count: u8,
    witness: &QuantizedAtomWitnessV2,
) -> Result<()> {
    let atoms = usize::from(witness.atom_count);
    if atoms == 0 || atoms > usize::from(outcome_count) || atoms > MAX_QUANTIZED_ATOMS {
        return Err(ErrorV2::InvalidAtomCount);
    }
    if witness.common_denominator == 0 {
        return Err(ErrorV2::InvalidCommonDenominator);
    }
    let first = basis.knots[0];
    let last = basis.knots[usize::from(basis.knot_count) - 1];
    let mut total = 0_u128;
    let mut divisor = witness.common_denominator;
    let mut atom = 0_u8;
    while usize::from(atom) < MAX_QUANTIZED_ATOMS {
        let index = usize::from(atom);
        let coordinate = witness.atom_coordinates[index];
        let mass = witness.atom_masses[index];
        if index < atoms {
            if coordinate < first || coordinate > last {
                return Err(ErrorV2::AtomCoordinateOutOfRange { atom });
            }
            if index != 0 && coordinate <= witness.atom_coordinates[index - 1] {
                return Err(ErrorV2::NonCanonicalAtomOrder { atom });
            }
            if mass == 0 {
                return Err(ErrorV2::ZeroAtomMass { atom });
            }
            total = total
                .checked_add(u128::from(mass))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            divisor = gcd(divisor, mass);
        } else if coordinate != 0 || mass != 0 {
            return Err(ErrorV2::NonCanonicalAtomPadding { atom });
        }
        atom += 1;
    }
    if total != u128::from(witness.common_denominator) {
        return Err(ErrorV2::AtomMassMismatch);
    }
    if divisor != 1 {
        return Err(ErrorV2::NonPrimitiveAtomScale);
    }
    Ok(())
}

fn validate_span_constraints(
    witness: &ContinuousPriceMeasureWitnessV2,
    degree: usize,
    spans: usize,
) -> Result<()> {
    let mut span = 0_u8;
    while usize::from(span) < spans {
        let offset = usize::from(span) * MOMENTS_PER_SPAN;
        let w0 = u128::from(witness.moments[offset]);
        let w1 = u128::from(witness.moments[offset + 1]);
        let w2 = u128::from(witness.moments[offset + 2]);
        if degree == 2 {
            let left = w1.checked_mul(w1).ok_or(ErrorV2::ArithmeticOverflow)?;
            let right = w0
                .checked_mul(w2)
                .and_then(|value| value.checked_mul(4))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            if left > right {
                return Err(ErrorV2::QuadraticMomentOutsideCone { span });
            }
        } else {
            let w3 = u128::from(witness.moments[offset + 3]);
            let left_square = w1.checked_mul(w1).ok_or(ErrorV2::ArithmeticOverflow)?;
            let left_product = w0
                .checked_mul(w2)
                .and_then(|value| value.checked_mul(3))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            if left_square > left_product {
                return Err(ErrorV2::CubicMomentOutsideCone {
                    span,
                    constraint: CubicConstraintV2::Left,
                });
            }
            let right_square = w2.checked_mul(w2).ok_or(ErrorV2::ArithmeticOverflow)?;
            let right_product = w1
                .checked_mul(w3)
                .and_then(|value| value.checked_mul(3))
                .ok_or(ErrorV2::ArithmeticOverflow)?;
            if right_square > right_product {
                return Err(ErrorV2::CubicMomentOutsideCone {
                    span,
                    constraint: CubicConstraintV2::Right,
                });
            }
        }
        span += 1;
    }
    Ok(())
}

fn reconstruct_prices(
    prices: &PriceVectorV2,
    witness: &ContinuousPriceMeasureWitnessV2,
    degree: usize,
    outcomes: usize,
    spans: usize,
) -> Result<()> {
    let mut accumulators = [0_u128; MAX_OUTCOMES];
    let mut span = 0_u8;
    while usize::from(span) < spans {
        let table = transfer_span_v2(prices.basis_degree, prices.outcome_count, span)?;
        let moment_offset = usize::from(span) * MOMENTS_PER_SPAN;
        let mut local_outcome = 0_usize;
        while local_outcome <= degree {
            let outcome = usize::from(span) + local_outcome;
            let mut local_moment = 0_usize;
            while local_moment <= degree {
                let term = u128::from(table.numerators[local_outcome][local_moment])
                    .checked_mul(u128::from(witness.moments[moment_offset + local_moment]))
                    .ok_or(ErrorV2::ArithmeticOverflow)?;
                accumulators[outcome] = accumulators[outcome]
                    .checked_add(term)
                    .ok_or(ErrorV2::ArithmeticOverflow)?;
                local_moment += 1;
            }
            local_outcome += 1;
        }
        span += 1;
    }

    let denominator = u128::from(transfer_denominator(prices.basis_degree)?);
    let witness_denominator = u128::from(witness.common_denominator);
    let witness_scale = denominator
        .checked_mul(witness_denominator)
        .ok_or(ErrorV2::ArithmeticOverflow)?;
    let mut outcome = 0_u8;
    while usize::from(outcome) < outcomes {
        let index = usize::from(outcome);
        if !ratios_equal(
            u128::from(prices.prices[index]),
            u128::from(prices.price_scale),
            accumulators[index],
            witness_scale,
        ) {
            return Err(ErrorV2::PriceReconstructionMismatch { outcome });
        }
        outcome += 1;
    }
    Ok(())
}

fn transfer_denominator(degree: u8) -> Result<u8> {
    match degree {
        2 => Ok(2),
        3 => Ok(12),
        _ => Err(ErrorV2::InvalidDegree),
    }
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn ratios_equal(
    left_numerator: u128,
    left_denominator: u128,
    right_numerator: u128,
    right_denominator: u128,
) -> bool {
    let left_divisor = gcd_u128(left_numerator, left_denominator);
    let right_divisor = gcd_u128(right_numerator, right_denominator);
    left_numerator / left_divisor == right_numerator / right_divisor
        && left_denominator / left_divisor == right_denominator / right_divisor
}

type Matrix = [[u8; MOMENTS_PER_SPAN]; MOMENTS_PER_SPAN];

const D2_SINGLE: Matrix = [[2, 0, 0, 0], [0, 2, 0, 0], [0, 0, 2, 0], [0, 0, 0, 0]];
const D2_LEFT: Matrix = [[2, 0, 0, 0], [0, 2, 1, 0], [0, 0, 1, 0], [0, 0, 0, 0]];
const D2_INTERIOR: Matrix = [[1, 0, 0, 0], [1, 2, 1, 0], [0, 0, 1, 0], [0, 0, 0, 0]];
const D2_RIGHT: Matrix = [[1, 0, 0, 0], [1, 2, 0, 0], [0, 0, 2, 0], [0, 0, 0, 0]];

fn degree_two_table(spans: u8, span: u8) -> Matrix {
    if spans == 1 {
        D2_SINGLE
    } else if span == 0 {
        D2_LEFT
    } else if span + 1 == spans {
        D2_RIGHT
    } else {
        D2_INTERIOR
    }
}

const D3_SINGLE: Matrix = [[12, 0, 0, 0], [0, 12, 0, 0], [0, 0, 12, 0], [0, 0, 0, 12]];
const D3_TWO_LEFT: Matrix = [[12, 0, 0, 0], [0, 12, 6, 3], [0, 0, 6, 6], [0, 0, 0, 3]];
const D3_TWO_RIGHT: Matrix = [[3, 0, 0, 0], [6, 6, 0, 0], [3, 6, 12, 0], [0, 0, 0, 12]];
const D3_THREE_LEFT: Matrix = [[12, 0, 0, 0], [0, 12, 6, 3], [0, 0, 6, 7], [0, 0, 0, 2]];
const D3_THREE_MIDDLE: Matrix = [[3, 0, 0, 0], [7, 8, 4, 2], [2, 4, 8, 7], [0, 0, 0, 3]];
const D3_THREE_RIGHT: Matrix = [[2, 0, 0, 0], [7, 6, 0, 0], [3, 6, 12, 0], [0, 0, 0, 12]];
const D3_LEFT_EDGE: Matrix = D3_THREE_LEFT;
const D3_LEFT_NEAR: Matrix = [[3, 0, 0, 0], [7, 8, 4, 2], [2, 4, 8, 8], [0, 0, 0, 2]];
const D3_INTERIOR: Matrix = [[2, 0, 0, 0], [8, 8, 4, 2], [2, 4, 8, 8], [0, 0, 0, 2]];
const D3_RIGHT_NEAR: Matrix = [[2, 0, 0, 0], [8, 8, 4, 2], [2, 4, 8, 7], [0, 0, 0, 3]];
const D3_RIGHT_EDGE: Matrix = D3_THREE_RIGHT;

fn degree_three_table(spans: u8, span: u8) -> Matrix {
    match spans {
        1 => D3_SINGLE,
        2 => {
            if span == 0 {
                D3_TWO_LEFT
            } else {
                D3_TWO_RIGHT
            }
        }
        3 => match span {
            0 => D3_THREE_LEFT,
            1 => D3_THREE_MIDDLE,
            _ => D3_THREE_RIGHT,
        },
        _ => {
            if span == 0 {
                D3_LEFT_EDGE
            } else if span == 1 {
                D3_LEFT_NEAR
            } else if span + 2 == spans {
                D3_RIGHT_NEAR
            } else if span + 1 == spans {
                D3_RIGHT_EDGE
            } else {
                D3_INTERIOR
            }
        }
    }
}
